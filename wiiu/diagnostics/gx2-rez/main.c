#include "terrain.h"
#include "../gx2-explicit-readback/retire.h"

#include <coreinit/memdefaultheap.h>
#include <gx2/context.h>
#include <gx2/registers.h>
#include <gx2/shaders.h>
#include <gx2/swap.h>
#include <gx2r/surface.h>
#include <proc_ui/procui.h>
#include <stdlib.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern const uint8_t _binary_terrain_gsh_start[];
extern const uint8_t _binary_terrain_rez_gsh_start[];
extern const uint8_t _binary_opaque_gsh_start[];
extern const uint8_t _binary_clear_gsh_start[];
/* Same aligned destination and 27 MHz clock as the hosted physical sampler. */
extern void GX2SampleBottomGPUCycle(uint64_t *destination);

enum { STOCK, REZ, WRONG_KILL };
enum {
   OUTPUTS = 2, LEVELS = 3, ORDERS = 2, ROUNDS = 4,
   FAR_MAX = 24, FIRST_GUARD = 1, GUARDS = 8, FIRST_FRONT = FIRST_GUARD + GUARDS,
   FRONTS = 2, FIRST_FAR = FIRST_FRONT + FRONTS, QUADS = FIRST_FAR + FAR_MAX,
};
static const unsigned levels[LEVELS] = {0, 8, FAR_MAX};
static const GX2ScanTarget scan[OUTPUTS] = {GX2_SCAN_TARGET_TV, GX2_SCAN_TARGET_DRC};
static const uint8_t background[OUTPUTS][4] = {{18,31,70,255}, {63,28,17,255}};

typedef struct {
   GX2ColorBuffer *color;
   GX2DepthBuffer depth;
   GX2ContextState *context;
   bool owns_depth;
} Target;

typedef struct {
   _Alignas(64) uint64_t value;
} CycleSlot;

_Static_assert(sizeof(CycleSlot) == 64, "one GPU counter per cache line");

typedef struct {
   HorizonGx2Terrain *terrain;
   HorizonGx2TerrainSection *quads;
   GX2PixelShader *pixel[3], *clear;
   uint32_t clear_offset;
   Target targets[OUTPUTS];
   Stage stage;
   uint8_t *rgba, *reference[OUTPUTS];
   CycleSlot cycles[OUTPUTS][2];
   bool pair_open;
} Fixture;

static Fixture fixture;
static uint64_t timings[ORDERS][LEVELS][ROUNDS][2][OUTPUTS];

static const _Alignas(16) float clip[OUTPUTS][16] = {
   {1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1},
   {0.91f,0,0,0.014f, 0,1,0,0, 0,0,1,0, 0,0,0,1},
};
static const _Alignas(16) float close[OUTPUTS][4] = {{11,8,13,0}, {9,12,17,0}};
static const _Alignas(16) float work[OUTPUTS][4] = {{89,5,11,0}, {92,5,19,0}};
static const _Alignas(16) float fog[4] = {0.18f,0.32f,0.53f,1};
static const _Alignas(16) float ranges[4] = {0,1024,86.40000152587890625f,96};

static const GX2RResourceFlags depth_flags = GX2R_RESOURCE_BIND_DEPTH_BUFFER |
   GX2R_RESOURCE_USAGE_GPU_WRITE | GX2R_RESOURCE_USAGE_FORCE_MEM2;

static bool distinct_targets(Fixture *f)
{
   f->targets[0].color = WHBGfxGetTVColourBuffer();
   f->targets[0].context = WHBGfxGetTVContextState();
   f->targets[1].color = WHBGfxGetDRCColourBuffer();
   f->targets[1].context = WHBGfxGetDRCContextState();
   const void *images[4];
   uint64_t allocated = 0;
   for (unsigned i = 0; i < OUTPUTS; ++i) {
      Target *t = &f->targets[i];
      if (!t->color || !t->context || t->color->surface.width > 1920 ||
          t->color->surface.height > 1080 || !t->color->surface.width || !t->color->surface.height)
         return false;
      // Match the scaled game's D32 format and MEM2 placement while keeping
      // the explicit native-size scan targets in this isolated fixture.
      GX2Surface *depth = &t->depth.surface;
      *depth = (GX2Surface){
         .dim = GX2_SURFACE_DIM_TEXTURE_2D, .width = t->color->surface.width,
         .height = t->color->surface.height, .depth = 1, .mipLevels = 1,
         .format = GX2_SURFACE_FORMAT_FLOAT_R32, .aa = GX2_AA_MODE1X,
         .use = GX2_SURFACE_USE_DEPTH_BUFFER, .tileMode = GX2_TILE_MODE_DEFAULT,
      };
      GX2CalcSurfaceSizeAndAlignment(depth);
      if (!depth->imageSize || !depth->alignment ||
          (uint64_t)depth->imageSize + depth->alignment + allocated > 14 * 1024 * 1024)
         return false;
      allocated += (uint64_t)depth->imageSize + depth->alignment;
      if (!GX2RCreateSurface(depth,depth_flags)) return false;
      t->owns_depth = true;
      t->depth.viewNumSlices = 1;
      t->depth.depthClear = 1.0f;
      GX2InitDepthBufferRegs(&t->depth);
      images[2*i] = t->color->surface.image;
      images[2*i+1] = depth->image;
      OSReport("GX2 ReZ target=%u scan=%u size=%ux%u depth_fmt=%x depth_bytes=%u\n",
               i,(unsigned)scan[i],depth->width,depth->height,(unsigned)depth->format,depth->imageSize);
   }
   for (unsigned i = 0; i < 4; ++i) {
      if (!images[i]) return false;
      for (unsigned j = 0; j < i; ++j)
         if (images[i] == images[j]) return false;
   }
   return f->targets[0].context != f->targets[1].context;
}

static bool load_pixels(Fixture *f)
{
   f->pixel[STOCK] = WHBGfxLoadGFDPixelShader(0, _binary_terrain_gsh_start);
   f->pixel[REZ] = WHBGfxLoadGFDPixelShader(0, _binary_terrain_rez_gsh_start);
   f->pixel[WRONG_KILL] = WHBGfxLoadGFDPixelShader(0, _binary_opaque_gsh_start);
   f->clear = WHBGfxLoadGFDPixelShader(0, _binary_clear_gsh_start);
   GX2PixelShader *native = f->pixel[STOCK], *rez = f->pixel[REZ];
   if (!native || !rez || !f->pixel[WRONG_KILL] || !f->clear || !native->program ||
       !rez->program || native->size != 142u * 4 || native->size != rez->size ||
       native->regs.db_shader_control != 0x50 || rez->regs.db_shader_control != 0x70 ||
       f->pixel[WRONG_KILL]->regs.db_shader_control != 0x10 ||
       native->mode != GX2_SHADER_MODE_UNIFORM_REGISTER || rez->mode != native->mode ||
       f->clear->mode != native->mode || memcmp(native->program,rez->program,native->size))
      return false;
   const GX2PixelShader *a = native, *b = rez;
   const size_t offset = offsetof(GX2PixelShader,regs.db_shader_control);
   const size_t end = sizeof(a->regs);
   const size_t tail = offset + sizeof(a->regs.db_shader_control);
   if (memcmp(&a->regs,&b->regs,offset) ||
       memcmp((const uint8_t *)&a->regs+tail,(const uint8_t *)&b->regs+tail,end-tail))
      return false;
   const GX2UniformVar *u = GX2GetPixelUniformVar(f->clear,"uClear");
   if (!u || u->type != GX2_SHADER_VAR_TYPE_FLOAT4 || u->block != -1 || u->offset % 4)
      return false;
   f->clear_offset = u->offset;
   return true;
}

static void quad(HorizonTerrainVertex *out, float x0, float x1, float y0, float y1, float z,
                 uint32_t layer, bool cutout, uint8_t r, uint8_t g, uint8_t b)
{
   const float pos[4][2] = {{x0,y0},{x1,y0},{x1,y1},{x0,y1}};
   for (unsigned i = 0; i < 4; ++i) {
      const unsigned u = layer != 4 ? 16384 : (i == 0 || i == 3 ? 0 : 32768);
      out[i] = (HorizonTerrainVertex){
         .position = {pos[i][0],pos[i][1],z,0},
         .uv = {(float)(layer << 16 | u),16384},
         .color_cutout = {r,g,b,cutout ? 255 : 0},
      };
   }
}

static uint32_t next(uint32_t *state)
{
   *state = *state * 1664525u + 1013904223u;
   return *state >> 16;
}

static int fill(void *data, HorizonTerrainVertex *out, uint32_t count)
{
   memcpy(out,data,count * sizeof(*out));
   return 0;
}

static bool build_mesh(Fixture *f)
{
   HorizonTerrainVertex q[4*QUADS];
   quad(q,-1,1,-1,1,1,3,false,255,255,255);
   HorizonTerrainVertex *g = q + FIRST_GUARD*4;
   quad(g   ,-0.98f,-0.52f,0.55f,0.96f,0.20f,1,true,255,15,15);
   quad(g+4 ,-0.98f,-0.52f,0.55f,0.96f,0.58f,1,false,18,255,50);
   quad(g+8 ,-0.48f,-0.02f,0.55f,0.96f,0.26f,2,true,15,20,255);
   quad(g+12,-0.48f,-0.02f,0.55f,0.96f,0.52f,3,false,255,255,255);
   quad(g+16, 0.02f, 0.48f,0.55f,0.96f,0.42f,3,false,255,12,30);
   quad(g+20, 0.02f, 0.48f,0.55f,0.96f,0.42f,3,false,8,255,40);
   quad(g+24, 0.52f, 0.98f,0.55f,0.96f,0.36f,0,false,220,45,210);
   quad(g+28, 0.52f, 0.98f,0.55f,0.96f,0.60f,3,false,45,255,20);
   quad(q + FIRST_FRONT*4,-0.96f,0.04f,-0.49f,0.48f,0.18f,3,false,42,164,62);
   quad(q + (FIRST_FRONT+1)*4,-0.02f,0.89f,-0.45f,0.43f,0.20f,4,true,219,70,30);
   uint32_t seed = 0x17aed64fu;
   for (unsigned i = 0; i < FAR_MAX; ++i) {
      const float jitter = (next(&seed)%12) * 0.001f;
      const uint8_t r = 48 + next(&seed)%196, g = 48 + next(&seed)%196, b = 48 + next(&seed)%196;
      const uint32_t layer = (i%4 == 0) ? 4 : ((i%3 == 0) ? 2 : 3);
      quad(q+(FIRST_FAR+i)*4,-0.97f+jitter,0.97f-jitter,-0.49f+jitter,
           0.48f-jitter,0.82f-0.021f*i,layer,layer!=3,r,g,b);
   }

   uint8_t atlas[32*4*4];
   static const uint8_t a[4] = {0,127,128,255};
   for (unsigned y = 0; y < 4; ++y)
      for (unsigned x = 0; x < 32; ++x) {
         const unsigned cell = x/4;
         const uint8_t alpha = cell == 4 ? (x%4 < 2 ? 0 : 255) : a[cell%4];
         memcpy(atlas+4*(y*32+x),(const uint8_t[]){247,251,244,alpha},4);
      }
   uint8_t light[16*16*4];
   memset(light,255,sizeof(light));
   uint32_t bytes;
   return horizon_gx2_terrain_create(&f->terrain,_binary_terrain_gsh_start,32,4,atlas,sizeof(atlas)) == 0 &&
      horizon_gx2_terrain_set_sprite_grid(f->terrain,8,4) == 0 &&
      horizon_gx2_terrain_lightmap(f->terrain,light) == 0 &&
      horizon_gx2_terrain_section_create(&f->quads,4*QUADS,fill,q,&bytes) == 0;
}

static bool buffers(Fixture *f)
{
   f->stage.storage = MEMAllocFromDefaultHeapEx(STAGE_CAP,STAGE_ALIGN);
   f->rgba = MEMAllocFromDefaultHeap(RGBA_CAP);
   if (!f->stage.storage || !f->rgba) return false;
   for (unsigned i = 0; i < OUTPUTS; ++i) {
      const GX2Surface *s = &f->targets[i].color->surface;
      f->reference[i] = malloc((size_t)s->width * s->height * 4);
      if (!f->reference[i]) return false;
   }
   return true;
}

static bool draw(Fixture *f, unsigned first, unsigned count, const float transform[16], const float origin[4])
{
   const uint32_t run[1][2] = {{first,count}};
   return horizon_gx2_terrain_draw_bound_solid_ranges(f->terrain,f->quads,transform,origin,run,1) == 0;
}

static bool begin_target(Fixture *f, unsigned out)
{
   const Target *t = &f->targets[out];
   if (out == 0) WHBGfxBeginRenderTV();
   else WHBGfxBeginRenderDRC();
   GX2SetContextState(t->context);
   GX2SetColorBuffer(t->color,GX2_RENDER_TARGET_0);
   GX2SetDepthBuffer(&t->depth);
   const uint32_t w = t->color->surface.width, h = t->color->surface.height;
   if (horizon_gx2_terrain_bind(f->terrain,w,h,0,0,w,h,fog,ranges,false) != 0) return false;
   const _Alignas(16) float seed_color[4] = {background[out][0]/255.0f,background[out][1]/255.0f,
                                            background[out][2]/255.0f,1.0f};
   GX2SetPixelShader(f->clear);
   GX2SetPixelUniformReg(f->clear_offset,4,seed_color);
   GX2SetDepthOnlyControl(TRUE,TRUE,GX2_COMPARE_FUNC_ALWAYS);
   if (!draw(f,0,1,clip[0],close[out])) return false;
   // The clear shader's uniform overlaps the terrain register bank.
   // Restore the entire stock terrain state outside the timed bracket.
   return horizon_gx2_terrain_bind(f->terrain,w,h,0,0,w,h,fog,ranges,false) == 0;
}

static bool draw_case(Fixture *f, unsigned out, unsigned level, unsigned order)
{
   const unsigned count = levels[level];
   if (!draw(f,FIRST_GUARD,GUARDS,clip[out],close[out])) return false;
   if (!order || !count) {
      if (!draw(f,FIRST_FRONT,1,clip[out],work[out])) return false;
   }
   for (unsigned i = 0; i < count; ++i) {
      if (order && i == count/2 && !draw(f,FIRST_FRONT,1,clip[out],work[out])) return false;
      if (!draw(f,FIRST_FAR+i,1,clip[out],work[out])) return false;
      if (i%4 == 0 && !draw(f,FIRST_FRONT+1,1,clip[out],work[out])) return false;
   }
   return true;
}

static void cycle(Fixture *f, unsigned out, unsigned edge)
{
   CycleSlot *slot = &f->cycles[out][edge];
   slot->value = UINT64_MAX;
   DCFlushRange(slot,sizeof(*slot));
   GX2SampleBottomGPUCycle(&slot->value);
}

static uint64_t elapsed_cycles(Fixture *f, unsigned out)
{
   uint64_t value[2];
   for (unsigned edge = 0; edge < 2; ++edge) {
      CycleSlot *slot = &f->cycles[out][edge];
      DCInvalidateRange(slot,sizeof(*slot));
      value[edge] = *(volatile uint64_t *)&slot->value;
   }
   if (!value[0] || value[1] == UINT64_MAX || value[1] <= value[0]) return 0;
   const uint64_t elapsed = value[1] - value[0];
   // Keep implausible counter jumps out of the hardware timing dataset.
   return elapsed <= 27000000ull * 10 ? elapsed : 0;
}

static const uint8_t *at(const Fixture *f, unsigned w, unsigned x, unsigned y)
{
   return f->rgba + ((size_t)y*w+x)*4;
}

static bool verify_pixels(Fixture *f, unsigned out, unsigned arm, unsigned level, unsigned order)
{
   const GX2ColorBuffer *color = f->targets[out].color;
   const unsigned w = color->surface.width, h = color->surface.height;
   const enum ReadStatus status = stage_read(&f->stage,color,f->rgba,RGBA_CAP);
   if (status != READ_OK) {
      OSReport("GX2 ReZ readback out=%u status=%u\n",out,status);
      return false;
   }

   const uint8_t *g0 = at(f,w,w/8,h/8), *g1 = at(f,w,3*w/8,h/8);
   const uint8_t *g2 = at(f,w,5*w/8,h/8), *g3 = at(f,w,7*w/8,h/8);
   bool controls = g0[3] == 127 && g1[3] == 128 && g2[3] == 255 && g3[3] == 0 &&
                   g1[2] > g1[0] && g2[1] > g2[0] && g2[1] > g2[2] && g3[0] > g3[1] &&
                   !memcmp(at(f,w,w/2,7*h/8),background[out],4);
   unsigned diff = 0;
   const size_t length = (size_t)w * h * 4;
   if (arm == STOCK) memcpy(f->reference[out],f->rgba,length);
   else for (size_t i = 0; i < length; i += 4)
      diff += memcmp(f->reference[out]+i,f->rgba+i,4) != 0;
   const bool hole = arm == WRONG_KILL ? g0[0] > g0[1] : g0[1] > g0[0];
   const bool valid = controls && hole && (arm == WRONG_KILL ? diff > w*h/30 : !diff);
   OSReport("GX2 ReZ pixels out=%u size=%ux%u arm=%u far=%u order=%u diff=%u ctrl=%u hole=%u %s\n",
            out,w,h,arm,levels[level],order,diff,(unsigned)controls,(unsigned)hole,valid ? "PASS" : "FAIL");
   return valid;
}

static bool render_pair(Fixture *f, unsigned arm, unsigned level, unsigned order, uint64_t clocks[OUTPUTS])
{
   if (f->pair_open || !WHBProcIsRunning() || !ProcUIInForeground()) return false;
   f->pair_open = true;
   WHBGfxBeginRender();
   uint32_t swaps, flips;
   OSTime flip_time, vsync;
   GX2GetSwapStatus(&swaps,&flips,&flip_time,&vsync);
   if (swaps != flips || !ProcUIInForeground()) {
      OSReport("GX2 ReZ scan ticket or foreground unconfirmed\n");
      return false;
   }
   for (unsigned out = 0; out < OUTPUTS; ++out) {
      if (!begin_target(f,out)) return false;
      GX2SetPixelShader(f->pixel[arm]);
      if (clocks) cycle(f,out,0);
      if (!draw_case(f,out,level,order)) return false;
      if (clocks) cycle(f,out,1);
      GX2CopyColorBufferToScanBuffer(f->targets[out].color,scan[out]);
   }

   bool okay = true;
   if (!clocks) {
      for (unsigned out = 0; out < OUTPUTS; ++out) {
         okay &= verify_pixels(f,out,arm,level,order);
         // A failed stage may have outstanding writes to the same storage.
         if (f->stage.copy_pending) return false;
      }
   }
   const OSTime prior = GX2GetLastSubmittedTimeStamp();
   WHBGfxFinishRender();
   const OSTime submitted = GX2GetLastSubmittedTimeStamp();
   const OSTime retired = GX2GetRetiredTimeStamp();
   stage_observe(&f->stage,submitted,retired);
   if (submitted <= prior || submitted <= 0 || retired < submitted) {
      OSReport("GX2 ReZ pair retirement unverified\n");
      return false;
   }
   f->pair_open = false;
   if (clocks) for (unsigned out = 0; out < OUTPUTS; ++out)
      clocks[out] = elapsed_cycles(f,out);
   return okay;
}

static bool correctness(Fixture *f)
{
   for (unsigned order = 0; order < ORDERS; ++order)
      for (unsigned l = 0; l < LEVELS; ++l) {
         if (!render_pair(f,STOCK,l,order,NULL) || !render_pair(f,REZ,l,order,NULL)) return false;
      }
   /* Fresh baseline from the same stream isolates overdraw and seed differences. */
   return render_pair(f,STOCK,0,0,NULL) && render_pair(f,WRONG_KILL,0,0,NULL);
}

static bool measure(Fixture *f)
{
   uint64_t warm[OUTPUTS];
   if (!render_pair(f,STOCK,0,0,warm)) return false;
   if (!warm[0] || !warm[1]) {
      OSReport("GX2 ReZ timing unavailable tv=%u drc=%u\n",warm[0] != 0,warm[1] != 0);
      return true;
   }
   bool all = true;
   for (unsigned order = 0; order < ORDERS; ++order)
      for (unsigned l = 0; l < LEVELS; ++l)
         for (unsigned repetition = 0; repetition < ROUNDS; ++repetition) {
            const unsigned a = (order + l + repetition)%2;
            for (unsigned turn = 0; turn < 2; ++turn) {
               const unsigned arm = (a + turn)%2;
               uint64_t *values = timings[order][l][repetition][arm];
               if (!render_pair(f,arm,l,order,values)) return false;
               all &= values[0] && values[1];
            }
         }
   for (unsigned order = 0; order < ORDERS; ++order)
      for (unsigned l = 0; l < LEVELS; ++l)
         for (unsigned rep = 0; rep < ROUNDS; ++rep)
            for (unsigned out = 0; out < OUTPUTS; ++out)
               OSReport("GX2 ReZ cycles hz=27000000 out=%u far=%u order=%u rep=%u stock=%llu rez=%llu\n",
                        out,levels[l],order,rep,
                        (unsigned long long)timings[order][l][rep][STOCK][out],
                        (unsigned long long)timings[order][l][rep][REZ][out]);
   OSReport("GX2 ReZ timing complete=%u\n",(unsigned)all);
   return true;
}

int main(void)
{
   WHBProcInit();
   const bool graphics = WHBGfxInit();
   Fixture *f = &fixture;
   bool ok = graphics && distinct_targets(f) && load_pixels(f) && build_mesh(f) && buffers(f);
   OSReport("GX2 ReZ descriptor and targets=%s\n",ok ? "PASS" : "FAIL");
   if (ok) {
      OSReport("GX2 ReZ parity stage start\n");
      ok = correctness(f);
      OSReport("GX2 ReZ parity stage result=%s\n",ok ? "PASS" : "FAIL");
   }
   if (ok) {
      OSReport("GX2 ReZ timing stage start\n");
      ok = measure(f);
   }
   if (graphics) retire_for_teardown(&f->stage,"GX2 ReZ fixture");
   for (unsigned i = 0; i < OUTPUTS; ++i) {
      free(f->reference[i]);
      if (f->targets[i].owns_depth)
         GX2RDestroySurfaceEx(&f->targets[i].depth.surface,depth_flags);
   }
   if (f->rgba) MEMFreeToDefaultHeap(f->rgba);
   if (f->stage.storage) MEMFreeToDefaultHeap(f->stage.storage);
   if (f->quads) horizon_gx2_terrain_section_destroy(f->quads);
   if (f->terrain) horizon_gx2_terrain_destroy(f->terrain);
   for (unsigned arm = 0; arm < 3; ++arm)
      if (f->pixel[arm]) WHBGfxFreePixelShader(f->pixel[arm]);
   if (f->clear) WHBGfxFreePixelShader(f->clear);
   if (graphics) WHBGfxShutdown();
   WHBProcShutdown();
   OSReport("GX2 ReZ fixture result=%s\n",ok ? "PASS" : "FAIL");
   return ok ? 0 : 1;
}
