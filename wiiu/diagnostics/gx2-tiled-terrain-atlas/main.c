/* Standalone diagnostic. Include the exact hosted terrain path with the
 * scratch-only texture factory so both arms use its normal shader and binds. */
#include "terrain.c"
#include "retire.h"
#include <gx2/clear.h>
#include <proc_ui/procui.h>
#include <whb/proc.h>

extern const uint8_t _binary_stock_gsh_start[];
extern const uint8_t _binary_test_color_gsh_start[];

enum { QUADS = 16, OUTPUTS = 2, PHASES = 2, PADDING = 0xa9 };
static const unsigned layer[QUADS] = {
   0, 1, 2, 31, 5 + 19 * COLUMNS, 4 + 19 * COLUMNS, 6 + 19 * COLUMNS, 5 + 20 * COLUMNS,
   1270, 1300, 1301, 1269, 30, 61, 255, 256,
};
static const unsigned patches[] = { 0, 1, 5 + 19 * COLUMNS, 1301 };
typedef struct { uint8_t rgba[4]; } Pixel;
typedef struct { unsigned x, y; } Cell;
typedef struct { float left, right, bottom, top; } Bounds;
typedef struct { float scale_x, scale_y, offset_x, offset_y; } View;
typedef struct { uint8_t *rgba; unsigned width, height; } Sample;
typedef struct { OSTime atlas_reader; OSTime scan_tail; } Receipt;
typedef enum { MutationReady, MutationForeground, MutationReceipt, MutationTail, MutationPending } Mutation;
static const View views[OUTPUTS] = {{1, 1, 0, 0}, {0.90f, 0.88f, 0.045f, -0.04f}};
static Pixel cell_pixels[CELL * CELL];

typedef struct {
   WHBGfxShaderGroup group;
   float (*positions)[4];
   uint32_t clip[4];
   uint32_t color;
} DrawClear;

static bool draw_clear_init(DrawClear *clear)
{
   static const char *const clip[] = {"uClipX", "uClipY", "uClipZ", "uClipW"};
   static const float quad[4][4] = {
      {-1,-1,.95f,0}, {-1,1,.95f,0}, {1,1,.95f,0}, {1,-1,.95f,0},
   };
   if (!WHBGfxLoadGFDShaderGroup(&clear->group, 0, _binary_test_color_gsh_start) ||
       !WHBGfxInitShaderAttribute(&clear->group, "aPosition", 0, 0, GX2_ATTRIB_FORMAT_FLOAT_32_32_32) ||
       !WHBGfxInitFetchShader(&clear->group)) return false;
   for (unsigned i = 0; i < 4; ++i) {
      const GX2UniformVar *uniform = GX2GetVertexUniformVar(clear->group.vertexShader, clip[i]);
      if (!uniform) return false;
      clear->clip[i] = uniform->offset;
   }
   const GX2UniformVar *color = GX2GetPixelUniformVar(clear->group.pixelShader, "uColor");
   if (!color) return false;
   clear->color = color->offset;
   clear->positions = MEMAllocFromDefaultHeapEx(sizeof(quad), 64);
   if (!clear->positions) return false;
   memcpy(clear->positions, quad, sizeof(quad));
   DCFlushRange(clear->positions, sizeof(quad));
   return true;
}

/* Stamp a known old color after the ordinary clear. The control omits only
 * the black draw so that a working hardware clear cannot fake its result. */
static void draw_clear_output(const DrawClear *clear, HorizonGx2Terrain *terrain,
                              unsigned w, unsigned h, bool omit_black)
{
   static const _Alignas(16) float identity[16] = {
      1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1,
   };
   static const _Alignas(16) float poison[4] = {1,0,1,1};
   static const _Alignas(16) float black[4] = {0,0,0,1};
   GX2SetViewport(0,0,w,h,0,1);
   GX2SetScissor(0,0,w,h);
   GX2SetRasterizerClipControlEx(TRUE,TRUE,TRUE);
   GX2SetTargetChannelMasks(GX2_CHANNEL_MASK_RGBA,0,0,0,0,0,0,0);
   GX2SetShaderMode(GX2_SHADER_MODE_UNIFORM_REGISTER);
   GX2SetFetchShader(&clear->group.fetchShader);
   GX2SetVertexShader(clear->group.vertexShader);
   GX2SetPixelShader(clear->group.pixelShader);
   for (unsigned i = 0; i < 4; ++i)
      GX2SetVertexUniformReg(clear->clip[i],4,identity + 4*i);
   GX2SetAttribBuffer(0,4*sizeof(*clear->positions),sizeof(*clear->positions),clear->positions);
   GX2SetAlphaTest(FALSE,GX2_COMPARE_FUNC_ALWAYS,0);
   GX2SetCullOnlyControl(GX2_FRONT_FACE_CCW,FALSE,FALSE);
   GX2SetDepthOnlyControl(TRUE,TRUE,GX2_COMPARE_FUNC_ALWAYS);
   GX2SetColorControl(GX2_LOGIC_OP_COPY,0,FALSE,TRUE);
   GX2SetPixelUniformReg(clear->color,4,poison);
   horizon_gx2_terrain_draw_quads(terrain,4);
   if (!omit_black) {
      GX2SetPixelUniformReg(clear->color,4,black);
      horizon_gx2_terrain_draw_quads(terrain,4);
   }
}

static bool clear_sentinel(const uint8_t *rgba, unsigned w, unsigned h, unsigned output,
                           unsigned phase, bool wrong, bool omit_black)
{
   /* The projected x=0 gutter separates quad columns 1 and 2 on both arms. */
   const unsigned col = (unsigned)((0.5f + 0.5f * views[output].offset_x) * w);
   const Pixel expected = {{omit_black ? 255 : 0, 0, omit_black ? 255 : 0, 255}};
   unsigned mask = 0;
   const uint8_t *first = NULL;
   for (unsigned i = 1; i <= 3; ++i) {
      const uint8_t *pixel = rgba + ((size_t)(h*i/4) * w + col) * sizeof(Pixel);
      if (!first) first = pixel;
      if (memcmp(pixel, &expected, sizeof(expected))) mask |= 1u << (i-1);
   }
   OSReport("GX2 tile atlas clear out=%u p=%u wrong=%u omit=%u rgba=%u/%u/%u/%u mask=%x %s\n",
            output,phase,wrong,omit_black,first[0],first[1],first[2],first[3],mask,mask ? "FAIL" : "PASS");
   return mask == 0;
}

static Cell cell_coords(unsigned which)
{
   return (Cell){ (which % COLUMNS) * CELL, (which / COLUMNS) * CELL };
}

static Pixel texture_pixel(unsigned which, unsigned x, unsigned y, bool animated)
{
   /* The stored CELL has exact duplicated edges around its 64x64 interior. */
   x = x == 0 ? 0 : x == SPRITE + 1 ? SPRITE - 1 : x - 1;
   y = y == 0 ? 0 : y == SPRITE + 1 ? SPRITE - 1 : y - 1;
   const unsigned a = animated ? 1 : 0;
   return (Pixel){{ 32 + (which * 13 + x * 7 + y * 3 + a * 53) % 211,
                   26 + (which * 31 + x * 3 + y * 9 + a * 59) % 201,
                   30 + (which * 43 + x * 9 + y * 2 + a * 61) % 205,
                   ((x ^ y ^ which ^ (a * 5)) & 1) ? 255 : 127 }};
}

static void fill_cell(unsigned which, bool animated)
{
   for (unsigned y = 0; y < CELL; ++y)
      for (unsigned x = 0; x < CELL; ++x)
         cell_pixels[y * CELL + x] = texture_pixel(which, x, y, animated);
}

static bool changes(unsigned which)
{
   for (size_t i = 0; i < sizeof(patches) / sizeof(*patches); ++i)
      if (patches[i] == which) return true;
   return false;
}

static size_t tiled_index(unsigned x, unsigned y)
{
   /* RGBA8 color microtiles store paired rows of four adjacent pixels. */
   const unsigned micro = (y & 6) * 8 + (x & 4) * 2 + (y & 1) * 4 + (x & 3);
   return ((y / 8) * (PITCH / 8) + (x / 8)) * 64 + micro;
}

static void copy_cell(Pixel *atlas, unsigned which, bool tiled, bool flush)
{
   const Cell c = cell_coords(which);
   for (unsigned y = 0; y < CELL; ++y) {
      if (!tiled) {
         memcpy(atlas + (c.y + y) * PITCH + c.x, cell_pixels + y * CELL,
                CELL * sizeof(Pixel));
         continue;
      }
      for (unsigned x = 0; x < CELL;) {
         unsigned count = 4 - ((c.x + x) & 3);
         if (count > CELL - x) count = CELL - x;
         memcpy(atlas + tiled_index(c.x + x, c.y + y),
                cell_pixels + y * CELL + x, count * sizeof(Pixel));
         x += count;
      }
   }
   if (tiled && flush) {
      const unsigned from_x = c.x / 8, to_x = (c.x + CELL + 7) / 8;
      const unsigned from_y = c.y / 8, to_y = (c.y + CELL + 7) / 8;
      for (unsigned y = from_y; y < to_y; ++y)
         DCStoreRangeNoSync(atlas + (y * (PITCH / 8) + from_x) * 64,
                            (to_x - from_x) * 64 * sizeof(Pixel));
   }
}

static int initial_pixels(void *context, uint8_t *storage, uint32_t pitch)
{
   if (context || !storage || pitch != PITCH) return -1;
   for (unsigned which = 0; which < COLUMNS * ROWS; ++which) {
      fill_cell(which, false);
      copy_cell((Pixel *)storage, which, fixture_atlas_layout == AtlasTile1D, false);
   }
   return 0;
}

static bool check_storage(const HorizonGx2Terrain *t, unsigned phase)
{
   if (!t || !t->atlas_storage) return false;
   const GX2Surface *surface = &t->texture.surface;
   const bool tiled = surface->tileMode == GX2_TILE_MODE_TILED_1D_THIN1;
   const unsigned stored_height = tiled ? TILED_HEIGHT : HEIGHT;
   const Pixel *atlas = (const Pixel *)t->atlas_storage;
   for (unsigned y = 0; y < stored_height; ++y) {
      for (unsigned x = 0; x < PITCH; ++x) {
         const Pixel actual = atlas[tiled ? tiled_index(x, y) : y * PITCH + x];
         Pixel expected = {{PADDING, PADDING, PADDING, PADDING}};
         if (x < WIDTH && y < HEIGHT) {
            const unsigned which = y / CELL * COLUMNS + x / CELL;
            expected = texture_pixel(which, x % CELL, y % CELL, phase && changes(which));
         }
         if (memcmp(&actual, &expected, sizeof(Pixel))) {
            OSReport("GX2 tile atlas storage mismatch layout=%u phase=%u x=%u y=%u\n",
                      (unsigned)tiled, phase, x, y);
            return false;
         }
      }
   }
   OSReport("GX2 tile atlas storage layout=%u phase=%u visible_and_padding=%u PASS\n",
             (unsigned)tiled, phase, stored_height * PITCH);
   return true;
}

static Mutation mutation_status(Receipt prior, OSTime submitted, OSTime retired, bool foreground)
{
   if (!foreground) return MutationForeground;
   if (prior.atlas_reader <= 0 || prior.scan_tail < prior.atlas_reader) return MutationReceipt;
   if (submitted != prior.scan_tail) return MutationTail;
   if (retired < prior.atlas_reader) return MutationPending;
   return MutationReady;
}

static const char *mutation_reason(Mutation status)
{
   switch (status) {
      case MutationReady: return "ready";
      case MutationForeground: return "foreground-lost";
      case MutationReceipt: return "invalid-reader-receipt";
      case MutationTail: return "untracked-submission";
      case MutationPending: return "atlas-reader-pending";
   }
   return "unknown";
}

static int patch_linear_pixels(void *context, uint8_t *storage, uint32_t pitch)
{
   if (context || !storage || pitch != PITCH) return -1;
   for (size_t i = 0; i < sizeof(patches) / sizeof(*patches); ++i) {
      fill_cell(patches[i], true);
      copy_cell((Pixel *)storage, patches[i], false, false);
   }
   return 0;
}

static bool animate_after(HorizonGx2Terrain *t, Receipt prior, bool report)
{
   const OSTime submitted = GX2GetLastSubmittedTimeStamp();
   const OSTime retired = GX2GetRetiredTimeStamp();
   const Mutation status = mutation_status(prior, submitted, retired, ProcUIInForeground());
   if (status != MutationReady) {
      if (report)
         OSReport("GX2 tile atlas animation refused reason=%s atlas_reader=%lld scan_tail=%lld submitted=%lld retired=%lld\n",
                  mutation_reason(status), (long long)prior.atlas_reader, (long long)prior.scan_tail,
                  (long long)submitted, (long long)retired);
      return false;
   }
   if (t->texture.surface.tileMode == GX2_TILE_MODE_LINEAR_ALIGNED) {
      uint32_t rects[sizeof(patches) / sizeof(*patches)][4];
      for (size_t i = 0; i < sizeof(patches) / sizeof(*patches); ++i) {
         const Cell c = cell_coords(patches[i]);
         rects[i][0] = c.x;
         rects[i][1] = c.y;
         rects[i][2] = rects[i][3] = CELL;
      }
      return !horizon_gx2_terrain_patch_atlas(t, patch_linear_pixels, NULL, rects,
                                             sizeof(patches) / sizeof(*patches));
   }
   const GX2Surface *surface = &t->texture.surface;
   if (surface->tileMode != GX2_TILE_MODE_TILED_1D_THIN1 ||
       surface->image != t->atlas_storage || surface->pitch != PITCH ||
       surface->imageSize != TILED_BYTES) return false;
   for (size_t i = 0; i < sizeof(patches) / sizeof(*patches); ++i) {
      fill_cell(patches[i], true);
      copy_cell((Pixel *)t->atlas_storage, patches[i], true, true);
   }
   OSMemoryBarrier();
   GX2Invalidate(GX2_INVALIDATE_MODE_TEXTURE, t->atlas_storage, TILED_BYTES);
   OSReport("GX2 tile atlas animation dirty_spans=36 cache_bytes=82944\n");
   return true;
}

static Bounds quad_bounds(unsigned q)
{
   const float left = -1.0f + (q % 4) * 0.5f + 0.010f;
   const float bottom = -1.0f + (q / 4) * 0.5f + 0.010f;
   return (Bounds){ left, left + 0.480f, bottom, bottom + 0.480f };
}

static int fill_geometry(void *context, HorizonTerrainVertex *vertices, uint32_t count)
{
   if (!vertices || count != QUADS * 4 || !context) return -1;
   const bool wrong = *(const bool *)context;
   for (unsigned q = 0; q < QUADS; ++q) {
      const Bounds b = quad_bounds(q);
      const unsigned tex = wrong ? (layer[q] + 3) % (COLUMNS * ROWS) : layer[q];
      for (unsigned i = 0; i < 4; ++i) {
         const bool right = i == 1 || i == 2;
         const bool top = i > 1;
         vertices[q * 4 + i] = (HorizonTerrainVertex) {
            .position = { right ? b.right : b.left, top ? b.top : b.bottom, 0.25f, 0.0f },
            .uv = { (float)((right ? 32768u : 0) | ((tex & 255u) << 16)),
                    (float)((top ? 0 : 32768u) | ((tex >> 8) << 16)) },
            .color_cutout = { 255, 255, 255, (q & 1) ? 255 : 0 },
         };
      }
   }
   return 0;
}

/* The last readback retired after both atlas draws. The remaining WHB
 * finish commands only read color and scan targets. Track their submission
 * to detect unrelated work before mutation, without claiming scan retirement. */
static bool frame_receipt(const Stage *stage, const OSTime copy[OUTPUTS], Receipt *receipt)
{
   const OSTime submitted = GX2GetLastSubmittedTimeStamp();
   const OSTime retired = GX2GetRetiredTimeStamp();
   const char *reason = "ready";
   if (!ProcUIInForeground()) reason = "foreground-lost";
   else if (stage->copy_pending) reason = "readback-pending";
   else if (copy[0] <= 0 || copy[1] <= copy[0]) reason = "missing-ordered-readbacks";
   else if (submitted < copy[1]) reason = "submission-precedes-reader";
   else if (retired < copy[1]) reason = "atlas-reader-pending";
   else *receipt = (Receipt){copy[1], submitted};
   OSReport("GX2 tile atlas prior reason=%s tv_reader=%lld pad_reader=%lld scan_tail=%lld retired=%lld\n",
             reason, (long long)copy[0], (long long)copy[1], (long long)submitted, (long long)retired);
   return receipt->atlas_reader > 0;
}

static bool expected_output(Sample *expected, const uint8_t *rgba, unsigned w, unsigned h,
                            unsigned output, unsigned phase, bool reference, bool wrong)
{
   const size_t pixels = (size_t)w * h;
   unsigned drawn = 0, cutout = 0, changed = 0;
   if (!reference && (!expected->rgba || expected->width != w || expected->height != h)) return false;
   for (size_t i = 0; i < pixels; ++i) {
      const uint8_t *p = rgba + i * 4;
      drawn += p[0] || p[1] || p[2];
      cutout += p[3] == 127;
      if (!reference) changed += memcmp(p, expected->rgba + i * 4, 4) != 0;
   }
   const bool ok = drawn > pixels / 2 && cutout > pixels / 10 &&
                   (wrong ? changed > pixels / 2 : changed == 0);
   if (reference && ok) {
      expected->rgba = malloc(pixels * 4);
      if (!expected->rgba) return false;
      expected->width = w;
      expected->height = h;
      memcpy(expected->rgba, rgba, pixels * 4);
   }
   OSReport("GX2 tile atlas sample output=%u layout=%u phase=%u wrong=%u size=%ux%u drawn=%u alpha127=%u changed=%u %s\n",
             output, !reference, phase, (unsigned)wrong, w, h, drawn, cutout, changed, ok ? "PASS" : "FAIL");
   return ok;
}

static bool render(Stage *stage, uint8_t *rgba, HorizonGx2Terrain *terrain,
                   HorizonGx2TerrainSection *geometry, Sample expected[OUTPUTS],
                   unsigned phase, bool reference, bool wrong, const DrawClear *clear,
                   bool omit_black, Receipt *receipt)
{
   *receipt = (Receipt){0};
   if (!ProcUIInForeground()) {
      OSReport("GX2 tile atlas render refused: foreground-lost\n");
      return false;
   }
   const float fog[4] = {0,0,0,1}, fog_range[4] = {10,100,10,100};
   const float origin[4] = {0,0,0,0};
   bool ok = true;
   OSTime copy[OUTPUTS] = {0};
   WHBGfxBeginRender();
   for (unsigned i = 0; i < OUTPUTS; ++i) {
      if (!i) WHBGfxBeginRenderTV();
      else WHBGfxBeginRenderDRC();
      GX2ColorBuffer *target = i ? WHBGfxGetDRCColourBuffer() : WHBGfxGetTVColourBuffer();
      GX2DepthBuffer *depth = i ? WHBGfxGetDRCDepthBuffer() : WHBGfxGetTVDepthBuffer();
      const unsigned w = target->surface.width, h = target->surface.height;
      const View view = views[i];
      const float clip[16] = {view.scale_x,0,0,view.offset_x, 0,view.scale_y,0,view.offset_y,
                              0,0,1,0, 0,0,0,1};
      GX2ClearBuffersEx(target, depth, 0,0,0,1, 1,0, GX2_CLEAR_FLAGS_DEPTH);
      GX2SetContextState(i ? WHBGfxGetDRCContextState() : WHBGfxGetTVContextState());
      GX2SetColorBuffer(target, GX2_RENDER_TARGET_0);
      GX2SetDepthBuffer(depth);
      draw_clear_output(clear,terrain,w,h,omit_black);
      if (horizon_gx2_terrain_draw(terrain, geometry, w,h, 0,0,w,h, clip,origin, fog,fog_range)) {
         ok = false;
      } else {
         const enum ReadStatus read = stage_read_with_receipt(stage, target, rgba, RGBA_CAP, &copy[i]);
         if (read != READ_OK) {
            OSReport("GX2 tile atlas read failure output=%u status=%u\n", i, read);
            ok = false;
         } else {
            ok &= clear_sentinel(rgba,w,h,i,phase,wrong,omit_black);
            if (!omit_black)
               ok &= expected_output(&expected[i], rgba, w,h, i, phase, reference,wrong);
         }
      }
      if (!i) WHBGfxFinishRenderTV();
      else WHBGfxFinishRenderDRC();
   }
   WHBGfxFinishRender();
   return frame_receipt(stage, copy, receipt) && ok;
}

typedef struct { float x0, x1, y0, y1; } Rect;
static bool patch_coverage(const Sample pair[PHASES][OUTPUTS], unsigned output, bool top_first,
                           unsigned *total, unsigned *outside)
{
   const Sample *initial = &pair[0][output], *animated = &pair[1][output];
   if (!initial->rgba || !animated->rgba || initial->width != animated->width ||
       initial->height != animated->height) return false;
   const unsigned w = initial->width, h = initial->height;
   const View view = views[output];
   Rect rects[QUADS];
   unsigned affected = 0;
   for (unsigned q = 0; q < QUADS; ++q) {
      if (!changes(layer[q])) continue;
      const Bounds b = quad_bounds(q);
      const float left = 0.5f * (1 + b.left * view.scale_x + view.offset_x);
      const float right = 0.5f * (1 + b.right * view.scale_x + view.offset_x);
      const float bottom = 0.5f * (1 + b.bottom * view.scale_y + view.offset_y);
      const float top = 0.5f * (1 + b.top * view.scale_y + view.offset_y);
      rects[affected++] = (Rect){ w * left - 2, w * right + 2,
         h * (top_first ? 1 - top : bottom) - 2,
         h * (top_first ? 1 - bottom : top) + 2 };
   }
   unsigned changed[QUADS] = {0};
   *total = *outside = 0;
   for (unsigned y = 0; y < h; ++y) for (unsigned x = 0; x < w; ++x) {
      const size_t p = ((size_t)y * w + x) * 4;
      if (!memcmp(initial->rgba + p, animated->rgba + p, 4)) continue;
      ++*total;
      unsigned box = 0;
      for (; box < affected; ++box) {
         const Rect r = rects[box];
         if (x >= r.x0 && x <= r.x1 && y >= r.y0 && y <= r.y1) {
            ++changed[box];
            break;
         }
      }
      *outside += box == affected;
   }
   if (*outside || affected != sizeof(patches) / sizeof(*patches)) return false;
   for (unsigned i = 0; i < affected; ++i) if (changed[i] < w * h / 100) return false;
   return true;
}

static bool check_linear_animation(const Sample pair[PHASES][OUTPUTS])
{
   bool result = true;
   for (unsigned i = 0; i < OUTPUTS; ++i) {
      unsigned changed = 0, outside = 0;
      bool ok = patch_coverage(pair, i, true, &changed, &outside);
      if (!ok) ok = patch_coverage(pair, i, false, &changed, &outside);
      OSReport("GX2 tile atlas reference animation output=%u changed=%u neighbour_delta=%u %s\n",
                i, changed, outside, ok ? "PASS" : "FAIL");
      result &= ok;
   }
   return result;
}

typedef enum { ArmFail, ArmPass, ArmSkipped } ArmResult;

static ArmResult run_arm(AtlasLayout layout, Stage *stage, uint8_t *rgba,
                         Sample pair[PHASES][OUTPUTS], const DrawClear *clear)
{
   if (!ProcUIInForeground()) return ArmFail;
   fixture_atlas_layout = layout;
   const bool reference = layout == AtlasLinear;
   HorizonGx2Terrain *terrain = NULL;
   const int init = horizon_gx2_terrain_create_mapped(&terrain, _binary_stock_gsh_start,
                                                     WIDTH, HEIGHT, initial_pixels, NULL);
   if (init != 0) {
      OSReport("GX2 tile atlas admission layout=%u code=%d\n", layout, init);
      return reference ? ArmFail : ArmSkipped;
   }
   HorizonGx2TerrainSection *geometry = NULL, *wrong = NULL;
   uint32_t bytes = 0;
   static uint8_t light[16 * 16 * 4];
   memset(light, 255, sizeof(light));
   bool wrong_cell = false;
   bool ok = !horizon_gx2_terrain_set_sprite_grid(terrain, COLUMNS, CELL) &&
             !horizon_gx2_terrain_lightmap(terrain, light) &&
             !horizon_gx2_terrain_section_create(&geometry, QUADS * 4, fill_geometry, &wrong_cell, &bytes) &&
             check_storage(terrain, 0);
   Receipt prior = {0};
   if (ok) ok = render(stage, rgba, terrain, geometry, pair[0], 0, reference, false, clear, false, &prior);
   if (ok) {
      const OSTime reader = prior.atlas_reader;
      const OSTime tail = prior.scan_tail;
      const bool controls =
         !animate_after(terrain, (Receipt){0}, false) &&
         !animate_after(terrain, (Receipt){reader, tail - 1}, false) &&
         mutation_status(prior, tail, reader - 1, true) == MutationPending &&
         mutation_status(prior, tail, reader, false) == MutationForeground &&
         mutation_status((Receipt){reader, reader + 1}, reader + 1, reader, true) == MutationReady;
      OSReport("GX2 tile atlas ownership controls layout=%u %s\n", layout,
                controls ? "PASS" : "FAIL");
      ok = controls && check_storage(terrain, 0) &&
           animate_after(terrain, prior, true) && check_storage(terrain, 1);
   }
   if (ok) ok = render(stage, rgba, terrain, geometry, pair[1], 1, reference, false, clear, false, &prior);
   if (ok && reference) ok = check_linear_animation(pair);
   if (ok && !reference) {
      wrong_cell = true;
      ok = !horizon_gx2_terrain_section_create(&wrong, QUADS * 4, fill_geometry, &wrong_cell, &bytes)
         && render(stage, rgba, terrain, wrong, pair[1], 1, false, true, clear, false, &prior);
   }
   if (ok && !reference)
      ok = render(stage, rgba, terrain, geometry, pair[1], 1, false, false, clear, true, &prior);

   // Even a control failure can leave a copy or draw outstanding. Keep all
   // texture, section and readback backing until the new teardown fence.
   retire_for_teardown(stage, "GX2 tile atlas owner");
   horizon_gx2_terrain_section_destroy_after_wait(wrong);
   horizon_gx2_terrain_section_destroy_after_wait(geometry);
   horizon_gx2_terrain_destroy(terrain);
   return ok ? ArmPass : ArmFail;
}

int main(void)
{
   WHBProcInit();
   const bool graphics = WHBGfxInit();
   Sample pair[PHASES][OUTPUTS] = {0};
   Stage stage = {0};
   DrawClear clear = {0};
   uint8_t *rgba = NULL;
   ArmResult result = ArmFail;
   if (graphics) {
      stage.storage = MEMAllocFromDefaultHeapEx(STAGE_CAP, STAGE_ALIGN);
      rgba = MEMAllocFromDefaultHeap(RGBA_CAP);
      if (stage.storage && rgba && draw_clear_init(&clear) &&
          run_arm(AtlasLinear, &stage, rgba, pair, &clear) == ArmPass)
         result = run_arm(AtlasTile1D, &stage, rgba, pair, &clear);

      if (clear.positions) MEMFreeToDefaultHeap(clear.positions);
      if (clear.group.vertexShader) WHBGfxFreeShaderGroup(&clear.group);
      if (rgba) MEMFreeToDefaultHeap(rgba);
      if (stage.storage) MEMFreeToDefaultHeap(stage.storage);
      WHBGfxShutdown();
   }
   for (unsigned p = 0; p < PHASES; ++p)
      for (unsigned i = 0; i < OUTPUTS; ++i)
         free(pair[p][i].rgba);
   WHBProcShutdown();
   OSReport("GX2 tile atlas diagnostic result=%s\n",
             result == ArmPass ? "PASS" : result == ArmSkipped ? "SKIP-linear-control-passed" : "FAIL");
   return result == ArmPass ? 0 : result == ArmSkipped ? 2 : 1;
}
