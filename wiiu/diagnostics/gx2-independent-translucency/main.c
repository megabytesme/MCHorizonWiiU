#include "scene.h"
#include "../gx2-explicit-readback/retire.h"
#include "loading_host.h"
#include "terrain.h"
#include <coreinit/memdefaultheap.h>
#include <coreinit/thread.h>
#include <gx2/registers.h>
#include <whb/gfx.h>
#include <whb/proc.h>

typedef struct {
   HorizonGx2Terrain *terrain;
   HorizonGx2TerrainSection *mesh, *background;
   Stage stage;
   uint8_t *rgba;
   enum Mode mode;
   unsigned fills, draws, backgrounds, views;
   unsigned submit_waits[VIEWS];
} Fixture;

static bool inside_pair;
static unsigned pair_waits;

BOOL __real_GX2DrawDone(void);
BOOL __wrap_GX2DrawDone(void)
{
   if (inside_pair) ++pair_waits;
   return __real_GX2DrawDone();
}

static int fill(void *context, HorizonTerrainVertex *vertices, uint32_t count)
{
   Fixture *f = context;
   ++f->fills;
   if (count != QUADS*4) return -1;
   const float corners[4][2] = {{-2,-2},{2,-2},{2,2},{-2,2}};
   for (unsigned q = 0; q < QUADS; ++q) {
      const bool red = q < 2;
      const bool reverse = q & 1;
      for (unsigned i = 0; i < 4; ++i) {
         const float *p = corners[reverse ? 3-i : i];
         vertices[q*4+i] = (HorizonTerrainVertex){
            .position = {p[0],p[1],red ? 1 : -1,0},
            .uv = {0.5f,0.5f},
            .color_cutout = {red ? 255 : 0,0,red ? 0 : 255,128},
         };
      }
   }
   return 0;
}

static int fill_background(void *context, HorizonTerrainVertex *vertices, uint32_t count)
{
   (void)context;
   if (count != 4) return -1;
   const float corners[4][2] = {{-2,-2},{2,-2},{2,2},{-2,2}};
   for (unsigned i = 0; i < 4; ++i)
      vertices[i] = (HorizonTerrainVertex){
         .position = {corners[i][0],corners[i][1],0.9999f,0},
         .uv = {0.5f,0.5f},
      };
   return 0;
}

static bool prepare(Fixture *f)
{
   const uint8_t white[4] = {255,255,255,255};
   uint8_t lightmap[16*16*4];
   memset(lightmap, 255, sizeof(lightmap));
   size_t gfd_length;
   const void *gfd = horizon_loading_host_terrain_gfd(&gfd_length);
   if (!gfd_length || horizon_gx2_terrain_create(&f->terrain,gfd,1,1,white,4) ||
       horizon_gx2_terrain_lightmap(f->terrain,lightmap)) return false;

   uint32_t vertex, one, two, actual;
   if (horizon_gx2_terrain_section_estimate(QUADS*4,&vertex) ||
       horizon_gx2_terrain_section_estimate_sorted(QUADS*4,&one) ||
       horizon_gx2_terrain_section_estimate_sorted_views(QUADS*4,VIEWS,&two) ||
       vertex >= one || (uint64_t)vertex + VIEWS*(uint64_t)(one-vertex) != two ||
       horizon_gx2_terrain_section_create_sorted_views(&f->mesh,QUADS*4,VIEWS,fill,f,&actual) ||
       actual != two || f->fills != 1) return false;

   uint32_t background_bytes;
   if (horizon_gx2_terrain_section_create(&f->background,4,fill_background,NULL,&background_bytes))
      return false;
   const uint32_t corners[] = {0,1,2,2,3,0};
   uint32_t orders[VIEWS][INDICES];
   for (unsigned lane = 0; lane < VIEWS; ++lane) {
      for (unsigned q = 0; q < QUADS; ++q) {
         // Red has the +Z slab. The +Z camera therefore draws blue first.
         const unsigned quad = lane ? q : (q+2)%QUADS;
         for (unsigned i = 0; i < 6; ++i)
            orders[lane][6*q+i] = 4*quad+corners[i];
      }
      if (horizon_gx2_terrain_section_update_indices_view(f->mesh,lane,orders[lane],INDICES))
         return false;
   }
   if (!horizon_gx2_terrain_section_update_indices_view(f->mesh,VIEWS,orders[0],INDICES))
      return false;

   f->stage.storage = MEMAllocFromDefaultHeapEx(STAGE_CAP,STAGE_ALIGN);
   f->rgba = MEMAllocFromDefaultHeap(RGBA_CAP);
   OSReport("GX2 independent blends vertices=%u vertex_owners=%u vertex_bytes=%u index_lanes=%u lane_bytes=%u aligned_mesh_bytes=%u background_bytes=%u readback_cap=%u\n",
            QUADS*4,f->fills,vertex,VIEWS,one-vertex,two,background_bytes,STAGE_CAP+RGBA_CAP);
   return f->stage.storage && f->rgba;
}

static int draw_target(void *context, uint32_t output, uint32_t w, uint32_t h)
{
   Fixture *f = context;
   if (output >= VIEWS || output != f->draws) return -1;
   const unsigned camera = view_camera(f->mode,output), lane = view_lane(f->mode,output);
   const float fog[4] = {0}, ranges[4] = {20,200,20,200};
   _Alignas(16) float clip[16], origin[4] = {0,0,camera ? 6 : -6,0};
   const _Alignas(16) float identity[16] = {1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1};
   const _Alignas(16) float zero[4] = {0};
   // An explicit GPU color/depth write avoids treating a previous output's
   // retained blend as this pair's clear value. Both outputs stay queued.
   if (horizon_gx2_terrain_bind(f->terrain,w,h,0,0,w,h,fog,ranges,false)) return -2;
   GX2SetDepthOnlyControl(TRUE,TRUE,GX2_COMPARE_FUNC_ALWAYS);
   if (horizon_gx2_terrain_draw_bound(f->terrain,f->background,identity,zero)) return -2;
   ++f->backgrounds;
   fixture_project(camera,(float)w/h,clip);
   // Stock translucent shader, depth test/write, front-face culling and alpha blend.
   if (horizon_gx2_terrain_bind(f->terrain,w,h,0,0,w,h,fog,ranges,true)) return -2;
   if (!horizon_gx2_terrain_draw_bound_view(f->terrain,f->mesh,VIEWS,clip,origin)) return -3;
   if (horizon_gx2_terrain_draw_bound_view(f->terrain,f->mesh,lane,clip,origin)) return -4;
   f->submit_waits[output] = pair_waits;
   ++f->draws;
   f->views |= 1u << output;
   return 0;
}

static const uint8_t *pixel(const uint8_t *rgba, uint32_t w, uint32_t x, uint32_t y)
{
   return rgba + ((size_t)y*w+x)*4;
}

static bool verify(Fixture *f, unsigned output, const GX2ColorBuffer *target, unsigned *mask)
{
   const enum ReadStatus read = stage_read(&f->stage,target,f->rgba,RGBA_CAP);
   if (read != READ_OK) {
      OSReport("GX2 independent blends staged readback output=%u status=%u\n",output,read);
      return false;
   }
   const uint32_t w = target->surface.width, h = target->surface.height;
   const unsigned camera = view_camera(f->mode,output), lane = view_lane(f->mode,output);
   const bool far_visible = !(fault_mask(f->mode) & (1u << output));
   const uint8_t *a = pixel(f->rgba,w,w/2,h/2);
   const uint8_t *b = pixel(f->rgba,w,w/2+h/15,h/2+h/15);
   const uint8_t *guard = pixel(f->rgba,w,w/16,h/2);
   static const uint8_t black[] = {0,0,0,255};
   if (!matches_pixel(a,camera,true) || !matches_pixel(b,camera,true)) *mask |= 1u << output;
   const bool ok = matches_pixel(a,camera,far_visible) && matches_pixel(b,camera,far_visible) &&
      memcmp(guard,black,4) == 0;
   OSReport("GX2 independent blends mode=%u output=%u camera=%u lane=%u size=%ux%u center=%u,%u,%u,%u offset=%u,%u,%u,%u guard=%u pass=%u\n",
            f->mode,output,camera,lane,w,h,a[0],a[1],a[2],a[3],b[0],b[1],b[2],b[3],
            memcmp(guard,black,4) == 0,ok);
   return ok;
}

static bool present(Fixture *f, enum Mode mode)
{
   f->mode = mode;
   f->draws = f->backgrounds = f->views = pair_waits = 0;
   const unsigned before = horizon_loading_host_completed_presents();
   inside_pair = true;
   const int status = horizon_loading_host_present(draw_target,f);
   inside_pair = false;
   // Readback happens after both native scan copies and the paired finish. Only
   // waits from this synchronous present count toward the overlap assertion.
   const bool both_queued = f->draws == VIEWS && f->backgrounds == VIEWS && f->views == 3 &&
      f->submit_waits[0] == f->submit_waits[1] && pair_waits > f->submit_waits[1];
   OSReport("GX2 independent blends pair mode=%u result=%d outputs=%u draws=%u background_draws=%u indices=%u waits=%u,%u,%u queued=%u\n",
            mode,status,horizon_loading_host_last_outputs(),f->draws,f->backgrounds,f->draws*INDICES,
            f->submit_waits[0],f->submit_waits[1],pair_waits,both_queued);
   if (status || !both_queued || horizon_loading_host_last_outputs() != 3 ||
       horizon_loading_host_completed_presents() != before+1) return false;

   unsigned observed = 0;
   bool ok = verify(f,0,WHBGfxGetTVColourBuffer(),&observed);
   if (ok) ok = verify(f,1,WHBGfxGetDRCColourBuffer(),&observed);
   ok &= observed == fault_mask(mode);
   OSReport("GX2 independent blends mode=%u mismatch_mask=%x expected=%x pass=%u\n",
            mode,observed,fault_mask(mode),ok);
   return ok;
}

int main(void)
{
   Fixture f = {0};
   WHBProcInit();
   const bool initialized = WHBGfxInit();
   bool ok = initialized && prepare(&f);
   if (ok) {
      const enum Mode modes[] = {INDEPENDENT,SWAP_CAMERAS,REUSE_TV_LANE,SWAP_LANES,INDEPENDENT};
      for (unsigned i = 0; ok && i < sizeof(modes)/sizeof(modes[0]); ++i)
         ok = present(&f,modes[i]);
   }
   if (initialized) retire_for_teardown(&f.stage, "GX2 independent blends");
   if (ok) OSSleepTicks(OSMillisecondsToTicks(1300));
   if (f.rgba) MEMFreeToDefaultHeap(f.rgba);
   if (f.stage.storage) MEMFreeToDefaultHeap(f.stage.storage);
   horizon_gx2_terrain_section_destroy_after_wait(f.mesh);
   horizon_gx2_terrain_section_destroy_after_wait(f.background);
   horizon_gx2_terrain_destroy(f.terrain);
   if (initialized) WHBGfxShutdown();
   WHBProcShutdown();
   OSReport("GX2 independent blends fixture %s\n",ok ? "PASS" : "FAIL");
   return ok ? 0 : 1;
}
