#include "terrain.h"
#include <coreinit/debug.h>
#ifdef HORIZON_TEST_BOUND_TERRAIN
#include <coreinit/time.h>
#endif
#include <gx2/event.h>
#include <gx2/surface.h>
#include <gx2r/surface.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern const uint8_t _binary_terrain_gsh_start[];

static void quad(HorizonTerrainVertex out[4], float x0, float x1, float z, float u,
                 const uint8_t rgba[4])
{
   const float points[4][2] = {{x0, -0.75f}, {x1, -0.75f}, {x1, 0.75f}, {x0, 0.75f}};
   for (unsigned i = 0; i < 4; ++i) {
      out[i] = (HorizonTerrainVertex){
         .position = {points[i][0], points[i][1], z, 0.0f},
         .uv = {u, 0.5f},
      };
      memcpy(out[i].color_cutout, rgba, 4);
   }
}

static int fill(void *source, HorizonTerrainVertex *vertices, uint32_t count)
{
   memcpy(vertices, source, count * sizeof(*vertices));
   return 0;
}

static bool readback(const GX2ColorBuffer *buffer, unsigned target)
{
   const GX2Surface *src = &buffer->surface;
   GX2Surface copy = {
      .dim = GX2_SURFACE_DIM_TEXTURE_2D, .width = src->width, .height = src->height,
      .depth = 1, .mipLevels = 1, .format = src->format,
      .aa = GX2_AA_MODE1X, .tileMode = GX2_TILE_MODE_LINEAR_SPECIAL,
   };
   const GX2RResourceFlags flags = GX2R_RESOURCE_BIND_TEXTURE |
      GX2R_RESOURCE_USAGE_CPU_READ | GX2R_RESOURCE_USAGE_GPU_WRITE;
   if (!GX2RCreateSurface(&copy, flags)) return false;
   GX2DrawDone();
   GX2CopySurface(src, 0, 0, &copy, 0, 0);
   GX2DrawDone();
   const uint8_t *pixels = GX2RLockSurfaceEx(&copy, 0, flags);
   bool ok = pixels != NULL;
   if (pixels) {
      const size_t row = (size_t)(src->height / 2) * copy.pitch;
      const unsigned xs[] = {src->width / 4, src->width * 3 / 4};
      for (unsigned i = 0; i < 2; ++i) {
         const uint8_t *rgb = pixels + (row + xs[i]) * 4;
         bool match = i == 0 ? rgb[0] == 0 && rgb[1] == 255 && rgb[2] == 0
            : rgb[0] >= 63 && rgb[0] <= 65 && rgb[1] == 0 && rgb[2] >= 94 && rgb[2] <= 97;
         OSReport("GX2 translucent fixture output=%u sample=%u rgb=%u,%u,%u pass=%u\n", target,
                  i, rgb[0], rgb[1], rgb[2], match);
         ok &= match;
      }
      GX2RUnlockSurfaceEx(&copy, 0, flags);
   }
   GX2RDestroySurfaceEx(&copy, flags);
   return ok;
}

#ifdef HORIZON_TEST_BOUND_TERRAIN
static bool measure_bindings(HorizonGx2Terrain *terrain, HorizonGx2TerrainSection *section,
                             const float matrix[16], const float zero[4], const float ranges[4])
{
   const bool per_section[] = {true, false, false, true};
   bool ok = true;
   for (unsigned arm = 0; arm < 4; ++arm) {
      WHBGfxBeginRender();
      WHBGfxBeginRenderTV();
      WHBGfxClearColor(0, 0, 0, 1);
      GX2ColorBuffer *color = WHBGfxGetTVColourBuffer();
      const uint32_t w = color->surface.width, h = color->surface.height;
      GX2DrawDone();
      const OSTime start = OSGetSystemTime();
      for (unsigned section_index = 0; section_index < 128; ++section_index) {
         if (section_index == 0 || per_section[arm])
            ok &= horizon_gx2_terrain_bind(terrain, w, h, 0, 0, w, h, zero, ranges, false) == 0;
         ok &= horizon_gx2_terrain_draw_bound(terrain, section, matrix, zero) == 0;
      }
      const OSTime elapsed = OSGetSystemTime() - start;
      OSReport("GX2 terrain binding fixture arm=%u per_section=%u submission_us=%u draws=128\n",
               arm, (unsigned)per_section[arm], (unsigned)OSTicksToMicroseconds(elapsed));
      WHBGfxFinishRenderTV();
      WHBGfxBeginRenderDRC();
      WHBGfxClearColor(0, 0, 0, 1);
      WHBGfxFinishRenderDRC();
      WHBGfxFinishRender();
   }
   return ok;
}
#endif

int main(void)
{
   WHBProcInit();
   if (!WHBGfxInit()) return 1;
   const uint8_t atlas[8] = {255, 255, 255, 255, 255, 255, 255, 128};
   uint8_t lightmap[16 * 16 * 4];
   memset(lightmap, 255, sizeof(lightmap));
   HorizonGx2Terrain *terrain = NULL;
   HorizonGx2TerrainSection *front = NULL, *sorted = NULL, *rear = NULL;
   HorizonTerrainVertex solid[4], translucent[12], behind[4];
   quad(solid, -1.0f, 0.0f, 0.3f, 0.25f, (uint8_t[4]){0, 255, 0, 0});
   quad(behind, 0.0f, 1.0f, 0.5f, 0.25f, (uint8_t[4]){255, 255, 0, 0});
   quad(translucent, -1.0f, 1.0f, 0.4f, 0.75f, (uint8_t[4]){255, 0, 0, 128});
   quad(translucent + 4, -1.0f, 1.0f, 0.7f, 0.25f, (uint8_t[4]){0, 0, 255, 128});
   quad(translucent + 8, -1.0f, 1.0f, 0.1f, 0.25f, (uint8_t[4]){255, 255, 255, 0});
   /* Submit the invisible near quad first to expose any mistaken depth write. */
   const uint32_t indices[] = {
      8, 9, 10, 10, 11, 8,
      4, 5, 6, 6, 7, 4,
      0, 1, 2, 2, 3, 0,
   };
   uint32_t bytes;
   bool ok = horizon_gx2_terrain_create(&terrain, _binary_terrain_gsh_start, 2, 1, atlas, sizeof(atlas)) == 0 &&
      horizon_gx2_terrain_lightmap(terrain, lightmap) == 0 &&
      horizon_gx2_terrain_section_create(&front, 4, fill, solid, &bytes) == 0 &&
      horizon_gx2_terrain_section_create(&rear, 4, fill, behind, &bytes) == 0 &&
      horizon_gx2_terrain_section_create_sorted(&sorted, 12, fill, translucent, &bytes) == 0 &&
      horizon_gx2_terrain_section_update_indices(sorted, indices, 18) == 0;
   if (ok) {
      const float identity[16] = {1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1};
      const float zero[4] = {0}, ranges[4] = {10, 20, 10, 20};
      HorizonGx2TerrainSection *sequence[] = {front, sorted, rear};
#ifdef HORIZON_TEST_BOUND_TERRAIN
      ok &= measure_bindings(terrain, front, identity, zero, ranges);
#endif
      WHBGfxBeginRender();
      for (unsigned i = 0; i < 2; ++i) {
         if (i == 0) WHBGfxBeginRenderTV();
         else WHBGfxBeginRenderDRC();
         WHBGfxClearColor(0, 0, 0, 1);
         GX2ColorBuffer *color = i == 0 ? WHBGfxGetTVColourBuffer() : WHBGfxGetDRCColourBuffer();
         const uint32_t w = color->surface.width, h = color->surface.height;
         for (unsigned j = 0; j < 3; ++j) {
#ifdef HORIZON_TEST_BOUND_TERRAIN
            ok &= horizon_gx2_terrain_bind(terrain, w, h, 0, 0, w, h, zero, ranges, j == 1) == 0;
            ok &= horizon_gx2_terrain_draw_bound(terrain, sequence[j], identity, zero) == 0;
#else
            ok &= horizon_gx2_terrain_draw(terrain, sequence[j],
                 w, h, 0, 0, w, h, identity, zero, zero, ranges) == 0;
#endif
         }
         ok &= readback(color, i);
         if (i == 0) WHBGfxFinishRenderTV();
         else WHBGfxFinishRenderDRC();
      }
      WHBGfxFinishRender();
   }
   if (front) horizon_gx2_terrain_section_destroy(front);
   if (rear) horizon_gx2_terrain_section_destroy(rear);
   if (sorted) horizon_gx2_terrain_section_destroy(sorted);
   if (terrain) horizon_gx2_terrain_destroy(terrain);
   OSReport("GX2 translucent fixture %s\n", ok ? "passed" : "failed");
   WHBGfxShutdown();
   WHBProcShutdown();
   return ok ? 0 : 1;
}
