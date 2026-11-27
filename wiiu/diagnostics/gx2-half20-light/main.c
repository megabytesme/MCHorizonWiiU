#include "terrain.h"
#include "retire.h"
#include "exact-half.h"
#include <coreinit/memdefaultheap.h>
#include <gx2/draw.h>
#include <gx2/mem.h>
#include <gx2/registers.h>
#include <gx2/shaders.h>
#include <gx2/utils.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern const uint8_t _binary_stock_gsh_start[];
extern const uint8_t _binary_half_gsh_start[];
extern const uint8_t _binary_clear_gsh_start[];

enum { Columns = 16, Rows = 17, Cell = 8, AtlasW = Columns * Cell, AtlasH = Rows * Cell };
enum { Quads = 8, Vertices = 4 * Quads };

typedef struct {
   uint16_t position[3], light;
   float uv[2];
   uint8_t color_cutout[4];
} CompactVertex;
_Static_assert(sizeof(CompactVertex) == 20 && _Alignof(CompactVertex) == 4, "20-byte terrain ABI");
_Static_assert(offsetof(CompactVertex, light) == 6, "compact light at byte six");
_Static_assert(offsetof(CompactVertex, uv) == 8, "packed UV alignment");
_Static_assert(offsetof(CompactVertex, color_cutout) == 16, "compact RGBA");
_Static_assert(sizeof(HorizonTerrainHalfVertex) == 24, "qualified half24 ABI");

enum Fetch { Half24, Pair20, WrongLane, Scalar20, FetchCount };
enum Arm { Stock, Half, Compact, Lane, Swap, Clamp, ArmCount };
enum Payload { NormalLight, SwappedLight, WrongClamp, PayloadCount };

typedef struct {
   WHBGfxShaderGroup group;
   uint32_t clip[4], origin;
} Layout;

#include "clear.inc"

typedef struct {
   HorizonGx2Terrain *terrain;
   HorizonGx2TerrainSection *stock;
   Layout layouts[FetchCount];
   HorizonTerrainHalfVertex *half;
   CompactVertex *compact[PayloadCount];
   Stage stage;
   Clear clear;
   uint8_t *rgba, *reference[2];
   unsigned coverage[2];
} Fixture;

static uint32_t little_word(const uint8_t *data)
{
   return (uint32_t)data[0] | (uint32_t)data[1] << 8 |
          (uint32_t)data[2] << 16 | (uint32_t)data[3] << 24;
}

static bool check_fetch_packet(const GX2FetchShader *fetch, uint32_t semantic,
                               unsigned format, unsigned offset, unsigned endian, unsigned x)
{
   if (fetch->attribCount != 4 || !fetch->program || fetch->size < 5 * 16) return false;
   const uint8_t *program = fetch->program;
   unsigned found = 0;
   for (unsigned at = 16; at + 16 <= fetch->size; at += 16) {
      const uint32_t word0 = little_word(program + at);
      const uint32_t word1 = little_word(program + at + 4);
      const uint32_t word2 = little_word(program + at + 8);
      if ((word0 & 31) != 1 || (word1 & 255) != semantic) continue;
      const unsigned mask = word1 >> 9 & 0xfff;
      const bool valid = (word0 >> 8 & 255) == 0xa0 && !(word1 & (1u << 21)) &&
         (word1 >> 22 & 63) == format && (word1 >> 28 & 3) == 2 && !(word1 & (1u << 30)) &&
         (word2 & 0xffff) == offset && (word2 >> 16 & 3) == endian &&
         mask == (x | (4 << 3) | (4 << 6) | (5 << 9));
      OSReport("GX2 half20 packet sem=%u fmt=%u nfa=%u unsigned=%u off=%u swap=%u dstx=%u mask=%03x valid=%u\n",
               semantic, (word1 >> 22) & 63, (word1 >> 28) & 3, !(word1 & (1u << 30)),
               word2 & 0xffff, (word2 >> 16) & 3, mask & 7, mask, valid);
      if (!valid) return false;
      ++found;
   }
   return found == 1;
}

static GX2AttribStream *attribute(WHBGfxShaderGroup *group, const char *name)
{
   const GX2VertexShader *shader = group->vertexShader;
   for (unsigned a = 0; a < shader->attribVarCount; ++a) {
      const GX2AttribVar *var = &shader->attribVars[a];
      if (strcmp(var->name, name) != 0) continue;
      for (unsigned i = 0; i < group->numAttributes; ++i)
         if (group->attributes[i].location == var->location) return &group->attributes[i];
   }
   return NULL;
}

static bool layout_init(Layout *layout, enum Fetch kind)
{
   WHBGfxShaderGroup *group = &layout->group;
   const bool old = kind == Half24;
   const unsigned color = old ? offsetof(HorizonTerrainHalfVertex, color_cutout) :
                               offsetof(CompactVertex, color_cutout);
   const unsigned uv = old ? offsetof(HorizonTerrainHalfVertex, uv) : offsetof(CompactVertex, uv);
   const unsigned light = old ? offsetof(HorizonTerrainHalfVertex, light) :
      kind == Scalar20 ? offsetof(CompactVertex, light) : offsetof(CompactVertex, position) + 4;
   const GX2AttribFormat light_format = old ? GX2_ATTRIB_FORMAT_FLOAT_32 :
      (GX2_ATTRIB_FLAG_SCALED | (kind == Scalar20 ? GX2_ATTRIB_TYPE_16 : GX2_ATTRIB_TYPE_16_16));
   const GX2AttribFormat position_format = GX2_ATTRIB_FLAG_SCALED | GX2_ATTRIB_TYPE_16_16_16_16_FLOAT;
   if (!WHBGfxLoadGFDShaderGroup(group, 0, _binary_half_gsh_start) ||
       !WHBGfxInitShaderAttribute(group, "aPosition", 0, 0, position_format) ||
       !WHBGfxInitShaderAttribute(group, "aColor", 0, color, GX2_ATTRIB_FORMAT_UNORM_8_8_8_8) ||
       !WHBGfxInitShaderAttribute(group, "aUv", 0, uv, GX2_ATTRIB_FORMAT_FLOAT_32_32) ||
       !WHBGfxInitShaderAttribute(group, "aLight", 0, light, light_format)) return false;
   GX2AttribStream *pos = attribute(group, "aPosition");
   GX2AttribStream *illum = attribute(group, "aLight");
   if (!pos || !illum || group->numAttributes != 4) return false;
   pos->mask = GX2_COMP_MAP(GX2_SQ_SEL_R, GX2_SQ_SEL_G, GX2_SQ_SEL_B, GX2_SQ_SEL_1);
   pos->endianSwap = GX2_ENDIAN_SWAP_8_IN_16;
   const unsigned component = kind == Pair20 ? GX2_SQ_SEL_G : GX2_SQ_SEL_R;
   illum->mask = GX2_COMP_MAP(component, GX2_SQ_SEL_0, GX2_SQ_SEL_0, GX2_SQ_SEL_1);
   illum->endianSwap = old ? GX2_ENDIAN_SWAP_8_IN_32 : GX2_ENDIAN_SWAP_8_IN_16;
   OSReport("GX2 half20 setup kind=%u stride=%u pos=%03x pmask=%08x light=%03x lmask=%08x off=%u\n",
            kind, old ? 24u : 20u, pos->format, pos->mask, illum->format, illum->mask, illum->offset);
   if (!WHBGfxInitFetchShader(group) ||
       !check_fetch_packet(&group->fetchShader, illum->location,
           old ? 0x0e : kind == Scalar20 ? 0x05 : 0x0f, light, illum->endianSwap, component))
      return false;
   static const char *const names[] = {"uClipX", "uClipY", "uClipZ", "uClipW", "uSectionOrigin"};
   for (unsigned i = 0; i < 5; ++i) {
      const GX2UniformVar *var = GX2GetVertexUniformVar(group->vertexShader, names[i]);
      if (!var || var->offset != i * 4) return false;
      if (i < 4) layout->clip[i] = var->offset;
      else layout->origin = var->offset;
   }
   return true;
}

static HorizonTerrainVertex fixture_vertex(unsigned i)
{
   const unsigned q = i / 4, c = i % 4;
   const bool right = c == 1 || c == 2, top = c >= 2;
   static const uint16_t tiles[Quads] = {0, 15, 16, 63, 127, 255, 256, 271};
   static const uint16_t uv[4][2] = {{0,0}, {32768,1}, {65535,65535}, {1,32768}};
   static const uint16_t light[Vertices] = {
      0x00ff,0xff00,0xf1ef,0x1080, 0xff10,0x7811,0x23f0,0xefaa,
      0x7c00,0x7d01,0xfc01,0x8080, 0xffff,0x00f1,0xfe10,0xe1f4,
      0xff00,0xf800,0x60ac,0x10fc, 0x102f,0x4ef2,0x00cf,0x250f,
      0x6d1a,0x0af8,0xf610,0x7e40, 0xaaf9,0x2baa,0xfa03,0xf39e,
   };
   const int x = -59 + 29 * (int)(q % 4) + (right ? 26 : 0);
   const int y = -51 + 50 * (int)(q / 4) + (top ? 45 : 0);
   const int z = 30 + (int)q;
   const uint32_t u = ((tiles[q] & 255u) << 16) | uv[c][0];
   const uint32_t v = ((uint32_t)(tiles[q] >> 8) << 16) | uv[c][1];
   return (HorizonTerrainVertex) {
      .position = {x / 64.0f, y / 64.0f, z / 64.0f, (float)light[i]},
      .uv = {(float)u, (float)v},
      .color_cutout = {148 + 11 * q, 226 - 39 * c, 211 - 23 * q, (q & 1) ? 255 : 0},
   };
}

static int stock_fill(void *context, HorizonTerrainVertex *out, uint32_t n)
{
   (void)context;
   if (n != Vertices) return -1;
   for (unsigned i = 0; i < n; ++i) out[i] = fixture_vertex(i);
   return 0;
}

static uint16_t corrupt_light(uint16_t light, enum Payload mode)
{
   if (mode == SwappedLight) return (uint16_t)(light << 8 | light >> 8);
   if (mode == WrongClamp) {
      unsigned block = light >> 8, sky = light & 255;
      if (block > 240) block = 112;
      if (sky > 240) sky = 112;
      return (uint16_t)(block << 8 | sky);
   }
   return light;
}

static bool upload(Fixture *f)
{
   f->half = MEMAllocFromDefaultHeapEx(Vertices * sizeof(*f->half), 64);
   if (!f->half) return false;
   for (unsigned k = 0; k < PayloadCount; ++k) {
      f->compact[k] = MEMAllocFromDefaultHeapEx(Vertices * sizeof(*f->compact[k]), 64);
      if (!f->compact[k]) return false;
   }
   for (unsigned i = 0; i < Vertices; ++i) {
      const HorizonTerrainVertex src = fixture_vertex(i);
      for (unsigned axis = 0; axis < 3; ++axis)
         if (!exact_half_position(src.position[axis], &f->half[i].position[axis])) return false;
      f->half[i].position[3] = 0x3800;
      f->half[i].light = src.position[3];
      memcpy(f->half[i].uv, src.uv, sizeof(src.uv));
      memcpy(f->half[i].color_cutout, src.color_cutout, sizeof(src.color_cutout));
      for (unsigned k = 0; k < PayloadCount; ++k) {
         CompactVertex *v = &f->compact[k][i];
         memcpy(v->position, f->half[i].position, sizeof(v->position));
         v->light = corrupt_light((uint16_t)src.position[3], (enum Payload)k);
         memcpy(v->uv, src.uv, sizeof(src.uv));
         memcpy(v->color_cutout, src.color_cutout, sizeof(src.color_cutout));
      }
   }
   GX2Invalidate(GX2_INVALIDATE_MODE_CPU_ATTRIBUTE_BUFFER, f->half, Vertices * sizeof(*f->half));
   for (unsigned k = 0; k < PayloadCount; ++k)
      GX2Invalidate(GX2_INVALIDATE_MODE_CPU_ATTRIBUTE_BUFFER, f->compact[k],
                    Vertices * sizeof(*f->compact[k]));
   return true;
}

static bool prepare(Fixture *f)
{
   static uint8_t atlas[AtlasW * AtlasH * 4];
   for (unsigned y = 0; y < AtlasH; ++y)
      for (unsigned x = 0; x < AtlasW; ++x) {
         const unsigned tile = y / Cell * Columns + x / Cell;
         const unsigned p = (y * AtlasW + x) * 4;
         atlas[p] = 48 + (tile * 67 + x * 5) % 160;
         atlas[p+1] = 48 + (tile * 37 + y * 7) % 160;
         atlas[p+2] = 48 + (tile * 9 + x * 3 + y * 5) % 160;
         atlas[p+3] = ((x % Cell >= 3) != (y % Cell >= 3)) ? 48 : 223;
      }
   uint32_t bytes;
   if (horizon_gx2_terrain_create(&f->terrain, _binary_stock_gsh_start, AtlasW, AtlasH, atlas, sizeof(atlas)) ||
       horizon_gx2_terrain_set_sprite_grid(f->terrain, Columns, Cell) ||
       horizon_gx2_terrain_section_create(&f->stock, Vertices, stock_fill, NULL, &bytes)) return false;
   for (unsigned i = 0; i < FetchCount; ++i)
      if (!layout_init(&f->layouts[i], (enum Fetch)i)) return false;
   f->stage.storage = MEMAllocFromDefaultHeapEx(STAGE_CAP, STAGE_ALIGN);
   f->rgba = MEMAllocFromDefaultHeap(RGBA_CAP);
   return f->stage.storage && f->rgba && clear_init(&f->clear) && upload(f);
}

static bool lightmap(Fixture *f, bool flat)
{
   uint8_t lights[16 * 16 * 4];
   for (unsigned y = 0; y < 16; ++y)
      for (unsigned x = 0; x < 16; ++x) {
         const unsigned p = (y * 16 + x) * 4;
         lights[p] = flat ? 121 : 42 + 14 * x;
         lights[p+1] = flat ? 151 : 233 - 12 * y;
         lights[p+2] = flat ? 203 : 24 + 6 * (x + y);
         lights[p+3] = 255;
      }
   return horizon_gx2_terrain_lightmap(f->terrain, lights) == 0;
}

static void direct(Fixture *f, enum Arm arm, const float clip[16], const float origin[4])
{
#ifdef DRAW_SCALAR20
   const enum Fetch kind = arm == Half ? Half24 : arm == Lane ? WrongLane : Scalar20;
#else
   const enum Fetch kind = arm == Half ? Half24 : arm == Lane ? WrongLane : Pair20;
#endif
   const Layout *layout = &f->layouts[kind];
   const uint32_t stride = arm == Half ? sizeof(*f->half) : sizeof(*f->compact[0]);
   const enum Payload payload = arm == Swap ? SwappedLight : arm == Clamp ? WrongClamp : NormalLight;
   void *vertices = arm == Half ? (void *)f->half : f->compact[payload];
   GX2SetVertexShader(layout->group.vertexShader);
   GX2SetFetchShader(&layout->group.fetchShader);
   for (unsigned i = 0; i < 4; ++i)
      GX2SetVertexUniformReg(layout->clip[i], 4, clip + 4 * i);
   GX2SetVertexUniformReg(layout->origin, 4, origin);
   GX2SetAttribBuffer(0, Vertices * stride, stride, vertices);
   horizon_gx2_terrain_draw_quads(f->terrain, Vertices);
}

static bool render(Fixture *f, enum Arm arm, bool flat)
{
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
      draw_clear(&f->clear, f->terrain, w, h);
      const bool bound = horizon_gx2_terrain_bind(f->terrain, w,h,0,0,w,h,fog,ranges,false) == 0;
      ok &= bound;
      if (bound) {
         if (arm == Stock)
            ok &= horizon_gx2_terrain_draw_bound(f->terrain, f->stock, clip, origin) == 0;
         else
            direct(f, arm, clip, origin);
      }
      if (output) WHBGfxFinishRenderDRC();
      else WHBGfxFinishRenderTV();
   }
   const OSTime before = GX2GetLastSubmittedTimeStamp();
   WHBGfxFinishRender();
   const OSTime submitted = GX2GetLastSubmittedTimeStamp(), retired = GX2GetRetiredTimeStamp();
   if (!ok || submitted <= before || retired < submitted) return false;
   for (unsigned i = 0; i < 2; ++i) {
      if (stage_read(&f->stage, colors[i], f->rgba, RGBA_CAP) != READ_OK) return false;
      const size_t pixels = (size_t)colors[i]->surface.width * colors[i]->surface.height;
      if (!f->reference[i]) f->reference[i] = MEMAllocFromDefaultHeap(pixels * 4);
      if (!f->reference[i]) return false;
      unsigned covered = 0, diff = 0, alpha = 0, mask = 0;
      for (size_t p = 0; p < pixels * 4; p += 4) {
         const uint8_t *a = f->rgba + p, *b = f->reference[i] + p;
         const bool visible = a[0] || a[1] || a[2];
         covered += visible;
         if (arm != Stock) {
            diff += memcmp(a, b, 4) != 0;
            alpha += a[3] != b[3];
            mask += visible != (b[0] || b[1] || b[2]);
         }
      }
      if (arm == Stock) {
         memcpy(f->reference[i], f->rgba, pixels * 4);
         f->coverage[i] = covered;
      }
      const bool pass = covered > pixels / 12 && covered == f->coverage[i] && !alpha && !mask &&
         (flat || arm <= Compact ? diff == 0 : diff > pixels / 60);
      OSReport("GX2 half20 flat=%u arm=%u out=%u coverage=%u diff=%u alpha=%u mask=%u pass=%u\n",
               flat, arm, i, covered, diff, alpha, mask, pass);
      ok &= pass;
   }
   return ok;
}

int main(void)
{
   static Fixture f;
   WHBProcInit();
   const bool gfx = WHBGfxInit();
   bool ok = gfx && prepare(&f);
   if (!ok) OSReport("GX2 half20 initialization FAIL\n");
   if (ok)
      for (unsigned flat = 0; flat < 2; ++flat) {
         if (!lightmap(&f, flat != 0) || !render(&f, Stock, flat != 0)) {
            ok = false;
            break;
         }
         for (enum Arm arm = Half; arm < ArmCount; ++arm)
            ok &= render(&f, arm, flat != 0);
      }
   if (gfx) {
      retire_for_teardown(&f.stage, "GX2 half20");
      horizon_gx2_terrain_section_destroy_after_wait(f.stock);
      horizon_gx2_terrain_destroy(f.terrain);
      for (unsigned i = 0; i < FetchCount; ++i)
         if (f.layouts[i].group.vertexShader) WHBGfxFreeShaderGroup(&f.layouts[i].group);
      if (f.half) MEMFreeToDefaultHeap(f.half);
      for (unsigned i = 0; i < PayloadCount; ++i)
         if (f.compact[i]) MEMFreeToDefaultHeap(f.compact[i]);
      if (f.clear.vertices) MEMFreeToDefaultHeap(f.clear.vertices);
      if (f.clear.shader.vertexShader) WHBGfxFreeShaderGroup(&f.clear.shader);
      for (unsigned i = 0; i < 2; ++i)
         if (f.reference[i]) MEMFreeToDefaultHeap(f.reference[i]);
      if (f.rgba) MEMFreeToDefaultHeap(f.rgba);
      if (f.stage.storage) MEMFreeToDefaultHeap(f.stage.storage);
      WHBGfxShutdown();
   }
   WHBProcShutdown();
   OSReport("GX2 half20 fixture result=%s\n", ok ? "PASS" : "FAIL");
   return !ok;
}
