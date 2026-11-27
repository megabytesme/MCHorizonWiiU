#include "terrain.h"
#include "../gx2-explicit-readback/retire.h"
#include <coreinit/memdefaultheap.h>
#include <gx2/clear.h>
#include <gx2/context.h>
#include <gx2/displaylist.h>
#include <stdlib.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern const uint8_t _binary_terrain_gsh_start[];

enum {
   SECTIONS = 3, DRAWS = 174, COMMAND_LIMIT = 256,
   LIST_CAP = 64 * 1024, LIST_ALIGN = 256, LIST_GUARD = 0x6b,
};

typedef struct {
   unsigned output, epoch, revision, width, height;
   HorizonGx2Terrain *terrain;
   HorizonGx2TerrainSection *section[SECTIONS];
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

enum Arm { STOCK, LIST_REPLAY, REJECT_AND_DRAW, LIST_AGAIN, WRONG_CAMERA, REVERSE_ORDER };

static uint8_t *commands(const List *list)
{
   return list->storage + LIST_ALIGN;
}

static bool same_view(const View *a, const View *b)
{
   return a->output == b->output && a->epoch == b->epoch &&
      a->revision == b->revision && a->width == b->width && a->height == b->height &&
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
   return horizon_gx2_terrain_draw_bound(view->terrain,view->section[section],view->clip,view->origin) == 0;
}

static bool direct(const View *view, unsigned count, bool reverse)
{
   for (unsigned i = 0; i < count; ++i) {
      const unsigned section = i % SECTIONS;
      if (!draw(view,reverse ? SECTIONS-1-section : section)) return false;
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
   for (unsigned i = 0; i < SECTIONS; ++i) {
      const uint32_t bytes = record(list,view,1,i);
      if (!bytes || bytes > COMMAND_LIMIT) return false;
      if (bytes > max_single) max_single = bytes;
   }
   if (record(list,view,DRAWS+1,0) || record(list,view,0,0)) return false;
   const uint32_t bytes = record(list,view,DRAWS,0);
   if (!bytes || bytes > DRAWS * COMMAND_LIMIT) return false;
   list->key = *view;
   list->bytes = bytes;
   OSReport("GX2 solid list fixture capacity output=%u draws=%u single_max=%u bytes=%u cap=%u result=PASS\n",
            view->output,DRAWS,max_single,bytes,LIST_CAP);
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
   bool ok;
   if (arm == WRONG_CAMERA) {
      ok = differences > 100 && !(cutout[1] > 170 && cutout[3] == 127);
   } else {
      ok = cutout[1] > 170 && cutout[3] == 127 && depth[2] > 170 && depth[3] == 128 &&
           tie[3] == 255;
      if (arm == REVERSE_ORDER)
         ok &= differences > 100 && tie[0] > 170 && tie[1] < 80;
      else
         ok &= differences == 0 && tie[1] > 170 &&
               (view->output == 0 ? tie[2] > 170 : tie[0] > 170);
   }
   OSReport("GX2 solid list fixture output=%u arm=%u size=%ux%u differences=%u "
            "samples=%u,%u,%u,%u/%u,%u,%u,%u/%u,%u,%u,%u result=%s\n",
            view->output,arm,view->width,view->height,differences,
            cutout[0],cutout[1],cutout[2],cutout[3],depth[0],depth[1],depth[2],depth[3],
            tie[0],tie[1],tie[2],tie[3],ok?"PASS":"FAIL");
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
   wrong.section[2] = other->section[2];
   return !queue_list(list,&wrong);
}

static bool render(View views[2], List lists[2], Stage *stage,
                   uint8_t *rgba, uint8_t *stock[2], enum Arm arm)
{
   const float fog[4] = {0.125f,0.35f,0.75f,1};
   const float ranges[4] = {0,1024,86.40000152587890625f,96};
   bool ok = true;
   WHBGfxBeginRender();
   for (unsigned i = 0; i < 2; ++i) {
      View *view = &views[i];
      List *list = &lists[i];
      if (i == 0) WHBGfxBeginRenderTV();
      else WHBGfxBeginRenderDRC();
      GX2ColorBuffer *color = i == 0 ? WHBGfxGetTVColourBuffer() : WHBGfxGetDRCColourBuffer();
      GX2DepthBuffer *depth = i == 0 ? WHBGfxGetTVDepthBuffer() : WHBGfxGetDRCDepthBuffer();
      view->width = color->surface.width;
      view->height = color->surface.height;
      GX2ClearBuffersEx(color,depth,0,0,0,1,1,0,GX2_CLEAR_FLAGS_DEPTH);
      GX2SetContextState(i == 0 ? WHBGfxGetTVContextState() : WHBGfxGetDRCContextState());
      GX2SetColorBuffer(color,GX2_RENDER_TARGET_0);
      GX2SetDepthBuffer(depth);
      if (horizon_gx2_terrain_bind(view->terrain,view->width,view->height,0,0,
                                   view->width,view->height,fog,ranges,false) != 0)
         return false;
      bool queued = false;
      if (arm == LIST_REPLAY || arm == LIST_AGAIN) {
         queued = (arm == LIST_AGAIN || build(list,view)) && queue_list(list,view);
         if (!queued)
            OSReport("GX2 solid list fixture output=%u uses ordinary fallback after recording failure\n",i);
         ok &= queued;
         if (queued)
            ok &= !can_record(list,GX2GetRetiredTimeStamp()) &&
                  !retire_use(list,list->before_use,list->before_use);
      } else if (arm == REJECT_AND_DRAW) {
         ok &= negatives(list,view,&views[1-i]);
      }
      View overridden = *view;
      if (arm == WRONG_CAMERA) {
         overridden.clip[3] += .4f;
         overridden.origin[0] += 18.0f;
      }
      if (!queued && !direct(&overridden,DRAWS,arm == REVERSE_ORDER)) return false;

      const enum ReadStatus status = stage_read(stage,color,rgba,RGBA_CAP);
      if (status != READ_OK) {
         OSReport("GX2 solid list fixture output=%u arm=%u readback=%u\n",i,arm,status);
         return false;
      }
      if (queued) {
         const OSTime submitted = GX2GetLastSubmittedTimeStamp();
         const OSTime retired = GX2GetRetiredTimeStamp();
         const bool verified = retire_use(list,submitted,retired);
         const bool no_early_write = verified &&
            !can_record(list,list->last_use-1) && can_record(list,retired);
         ok &= no_early_write;
         OSReport("GX2 solid list fixture retirement output=%u submitted=%lld retired=%lld result=%s\n",
                  i,(long long)submitted,(long long)retired,no_early_write?"PASS":"FAIL");
      }
      ok &= pixels(rgba,view,arm,&stock[i]);
      if (i == 0) WHBGfxFinishRenderTV();
      else WHBGfxFinishRenderDRC();
   }
   WHBGfxFinishRender();
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
   View views[2] = {
      {.output=0,.epoch=1,.revision=1,.clip={1,0,0,-.025f,0,1,0,0,0,0,1,0,0,0,0,1},.origin={24,8,-8,0}},
      {.output=1,.epoch=1,.revision=2,.clip={.89f,0,0,.07f,0,.92f,0,.03f,0,0,1,0,0,0,0,1},.origin={32,-4,8,0}},
   };
   List lists[2] = {0};
   for (unsigned i = 0; ok && i < 2; ++i) {
      View *v = &views[i];
      v->terrain = terrain;
      HorizonTerrainVertex vertices[SECTIONS][12];
      quad(vertices[0],-.55f,.4f,.6f,1,0,0,255,0);
      quad(vertices[0]+4,-.55f,-.4f,.6f,3,0,255,255,255);
      quad(vertices[0]+8,.52f,0,.33f,3,0,255,0,0);
      quad(vertices[1],-.55f,.4f,.2f,1,255,255,0,0);
      quad(vertices[1]+4,-.55f,-.4f,.2f,2,255,0,0,255);
      quad(vertices[2],.52f,0,.33f,3,0,i?255:0,255,i?0:255);
      const unsigned sizes[] = {12,8,4};
      uint32_t bytes;
      for (unsigned s = 0; ok && s < SECTIONS; ++s)
         ok = horizon_gx2_terrain_section_create(&v->section[s],sizes[s],fill,vertices[s],&bytes) == 0;
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
   for (enum Arm arm = STOCK; ok && arm <= REVERSE_ORDER; ++arm)
      ok = render(views,lists,&stage,rgba,stock,arm);

   // A driver timeout or missing submission timestamp cannot free any of the
   // command, texture, color, depth or index memory still owned by a GPU packet.
   if (gfx) {
      verify_recorder_closed();
      retire_for_teardown(&stage,"GX2 solid list fixture");
   }
   for (unsigned i = 0; i < 2; ++i) {
      for (unsigned s = 0; s < SECTIONS; ++s)
         horizon_gx2_terrain_section_destroy_after_wait(views[i].section[s]);
      if (lists[i].storage) MEMFreeToDefaultHeap(lists[i].storage);
      free(stock[i]);
   }
   if (rgba) MEMFreeToDefaultHeap(rgba);
   if (stage.storage) MEMFreeToDefaultHeap(stage.storage);
   if (terrain) horizon_gx2_terrain_destroy(terrain);
   if (gfx) WHBGfxShutdown();
   WHBProcShutdown();
   OSReport("GX2 solid list fixture result=%s\n",ok?"PASS":"FAIL");
   return ok?0:1;
}
