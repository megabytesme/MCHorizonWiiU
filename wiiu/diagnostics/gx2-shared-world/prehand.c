#include "world_scale.h"
#include "hud.h"
#include "terrain.h"
#include <coreinit/cache.h>
#include <coreinit/debug.h>
#include <coreinit/memdefaultheap.h>
#include <gx2/draw.h>
#include <gx2/event.h>
#include <gx2/mem.h>
#include <gx2/registers.h>
#include <gx2/shaders.h>
#include <gx2/state.h>
#include <gx2/surface.h>
#include <gx2/utils.h>
#include <stdbool.h>
#include <math.h>
#include <stdint.h>
#include <string.h>
#include <whb/gfx.h>
#include <whb/proc.h>

#include "../gx2-explicit-readback/stage.h"

extern const uint8_t _binary_hud_gsh_start[];

typedef struct HorizonGx2Outline HorizonGx2Outline;
typedef struct {
   float clip[4], fog_distance[2];
   uint8_t rgba[4];
} OutlineVertex;

int horizon_gx2_outline_create(HorizonGx2Outline **out, uint32_t capacity);
void horizon_gx2_outline_destroy(HorizonGx2Outline *outline);
int horizon_gx2_outline_draw(HorizonGx2Outline *outline, uint32_t target, const uint32_t rect[4],
                             const OutlineVertex *vertices, uint32_t count, uint32_t secondary_count,
                             const float fog_color[4], const float fog_ranges[4]);

extern const uint8_t _binary_terrain_gsh_start[];

typedef struct HorizonGx2Hand HorizonGx2Hand;
typedef struct {
   float position_light[4], uv[2];
   uint8_t color[4];
   int8_t normal[4];
} HandVertex;
_Static_assert(sizeof(HandVertex) == 32, "Incorrect GX2 hand ABI");
int horizon_gx2_hand_create(HorizonGx2Hand **out);
void horizon_gx2_hand_destroy(HorizonGx2Hand *hand);
void horizon_gx2_hand_begin_frame(HorizonGx2Hand *hand, bool share_uploads);
int horizon_gx2_hand_draw(HorizonGx2Hand *hand, uint32_t target, const uint32_t rect[4],
                          const GX2Texture *atlas, HorizonGx2Terrain *terrain,
                          const HandVertex *vertices, uint32_t count, const float clip_rows[16],
                          const float fog_color[4], const float fog_ranges[4], uint32_t nether);

static bool inside_pair;
static unsigned between_pass_draw_done;
BOOL __real_GX2DrawDone(void);
BOOL __wrap_GX2DrawDone(void)
{
   if (inside_pair) ++between_pass_draw_done;
   return __real_GX2DrawDone();
}

static bool scene(HorizonGx2Outline *geometry, uint32_t output, const uint32_t world[4],
                   const uint32_t viewport[4])
{
   const float aspect = (float)viewport[2] / (float)viewport[3];
   const float fog[4] = {0,0,0,0}, ranges[4] = {1,2,1,2};
   const uint8_t rgba[][4] = {{248,177,33,255}, {23,71,209,255}, {125,27,147,255},
                               {231,121,61,255}, {100,220,170,255}};
   const float spans[][2] = {{-0.06f,0.42f}, {-3.1f,-0.95f}, {-0.95f,-0.125f}, {-0.125f,0.63f}, {0.63f,3.1f}};
   const int corners[6][2] = {{0,-1},{1,-1},{1,1},{0,-1},{1,1},{0,1}};
   OutlineVertex vertices[30];
   for (uint32_t band = 0; band < 5; ++band) {
      for (uint32_t i = 0; i < 6; ++i) {
         OutlineVertex *v = &vertices[band*6+i];
         const float y = corners[i][1] * (band ? 1.0f : 0.45f);
         *v = (OutlineVertex){.clip = {spans[band][corners[i][0]]/aspect, y, band ? 0.75f : 0.375f, 1}};
         memcpy(v->rgba, rgba[band], 4);
      }
   }
   return horizon_gx2_outline_draw(geometry, output, world, vertices, 30, 0, fog, ranges) == 0;
}

typedef struct {
   float x[2], y[2], depth, sample_u;
   uint8_t rgba[4];
} HandQuad;

static const HandQuad hands[] = {
   {{ 0.05f,  0.47f}, {-0.22f,  0.25f}, 0.16f, 0.125f, {213, 35, 44,255}},
   {{ 0.25f,  0.70f}, {-0.40f,  0.12f}, 0.27f, 0.125f, {30, 78,223,255}},
   {{-0.90f, -0.50f}, {-0.20f,  0.20f}, 0.12f, 0.875f, { 0,255,255,255}},
   {{-0.58f, -0.33f}, { 0.28f,  0.65f}, 0.25f, 0.375f, {34,201, 91,255}},
   // TV-only marker covers ordinary shared world color and no shared edge.
   {{-0.42f, -0.19f}, {-0.70f, -0.45f}, 0.21f, 0.125f, {255,0,255,255}},
};

static bool draw_hand(HorizonGx2Hand *hand, HorizonGx2Terrain *terrain,
                      const GX2Texture *atlas, uint32_t target,
                      const uint32_t world[4], const uint32_t viewport[4])
{
   const float aspect = (float)viewport[2] / viewport[3];
   const float fog[4] = {0,0,0,0}, ranges[4] = {1,2,1,2};
   const float focal = 1.0f / tanf(35.0f * 0.017453292519943295f);
   const float near = 0.05f, far = 384.0f;
   const float clip[16] = {focal/aspect,0,0,0, 0,focal,0,0,
      0,0,far/(near-far),far*near/(near-far), 0,0,-1,0};
   static const unsigned ends[6][2] = {{0,0},{1,0},{1,1},{0,0},{1,1},{0,1}};
   HandVertex vertices[sizeof(hands)/sizeof(*hands)*6];
   const unsigned quads = sizeof(hands)/sizeof(*hands) - (target == 1);
   for (unsigned q = 0; q < quads; ++q) {
      const HandQuad *shape = &hands[q];
      for (unsigned i = 0; i < 6; ++i) {
         HandVertex *v = &vertices[q*6+i];
         *v = (HandVertex){
            .position_light = {
                shape->x[ends[i][0]] * shape->depth / focal,
                shape->y[ends[i][1]] * shape->depth / focal,
                -shape->depth, 0xF0F0,
            },
            .uv = {shape->sample_u, 0.5f},
         };
         memcpy(v->color, shape->rgba, 4);
      }
   }
   return horizon_gx2_hand_draw(hand,target,world,atlas,terrain,vertices,
                                quads*6,clip,fog,ranges,0) == 0;
}

typedef struct {
   HorizonHudVertex vertices[6];
   HorizonHudRun run;
   HorizonHudFrame frame;
} HudUpload;

static void build_hud(HudUpload *hud, uint32_t target, const uint32_t rect[4])
{
   const float x = target ? 14 : 36, y = target ? 18 : 31;
   const float corners[6][2] = {{x,y},{x+19,y},{x+19,y+14},{x,y},{x+19,y+14},{x,y+14}};
   for (uint32_t i = 0; i < 6; ++i) {
      memcpy(hud->vertices[i].pos, corners[i], sizeof(hud->vertices[i].pos));
      hud->vertices[i].uv[0] = hud->vertices[i].uv[1] = 0.5f;
      hud->vertices[i].color = target ? 0xffefdf08 : 0xff00ff00;
   }
   hud->run = (HorizonHudRun){0, 6, HORIZON_HUD_ALPHA, {0,0,0,0}};
   hud->frame = (HorizonHudFrame){.vertices=hud->vertices, .vertex_count=6, .runs=&hud->run, .run_count=1,
      .viewport_width=rect[2], .viewport_height=rect[3], .scale=1};
}

static bool display_edge(uint32_t x, uint32_t y, const uint32_t view[4])
{
   if (x < view[0] || x >= view[0]+view[2] || y < view[1] || y >= view[1]+view[3])
      return false;
   const double aspect = (double)view[2] / view[3];
   const double front_y = view[3] * 0.5 * 0.45;
   const double center_x = view[0] + view[2] * 0.5, center_y = view[1] + view[3] * 0.5;
   const double scaled = view[2] * 0.5 / aspect;
   const double edges[] = {-0.95,-0.125,0.63};
   for (unsigned i=0;i<3;++i)
      if (fabs(x - (center_x + scaled * edges[i])) <= 3.0) return true;

   const double left = center_x + scaled * -0.06, right = center_x + scaled * 0.42;
   if (fabs(y - center_y) <= front_y + 3.0 && (fabs(x-left) <= 3.0 || fabs(x-right) <= 3.0))
      return true;
   return x >= left-3.0 && x <= right+3.0 && fabs(fabs(y-center_y)-front_y) <= 3.0;
}

static bool agrees_sample(const uint8_t *sample, const uint8_t expected[4], unsigned fuzz)
{
   for (unsigned i = 0; i < 4; ++i) {
      const int delta = (int)sample[i] - expected[i];
      if (delta < -(int)fuzz || delta > (int)fuzz) return false;
   }
   return true;
}

static const uint8_t *at(const uint8_t *pixels, uint32_t full_width, uint32_t x, uint32_t y)
{
   return pixels + ((size_t)y*full_width + x)*4;
}

static bool independent_checks(const uint8_t *pixels, const GX2ColorBuffer *color,
                               uint32_t output, const uint32_t view[4])
{
   const uint32_t width = color->surface.width;
   const double center_x = view[0] + view[2]*0.5, center_y = view[1] + view[3]*0.5;
   const double radius = view[3]*0.5;
   const double positions[][2] = {{0.35,0.015},{0.57,-0.28},{-0.68,0},{-0.47,0.43},{-0.30,-0.58}};
   const uint8_t colors[][4] = {
      {213,35,44,255}, {30,78,223,255}, {125,27,147,255}, {79,114,119,255}, {255,0,255,255},
   };
   const uint8_t unmarked[] = {125,27,147,255};
   const uint8_t hud[2][4] = {{0,255,0,255},{8,223,239,255}};
   const uint32_t hud_x = view[0] + (output ? 16 : 38), hud_y = view[1] + (output ? 20 : 33);
   const uint8_t *hud_sample = at(pixels,width,hud_x,hud_y);
   bool ok = agrees_sample(hud_sample,hud[output],0);
   if (!ok)
      OSReport("GX2 prehand HUD failure output=%u got=%u,%u,%u,%u want=%u,%u,%u,%u\n",
               output,hud_sample[0],hud_sample[1],hud_sample[2],hud_sample[3],
               hud[output][0],hud[output][1],hud[output][2],hud[output][3]);
   for (unsigned i = 0; i < sizeof(positions)/sizeof(*positions); ++i) {
      const uint32_t x = (uint32_t)(center_x + positions[i][0]*radius);
      const uint32_t y = (uint32_t)(center_y - positions[i][1]*view[3]*0.5);
      const uint8_t *expected = i == 4 && output == 1 ? unmarked : colors[i];
      const uint8_t *sample = at(pixels,width,x,y);
      const bool pass = agrees_sample(sample,expected,i==3 ? 1 : 0);
      if (!pass)
         OSReport("GX2 prehand sample failure output=%u point=%u got=%u,%u,%u,%u want=%u,%u,%u,%u\n",
                  output,i,sample[0],sample[1],sample[2],sample[3],
                  expected[0],expected[1],expected[2],expected[3]);
      ok &= pass;
   }
   if (view[0] > 0) {
      const uint8_t black[] = {0,0,0,255};
      ok &= agrees_sample(at(pixels,width,0,(uint32_t)center_y),black,0);
   }
   return ok;
}

static bool compare(Stage *stage, uint8_t *current, uint8_t *reference[2],
                    HorizonGx2WorldScale *scale, HorizonGx2Outline *geometry[2],
                    HorizonGx2Hud *hud[2], HorizonGx2Hand *hand, HorizonGx2Terrain *terrain,
                    const GX2Texture *hand_atlas, const uint32_t size[4], uint32_t view[2][4],
                    bool shared, bool compare_reference, bool narrow)
{
   const float clear[4] = {0.21f,0.095f,0.26f,1};
   bool ok = true, seeded = false;
   unsigned drawn = 0, hand_draws = 0;
   HudUpload uploads[2] = {0};
   const GX2ColorBuffer *colors[2];
   for (uint32_t i=0;i<2;++i) {
      build_hud(&uploads[i],i,view[i]);
      if (horizon_gx2_hud_upload_after_wait(hud[i],&uploads[i].frame) != HORIZON_HUD_OK)
         return false;
   }
   horizon_gx2_hand_begin_frame(hand,false);
   between_pass_draw_done = 0;
   inside_pair = true;
   WHBGfxBeginRender();
   for (uint32_t output = 0; output < 2; ++output) {
      if (output == 0) WHBGfxBeginRenderTV();
      else WHBGfxBeginRenderDRC();
      WHBGfxClearColor(0,0,0,1);
      const uint32_t world[4] = {0,0,size[output*2],size[output*2+1]};
      if (output == 1 && seeded) {
         horizon_gx2_world_scale_resume_seeded_gamepad(scale);
      } else {
         horizon_gx2_world_scale_begin(scale,output,clear);
         const bool world_ok = scene(geometry[output],output,world,view[output]);
         ++drawn;
         ok &= world_ok;
         if (shared && output == 0 && world_ok) {
            seeded = horizon_gx2_world_scale_seed_gamepad_before_hand(scale,view[1]);
            ok &= seeded;
         }
      }
      const bool hand_ok = draw_hand(hand,terrain,hand_atlas,output,world,view[output]);
      hand_draws += hand_ok;
      ok &= hand_ok;
      horizon_gx2_world_scale_end(scale,output,view[output],hand_ok);
      const uint32_t *rect = view[output];
      ok &= horizon_gx2_hud_draw_uploaded_to_output(
         hud[output], &uploads[output].frame, rect[0],rect[1],rect[2],rect[3],output) == HORIZON_HUD_OK;
      colors[output] = output == 0 ? WHBGfxGetTVColourBuffer() : WHBGfxGetDRCColourBuffer();
      if (output == 0) WHBGfxFinishRenderTV();
      else WHBGfxFinishRenderDRC();
   }
   inside_pair = false;
   WHBGfxFinishRender();
   ok &= between_pass_draw_done == 0;
   for (uint32_t output=0;output<2;++output) {
      const GX2ColorBuffer *color = colors[output];
      const enum ReadStatus status = stage_read(stage, color, current, RGBA_CAP);
      if (status != READ_OK) {
         OSReport("GX2 first person prehand narrow=%u shared=%u output=%u read_status=%u\n",
                  narrow, shared, output, (unsigned)status);
         return false;
      }
      const uint32_t w=color->surface.width,h=color->surface.height;
      const size_t pixels=(size_t)w*h;
      uint32_t different = 0, outside = 0, marker_pixels = 0;
      static const uint8_t magenta[] = {255,0,255,255};
      for (size_t i=0;i<pixels;++i) {
         marker_pixels += memcmp(current+i*4,magenta,4) == 0;
         if (shared && compare_reference && memcmp(current+i*4,reference[output]+i*4,4) != 0) {
            ++different;
            if (!display_edge(i%w,i/w,view[output])) {
               if (outside++ < 4)
                  OSReport("GX2 prehand outside edge output=%u x=%u y=%u got=%u,%u,%u,%u ref=%u,%u,%u,%u\n",
                           output,(uint32_t)(i%w),(uint32_t)(i/w),
                           current[i*4],current[i*4+1],current[i*4+2],current[i*4+3],
                           reference[output][i*4],reference[output][i*4+1],
                           reference[output][i*4+2],reference[output][i*4+3]);
            }
         }
      }
      const bool samples = independent_checks(current,color,output,view[output]);
      const bool marker = output == 0 ? marker_pixels > 50 : marker_pixels == 0;
      const bool passed = samples && marker && !outside && (output || !different);
      ok &= passed;
      if (!shared) memcpy(reference[output],current,pixels*4);
      OSReport("GX2 first person prehand narrow=%u shared=%u compare=%u output=%u different=%u outside=%u marker=%u samples=%u pass=%u\n",
               narrow,shared,compare_reference,output,different,outside,marker_pixels,samples,passed);
   }
   OSReport("GX2 first person prehand narrow=%u shared=%u seed=%u world_draws=%u hand_draws=%u midpair_drawdone=%u\n",
            narrow,shared,seeded,drawn,hand_draws,between_pass_draw_done);
   return ok && hand_draws == 2 && drawn == (shared ? 1u : 2u);
}

int main(void)
{
   WHBProcInit();
   const bool graphics=WHBGfxInit();
   Stage stage={0};
   uint8_t *rgba=NULL,*reference[2]={NULL,NULL};
   HorizonGx2Hud *hud[2]={NULL,NULL}, *hand_texels=NULL;
   HorizonGx2Outline *geometry[2]={NULL,NULL};
   HorizonGx2Hand *hand=NULL;
   HorizonGx2Terrain *terrain=NULL;
   const uint8_t white[4]={255,255,255,255};
   const uint8_t texels[16]={255,255,255,255, 255,255,255,128,
      255,255,255,255, 255,255,255,0};
   uint8_t lightmap[16*16*4];
   memset(lightmap,255,sizeof(lightmap));
   bool ok = graphics && horizon_gx2_outline_create(&geometry[0],30) == 0 &&
      horizon_gx2_outline_create(&geometry[1],30) == 0 &&
      horizon_gx2_hud_create(&hud[0],_binary_hud_gsh_start,1,1,white,sizeof(white),6) == 0 &&
      horizon_gx2_hud_create(&hud[1],_binary_hud_gsh_start,1,1,white,sizeof(white),6) == 0 &&
      horizon_gx2_hud_create(&hand_texels,_binary_hud_gsh_start,4,1,texels,sizeof(texels),6) == 0 &&
      horizon_gx2_hand_create(&hand) == 0 &&
      horizon_gx2_terrain_create(&terrain,_binary_terrain_gsh_start,1,1,white,sizeof(white)) == 0 &&
      horizon_gx2_terrain_lightmap(terrain,lightmap) == 0;
   const GX2ColorBuffer *tv = graphics ? WHBGfxGetTVColourBuffer() : NULL;
   const GX2ColorBuffer *drc = graphics ? WHBGfxGetDRCColourBuffer() : NULL;
   const uint32_t tv_width = tv ? tv->surface.width : 0, tv_height = tv ? tv->surface.height : 0;
   ok &= tv && drc && ((tv_width == 1920 && tv_height == 1080) ||
                      (tv_width == 1280 && tv_height == 720)) &&
      drc->surface.width == 854 && drc->surface.height == 480;
   if (ok) {
      stage.storage=MEMAllocFromDefaultHeapEx(STAGE_CAP,STAGE_ALIGN);
      rgba=MEMAllocFromDefaultHeap(RGBA_CAP);
      reference[0]=MEMAllocFromDefaultHeap((size_t)tv_width*tv_height*4);
      reference[1]=MEMAllocFromDefaultHeap(854*480*4);
      ok=stage.storage && rgba && reference[0] && reference[1];
   }
   OSReport("GX2 first person prehand init=%u tv=%ux%u drc=%ux%u\n",
            ok, tv_width, tv_height, drc ? drc->surface.width : 0, drc ? drc->surface.height : 0);
   bool retired_all = true;
   for (unsigned shape=0;ok && shape<2;++shape) {
      const uint32_t width = shape ? 714 : 854;
      uint32_t view[2][4]={{0,0,tv_width,tv_height-1},{(854-width)/2,0,width,480}};
      const uint32_t sizes[4]={(tv_width*75+50)/100,((tv_height-1)*75+50)/100,
         (width*75+50)/100,(480*75+50)/100};
      uint32_t world_bytes;
      HorizonGx2WorldScale *scale = horizon_gx2_world_scale_create(sizes,&world_bytes);
      const GX2Texture *atlas = horizon_gx2_hud_atlas(hand_texels);
      ok = scale && atlas && horizon_gx2_world_scale_prepare_shared(scale,view[0],view[1]);
      // Exercise the first GamePad use without priming it with the independent arm.
      if (ok && shape == 0)
         ok = compare(&stage,rgba,reference,scale,geometry,hud,hand,terrain,atlas,sizes,view,true,false,shape);
      if (ok)
         ok = compare(&stage,rgba,reference,scale,geometry,hud,hand,terrain,atlas,sizes,view,false,false,shape) &&
              compare(&stage,rgba,reference,scale,geometry,hud,hand,terrain,atlas,sizes,view,true,true,shape);
      OSTime submitted,retired;
      retired_all = !stage.copy_pending && finish_commands(&submitted,&retired);
      if (retired_all) horizon_gx2_world_scale_destroy(scale);
      else ok=false;
   }
   if (graphics && retired_all) {
      OSTime submitted, retired;
      retired_all = !stage.copy_pending && finish_commands(&submitted, &retired);
      ok &= retired_all;
   }
   if (retired_all) {
      if (stage.storage) MEMFreeToDefaultHeap(stage.storage);
      if (rgba) MEMFreeToDefaultHeap(rgba);
      for (uint32_t i=0;i<2;++i) {
         if (reference[i]) MEMFreeToDefaultHeap(reference[i]);
         horizon_gx2_outline_destroy(geometry[i]);
         horizon_gx2_hud_destroy(hud[i]);
      }
      horizon_gx2_hand_destroy(hand);
      horizon_gx2_terrain_destroy(terrain);
      horizon_gx2_hud_destroy(hand_texels);
      if (graphics) WHBGfxShutdown();
      WHBProcShutdown();
   }
   OSReport("GX2 first person prehand fixture %s\n",ok ? "PASS" : "FAIL");
   return ok ? 0 : 1;
}
