/* One stock terrain binding and immutable geometry per paired primitive test. */
#include "terrain.c"
#include "retire.h"
#include "draw_clear.h"
#include <proc_ui/procui.h>
#include <whb/proc.h>

extern const uint8_t _binary_stock_gsh_start[];

enum {
   SIDE = 16, CELL = SIDE + 2, COLS = 3, WIDTH = COLS * CELL, HEIGHT = CELL,
   QUADS = 8, END = TERRAIN_INDEX_QUADS + QUADS, VERTICES = END * 4,
   OUTPUTS = 2,
};
enum Arm { STOCK, REPEAT, AUTO_QUADS, WRONG_DIAGONAL, NO_DEPTH_WRITE, ARM_COUNT, COLD_REFERENCE = ARM_COUNT };

typedef struct {
   uint8_t *rgba;
   unsigned w, h;
} Image;

static int fill_atlas(void *unused, uint8_t *pixels, uint32_t pitch)
{
   (void)unused;
   if (!pixels || pitch < WIDTH) return -1;
   for (unsigned y = 0; y < HEIGHT; ++y) {
      for (unsigned x = 0; x < WIDTH; ++x) {
         const unsigned layer = x / CELL;
         const unsigned s = x % CELL, t = y;
         const unsigned u = s == 0 ? 0 : s == CELL - 1 ? SIDE - 1 : s - 1;
         const unsigned v = t == 0 ? 0 : t == CELL - 1 ? SIDE - 1 : t - 1;
         uint8_t *p = pixels + ((size_t)y * pitch + x) * 4;
         p[0] = 77 + (u * 3 + layer * 41 + v) % 171;
         p[1] = 27 + (u + layer * 17 + v * 5) % 177;
         p[2] = 59 + (u * 5 + layer * 13 + v * 7) % 193;
         p[3] = ((u / 2 + v / 3 + layer) & 1) ? 127 : 128;
      }
   }
   return 0;
}

static int fill_mesh(void *unused, HorizonTerrainVertex *vertices, uint32_t count)
{
   (void)unused;
   if (!vertices || count != VERTICES) return -1;
   memset(vertices, 0, (size_t)count * sizeof(*vertices));
   for (unsigned bank = 0; bank < 2; ++bank) {
      const unsigned first = bank ? TERRAIN_INDEX_QUADS : 0;
      for (unsigned q = 0; q < QUADS; ++q) {
         const float x = -.91f + (q % 4) * .46f + (bank ? .04f : 0);
         const float y = -.82f + (q / 4) * .67f;
         for (unsigned corner = 0; corner < 4; ++corner) {
            const unsigned i = q == 7 ? 3u - corner : corner;
            const bool high = i >= 2, right = i == 1 || i == 2;
            const unsigned tile = (q + bank) % COLS;
            const float z[4] = {.20f, .61f, .12f, .46f};
            vertices[(first + q) * 4 + corner] = (HorizonTerrainVertex){
               .position = {x + (right ? .52f : 0), y + (high ? .69f : 0),
                            q == 0 && i == 0 ? -1.2f : z[i] + .04f * bank,
                            (float)((i * 39 + q * 3) * 256 + (i * 13 + q * 11))},
               .uv = {(float)((right ? (q & 1 ? 65535u : 32768u) : 0) | (tile << 16)),
                      (float)(high ? 0u : 32768u)},
               .color_cutout = {255 - i * 41, 107 + i * 33, 97 + i * 29,
                                q & 1 ? 255 : 0},
            };
         }
      }
   }
   return 0;
}

static const _Alignas(GX2_VERTEX_BUFFER_ALIGNMENT) float tail_vertices[4][4] = {
   {-.38f,-.38f,.34f,0}, {-.38f,.34f,.34f,0}, {.95f,.34f,.34f,0}, {.95f,-.38f,.34f,0},
};

static void depth_witness(const DrawClear *clear, HorizonGx2Terrain *terrain)
{
   static const _Alignas(16) float green[4] = {0,1,0,1};
   // Clear shader, full viewport and identity transform were installed in
   // this context by draw_clear_output before terrain rasterization.
   GX2SetShaderMode(GX2_SHADER_MODE_UNIFORM_REGISTER);
   GX2SetVertexShader(clear->group.vertexShader);
   GX2SetPixelShader(clear->group.pixelShader);
   GX2SetFetchShader(&clear->group.fetchShader);
   static const _Alignas(16) float identity[16] = {
      1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1,
   };
   for (unsigned i = 0; i < 4; ++i)
      GX2SetVertexUniformReg(clear->clip[i],4,identity + i * 4);
   GX2SetPixelUniformReg(clear->color,4,green);
   GX2SetAttribBuffer(0,sizeof(tail_vertices),sizeof(tail_vertices[0]),tail_vertices);
   GX2SetCullOnlyControl(GX2_FRONT_FACE_CCW,FALSE,FALSE);
   GX2SetDepthOnlyControl(TRUE,FALSE,GX2_COMPARE_FUNC_LEQUAL);
   horizon_gx2_terrain_draw_quads(terrain,4);
}

static bool alternate_index_create(GX2RBuffer *buffer)
{
   uint32_t bytes;
   if (terrain_buffer_create(buffer,GX2R_RESOURCE_BIND_INDEX_BUFFER,sizeof(uint16_t),QUADS*6,&bytes))
      return false;
   uint16_t *p = GX2RLockBufferEx(buffer,0);
   if (!p) return false;
   static const uint16_t corners[6] = {0,1,3,1,2,3};
   for (unsigned q = 0; q < QUADS; ++q)
      for (unsigned i = 0; i < 6; ++i) p[q*6+i] = q*4+corners[i];
   GX2RUnlockBufferEx(buffer,0);
   return true;
}

static void draw_range(HorizonGx2Terrain *terrain, GX2RBuffer *alternate, enum Arm arm,
                       uint32_t first, uint32_t quads)
{
   if (arm == AUTO_QUADS) {
      // Keep the exact same slice bound as the stock indexed path.
      for (uint32_t base = first; quads != 0;) {
         const uint32_t batch = quads < TERRAIN_INDEX_QUADS ? quads : TERRAIN_INDEX_QUADS;
         GX2DrawEx(GX2_PRIMITIVE_MODE_QUADS,batch*4,base*4,1);
         base += batch;
         quads -= batch;
      }
   } else if (arm == WRONG_DIAGONAL) {
      GX2RDrawIndexed(GX2_PRIMITIVE_MODE_TRIANGLES,alternate,GX2_INDEX_TYPE_U16,QUADS*6,0,first*4,1);
      if (quads == END)
         GX2RDrawIndexed(GX2_PRIMITIVE_MODE_TRIANGLES,alternate,GX2_INDEX_TYPE_U16,
                         QUADS*6,0,TERRAIN_INDEX_QUADS*4,1);
   } else {
      terrain_draw_indexed_range(&terrain->quad_indices,first,quads);
   }
}

static bool compare(Image *reference, const uint8_t *rgba, unsigned w, unsigned h,
                    unsigned scene, enum Arm arm, unsigned output)
{
   const size_t pixels = (size_t)w*h;
   const uint8_t *last = rgba + (pixels - 1) * 4;
   const bool clear = !last[0] && !last[1] && !last[2] && last[3] == 255;
   unsigned colored = 0, one_color = 0, changed = 0, alpha = 0;
   const bool reference_arm = arm == STOCK || arm == COLD_REFERENCE;
   if (!reference_arm && (!reference->rgba || w != reference->w || h != reference->h)) return false;
   uint64_t digest = UINT64_C(14695981039346656037);
   for (size_t i = 0; i < pixels; ++i) {
      const uint8_t *p = rgba + i*4;
      colored += p[0] || p[2];
      one_color += !p[0] && p[1] == 255 && !p[2];
      for (unsigned byte = 0; byte < 4; ++byte)
         digest = (digest ^ p[byte]) * UINT64_C(1099511628211);
      if (!reference_arm && memcmp(p,reference->rgba + i*4,4)) {
         ++changed;
         alpha += p[3] != reference->rgba[i*4+3];
      }
   }
   if (arm == STOCK) {
      reference->rgba = malloc(pixels*4);
      if (!reference->rgba) return false;
      reference->w = w;
      reference->h = h;
      memcpy(reference->rgba,rgba,pixels*4);
   }
   const bool ok = clear && colored > pixels/10 && one_color > pixels/100 &&
      ((arm == WRONG_DIAGONAL || arm == NO_DEPTH_WRITE) ? changed > pixels/1000 : changed == 0);
   OSReport("GX2 auto quad s=%u arm=%u out=%u draws=%u green=%u clear=%u diff=%u alpha=%u rgba=%016llx %s\n",
            scene,arm,output,colored,one_color,clear,changed,alpha,(unsigned long long)digest,ok ? "PASS" : "FAIL");
   return ok;
}

static bool render(Stage *stage, uint8_t *rgba, const DrawClear *clear, HorizonGx2Terrain *terrain,
                   HorizonGx2TerrainSection *section, GX2RBuffer *alternate,
                   Image expected[OUTPUTS], unsigned scene, enum Arm arm)
{
   if (!ProcUIInForeground()) return false;
   static const _Alignas(16) float color[4] = {.13f,.31f,.36f,.84f};
   static const _Alignas(16) float fog[4] = {0,2.2f,1.5f,3.7f};
   static const _Alignas(16) float origin[4] = {.18f,.27f,.37f,0};
   static const _Alignas(16) float clip[OUTPUTS][16] = {
      {1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1},
      {.89f,0,.12f,.024f, 0,.78f,0,-.045f, 0,0,1,0, 0,0,.21f,.93f},
   };
   const uint32_t first = scene ? TERRAIN_INDEX_QUADS : 0;
   const uint32_t quads = scene ? QUADS : END;
   OSTime copies[OUTPUTS] = {0};
   bool ok = true;
   WHBGfxBeginRender();
   for (unsigned i = 0; i < OUTPUTS; ++i) {
      if (i) WHBGfxBeginRenderDRC(); else WHBGfxBeginRenderTV();
      GX2ColorBuffer *target = i ? WHBGfxGetDRCColourBuffer() : WHBGfxGetTVColourBuffer();
      GX2DepthBuffer *depth = i ? WHBGfxGetDRCDepthBuffer() : WHBGfxGetTVDepthBuffer();
      GX2SetContextState(i ? WHBGfxGetDRCContextState() : WHBGfxGetTVContextState());
      GX2SetColorBuffer(target,GX2_RENDER_TARGET_0);
      GX2SetDepthBuffer(depth);
      const unsigned w = target->surface.width, h = target->surface.height;
      draw_clear_output(clear,terrain,w,h,false);
      horizon_gx2_terrain_set_solid_back_culling(terrain,scene != 0);
      if (horizon_gx2_terrain_bind(terrain,w,h,0,0,w,h,color,fog,false) ||
          !bind_section(terrain,section,clip[i],origin)) {
         ok = false;
      } else {
         if (arm == NO_DEPTH_WRITE) GX2SetDepthOnlyControl(TRUE,FALSE,GX2_COMPARE_FUNC_LEQUAL);
         draw_range(terrain,alternate,arm,first,quads);
         depth_witness(clear,terrain);
         const enum ReadStatus status = stage_read_with_receipt(stage,target,rgba,RGBA_CAP,&copies[i]);
         if (status != READ_OK) {
            OSReport("GX2 auto quad s=%u arm=%u out=%u read=%u FAIL\n",scene,arm,i,status);
            ok = false;
         } else {
            ok &= compare(&expected[i],rgba,w,h,scene,arm,i);
         }
      }
      if (i) WHBGfxFinishRenderDRC(); else WHBGfxFinishRenderTV();
   }
   WHBGfxFinishRender();
   return ok && copies[0] > 0 && copies[1] > copies[0];
}

int main(void)
{
   WHBProcInit();
   const bool graphics = WHBGfxInit();
   Stage stage = {0};
   DrawClear clear = {0};
   HorizonGx2Terrain *terrain = NULL;
   HorizonGx2TerrainSection *section = NULL;
   GX2RBuffer alternate = {0};
   uint8_t *rgba = NULL;
   bool ok = false;
   uint32_t geometry_bytes = 0;
   if (graphics) {
      stage.storage = MEMAllocFromDefaultHeapEx(STAGE_CAP,STAGE_ALIGN);
      rgba = MEMAllocFromDefaultHeap(RGBA_CAP);
      uint8_t light[16*16*4];
      for (unsigned i = 0; i < 16*16; ++i) {
         light[i*4] = 40 + i/16*12;
         light[i*4+1] = 26 + i%16*14;
         light[i*4+2] = 216 - i/16*11;
         light[i*4+3] = 255;
      }
      ok = stage.storage && rgba &&
         !horizon_gx2_terrain_create_mapped(&terrain,_binary_stock_gsh_start,WIDTH,HEIGHT,fill_atlas,NULL) &&
         !horizon_gx2_terrain_set_sprite_grid(terrain,COLS,CELL) &&
         !horizon_gx2_terrain_lightmap(terrain,light) &&
         !horizon_gx2_terrain_section_create(&section,VERTICES,fill_mesh,NULL,&geometry_bytes) &&
         alternate_index_create(&alternate) && draw_clear_init(&clear);
      OSReport("GX2 auto quad setup vertices=%u geometry=%u index_alloc=%u %s\n",
               VERTICES,geometry_bytes,horizon_gx2_terrain_index_allocation_bytes(terrain),ok ? "PASS" : "FAIL");
      if (ok) {
         GX2Invalidate(GX2_INVALIDATE_MODE_CPU_ATTRIBUTE_BUFFER,(void *)tail_vertices,sizeof(tail_vertices));
         static const enum Arm order[2][ARM_COUNT] = {
            {STOCK,REPEAT,AUTO_QUADS,WRONG_DIAGONAL,NO_DEPTH_WRITE},
            {STOCK,AUTO_QUADS,REPEAT,NO_DEPTH_WRITE,WRONG_DIAGONAL},
         };
         // Retain both independent cold stock observations before choosing a
         // reference. No default state or stock comparison is reclassified.
         Image cold[OUTPUTS] = {0};
         ok &= render(&stage,rgba,&clear,terrain,section,&alternate,cold,0,COLD_REFERENCE);
         for (unsigned scene = 0; scene < 2; ++scene) {
            Image images[OUTPUTS] = {0};
            for (unsigned i = 0; i < ARM_COUNT; ++i)
               ok &= render(&stage,rgba,&clear,terrain,section,&alternate,images,scene,order[scene][i]);
            free(images[0].rgba);
            free(images[1].rgba);
         }
      }
      retire_for_teardown(&stage,"GX2 auto quad");
      terrain_buffer_release(&alternate);
      horizon_gx2_terrain_section_destroy_after_wait(section);
      horizon_gx2_terrain_destroy(terrain);
      if (clear.positions) MEMFreeToDefaultHeap(clear.positions);
      if (clear.group.vertexShader) WHBGfxFreeShaderGroup(&clear.group);
      if (rgba) MEMFreeToDefaultHeap(rgba);
      if (stage.storage) MEMFreeToDefaultHeap(stage.storage);
      WHBGfxShutdown();
   }
   WHBProcShutdown();
   OSReport("GX2 auto quad fixture result=%s\n",ok ? "PASS" : "FAIL");
   return ok ? 0 : 1;
}
