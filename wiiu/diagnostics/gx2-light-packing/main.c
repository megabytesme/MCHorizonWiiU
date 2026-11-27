#include "terrain.h"
#include "retire.h"
#include "light-encoding.h"
#include <coreinit/memdefaultheap.h>
#include <gx2/draw.h>
#include <gx2/registers.h>
#include <gx2/shaders.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern const uint8_t _binary_stock_gsh_start[];
extern const uint8_t _binary_centered_gsh_start[];
extern const uint8_t _binary_combined_gsh_start[];
extern const uint8_t _binary_bad_alpha_gsh_start[];
extern const uint8_t _binary_clear_gsh_start[];

enum { Columns = 16, Rows = 17, Cell = 8, AtlasW = Columns * Cell, AtlasH = Rows * Cell };
enum { SolidQuads = 8, TranslucentQuads = 3 };
enum Shader { STOCK_SHADER, CENTERED_SHADER, COMBINED_SHADER, BAD_ALPHA_SHADER, ShaderCount };
enum Format { RAW, CENTERED, SWAPPED, FormatCount };
enum Arm { STOCK, CENTERED_ONLY, COMBINED, WRONG_LIGHT, WRONG_ALPHA, ArmCount };
struct SectionInput { enum Format format; bool translucent; };

typedef struct {
   WHBGfxShaderGroup shader;
   float *vertices;
   uint32_t clip[4], color;
} Clear;

typedef struct {
   HorizonGx2Terrain *owner[ShaderCount];
   HorizonGx2TerrainSection *solid[FormatCount], *translucent[FormatCount];
   Stage stage;
   Clear clear;
   uint8_t *rgba, *reference[2];
   unsigned coverage[2];
} Fixture;

static bool clear_init(Clear *clear)
{
   WHBGfxShaderGroup *shader = &clear->shader;
   if (!WHBGfxLoadGFDShaderGroup(shader, 0, _binary_clear_gsh_start) ||
       !WHBGfxInitShaderAttribute(shader, "aPosition", 0, 0, GX2_ATTRIB_FORMAT_FLOAT_32_32_32) ||
       !WHBGfxInitFetchShader(shader)) return false;
   static const char *const names[4] = {"uClipX", "uClipY", "uClipZ", "uClipW"};
   for (unsigned i = 0; i < 4; ++i) {
      const GX2UniformVar *var = GX2GetVertexUniformVar(shader->vertexShader, names[i]);
      if (!var) return false;
      clear->clip[i] = var->offset;
   }
   const GX2UniformVar *var = GX2GetPixelUniformVar(shader->pixelShader, "uColor");
   if (!var) return false;
   clear->color = var->offset;
   static const float verts[16] = {-1,-1,1,0, -1,1,1,0, 1,1,1,0, 1,-1,1,0};
   clear->vertices = MEMAllocFromDefaultHeapEx(sizeof(verts), 64);
   if (!clear->vertices) return false;
   memcpy(clear->vertices, verts, sizeof(verts));
   DCFlushRange(clear->vertices, sizeof(verts));
   return true;
}

static void draw_clear(const Clear *clear, HorizonGx2Terrain *terrain, unsigned w, unsigned h)
{
   static const _Alignas(16) float identity[16] = {1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1};
   static const _Alignas(16) float black[4] = {0,0,0,1};
   GX2SetViewport(0, 0, w, h, 0, 1);
   GX2SetScissor(0, 0, w, h);
   GX2SetShaderMode(GX2_SHADER_MODE_UNIFORM_REGISTER);
   GX2SetFetchShader(&clear->shader.fetchShader);
   GX2SetVertexShader(clear->shader.vertexShader);
   GX2SetPixelShader(clear->shader.pixelShader);
   for (unsigned i = 0; i < 4; ++i)
      GX2SetVertexUniformReg(clear->clip[i], 4, identity + i * 4);
   GX2SetPixelUniformReg(clear->color, 4, black);
   GX2SetAttribBuffer(0, 16 * sizeof(float), 4 * sizeof(float), clear->vertices);
   GX2SetCullOnlyControl(GX2_FRONT_FACE_CCW, FALSE, FALSE);
   GX2SetAlphaTest(FALSE, GX2_COMPARE_FUNC_ALWAYS, 0);
   GX2SetDepthOnlyControl(TRUE, TRUE, GX2_COMPARE_FUNC_ALWAYS);
   GX2SetColorControl(GX2_LOGIC_OP_COPY, 0, FALSE, TRUE);
   GX2SetTargetChannelMasks((GX2ChannelMask)15, 0, 0, 0, 0, 0, 0, 0);
   horizon_gx2_terrain_draw_quads(terrain, 4);
}

static int fill_vertices(void *raw, HorizonTerrainVertex *out, uint32_t count)
{
   const struct SectionInput *input = raw;
   if (count != 4u * (input->translucent ? TranslucentQuads : SolidQuads)) return -1;
   static const uint16_t tiles[SolidQuads] = {0, 15, 16, 63, 127, 255, 256, 271};
   static const uint16_t uv[4][2] = {{0,0}, {32768,1}, {65535,65535}, {1,32768}};
   static const uint16_t edges[8] = {0x00ff, 0xf000, 0xf1ef, 0x1080, 0xff10, 0x7811, 0x23f0, 0xefaa};
   for (unsigned i = 0; i < count; ++i) {
      const unsigned q = i / 4, c = i % 4;
      const bool right = c == 1 || c == 2, top = c >= 2;
      float x, y, z;
      if (input->translucent) {
         x = -.78f + .41f * q + (right ? .50f : 0);
         y = -.46f + .16f * q + (top ? .68f : 0);
         z = .27f - .02f * q;
      } else {
         x = -.91f + .46f * (q % 4) + (right ? .43f : 0);
         y = -.76f + .78f * (q / 4) + (top ? .71f : 0);
         z = .50f + .025f * q;
      }
      uint16_t light = i < 8 ? edges[i] :
         (uint16_t)((((q * 33 + c * 13) & 255) << 8) | ((q * 71 + c * 77 + 5) & 255));
      if (input->format == SWAPPED) light = (uint16_t)((light << 8) | (light >> 8));
      const float packed = input->format == RAW ? (float)light : centered_light(light);
      const uint16_t tile = tiles[q + (input->translucent ? 2 : 0)];
      const uint32_t u = ((tile & 255u) << 16) | uv[c][0];
      const uint32_t v = ((uint32_t)(tile >> 8) << 16) | uv[c][1];
      const uint8_t alpha = input->translucent ? 55 + 31 * c + 18 * q : (q & 1) ? 255 : 0;
      out[i] = (HorizonTerrainVertex) {
         .position = {x, y, z, packed}, .uv = {(float)u, (float)v},
         .color_cutout = {148 + 11 * q, 226 - 39 * c, 211 - 23 * q, alpha},
      };
   }
   return 0;
}

static void fill_assets(uint8_t *atlas, uint8_t *light)
{
   for (unsigned y = 0; y < AtlasH; ++y)
      for (unsigned x = 0; x < AtlasW; ++x) {
         const unsigned tile = y / Cell * Columns + x / Cell;
         const unsigned p = (y * AtlasW + x) * 4;
         atlas[p] = 48 + (tile * 67 + x * 5) % 160;
         atlas[p+1] = 48 + (tile * 37 + y * 7) % 160;
         atlas[p+2] = 48 + (tile * 9 + x * 3 + y * 5) % 160;
         atlas[p+3] = ((x % Cell >= 3) != (y % Cell >= 3)) ? 48 : 223;
      }
   for (unsigned y = 0; y < 16; ++y)
      for (unsigned x = 0; x < 16; ++x) {
         const unsigned p = (y * 16 + x) * 4;
         light[p] = 42 + 14 * x;
         light[p+1] = 233 - 12 * y;
         light[p+2] = 24 + 6 * (x + y);
         light[p+3] = 255;
      }
}

static bool init(Fixture *f)
{
   static uint8_t atlas[AtlasW * AtlasH * 4], light[16 * 16 * 4];
   const void *gfd[ShaderCount] = {_binary_stock_gsh_start, _binary_centered_gsh_start,
      _binary_combined_gsh_start, _binary_bad_alpha_gsh_start};
   fill_assets(atlas, light);
   for (unsigned i = 0; i < ShaderCount; ++i)
      if (horizon_gx2_terrain_create(&f->owner[i], gfd[i], AtlasW, AtlasH, atlas, sizeof(atlas)) ||
          horizon_gx2_terrain_set_sprite_grid(f->owner[i], Columns, Cell) ||
          horizon_gx2_terrain_lightmap(f->owner[i], light)) return false;
   static const uint32_t sorted_indices[18] = {0,1,2, 2,3,0, 4,5,6, 6,7,4, 8,9,10, 10,11,8};
   for (unsigned i = 0; i < FormatCount; ++i) {
      struct SectionInput input = {.format = (enum Format)i};
      uint32_t bytes;
      if (horizon_gx2_terrain_section_create(&f->solid[i], SolidQuads * 4,
                                             fill_vertices, &input, &bytes)) return false;
      input.translucent = true;
      if (horizon_gx2_terrain_section_create_sorted(&f->translucent[i], TranslucentQuads * 4,
                                                    fill_vertices, &input, &bytes) ||
          horizon_gx2_terrain_section_update_indices(f->translucent[i], sorted_indices, 18)) return false;
   }
   f->stage.storage = MEMAllocFromDefaultHeapEx(STAGE_CAP, STAGE_ALIGN);
   f->rgba = MEMAllocFromDefaultHeap(RGBA_CAP);
   return f->stage.storage && f->rgba && clear_init(&f->clear);
}

static bool render(Fixture *f, enum Arm arm, bool mixed)
{
   static const enum Shader shaders[ArmCount] = {
      STOCK_SHADER, CENTERED_SHADER, COMBINED_SHADER, COMBINED_SHADER, BAD_ALPHA_SHADER,
   };
   static const enum Format formats[ArmCount] = {RAW, CENTERED, CENTERED, SWAPPED, CENTERED};
   HorizonGx2Terrain *terrain = f->owner[shaders[arm]];
   const GX2ColorBuffer *colors[2] = {WHBGfxGetTVColourBuffer(), WHBGfxGetDRCColourBuffer()};
   const _Alignas(16) float fog[4] = {.41f,.58f,.73f,1};
   const _Alignas(16) float ranges[4] = {0,1024,86.4f,96};
   bool ok = true;
   WHBGfxBeginRender();
   for (unsigned output = 0; output < 2; ++output) {
      if (output) WHBGfxBeginRenderDRC();
      else WHBGfxBeginRenderTV();
      const unsigned w = colors[output]->surface.width, h = colors[output]->surface.height;
      const _Alignas(16) float clip[16] = {
         output ? .86f : .96f,0,0,output ? .065f : -.025f,
         0,output ? .96f : .88f,0,output ? -.08f : .04f, 0,0,1,0, 0,0,0,1,
      };
      const _Alignas(16) float origin[4] = {output ? 91.0f : 86.8f, 3, .25f, 0};
      draw_clear(&f->clear, terrain, w, h);
      ok &= horizon_gx2_terrain_bind(terrain, w,h,0,0,w,h,fog,ranges,false) == 0 &&
            horizon_gx2_terrain_draw_bound(terrain, f->solid[formats[arm]], clip, origin) == 0;
      if (mixed)
         ok &= horizon_gx2_terrain_bind(terrain, w,h,0,0,w,h,fog,ranges,true) == 0 &&
               horizon_gx2_terrain_draw_bound(terrain, f->translucent[formats[arm]], clip, origin) == 0;
      if (output) WHBGfxFinishRenderDRC();
      else WHBGfxFinishRenderTV();
   }
   const OSTime before = GX2GetLastSubmittedTimeStamp();
   WHBGfxFinishRender();
   const OSTime submitted = GX2GetLastSubmittedTimeStamp(), retired = GX2GetRetiredTimeStamp();
   const bool pair_retired = submitted > before && retired >= submitted;
   if (!ok || !pair_retired) {
      OSReport("GX2 light packing render error mixed=%u arm=%u submitted=%u retired=%u\n",
               mixed, arm, submitted > before, retired >= submitted);
      return false;
   }
   for (unsigned i = 0; i < 2; ++i) {
      const unsigned w = colors[i]->surface.width, h = colors[i]->surface.height;
      if (stage_read(&f->stage, colors[i], f->rgba, RGBA_CAP) != READ_OK) return false;
      const size_t pixels = (size_t)w * h;
      if (!f->reference[i]) f->reference[i] = MEMAllocFromDefaultHeap(pixels * 4);
      if (!f->reference[i]) return false;
      unsigned coverage = 0, mismatch = 0, alpha_mismatch = 0, coverage_mismatch = 0;
      uint32_t hash = 2166136261u;
      for (size_t p = 0; p < pixels * 4; p += 4) {
         const uint8_t *actual = f->rgba + p, *expected = f->reference[i] + p;
         const bool visible = actual[0] || actual[1] || actual[2];
         coverage += visible;
         for (unsigned c = 0; c < 4; ++c) hash = (hash ^ actual[c]) * 16777619u;
         if (arm != STOCK) {
            mismatch += memcmp(actual, expected, 4) != 0;
            alpha_mismatch += actual[3] != expected[3];
            coverage_mismatch += visible != (expected[0] || expected[1] || expected[2]);
         }
      }
      if (arm == STOCK) {
         memcpy(f->reference[i], f->rgba, pixels * 4);
         f->coverage[i] = coverage;
      }
      const bool pass = coverage > pixels / 12 && (arm < WRONG_LIGHT ?
         mismatch == 0 && coverage == f->coverage[i] :
         mismatch > pixels / 48 && (arm == WRONG_LIGHT ?
            alpha_mismatch == 0 && coverage_mismatch == 0 : coverage_mismatch > pixels / 96));
      OSReport("GX2 light packing mixed=%u arm=%u out=%u hash=%08x covered=%u diff=%u alpha=%u mask=%u pass=%u\n",
               mixed, arm, i, hash, coverage, mismatch, alpha_mismatch, coverage_mismatch, pass);
      ok &= pass;
   }
   return ok;
}

int main(void)
{
   static Fixture f;
   WHBProcInit();
   const bool gfx = WHBGfxInit();
   bool ok = gfx && init(&f);
   if (!ok) OSReport("GX2 light packing initialization FAIL\n");
   if (ok)
      for (unsigned mixed = 0; mixed < 2; ++mixed) {
         if (!render(&f, STOCK, mixed != 0)) {
            ok = false;
            break;
         }
         for (enum Arm arm = CENTERED_ONLY; arm < ArmCount; ++arm)
            ok &= render(&f, arm, mixed != 0);
      }
   if (gfx) {
      retire_for_teardown(&f.stage, "GX2 light packing");
      for (unsigned i = 0; i < FormatCount; ++i) {
         horizon_gx2_terrain_section_destroy_after_wait(f.translucent[i]);
         horizon_gx2_terrain_section_destroy_after_wait(f.solid[i]);
      }
      for (unsigned i = 0; i < ShaderCount; ++i) horizon_gx2_terrain_destroy(f.owner[i]);
      if (f.clear.vertices) MEMFreeToDefaultHeap(f.clear.vertices);
      if (f.clear.shader.vertexShader) WHBGfxFreeShaderGroup(&f.clear.shader);
      for (unsigned i = 0; i < 2; ++i)
         if (f.reference[i]) MEMFreeToDefaultHeap(f.reference[i]);
      if (f.rgba) MEMFreeToDefaultHeap(f.rgba);
      if (f.stage.storage) MEMFreeToDefaultHeap(f.stage.storage);
      WHBGfxShutdown();
   }
   WHBProcShutdown();
   OSReport("GX2 light packing fixture result=%s\n", ok ? "PASS" : "FAIL");
   return !ok;
}
