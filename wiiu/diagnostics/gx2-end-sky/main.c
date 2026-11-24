#include "end_sky.h"
#include "hud.h"

#include <coreinit/debug.h>
#include <gx2/event.h>
#include <gx2/surface.h>
#include <gx2r/surface.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern const uint8_t _binary_hud_gsh_start[];

static bool samples(const GX2ColorBuffer *color, uint32_t output)
{
   const GX2Surface *src = &color->surface;
   GX2Surface copy = {
      .dim = GX2_SURFACE_DIM_TEXTURE_2D, .width = src->width, .height = src->height,
      .depth = 1, .mipLevels = 1, .format = src->format,
      .aa = GX2_AA_MODE1X, .tileMode = GX2_TILE_MODE_LINEAR_SPECIAL,
   };
   const GX2RResourceFlags flags = GX2R_RESOURCE_BIND_TEXTURE |
      GX2R_RESOURCE_USAGE_CPU_READ | GX2R_RESOURCE_USAGE_GPU_WRITE;
   if (!GX2RCreateSurface(&copy, flags)) return false;
   GX2DrawDone();
   GX2CopySurface(src, 0, 0, &copy, 0, 0);
   GX2DrawDone();
   const uint8_t *pixels = GX2RLockSurfaceEx(&copy, 0, flags);
   bool ok = pixels != NULL;
   if (pixels) {
      const float tile_pos[6][2] = {
         {2.25f, 5.25f}, {2.75f, 5.25f}, {2.25f, 5.75f}, {2.75f, 5.75f},
         {7.975f, 5.25f}, {8.025f, 5.25f},
      };
      const uint8_t expected[2][6][4] = {
         {{48, 16, 24, 192}, {8, 36, 24, 192}, {8, 16, 64, 192}, {8, 16, 24, 192},
          {8, 36, 24, 192}, {48, 16, 24, 192}},
         {{28, 16, 44, 192}, {8, 56, 64, 192}, {8, 16, 24, 192}, {48, 56, 24, 192},
          {8, 56, 64, 192}, {28, 16, 44, 192}},
      };
      for (unsigned i = 0; i < 6; ++i) {
         const size_t x = (size_t)(tile_pos[i][0] * (float)src->width / 16.0f);
         const size_t y = (size_t)(tile_pos[i][1] * (float)src->height / 16.0f);
         const uint8_t *rgba = pixels + (y * copy.pitch + x) * 4;
         bool match = true;
         for (unsigned channel = 0; channel < 4; ++channel)
            match &= abs((int)rgba[channel] - expected[output][i][channel]) <= 1;
         OSReport("GX2 End sky fixture output=%u sample=%u rgba=%u,%u,%u,%u pass=%u\n",
                  output, i, rgba[0], rgba[1], rgba[2], rgba[3], match);
         ok &= match;
      }
      GX2RUnlockSurfaceEx(&copy, 0, flags);
   }
   GX2RDestroySurfaceEx(&copy, flags);
   return ok;
}

int main(void)
{
   WHBProcInit();
   if (!WHBGfxInit()) return 1;
   HorizonGx2EndSky *sky = NULL;
   HorizonGx2Hud *hud = NULL;
   const float vertices[HORIZON_END_SKY_VERTICES][5] = {
      {-1, 1, 0.5f, 0, 0}, {-1, -1, 0.5f, 0, 16}, {1, -1, 0.5f, 16, 16},
      {-1, 1, 0.5f, 0, 0}, {1, -1, 0.5f, 16, 16}, {1, 1, 0.5f, 16, 0},
   };
   uint8_t atlas[4][8][4];
   memset(atlas, 255, sizeof(atlas));
   const uint8_t sprites[2][2][2][4] = {
      {{{255, 0, 0, 255}, {0, 255, 0, 128}}, {{0, 0, 255, 255}, {255, 255, 255, 0}}},
      {{{0, 255, 255, 255}, {255, 0, 255, 128}}, {{255, 255, 0, 255}, {0, 0, 0, 255}}},
   };
   for (unsigned i = 0; i < 2; ++i)
      for (unsigned y = 0; y < 2; ++y)
         memcpy((uint8_t *)atlas[1 + y] + 4 * (1 + 4 * i), sprites[i][y], sizeof(sprites[i][y]));
   bool ok = horizon_gx2_end_sky_create(&sky, vertices) == 0 &&
      horizon_gx2_hud_create(&hud, _binary_hud_gsh_start, 8, 4,
                             &atlas[0][0][0], sizeof(atlas), 6) == 0;
   if (ok) {
      const float clip_rows[2][4][4] = {
         {{1, 0, 0, 0}, {0, 1, 0, 0}, {0, 0, 1, 0}, {0, 0, 0, 1}},
         {{-1, 0, 0, 0}, {0, 1, 0, 0}, {0, 0, 1, 0}, {0, 0, 0, 1}},
      };
      const float bounds[2][4] = {{0.125f, 0.25f, 0.375f, 0.75f}, {0.625f, 0.25f, 0.875f, 0.75f}};
      WHBGfxBeginRender();
      for (uint32_t i = 0; i < 2; ++i) {
         if (i == 0) WHBGfxBeginRenderTV();
         else WHBGfxBeginRenderDRC();
         WHBGfxClearColor(8.0f / 255.0f, 16.0f / 255.0f, 24.0f / 255.0f, 192.0f / 255.0f);
         const GX2ColorBuffer *color = i == 0 ? WHBGfxGetTVColourBuffer() : WHBGfxGetDRCColourBuffer();
         const uint32_t rect[4] = {0, 0, color->surface.width, color->surface.height};
         ok &= horizon_gx2_end_sky_draw(sky, rect, horizon_gx2_hud_atlas(hud), bounds[i], clip_rows[i]) == 0;
         ok &= samples(color, i);
         if (i == 0) WHBGfxFinishRenderTV();
         else WHBGfxFinishRenderDRC();
      }
      WHBGfxFinishRender();
   }
   OSReport("GX2 End sky fixture %s\n", ok ? "passed" : "failed");
   horizon_gx2_end_sky_destroy(sky);
   horizon_gx2_hud_destroy(hud);
   WHBGfxShutdown();
   WHBProcShutdown();
   return ok ? 0 : 1;
}
