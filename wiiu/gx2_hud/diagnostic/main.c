/* Explicit test fixture for the real GX2 HUD presenter; it contains no game assets. */
#include "../hud.h"
#include "../cit_glint.h"
#include <coreinit/debug.h>
#include <coreinit/filesystem.h>
#include <coreinit/thread.h>
#include <coreinit/time.h>
#include <gx2/event.h>
#include <gx2/surface.h>
#include <gx2r/surface.h>
#include <stdio.h>
#include <string.h>
#include <whb/gfx.h>
#include <whb/proc.h>
#include <whb/sdcard.h>

extern const unsigned char _binary_hud_diag_gsh_start[];
static FSClient sClient;
static FSCmdBlock sCmd;
static int sFileReady;
static _Alignas(64) char sLog[512];

enum { LAYOUT_WIDTH = 1280, LAYOUT_HEIGHT = 720, ATLAS_WIDTH = 10, VERTEX_COUNT = 168 };

static void status(const char *message)
{
   OSReport("Horizon HUD diagnostic: %s\n", message);
   if (!sFileReady) return;
   char path[256];
   snprintf(path, sizeof(path), "%s/wiiu/apps/HorizonWiiUHudDiag/hud-diagnostic-status.log", WHBGetSdCardMountPath());
   FSFileHandle file;
   if (FSOpenFile(&sClient, &sCmd, path, "w", &file, FS_ERROR_FLAG_NONE) == FS_STATUS_OK) {
      snprintf(sLog, sizeof(sLog), "%s\n", message);
      FSWriteFile(&sClient, &sCmd, (uint8_t *)sLog, 1, (uint32_t)strlen(sLog), file, 0, FS_ERROR_FLAG_NONE);
      FSCloseFile(&sClient, &sCmd, file, FS_ERROR_FLAG_NONE);
   }
}

static void quad(HorizonHudVertex *v, float x0, float y0, float x1, float y1,
                 float u, float depth, uint32_t color)
{
   const float points[6][2] = {{x0,y0},{x1,y0},{x1,y1},{x0,y0},{x1,y1},{x0,y1}};
   for (unsigned i = 0; i < 6; ++i) {
      v[i] = (HorizonHudVertex){.pos = {points[i][0], points[i][1]}, .uv = {u, 0.5f},
                               .color = color, .depth = depth};
   }
}

static int readback(GX2Surface *src, char *message, size_t capacity)
{
   GX2Surface dst = {0};
   dst.dim = GX2_SURFACE_DIM_TEXTURE_2D;
   dst.width = src->width; dst.height = src->height; dst.depth = 1;
   dst.mipLevels = 1; dst.format = src->format; dst.aa = GX2_AA_MODE1X;
   /* Cemu 2.6 triggers a texture readback for this diagnostic-only copy. */
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
   const struct {
      const char *name;
      uint32_t x, y;
      uint8_t rgba[4];
   } samples[] = {
      {"background", 8, 8, {51, 51, 51, 64}},
      {"patched atlas", 64, 64, {255, 0, 0, 255}},
      {"vertex tint", 224, 64, {0, 255, 0, 255}},
      {"blue texel", 384, 64, {0, 0, 255, 255}},
      {"transparent discard", 544, 64, {51, 51, 51, 64}},
      {"red texel", 704, 64, {255, 0, 0, 255}},
      {"green texel", 864, 64, {0, 255, 0, 255}},
      {"additive", 1024, 64, {153, 102, 76, 191}},
      {"alpha after additive", 1184, 64, {0, 0, 255, 255}},
      {"near depth wins", 104, 264, {255, 0, 0, 255}},
      {"left of depth scissor", 48, 264, {51, 51, 51, 64}},
      {"above depth scissor", 104, 216, {51, 51, 51, 64}},
      {"next preview clears depth", 312, 264, {0, 0, 255, 255}},
      {"alpha ignores previous depth", 400, 264, {0, 255, 0, 255}},
      {"alpha outside previous scissor", 488, 264, {0, 255, 0, 255}},
      {"invert", 576, 264, {204, 204, 204, 255}},
      {"transparent vignette texel", 736, 264, {0, 51, 51, 64}},
      {"vignette interpolation", 896, 264, {26, 26, 51, 64}},
      {"alpha after vignette", 776, 264, {0, 153, 25, 160}},
      {"glint strength and repeat", 64, 384, {115, 67, 55, 255}},
      {"glint below mask cutoff", 224, 384, {51, 51, 51, 64}},
      {"glint above mask cutoff", 384, 384, {115, 67, 55, 90}},
      {"end portal", 576, 384, {162, 25, 157, 255}},
      {"CIT slot one", 688, 384, {89, 25, 25, 64}},
      {"CIT slot one then two", 720, 384, {44, 12, 77, 64}},
      {"CIT slot one then two then one", 752, 384, {86, 6, 38, 64}},
      {"CIT slot two", 816, 384, {25, 25, 89, 64}},
      {"CIT below atlas mask cutoff", 880, 384, {51, 51, 51, 64}},
      {"CIT below layer alpha cutoff", 952, 384, {51, 51, 51, 64}},
      {"vanilla glint after CIT", 1040, 384, {115, 67, 55, 255}},
   };
   int ok = 1;
   for (size_t i = 0; i < sizeof(samples) / sizeof(samples[0]); ++i) {
      const uint32_t x = samples[i].x * dst.width / LAYOUT_WIDTH;
      const uint32_t y = samples[i].y * dst.height / LAYOUT_HEIGHT;
      const uint8_t *actual = p + ((size_t)y * dst.pitch + x) * 4;
      for (unsigned channel = 0; channel < 4; ++channel) {
         const int delta = (int)actual[channel] - samples[i].rgba[channel];
         if (delta >= -3 && delta <= 3) continue;
         snprintf(message, capacity, "%s at %u,%u got %u,%u,%u,%u expected %u,%u,%u,%u",
                  samples[i].name, x, y, actual[0], actual[1], actual[2], actual[3],
                  samples[i].rgba[0], samples[i].rgba[1], samples[i].rgba[2], samples[i].rgba[3]);
         ok = 0;
         break;
      }
      if (!ok) break;
   }
   if (ok) snprintf(message, capacity, "pass %ux%u", src->width, src->height);
   GX2RUnlockSurfaceEx(&dst, 0, flags);
   GX2RDestroySurfaceEx(&dst, flags);
   return ok;
}

int main(void)
{
   WHBProcInit();
   if (WHBMountSdCard() && FSAddClient(&sClient, FS_ERROR_FLAG_NONE) == FS_STATUS_OK) {
      FSInitCmdBlock(&sCmd);
      sFileReady = 1;
   }
   status("entry reached");
   if (!WHBGfxInit()) { status("GX2 initialization failed"); return 1; }
   const uint8_t atlas[] = {0,0,0,255, 255,255,255,255, 0,0,255,255, 255,0,0,0,
                            255,0,0,255, 0,255,0,255, 255,255,255,25, 255,255,255,26,
                            0,255,0,255, 127,0,63,255};
   const uint8_t white[] = {255,255,255,255};
   const uint8_t glint[] = {255,128,64,7, 32,0,0,255};
   const uint8_t cit_pixels[] = {255,0,0,255, 0,255,0,255, 0,0,255,255, 0,0,0,255};
   const HorizonHudCitSlot cit_slots[] = {
      {{0.5f, 1, 1, 1}, {4, 5, 0, 1}, 0, 2},
      {{1, 1, 1, 1}, {4, 5, 0, 1}, 1, 2},
   };
   HorizonGx2Hud *hud = NULL;
   HorizonHudResult result = horizon_gx2_hud_create(&hud, _binary_hud_diag_gsh_start, ATLAS_WIDTH, 1,
                                                    atlas, sizeof(atlas), VERTEX_COUNT);
   if (result == HORIZON_HUD_OK) {
      result = horizon_gx2_hud_set_glint_texture(hud, 2, 1, glint, sizeof(glint));
      horizon_gx2_hud_set_glint_style(hud, 0.5f, NULL);
   }
   if (result != HORIZON_HUD_OK) {
      char report[96];
      snprintf(report, sizeof(report), "presenter setup failed=%d", result);
      status(report);
      horizon_gx2_hud_destroy(hud);
      WHBGfxShutdown();
      if (sFileReady) FSDelClient(&sClient, FS_ERROR_FLAG_NONE);
      WHBUnmountSdCard(); WHBProcShutdown();
      return 2;
   }
   HorizonHudVertex vertices[VERTEX_COUNT];
   const float texel = 1.0f / ATLAS_WIDTH;
   const float white_uv = 1.5f * texel;
   quad(vertices, 32, 32, 160, 160, 0.5f * texel, 0, UINT32_C(0xff0000ff));
   quad(vertices + 6, 192, 32, 320, 160, white_uv, 0, UINT32_C(0xff00ff00));
   quad(vertices + 12, 352, 32, 480, 160, 2.5f * texel, 0, UINT32_C(0xffffffff));
   quad(vertices + 18, 512, 32, 640, 160, 3.5f * texel, 0, UINT32_C(0xffffffff));
   quad(vertices + 24, 672, 32, 800, 160, 4.5f * texel, 0, UINT32_C(0xffffffff));
   quad(vertices + 30, 832, 32, 960, 160, 5.5f * texel, 0, UINT32_C(0xffffffff));
   quad(vertices + 36, 992, 32, 1120, 160, white_uv, 0, UINT32_C(0x80193366));
   quad(vertices + 42, 1152, 32, 1280, 160, 2.5f * texel, 0, UINT32_C(0xffffffff));

   /* Near red must win over far green, even though green is submitted last.
    * Each preview starts with a fresh depth buffer, including repeated clips. */
   quad(vertices + 48, 32, 192, 208, 320, white_uv, 0.2f, UINT32_C(0xff0000ff));
   quad(vertices + 54, 32, 192, 208, 320, white_uv, 0.8f, UINT32_C(0xff00ff00));
   quad(vertices + 60, 240, 192, 448, 320, white_uv, 0.2f, UINT32_C(0xff0000ff));
   quad(vertices + 66, 240, 192, 448, 320, white_uv, 0.8f, UINT32_C(0xff00ff00));
   quad(vertices + 72, 240, 192, 448, 320, white_uv, 0.8f, UINT32_C(0xffff0000));
   quad(vertices + 78, 384, 240, 520, 288, white_uv, 0.95f, UINT32_C(0xff00ff00));
   quad(vertices + 84, 544, 224, 640, 304, white_uv, 0, UINT32_C(0xffffffff));
   quad(vertices + 90, 672, 224, 800, 304, 3.5f * texel, 0, UINT32_C(0xffffffff));
   quad(vertices + 96, 832, 224, 960, 304, 5.0f * texel, 0, UINT32_C(0xffffffff));
   quad(vertices + 102, 764, 240, 792, 288, white_uv, 0, UINT32_C(0x8000ff00));

   /* Texture alpha contributes only as the item mask. At half strength,
    * the first glint texel contributes (64, 16, 4) after squaring its RGB. */
   quad(vertices + 108, 32, 352, 160, 432, 2.0f + white_uv, 0, UINT32_C(0xffffffff));
   quad(vertices + 114, 192, 352, 320, 432, 2.0f + 6.5f * texel, 0, UINT32_C(0xffffffff));
   quad(vertices + 120, 352, 352, 480, 432, 2.0f + 7.5f * texel, 0, UINT32_C(0xffffffff));
   for (unsigned i = 108; i < 126; ++i) {
      vertices[i].glint_uv[0] = 1.25f;
      vertices[i].glint_uv[1] = 0.5f;
   }
   quad(vertices + 126, 544, 352, 640, 432, white_uv, 0, UINT32_C(0xffffffff));
   /* Sum of Horizon's original 15 layers over the two solid sprite texels. */
   const float portal[2][4] = {{8 * texel, 0, texel, 1}, {9 * texel, 0, texel, 1}};
   horizon_gx2_hud_set_end_portal(hud, portal, 0.123f);
   quad(vertices + 132, 672, 352, 800, 432, white_uv, 0, UINT32_C(0x80000000));
   quad(vertices + 138, 704, 352, 832, 432, white_uv, 0, UINT32_C(0x80000001));
   quad(vertices + 144, 736, 352, 772, 432, white_uv, 0, UINT32_C(0x80000000));
   quad(vertices + 150, 848, 352, 912, 432, 6.5f * texel, 0, UINT32_C(0x80000000));
   quad(vertices + 156, 928, 352, 992, 432, white_uv, 0, UINT32_C(0x10000000));
   quad(vertices + 162, 1008, 352, 1104, 432, 2.0f + white_uv, 0, UINT32_C(0xffffffff));
   for (unsigned i = 132; i < 162; ++i) {
      vertices[i].glint_uv[0] = (vertices[i].color & 255) ? 1.5f : 1.375f;
      vertices[i].glint_uv[1] = 0.5f;
   }
   for (unsigned i = 162; i < VERTEX_COUNT; ++i) {
      vertices[i].glint_uv[0] = 1.25f;
      vertices[i].glint_uv[1] = 0.5f;
   }
   HorizonHudRun runs[] = {
      {0, 36, HORIZON_HUD_ALPHA, {0}},
      {36, 42, HORIZON_HUD_ADDITIVE, {0}},
      {42, 48, HORIZON_HUD_ALPHA, {0}},
      {48, 60, HORIZON_HUD_DEPTH, {64, 224, 112, 80}},
      {60, 72, HORIZON_HUD_DEPTH, {264, 224, 160, 80}},
      {72, 78, HORIZON_HUD_DEPTH, {264, 224, 160, 80}},
      {78, 84, HORIZON_HUD_ALPHA, {0}},
      {84, 90, HORIZON_HUD_INVERT, {0}},
      {90, 102, HORIZON_HUD_VIGNETTE, {0}},
      {102, 108, HORIZON_HUD_ALPHA, {0}},
      {108, 126, HORIZON_HUD_ADDITIVE, {0}},
      {126, 132, HORIZON_HUD_END_PORTAL, {0}},
      {132, 162, HORIZON_HUD_CIT_GLINT, {2}},
      {162, VERTEX_COUNT, HORIZON_HUD_ADDITIVE, {0}},
   };
   HorizonHudPatch patch = {0,0,1,1,white,sizeof(white)};
   HorizonHudFrame frame = {vertices, VERTEX_COUNT, runs, sizeof(runs) / sizeof(runs[0]),
                            &patch, 1, LAYOUT_WIDTH, LAYOUT_HEIGHT, 1.0f, 0};
   HorizonHudFrame reject_frame = frame;
   reject_frame.vertex_count = runs[0].end;
   reject_frame.run_count = 1;
   runs[0].blend = HORIZON_HUD_CIT_GLINT;
   if (horizon_gx2_hud_draw(hud, &reject_frame) != HORIZON_HUD_UNSUPPORTED) {
      status("CIT draw accepted without textures");
      horizon_gx2_hud_destroy(hud); WHBGfxShutdown();
      if (sFileReady) FSDelClient(&sClient, FS_ERROR_FLAG_NONE);
      WHBUnmountSdCard(); WHBProcShutdown();
      return 3;
   }
   runs[0].blend = HORIZON_HUD_ALPHA;
   result = horizon_gx2_hud_set_cit_glints(hud, 2, 1, cit_pixels, sizeof(cit_pixels),
                                          cit_slots, sizeof(cit_slots) / sizeof(cit_slots[0]));
   if (result != HORIZON_HUD_OK) {
      status("CIT texture setup failed");
      horizon_gx2_hud_destroy(hud); WHBGfxShutdown();
      if (sFileReady) FSDelClient(&sClient, FS_ERROR_FLAG_NONE);
      WHBUnmountSdCard(); WHBProcShutdown();
      return 3;
   }
   int verified = 0;
   if (WHBProcIsRunning()) {
      GX2Surface *targets[] = {&WHBGfxGetTVColourBuffer()->surface, &WHBGfxGetDRCColourBuffer()->surface};
      char output_reports[2][160];
      verified = 1;
      WHBGfxBeginRender();
      for (uint32_t output = 0; output < 2; ++output) {
         if (output == 0) WHBGfxBeginRenderTV();
         else WHBGfxBeginRenderDRC();
         WHBGfxClearColor(0.2f, 0.2f, 0.2f, 0.25f);
         GX2Surface *target = targets[output];
         result = horizon_gx2_hud_draw_to_output(hud, &frame, 0, 0, target->width, target->height, output);
         char *output_report = output_reports[output];
         if (result == HORIZON_HUD_OK) {
            verified &= readback(target, output_report, sizeof(output_reports[output]));
         } else {
            snprintf(output_report, sizeof(output_reports[output]), "presenter draw failed=%d", result);
            verified = 0;
         }
         if (output == 0) WHBGfxFinishRenderTV();
         else WHBGfxFinishRenderDRC();
      }
      WHBGfxFinishRender();
      char report[384];
      snprintf(report, sizeof(report), "readback=%s TV=[%s] DRC=[%s]",
               verified ? "pass" : "mismatch", output_reports[0], output_reports[1]);
      status(report);
      /* Leave the fixture visible for photos even if hardware readback fails. */
      const OSTime deadline = OSGetTime() + OSSecondsToTicks(15);
      while (OSGetTime() < deadline && WHBProcIsRunning())
         OSSleepTicks(OSMillisecondsToTicks(20));
   } else status("process not in foreground");
   horizon_gx2_hud_destroy(hud);
   WHBGfxShutdown();
   if (sFileReady) FSDelClient(&sClient, FS_ERROR_FLAG_NONE);
   WHBUnmountSdCard();
   WHBProcShutdown();
   return verified ? 0 : 4;
}
