#include "hud.h"
#include "terrain.h"
#include <coreinit/debug.h>
#include <gx2/event.h>
#include <gx2/surface.h>
#include <gx2/texture.h>
#include <gx2r/surface.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern const uint8_t _binary_hud_gsh_start[];
extern const uint8_t _binary_terrain_gsh_start[];

typedef struct HorizonGx2Hand HorizonGx2Hand;
typedef struct HandVertex {
   float position_light[4], uv[2];
   uint8_t color[4];
   int8_t normal[4];
} HandVertex;
int horizon_gx2_hand_create(HorizonGx2Hand **out);
void horizon_gx2_hand_destroy(HorizonGx2Hand *hand);
int horizon_gx2_draw_models(HorizonGx2Hand *hand, uint32_t target, const uint32_t rect[4],
                             const GX2Texture *atlas, HorizonGx2Terrain *terrain,
                             const HandVertex *vertices, uint32_t count, const float clip_rows[16],
                             const float fog_color[4], const float fog_ranges[4], uint32_t nether,
                             uint32_t first_person);

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
      const uint32_t offsets[] = {src->width / 8, src->width * 3 / 8, src->width * 3 / 4};
      const uint8_t expected[3][3] = {{255, 0, 0}, {0, 128, 0}, {0, 0, 255}};
      for (unsigned i = 0; i < 3; ++i) {
         const uint8_t *rgb = pixels + (row + offsets[i]) * 4;
         const bool match = memcmp(rgb, expected[i], 3) == 0;
         OSReport("GX2 entity fixture output=%u sample=%u rgb=%u,%u,%u pass=%u\n",
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
   HorizonGx2Hand *hand = NULL;
   HorizonGx2Hud *hud = NULL;
   HorizonGx2Terrain *terrain = NULL;
   HorizonGx2Outline *world = NULL, *later = NULL;
   const uint8_t atlas[8] = {0, 255, 0, 255, 0, 0, 0, 0};
   uint8_t lightmap[16 * 16 * 4];
   for (unsigned i = 0; i < sizeof(lightmap); i += 4) {
      lightmap[i] = lightmap[i + 1] = lightmap[i + 2] = 128;
      lightmap[i + 3] = 255;
   }
   bool ok = horizon_gx2_hand_create(&hand) == 0 &&
             horizon_gx2_outline_create(&world, 6) == 0 &&
             horizon_gx2_outline_create(&later, 6) == 0 &&
             horizon_gx2_hud_create(&hud, _binary_hud_gsh_start, 2, 1, atlas, sizeof(atlas), 6) == 0 &&
             horizon_gx2_terrain_create(&terrain, _binary_terrain_gsh_start, 2, 1, atlas, sizeof(atlas)) == 0 &&
             horizon_gx2_terrain_lightmap(terrain, lightmap) == 0;
   OSReport("GX2 entity fixture init=%u\n", ok);
   if (ok) {
      const float identity[16] = {1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1};
      const float fog[4] = {0, 0, 0, 0}, ranges[4] = {1, 2, 1, 2};
      const float corners[4][2] = {{-1, -1}, {1, -1}, {1, 1}, {-1, 1}};
      const unsigned indices[6] = {0, 1, 2, 0, 2, 3};
      OutlineVertex wall[6], background[6];
      HandVertex entity[4];
      for (unsigned j = 0; j < 6; ++j) {
         const float *p = corners[indices[j]];
         wall[j] = (OutlineVertex){.clip = {p[0] * 0.25f - 0.75f, p[1], 0.25f, 1},
                                   .rgba = {255, 0, 0, 255}};
         background[j] = (OutlineVertex){.clip = {p[0], p[1], 0.875f, 1},
                                         .rgba = {0, 0, 255, 255}};
      }
      for (unsigned j = 0; j < 4; ++j) {
         const float *p = corners[j];
         entity[j] = (HandVertex){.position_light = {p[0], p[1], 0.75f, 0},
                                 .uv = {(p[0] + 1) * 0.5f, 0.5f}, .color = {255, 255, 255, 255},
                                 .normal = {0, 0, 0, 1}};
      }
      WHBGfxBeginRender();
      for (uint32_t i = 0; i < 2; ++i) {
         if (i == 0) WHBGfxBeginRenderTV();
         else WHBGfxBeginRenderDRC();
         WHBGfxClearColor(1, 1, 1, 1);
         const GX2ColorBuffer *color = i == 0 ? WHBGfxGetTVColourBuffer() : WHBGfxGetDRCColourBuffer();
         const uint32_t rect[4] = {0, 0, color->surface.width, color->surface.height};
         ok &= horizon_gx2_outline_draw(world, i, rect, wall, 6, 0, fog, ranges) == 0;
         ok &= horizon_gx2_draw_models(hand, i, rect, horizon_gx2_hud_atlas(hud), terrain,
                                       entity, 4, identity, fog, ranges, 0, 0) == 0;
         ok &= horizon_gx2_outline_draw(later, i, rect, background, 6, 0, fog, ranges) == 0;
         ok &= samples(color, i);
         if (i == 0) WHBGfxFinishRenderTV();
         else WHBGfxFinishRenderDRC();
      }
      WHBGfxFinishRender();
   }
   OSReport("GX2 entity fixture %s\n", ok ? "passed" : "failed");
   horizon_gx2_hand_destroy(hand);
   horizon_gx2_outline_destroy(later);
   horizon_gx2_outline_destroy(world);
   horizon_gx2_hud_destroy(hud);
   horizon_gx2_terrain_destroy(terrain);
   WHBGfxShutdown();
   WHBProcShutdown();
   return ok ? 0 : 1;
}
