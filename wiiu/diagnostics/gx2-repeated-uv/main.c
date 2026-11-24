#include "terrain.h"
#include <coreinit/debug.h>
#include <gx2/event.h>
#include <gx2/surface.h>
#include <gx2r/surface.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern const uint8_t _binary_terrain_gsh_start[];
enum { COLS = 13, CELL = 4, ATLAS_WIDTH = COLS * CELL, ATLAS_HEIGHT = 80 };
static const unsigned target_layer = 256, neighbour_layer = 257;
static const uint8_t phases[][2][4] = {
   {{255, 0, 0, 255}, {0, 0, 255, 255}},
   {{255, 255, 0, 255}, {255, 0, 255, 255}},
};
static const uint8_t neighbour[2][4] = {{0, 255, 0, 255}, {0, 255, 0, 255}};

static void cell(uint32_t layer, const uint8_t colors[2][4], uint8_t *pixels, uint32_t pitch)
{
   uint32_t x0 = layer % COLS * CELL, y0 = layer / COLS * CELL;
   for (unsigned y = 0; y < CELL; ++y)
      for (unsigned x = 0; x < CELL; ++x)
         memcpy(pixels + ((y0 + y) * pitch + x0 + x) * 4, colors[x / 2], 4);
}

static int patch(void *phase, uint8_t *pixels, uint32_t pitch)
{
   cell(target_layer, phases[*(const unsigned *)phase], pixels, pitch);
   return 0;
}

static void quad(HorizonTerrainVertex out[4], uint32_t layer, float low, float high, uint32_t right_uv)
{
   const float points[4][2] = {{-1, low}, {1, low}, {1, high}, {-1, high}};
   for (unsigned i = 0; i < 4; ++i) {
      uint32_t u = (i == 1 || i == 2) ? right_uv : 0;
      out[i] = (HorizonTerrainVertex){
         .position = {points[i][0], points[i][1], 0.4f, 0.0f},
         .uv = {(float)(u | ((layer & 0xff) << 16)), (float)(16384 | ((layer >> 8) << 16))},
         .color_cutout = {255, 255, 255, 0},
      };
   }
}

static int fill(void *source, HorizonTerrainVertex *vertices, uint32_t count)
{
   memcpy(vertices, source, count * sizeof(*vertices));
   return 0;
}

static bool readback(const GX2ColorBuffer *buffer, unsigned target, unsigned phase)
{
   const GX2Surface *src = &buffer->surface;
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
      for (unsigned i = 0; i < 5; ++i) {
         const unsigned x = i == 4 ? src->width / 2 : ((i * 2 + 1) * src->width) / 8;
         const unsigned y = i == 4 ? src->height * 3 / 4 : src->height / 4;
         const uint8_t *rgb = pixels + ((size_t)y * copy.pitch + x) * 4;
         const uint8_t *expected = i == 4 ? neighbour[0] : phases[phase][i % 2];
         bool match = !memcmp(rgb, expected, 3);
         OSReport("GX2 terrain texture fixture output=%u phase=%u sample=%u rgb=%u,%u,%u pass=%u\n",
                  target, phase, i, rgb[0], rgb[1], rgb[2], match);
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
   uint8_t atlas[ATLAS_WIDTH * ATLAS_HEIGHT * 4] = {0};
   uint8_t lightmap[16 * 16 * 4];
   memset(lightmap, 255, sizeof(lightmap));
   cell(target_layer, phases[0], atlas, ATLAS_WIDTH);
   cell(neighbour_layer, neighbour, atlas, ATLAS_WIDTH);
   HorizonGx2Terrain *terrain = NULL;
   HorizonGx2TerrainSection *section = NULL;
   HorizonTerrainVertex vertices[8];
   quad(vertices, target_layer, 0, 1, 65535);
   quad(vertices + 4, neighbour_layer, -1, 0, 32768);
   uint32_t bytes;
   bool ok = horizon_gx2_terrain_create(&terrain, _binary_terrain_gsh_start,
                                          ATLAS_WIDTH, ATLAS_HEIGHT, atlas, sizeof(atlas)) == 0 &&
      horizon_gx2_terrain_set_sprite_grid(terrain, COLS, CELL) == 0 &&
      horizon_gx2_terrain_lightmap(terrain, lightmap) == 0 &&
      horizon_gx2_terrain_section_create(&section, 8, fill, vertices, &bytes) == 0;
   const float identity[16] = {1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1};
   const float zero[4] = {0}, ranges[4] = {10, 20, 10, 20};
   for (unsigned phase = 0; ok && phase < 2; ++phase) {
      if (phase == 1) {
         const uint32_t rects[][4] = {{target_layer % COLS * CELL, target_layer / COLS * CELL, CELL, CELL}};
         if (horizon_gx2_terrain_patch_atlas(terrain, patch, &phase, rects, 1) != 0) {
            OSReport("GX2 terrain texture fixture patch failed\n");
            ok = false;
            break;
         }
      }
      WHBGfxBeginRender();
      for (unsigned i = 0; i < 2; ++i) {
         if (i == 0) WHBGfxBeginRenderTV();
         else WHBGfxBeginRenderDRC();
         WHBGfxClearColor(0, 0, 0, 1);
         GX2ColorBuffer *color = i == 0 ? WHBGfxGetTVColourBuffer() : WHBGfxGetDRCColourBuffer();
         const uint32_t w = color->surface.width, h = color->surface.height;
         ok &= horizon_gx2_terrain_draw(terrain, section, w, h, 0, 0, w, h,
                                       identity, zero, zero, ranges) == 0;
         ok &= readback(color, i, phase);
         if (i == 0) WHBGfxFinishRenderTV();
         else WHBGfxFinishRenderDRC();
      }
      WHBGfxFinishRender();
   }
   if (section) horizon_gx2_terrain_section_destroy(section);
   if (terrain) horizon_gx2_terrain_destroy(terrain);
   OSReport("GX2 terrain texture fixture %s\n", ok ? "passed" : "failed");
   WHBGfxShutdown();
   WHBProcShutdown();
   return ok ? 0 : 1;
}
