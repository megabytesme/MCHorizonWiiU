#include "hud.h"
#include "precipitation.h"

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

extern const uint8_t _binary_hud_gsh_start[];
extern const uint8_t _binary_terrain_gsh_start[];

typedef struct HorizonGx2Outline HorizonGx2Outline;
typedef struct OutlineVertex {
   float clip[4], fog_distance[2];
   uint8_t rgba[4];
} OutlineVertex;

int horizon_gx2_outline_create(HorizonGx2Outline **out, uint32_t capacity);
void horizon_gx2_outline_destroy(HorizonGx2Outline *outline);
int horizon_gx2_outline_draw(HorizonGx2Outline *outline, uint32_t target, const uint32_t rect[4],
                             const OutlineVertex *vertices, uint32_t count, uint32_t secondary_count,
                             const float fog_color[4], const float fog_ranges[4]);

static void weather_quad(HorizonPrecipitationVertex vertices[4], float left, float right,
                         float depth, float uv, float origin)
{
   const float corners[4][2] = {{left, -1}, {right, -1}, {right, 1}, {left, 1}};
   for (unsigned i = 0; i < 4; ++i) {
      vertices[i] = (HorizonPrecipitationVertex){
         .position_light = {corners[i][0], corners[i][1], depth, 0},
         .uv = {uv, 0.5f}, .rect = {origin, 0, 0.5f, 1}, .rgba = {255, 255, 255, 128},
      };
   }
}

static void wall(OutlineVertex vertices[6], float left, float right, float depth)
{
   const float corners[4][2] = {{left, -1}, {right, -1}, {right, 1}, {left, 1}};
   const unsigned indices[6] = {0, 1, 2, 0, 2, 3};
   for (unsigned i = 0; i < 6; ++i) {
      const float *p = corners[indices[i]];
      vertices[i] = (OutlineVertex){.clip = {p[0], p[1], depth, 1}, .rgba = {255, 0, 0, 255}};
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
      const size_t row = (size_t)(src->height / 2) * copied.pitch;
      const uint8_t expected[4][3] = {{255, 0, 0}, {63, 32, 64}, {255, 0, 0}, {127, 0, 64}};
      for (unsigned i = 0; i < 4; ++i) {
         const uint8_t *rgb = pixels + (row + src->width * (i * 2 + 1) / 8) * 4;
         bool match = true;
         for (unsigned c = 0; c < 3; ++c)
            match &= abs((int)rgb[c] - expected[i][c]) <= 1;
         OSReport("GX2 precipitation fixture output=%u sample=%u rgb=%u,%u,%u pass=%u\n",
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
   HorizonGx2Precipitation *weather = NULL;
   HorizonGx2Hud *hud = NULL;
   HorizonGx2Terrain *terrain = NULL;
   HorizonGx2Outline *world = NULL;
   const uint8_t atlas[16] = {
      0, 255, 0, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 0, 255,
   };
   uint8_t lightmap[16 * 16 * 4];
   for (unsigned i = 0; i < sizeof(lightmap); i += 4) {
      lightmap[i] = lightmap[i + 1] = lightmap[i + 2] = 128;
      lightmap[i + 3] = 255;
   }
   HorizonPrecipitationVertex quads[20];
   weather_quad(quads, -1, -0.5f, 0.5f, -0.75f, 0);
   weather_quad(quads + 4, -0.5f, 0, 0.5f, -0.75f, 0);
   weather_quad(quads + 8, 0, 0.5f, 0.5f, -0.25f, 0);
   weather_quad(quads + 12, -0.5f, 0, 0.6f, 1.25f, 0.5f);
   weather_quad(quads + 16, 0.5f, 1, 0.5f, 1.25f, 0.5f);
   bool ok = horizon_gx2_precipitation_create(&weather) == 0 &&
             horizon_gx2_outline_create(&world, 12) == 0 &&
             horizon_gx2_hud_create(&hud, _binary_hud_gsh_start, 4, 1, atlas, sizeof(atlas), 6) == 0 &&
             horizon_gx2_terrain_create(&terrain, _binary_terrain_gsh_start, 4, 1, atlas, sizeof(atlas)) == 0 &&
             horizon_gx2_terrain_lightmap(terrain, lightmap) == 0 &&
             horizon_gx2_precipitation_upload(weather, quads, 20) == 0;
   OSReport("GX2 precipitation fixture init=%u\n", ok);
   if (ok) {
      const float identity[16] = {1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1};
      const float fog[4] = {0, 0, 0, 0}, ranges[4] = {1, 2, 1, 2};
      OutlineVertex background[12];
      wall(background, -1, 1, 0.75f);
      wall(background + 6, -1, -0.5f, 0.25f);
      WHBGfxBeginRender();
      for (uint32_t i = 0; i < 2; ++i) {
         if (i == 0) WHBGfxBeginRenderTV();
         else WHBGfxBeginRenderDRC();
         WHBGfxClearColor(1, 1, 1, 1);
         const GX2ColorBuffer *color = i == 0 ? WHBGfxGetTVColourBuffer() : WHBGfxGetDRCColourBuffer();
         const uint32_t rect[4] = {0, 0, color->surface.width, color->surface.height};
         ok &= horizon_gx2_outline_draw(world, i, rect, background, 12, 0, fog, ranges) == 0;
         ok &= horizon_gx2_precipitation_draw(weather, rect, horizon_gx2_hud_atlas(hud),
                                             terrain, identity, fog, ranges) == 0;
         ok &= samples(color, i);
         if (i == 0) WHBGfxFinishRenderTV();
         else WHBGfxFinishRenderDRC();
      }
      WHBGfxFinishRender();
   }
   OSReport("GX2 precipitation fixture %s\n", ok ? "passed" : "failed");
   horizon_gx2_precipitation_destroy(weather);
   horizon_gx2_outline_destroy(world);
   horizon_gx2_hud_destroy(hud);
   horizon_gx2_terrain_destroy(terrain);
   WHBGfxShutdown();
   WHBProcShutdown();
   return ok ? 0 : 1;
}
