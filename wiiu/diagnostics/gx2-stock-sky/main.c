#include "stock_sky.h"

#include <coreinit/debug.h>
#include <gx2/event.h>
#include <gx2/surface.h>
#include <gx2r/surface.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <whb/gfx.h>
#include <whb/proc.h>

static void quad(HorizonSkyVertex vertices[6], float x_min, float y_min, float x_max,
                 float y_max, float factor_0, float factor_1)
{
   const float corners[4][4] = {
      {x_min, y_min, 0.5f, 1.0f}, {x_max, y_min, 0.5f, 1.0f},
      {x_min, y_max, 0.5f, 1.0f}, {x_max, y_max, 0.5f, 1.0f},
   };
   const unsigned index[6] = {0, 1, 2, 2, 1, 3};
   for (unsigned i = 0; i < 6; ++i) {
      memcpy(vertices[i].clip, corners[index[i]], sizeof(vertices[i].clip));
      vertices[i].factors[0] = factor_0;
      vertices[i].factors[1] = factor_1;
   }
}

static bool check_output(const GX2ColorBuffer *color, uint32_t output)
{
   const GX2Surface *src = &color->surface;
   GX2Surface copied = {
      .dim = GX2_SURFACE_DIM_TEXTURE_2D, .width = src->width, .height = src->height,
      .depth = 1, .mipLevels = 1, .format = src->format,
      .aa = GX2_AA_MODE1X, .tileMode = GX2_TILE_MODE_LINEAR_SPECIAL,
   };
   const GX2RResourceFlags flags = GX2R_RESOURCE_BIND_TEXTURE |
      GX2R_RESOURCE_USAGE_CPU_READ | GX2R_RESOURCE_USAGE_GPU_WRITE;
   if (!GX2RCreateSurface(&copied, flags)) return false;
   GX2DrawDone();
   GX2CopySurface(src, 0, 0, &copied, 0, 0);
   GX2DrawDone();
   const uint8_t *pixels = GX2RLockSurfaceEx(&copied, 0, flags);
   bool ok = pixels != NULL;
   if (pixels) {
      const uint32_t x = src->width / 8, y = src->height / 4;
      const size_t offsets[] = {
         (size_t)y * copied.pitch + x,
         (size_t)y * copied.pitch + 3 * x,
         (size_t)y * copied.pitch + 5 * x,
         (size_t)y * copied.pitch + 7 * x,
         (size_t)(3 * y) * copied.pitch + 7 * x,
      };
      const uint8_t expected[5][3] = {
         {0, 0, 255}, {0, 128, 128}, {0, 255, 0}, {128, 0, 128}, {0, 0, 0},
      };
      for (unsigned i = 0; i < 5; ++i) {
         const uint8_t *rgb = pixels + offsets[i] * 4;
         bool match = true;
         for (unsigned channel = 0; channel < 3; ++channel)
            match &= abs((int)rgb[channel] - expected[i][channel]) <= 1;
         OSReport("GX2 stock sky fixture output=%u sample=%u rgb=%u,%u,%u pass=%u\n",
                  output, i, rgb[0], rgb[1], rgb[2], match);
         ok &= match;
      }
      GX2RUnlockSurfaceEx(&copied, 0, flags);
   }
   GX2RDestroySurfaceEx(&copied, flags);
   return ok;
}

int main(void)
{
   WHBProcInit();
   if (!WHBGfxInit()) return 1;
   HorizonGx2StockSky *sky = NULL;
   bool ok = horizon_gx2_stock_sky_create(&sky) == 0;
   if (ok) {
      const float end = 64.0f;
      const float fog[4] = {0, 1, 0, 1}, top[4] = {0, 0, 1, 1}, sunrise[4] = {1, 0, 0, 0.5f};
      const uint32_t counts[3] = {24, 48, 24};
      HorizonSkyVertex vertices[96];
      for (unsigned i = 0; i < 96; ++i)
         vertices[i] = (HorizonSkyVertex){.clip = {0, 0, 0.5f, 1}};
      quad(vertices, -1, -1, -0.5f, 1, 0, 0);
      quad(vertices + 6, -0.5f, -1, 0, 1, 32, 0);
      quad(vertices + 12, 0, -1, 0.5f, 1, 0, 65);
      quad(vertices + 18, 0.5f, -1, 1, 1, 0, 0);
      quad(vertices + 24, 0.5f, -1, 1, 1, 1, 0);
      quad(vertices + 72, 0.5f, -1, 1, -0.1f, 0, 0);
      WHBGfxBeginRender();
      for (uint32_t i = 0; i < 2; ++i) {
         if (i == 0) WHBGfxBeginRenderTV();
         else WHBGfxBeginRenderDRC();
         WHBGfxClearColor(1, 1, 1, 1);
         const GX2ColorBuffer *color = i == 0 ? WHBGfxGetTVColourBuffer() : WHBGfxGetDRCColourBuffer();
         const uint32_t rect[4] = {0, 0, color->surface.width, color->surface.height};
         ok &= horizon_gx2_stock_sky_draw(sky, i, rect, vertices, counts, fog, end, top, sunrise) == 0;
         ok &= check_output(color, i);
         if (i == 0) WHBGfxFinishRenderTV();
         else WHBGfxFinishRenderDRC();
      }
      WHBGfxFinishRender();
   }
   OSReport("GX2 stock sky fixture %s\n", ok ? "passed" : "failed");
   horizon_gx2_stock_sky_destroy(sky);
   WHBGfxShutdown();
   WHBProcShutdown();
   return ok ? 0 : 1;
}
