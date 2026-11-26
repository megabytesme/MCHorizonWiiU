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
int horizon_gx2_hand_draw(HorizonGx2Hand *hand, uint32_t target, const uint32_t rect[4],
                          const GX2Texture *atlas, const HorizonGx2Terrain *terrain,
                          const HandVertex *vertices, uint32_t count, const float clip_rows[16],
                          const float fog_color[4], const float fog_ranges[4], uint32_t nether);

#ifdef HORIZON_TEST_MODEL_OVERLAYS
void horizon_gx2_hand_begin_frame(HorizonGx2Hand *hand, bool share_uploads);
int horizon_gx2_draw_models_overlay(HorizonGx2Hand *hand, uint32_t target, const uint32_t rect[4],
                                     const GX2Texture *atlas, HorizonGx2Terrain *terrain,
                                     const HandVertex *vertices, uint32_t count, const float clip_rows[16],
                                     const float fog_color[4], const float fog_ranges[4], uint32_t nether,
                                     const float hurt_overlay[4]);
#endif

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

enum { MAX_READBACKS = 6 };
static GX2Surface readbacks[MAX_READBACKS];
static unsigned readback_count;
static const GX2RResourceFlags readback_flags = GX2R_RESOURCE_BIND_TEXTURE |
   GX2R_RESOURCE_USAGE_CPU_READ | GX2R_RESOURCE_USAGE_GPU_WRITE;

static bool samples(const GX2ColorBuffer *color, const char *scene, uint32_t output,
                    unsigned count, const uint8_t expected[][3])
{
   const GX2Surface *src = &color->surface;
   if (readback_count == MAX_READBACKS) return false;
   GX2Surface *copied = &readbacks[readback_count];
   *copied = (GX2Surface){
      .dim = GX2_SURFACE_DIM_TEXTURE_2D, .width = src->width, .height = src->height,
      .depth = 1, .mipLevels = 1, .format = src->format,
      .aa = GX2_AA_MODE1X, .tileMode = GX2_TILE_MODE_LINEAR_SPECIAL,
   };
   if (!GX2RCreateSurface(copied, readback_flags)) return false;
   ++readback_count;
   GX2DrawDone();
   GX2CopySurface(src, 0, 0, copied, 0, 0);
   GX2DrawDone();
   const uint8_t *pixels = GX2RLockSurfaceEx(copied, 0, readback_flags);
   bool ok = pixels != NULL;
   if (pixels) {
      const size_t row = (size_t)(src->height / 2) * copied->pitch;
      for (unsigned i = 0; i < count; ++i) {
         const uint32_t x = src->width * (2 * i + 1) / (2 * count);
         const uint8_t *rgb = pixels + (row + x) * 4;
         const bool match = memcmp(rgb, expected[i], 3) == 0;
         OSReport("GX2 hand fixture scene=%s output=%u sample=%u rgb=%u,%u,%u pass=%u\n",
                  scene, output, i, rgb[0], rgb[1], rgb[2], match);
         ok &= match;
      }
      GX2RUnlockSurfaceEx(copied, 0, readback_flags);
   }
   return ok;
}

#ifdef HORIZON_TEST_MODEL_OVERLAYS
static bool check_overlays(HorizonGx2Hand *hand, HorizonGx2Terrain *terrain, HorizonGx2Hud *hud,
                           HorizonGx2Outline *world,
                           const float identity[16], const float fog[4], const float ranges[4])
{
   const unsigned cases = 6;
   const uint8_t keys[] = {0, 255, 63, 165, 255, 255};
   HandVertex models[6 * 4];
   for (unsigned i = 0; i < cases; ++i) {
      const float left = -1.0f + (2.0f * i) / cases;
      const float right = -1.0f + (2.0f * (i + 1)) / cases;
      const float corners[4][2] = {{left, -1}, {right, -1}, {right, 1}, {left, 1}};
      for (unsigned j = 0; j < 4; ++j) {
         models[4 * i + j] = (HandVertex){
            // The lightmap is uniform, but the packed light uses both coordinates.
            .position_light = {corners[j][0], corners[j][1], 0.5f, keys[i] * 65536.0f + 0xa080},
            .uv = {i == 5 ? 0.75f : 0.25f, 0.5f},
            .color = {255, 255, 255, 255},
            // A down normal has exactly 0.4 cardinal light in the Overworld.
            .normal = {0, i == 4 ? -127 : 0, 0, i == 4 ? 127 : 0},
         };
      }
   }
   // Write a backdrop so the transparent sample does not depend on Cemu
   // materializing an untouched region of a deferred clear.
   const OutlineVertex backdrop[6] = {
      {.clip = {-1, -1, 0.75f, 1}, .rgba = {0, 0, 255, 255}},
      {.clip = { 1, -1, 0.75f, 1}, .rgba = {0, 0, 255, 255}},
      {.clip = { 1,  1, 0.75f, 1}, .rgba = {0, 0, 255, 255}},
      {.clip = {-1, -1, 0.75f, 1}, .rgba = {0, 0, 255, 255}},
      {.clip = { 1,  1, 0.75f, 1}, .rgba = {0, 0, 255, 255}},
      {.clip = {-1,  1, 0.75f, 1}, .rgba = {0, 0, 255, 255}},
   };
   const float hurt[4] = {1.0f, 0, 0, 178.0f / 255.0f};
   const float custom[4] = {0, 0, 1.0f, 178.0f / 255.0f};
   const uint8_t expected[2][6][3] = {
      {{0, 128, 0}, {39, 89, 0}, {96, 128, 96}, {45, 128, 45}, {39, 36, 0}, {0, 0, 255}},
      {{0, 128, 0}, {0, 89, 39}, {96, 128, 96}, {45, 128, 45}, {0, 36, 39}, {0, 0, 255}},
   };
   bool ok = true;
   for (unsigned arm = 0; arm < 2; ++arm) {
      horizon_gx2_hand_begin_frame(hand, arm != 0);
      WHBGfxBeginRender();
      for (uint32_t i = 0; i < 2; ++i) {
         if (i == 0) WHBGfxBeginRenderTV();
         else WHBGfxBeginRenderDRC();
         WHBGfxClearColor(0, 0, 1, 1);
         const GX2ColorBuffer *color = i == 0 ? WHBGfxGetTVColourBuffer() : WHBGfxGetDRCColourBuffer();
         const uint32_t rect[4] = {0, 0, color->surface.width, color->surface.height};
         ok &= horizon_gx2_outline_draw(world, i, rect, backdrop, 6, 0, fog, ranges) == 0;
         const int drawn = horizon_gx2_draw_models_overlay(hand, i, rect, horizon_gx2_hud_atlas(hud), terrain,
                                                          models, cases * 4, identity, fog, ranges,
                                                          0, arm == 0 ? hurt : custom);
         OSReport("GX2 hand overlay dispatch arm=%u output=%u result=%d\n", arm, i, drawn);
         ok &= drawn == 0;
         ok &= samples(color, arm == 0 ? "stock-overlay" : "custom-overlay-shared", i, cases,
                       expected[arm]);
         if (i == 0) WHBGfxFinishRenderTV();
         else WHBGfxFinishRenderDRC();
      }
      WHBGfxFinishRender();
   }
   return ok;
}
#endif

int main(void)
{
   WHBProcInit();
   if (!WHBGfxInit()) return 1;
   HorizonGx2Hand *hand = NULL;
   HorizonGx2Hud *hud = NULL;
   HorizonGx2Terrain *terrain = NULL;
   HorizonGx2Outline *world = NULL;
   const uint8_t atlas[8] = {255, 255, 255, 255, 0, 0, 0, 0};
   const uint8_t green[4] = {0, 255, 0, 255};
   const HorizonHudPatch patch = {.width = 1, .height = 1, .rgba = green, .length = sizeof(green)};
   const HorizonHudFrame patches = {.patches = &patch, .patch_count = 1,
                                    .viewport_width = 1, .viewport_height = 1, .scale = 1};
   uint8_t lightmap[16 * 16 * 4];
   for (unsigned i = 0; i < sizeof(lightmap); i += 4) {
      lightmap[i] = lightmap[i + 1] = lightmap[i + 2] = 128;
      lightmap[i + 3] = 255;
   }
   bool ok = horizon_gx2_hand_create(&hand) == 0 &&
             horizon_gx2_outline_create(&world, 6) == 0 &&
             horizon_gx2_hud_create(&hud, _binary_hud_gsh_start, 2, 1, atlas, sizeof(atlas), 6) == 0 &&
             horizon_gx2_hud_apply_patches_after_wait(hud, &patches) == 0 &&
             horizon_gx2_terrain_create(&terrain, _binary_terrain_gsh_start, 2, 1, atlas, sizeof(atlas)) == 0 &&
             horizon_gx2_terrain_lightmap(terrain, lightmap) == 0;
   OSReport("GX2 hand fixture init=%u\n", ok);
   if (ok) {
      const float identity[16] = {1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1};
      const float fog[4] = {0, 0, 0, 0}, ranges[4] = {1, 2, 1, 2};
      const float corners[4][2] = {{-1, -1}, {1, -1}, {1, 1}, {-1, 1}};
      const unsigned indices[6] = {0, 1, 2, 0, 2, 3};
      const uint8_t expected[2][3] = {{0, 128, 0}, {255, 0, 0}};
      OutlineVertex wall[6];
      HandVertex item[6];
      for (unsigned j = 0; j < 6; ++j) {
         const float *p = corners[indices[j]];
         wall[j] = (OutlineVertex){.clip = {p[0], p[1], 0.25f, 1}, .rgba = {255, 0, 0, 255}};
         item[j] = (HandVertex){.position_light = {p[0], p[1], 0.75f, 0},
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
         ok &= horizon_gx2_hand_draw(hand, i, rect, horizon_gx2_hud_atlas(hud), terrain,
                                     item, 6, identity, fog, ranges, 0) == 0;
         ok &= samples(color, "hand-depth", i, 2, expected);
         if (i == 0) WHBGfxFinishRenderTV();
         else WHBGfxFinishRenderDRC();
      }
      WHBGfxFinishRender();
#ifdef HORIZON_TEST_MODEL_OVERLAYS
      ok &= check_overlays(hand, terrain, hud, world, identity, fog, ranges);
#endif
   }
   OSReport("GX2 hand fixture %s\n", ok ? "passed" : "failed");
   horizon_gx2_hand_destroy(hand);
   horizon_gx2_outline_destroy(world);
   horizon_gx2_hud_destroy(hud);
   horizon_gx2_terrain_destroy(terrain);
   for (unsigned i = 0; i < readback_count; ++i)
      GX2RDestroySurfaceEx(&readbacks[i], readback_flags);
   WHBGfxShutdown();
   WHBProcShutdown();
   return ok ? 0 : 1;
}