/* WUT owns GX2 display targets. Rust owns the launcher and session state. */
#include "hud.h"

#include <coreinit/debug.h>
#include <coreinit/foreground.h>
#include <coreinit/thread.h>
#include <coreinit/time.h>
#include <gx2/surface.h>
#include <proc_ui/procui.h>
#include <vpad/input.h>
#include <whb/gfx.h>
#ifdef HORIZON_WIIU_HOSTED
#include <nn/ac.h>
#include "gx2_profile.h"
#endif

enum { VIEW_WIDTH = 1280, VIEW_HEIGHT = 720, VPAD_CAPACITY = 16 };
typedef struct LauncherSample { uint32_t buttons, touch, x, y; } LauncherSample;
static HorizonGx2Hud *view;
#ifdef HORIZON_WIIU_HOSTED
static bool game_focused;
static bool game_release_pending;
static bool app_exiting;
extern void horizon_wiiu_app_entropy_first(void);
extern void horizon_wiiu_memory_init(void);
extern void horizon_wiiu_memory_report_display_surfaces(void);
extern int32_t horizon_swkbd_is_open(void);
extern int32_t horizon_swkbd_calc(const void *status);
extern int32_t horizon_swkbd_draw(uint32_t target);
#endif
extern const unsigned char _binary_pregame_gsh_start[];
extern int32_t horizon_pregame_run(void);

void shell_trace(const char *message)
{
   OSReport("Horizon pregame: %s\n", message);
}

void shell_measure(uint32_t tv, uint32_t drc, uint32_t atlas)
{
   OSReport("Horizon pregame metrics: tv_vertices=%u drc_vertices=%u atlas_bytes=%u cpu_quad_bytes=%u\n",
            tv, drc, atlas, (uint32_t)((tv + drc) * sizeof(HorizonHudVertex)));
}

static void saves_ready(void)
{
   OSSavesDone_ReadyToRelease();
}

int32_t shell_process(void)
{
#ifdef HORIZON_WIIU_HOSTED
   if (app_exiting)
      return 2;
#endif
   switch (ProcUIProcessMessages(TRUE)) {
      case PROCUI_STATUS_IN_FOREGROUND: return 1;
      case PROCUI_STATUS_EXITING:
#ifdef HORIZON_WIIU_HOSTED
         app_exiting = true;
#endif
         return 2;
      case PROCUI_STATUS_RELEASE_FOREGROUND: return 3;
      default:
         OSSleepTicks(OSMillisecondsToTicks(16));
         return 0;
   }
}

void shell_release(void)
{
   ProcUIDrawDoneRelease();
}

#ifdef HORIZON_WIIU_HOSTED
/* Cafe network bridge used by the existing authenticated LAN service. */
int32_t horizon_wiiu_assigned_ipv4(uint32_t *out)
{
   return NNResult_IsSuccess(ACGetAssignedAddress(out)) ? 0 : -1;
}

/* Both front ends use this one application pump, on the same thread. */
int32_t horizon_wiiu_app_is_running(void)
{
   const int32_t status = shell_process();
   game_focused = status == 1;
   game_release_pending = status == 3;
   return status != 2;
}

int32_t horizon_wiiu_app_is_focused(void)
{
   return game_focused;
}

void horizon_wiiu_hosted_ack_release(void)
{
   if (game_release_pending) {
      game_release_pending = false;
      shell_release();
   }
}

void shell_destroy_view(void)
{
   horizon_gx2_hud_destroy(view);
   view = NULL;
}
#endif

static uint32_t map_buttons(uint32_t buttons)
{
   return (buttons & VPAD_BUTTON_UP ? 1 : 0) |
          (buttons & VPAD_BUTTON_DOWN ? 2 : 0) |
          (buttons & VPAD_BUTTON_A ? 4 : 0) |
          (buttons & VPAD_BUTTON_B ? 8 : 0) |
          (buttons & VPAD_BUTTON_LEFT ? 16 : 0) |
          (buttons & VPAD_BUTTON_RIGHT ? 32 : 0);
}

int32_t shell_poll_input(LauncherSample *samples)
{
#ifdef HORIZON_WIIU_HOSTED
   horizon_gx2_profile_begin(GX2_PROFILE_NIGHT);
   const uint64_t input_stamp = horizon_gx2_profile_stamp();
#endif
   VPADStatus batch[VPAD_CAPACITY];
   VPADReadError error = VPAD_READ_SUCCESS;
   const int32_t count = VPADRead(VPAD_CHAN_0, batch, VPAD_CAPACITY, &error);
#ifdef HORIZON_WIIU_HOSTED
   horizon_gx2_profile_end(GX2_PROFILE_INPUT, input_stamp);
   if (horizon_swkbd_is_open()) {
      /* Back out even if the system keyboard accepts no input in the emulator. */
      if (error == VPAD_READ_SUCCESS && count > 0 && (batch[0].trigger & VPAD_BUTTON_MINUS)) {
         samples[0] = (LauncherSample){.buttons = 64};
         return 1;
      }
      horizon_swkbd_calc(error == VPAD_READ_SUCCESS && count > 0 ? &batch[0] : NULL);
      return 0;
   }
#endif
   if (error == VPAD_READ_NO_SAMPLES || error == VPAD_READ_BUSY)
      return 0;
   if (error != VPAD_READ_SUCCESS || count < 0 || count > VPAD_CAPACITY)
      return -1;

   /* WUT supplies newest first. Process all hardware samples in order. */
   for (int32_t index = 0; index < count; ++index) {
      const VPADStatus *sample = &batch[count - 1 - index];
      LauncherSample out = {.buttons = map_buttons(sample->hold)};
      if (sample->tpNormal.touched) {
         VPADTouchData calibrated = {0};
         VPADGetTPCalibratedPointEx(VPAD_CHAN_0, VPAD_TP_1280X720, &calibrated, &sample->tpNormal);
         out.touch = 2;
         if (calibrated.touched && calibrated.validity == VPAD_VALID &&
             calibrated.x < VIEW_WIDTH && calibrated.y < VIEW_HEIGHT) {
            out.touch = 1;
            out.x = calibrated.x;
            out.y = calibrated.y;
         }
      }
      samples[index] = out;
   }
   return count;
}

bool shell_create_view(const uint8_t *atlas, uint32_t width, uint32_t height, uint32_t capacity)
{
   return horizon_gx2_hud_create(&view, _binary_pregame_gsh_start, width, height,
                                atlas, (size_t)width * height * 4, capacity) == HORIZON_HUD_OK;
}

static bool draw(const HorizonHudVertex *vertices, uint32_t count, uint32_t color,
                 const GX2ColorBuffer *target, uint32_t output)
{
   const uint32_t w = target->surface.width;
   const uint32_t h = target->surface.height;
   /* Integer 16:9 fit also works on a 4:3 TV and the 854x480 DRC. */
   const uint32_t scale = (w / 16 < h / 9) ? w / 16 : h / 9;
   const uint32_t width = 16 * scale;
   const uint32_t height = 9 * scale;
   WHBGfxClearColor((color & 255) / 255.0f, ((color >> 8) & 255) / 255.0f,
                    ((color >> 16) & 255) / 255.0f, 1.0f);
   if (!count) return true;
   const HorizonHudRun run = {.end = count, .blend = HORIZON_HUD_ALPHA};
   const HorizonHudFrame frame = {
      .vertices = vertices, .vertex_count = count, .runs = &run, .run_count = 1,
      .viewport_width = VIEW_WIDTH, .viewport_height = VIEW_HEIGHT, .scale = 1.0f,
   };
   return horizon_gx2_hud_draw_to_output(view, &frame, (w - width) / 2, (h - height) / 2,
                                         width, height, output) == HORIZON_HUD_OK;
}

bool shell_present(const HorizonHudVertex *tv_vertices, uint32_t tv_count, uint32_t tv_color,
                   const HorizonHudVertex *drc_vertices, uint32_t drc_count, uint32_t drc_color)
{
   GX2ColorBuffer *tv = WHBGfxGetTVColourBuffer();
   GX2ColorBuffer *drc = WHBGfxGetDRCColourBuffer();
   if (!tv || !drc || !tv->surface.image || !drc->surface.image)
      return false;

#ifdef HORIZON_WIIU_HOSTED
   uint64_t stamp = horizon_gx2_profile_stamp();
#endif
   WHBGfxBeginRender();
#ifdef HORIZON_WIIU_HOSTED
   horizon_gx2_profile_end(GX2_PROFILE_WAIT_FLIP, stamp);
   stamp = horizon_gx2_profile_stamp();
#endif
   WHBGfxBeginRenderTV();
   const bool tv_ok = draw(tv_vertices, tv_count, tv_color, tv, 0);
#ifdef HORIZON_WIIU_HOSTED
   horizon_swkbd_draw(0);
#endif
   WHBGfxFinishRenderTV();
#ifdef HORIZON_WIIU_HOSTED
   horizon_gx2_profile_end(GX2_PROFILE_SUBMIT_TV, stamp);
   stamp = horizon_gx2_profile_stamp();
#endif
   WHBGfxBeginRenderDRC();
   const bool drc_ok = draw(drc_vertices, drc_count, drc_color, drc, 1);
#ifdef HORIZON_WIIU_HOSTED
   horizon_swkbd_draw(1);
#endif
   WHBGfxFinishRenderDRC();
#ifdef HORIZON_WIIU_HOSTED
   horizon_gx2_profile_end(GX2_PROFILE_SUBMIT_DRC, stamp);
   stamp = horizon_gx2_profile_stamp();
#endif
   WHBGfxFinishRender();
#ifdef HORIZON_WIIU_HOSTED
   horizon_gx2_profile_end(GX2_PROFILE_FINISH, stamp);
   if (tv_ok && drc_ok) horizon_gx2_profile_count(GX2_PROFILE_PRESENTED, 1);
   horizon_gx2_profile_count(GX2_PROFILE_HUD_VERTICES, tv_count + drc_count);
   horizon_gx2_profile_finish();
#endif
   return tv_ok && drc_ok;
}

int main(void)
{
#ifdef HORIZON_WIIU_HOSTED
   horizon_wiiu_app_entropy_first();
   horizon_wiiu_memory_init();
#endif
   ProcUIInit(saves_ready);
   if (!WHBGfxInit()) {
      shell_trace("gx2.init_failed");
      ProcUIShutdown();
      return 1;
   }

#ifdef HORIZON_WIIU_HOSTED
   horizon_wiiu_memory_report_display_surfaces();
   horizon_gx2_profile_init();
#endif
   const int32_t result = horizon_pregame_run();
#ifdef HORIZON_WIIU_HOSTED
   horizon_gx2_profile_flush();
#endif
   horizon_gx2_hud_destroy(view);
   shell_trace("view.released");
   WHBGfxShutdown();
   shell_trace("gx2.closed");
   ProcUIShutdown();
   shell_trace("procui.closed");
   return result;
}
