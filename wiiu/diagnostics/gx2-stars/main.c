#include "stars.h"
#include "stock_sky.h"

#include <coreinit/debug.h>
#include <gx2/event.h>
#include <gx2/surface.h>
#include <gx2r/surface.h>
#include <stdbool.h>
#include <stdlib.h>
#include <whb/gfx.h>
#include <whb/proc.h>

static void sky_quad(HorizonSkyVertex vertices[6], float left, float right, float top)
{
   const unsigned indices[] = {0, 1, 2, 0, 2, 3};
   const float corners[4][2] = {{left, -1}, {right, -1}, {right, top}, {left, top}};
   for (unsigned i = 0; i < 6; ++i) {
      const float *p = corners[indices[i]];
      vertices[i] = (HorizonSkyVertex){.clip = {p[0], p[1], 0.5f, 1}};
   }
}

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
      const size_t x = src->width / 4, y = src->height / 4;
      const size_t offsets[] = {
         y * copy.pitch + x, y * copy.pitch + 3 * x,
         3 * y * copy.pitch + x, 3 * y * copy.pitch + 3 * x,
      };
      const uint8_t expected[2][4][4] = {
         {{96, 128, 160, 255}, {32, 64, 96, 255}, {96, 128, 160, 255}, {0, 0, 0, 255}},
         {{32, 64, 96, 255}, {48, 80, 112, 255}, {32, 64, 96, 255}, {0, 0, 0, 255}},
      };
      for (unsigned i = 0; i < 4; ++i) {
         const uint8_t *rgba = pixels + offsets[i] * 4;
         bool match = true;
         for (unsigned channel = 0; channel < 4; ++channel)
            match &= abs((int)rgba[channel] - expected[output][i][channel]) <= 1;
         OSReport("GX2 stars fixture output=%u sample=%u rgba=%u,%u,%u,%u pass=%u\n",
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
   HorizonGx2Stars *stars = NULL;
   HorizonGx2StockSky *sky = NULL;
   const float vertices[6][3] = {
      {-0.9f, -0.9f, 0.5f}, {-0.1f, -0.9f, 0.5f}, {-0.1f, 0.9f, 0.5f},
      {-0.9f, -0.9f, 0.5f}, {-0.1f, 0.9f, 0.5f}, {-0.9f, 0.9f, 0.5f},
   };
   bool ok = horizon_gx2_stars_create(&stars, vertices, 6) == 0 &&
      horizon_gx2_stock_sky_create(&sky) == 0;
   if (ok) {
      const float fog[4] = {0, 0, 0, 1}, top[4] = {0.125f, 0.25f, 0.375f, 1};
      const float sunrise[4] = {0};
      const uint32_t counts[3] = {24, 0, 24};
      HorizonSkyVertex sky_vertices[48] = {0};
      sky_quad(sky_vertices, -1, 1, 1);
      sky_quad(sky_vertices + 24, 0, 1, -0.1f);
      const float clip_rows[2][4][4] = {
         {{1, 0, 0, 0}, {0, 1, 0, 0}, {0, 0, 1, 0}, {0, 0, 0, 1}},
         {{-2, 0, 0, 0}, {0, 2, 0, 0}, {0, 0, 2, 0}, {0, 0, 0, 2}},
      };
      WHBGfxBeginRender();
      for (uint32_t i = 0; i < 2; ++i) {
         if (i == 0) WHBGfxBeginRenderTV();
         else WHBGfxBeginRenderDRC();
         const GX2ColorBuffer *color = i == 0 ? WHBGfxGetTVColourBuffer() : WHBGfxGetDRCColourBuffer();
         const uint32_t rect[4] = {0, 0, color->surface.width, color->surface.height};
         ok &= horizon_gx2_stock_sky_draw_base(sky, i, rect, sky_vertices, counts, fog, 64, top, sunrise) == 0;
         ok &= horizon_gx2_stars_draw(stars, rect, clip_rows[i], i == 0 ? 0.5f : 0.25f) == 0;
         ok &= horizon_gx2_stock_sky_draw_dark(sky, i, rect, counts, fog, 64) == 0;
         ok &= samples(color, i);
         if (i == 0) WHBGfxFinishRenderTV();
         else WHBGfxFinishRenderDRC();
      }
      WHBGfxFinishRender();
   }
   OSReport("GX2 stars fixture %s\n", ok ? "passed" : "failed");
   horizon_gx2_stars_destroy(stars);
   horizon_gx2_stock_sky_destroy(sky);
   WHBGfxShutdown();
   WHBProcShutdown();
   return ok ? 0 : 1;
}
