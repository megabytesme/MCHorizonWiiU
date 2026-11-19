/* Diagnostic only: real Horizon boot atlas and batch, no Minecraft assets. */
#include "../gx2_hud/hud.h"
#include <coreinit/debug.h>
#include <coreinit/filesystem.h>
#include <gx2/event.h>
#include <gx2/surface.h>
#include <gx2r/surface.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <whb/gfx.h>
#include <whb/proc.h>
#include <whb/sdcard.h>

extern const unsigned char _binary_hud_probe_gsh_start[];
extern const unsigned char _binary_hud_probe_gsh_end[];
typedef struct Probe Probe;
typedef struct Metrics {
   float scale, first_x, first_y, first_u, first_v;
   uint32_t color;
   uint8_t sample[4];
} Metrics;
extern int32_t horizon_hud_probe_create(const void *, size_t, Probe **);
extern int32_t horizon_hud_probe_draw(Probe *, uint32_t, uint32_t, HorizonHudVertex *, Metrics *);
extern void horizon_hud_probe_destroy(Probe *);

static FSClient client;
static FSCmdBlock cmd;
static int file_ready;
static _Alignas(64) char log_buffer[512];

static void status(const char *message)
{
   OSReport("Horizon Rust HUD probe: %s\n", message);
   if (!file_ready) return;
   char path[256];
   snprintf(path, sizeof(path), "%s/wiiu/apps/HorizonWiiU/rust-hud-status.log", WHBGetSdCardMountPath());
   FSFileHandle file;
   if (FSOpenFile(&client, &cmd, path, "w", &file, FS_ERROR_FLAG_NONE) == FS_STATUS_OK) {
      snprintf(log_buffer, sizeof(log_buffer), "%s\n", message);
      FSWriteFile(&client, &cmd, (uint8_t *)log_buffer, 1, (uint32_t)strlen(log_buffer), file, 0, FS_ERROR_FLAG_NONE);
      FSCloseFile(&client, &cmd, file, FS_ERROR_FLAG_NONE);
   }
}

static int readback(char *message, size_t capacity)
{
   GX2Surface *src = &WHBGfxGetTVColourBuffer()->surface;
   if (src->width <= 224 || src->height <= 64) {
      snprintf(message, capacity, "readback target too small=%ux%u", src->width, src->height);
      return 0;
   }
   GX2Surface dst = {0};
   dst.dim = GX2_SURFACE_DIM_TEXTURE_2D;
   dst.width = src->width; dst.height = src->height; dst.depth = 1;
   dst.mipLevels = 1; dst.format = src->format; dst.aa = GX2_AA_MODE1X;
   /* Cemu-specific copy path used by the prior verified C HUD diagnostic. */
   dst.tileMode = GX2_TILE_MODE_LINEAR_SPECIAL;
   const GX2RResourceFlags flags = GX2R_RESOURCE_BIND_TEXTURE | GX2R_RESOURCE_USAGE_CPU_READ | GX2R_RESOURCE_USAGE_GPU_WRITE;
   if (!GX2RCreateSurface(&dst, flags)) {
      snprintf(message, capacity, "readback surface allocation failed");
      return 0;
   }
   GX2DrawDone();
   GX2CopySurface(src, 0, 0, &dst, 0, 0);
   GX2DrawDone();
   const uint8_t *p = GX2RLockSurfaceEx(&dst, 0, flags);
   if (!p) {
      GX2RDestroySurfaceEx(&dst, flags);
      snprintf(message, capacity, "readback surface lock failed");
      return 0;
   }
   const uint8_t *red = p + ((size_t)64 * dst.pitch + 64) * 4;
   const uint8_t *green = p + ((size_t)64 * dst.pitch + 224) * 4;
   const uint8_t *background = p + ((size_t)8 * dst.pitch + 8) * 4;
   const int ok = red[0] > 200 && red[1] < 30 && red[2] < 30 &&
                  green[0] < 30 && green[1] > 200 && green[2] < 30 &&
                  background[0] > 30 && background[0] < 80;
   snprintf(message, capacity,
            "readback=%s target=%ux%u red=%u,%u,%u,%u green=%u,%u,%u,%u background=%u,%u,%u,%u",
            ok ? "pass" : "mismatch", src->width, src->height,
            red[0],red[1],red[2],red[3],green[0],green[1],green[2],green[3],
            background[0],background[1],background[2],background[3]);
   GX2RUnlockSurfaceEx(&dst, 0, flags);
   GX2RDestroySurfaceEx(&dst, flags);
   return ok;
}

int main(void)
{
   WHBProcInit();
   if (WHBMountSdCard() && FSAddClient(&client, FS_ERROR_FLAG_NONE) == FS_STATUS_OK) {
      FSInitCmdBlock(&cmd);
      file_ready = 1;
   }
   status("entry reached");
   if (!WHBGfxInit()) { status("GX2 initialization failed"); return 1; }
   Probe *probe = NULL;
   size_t shader_size = (size_t)(_binary_hud_probe_gsh_end - _binary_hud_probe_gsh_start);
   int result = horizon_hud_probe_create(_binary_hud_probe_gsh_start, shader_size, &probe);
   if (result != 0) {
      char report[96];
      snprintf(report, sizeof(report), "presenter create failed=%d", result);
      status(report);
      WHBGfxShutdown();
      if (file_ready) FSDelClient(&client, FS_ERROR_FLAG_NONE);
      WHBUnmountSdCard(); WHBProcShutdown();
      return 2;
   }
   int verified = 0;
   if (WHBProcIsRunning()) {
      WHBGfxBeginRender(); WHBGfxBeginRenderTV();
      WHBGfxClearColor(0.2f, 0.2f, 0.2f, 1.0f);
      GX2Surface *tv = &WHBGfxGetTVColourBuffer()->surface;
      HorizonHudVertex vertices[12];
      Metrics metrics = {0};
      result = horizon_hud_probe_draw(probe, tv->width, tv->height, vertices, &metrics);
      char report[384];
      if (result == 0) {
         verified = readback(report, sizeof(report));
         if (!verified) {
            char original[180];
            snprintf(original, sizeof(original), "%.170s", report);
            /* Reuse Horizon's exact vertices with a known white control atlas. */
            const uint8_t white[] = {255,255,255,255, 255,255,255,255};
            HorizonGx2Hud *control = NULL;
            HorizonHudResult control_result = horizon_gx2_hud_create(
               &control, _binary_hud_probe_gsh_start, 2, 1, white, sizeof(white), 12);
            char control_report[180] = "control create failed";
            if (control_result == HORIZON_HUD_OK) {
               const HorizonHudRun runs[] = {{0,12,HORIZON_HUD_ALPHA,{0}}};
               const HorizonHudFrame frame = {vertices,12,runs,1,NULL,0,tv->width,tv->height,metrics.scale,0};
               control_result = horizon_gx2_hud_draw(control, &frame);
               if (control_result == HORIZON_HUD_OK) readback(control_report, sizeof(control_report));
               else snprintf(control_report, sizeof(control_report), "control draw failed=%d", control_result);
               horizon_gx2_hud_destroy(control);
            }
            snprintf(report, sizeof(report),
                     "rust=%.120s control=%.120s pos=%d,%d uv_ppm=%d,%d color=%08x texel=%u,%u,%u,%u",
                     original, control_report, (int)(metrics.first_x * metrics.scale),
                     (int)(metrics.first_y * metrics.scale), (int)(metrics.first_u * 1000000),
                     (int)(metrics.first_v * 1000000), metrics.color,
                     metrics.sample[0],metrics.sample[1],metrics.sample[2],metrics.sample[3]);
         }
      } else snprintf(report, sizeof(report), "presenter draw failed=%d", result);
      WHBGfxFinishRenderTV(); WHBGfxFinishRender();
      status(report);
   } else status("process not in foreground");
   horizon_hud_probe_destroy(probe);
   WHBGfxShutdown();
   if (file_ready) FSDelClient(&client, FS_ERROR_FLAG_NONE);
   WHBUnmountSdCard(); WHBProcShutdown();
   return verified ? 0 : 4;
}
