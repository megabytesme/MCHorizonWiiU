#include "lightning.h"

#include <coreinit/debug.h>
#include <coreinit/thread.h>
#include <coreinit/time.h>
#include <string.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern const uint8_t _binary_terrain_gsh_start[];

#define QUAD(left, right, z, r, g, b, a) \
   {{left, -.40f, z}, {r, g, b, a}}, {{left, .40f, z}, {r, g, b, a}}, \
   {{right, .40f, z}, {r, g, b, a}}, {{right, -.40f, z}, {r, g, b, a}}

static const HorizonGx2LightningVertex vertices[] = {
   QUAD(-.75f, -.25f, .2f, 200, 0, 0, 128),
   QUAD(-.75f, -.25f, .5f, 0, 0, 200, 128),
   QUAD(.25f, .75f, .8f, 255, 255, 0, 255),
};

static int fill_depth(void *ctx, HorizonTerrainVertex *out, uint32_t count)
{
   (void)ctx;
   const HorizonTerrainVertex vertices[] = {
      {{-1, -1, .6f, 61680}, {.5f, .5f}, {16, 24, 32, 0}},
      {{-1,  1, .6f, 61680}, {.5f, .5f}, {16, 24, 32, 0}},
      {{ 1,  1, .6f, 61680}, {.5f, .5f}, {16, 24, 32, 0}},
      {{ 1, -1, .6f, 61680}, {.5f, .5f}, {16, 24, 32, 0}},
   };
   if (count != sizeof(vertices) / sizeof(vertices[0])) return -1;
   memcpy(out, vertices, sizeof(vertices));
   return 0;
}

static bool draw_both(HorizonGx2Terrain *terrain, HorizonGx2TerrainSection *depth,
                      HorizonGx2Lightning *lightning)
{
   bool ok = true;
   WHBGfxBeginRender();
   for (uint32_t output = 0; output < 2; ++output) {
      if (output == 0) WHBGfxBeginRenderTV();
      else WHBGfxBeginRenderDRC();

      WHBGfxClearColor(16.0f / 255.0f, 24.0f / 255.0f, 32.0f / 255.0f, 1.0f);
      const GX2ColorBuffer *color =
         output == 0 ? WHBGfxGetTVColourBuffer() : WHBGfxGetDRCColourBuffer();
      const uint32_t rect[] = {0, 0, color->surface.width, color->surface.height};
      const float rows[4][4] = {
         {output == 0 ? 1.0f : -1.0f, 0, 0, 0}, {0, 1, 0, 0},
         {0, 0, 1, 0}, {0, 0, 0, 1},
      };
      float terrain_rows[16];
      memcpy(terrain_rows, rows, sizeof(rows));
      const float origin[4] = {0}, fog[4] = {0}, range[4] = {1, 2, 1, 2};
      ok &= horizon_gx2_terrain_draw(terrain, depth, rect[2], rect[3],
                 rect[0], rect[1], rect[2], rect[3], terrain_rows, origin, fog, range) == 0;
      ok &= horizon_gx2_lightning_draw(lightning, terrain, rect, rows) == 0;
      if (output == 0) WHBGfxFinishRenderTV();
      else WHBGfxFinishRenderDRC();
   }
   WHBGfxFinishRender();
   return ok;
}

int main(void)
{
   WHBProcInit();
   if (!WHBGfxInit()) {
      WHBProcShutdown();
      return 1;
   }
   HorizonGx2Lightning *lightning = NULL;
   HorizonGx2Terrain *terrain = NULL;
   HorizonGx2TerrainSection *depth = NULL;
   const uint8_t rgba[4] = {255, 255, 255, 255};
   uint8_t lightmap[16 * 16 * 4];
   memset(lightmap, 255, sizeof(lightmap));
   uint32_t depth_bytes;
   bool ok = horizon_gx2_terrain_create(&terrain, _binary_terrain_gsh_start, 1, 1,
                                      rgba, sizeof(rgba)) == 0 &&
      horizon_gx2_terrain_lightmap(terrain, lightmap) == 0 &&
      horizon_gx2_terrain_section_create(&depth, 4, fill_depth, NULL, &depth_bytes) == 0 &&
      horizon_gx2_lightning_create(&lightning) == 0 &&
      horizon_gx2_lightning_upload(lightning, vertices, sizeof(vertices) / sizeof(vertices[0])) == 0 &&
      draw_both(terrain, depth, lightning);
   if (ok) {
      OSReport("GX2 lightning visual ready\n");
      const OSTime deadline = OSGetSystemTime() + OSMillisecondsToTicks(7000);
      while (ok && WHBProcIsRunning() && OSGetSystemTime() < deadline) {
         ok = draw_both(terrain, depth, lightning);
         OSSleepTicks(OSMillisecondsToTicks(16));
      }
   }
   OSReport("GX2 lightning fixture result=%s\n", ok ? "SUBMITTED" : "FAILED");
   horizon_gx2_lightning_destroy(lightning);
   if (depth) horizon_gx2_terrain_section_destroy(depth);
   if (terrain) horizon_gx2_terrain_destroy(terrain);
   WHBGfxShutdown();
   WHBProcShutdown();
   return ok ? 0 : 1;
}
