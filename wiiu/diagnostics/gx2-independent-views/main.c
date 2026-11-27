#include "scene.h"
#include "../gx2-explicit-readback/stage.h"
#include "hud.h"
#include "loading_host.h"
#include "terrain.h"
#include <coreinit/memdefaultheap.h>
#include <coreinit/thread.h>
#include <gx2/registers.h>
#include <whb/gfx.h>
#include <whb/proc.h>

enum { MAX_SECTION_BYTES = 48 * 1024 * 1024 };
#define QUADS_PER_TILE (TILE_SIDE * TILE_SIDE)
#define WITNESS_QUADS (SEATS + 1)

typedef struct {
   HorizonHudVertex vertices[6];
   HorizonHudRun run;
   HorizonHudFrame frame;
} Marker;

typedef struct {
   HorizonGx2Terrain *terrain;
   HorizonGx2TerrainSection *sections[SEATS][SECTIONS];
   HorizonGx2Hud *hud[SEATS];
   Marker marker[SEATS];
   Stage stage;
   uint8_t *rgba;
   unsigned sites, tv_views, pose;
   unsigned draws, done, masks[SEATS];
   bool distant;
   enum Fault fault;
} Fixture;

static void quad(HorizonTerrainVertex *v, float x, float y, float w, float h, float z,
                 const uint8_t color[4])
{
   const float corners[4][2] = {{x,y},{x+w,y},{x+w,y+h},{x,y+h}};
   for (unsigned i = 0; i < 4; ++i)
      v[i] = (HorizonTerrainVertex){.position = {corners[i][0], corners[i][1], z, 0.0f},
         .uv = {0.5f, 0.5f}, .color_cutout = {color[0], color[1], color[2], 0}};
}

static int fill(void *context, HorizonTerrainVertex *vertices, uint32_t count)
{
   const unsigned index = *(const unsigned *)context;
   if (count == 4 * WITNESS_QUADS) {
      for (unsigned i = 0; i < SEATS; ++i)
         quad(vertices + i*4, (float)stripe_x(i) - 0.5f, 6.8f, 1, 2.4f, 4, stripe_color(i, index));
      // Draw farther last. The nearer witnesses only survive if depth writes and tests work.
      quad(vertices + SEATS*4, -64, -32, 128, 80, 0, backdrop);
      return 0;
   }
   if (count != 4 * QUADS_PER_TILE) return -1;
   for (unsigned y = 0; y < TILE_SIDE; ++y)
      for (unsigned x = 0; x < TILE_SIDE; ++x) {
         const uint8_t shade = (x + y + index) & 1 ? 112 : 144;
         const uint8_t color[4] = {shade, shade, shade, 255};
         quad(vertices + (y*TILE_SIDE+x)*4, x*0.8f, y*0.8f, 0.8f, 0.8f, 0, color);
      }
   return 0;
}

static void drop_sections(Fixture *f)
{
   for (unsigned i = 0; i < f->sites; ++i)
      for (unsigned j = 0; j < SECTIONS; ++j) {
         horizon_gx2_terrain_section_destroy_after_wait(f->sections[i][j]);
         f->sections[i][j] = NULL;
      }
   f->sites = 0;
}

static bool load_sections(Fixture *f, unsigned sites)
{
   uint32_t witness_bytes, tile_bytes;
   if (!sites || sites > SEATS || f->sites ||
       horizon_gx2_terrain_section_estimate(4*WITNESS_QUADS, &witness_bytes) ||
       horizon_gx2_terrain_section_estimate(4*QUADS_PER_TILE, &tile_bytes))
      return false;
   const uint64_t expected = sites * ((uint64_t)witness_bytes + (SECTIONS-1) * (uint64_t)tile_bytes);
   if (expected > MAX_SECTION_BYTES) return false;
   f->sites = sites;
   unsigned allocations = 0;
   uint64_t bytes = 0;
   for (unsigned i = 0; i < sites; ++i) {
      for (unsigned j = 0; j < SECTIONS; ++j) {
         uint32_t actual;
         const uint32_t vertices = 4 * (j ? QUADS_PER_TILE : WITNESS_QUADS);
         const uint32_t estimate = j ? tile_bytes : witness_bytes;
         if (horizon_gx2_terrain_section_create(&f->sections[i][j], vertices, fill, &i, &actual) ||
             actual != estimate || !f->sections[i][j])
            return false;
         ++allocations;
         bytes += actual;
      }
   }
   OSReport("GX2 independent views resident sites=%u sections=%u allocation_bytes=%u cap=%u\n",
            sites, allocations, (unsigned)bytes, MAX_SECTION_BYTES);
   return bytes == expected;
}

static void build_marker(Marker *m, Rect r, unsigned seat, unsigned pose)
{
   const float x = marker_x(seat), y = marker_y(pose);
   const float pos[6][2] = {{x,y},{x+9,y},{x+9,y+9},{x,y},{x+9,y+9},{x,y+9}};
   const uint8_t *color = palette[SEATS-1-seat];
   const uint32_t abgr = (uint32_t)color[3] << 24 | (uint32_t)color[2] << 16 |
      (uint32_t)color[1] << 8 | color[0];
   for (unsigned i = 0; i < 6; ++i)
      m->vertices[i] = (HorizonHudVertex){.pos = {pos[i][0],pos[i][1]},
         .uv = {0.5f,0.5f}, .color = abgr};
   m->run = (HorizonHudRun){0,6,HORIZON_HUD_ALPHA,{0}};
   m->frame = (HorizonHudFrame){.vertices = m->vertices, .vertex_count = 6, .runs = &m->run,
      .run_count = 1, .viewport_width = r.w, .viewport_height = r.h, .scale = 1};
}

static int draw_target(void *context, uint32_t output, uint32_t w, uint32_t h)
{
   Fixture *f = context;
   const unsigned first = output ? SEATS-1 : 0;
   const unsigned stop = output ? SEATS : f->tv_views;
   const float ranges[4] = {9,11,9,11};
   for (unsigned seat = first; seat < stop; ++seat) {
      const Rect r = viewport(w, h, f->tv_views, output, seat);
      const Rect world = world_viewport(r, seat, f->fault);
      const uint8_t *rgb = row_color(seat);
      const float fog[4] = {rgb[0]/255.0f, rgb[1]/255.0f, rgb[2]/255.0f, 1};
      if (horizon_gx2_terrain_bind_variant(f->terrain, w, h, world.x, world.y, world.w, world.h,
                                           fog, ranges, false, f->fault != GENERAL_SHADER))
         return -1;
      if (f->fault != NO_SCISSOR)
         GX2SetScissor(world.x+GAP, world.y, world.w-2*GAP, world.h);
      if (f->fault == NO_DEPTH)
         GX2SetDepthOnlyControl(FALSE, FALSE, GX2_COMPARE_FUNC_ALWAYS);

      const unsigned index = site(seat, f->distant);
      const unsigned owner = f->fault == WRONG_SITE && output ? (index+1)%SEATS : index;
      const unsigned camera = f->fault == REUSE_CAMERA ? 0 : seat;
      if (owner >= f->sites) return -2;
      // Geometry comes first, then the proof draws. All streams are immutable for the pair.
      for (unsigned batch = 1; batch <= SECTIONS; ++batch) {
         const unsigned j = batch % SECTIONS;
         _Alignas(16) float clip[16], origin[4];
         section_projection(world, index, camera, f->pose, j, clip, origin);
         if (horizon_gx2_terrain_draw_bound(f->terrain, f->sections[owner][j], clip, origin))
            return -3;
         ++f->draws;
      }
      if (horizon_gx2_hud_draw_uploaded_to_output(f->hud[seat], &f->marker[seat].frame,
                                                 r.x, r.y, r.w, r.h, output) != HORIZON_HUD_OK)
         return -4;
      f->done |= 1u << seat;
   }
   return 0;
}

static unsigned check_view(const uint8_t *rgba, uint32_t w, Rect r, unsigned seat,
                           unsigned pose, bool distant)
{
   Sample expected[CHECKS];
   samples(r, seat, pose, distant, expected);
   unsigned wrong = 0;
   for (unsigned i = 0; i < CHECKS; ++i) {
      const Sample s = expected[i];
      if (memcmp(rgba + ((size_t)s.y*w+s.x)*4, s.color, 4)) wrong |= 1u << i;
   }
   const uint8_t *right = rgba + ((size_t)(r.y+r.h/2)*w+r.x+r.w-2)*4;
   if (memcmp(right, clear, 4)) wrong |= 1u << EDGE;
   const uint8_t *bottom = rgba + ((size_t)(r.y+r.h-2)*w+r.x+r.w/2)*4;
   if (memcmp(bottom, row_color(seat), 4)) wrong |= 1u << SEAM;
   return wrong;
}

static uint32_t pixel_word(const uint8_t *rgba, uint32_t width, uint32_t x, uint32_t y)
{
   const uint8_t *p = rgba + ((size_t)y*width+x)*4;
   return (uint32_t)p[0] << 24 | (uint32_t)p[1] << 16 | (uint32_t)p[2] << 8 | p[3];
}

static bool center_crop_rejected(const uint8_t *tv, const uint32_t dims[4])
{
   const double scale = fmin((double)dims[0]/dims[2], (double)dims[1]/dims[3]);
   const double left = (dims[0] - dims[2]*scale) * 0.5;
   const double top = (dims[1] - dims[3]*scale) * 0.5;
   Sample expected[CHECKS];
   samples((Rect){0,0,dims[2],dims[3]}, SEATS-1, 0, true, expected);
   unsigned misses = 0;
   for (unsigned i = CENTER; i <= STRIPE; ++i) {
      const uint32_t x = left + (expected[i].x + 0.5)*scale;
      const uint32_t y = top + (expected[i].y + 0.5)*scale;
      misses += memcmp(tv + ((size_t)y*dims[0]+x)*4, expected[i].color, 4) != 0;
   }
   OSReport("GX2 independent views final-TV crop terrain failures=%u/2\n", misses);
   return misses == 2;
}

static bool render(Fixture *f, unsigned tv_views, unsigned pose, enum Fault fault)
{
   uint32_t dims[4];
   if (horizon_loading_host_dimensions(dims)) return false;
   f->tv_views = tv_views;
   f->pose = pose;
   f->fault = fault;
   f->done = f->draws = 0;
   memset(f->masks, 0, sizeof(f->masks));
   for (unsigned view = 0; view <= tv_views; ++view) {
      const unsigned output = view == tv_views;
      const unsigned seat = output ? SEATS-1 : view;
      const Rect r = viewport(dims[output*2], dims[output*2+1], tv_views, output, seat);
      if (r.w < 100 || r.h < 80 || dims[output*2] > 1920 || dims[output*2+1] > 1080) return false;
      build_marker(&f->marker[seat], r, seat, pose);
      if (horizon_gx2_hud_upload_after_wait(f->hud[seat], &f->marker[seat].frame) != HORIZON_HUD_OK)
         return false;
   }
   const unsigned before = horizon_loading_host_completed_presents();
   const unsigned draws = (tv_views+1) * SECTIONS;
   const unsigned mask = ((1u << tv_views) - 1) | (1u << (SEATS-1));
   const int status = horizon_loading_host_present(draw_target, f);
   if (status || horizon_loading_host_last_outputs() != 3 || f->draws != draws ||
       f->done != mask || horizon_loading_host_completed_presents() != before+1) {
      OSReport("GX2 independent views pair status=%d outputs=%u views=%x draws=%u/%u\n",
               status, horizon_loading_host_last_outputs(), f->done, f->draws, draws);
      return false;
   }

   const GX2ColorBuffer *target[] = {WHBGfxGetTVColourBuffer(), WHBGfxGetDRCColourBuffer()};
   unsigned bad[CHECKS] = {0};
   bool exact_masks = true;
   for (unsigned output = 0; output < 2; ++output) {
      const enum ReadStatus read = stage_read(&f->stage, target[output], f->rgba, RGBA_CAP);
      if (read != READ_OK) {
         OSReport("GX2 independent views staged readback failed output=%u status=%u\n", output, read);
         return false;
      }
      const unsigned first = output ? SEATS-1 : 0;
      const unsigned end = output ? SEATS : tv_views;
      for (unsigned seat = first; seat < end; ++seat) {
         const uint32_t w = dims[2*output], h = dims[2*output+1];
         const Rect r = viewport(w, h, tv_views, output, seat);
         const unsigned wrong = check_view(f->rgba, w, r, seat, pose, f->distant);
         f->masks[seat] = wrong;
         const unsigned expected = expected_mask(fault, seat);
         exact_masks &= wrong == expected;
         if ((wrong ^ expected) & (1u << EDGE))
            OSReport("GX2 independent views unexpected edge seat=%u fault=%u left=%08x right=%08x\n",
                     seat, fault, pixel_word(f->rgba, w, r.x+1, r.y+r.h/2),
                     pixel_word(f->rgba, w, r.x+r.w-2, r.y+r.h/2));
         if ((wrong ^ expected) & (1u << SEAM))
            OSReport("GX2 independent views unexpected seam seat=%u fault=%u top=%08x bottom=%08x\n",
                     seat, fault, pixel_word(f->rgba, w, r.x+r.w/2, r.y+1),
                     pixel_word(f->rgba, w, r.x+r.w/2, r.y+r.h-2));
         for (unsigned i = 0; i < CHECKS; ++i) bad[i] += (wrong >> i) & 1;
      }
      if (!output && f->distant && !pose && fault == GOOD)
         exact_masks &= center_crop_rejected(f->rgba, dims);
   }
   OSReport("GX2 independent views layout=%u+1 distant=%u pose=%u fault=%u outputs=3 views=%x draws=%u indices=%u failures=%u,%u,%u,%u seam_failures=%u masks=%x,%x,%x,%x,%x pass=%u\n",
            tv_views, f->distant, pose, fault, f->done, f->draws,
            (tv_views+1) * ((SECTIONS-1)*QUADS_PER_TILE+WITNESS_QUADS)*6,
            bad[CENTER], bad[STRIPE], bad[EDGE], bad[HUD], bad[SEAM],
            f->masks[0], f->masks[1], f->masks[2], f->masks[3], f->masks[4], exact_masks);
   return exact_masks;
}

int main(void)
{
   static Fixture f;
   WHBProcInit();
   const bool graphics = WHBGfxInit();
   const uint8_t white[4] = {255,255,255,255};
   uint8_t lightmap[16*16*4];
   memset(lightmap, 255, sizeof(lightmap));
   size_t terrain_gfd_size, hud_gfd_size;
   const void *terrain_gfd = horizon_loading_host_terrain_gfd(&terrain_gfd_size);
   const void *hud_gfd = horizon_loading_host_hud_gfd(&hud_gfd_size);
   bool ok = graphics && terrain_gfd_size && hud_gfd_size &&
      horizon_gx2_terrain_create(&f.terrain, terrain_gfd, 1, 1, white, 4) == 0 &&
      horizon_gx2_terrain_load_opaque_shader(f.terrain, horizon_loading_host_opaque_terrain_gfd()) &&
      horizon_gx2_terrain_lightmap(f.terrain, lightmap) == 0;
   for (unsigned i = 0; ok && i < SEATS; ++i)
      ok = horizon_gx2_hud_create(&f.hud[i], hud_gfd, 1, 1, white, 4, 6) == HORIZON_HUD_OK;
   if (ok) {
      f.stage.storage = MEMAllocFromDefaultHeapEx(STAGE_CAP, STAGE_ALIGN);
      f.rgba = MEMAllocFromDefaultHeap(RGBA_CAP);
      ok = f.stage.storage && f.rgba;
      OSReport("GX2 independent views readback_cap=%u shared_index_bytes=%u\n",
               (unsigned)(STAGE_CAP+RGBA_CAP), horizon_gx2_terrain_index_allocation_bytes(f.terrain));
   }
   const unsigned layouts[] = {1,2,4};
   for (unsigned d = 0; ok && d < 2; ++d) {
      f.distant = d;
      ok = load_sections(&f, d ? SEATS : 1);
      for (unsigned l = 0; ok && l < sizeof(layouts)/sizeof(layouts[0]); ++l)
         for (unsigned p = 0; ok && p < 2; ++p)
            ok = render(&f, layouts[l], p, GOOD);
      if (!d && ok) {
         OSTime submitted, retired;
         ok = !f.stage.copy_pending && finish_commands(&submitted, &retired);
         if (ok) drop_sections(&f);
      }
   }

   // Run the independent positive matrix before introducing corrupted pixels.
   // Check the cross-view arm from a clean pair; leave the no-scissor arm last.
   if (ok) {
      ok = render(&f, 4, 1, GENERAL_SHADER);
      const enum Fault faults[] = {CROSS_VIEW, REUSE_CAMERA, NO_DEPTH, WRONG_SITE, NO_SCISSOR};
      for (unsigned i = 0; i < sizeof(faults)/sizeof(faults[0]) && !f.stage.copy_pending; ++i)
         ok &= render(&f, 4, 0, faults[i]);
   }

   // A failed copy may still own this backing. Do not destroy anything while it is outstanding.
   OSTime submitted, retired;
   if (!f.stage.copy_pending && (!graphics || finish_commands(&submitted, &retired))) {
      if (ok) OSSleepTicks(OSMillisecondsToTicks(1300));
      if (f.stage.storage) MEMFreeToDefaultHeap(f.stage.storage);
      if (f.rgba) MEMFreeToDefaultHeap(f.rgba);
      drop_sections(&f);
      for (unsigned i = 0; i < SEATS; ++i) horizon_gx2_hud_destroy(f.hud[i]);
      horizon_gx2_terrain_destroy(f.terrain);
      if (graphics) WHBGfxShutdown();
      WHBProcShutdown();
   } else ok = false;
   OSReport("GX2 independent views fixture %s\n", ok ? "PASS" : "FAIL");
   return ok ? 0 : 1;
}
