#include "terrain.h"

#include <coreinit/debug.h>
#include <gx2/event.h>
#include <gx2/surface.h>
#include <gx2r/surface.h>
#include <stdlib.h>
#include <string.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern const uint8_t _binary_terrain_gsh_start[];
extern const uint8_t _binary_opaque_gsh_start[];
extern const uint8_t _binary_clear_pixel_gsh_start[];
extern const uint8_t _binary_clear_vertex_gsh_start[];
extern const uint8_t _binary_missing_export_gsh_start[];

enum ShaderArm { OPAQUE_CONTROL, CLEAR_PIXEL, CLEAR_VERTEX };
enum { ATLAS_WIDTH = 16, ATLAS_HEIGHT = 4, LIGHT_SIDE = 16 };

static void quad(HorizonTerrainVertex *out, float left, float top, float depth,
                 unsigned layer, uint8_t cutout, uint8_t red, uint8_t green, uint8_t blue)
{
   const float points[4][2] = {
      {left, top - 1}, {left + 1, top - 1}, {left + 1, top}, {left, top},
   };
   for (unsigned i = 0; i < 4; ++i) {
      out[i] = (HorizonTerrainVertex){
         .position = {points[i][0], points[i][1], depth, (float)(i * 16 * 257)},
         .uv = {(float)(16384 | layer << 16), 16384},
         .color_cutout = {red, green, blue, cutout},
      };
   }
}

static int fill(void *source, HorizonTerrainVertex *vertices, uint32_t count)
{
   memcpy(vertices, source, count * sizeof(*vertices));
   return 0;
}

static bool expected_samples(const GX2Surface *surface, const uint8_t *pixels)
{
   const uint8_t expected[4][4] = {
      {0, 255, 0, 127}, {0, 255, 255, 255}, {255, 0, 0, 0}, {0, 0, 255, 128},
   };
   for (unsigned i = 0; i < 4; ++i) {
      const unsigned x = (i & 1 ? 3 : 1) * surface->width / 4;
      const unsigned y = (i & 2 ? 3 : 1) * surface->height / 4;
      const uint8_t *pixel = pixels + ((size_t)y * surface->pitch + x) * 4;
      if (memcmp(pixel, expected[i], 4)) return false;
   }
   return true;
}

static bool check_output(const GX2ColorBuffer *buffer, unsigned output, enum ShaderArm arm,
                         bool varying_light, uint8_t **baseline)
{
   const GX2Surface *source = &buffer->surface;
   GX2Surface copy = {
      .dim = GX2_SURFACE_DIM_TEXTURE_2D,
      .width = source->width,
      .height = source->height,
      .depth = 1,
      .mipLevels = 1,
      .format = source->format,
      .aa = GX2_AA_MODE1X,
      .tileMode = GX2_TILE_MODE_LINEAR_SPECIAL,
   };
   const GX2RResourceFlags flags = GX2R_RESOURCE_BIND_TEXTURE |
      GX2R_RESOURCE_USAGE_CPU_READ | GX2R_RESOURCE_USAGE_GPU_WRITE;
   if (!GX2RCreateSurface(&copy, flags)) return false;
   GX2DrawDone();
   GX2CopySurface(source, 0, 0, &copy, 0, 0);
   GX2DrawDone();
   const uint8_t *pixels = GX2RLockSurfaceEx(&copy, 0, flags);
   bool ok = pixels != NULL;
   if (pixels) {
      const size_t row_size = (size_t)source->width * 4;
      if (arm == OPAQUE_CONTROL) *baseline = malloc(source->height * row_size);
      ok = *baseline != NULL;
      for (uint32_t y = 0; ok && y < source->height; ++y) {
         const uint8_t *row = pixels + (size_t)y * copy.pitch * 4;
         uint8_t *saved = *baseline + (size_t)y * row_size;
         if (arm == OPAQUE_CONTROL) memcpy(saved, row, row_size);
         else ok = memcmp(saved, row, row_size) == 0;
      }
      if (!varying_light) ok &= expected_samples(&copy, pixels);
      GX2RUnlockSurfaceEx(&copy, 0, flags);
   }
   GX2RDestroySurfaceEx(&copy, flags);
   OSReport("GX2 opaque screen output=%u variant=%u light=%u bytes=%u result=%s\n",
            output, (unsigned)arm, (unsigned)varying_light,
            source->width * source->height * 4, ok ? "PASS" : "FAIL");
   return ok;
}

static bool render(HorizonGx2Terrain *terrain, HorizonGx2TerrainSection *opaque,
                   HorizonGx2TerrainSection *mixed, enum ShaderArm arm, bool varying_light,
                   uint8_t *baseline[2])
{
   _Alignas(16) float clip[16] = {1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1};
   const float origin[4] = {0};
   const float fog_color[4] = {0.125f, 0.35f, 0.75f, 1.0f};
   const float ranges[4] = {10, 20, 10, 20};
   bool ok = true;
   WHBGfxBeginRender();
   for (unsigned output = 0; output < 2; ++output) {
      if (output) WHBGfxBeginRenderDRC();
      else WHBGfxBeginRenderTV();
      WHBGfxClearColor(0, 0, 0, 1);
      const GX2ColorBuffer *color = output ? WHBGfxGetDRCColourBuffer() : WHBGfxGetTVColourBuffer();
      const uint32_t width = color->surface.width;
      const uint32_t height = color->surface.height;
      clip[0] = output ? 0.94f : 1.0f;
      ok &= horizon_gx2_terrain_bind_variant(terrain, width, height, 0, 0, width, height,
                                            fog_color, ranges, false, true) == 0;
      if (arm != OPAQUE_CONTROL)
         ok &= horizon_gx2_terrain_set_clear_fog(terrain, true, fog_color, ranges);
      ok &= horizon_gx2_terrain_draw_bound(terrain, opaque, clip, origin) == 0;
      if (arm != OPAQUE_CONTROL)
         ok &= horizon_gx2_terrain_set_clear_fog(terrain, false, fog_color, ranges);
      ok &= horizon_gx2_terrain_bind(terrain, width, height, 0, 0, width, height,
                                    fog_color, ranges, false) == 0;
      ok &= horizon_gx2_terrain_draw_bound(terrain, mixed, clip, origin) == 0;
      ok &= check_output(color, output, arm, varying_light, &baseline[output]);
      if (output) WHBGfxFinishRenderDRC();
      else WHBGfxFinishRenderTV();
   }
   WHBGfxFinishRender();
   return ok;
}

static bool init_terrain(HorizonGx2Terrain **out, const uint8_t *atlas, const uint8_t *light)
{
   return horizon_gx2_terrain_create(out, _binary_terrain_gsh_start, ATLAS_WIDTH, ATLAS_HEIGHT,
                                     atlas, ATLAS_WIDTH * ATLAS_HEIGHT * 4) == 0 &&
      horizon_gx2_terrain_set_sprite_grid(*out, 4, 4) == 0 &&
      horizon_gx2_terrain_lightmap(*out, light) == 0 &&
      !horizon_gx2_terrain_load_opaque_shader(*out, _binary_terrain_gsh_start) &&
      horizon_gx2_terrain_load_opaque_shader(*out, _binary_opaque_gsh_start) &&
      horizon_gx2_terrain_load_clear_fog_shader(*out, _binary_clear_pixel_gsh_start);
}

int main(void)
{
   WHBProcInit();
   const bool graphics = WHBGfxInit();
   HorizonGx2Terrain *terrain = NULL, *optimized = NULL;
   HorizonGx2TerrainSection *opaque = NULL, *mixed = NULL;
   uint8_t atlas[ATLAS_WIDTH * ATLAS_HEIGHT * 4], light[LIGHT_SIDE * LIGHT_SIDE * 4];
   const uint8_t alphas[4] = {0, 127, 128, 255};
   memset(light, 255, sizeof(light));
   for (unsigned i = 0; i < ATLAS_WIDTH * ATLAS_HEIGHT; ++i)
      memcpy(atlas + i * 4, (uint8_t[]){255, 255, 255, alphas[(i % ATLAS_WIDTH) / 4]}, 4);
   HorizonTerrainVertex front[8], background[24];
   quad(front,     -1, 1, 0.2f, 1,   0,   0, 255,   0);
   quad(front + 4, -1, 0, 0.2f, 0,   0, 255,   0,   0);
   quad(background,      -1, 1, 0.6f, 3,   0, 255, 255, 255);
   quad(background +  4, -1, 0, 0.6f, 3,   0, 255, 255, 255);
   quad(background +  8,  0, 1, 0.2f, 1, 255, 255,   0,   0);
   quad(background + 12,  0, 1, 0.6f, 3,   0,   0, 255, 255);
   quad(background + 16,  0, 0, 0.2f, 2, 255,   0,   0, 255);
   quad(background + 20,  0, 0, 0.6f, 3,   0, 255, 255, 255);

   bool ok = graphics && init_terrain(&terrain, atlas, light) && init_terrain(&optimized, atlas, light) &&
      !horizon_gx2_terrain_load_clear_fog_vertex(optimized, _binary_missing_export_gsh_start) &&
      horizon_gx2_terrain_load_clear_fog_vertex(optimized, _binary_clear_vertex_gsh_start);
   OSReport("GX2 clear vertex loader guard=%s\n", ok ? "PASS" : "FAIL");
   uint32_t allocation;
   ok = ok && horizon_gx2_terrain_section_create(&opaque, 8, fill, front, &allocation) == 0 &&
      horizon_gx2_terrain_section_create(&mixed, 24, fill, background, &allocation) == 0;
   for (unsigned varying = 0; ok && varying < 2; ++varying) {
      if (varying) {
         for (unsigned y = 0; y < LIGHT_SIDE; ++y) {
            for (unsigned x = 0; x < LIGHT_SIDE; ++x) {
               uint8_t *pixel = light + (y * LIGHT_SIDE + x) * 4;
               pixel[0] = 32 + y * 13;
               pixel[1] = 255 - x * 13;
               pixel[2] = 96 + x * 7;
            }
         }
         ok = horizon_gx2_terrain_lightmap(terrain, light) == 0 &&
            horizon_gx2_terrain_lightmap(optimized, light) == 0;
      }
      uint8_t *baseline[2] = {0};
      if (ok) {
         ok = render(terrain, opaque, mixed, OPAQUE_CONTROL, varying, baseline);
         ok &= render(terrain, opaque, mixed, CLEAR_PIXEL, varying, baseline);
         ok &= render(optimized, opaque, mixed, CLEAR_VERTEX, varying, baseline);
      }
      free(baseline[0]);
      free(baseline[1]);
   }

   horizon_gx2_terrain_section_destroy(opaque);
   horizon_gx2_terrain_section_destroy(mixed);
   horizon_gx2_terrain_destroy(optimized);
   horizon_gx2_terrain_destroy(terrain);
   if (graphics) WHBGfxShutdown();
   WHBProcShutdown();
   OSReport("GX2 clear vertex fixture result=%s\n", ok ? "PASS" : "FAIL");
   return ok ? 0 : 1;
}
