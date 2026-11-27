/* Static dual-output comparison using the exact assembled terrain backend. */
#include "terrain.c"
#include "retire.h"
#include <gx2/clear.h>
#include <proc_ui/procui.h>
#include <whb/proc.h>

extern const uint8_t _binary_stock_gsh_start[];
extern const uint8_t _binary_clamped_gsh_start[];
extern const uint8_t _binary_gutter_gsh_start[];

#include "../gx2-tiled-terrain-atlas/draw_clear.h"

enum { SIDE = 64, CELL = 66, COLS = 31, WIDTH = 2046, HEIGHT = 2772, QUADS = 6 };
static const unsigned layers[] = {0, 1, 31, 511, 1023, 1300};
typedef struct { uint8_t *rgba; unsigned width, height; } Image;
typedef struct { bool translucent, wrong; } Geometry;

static int pixels(void *context, uint8_t *data, uint32_t pitch)
{
   if (!context || !data || pitch < WIDTH) return -1;
   const unsigned phase = *(const unsigned *)context;
   for (unsigned k = 0; k < QUADS; ++k) {
      const unsigned layer = layers[k];
      const unsigned ox = (layer % COLS) * CELL, oy = (layer / COLS) * CELL;
      for (unsigned y = 0; y < CELL; ++y) for (unsigned x = 0; x < CELL; ++x) {
         const unsigned tx = x == 0 ? 0 : x == CELL - 1 ? SIDE - 1 : x - 1;
         const unsigned ty = y == 0 ? 0 : y == CELL - 1 ? SIDE - 1 : y - 1;
         uint8_t *p = data + ((size_t)(oy + y) * pitch + ox + x) * 4;
         p[0] = 31 + (tx * 3 + ty * 7 + k * 17 + phase * 47) % 211;
         p[1] = 13 + (tx * 5 + ty * 2 + k * 29 + phase * 31) % 223;
         p[2] = 37 + (tx * 5 + ty * 3 + k * 41 + phase * 53) % 201;
         p[3] = ((tx ^ ty ^ k ^ phase) & 1) ? 128 : 127;
      }
   }
   return 0;
}

static int geometry(void *context, HorizonTerrainVertex *out, uint32_t size)
{
   if (!context || !out || size != QUADS * 4) return -1;
   const Geometry *scenario = context;
   for (unsigned q = 0; q < QUADS; ++q) {
      const float left = -0.87f + (q % 3) * 0.55f;
      const float bottom = -0.81f + (q / 3) * (scenario->translucent ? 0.46f : 0.77f);
      const float z = 0.45f - q * 0.044f;
      const unsigned tile = layers[scenario->wrong ? QUADS - q - 1 : q];
      for (unsigned i = 0; i < 4; ++i) {
         const bool top = i >= 2, right = i == 1 || i == 2;
         out[q * 4 + i] = (HorizonTerrainVertex) {
            .position = { left + (right ? 0.42f : 0), bottom + (top ? 0.61f : 0), z, 0 },
            .uv = { (float)((right ? (q & 1 ? 65535u : 32768u) : 0) | ((tile & 255u) << 16)),
                    (float)((top ? 0 : 32768u) | ((tile >> 8) << 16)) },
            .color_cutout = { 255, 237, 211, scenario->translucent ? 193 : (q & 1 ? 255 : 0) },
         };
      }
   }
   return 0;
}

static bool compare(Image *reference, const uint8_t *rgba, unsigned width, unsigned height,
                    unsigned arm, unsigned phase, bool trans, unsigned view)
{
   const size_t count = (size_t)width * height;
   const uint8_t *sentinel = rgba + ((size_t)height * width - 1) * 4;
   const bool clear_ok = sentinel[0] == 0 && sentinel[1] == 0 && sentinel[2] == 0 && sentinel[3] == 255;
   unsigned colored = 0, changed = 0, alpha = 0;
   if (arm && (!reference->rgba || reference->width != width || reference->height != height)) return false;
   for (size_t i = 0; i < count; ++i) {
      const uint8_t *pixel = rgba + i * 4;
      colored += pixel[0] || pixel[1] || pixel[2];
      if (arm && memcmp(pixel, reference->rgba + i * 4, 4)) {
         ++changed;
         alpha += pixel[3] != reference->rgba[i * 4 + 3];
      }
   }
   if (!arm) {
      reference->rgba = malloc(count * 4);
      if (!reference->rgba) return false;
      reference->width = width;
      reference->height = height;
      memcpy(reference->rgba, rgba, count * 4);
   }
   const bool ok = clear_ok && colored > count / 8 && (arm == 4 ? changed > count / 10 : changed == 0);
   OSReport("GX2 atlas parity arm=%u phase=%u trans=%u out=%u pixels=%u coverage=%u clear=%u diff=%u alpha=%u result=%s\n",
            arm, phase, (unsigned)trans, view, (unsigned)count, colored, clear_ok, changed, alpha, ok ? "PASS" : "FAIL");
   return ok;
}

static bool run(Stage *stage, uint8_t *rgba, Image reference[2], const DrawClear *clear,
                const uint8_t *gfd, unsigned arm, unsigned phase, bool trans)
{
   static const float fog[4] = {0.16f,0.25f,0.32f,1}, ranges[4] = {0,4,0,3}, origin[4] = {0};
   static const float matrices[2][16] = {
      {1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1},
      {.91f,0,0,.035f, 0,.88f,0,-.04f, 0,0,1,0, 0,0,0,1},
   };
   HorizonGx2Terrain *terrain = NULL;
   HorizonGx2TerrainSection *section = NULL;
   const int created = horizon_gx2_terrain_create_mapped(&terrain, gfd, WIDTH, HEIGHT, pixels, &phase);
   if (created) {
      OSReport("GX2 atlas parity terrain allocation arm=%u result=%d\n",arm,created);
      return false;
   }
   static uint8_t light[16*16*4];
   memset(light, 255, sizeof(light));
   Geometry input = {trans, arm == 4};
   uint32_t bytes = 0;
   bool ok = !horizon_gx2_terrain_set_sprite_grid(terrain, COLS, CELL) &&
      !horizon_gx2_terrain_lightmap(terrain, light) &&
      !horizon_gx2_terrain_section_create(&section, QUADS*4, geometry, &input, &bytes);
   if (ok) {
      OSTime readback[2] = {0};
      WHBGfxBeginRender();
      for (unsigned out = 0; out < 2; ++out) {
         if (out) WHBGfxBeginRenderDRC(); else WHBGfxBeginRenderTV();
         GX2ColorBuffer *target = out ? WHBGfxGetDRCColourBuffer() : WHBGfxGetTVColourBuffer();
         GX2DepthBuffer *depth = out ? WHBGfxGetDRCDepthBuffer() : WHBGfxGetTVDepthBuffer();
         const unsigned w = target->surface.width, h = target->surface.height;
         GX2SetContextState(out ? WHBGfxGetDRCContextState() : WHBGfxGetTVContextState());
         GX2SetColorBuffer(target, GX2_RENDER_TARGET_0);
         GX2SetDepthBuffer(depth);
         draw_clear_output(clear, terrain, w, h, false);
         if (horizon_gx2_terrain_bind(terrain,w,h,0,0,w,h,fog,ranges,trans) != 0 ||
             horizon_gx2_terrain_draw_bound(terrain,section,matrices[out],origin) != 0 ||
             stage_read_with_receipt(stage,target,rgba,RGBA_CAP,&readback[out]) != READ_OK) {
            OSReport("GX2 atlas parity readback arm=%u phase=%u trans=%u out=%u FAIL\n",arm,phase,(unsigned)trans,out);
            ok = false;
         } else {
            ok &= compare(&reference[out],rgba,w,h,arm,phase,trans,out);
         }
         if (out) WHBGfxFinishRenderDRC(); else WHBGfxFinishRenderTV();
      }
      WHBGfxFinishRender();
      ok &= readback[0] > 0 && readback[1] > readback[0];
   }
   retire_for_teardown(stage,"GX2 atlas parity");
   horizon_gx2_terrain_section_destroy_after_wait(section);
   horizon_gx2_terrain_destroy(terrain);
   return ok;
}

int main(void)
{
   WHBProcInit();
   const bool graphics = WHBGfxInit();
   DrawClear clear = {0};
   Stage stage = {0};
   uint8_t *rgba = NULL;
   bool ok = false;
   if (graphics) {
      stage.storage = MEMAllocFromDefaultHeapEx(STAGE_CAP,STAGE_ALIGN);
      rgba = MEMAllocFromDefaultHeap(RGBA_CAP);
      if (stage.storage && rgba && draw_clear_init(&clear)) {
         const uint8_t *programs[] = {_binary_stock_gsh_start, _binary_stock_gsh_start,
             _binary_clamped_gsh_start, _binary_gutter_gsh_start, _binary_stock_gsh_start};
         ok = true;
         for (unsigned phase = 0; phase < 2; ++phase) for (unsigned trans = 0; trans < 2; ++trans) {
            Image baseline[2] = {0};
            static const unsigned orders[2][5] = {{0,1,2,3,4}, {0,3,2,1,4}};
            for (unsigned slot = 0; slot < sizeof(orders[phase])/sizeof(*orders[phase]); ++slot) {
               const unsigned arm = orders[phase][slot];
               ok &= run(&stage,rgba,baseline,&clear,programs[arm],arm,phase,trans != 0);
            }
            free(baseline[0].rgba);
            free(baseline[1].rgba);
         }
      }
      if (clear.positions) MEMFreeToDefaultHeap(clear.positions);
      if (clear.group.vertexShader) WHBGfxFreeShaderGroup(&clear.group);
      if (rgba) MEMFreeToDefaultHeap(rgba);
      if (stage.storage) MEMFreeToDefaultHeap(stage.storage);
      WHBGfxShutdown();
   }
   WHBProcShutdown();
   OSReport("GX2 atlas parity fixture result=%s\n",ok ? "PASS" : "FAIL");
   return ok ? 0 : 1;
}
