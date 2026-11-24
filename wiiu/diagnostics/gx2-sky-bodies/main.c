#include "hud.h"
#include "sky_bodies.h"
#include "stock_sky.h"

#include <coreinit/debug.h>
#include <gx2/event.h>
#include <gx2/surface.h>
#include <gx2r/surface.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern const uint8_t _binary_hud_gsh_start[];

static const unsigned indices[6] = {0, 1, 2, 0, 2, 3};

static void sky_quad(HorizonSkyVertex vertices[6], float left, float bottom,
                     float right, float top, float coverage)
{
   const float corners[4][2] = {{left, bottom}, {right, bottom}, {right, top}, {left, top}};
   for (unsigned i = 0; i < 6; ++i) {
      const float *p = corners[indices[i]];
      vertices[i] = (HorizonSkyVertex){.clip = {p[0], p[1], 0.5f, 1}, .factors = {coverage, 0}};
   }
}

static void body_quad(HorizonSkyBodyVertex vertices[6], float left, float right,
                      float u0, float u1, uint8_t rain)
{
   const float corners[4][3] = {{left, -1, u0}, {right, -1, u1}, {right, 1, u1}, {left, 1, u0}};
   for (unsigned i = 0; i < 6; ++i) {
      const float *p = corners[indices[i]];
      vertices[i] = (HorizonSkyBodyVertex){
         .clip = {p[0], p[1], 0.5f, 1}, .uv = {p[2], 0.5f}, .rgba = {255, 255, 255, rain},
      };
   }
}

static bool samples(const GX2ColorBuffer *color, uint32_t output)
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
      const uint8_t expected[5][4] = {
         {64, 32, 64, 255}, {0, 0, 64, 255}, {0, 32, 128, 255},
         {132, 16, 32, 255}, {0, 0, 0, 255},
      };
      for (unsigned i = 0; i < 5; ++i) {
         const uint8_t *rgba = pixels + offsets[i] * 4;
         bool match = true;
         for (unsigned channel = 0; channel < 4; ++channel)
            match &= abs((int)rgba[channel] - expected[i][channel]) <= 1;
         OSReport("GX2 sky bodies fixture output=%u sample=%u rgba=%u,%u,%u,%u pass=%u\n",
                  output, i, rgba[0], rgba[1], rgba[2], rgba[3], match);
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
   HorizonGx2SkyBodies *bodies = NULL;
   HorizonGx2Hud *hud = NULL;
   const uint8_t atlas[16] = {255, 128, 0, 128, 255, 255, 255, 0,
                             0, 128, 255, 255, 64, 255, 0, 64};
   bool ok = horizon_gx2_stock_sky_create(&sky) == 0 &&
      horizon_gx2_sky_bodies_create(&bodies) == 0 &&
      horizon_gx2_hud_create(&hud, _binary_hud_gsh_start, 4, 1, atlas, sizeof(atlas), 6) == 0;
   OSReport("GX2 sky bodies fixture init=%u\n", ok);
   if (ok) {
      const float fog[4] = {0, 0, 0, 1}, top[4] = {0, 0, 0.25f, 1};
      const float sunrise[4] = {1, 0, 0, 0.5f};
      const uint32_t counts[3] = {24, 48, 24};
      HorizonSkyVertex sky_vertices[96];
      for (unsigned i = 0; i < 96; ++i)
         sky_vertices[i] = (HorizonSkyVertex){.clip = {0, 0, 0.5f, 1}};
      sky_quad(sky_vertices, -1, -1, 1, 1, 0);
      sky_quad(sky_vertices + 24, 0.5f, -1, 1, 1, 1);
      sky_quad(sky_vertices + 72, 0.5f, -1, 1, -0.1f, 0);
      HorizonSkyBodyVertex body_vertices[12];
      body_quad(body_vertices, -1, 0, 0, 0.5f, 128);
      body_quad(body_vertices + 6, 0, 1, 0.5f, 1, 64);
      WHBGfxBeginRender();
      for (uint32_t i = 0; i < 2; ++i) {
         if (i == 0) WHBGfxBeginRenderTV();
         else WHBGfxBeginRenderDRC();
         WHBGfxClearColor(1, 1, 1, 1);
         const GX2ColorBuffer *color = i == 0 ? WHBGfxGetTVColourBuffer() : WHBGfxGetDRCColourBuffer();
         const uint32_t rect[4] = {0, 0, color->surface.width, color->surface.height};
         ok &= horizon_gx2_stock_sky_draw_base(sky, i, rect, sky_vertices, counts, fog, 64, top, sunrise) == 0;
         ok &= horizon_gx2_sky_bodies_draw(bodies, i, rect, horizon_gx2_hud_atlas(hud), body_vertices, 12) == 0;
         ok &= horizon_gx2_stock_sky_draw_dark(sky, i, rect, counts, fog, 64) == 0;
         ok &= samples(color, i);
         if (i == 0) WHBGfxFinishRenderTV();
         else WHBGfxFinishRenderDRC();
      }
      WHBGfxFinishRender();
   }
   OSReport("GX2 sky bodies fixture %s\n", ok ? "passed" : "failed");
   horizon_gx2_sky_bodies_destroy(bodies);
   horizon_gx2_stock_sky_destroy(sky);
   horizon_gx2_hud_destroy(hud);
   WHBGfxShutdown();
   WHBProcShutdown();
   return ok ? 0 : 1;
}
