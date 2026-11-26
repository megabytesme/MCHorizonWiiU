#include "terrain.h"
#include <coreinit/debug.h>
#include <gx2/event.h>
#include <gx2/registers.h>
#include <gx2/surface.h>
#include <gx2r/surface.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern const uint8_t _binary_terrain_gsh_start[];
enum { COLUMNS = 4, CELLS = 8 };

static void append_quad(HorizonTerrainVertex **next, unsigned cell, float depth,
                        bool back, bool cutout, bool alpha_above_threshold,
                        const uint8_t tint[3])
{
   const float left = -0.95f + (float)(cell % COLUMNS) * 0.5f;
   const float bottom = cell < COLUMNS ? 0.05f : -0.95f;
   const float corners[4][2] = {
      {left, bottom}, {left + 0.4f, bottom},
      {left + 0.4f, bottom + 0.9f}, {left, bottom + 0.9f},
   };
   for (unsigned i = 0; i < 4; ++i) {
      const unsigned corner = back ? 3 - i : i;
      (*next)[i] = (HorizonTerrainVertex){
         .position = {corners[corner][0], corners[corner][1], depth, 0.0f},
         .uv = {alpha_above_threshold ? 0.75f : 0.25f, 0.5f},
         .color_cutout = {tint[0], tint[1], tint[2], cutout ? 255 : 0},
      };
   }
   *next += 4;
}

static int fill(void *source, HorizonTerrainVertex *vertices, uint32_t count)
{
   memcpy(vertices, source, count * sizeof(*vertices));
   return 0;
}

static bool readback(const GX2ColorBuffer *buffer, unsigned output, bool culling)
{
   static const uint8_t expected[2][CELLS][3] = {
      {
         {0, 255, 0}, {255, 0, 0}, {0, 0, 255}, {255, 255, 0},
         {0, 255, 255}, {0, 255, 255}, {255, 0, 255}, {0, 255, 0},
      },
      {
         {0, 255, 0}, {0, 0, 0}, {0, 0, 255}, {0, 0, 0},
         {0, 255, 255}, {0, 255, 255}, {255, 0, 255}, {0, 255, 0},
      },
   };
   const GX2Surface *source = &buffer->surface;
   GX2Surface staging = {
      .dim = GX2_SURFACE_DIM_TEXTURE_2D, .width = source->width, .height = source->height,
      .depth = 1, .mipLevels = 1, .format = source->format, .aa = GX2_AA_MODE1X,
      .tileMode = GX2_TILE_MODE_LINEAR_SPECIAL,
   };
   const GX2RResourceFlags flags = GX2R_RESOURCE_BIND_TEXTURE |
      GX2R_RESOURCE_USAGE_CPU_READ | GX2R_RESOURCE_USAGE_GPU_WRITE;
   if (!GX2RCreateSurface(&staging, flags)) return false;
   GX2DrawDone();
   GX2CopySurface(source, 0, 0, &staging, 0, 0);
   GX2DrawDone();
   const uint8_t *pixels = GX2RLockSurfaceEx(&staging, 0, flags);
   bool ok = pixels != NULL;
   if (pixels) {
      for (unsigned cell = 0; cell < CELLS; ++cell) {
         const unsigned x = ((cell % COLUMNS) * 2 + 1) * source->width / (2 * COLUMNS);
         const unsigned y = (cell < COLUMNS ? 1 : 3) * source->height / 4;
         const uint8_t *rgb = pixels + ((size_t)y * staging.pitch + x) * 4;
         const bool match = !memcmp(rgb, expected[culling][cell], 3);
         OSReport("GX2 terrain winding output=%u cull=%u cell=%u rgb=%u,%u,%u result=%s\n",
                  output, (unsigned)culling, cell, rgb[0], rgb[1], rgb[2], match ? "PASS" : "FAIL");
         ok &= match;
      }
      GX2RUnlockSurfaceEx(&staging, 0, flags);
   }
   GX2RDestroySurfaceEx(&staging, flags);
   return ok;
}

static bool render(HorizonGx2Terrain *terrain, HorizonGx2TerrainSection *section, bool culling)
{
   const float identity[16] = {1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1};
   const float zero[4] = {0}, ranges[4] = {10, 20, 10, 20};
   bool ok = true;
   WHBGfxBeginRender();
   for (unsigned output = 0; output < 2; ++output) {
      if (output == 0) WHBGfxBeginRenderTV();
      else WHBGfxBeginRenderDRC();
      WHBGfxClearColor(0, 0, 0, 1);
      GX2ColorBuffer *color = output == 0 ? WHBGfxGetTVColourBuffer() : WHBGfxGetDRCColourBuffer();
      const uint32_t w = color->surface.width, h = color->surface.height;
      if (horizon_gx2_terrain_bind(terrain, w, h, 0, 0, w, h, zero, ranges, false) == 0) {
         GX2SetCullOnlyControl(GX2_FRONT_FACE_CCW, FALSE, culling);
         ok &= horizon_gx2_terrain_draw_bound(terrain, section, identity, zero) == 0;
         ok &= readback(color, output, culling);
      } else {
         ok = false;
      }
      if (output == 0) WHBGfxFinishRenderTV();
      else WHBGfxFinishRenderDRC();
   }
   WHBGfxFinishRender();
   return ok;
}

static bool exercise(void)
{
   const uint8_t atlas[8] = {255, 255, 255, 127, 255, 255, 255, 128};
   const uint8_t red[3] = {255, 0, 0}, green[3] = {0, 255, 0}, blue[3] = {0, 0, 255};
   const uint8_t yellow[3] = {255, 255, 0}, cyan[3] = {0, 255, 255}, magenta[3] = {255, 0, 255};
   uint8_t lightmap[16 * 16 * 4];
   memset(lightmap, 255, sizeof(lightmap));

   HorizonTerrainVertex vertices[48], *next = vertices;
   append_quad(&next, 0, 0.3f, false, false, false, green);
   append_quad(&next, 1, 0.3f, true, false, false, red);
   append_quad(&next, 2, 0.3f, false, true, true, blue);
   append_quad(&next, 3, 0.3f, true, true, true, yellow);
   // A discarded near cutout must not hide the later opaque background.
   append_quad(&next, 4, 0.2f, false, true, false, red);
   append_quad(&next, 5, 0.2f, true, true, false, red);
   append_quad(&next, 4, 0.5f, false, false, false, cyan);
   append_quad(&next, 5, 0.5f, false, false, false, cyan);
   // Assets that emit both windings keep their front face whichever way the pair is ordered.
   append_quad(&next, 6, 0.3f, false, true, true, magenta);
   append_quad(&next, 6, 0.3f, true, true, true, magenta);
   append_quad(&next, 7, 0.3f, true, false, false, green);
   append_quad(&next, 7, 0.3f, false, false, false, green);

   HorizonGx2Terrain *terrain = NULL;
   HorizonGx2TerrainSection *section = NULL;
   uint32_t bytes;
   bool ok = horizon_gx2_terrain_create(&terrain, _binary_terrain_gsh_start, 2, 1, atlas, sizeof(atlas)) == 0 &&
      horizon_gx2_terrain_lightmap(terrain, lightmap) == 0 &&
      horizon_gx2_terrain_section_create(&section, (uint32_t)(next - vertices), fill, vertices, &bytes) == 0;
   if (ok) {
      ok = render(terrain, section, false);
      ok &= render(terrain, section, true);
   }
   if (section) horizon_gx2_terrain_section_destroy(section);
   if (terrain) horizon_gx2_terrain_destroy(terrain);
   return ok;
}

int main(void)
{
   WHBProcInit();
   const bool graphics = WHBGfxInit();
   const bool ok = graphics && exercise();
   if (graphics) WHBGfxShutdown();
   WHBProcShutdown();
   OSReport("GX2 terrain winding diagnostic result=%s\n", ok ? "PASS" : "FAIL");
   return ok ? 0 : 1;
}
