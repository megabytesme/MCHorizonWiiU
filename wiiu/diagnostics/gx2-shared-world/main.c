#include "world_scale.h"
#include "hud.h"
#include "../gx2-explicit-readback/stage.h"
#include <coreinit/memdefaultheap.h>
#include <math.h>
#include <whb/gfx.h>
#include <whb/proc.h>

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
      hud->vertices[i].color = 0xff00ff00;
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

static bool compare(Stage *stage, uint8_t *current, uint8_t *reference[2],
                    HorizonGx2WorldScale *scale, HorizonGx2Outline *geometry[2],
                    HorizonGx2Hud *hud[2], const uint32_t size[4], uint32_t view[2][4],
                    bool shared, bool wide)
{
   const float clear[4] = {0.21f,0.095f,0.26f,1};
   bool ok = true;
   unsigned drawn = 0;
   HudUpload uploads[2] = {0};
   const GX2ColorBuffer *colors[2];
   for (uint32_t i=0;i<2;++i) {
      build_hud(&uploads[i],i,view[i]);
      if (horizon_gx2_hud_upload_after_wait(hud[i],&uploads[i].frame) != HORIZON_HUD_OK)
         return false;
   }
   WHBGfxBeginRender();
   for (uint32_t output = 0; output < 2; ++output) {
      if (output == 0) WHBGfxBeginRenderTV();
      else WHBGfxBeginRenderDRC();
      WHBGfxClearColor(0,0,0,1);
      if (output == 1 && shared) {
         ok &= horizon_gx2_world_scale_composite_shared(scale, view[output]);
      } else {
         const uint32_t world[4] = {0,0,size[output*2],size[output*2+1]};
         horizon_gx2_world_scale_begin(scale,output,clear);
         ok &= scene(geometry[output],output,world,view[output]);
         horizon_gx2_world_scale_end(scale,output,view[output],true);
         ++drawn;
      }
      const uint32_t *rect = view[output];
      ok &= horizon_gx2_hud_draw_uploaded_to_output(
         hud[output], &uploads[output].frame, rect[0],rect[1],rect[2],rect[3],output) == HORIZON_HUD_OK;
      colors[output] = output == 0 ? WHBGfxGetTVColourBuffer() : WHBGfxGetDRCColourBuffer();
      if (output == 0) WHBGfxFinishRenderTV();
      else WHBGfxFinishRenderDRC();
   }
   WHBGfxFinishRender();
   for (uint32_t output=0;output<2;++output) {
      const GX2ColorBuffer *color = colors[output];
      if (stage_read(stage,color,current,RGBA_CAP) != READ_OK) return false;
      const uint32_t w=color->surface.width,h=color->surface.height;
      const size_t pixels=(size_t)w*h;
      if (!(output == 0 ? w==1920 && h==1080 : w==854 && h==480)) return false;
      if (!shared) memcpy(reference[output],current,pixels*4);
      else {
         uint32_t different = 0, outside = 0, wrong_hud = 0;
         uint32_t hud_x = view[output][0] + (output ? 16 : 38);
         uint32_t hud_y = view[output][1] + (output ? 20 : 33);
         for (size_t i=0;i<pixels;++i) {
            if (memcmp(current+i*4,reference[output]+i*4,4) != 0) {
               ++different;
               outside += !display_edge(i%w,i/w,view[output]);
            }
         }
         const size_t hud_point = ((size_t)hud_y*w+hud_x)*4;
         static const uint8_t green[4] = {0,255,0,255};
         wrong_hud += memcmp(current+hud_point,green,4) != 0;
         wrong_hud += memcmp(reference[output]+hud_point,green,4) != 0;
         // Verify the foreground plane independently of the reference comparison.
         const size_t center = ((size_t)(view[output][1]+view[output][3]/2)*w+
            view[output][0]+view[output][2]/2)*4;
         const uint8_t foreground[] = {248,177,33,255};
         const bool passed = !wrong_hud && !outside &&
            (output || !different) && memcmp(current+center,foreground,4) == 0;
         ok &= passed;
         OSReport("GX2 shared world wide=%u output=%u different=%u outside=%u pixels=%u wrong_hud=%u pass=%u\n",
                  wide,output,different,outside,(unsigned)pixels,wrong_hud,passed);
      }
   }
   OSReport("GX2 shared world wide=%u active=%u world_draws=%u\n",wide,shared,drawn);
   return ok && drawn == (shared ? 1u : 2u);
}

int main(void)
{
   WHBProcInit();
   const bool graphics=WHBGfxInit();
   Stage stage={0};
   uint8_t *rgba=NULL,*reference[2]={NULL,NULL};
   HorizonGx2Hud *hud[2]={NULL,NULL};
   HorizonGx2Outline *geometry[2]={NULL,NULL};
   const uint8_t white[4]={255,255,255,255};
   const uint32_t sizes[4]={1440,809,640,360};
   uint32_t world_bytes=0;
   HorizonGx2WorldScale *scale = graphics ? horizon_gx2_world_scale_create(sizes,&world_bytes) : NULL;
   bool ok = scale && horizon_gx2_outline_create(&geometry[0],30) == 0 &&
      horizon_gx2_outline_create(&geometry[1],30) == 0 &&
      horizon_gx2_hud_create(&hud[0],_binary_hud_gsh_start,1,1,white,sizeof(white),6) == 0 &&
      horizon_gx2_hud_create(&hud[1],_binary_hud_gsh_start,1,1,white,sizeof(white),6) == 0;
   if (ok) {
      stage.storage=MEMAllocFromDefaultHeapEx(STAGE_CAP,STAGE_ALIGN);
      rgba=MEMAllocFromDefaultHeap(RGBA_CAP);
      reference[0]=MEMAllocFromDefaultHeap(RGBA_CAP);
      reference[1]=MEMAllocFromDefaultHeap(854*480*4);
      ok=stage.storage && rgba && reference[0] && reference[1];
   }
   uint32_t viewports[2][4]={{0,0,1920,1079},{0,0,854,480}};
   for (unsigned shape=0;ok && shape<2;++shape) {
      if (shape) { viewports[1][0]=70; viewports[1][2]=714; }
      ok = horizon_gx2_world_scale_prepare_shared(scale,viewports[0],viewports[1]) &&
         !horizon_gx2_world_scale_composite_shared(scale,(uint32_t[]){0,0,32,32}) &&
         compare(&stage,rgba,reference,scale,geometry,hud,sizes,viewports,false,shape) &&
         compare(&stage,rgba,reference,scale,geometry,hud,sizes,viewports,true,shape);
   }
   if (ok) {
      const uint32_t too_narrow[4]={0,0,640,480};
      ok = !horizon_gx2_world_scale_prepare_shared(scale,too_narrow,viewports[1]) &&
         !horizon_gx2_world_scale_composite_shared(scale,viewports[1]);
   }
   OSTime submitted,retired;
   if (!stage.copy_pending && (!graphics || finish_commands(&submitted,&retired))) {
      if (stage.storage) MEMFreeToDefaultHeap(stage.storage);
      if (rgba) MEMFreeToDefaultHeap(rgba);
      for (uint32_t i=0;i<2;++i) {
         if (reference[i]) MEMFreeToDefaultHeap(reference[i]);
         horizon_gx2_outline_destroy(geometry[i]);
         horizon_gx2_hud_destroy(hud[i]);
      }
      horizon_gx2_world_scale_destroy(scale);
      if (graphics) WHBGfxShutdown();
      WHBProcShutdown();
   } else ok=false;
   OSReport("GX2 shared world fixture %s\n",ok ? "PASS" : "FAIL");
   return ok ? 0 : 1;
}
