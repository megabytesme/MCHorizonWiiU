#include "terrain.h"
#include <coreinit/debug.h>
#include <gx2/event.h>
#include <gx2/surface.h>
#include <gx2/texture.h>
#include <gx2/utils.h>
#include <gx2r/surface.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>
#include <whb/gfx.h>
#include <whb/proc.h>

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
                            uint32_t pass);

static const GX2RResourceFlags atlas_flags = GX2R_RESOURCE_BIND_TEXTURE |
   GX2R_RESOURCE_USAGE_CPU_WRITE | GX2R_RESOURCE_USAGE_GPU_READ;

static bool create_atlas(GX2Texture *atlas)
{
   atlas->surface = (GX2Surface){
      .dim = GX2_SURFACE_DIM_TEXTURE_2D, .width = 1, .height = 1, .depth = 1, .mipLevels = 1,
      .format = GX2_SURFACE_FORMAT_UNORM_R8_G8_B8_A8,
      .tileMode = GX2_TILE_MODE_LINEAR_ALIGNED,
   };
   if (!GX2RCreateSurface(&atlas->surface, atlas_flags)) return false;
   uint8_t *pixels = GX2RLockSurfaceEx(&atlas->surface, 0, atlas_flags);
   if (!pixels) return false;
   memset(pixels, 255, 4);
   GX2RUnlockSurfaceEx(&atlas->surface, 0, atlas_flags);
   atlas->viewNumMips = atlas->viewNumSlices = 1;
   atlas->compMap = GX2_COMP_MAP(GX2_SQ_SEL_R, GX2_SQ_SEL_G, GX2_SQ_SEL_B, GX2_SQ_SEL_A);
   GX2InitTextureRegs(atlas);
   return true;
}

static void quad(HandVertex out[4], float left, float right, float depth, const uint8_t rgba[4])
{
   const float xy[4][2] = {{left, -1}, {right, -1}, {right, 1}, {left, 1}};
   for (unsigned i = 0; i < 4; ++i) {
      out[i] = (HandVertex){
         .position_light = {xy[i][0], xy[i][1], depth, 0},
         .uv = {0.5f, 0.5f},
      };
      memcpy(out[i].color, rgba, 4);
   }
}

static int fill(void *source, HorizonTerrainVertex *vertices, uint32_t count)
{
   memcpy(vertices, source, count * sizeof(*vertices));
   return 0;
}

static bool readback(const GX2ColorBuffer *color, uint32_t target)
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
      for (unsigned i = 0; i < 2; ++i) {
         const unsigned x = (i ? 3 * src->width : src->width) / 4;
         const uint8_t *rgb = pixels + ((size_t)(src->height / 2) * copy.pitch + x) * 4;
         const bool match = !i ? rgb[0] == 0 && rgb[1] == 255 && rgb[2] == 0 :
            rgb[0] >= 127 && rgb[0] <= 129 && rgb[1] == 0 && rgb[2] >= 126 && rgb[2] <= 128;
         OSReport("GX2 particle fixture output=%u sample=%u rgb=%u,%u,%u pass=%u\n",
                  target, i, rgb[0], rgb[1], rgb[2], match);
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
   GX2Texture atlas = {0};
   HorizonGx2Hand *models = NULL;
   HorizonGx2Terrain *terrain = NULL;
   HorizonGx2TerrainSection *probe = NULL;
   const uint8_t white[4] = {255, 255, 255, 255};
   uint8_t lightmap[16 * 16 * 4];
   memset(lightmap, 255, sizeof(lightmap));
   HandVertex world[8], particle[4];
   quad(world, -1, 0, 0.25f, (uint8_t[4]){0, 255, 0, 255});
   quad(world + 4, 0, 1, 0.85f, (uint8_t[4]){0, 0, 255, 255});
   quad(particle, -1, 1, 0.50f, (uint8_t[4]){255, 0, 0, 128});
   const float probe_xy[4][2] = {{0, -1}, {1, -1}, {1, 1}, {0, 1}};
   HorizonTerrainVertex probe_vertices[4];
   for (unsigned i = 0; i < 4; ++i) {
      probe_vertices[i] = (HorizonTerrainVertex){
         .position = {probe_xy[i][0], probe_xy[i][1], 0.65f, 0},
         .uv = {0.5f, 0.5f}, .color_cutout = {255, 255, 0, 0},
      };
   }
   uint32_t bytes;
   bool ok = create_atlas(&atlas) && horizon_gx2_hand_create(&models) == 0 &&
      horizon_gx2_terrain_create(&terrain, _binary_terrain_gsh_start, 1, 1, white, sizeof(white)) == 0 &&
      horizon_gx2_terrain_lightmap(terrain, lightmap) == 0 &&
      horizon_gx2_terrain_section_create(&probe, 4, fill, probe_vertices, &bytes) == 0;
   if (ok) {
      const float identity[16] = {1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1};
      const float zero[4] = {0}, ranges[4] = {10, 20, 10, 20};
      WHBGfxBeginRender();
      for (unsigned i = 0; i < 2; ++i) {
         if (!i) WHBGfxBeginRenderTV();
         else WHBGfxBeginRenderDRC();
         WHBGfxClearColor(0, 0, 0, 1);
         const GX2ColorBuffer *color = i ? WHBGfxGetDRCColourBuffer() : WHBGfxGetTVColourBuffer();
         const uint32_t w = color->surface.width, h = color->surface.height;
         const uint32_t rect[4] = {0, 0, w, h};
         ok &= horizon_gx2_draw_models(models, i, rect, &atlas, terrain, world, 8,
                                        identity, zero, ranges, 0, 0) == 0;
         ok &= horizon_gx2_draw_models(models, i, rect, &atlas, terrain, particle, 4,
                                        identity, zero, ranges, 0, 2) == 0;
         ok &= horizon_gx2_terrain_draw(terrain, probe, w, h, 0, 0, w, h,
                                        identity, zero, zero, ranges) == 0;
         ok &= readback(color, i);
         if (!i) WHBGfxFinishRenderTV();
         else WHBGfxFinishRenderDRC();
      }
      WHBGfxFinishRender();
   }
   if (probe) horizon_gx2_terrain_section_destroy(probe);
   if (models) horizon_gx2_hand_destroy(models);
   if (terrain) horizon_gx2_terrain_destroy(terrain);
   if (atlas.surface.image) GX2RDestroySurfaceEx(&atlas.surface, atlas_flags);
   OSReport("GX2 particle fixture %s\n", ok ? "passed" : "failed");
   WHBGfxShutdown();
   WHBProcShutdown();
   return ok ? 0 : 1;
}
