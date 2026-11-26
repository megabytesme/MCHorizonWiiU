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
#include <stdint.h>
#include <string.h>
#include <whb/gfx.h>
#include <whb/proc.h>

#ifdef CEMU_BASELINE
#include <gx2r/surface.h>
#include "baseline.inc"
#endif

extern const uint8_t _binary_pattern_gsh_start[];

#include "stage.h"

typedef struct {
   float position[2], uv[2];
} Vertex;

static _Alignas(GX2_VERTEX_BUFFER_ALIGNMENT) Vertex triangle[] = {
   {{-1,  1}, {0, 0}},
   {{-1, -3}, {0, 2}},
   {{ 3,  1}, {2, 0}},
};

static void draw(const WHBGfxShaderGroup *shader, const GX2ColorBuffer *color)
{
   const uint32_t w = color->surface.width, h = color->surface.height;
   GX2SetViewport(0, 0, w, h, 0, 1);
   GX2SetScissor(w / 8, h / 8, w * 3 / 4, h * 3 / 4);
   GX2SetShaderMode(shader->vertexShader->mode);
   GX2SetVertexShader(shader->vertexShader);
   GX2SetPixelShader(shader->pixelShader);
   GX2SetFetchShader(&shader->fetchShader);
   GX2SetAttribBuffer(0, sizeof(triangle), sizeof(triangle[0]), triangle);
   GX2SetRasterizerClipControlEx(TRUE, TRUE, TRUE);
   GX2SetCullOnlyControl(GX2_FRONT_FACE_CCW, FALSE, FALSE);
   GX2SetDepthOnlyControl(FALSE, FALSE, GX2_COMPARE_FUNC_ALWAYS);
   GX2SetAlphaTest(FALSE, GX2_COMPARE_FUNC_ALWAYS, 0);
   GX2SetTargetChannelMasks(GX2_CHANNEL_MASK_RGBA, 0, 0, 0, 0, 0, 0, 0);
   GX2SetColorControl(GX2_LOGIC_OP_COPY, 1, FALSE, TRUE);
   GX2SetBlendControl(GX2_RENDER_TARGET_0, GX2_BLEND_MODE_ONE, GX2_BLEND_MODE_ZERO,
      GX2_BLEND_COMBINE_MODE_ADD, FALSE, GX2_BLEND_MODE_ONE, GX2_BLEND_MODE_ZERO,
      GX2_BLEND_COMBINE_MODE_ADD);
   GX2DrawEx(GX2_PRIMITIVE_MODE_TRIANGLES, 3, 0, 1);
}

static bool pixels_match(const uint8_t *rgba, unsigned w, unsigned h,
                         const uint8_t border[4])
{
   static const uint8_t colors[][4] = {
      {255, 0, 0, 255}, {0, 255, 0, 255}, {0, 0, 255, 255}, {255, 255, 0, 255},
   };
   static const unsigned samples[] = {1, 4, 12, 15};
   for (unsigned yi = 0; yi < 4; ++yi) {
      for (unsigned xi = 0; xi < 4; ++xi) {
         const unsigned x = w * samples[xi] / 16, y = h * samples[yi] / 16;
         const bool inside = (xi == 1 || xi == 2) && (yi == 1 || yi == 2);
         const uint8_t *expected = inside ? colors[(yi == 2) * 2 + (xi == 2)] : border;
         const uint8_t *pixel = rgba + ((size_t)y * w + x) * 4;
         if (memcmp(pixel, expected, 4)) {
            OSReport("GX2 stage pixel x=%u y=%u expected=%u,%u,%u,%u actual=%u,%u,%u,%u\n",
                     x, y, expected[0], expected[1], expected[2], expected[3],
                     pixel[0], pixel[1], pixel[2], pixel[3]);
            return false;
         }
      }
   }
   return true;
}

#ifdef CEMU_BASELINE
static uint64_t pixel_digest(const uint8_t *bytes, size_t length)
{
   uint64_t hash = UINT64_C(14695981039346656037);
   for (size_t i = 0; i < length; ++i) {
      hash ^= bytes[i];
      hash *= UINT64_C(1099511628211);
   }
   return hash;
}
#endif

static bool render(Stage *stage, uint8_t *rgba, const WHBGfxShaderGroup *shader, unsigned round)
{
   static const uint8_t borders[2][2][4] = {
      {{0, 255, 255, 255}, {255, 0, 255, 255}},
      {{0, 0, 0, 255}, {255, 255, 255, 255}},
   };
   WHBGfxBeginRender();
   for (unsigned target = 0; target < 2; ++target) {
      if (target == 0) WHBGfxBeginRenderTV();
      else WHBGfxBeginRenderDRC();
      GX2ColorBuffer *color = target == 0 ? WHBGfxGetTVColourBuffer() : WHBGfxGetDRCColourBuffer();
      const unsigned w = color->surface.width, h = color->surface.height;
      const uint8_t *border = borders[round][target];
      WHBGfxClearColor(border[0] / 255.0f, border[1] / 255.0f, border[2] / 255.0f, 1);
      draw(shader, color);

      const bool short_ok = stage_read(stage, color, rgba, 1) == READ_LENGTH;
      const enum ReadStatus result = stage_read(stage, color, rgba, RGBA_CAP);
      const bool explicit_ok = short_ok && result == READ_OK && pixels_match(rgba, w, h, border);
      OSReport("GX2 explicit stage round=%u output=%u size=%ux%u status=%u result=%s\n",
               round, target, w, h, result, explicit_ok ? "PASS" : "FAIL");
      if (stage->copy_pending) return false;
#ifdef CEMU_BASELINE
      const size_t used = (size_t)w * h * 4;
      const uint64_t actual_digest = result == READ_OK ? pixel_digest(rgba, used) : 0;
      const bool baseline_ok = horizon_loading_host_read_rgba(target, rgba, RGBA_CAP) == 0 &&
         pixels_match(rgba, w, h, border);
      const uint64_t expected_digest = baseline_ok ? pixel_digest(rgba, used) : 0;
      OSReport("GX2 baseline stage round=%u output=%u size=%ux%u explicit=%016llx baseline=%016llx result=%s\n",
               round, target, w, h, (unsigned long long)actual_digest, (unsigned long long)expected_digest,
               baseline_ok && actual_digest == expected_digest ? "PASS" : "FAIL");
      if (!baseline_ok || actual_digest != expected_digest) return false;
#endif
      if (!explicit_ok) return false;
      if (target == 0) WHBGfxFinishRenderTV();
      else WHBGfxFinishRenderDRC();
   }
   WHBGfxFinishRender();
   return true;
}

int main(void)
{
   WHBProcInit();
   const bool graphics = WHBGfxInit();
   Stage stage = {0};
   uint8_t *rgba = NULL;
   WHBGfxShaderGroup shader;
   const bool shader_loaded = graphics && WHBGfxLoadGFDShaderGroup(&shader, 0, _binary_pattern_gsh_start);
   bool ok = shader_loaded &&
      WHBGfxInitShaderAttribute(&shader, "position", 0, offsetof(Vertex, position), GX2_ATTRIB_FORMAT_FLOAT_32_32) &&
      WHBGfxInitShaderAttribute(&shader, "uv", 0, offsetof(Vertex, uv), GX2_ATTRIB_FORMAT_FLOAT_32_32) &&
      WHBGfxInitFetchShader(&shader);
   if (ok) {
      stage.storage = MEMAllocFromDefaultHeapEx(STAGE_CAP, STAGE_ALIGN);
      rgba = MEMAllocFromDefaultHeap(RGBA_CAP);
      ok = stage.storage && rgba;
   }
   if (ok) {
      GX2Invalidate(GX2_INVALIDATE_MODE_CPU_ATTRIBUTE_BUFFER, triangle, sizeof(triangle));
      ok = render(&stage, rgba, &shader, 0) && render(&stage, rgba, &shader, 1);
   }

   // With no proven copy retirement, keep all potentially referenced graphics allocations intact.
   OSTime submitted, retired;
   if (!stage.copy_pending && (!graphics || finish_commands(&submitted, &retired))) {
      if (stage.storage) MEMFreeToDefaultHeap(stage.storage);
      if (rgba) MEMFreeToDefaultHeap(rgba);
      if (shader_loaded) WHBGfxFreeShaderGroup(&shader);
      if (graphics) WHBGfxShutdown();
      WHBProcShutdown();
   } else {
      ok = false;
      OSReport("GX2 stage graphics ownership retained pending recovery\n");
   }
   OSReport("GX2 explicit readback comparison result=%s\n", ok ? "PASS" : "FAIL");
   return ok ? 0 : 1;
}
