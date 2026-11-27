#include "terrain.h"
#include "world_scale.h"
#include "../gx2-explicit-readback/retire.h"
#include <coreinit/memdefaultheap.h>
#include <gx2/clear.h>
#include <gx2/context.h>
#include <gx2/displaylist.h>
#include <gx2/draw.h>
#include <gx2/registers.h>
#include <gx2/shaders.h>
#include <stdlib.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern const uint8_t _binary_terrain_gsh_start[];
extern const uint8_t _binary_solid_list_color_gsh_start[];

enum {
   DRAWS = 174, COMMAND_LIMIT = 256,
   LIST_CAP = 64 * 1024, LIST_ALIGN = 256, LIST_GUARD = 0x6b,
};

typedef struct {
   unsigned output, epoch, revision, width, height;
   unsigned world_width, world_height;
   HorizonGx2Terrain *terrain;
   HorizonGx2TerrainSection *section[DRAWS];
   _Alignas(16) float clip[16];
   float origin[4];
} View;

typedef struct {
   uint8_t *storage;
   View key;
   uint32_t bytes;
   bool unsubmitted;
   OSTime before_use, last_use;
} List;

enum Arm {
   STOCK, LIST_REPLAY, REJECT_AND_DRAW, LIST_AGAIN, CLOSED_RECORD_ABORT,
   RERECORD, WRONG_CAMERA, REVERSE_ORDER, WRONG_TAIL_DEPTH,
};

typedef struct {
   float position[4], padding[4];
} TailVertex;

typedef struct {
   WHBGfxShaderGroup shader;
   TailVertex *vertices;
   uint32_t clip[4], color;
} Tail;

static const _Alignas(16) float identity[16] = {
   1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1,
};

static void tail_quad(TailVertex out[4], float x0, float y0, float x1, float y1, float depth)
{
   out[0] = (TailVertex){{x0,y0,depth,0},{0}};
   out[1] = (TailVertex){{x0,y1,depth,0},{0}};
   out[2] = (TailVertex){{x1,y1,depth,0},{0}};
   out[3] = (TailVertex){{x1,y0,depth,0},{0}};
}

static bool tail_init(Tail *tail)
{
   WHBGfxShaderGroup *shader = &tail->shader;
   if (!WHBGfxLoadGFDShaderGroup(shader,0,_binary_solid_list_color_gsh_start) ||
       !WHBGfxInitShaderAttribute(shader,"aPosition",0,0,GX2_ATTRIB_FORMAT_FLOAT_32_32_32) ||
       !WHBGfxInitFetchShader(shader)) return false;
   static const char *names[] = {"uClipX","uClipY","uClipZ","uClipW"};
   for (unsigned i = 0; i < 4; ++i) {
      const GX2UniformVar *var = GX2GetVertexUniformVar(shader->vertexShader,names[i]);
      if (!var) return false;
      tail->clip[i] = var->offset;
   }
   const GX2UniformVar *var = GX2GetPixelUniformVar(shader->pixelShader,"uColor");
   if (!var) return false;
   tail->color = var->offset;
   tail->vertices = MEMAllocFromDefaultHeapEx(sizeof(TailVertex)*12,64);
   if (!tail->vertices) return false;
   // Drawn clear isolates repeated DRC frames. The next quads probe
   // blending and depth against the earlier opaque terrain.
   tail_quad(tail->vertices,-1,-1,1,1,.95f);
   tail_quad(tail->vertices+4,.565f,-.08f,.645f,.08f,.22f);
   tail_quad(tail->vertices+8,.42f,-.08f,.485f,.08f,.55f);
   DCFlushRange(tail->vertices,sizeof(TailVertex)*12);
   return true;
}

static void tail_draw(const Tail *tail, const View *view, unsigned quad, bool force_depth)
{
   static const _Alignas(16) float black[4] = {0,0,0,1};
   static const _Alignas(16) float purple[4] = {1,0,1,.5f};
   const bool clear = quad == 0;
   const float *clip = clear ? identity : view->clip;
   GX2SetShaderMode(GX2_SHADER_MODE_UNIFORM_REGISTER);
   GX2SetFetchShader(&tail->shader.fetchShader);
   GX2SetVertexShader(tail->shader.vertexShader);
   GX2SetPixelShader(tail->shader.pixelShader);
   for (unsigned i = 0; i < 4; ++i)
      GX2SetVertexUniformReg(tail->clip[i],4,clip+4*i);
   GX2SetPixelUniformReg(tail->color,4,clear ? black : purple);
   GX2SetAttribBuffer(0,4*sizeof(TailVertex),sizeof(TailVertex),tail->vertices+4*quad);
   GX2SetAlphaTest(FALSE,GX2_COMPARE_FUNC_ALWAYS,0);
   GX2SetCullOnlyControl(GX2_FRONT_FACE_CCW,FALSE,FALSE);
   GX2SetDepthOnlyControl(TRUE,TRUE,clear || force_depth ? GX2_COMPARE_FUNC_ALWAYS : GX2_COMPARE_FUNC_LEQUAL);
   GX2SetColorControl(GX2_LOGIC_OP_COPY,clear ? 0 : 1,FALSE,TRUE);
   GX2SetBlendControl(GX2_RENDER_TARGET_0,GX2_BLEND_MODE_SRC_ALPHA,GX2_BLEND_MODE_INV_SRC_ALPHA,
                      GX2_BLEND_COMBINE_MODE_ADD,TRUE,GX2_BLEND_MODE_ONE,
                      GX2_BLEND_MODE_INV_SRC_ALPHA,GX2_BLEND_COMBINE_MODE_ADD);
   horizon_gx2_terrain_draw_quads(view->terrain,4);
}

static bool tail_color(const uint8_t *pixel, unsigned output)
{
   return pixel[1] > 80 && pixel[1] < 180 && pixel[3] == 255 &&
      (output == 0 ? pixel[0] > 80 && pixel[0] < 180 && pixel[2] > 170
                   : pixel[0] > 170 && pixel[2] > 80 && pixel[2] < 180);
}


static uint8_t *commands(const List *list)
{
   return list->storage + LIST_ALIGN;
}

static bool same_view(const View *a, const View *b)
{
   return a->output == b->output && a->epoch == b->epoch &&
      a->revision == b->revision && a->width == b->width && a->height == b->height &&
      a->world_width == b->world_width && a->world_height == b->world_height &&
      a->terrain == b->terrain && !memcmp(a->section,b->section,sizeof(a->section)) &&
      !memcmp(a->clip,b->clip,sizeof(a->clip)) && !memcmp(a->origin,b->origin,sizeof(a->origin));
}

static bool fits(unsigned draws)
{
   return draws && draws <= DRAWS && draws <= LIST_CAP / COMMAND_LIMIT;
}

static bool can_record(const List *list, OSTime retired)
{
   return !list->unsubmitted && (!list->last_use || retired >= list->last_use);
}

static bool queue_list(List *list, const View *view)
{
   if (!list->bytes || !same_view(&list->key,view)) return false;
   const OSTime submitted = GX2GetLastSubmittedTimeStamp();
   if (!list->unsubmitted || submitted > list->before_use) list->before_use = submitted;
   list->unsubmitted = true;
   GX2CallDisplayList(commands(list), list->bytes);
   return true;
}

static bool retire_use(List *list, OSTime submitted, OSTime retired)
{
   if (!list->unsubmitted || submitted <= list->before_use || submitted < list->last_use ||
       submitted <= 0 || retired < submitted) return false;
   list->unsubmitted = false;
   list->last_use = submitted;
   return true;
}

static inline bool draw(const View *view, unsigned section)
{
   if (section < 3)
      return horizon_gx2_terrain_draw_bound(view->terrain,view->section[section],view->clip,view->origin) == 0;
   _Alignas(16) float clip[16], origin[4];
   memcpy(clip,view->clip,sizeof(clip));
   memcpy(origin,view->origin,sizeof(origin));
   clip[3] += (float)(section % 3) * .0003f;
   clip[7] -= (float)(section % 4) * .0002f;
   origin[0] += (float)(section % 16);
   return horizon_gx2_terrain_draw_bound(view->terrain,view->section[section],clip,origin) == 0;
}

static bool direct(const View *view, unsigned count, bool reverse)
{
   for (unsigned i = 0; i < count; ++i) {
      if (!draw(view,reverse ? count-1-i : i)) return false;
   }
   return true;
}

static bool guard_ok(const uint8_t *bytes)
{
   for (unsigned i = 0; i < CACHE_LINE; ++i)
      if (bytes[i] != LIST_GUARD) return false;
   return true;
}

/* No callbacks or foreground pump while GX2 owns the command-list recorder.
 * Holding allocations is safer than asking WHB to tear down an open recorder. */
static void verify_recorder_closed(void)
{
   if (!GX2GetDisplayListWriteStatus()) return;
   OSReport("GX2 solid list fixture retaining resources: recorder did not close\n");
   for (;;) OSSleepTicks(OSMillisecondsToTicks(16));
}

static uint32_t record(List *list, const View *view, unsigned count, unsigned first)
{
   verify_recorder_closed();
   if (!fits(count) || !can_record(list,GX2GetRetiredTimeStamp())) return 0;
   list->bytes = 0;
   uint8_t *bytes = commands(list);
   uint8_t *before = bytes - CACHE_LINE;
   uint8_t *after = bytes + LIST_CAP;
   memset(before,LIST_GUARD,CACHE_LINE);
   memset(after,LIST_GUARD,CACHE_LINE);
   DCFlushRange(before,CACHE_LINE);
   DCFlushRange(after,CACHE_LINE);
   DCInvalidateRange(bytes,LIST_CAP);

   GX2BeginDisplayListEx(bytes,LIST_CAP,FALSE);
   if (!GX2GetDisplayListWriteStatus()) return 0;
   bool ok = count == 1 ? draw(view,first) : direct(view,count,false);
   const uint32_t used = GX2EndDisplayList(bytes);
   verify_recorder_closed();
   DCInvalidateRange(before,CACHE_LINE);
   DCInvalidateRange(after,CACHE_LINE);
   if (!ok || !used || used > LIST_CAP || used % CACHE_LINE ||
       !guard_ok(before) || !guard_ok(after)) return 0;
   DCFlushRange(bytes,used);
   OSMemoryBarrier();
   return used;
}

static bool build(List *list, const View *view)
{
   unsigned max_single = 0;
   for (unsigned i = 0; i < DRAWS; ++i) {
      const uint32_t bytes = record(list,view,1,i);
      if (!bytes || bytes > COMMAND_LIMIT) return false;
      if (bytes > max_single) max_single = bytes;
   }
   if (record(list,view,DRAWS+1,0) || record(list,view,0,0)) return false;
   const uint32_t bytes = record(list,view,DRAWS,0);
   if (!bytes || bytes > DRAWS * COMMAND_LIMIT) return false;
   list->key = *view;
   list->bytes = bytes;
   OSReport("GX2 solid list fixture capacity output=%u draws=%u single_max=%u bytes=%u cap=%u distinct=%u result=PASS\n",
            view->output,DRAWS,max_single,bytes,LIST_CAP,DRAWS);
   return true;
}

static void quad(HorizonTerrainVertex *out, float center_x, float center_y, float depth,
                 unsigned layer, uint8_t cutout, uint8_t r, uint8_t g, uint8_t b)
{
   const float offset[4][2] = {{-.14f,-.12f},{.14f,-.12f},{.14f,.12f},{-.14f,.12f}};
   for (unsigned i = 0; i < 4; ++i) {
      out[i] = (HorizonTerrainVertex){
         .position = {center_x+offset[i][0],center_y+offset[i][1],depth,(float)(16*(i+4)*257)},
         .uv = {(float)(16384 | layer << 16),16384},
         .color_cutout = {r,g,b,cutout},
      };
   }
}

static int fill(void *source, HorizonTerrainVertex *vertices, uint32_t count)
{
   memcpy(vertices,source,(size_t)count*sizeof(*vertices));
   return 0;
}

static const uint8_t *sample(const uint8_t *rgba, const View *view, float x, float y)
{
   const unsigned px = (unsigned)((view->clip[0]*x+view->clip[3]+1)*0.5f*view->width);
   const unsigned py = (unsigned)((1-view->clip[5]*y-view->clip[7])*0.5f*view->height);
   return rgba+((size_t)py*view->width+px)*4;
}

static bool pixels(const uint8_t *rgba, const View *view, enum Arm arm, uint8_t **stock)
{
   const size_t pixels = (size_t)view->width*view->height;
   if (arm == STOCK) {
      *stock = malloc(pixels*4);
      if (!*stock) return false;
      memcpy(*stock,rgba,pixels*4);
   }
   unsigned differences = 0;
   for (size_t i = 0; i < pixels; ++i)
      differences += memcmp(rgba+i*4,*stock+i*4,4) != 0;
   const uint8_t *cutout = sample(rgba,view,-.55f,.4f);
   const uint8_t *depth = sample(rgba,view,-.55f,-.4f);
   const uint8_t *tie = sample(rgba,view,.52f,0);
   const uint8_t *front = sample(rgba,view,.605f,0);
   const uint8_t *behind = sample(rgba,view,.455f,0);
   const bool overlay = tail_color(front,view->output);
   const bool preserved_depth = !memcmp(tie,behind,4);
   bool ok;
   if (arm == WRONG_CAMERA) {
      ok = differences > 100 && !(cutout[1] > 170 && cutout[3] == 127);
   } else {
      ok = cutout[1] > 170 && cutout[3] == 127 && depth[2] > 170 && depth[3] == 128 &&
           tie[3] == 255;
      if (arm == REVERSE_ORDER)
         ok &= differences > 100 && tie[0] > 170 && tie[1] < 80;
      else if (arm == WRONG_TAIL_DEPTH)
         ok &= differences > 100 && overlay && !preserved_depth && tail_color(behind,view->output);
      else
         ok &= differences == 0 && tie[1] > 170 && overlay && preserved_depth &&
               (view->output == 0 ? tie[2] > 170 : tie[0] > 170);
   }
   OSReport("GX2 solid list fixture output=%u arm=%u size=%ux%u differences=%u "
            "samples=%u,%u,%u,%u/%u,%u,%u,%u/%u,%u,%u,%u result=%s\n",
            view->output,arm,view->width,view->height,differences,
            cutout[0],cutout[1],cutout[2],cutout[3],depth[0],depth[1],depth[2],depth[3],
            tie[0],tie[1],tie[2],tie[3],ok?"PASS":"FAIL");
   OSReport("GX2 solid list trailing output=%u arm=%u blended=%u,%u,%u,%u occluded=%u,%u,%u,%u preserved=%u\n",
            view->output,arm,front[0],front[1],front[2],front[3],behind[0],behind[1],behind[2],behind[3],
            (unsigned)preserved_depth);
   return ok;
}

static bool negatives(List *list, const View *view, const View *other)
{
   View wrong = *view;
   wrong.clip[3] += .4f;
   if (queue_list(list,&wrong) || queue_list(list,other)) return false;
   wrong = *view;
   wrong.origin[0] += 1.0f;
   if (queue_list(list,&wrong)) return false;
   wrong = *view;
   ++wrong.epoch;
   if (queue_list(list,&wrong)) return false;
   wrong = *view;
   ++wrong.revision;
   if (queue_list(list,&wrong)) return false;
   wrong = *view;
   --wrong.world_width;
   if (queue_list(list,&wrong)) return false;
   wrong = *view;
   wrong.section[2] = other->section[2];
   return !queue_list(list,&wrong);
}

static bool render(View views[2], List lists[2], const Tail *tail, Stage *stage,
                   uint8_t *rgba, uint8_t *stock[2], enum Arm arm,
                   HorizonGx2WorldScale *scale, const uint32_t world_sizes[4])
{
   const float fog[4] = {0.125f,0.35f,0.75f,1};
   const float ranges[4] = {0,1024,86.40000152587890625f,96};
   const float clear[4] = {0,0,0,1};
   bool ok = true;
   GX2ColorBuffer *colors[2];
   unsigned queued_mask = 0;
   WHBGfxBeginRender();
   for (unsigned i = 0; i < 2; ++i) {
      View *view = &views[i];
      List *list = &lists[i];
      if (i == 0) WHBGfxBeginRenderTV();
      else WHBGfxBeginRenderDRC();
      GX2ColorBuffer *color = i == 0 ? WHBGfxGetTVColourBuffer() : WHBGfxGetDRCColourBuffer();
      GX2DepthBuffer *depth = i == 0 ? WHBGfxGetTVDepthBuffer() : WHBGfxGetDRCDepthBuffer();
      colors[i] = color;
      view->width = color->surface.width;
      view->height = color->surface.height;
      view->world_width = scale ? world_sizes[2*i] : view->width;
      view->world_height = scale ? world_sizes[2*i+1] : view->height;
      if (scale) {
         horizon_gx2_world_scale_begin(scale,i,clear,false);
      } else {
         GX2ClearBuffersEx(color,depth,0,0,0,1,1,0,GX2_CLEAR_FLAGS_DEPTH);
         GX2SetContextState(i == 0 ? WHBGfxGetTVContextState() : WHBGfxGetDRCContextState());
         GX2SetColorBuffer(color,GX2_RENDER_TARGET_0);
         GX2SetDepthBuffer(depth);
         GX2SetViewport(0,0,view->width,view->height,0,1);
         GX2SetScissor(0,0,view->width,view->height);
      }
      GX2SetRasterizerClipControlEx(TRUE,TRUE,TRUE);
      GX2SetTargetChannelMasks(GX2_CHANNEL_MASK_RGBA,0,0,0,0,0,0,0);
      tail_draw(tail,view,0,false);
      if (horizon_gx2_terrain_bind(view->terrain,view->world_width,view->world_height,0,0,
                                   view->world_width,view->world_height,fog,ranges,false) != 0)
         return false;
      bool queued = false;
      if (arm == LIST_REPLAY || arm == LIST_AGAIN || arm == RERECORD) {
         queued = (arm == LIST_AGAIN || build(list,view)) && queue_list(list,view);
         if (!queued)
            OSReport("GX2 solid list fixture output=%u uses ordinary fallback after recording failure\n",i);
         ok &= queued;
         if (queued) {
            queued_mask |= 1u << i;
            ok &= !can_record(list,GX2GetRetiredTimeStamp()) &&
                  !retire_use(list,list->before_use,list->before_use);
         }
      } else if (arm == REJECT_AND_DRAW) {
         ok &= negatives(list,view,&views[1-i]);
      } else if (arm == CLOSED_RECORD_ABORT) {
         const uint32_t rejected = record(list,view,DRAWS,0);
         const bool ready = rejected && !list->bytes && !queue_list(list,view);
         ok &= ready;
         OSReport("GX2 solid list closed abort output=%u bytes=%u prior_invalidated=%u\n",
                  i,rejected,(unsigned)ready);
      }
      View overridden = *view;
      if (arm == WRONG_CAMERA) {
         overridden.clip[3] += .4f;
         overridden.origin[0] += 18.0f;
      }
      if (!queued && !direct(&overridden,DRAWS,arm == REVERSE_ORDER)) return false;

      // No context reset, flush or fence between the solids and the tail.
      tail_draw(tail,view,1,false);
      tail_draw(tail,view,2,arm == WRONG_TAIL_DEPTH);
      if (scale)
         horizon_gx2_world_scale_end(scale,i,(const uint32_t[]){0,0,view->width,view->height},true);
      if (i == 0) WHBGfxFinishRenderTV();
      else WHBGfxFinishRenderDRC();
   }

   // Both lists belong to this pair until the ordinary presentation tail
   // proves retirement. The later readback fences must not supply that proof.
   const unsigned held_mask = (lists[0].unsubmitted ? 1u : 0u) |
                              (lists[1].unsubmitted ? 2u : 0u);
   ok &= held_mask == queued_mask;
   const OSTime before_finish = queued_mask ? GX2GetLastSubmittedTimeStamp() : 0;
   WHBGfxFinishRender();
   if (queued_mask) {
      const OSTime submitted = GX2GetLastSubmittedTimeStamp();
      const OSTime retired = GX2GetRetiredTimeStamp();
      unsigned verified_mask = 0;
      for (unsigned i = 0; i < 2; ++i) {
         List *list = &lists[i];
         if (!(queued_mask & (1u << i))) continue;
         const bool verified = submitted > before_finish && retire_use(list,submitted,retired);
         const bool no_early_write = verified &&
            !can_record(list,list->last_use-1) && can_record(list,retired);
         if (no_early_write) verified_mask |= 1u << i;
         OSReport("GX2 solid list fixture retirement output=%u submitted=%lld retired=%lld result=%s\n",
                  i,(long long)submitted,(long long)retired,no_early_write?"PASS":"FAIL");
      }
      const bool paired = held_mask == 3 && queued_mask == 3 && verified_mask == 3;
      ok &= paired;
      OSReport("GX2 solid list pair retirement scaled=%u arm=%u held=%u retired=%u result=%s\n",
               (unsigned)(scale != NULL),arm,held_mask,verified_mask,paired?"PASS":"FAIL");
   }
   for (unsigned i = 0; i < 2; ++i) {
      const enum ReadStatus status = stage_read(stage,colors[i],rgba,RGBA_CAP);
      if (status != READ_OK) {
         OSReport("GX2 solid list fixture output=%u arm=%u readback=%u\n",i,arm,status);
         return false;
      }
      ok &= pixels(rgba,&views[i],arm,&stock[i]);
   }
   return ok;
}

int main(void)
{
   WHBProcInit();
   const bool gfx = WHBGfxInit();
   HorizonGx2Terrain *terrain = NULL;
   const uint8_t alpha[] = {0,127,128,255};
   uint8_t atlas[16*4*4], light[16*16*4];
   memset(light,255,sizeof(light));
   for (unsigned i = 0; i < 16*4; ++i)
      memcpy(atlas+i*4,(uint8_t[]){255,255,255,alpha[(i%16)/4]},4);
   bool ok = gfx &&
      horizon_gx2_terrain_create(&terrain,_binary_terrain_gsh_start,16,4,atlas,sizeof(atlas)) == 0 &&
      horizon_gx2_terrain_set_sprite_grid(terrain,4,4) == 0 &&
      horizon_gx2_terrain_lightmap(terrain,light) == 0;
   Tail tail = {0};
   ok = ok && tail_init(&tail);
   View views[2] = {
      {.output=0,.epoch=1,.revision=1,.clip={1,0,0,-.025f,0,1,0,0,0,0,1,0,0,0,0,1},.origin={24,8,-8,0}},
      {.output=1,.epoch=1,.revision=2,.clip={.89f,0,0,.07f,0,.92f,0,.03f,0,0,1,0,0,0,0,1},.origin={32,-4,8,0}},
   };
   List lists[2] = {0};
   for (unsigned i = 0; ok && i < 2; ++i) {
      View *v = &views[i];
      v->terrain = terrain;
      HorizonTerrainVertex vertices[3][12];
      quad(vertices[0],-.55f,.4f,.6f,1,0,0,255,0);
      quad(vertices[0]+4,-.55f,-.4f,.6f,3,0,255,255,255);
      quad(vertices[0]+8,.52f,0,.33f,3,0,255,0,0);
      quad(vertices[1],-.55f,.4f,.2f,1,255,255,0,0);
      quad(vertices[1]+4,-.55f,-.4f,.2f,2,255,0,0,255);
      quad(vertices[2],.52f,0,.33f,3,0,i?255:0,255,i?0:255);
      const unsigned sizes[] = {12,8,4};
      for (unsigned s = 0; ok && s < DRAWS; ++s) {
         HorizonTerrainVertex content[20];
         const unsigned count = s < 3 ? sizes[s] : 4*(1+s%5);
         if (s < 3) {
            memcpy(content,vertices[s],count*sizeof(*content));
         } else {
            const float x = -.185f + (float)(s%13)*.03f;
            const float y = -.82f + (float)(s/13)*.12f;
            for (unsigned q = 0; q < count; q += 4) {
               quad(content+q,x,y,.7f,3,0,(uint8_t)(s*11),(uint8_t)(s*23),i?64:192);
               for (unsigned c = q; c < q+4; ++c) {
                  content[c].position[0] = x + (content[c].position[0]-x)*.07f;
                  content[c].position[1] = y + (content[c].position[1]-y)*.23f;
               }
            }
         }
         uint32_t bytes;
         ok = horizon_gx2_terrain_section_create(&v->section[s],count,fill,content,&bytes) == 0 &&
            v->section[s] && bytes != 0;
         for (unsigned previous = 0; ok && previous < s; ++previous)
            ok = v->section[s] != v->section[previous];
         for (unsigned previous = 0; ok && i && previous < DRAWS; ++previous)
            ok = v->section[s] != views[0].section[previous];
      }
      lists[i].storage = MEMAllocFromDefaultHeapEx(LIST_CAP+2*LIST_ALIGN,LIST_ALIGN);
      ok &= lists[i].storage != NULL;
   }
   Stage stage = {0};
   uint8_t *rgba = NULL, *stock[2] = {0};
   if (ok) {
      stage.storage = MEMAllocFromDefaultHeapEx(STAGE_CAP,STAGE_ALIGN);
      rgba = MEMAllocFromDefaultHeap(RGBA_CAP);
      ok = stage.storage && rgba;
   }
   if (ok) OSReport("GX2 solid list fixture geometry unique_tv=%u unique_drc=%u disjoint=1\n",DRAWS,DRAWS);
   const uint32_t world_sizes[4] = {1440,809,641,360};
   for (enum Arm arm = STOCK; ok && arm <= WRONG_TAIL_DEPTH; ++arm)
      ok = render(views,lists,&tail,&stage,rgba,stock,arm,NULL,world_sizes);

   uint32_t surface_bytes = 0;
   HorizonGx2WorldScale *scale = NULL;
   if (ok) {
      scale = horizon_gx2_world_scale_create(world_sizes,&surface_bytes);
      ok = scale && surface_bytes;
      OSReport("GX2 solid list fixture scaled tv=%ux%u drc=%ux%u bytes=%u result=%s\n",
               world_sizes[0],world_sizes[1],world_sizes[2],world_sizes[3],surface_bytes,ok?"PASS":"FAIL");
   }
   for (unsigned i = 0; i < 2; ++i) {
      free(stock[i]);
      stock[i] = NULL;
   }
   for (enum Arm arm = STOCK; ok && arm <= WRONG_TAIL_DEPTH; ++arm)
      ok = render(views,lists,&tail,&stage,rgba,stock,arm,scale,world_sizes);

   // A driver timeout or missing submission timestamp cannot free any of the
   // command, texture, color, depth or index memory still owned by a GPU packet.
   if (gfx) {
      verify_recorder_closed();
      retire_for_teardown(&stage,"GX2 solid list fixture");
   }
   for (unsigned i = 0; i < 2; ++i) {
      for (unsigned s = 0; s < DRAWS; ++s)
         horizon_gx2_terrain_section_destroy_after_wait(views[i].section[s]);
      if (lists[i].storage) MEMFreeToDefaultHeap(lists[i].storage);
      free(stock[i]);
   }
   if (rgba) MEMFreeToDefaultHeap(rgba);
   if (stage.storage) MEMFreeToDefaultHeap(stage.storage);
   if (tail.vertices) MEMFreeToDefaultHeap(tail.vertices);
   if (tail.shader.vertexShader) WHBGfxFreeShaderGroup(&tail.shader);
   if (scale) horizon_gx2_world_scale_destroy(scale);
   if (terrain) horizon_gx2_terrain_destroy(terrain);
   if (gfx) WHBGfxShutdown();
   WHBProcShutdown();
   OSReport("GX2 solid list fixture result=%s\n",ok?"PASS":"FAIL");
   return ok?0:1;
}
