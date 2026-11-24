#include <coreinit/debug.h>
#include <gx2/event.h>
#include <gx2/surface.h>
#include <gx2r/surface.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>
#include <whb/gfx.h>
#include <whb/proc.h>

typedef struct HorizonGx2Outline HorizonGx2Outline;
typedef struct OutlineVertex {
   float clip[4];
   float fog_distance[2];
   uint8_t rgba[4];
} OutlineVertex;

int horizon_gx2_outline_create(HorizonGx2Outline **out, uint32_t capacity);
void horizon_gx2_outline_destroy(HorizonGx2Outline *outline);
int horizon_gx2_outline_draw(HorizonGx2Outline *outline, uint32_t target, const uint32_t rect[4],
                             const OutlineVertex *vertices, uint32_t count, uint32_t secondary_count,
                             const float fog_color[4], const float fog_ranges[4]);

static void quad(OutlineVertex vertices[6], float x_min, float y_min, float x_max,
                 float y_max, float z, const uint8_t rgba[4])
{
   const float corners[4][4] = {
      {x_min, y_min, z, 1.0f}, {x_max, y_min, z, 1.0f},
      {x_min, y_max, z, 1.0f}, {x_max, y_max, z, 1.0f},
   };
   const unsigned index[6] = {0, 1, 2, 2, 1, 3};
   for (unsigned i = 0; i < 6; ++i) {
      vertices[i] = (OutlineVertex){0};
      memcpy(vertices[i].clip, corners[index[i]], sizeof(vertices[i].clip));
      memcpy(vertices[i].rgba, rgba, sizeof(vertices[i].rgba));
   }
}

static bool check_output(const GX2ColorBuffer *color, uint32_t output)
{
   const GX2Surface *src = &color->surface;
   GX2Surface copied = {
      .dim = GX2_SURFACE_DIM_TEXTURE_2D, .width = src->width, .height = src->height,
      .depth = 1, .mipLevels = 1, .format = src->format,
      .aa = GX2_AA_MODE1X, .tileMode = GX2_TILE_MODE_LINEAR_SPECIAL,
   };
   const GX2RResourceFlags flags = GX2R_RESOURCE_BIND_TEXTURE |
      GX2R_RESOURCE_USAGE_CPU_READ | GX2R_RESOURCE_USAGE_GPU_WRITE;
   if (!GX2RCreateSurface(&copied, flags)) return false;
   GX2DrawDone();
   GX2CopySurface(src, 0, 0, &copied, 0, 0);
   GX2DrawDone();
   const uint8_t *pixels = GX2RLockSurfaceEx(&copied, 0, flags);
   bool ok = pixels != NULL;
   if (pixels) {
      const uint32_t x = src->width / 4;
      const uint32_t y = src->height / 2;
      const size_t offsets[] = {
         (size_t)y * copied.pitch + src->width / 2,
         (size_t)y * copied.pitch + x,
         (size_t)(y + 2) * copied.pitch + x,
         (size_t)(y + 8) * copied.pitch + x,
      };
      const uint8_t expected[4][3] = {{255, 0, 0}, {87, 255, 225}, {0, 0, 0}, {255, 255, 255}};
      for (unsigned i = 0; i < 4; ++i) {
         const uint8_t *rgb = pixels + offsets[i] * 4;
         bool match = memcmp(rgb, expected[i], 3) == 0;
         OSReport("GX2 outline fixture output=%u sample=%u rgb=%u,%u,%u pass=%u\n",
                  output, i, rgb[0], rgb[1], rgb[2], match);
         ok &= match;
      }
      GX2RUnlockSurfaceEx(&copied, 0, flags);
   }
   GX2RDestroySurfaceEx(&copied, flags);
   return ok;
}

int main(void)
{
   WHBProcInit();
   if (!WHBGfxInit()) return 1;
   HorizonGx2Outline *blocker = NULL, *outline = NULL;
   bool ok = horizon_gx2_outline_create(&blocker, 6) == 0 &&
             horizon_gx2_outline_create(&outline, 12) == 0;
   if (ok) {
      WHBGfxBeginRender();
      for (uint32_t i = 0; i < 2; ++i) {
         if (i == 0) WHBGfxBeginRenderTV();
         else WHBGfxBeginRenderDRC();
         WHBGfxClearColor(1.0f, 1.0f, 1.0f, 1.0f);
         const GX2ColorBuffer *color = i == 0 ? WHBGfxGetTVColourBuffer() : WHBGfxGetDRCColourBuffer();
         const uint32_t rect[4] = {0, 0, color->surface.width, color->surface.height};
         const float height = rect[3];
         const float no_fog[4] = {0, 0, 0, 0}, ranges[4] = {1, 2, 1, 2};
         OutlineVertex occluder[6], strokes[12];
         quad(occluder, -0.2f, -0.2f, 0.2f, 0.2f, 0.25f, (uint8_t[4]){255, 0, 0, 255});
         quad(strokes, -0.8f, -7.0f / height, 0.8f, 7.0f / height, 0.5f,
              (uint8_t[4]){0, 0, 0, 255});
         quad(strokes + 6, -0.8f, -2.5f / height, 0.8f, 2.5f / height, 0.5f,
              (uint8_t[4]){87, 255, 225, 255});
         ok &= horizon_gx2_outline_draw(blocker, i, rect, occluder, 6, 0, no_fog, ranges) == 0;
         ok &= horizon_gx2_outline_draw(outline, i, rect, strokes, 12, 6, no_fog, ranges) == 0;
         ok &= check_output(color, i);
         if (i == 0) WHBGfxFinishRenderTV();
         else WHBGfxFinishRenderDRC();
      }
      WHBGfxFinishRender();
   }
   OSReport("GX2 outline fixture %s\n", ok ? "passed" : "failed");
   horizon_gx2_outline_destroy(outline);
   horizon_gx2_outline_destroy(blocker);
   WHBGfxShutdown();
   WHBProcShutdown();
   return ok ? 0 : 1;
}