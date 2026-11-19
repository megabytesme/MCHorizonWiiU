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

enum { VIEW_WIDTH = 1280, VIEW_HEIGHT = 720, VPAD_CAPACITY = 16 };
typedef struct LauncherSample { uint32_t buttons, touch, x, y; } LauncherSample;
static HorizonGx2Hud *view;
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
   switch (ProcUIProcessMessages(TRUE)) {
      case PROCUI_STATUS_IN_FOREGROUND: return 1;
      case PROCUI_STATUS_EXITING: return 2;
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

static uint32_t map_buttons(uint32_t buttons)
{
   return (buttons & VPAD_BUTTON_UP ? 1 : 0) |
          (buttons & VPAD_BUTTON_DOWN ? 2 : 0) |
          (buttons & VPAD_BUTTON_A ? 4 : 0) |
          (buttons & VPAD_BUTTON_B ? 8 : 0);
}

int32_t shell_poll_input(LauncherSample *samples)
{
   VPADStatus batch[VPAD_CAPACITY];
   VPADReadError error = VPAD_READ_SUCCESS;
   const int32_t count = VPADRead(VPAD_CHAN_0, batch, VPAD_CAPACITY, &error);
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

   WHBGfxBeginRender();
   WHBGfxBeginRenderTV();
   const bool tv_ok = draw(tv_vertices, tv_count, tv_color, tv, 0);
   WHBGfxFinishRenderTV();
   WHBGfxBeginRenderDRC();
   const bool drc_ok = draw(drc_vertices, drc_count, drc_color, drc, 1);
   WHBGfxFinishRenderDRC();
   WHBGfxFinishRender();
   return tv_ok && drc_ok;
}

int main(void)
{
   ProcUIInit(saves_ready);
   if (!WHBGfxInit()) {
      shell_trace("gx2.init_failed");
      ProcUIShutdown();
      return 1;
   }

   const int32_t result = horizon_pregame_run();
   horizon_gx2_hud_destroy(view);
   shell_trace("view.released");
   WHBGfxShutdown();
   shell_trace("gx2.closed");
   ProcUIShutdown();
   shell_trace("procui.closed");
   return result;
}
