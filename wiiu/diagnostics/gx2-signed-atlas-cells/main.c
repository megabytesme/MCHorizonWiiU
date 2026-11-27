#include "terrain.h"
#include "retire.h"
#include <coreinit/memdefaultheap.h>
#include <gx2/clear.h>
#include <stdlib.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern const uint8_t _binary_stock_gsh_start[];
extern const uint8_t _binary_atlas_cell_gsh_start[];

enum { COLUMNS = 31, ROWS = 42, CELL = 66, WIDTH = COLUMNS * CELL, HEIGHT = ROWS * CELL };
enum Arm { STOCK, PACKED, WRONG_CELL };

static int fill_vertices(void *source, HorizonTerrainVertex *vertices, uint32_t count)
{
   memcpy(vertices, source, count * sizeof(*vertices));
   return 0;
}

static int fill_cell_vertices(void *source, HorizonTerrainAtlasCellVertex *vertices, uint32_t count)
{
   memcpy(vertices, source, count * sizeof(*vertices));
   return 0;
}

static unsigned floor_log2(uint32_t v)
{
   return 31u - (unsigned)__builtin_clz(v | 1u);
}

static int16_t quantized_uv(uint16_t value, unsigned layer_byte)
{
   const uint32_t uv = value;
   const unsigned local = floor_log2(uv);
   const unsigned shift = layer_byte ? floor_log2(layer_byte) + 8 : (local > 8 ? local - 8 : 0);
   uint32_t rounded = uv;
   if (shift) {
      const unsigned quantum = 1u << shift;
      rounded = (uv + (quantum >> 1) - 1 + ((uv >> shift) & 1)) & ~(quantum - 1);
   }
   /* The preserved stock export cannot land at 65535/32768. */
   if (rounded > UINT16_MAX) rounded = UINT16_MAX;
   return (int32_t)rounded - 32768;
}

static void make_vertices(HorizonTerrainVertex vertices[64],
                          HorizonTerrainAtlasCellVertex packed[64], enum Arm arm)
{
   static const unsigned layers[16] = {0, 30, 31, 32, 61, 62, 63, 252, 253, 254, 255, 256, 1270, 1271, 1300, 1301};
   static const uint16_t corner_uv[4][2] = {{0, 0}, {32768, 1}, {32769, 32768}, {1, 65535}};
   for (unsigned quad = 0; quad < 16; quad++) {
      unsigned layer = layers[quad];
      unsigned column = layer % COLUMNS;
      const unsigned row = layer / COLUMNS;
      if (arm == WRONG_CELL) column = (column + 1) % COLUMNS;
      for (unsigned corner = 0; corner < 4; corner++) {
         const unsigned right = corner == 1 || corner == 2;
         const unsigned upper = corner >= 2;
         const uint16_t u = quad % 4 == 0 ? 32768 : corner_uv[corner][0];
         const uint16_t v = quad % 4 == 1 ? 0 : corner_uv[corner][1];
         const unsigned i = quad * 4 + corner;
         HorizonTerrainVertex out = {
            .position = {-1.0f + (quad % 4 + right) * 0.5f,
                         -1.0f + (quad / 4 + upper) * 0.5f,
                         0.25f, (float)((quad + corner) * 12 * 257)},
            .color_cutout = {255, 226, 196, quad % 2 ? 255 : 0},
         };
         if (arm == STOCK) {
            out.uv[0] = (float)(u | (layer & 255) << 16);
            out.uv[1] = (float)(v | (layer >> 8) << 16);
            vertices[i] = out;
         } else {
            packed[i] = (HorizonTerrainAtlasCellVertex){
               .uv_cell = { quantized_uv(u, layer & 255), quantized_uv(v, layer >> 8), column, row },
            };
            memcpy(packed[i].position, out.position, sizeof(out.position));
            memcpy(packed[i].color_cutout, out.color_cutout, sizeof(out.color_cutout));
         }
      }
   }
}

static bool render(Stage *stage, uint8_t *rgba, HorizonGx2Terrain *terrain,
                   HorizonGx2TerrainSection *section, enum Arm arm,
                   bool varying, uint8_t *baseline[2])
{
   const float color[4] = {0.3f, 0.5f, 0.8f, 1}, ranges[4] = {0, 1024, 86.4f, 96};
   const float origin[4] = {42, 53, 11, 0};
   bool ok = true;
   WHBGfxBeginRender();
   for (unsigned output = 0; output < 2; output++) {
      if (!output) WHBGfxBeginRenderTV();
      else WHBGfxBeginRenderDRC();
      GX2ColorBuffer *target = output ? WHBGfxGetDRCColourBuffer() : WHBGfxGetTVColourBuffer();
      GX2DepthBuffer *depth = output ? WHBGfxGetDRCDepthBuffer() : WHBGfxGetTVDepthBuffer();
      const uint32_t w = target->surface.width, h = target->surface.height;
      const size_t pixels = (size_t)w * h;
      GX2ClearBuffersEx(target, depth, 0, 0, 0, 1, 1, 0, GX2_CLEAR_FLAGS_DEPTH);
      GX2SetContextState(output ? WHBGfxGetDRCContextState() : WHBGfxGetTVContextState());
      GX2SetColorBuffer(target, GX2_RENDER_TARGET_0);
      GX2SetDepthBuffer(depth);
      const float clip[16] = {output ? 0.90f : 1.0f,0,0,output ? 0.05f : 0.0f,
                              0,1,0,0, 0,0,1,0, 0,0,0,1};
      if (horizon_gx2_terrain_bind(terrain,w,h,0,0,w,h,color,ranges,false) ||
          horizon_gx2_terrain_draw_bound(terrain,section,clip,origin))
         return false;
      const enum ReadStatus status = stage_read(stage, target, rgba, RGBA_CAP);
      if (status != READ_OK) {
         OSReport("GX2 atlas cell readback output=%u arm=%u status=%u\n", output, arm, status);
         return false;
      }
      if (arm == STOCK) {
         baseline[output] = malloc(pixels * 4);
         if (!baseline[output]) return false;
         memcpy(baseline[output], rgba, pixels * 4);
      }
      unsigned drawn = 0, changed = 0, cutout = 0;
      for (size_t i = 0; i < pixels; i++) {
         const uint8_t *pixel = rgba + i * 4;
         drawn += pixel[0] || pixel[1] || pixel[2];
         cutout += pixel[3] == 127;
         changed += memcmp(pixel, baseline[output] + i * 4, 4) != 0;
      }
      const bool pass = drawn > pixels / 2 && cutout > pixels / 10 &&
         (arm == WRONG_CELL ? changed > pixels / 2 : changed == 0);
      OSReport("GX2 atlas cell output=%u arm=%u varying=%u size=%ux%u drawn=%u cutout=%u changed=%u result=%s\n",
               output, arm, varying, w, h, drawn, cutout, changed, pass ? "PASS" : "FAIL");
      ok &= pass;
      if (!output) WHBGfxFinishRenderTV();
      else WHBGfxFinishRenderDRC();
   }
   WHBGfxFinishRender();
   return ok;
}

int main(void)
{
   WHBProcInit();
   const bool graphics = WHBGfxInit();
   static uint8_t atlas[WIDTH * HEIGHT * 4], light[16 * 16 * 4];
   for (unsigned y = 0; y < HEIGHT; y++) {
      for (unsigned x = 0; x < WIDTH; x++) {
         uint8_t *pixel = atlas + (y * WIDTH + x) * 4;
         const unsigned layer = y / CELL * COLUMNS + x / CELL;
         const unsigned ix = x % CELL == 0 ? 1 : x % CELL == CELL - 1 ? CELL - 2 : x % CELL;
         const unsigned iy = y % CELL == 0 ? 1 : y % CELL == CELL - 1 ? CELL - 2 : y % CELL;
         memcpy(pixel, (uint8_t[]){48 + (layer * 13 + ix * 8) % 170,
                                  90 + (layer * 31 + iy * 8) % 120,
                                  88 + (layer * 43 + ix * 6) % 151,
                                  (ix + iy) % 2 ? 255 : 127}, 4);
      }
   }
   HorizonGx2Terrain *terrain[2] = {0};
   HorizonGx2TerrainSection *sections[3] = {0};
   bool ok = graphics;
   for (unsigned i = 0; ok && i < 2; i++)
      ok = !horizon_gx2_terrain_create(&terrain[i], _binary_stock_gsh_start, WIDTH, HEIGHT, atlas, sizeof(atlas)) &&
           !horizon_gx2_terrain_set_sprite_grid(terrain[i], COLUMNS, CELL);
   if (ok) {
      ok = horizon_gx2_terrain_load_atlas_cells(terrain[1], _binary_atlas_cell_gsh_start);
      OSReport("GX2 atlas-cell quantized hosted loader=%s\n", ok ? "ready" : "rejected");
   }
   for (unsigned i = 0; ok && i < 3; i++) {
      HorizonTerrainVertex vertices[64];
      HorizonTerrainAtlasCellVertex packed[64];
      make_vertices(vertices, packed, i);
      uint32_t bytes;
      ok = i == STOCK ?
         !horizon_gx2_terrain_section_create(&sections[i], 64, fill_vertices, vertices, &bytes) :
         !horizon_gx2_terrain_section_create_atlas_cells(&sections[i], 64, 0, fill_cell_vertices, packed, &bytes);
   }
   Stage stage = {0};
   uint8_t *rgba = NULL;
   if (ok) {
      stage.storage = MEMAllocFromDefaultHeapEx(STAGE_CAP, STAGE_ALIGN);
      rgba = MEMAllocFromDefaultHeap(RGBA_CAP);
      ok = stage.storage && rgba;
   }
   for (unsigned varying = 0; ok && varying < 2; varying++) {
      for (unsigned i = 0; i < 256; i++)
         memcpy(light + i * 4, (uint8_t[]){varying ? 64 + i % 16 * 10 : 255,
                                         varying ? 247 - i / 16 * 9 : 255,
                                         varying ? 110 + i % 16 * 8 : 255, 255}, 4);
      uint8_t *baseline[2] = {0};
      ok = !horizon_gx2_terrain_lightmap(terrain[0], light) &&
           !horizon_gx2_terrain_lightmap(terrain[1], light) &&
           render(&stage, rgba, terrain[0], sections[0], STOCK, varying, baseline) &&
           render(&stage, rgba, terrain[1], sections[1], PACKED, varying, baseline) &&
           render(&stage, rgba, terrain[1], sections[2], WRONG_CELL, varying, baseline);
      free(baseline[0]);
      free(baseline[1]);
   }
   if (graphics) retire_for_teardown(&stage, "GX2 atlas cell");
   if (rgba) MEMFreeToDefaultHeap(rgba);
   if (stage.storage) MEMFreeToDefaultHeap(stage.storage);
   for (unsigned i = 0; i < 3; i++) horizon_gx2_terrain_section_destroy(sections[i]);
   for (unsigned i = 0; i < 2; i++) horizon_gx2_terrain_destroy(terrain[i]);
   if (graphics) WHBGfxShutdown();
   WHBProcShutdown();
   OSReport("GX2 atlas cell fixture result=%s\n", ok ? "PASS" : "FAIL");
   return ok ? 0 : 1;
}
