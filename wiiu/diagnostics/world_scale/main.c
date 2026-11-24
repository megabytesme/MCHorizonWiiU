#include "world_scale.c"
#include "hud.h"
#include "loading_host.h"
#include <coreinit/debug.h>
#include <coreinit/thread.h>
#include <coreinit/time.h>
#include <gx2/event.h>
#include <stdio.h>
#include <whb/proc.h>

static int texture(GX2Texture *texture)
{
   const GX2RResourceFlags flags = GX2R_RESOURCE_BIND_TEXTURE | GX2R_RESOURCE_USAGE_CPU_WRITE |
      GX2R_RESOURCE_USAGE_GPU_READ;
   texture->surface = world_surface(1, 2, GX2_SURFACE_FORMAT_UNORM_R8_G8_B8_A8);
   texture->surface.tileMode = GX2_TILE_MODE_LINEAR_ALIGNED;
   if (!GX2RCreateSurface(&texture->surface, flags)) return 0;
   uint8_t *pixels = GX2RLockSurfaceEx(&texture->surface, 0, flags);
   if (!pixels) return 0;
   memcpy(pixels, (uint8_t[]){255, 0, 0, 255}, 4);
   memcpy(pixels + texture->surface.pitch * 4, (uint8_t[]){0, 0, 255, 255}, 4);
   GX2RUnlockSurfaceEx(&texture->surface, 0, flags);
   texture->viewNumMips = 1;
   texture->viewNumSlices = 1;
   texture->compMap = GX2_COMP_MAP(GX2_SQ_SEL_R, GX2_SQ_SEL_G, GX2_SQ_SEL_B, GX2_SQ_SEL_A);
   GX2InitTextureRegs(texture);
   return 1;
}

static void draw_pattern(HorizonGx2WorldScale *scale, const GX2Texture *texture,
                         uint32_t x, uint32_t y, uint32_t w, uint32_t h)
{
   GX2Sampler point;
   GX2InitSampler(&point, GX2_TEX_CLAMP_MODE_CLAMP, GX2_TEX_XY_FILTER_MODE_POINT);
   GX2SetViewport(x, y, w, h, 0.0f, 1.0f);
   GX2SetScissor(x, y, w, h);
   GX2SetShaderMode(scale->shader.pixelShader->mode);
   GX2SetVertexShader(scale->shader.vertexShader);
   GX2SetPixelShader(scale->shader.pixelShader);
   GX2SetFetchShader(&scale->shader.fetchShader);
   GX2RSetAttributeBuffer(&scale->geometry, 0, sizeof(CompositeVertex), 0);
   GX2SetPixelTexture(texture, scale->texture_slot);
   GX2SetPixelSampler(&point, scale->texture_slot);
   GX2SetDepthOnlyControl(TRUE, TRUE, GX2_COMPARE_FUNC_LEQUAL);
   GX2SetAlphaTest(FALSE, GX2_COMPARE_FUNC_ALWAYS, 0.0f);
   GX2SetCullOnlyControl(GX2_FRONT_FACE_CCW, FALSE, FALSE);
   GX2SetRasterizerClipControlEx(TRUE, TRUE, TRUE);
   GX2SetColorControl(GX2_LOGIC_OP_COPY, 0, FALSE, TRUE);
   GX2SetTargetChannelMasks(GX2_CHANNEL_MASK_RGBA, 0, 0, 0, 0, 0, 0, 0);
   GX2DrawEx(GX2_PRIMITIVE_MODE_TRIANGLES, 3, 0, 1);
}

static bool draw_hud(HorizonGx2Hud *hud, uint32_t target, uint32_t w, uint32_t h)
{
   const float corners[6][2] = {{4, 20}, {7, 20}, {7, 30}, {4, 20}, {7, 30}, {4, 30}};
   HorizonHudVertex vertices[6] = {0};
   for (uint32_t i = 0; i < 6; ++i) {
      memcpy(vertices[i].pos, corners[i], sizeof(vertices[i].pos));
      vertices[i].uv[0] = vertices[i].uv[1] = 0.5f;
      vertices[i].color = 0xff00ff00;
   }
   const HorizonHudRun run = {0, 6, HORIZON_HUD_ALPHA, {0}};
   const HorizonHudFrame frame = {
      .vertices = vertices, .vertex_count = 6, .runs = &run, .run_count = 1,
      .viewport_width = w, .viewport_height = h, .scale = 1.0f,
   };
   return horizon_gx2_hud_draw_to_output(hud, &frame, 0, 0, w, h, target) == HORIZON_HUD_OK;
}

static bool readback(uint32_t w, uint32_t h, uint32_t target, uint32_t percent)
{
   size_t bytes = (size_t)w * h * 4;
   uint8_t *pixels = malloc(bytes);
   if (!pixels) return false;
   if (horizon_loading_host_read_rgba(target, pixels, bytes) != 0) {
      free(pixels);
      return false;
   }
   const uint8_t *margin = pixels + (2 * w + 2) * 4;
   const uint8_t *top = pixels + (h / 4 * w + w / 2) * 4;
   const uint8_t *bottom = pixels + (h * 3 / 4 * w + w / 2) * 4;
   const uint8_t *center = pixels + ((h / 2 - 2) * w + w / 2) * 4;
   const uint8_t *before_center = center - w * 4;
   const uint8_t *hud = pixels + (22 * w + 6) * 4;
   const bool smooth = percent != 50 || (center[0] > 30 && center[2] > 30) ||
      (before_center[0] > 30 && before_center[2] > 30);
   const bool success = margin[0] == 0 && margin[1] == 0 && margin[2] == 0 &&
      top[0] >= 245 && top[1] <= 10 && top[2] <= 10 &&
      bottom[0] <= 10 && bottom[1] <= 10 && bottom[2] >= 245 &&
      hud[0] == 0 && hud[1] == 255 && hud[2] == 0 && smooth;
   OSReport("GX2 scale diagnostic scale=%u target=%u result=%s margin=%u,%u,%u top=%u,%u,%u bottom=%u,%u,%u hud=%u,%u,%u center=%u,%u,%u\n",
            percent, target, success ? "PASS" : "FAIL", margin[0], margin[1], margin[2],
            top[0], top[1], top[2], bottom[0], bottom[1], bottom[2],
            hud[0], hud[1], hud[2], center[0], center[1], center[2]);
   free(pixels);
   return success;
}

int main(void)
{
   WHBProcInit();
   if (!WHBGfxInit()) return 1;
   GX2Texture pattern = {0};
   if (!texture(&pattern)) return 2;
   HorizonGx2Hud *hud;
   const uint8_t white[] = {255, 255, 255, 255};
   size_t gfd_length;
   const uint8_t *gfd = horizon_loading_host_hud_gfd(&gfd_length);
   if (!gfd || !gfd_length || horizon_gx2_hud_create(&hud, gfd, 1, 1, white, sizeof(white), 6) !=
       HORIZON_HUD_OK) return 3;

   uint32_t over_budget_bytes = 0;
   const uint32_t oversized[] = {1920, 1080, 1920, 1080};
   HorizonGx2WorldScale *oversized_state = horizon_gx2_world_scale_create(oversized, &over_budget_bytes);
   const bool budget_respected = !oversized_state && !over_budget_bytes;
   horizon_gx2_world_scale_destroy(oversized_state);
   OSReport("GX2 scale diagnostic allocation budget result=%s\n", budget_respected ? "PASS" : "FAIL");

   GX2Surface *outputs[] = {&WHBGfxGetTVColourBuffer()->surface, &WHBGfxGetDRCColourBuffer()->surface};
   const uint32_t scales[] = {100, 75, 50, 95, 75};
   bool passed = budget_respected;
   for (unsigned i = 0; i < sizeof(scales) / sizeof(scales[0]); ++i) {
      const uint32_t percent = scales[i];
      uint32_t scaled[4];
      uint32_t viewports[2][4];
      for (unsigned target = 0; target < 2; ++target) {
         uint32_t w = outputs[target]->width - 22, h = outputs[target]->height - 26;
         scaled[target * 2] = (w * percent + 50) / 100;
         scaled[target * 2 + 1] = (h * percent + 50) / 100;
         viewports[target][0] = 9;
         viewports[target][1] = 11;
         viewports[target][2] = w;
         viewports[target][3] = h;
      }
      // At 100% the diagnostic only needs the shader and geometry for its
      // unscaled reference. The game bypasses this object at 100%.
      const uint32_t minimal[] = {1, 1, 1, 1};
      uint32_t bytes;
      HorizonGx2WorldScale *state = horizon_gx2_world_scale_create(
         percent == 100 ? minimal : scaled, &bytes);
      if (!state) {
         OSReport("GX2 scale diagnostic: create failed scale=%u\n", percent);
         passed = false;
         break;
      }
      OSReport("GX2 scale diagnostic: dimensions scale=%u TV=%ux%u DRC=%ux%u bytes=%u\n",
               percent, scaled[0], scaled[1], scaled[2], scaled[3], percent == 100 ? 0 : bytes);
      WHBGfxBeginRender();
      for (unsigned target = 0; target < 2; ++target) {
         if (target == 0) WHBGfxBeginRenderTV();
         else WHBGfxBeginRenderDRC();
         WHBGfxClearColor(0.0f, 0.0f, 0.0f, 1.0f);
         if (percent < 100) {
            horizon_gx2_world_scale_begin(state, target, (float[]){0.0f, 1.0f, 0.0f, 1.0f});
            draw_pattern(state, &pattern, 0, 0, scaled[target * 2], scaled[target * 2 + 1]);
            horizon_gx2_world_scale_end(state, target, viewports[target], true);
         } else {
            draw_pattern(state, &pattern, 9, 11, viewports[target][2], viewports[target][3]);
         }
         const uint32_t output_w = outputs[target]->width, output_h = outputs[target]->height;
         passed &= draw_hud(hud, target, output_w, output_h);
         passed &= readback(output_w, output_h, target, percent);
         if (target == 0) WHBGfxFinishRenderTV();
         else WHBGfxFinishRenderDRC();
      }
      WHBGfxFinishRender();
      horizon_gx2_world_scale_destroy(state);
   }
   horizon_gx2_hud_destroy(hud);
   GX2RDestroySurfaceEx(&pattern.surface, 0);
   OSReport("GX2 scale diagnostic done result=%s\n", passed ? "PASS" : "FAIL");
   const OSTime deadline = OSGetTime() + OSSecondsToTicks(3);
   while (OSGetTime() < deadline && WHBProcIsRunning())
      OSSleepTicks(OSMillisecondsToTicks(20));
   WHBGfxShutdown();
   WHBProcShutdown();
   return passed ? 0 : 3;
}
