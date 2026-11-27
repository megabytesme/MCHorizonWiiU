#include "terrain.h"
#include "../gx2-explicit-readback/retire.h"
#include <coreinit/memdefaultheap.h>
#include <gx2/clear.h>
#include <gx2/context.h>
#include <stdlib.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern const uint8_t _binary_terrain_gsh_start[];
extern const uint8_t _binary_terrain_near_fog_gsh_start[];

enum Arm { STOCK, GUARDED_NEAR, FORCE_FAR_NEGATIVE };

static void quad(HorizonTerrainVertex *out, float left, float top, float depth,
                 unsigned layer, uint8_t cutout, uint8_t r, uint8_t g, uint8_t b)
{
   const float p[4][2] = {{left, top-1}, {left+1, top-1}, {left+1, top}, {left, top}};
   for (unsigned i = 0; i < 4; ++i) {
      out[i] = (HorizonTerrainVertex){
         .position = {p[i][0], p[i][1], depth, (float)(i * 16 * 257)},
         .uv = {(float)(16384 | layer << 16), 16384},
         .color_cutout = {r,g,b,cutout},
      };
   }
}

static int fill(void *source, HorizonTerrainVertex *vertices, uint32_t count)
{
   memcpy(vertices, source, count * sizeof(*vertices));
   return 0;
}

static const uint8_t *sample(const uint8_t *rgba, uint32_t w, uint32_t h,
                             unsigned quarter_x, unsigned quarter_y)
{
   return rgba + ((size_t)(h * quarter_y / 4) * w + w * quarter_x / 4) * 4;
}

static bool check_pixels(const uint8_t *rgba, uint32_t w, uint32_t h, unsigned output,
                         enum Arm arm, bool varying, uint8_t **stock)
{
   const size_t length = (size_t)w * h * 4;
   if (arm == STOCK) {
      *stock = malloc(length);
      if (!*stock) return false;
      memcpy(*stock, rgba, length);
   }
   unsigned left_diff = 0, right_diff = 0;
   unsigned max_far = 0;
   for (unsigned y = 0; y < h; ++y) {
      for (unsigned x = 0; x < w; ++x) {
         const size_t i = ((size_t)y * w + x) * 4;
         if (!memcmp(rgba + i, *stock + i, 4)) continue;
         if (x < w/2) ++left_diff;
         else {
            ++right_diff;
            for (unsigned c = 0; c < 3; ++c) {
               const unsigned delta = (unsigned)abs((int)rgba[i+c] - (int)(*stock)[i+c]);
               if (delta > max_far) max_far = delta;
            }
         }
      }
   }
   const uint8_t *green = sample(rgba,w,h,1,1);
   const uint8_t *blue = sample(rgba,w,h,1,3);
   const uint8_t *far = sample(rgba,w,h,3,1);
   // A cutout at alpha127 must leave the deeper ordinary green/alpha127.
   // Alpha128 writes depth and rejects the white layer behind the blue.
   bool ok = green[3] == 127 && blue[3] == 128 && far[3] == 255;
   if (!varying) {
      ok &= green[1] > 180 && green[0] > 0 && green[1] > green[2];
      ok &= blue[2] > 180 && blue[0] > 0 && blue[2] > blue[1];
   }
   if (arm == FORCE_FAR_NEGATIVE)
      ok &= left_diff == 0 && right_diff > (uint64_t)w*h/8 && max_far >= 30;
   else
      ok &= left_diff == 0 && right_diff == 0;

   OSReport("GX2 near fog fixture output=%u arm=%u varying=%u size=%ux%u "
            "left_diff=%u right_diff=%u max_far=%u samples=%u,%u,%u,%u/%u,%u,%u,%u/%u,%u,%u,%u result=%s\n",
            output, arm, (unsigned)varying, w, h, left_diff, right_diff, max_far,
            green[0],green[1],green[2],green[3], blue[0],blue[1],blue[2],blue[3],
            far[0],far[1],far[2],far[3], ok ? "PASS" : "FAIL");
   return ok;
}

static bool render(HorizonGx2Terrain *terrain, HorizonGx2TerrainSection *near,
                   HorizonGx2TerrainSection *far, Stage *stage, uint8_t *rgba,
                   enum Arm arm, bool varying, uint8_t *stock[2])
{
   const float close[4] = {58,48,-15,0}, distant[4] = {94,0,0,0};
   const float fog[4] = {0.125f,0.35f,0.75f,1};
   const float ranges[4] = {0,1024,86.40000152587890625f,96};
   bool ok = true;
   WHBGfxBeginRender();
   for (unsigned output = 0; output < 2; ++output) {
      if (output == 0) WHBGfxBeginRenderTV();
      else WHBGfxBeginRenderDRC();
      GX2ColorBuffer *color = output == 0 ? WHBGfxGetTVColourBuffer() : WHBGfxGetDRCColourBuffer();
      GX2DepthBuffer *depth = output == 0 ? WHBGfxGetTVDepthBuffer() : WHBGfxGetDRCDepthBuffer();
      const uint32_t w = color->surface.width, h = color->surface.height;

      GX2ClearBuffersEx(color, depth, 0, 0, 0, 1, 1, 0, GX2_CLEAR_FLAGS_DEPTH);
      GX2SetContextState(output == 0 ? WHBGfxGetTVContextState() : WHBGfxGetDRCContextState());
      GX2SetColorBuffer(color, GX2_RENDER_TARGET_0);
      GX2SetDepthBuffer(depth);
      _Alignas(16) const float clip[16] = {output == 0 ? 1.0f : 0.91f,0,0,0,
                                           0,1,0,0, 0,0,1,0, 0,0,0,1};
      if (horizon_gx2_terrain_bind(terrain,w,h,0,0,w,h,fog,ranges,false) != 0)
         return false;
      if (arm != STOCK && !horizon_gx2_terrain_set_near_fog(terrain,true))
         return false;
      if (horizon_gx2_terrain_draw_bound(terrain,near,clip,close) != 0)
         return false;
      if (arm == GUARDED_NEAR && !horizon_gx2_terrain_set_near_fog(terrain,false))
         return false;
      if (horizon_gx2_terrain_draw_bound(terrain,far,clip,distant) != 0)
         return false;
      if (arm == FORCE_FAR_NEGATIVE && !horizon_gx2_terrain_set_near_fog(terrain,false))
         return false;
      const enum ReadStatus status = stage_read(stage, color, rgba, RGBA_CAP);
      if (status != READ_OK) {
         OSReport("GX2 near fog fixture readback output=%u arm=%u status=%u\n", output, arm, status);
         return false;
      }
      ok &= check_pixels(rgba, w, h, output, arm, varying, &stock[output]);
      if (output == 0) WHBGfxFinishRenderTV();
      else WHBGfxFinishRenderDRC();
   }
   WHBGfxFinishRender();
   return ok;
}

int main(void)
{
   WHBProcInit();
   const bool graphics = WHBGfxInit();
   HorizonGx2Terrain *terrain = NULL;
   HorizonGx2TerrainSection *near = NULL, *far = NULL;
   uint8_t atlas[16*4*4], light[16*16*4];
   const uint8_t alphas[4] = {0,127,128,255};
   memset(light,255,sizeof(light));
   for (unsigned i = 0; i < 16*4; ++i)
      memcpy(atlas+i*4,(uint8_t[]){255,255,255,alphas[(i%16)/4]},4);
   HorizonTerrainVertex local[16], distant[8];
   quad(local,-1,1,0.2f,1,255,255,0,0);
   quad(local+4,-1,1,0.6f,1,0,0,255,0);
   quad(local+8,-1,0,0.2f,2,255,0,0,255);
   quad(local+12,-1,0,0.6f,3,0,255,255,255);
   quad(distant,0,1,0.35f,3,0,255,255,32);
   quad(distant+4,0,0,0.35f,3,0,255,224,32);
   uint32_t bytes;
   bool ok = graphics && horizon_gx2_terrain_create(&terrain,_binary_terrain_gsh_start,16,4,atlas,sizeof(atlas)) == 0 &&
      horizon_gx2_terrain_set_sprite_grid(terrain,4,4) == 0 &&
      horizon_gx2_terrain_section_create(&near,16,fill,local,&bytes) == 0 &&
      horizon_gx2_terrain_section_create(&far,8,fill,distant,&bytes) == 0;
   Stage stage = {0};
   uint8_t *rgba = NULL;
   if (ok) {
      ok = !horizon_gx2_terrain_load_near_fog(terrain,_binary_terrain_gsh_start) &&
           horizon_gx2_terrain_load_near_fog(terrain,_binary_terrain_near_fog_gsh_start) &&
           !horizon_gx2_terrain_set_near_fog(terrain,true);
      OSReport("GX2 near fog fixture shader guard=%s\n", ok ? "PASS" : "FAIL");
      stage.storage = MEMAllocFromDefaultHeapEx(STAGE_CAP,STAGE_ALIGN);
      rgba = MEMAllocFromDefaultHeap(RGBA_CAP);
      ok &= stage.storage && rgba;
   }
   for (unsigned varying = 0; ok && varying < 2; ++varying) {
      if (varying) {
         for (unsigned y = 0; y < 16; ++y)
            for (unsigned x = 0; x < 16; ++x) {
               uint8_t *p = light + (y * 16 + x) * 4;
               p[0] = 64 + y * 12;
               p[1] = 255 - x * 10;
               p[2] = 110 + x * 8;
            }
      }
      uint8_t *stock[2] = {0};
      ok = horizon_gx2_terrain_lightmap(terrain,light) == 0 &&
           render(terrain,near,far,&stage,rgba,STOCK,varying,stock) &&
           render(terrain,near,far,&stage,rgba,GUARDED_NEAR,varying,stock) &&
           render(terrain,near,far,&stage,rgba,FORCE_FAR_NEGATIVE,varying,stock);
      free(stock[0]);
      free(stock[1]);
   }
   if (graphics) retire_for_teardown(&stage, "GX2 near fog fixture");
   if (rgba) MEMFreeToDefaultHeap(rgba);
   if (stage.storage) MEMFreeToDefaultHeap(stage.storage);
   if (near) horizon_gx2_terrain_section_destroy(near);
   if (far) horizon_gx2_terrain_section_destroy(far);
   if (terrain) horizon_gx2_terrain_destroy(terrain);
   if (graphics) WHBGfxShutdown();
   WHBProcShutdown();
   OSReport("GX2 near fog fixture result=%s\n",ok ? "PASS" : "FAIL");
   return ok ? 0 : 1;
}
