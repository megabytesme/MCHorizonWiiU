/* Build against the hosted staged terrain renderer and its compiled GFD. */
#include "terrain.h"
#include <coreinit/debug.h>
#include <coreinit/time.h>
#include <gx2/event.h>
#include <gx2r/surface.h>
#include <stdlib.h>
#include <string.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern const uint8_t _binary_terrain_gsh_start[];

static int fill(void *context, HorizonTerrainVertex *vertices, uint32_t count)
{
   (void)context;
   if (count != 16) return -1;
   const uint32_t light[] = {0, 240 << 8, 240, (120 << 8) | 40};
   for (uint32_t i = 0; i < 4; ++i) {
      const float x = -1.0f + i * 0.5f;
      const float corners[][2] = {{x, -1}, {x + 0.5f, -1}, {x + 0.5f, 1}, {x, 1}};
      for (uint32_t corner = 0; corner < 4; ++corner) {
         vertices[i * 4 + corner] = (HorizonTerrainVertex){
            .position = {corners[corner][0], corners[corner][1], 0.5f, (float)light[i]},
            .color = {1, 1, 1}, .uv = {0.5f, 0.5f}, .cutout = 0,
         };
      }
   }
   return 0;
}

static bool check(GX2Surface *source, uint32_t target)
{
   GX2Surface linear = {0};
   linear.dim = GX2_SURFACE_DIM_TEXTURE_2D;
   linear.width = source->width;
   linear.height = source->height;
   linear.depth = linear.mipLevels = 1;
   linear.format = GX2_SURFACE_FORMAT_UNORM_R8_G8_B8_A8;
   linear.aa = GX2_AA_MODE1X;
   linear.tileMode = GX2_TILE_MODE_LINEAR_SPECIAL;
   const GX2RResourceFlags flags = GX2R_RESOURCE_BIND_TEXTURE |
      GX2R_RESOURCE_USAGE_CPU_READ | GX2R_RESOURCE_USAGE_GPU_WRITE;
   if (!GX2RCreateSurface(&linear, flags)) return false;
   GX2DrawDone();
   GX2CopySurface(source, 0, 0, &linear, 0, 0);
   GX2DrawDone();
   const uint8_t *pixels = GX2RLockSurfaceEx(&linear, 0, flags);
   bool ok = pixels != NULL;
   const int expected[][3] = {{0, 0, 32}, {255, 0, 32}, {0, 255, 32}, {128, 43, 32}};
   if (pixels) {
      for (uint32_t i = 0; i < 4; ++i) {
         const size_t x = linear.width * (i * 2 + 1) / 8;
         const size_t y = linear.height / 2;
         const uint8_t *rgb = pixels + (y * linear.pitch + x) * 4;
         OSReport("GX2 lightmap sample target=%u index=%u rgb=%u,%u,%u\n",
                  target, i, rgb[0], rgb[1], rgb[2]);
         for (uint32_t c = 0; c < 3; ++c)
            if (abs(rgb[c] - expected[i][c]) > 2) ok = false;
      }
      GX2RUnlockSurfaceEx(&linear, 0, flags);
   }
   GX2RDestroySurfaceEx(&linear, flags);
   return ok;
}

int main(void)
{
   WHBProcInit();
   if (!WHBGfxInit()) return 2;
   int result = 1;
   HorizonGx2Terrain *terrain = NULL;
   HorizonGx2TerrainSection *section = NULL;
   uint32_t bytes;
   uint8_t atlas[4] = {255, 255, 255, 255};
   uint8_t lightmap[16 * 16 * 4];
   for (uint32_t y = 0; y < 16; ++y)
      for (uint32_t x = 0; x < 16; ++x) {
         uint8_t *pixel = &lightmap[(y * 16 + x) * 4];
         pixel[0] = x * 17; pixel[1] = y * 17; pixel[2] = 32; pixel[3] = 255;
      }
   if (horizon_gx2_terrain_create(&terrain, _binary_terrain_gsh_start, 1, 1, atlas, sizeof(atlas)) ||
       horizon_gx2_terrain_section_create(&section, 16, fill, NULL, &bytes) ||
       horizon_gx2_terrain_lightmap(terrain, lightmap)) goto exit;
   const float clip[16] = {1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1};
   const float origin[4] = {0, 0, 0, 0}, fog[4] = {0, 0, 0, 0};
   const float ranges[4] = {0, 1, 0, 1};
   bool ok = true, inspect = true;
   const int64_t deadline = OSGetSystemTime() + OSSecondsToTicks(12);
   do {
      WHBGfxBeginRender();
      for (uint32_t target = 0; target < 2; ++target) {
         if (!target) WHBGfxBeginRenderTV();
         else WHBGfxBeginRenderDRC();
         GX2ColorBuffer *color = !target ? WHBGfxGetTVColourBuffer() : WHBGfxGetDRCColourBuffer();
         const uint32_t width = color->surface.width, height = color->surface.height;
         WHBGfxClearColor(0.7f, 0.7f, 0.7f, 1);
         if (horizon_gx2_terrain_draw(terrain, section, width, height, 0, 0, width, height,
                                      clip, origin, fog, ranges) ||
             (inspect && !check(&color->surface, target))) ok = false;
         if (!target) WHBGfxFinishRenderTV();
         else WHBGfxFinishRenderDRC();
      }
      WHBGfxFinishRender();
      inspect = false;
   } while (ok && OSGetSystemTime() < deadline && WHBProcIsRunning());
   result = ok ? 0 : 1;
exit:
   OSReport("GX2 lightmap diagnostic %s\n", result ? "FAIL" : "PASS");
   horizon_gx2_terrain_section_destroy(section);
   horizon_gx2_terrain_destroy(terrain);
   WHBGfxShutdown();
   WHBProcShutdown();
   return result;
}
