#include <coreinit/debug.h>
#include <gx2/event.h>
#include <gx2/surface.h>
#include <gx2/texture.h>
#include <gx2/utils.h>
#include <gx2r/surface.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>
#include <whb/gfx.h>
#include <whb/proc.h>

typedef struct HorizonGx2Crumbling HorizonGx2Crumbling;
typedef struct CrumblingVertex {
   float position[3], uv[2], sprite[4];
} CrumblingVertex;

int horizon_gx2_crumbling_create(HorizonGx2Crumbling **out);
void horizon_gx2_crumbling_destroy(HorizonGx2Crumbling *crumbling);
int horizon_gx2_crumbling_upload(HorizonGx2Crumbling *crumbling, uint32_t target,
                                 const CrumblingVertex *first, uint32_t first_count,
                                 const CrumblingVertex *second, uint32_t second_count);
int horizon_gx2_crumbling_draw(HorizonGx2Crumbling *crumbling, uint32_t target,
                              const uint32_t rect[4], const GX2Texture *atlas,
                              const float clip_rows[16], const float fog_color[4],
                              const float fog_ranges[4], uint32_t start, uint32_t count);

static const GX2RResourceFlags atlas_flags = GX2R_RESOURCE_BIND_TEXTURE |
   GX2R_RESOURCE_USAGE_CPU_WRITE | GX2R_RESOURCE_USAGE_GPU_READ;

static bool prepare_atlas(GX2Texture *texture)
{
   GX2Surface *surface = &texture->surface;
   *surface = (GX2Surface){
      .dim = GX2_SURFACE_DIM_TEXTURE_2D, .width = 4, .height = 2, .depth = 1, .mipLevels = 1,
      .format = GX2_SURFACE_FORMAT_UNORM_R8_G8_B8_A8, .aa = GX2_AA_MODE1X,
      .use = GX2_SURFACE_USE_TEXTURE, .tileMode = GX2_TILE_MODE_LINEAR_ALIGNED,
   };
   if (!GX2RCreateSurface(surface, atlas_flags)) return false;
   uint8_t *pixels = GX2RLockSurfaceEx(surface, 0, atlas_flags);
   if (!pixels) return false;
   const uint8_t row[16] = {64, 64, 64, 255, 0, 0, 0, 0, 96, 32, 64, 255, 96, 32, 64, 255};
   for (uint32_t y = 0; y < surface->height; ++y)
      memcpy(pixels + (size_t)y * surface->pitch * 4, row, sizeof(row));
   GX2RUnlockSurfaceEx(surface, 0, atlas_flags);
   texture->viewNumMips = texture->viewNumSlices = 1;
   texture->compMap = GX2_COMP_MAP(GX2_SQ_SEL_R, GX2_SQ_SEL_G, GX2_SQ_SEL_B, GX2_SQ_SEL_A);
   GX2InitTextureRegs(texture);
   return true;
}

static void quad(CrumblingVertex vertices[6], unsigned stage)
{
   const float left = stage ? 0.0f : -1.0f;
   const float from_u = stage ? 0.0f : -1.0f;
   const float corners[4][4] = {
      {left, -0.75f, from_u, 0}, {left + 1, -0.75f, 1, 0},
      {left + 1, 0.75f, 1, 1}, {left, 0.75f, from_u, 1},
   };
   const unsigned index[6] = {0, 1, 2, 0, 2, 3};
   for (unsigned i = 0; i < 6; ++i) {
      const float *c = corners[index[i]];
      vertices[i] = (CrumblingVertex){
         .position = {c[0], c[1], 0.5f}, .uv = {c[2], c[3]},
         .sprite = {stage ? 32768.0f / 65535.0f : 0.0f, 0.0f, 0.5f, 1.0f},
      };
   }
}

static bool samples(const GX2ColorBuffer *color, uint32_t output)
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
      const size_t row = (size_t)(src->height / 2) * copied.pitch;
      const uint32_t offsets[] = {src->width / 16, src->width * 3 / 16,
                                  src->width * 5 / 16, src->width * 3 / 4};
      const uint8_t expected[4][3] = {{128, 128, 128}, {255, 255, 255}, {128, 128, 128}, {192, 64, 128}};
      for (unsigned i = 0; i < 4; ++i) {
         const uint8_t *rgb = pixels + (row + offsets[i]) * 4;
         const bool match = memcmp(rgb, expected[i], 3) == 0;
         OSReport("GX2 crumbling fixture output=%u sample=%u rgb=%u,%u,%u pass=%u\n",
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
   HorizonGx2Crumbling *crumbling = NULL;
   GX2Texture atlas = {0};
   bool ok = horizon_gx2_crumbling_create(&crumbling) == 0 && prepare_atlas(&atlas);
   OSReport("GX2 crumbling fixture init=%u\n", ok);
   if (ok) {
      const float identity[16] = {1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1};
      const float fog[4] = {0, 0, 0, 0}, ranges[4] = {1, 2, 1, 2};
      CrumblingVertex pre[6], post[6];
      quad(pre, 0);
      quad(post, 1);
      WHBGfxBeginRender();
      for (uint32_t i = 0; i < 2; ++i) {
         if (i == 0) WHBGfxBeginRenderTV();
         else WHBGfxBeginRenderDRC();
         WHBGfxClearColor(1, 1, 1, 1);
         const GX2ColorBuffer *color = i == 0 ? WHBGfxGetTVColourBuffer() : WHBGfxGetDRCColourBuffer();
         const uint32_t rect[4] = {0, 0, color->surface.width, color->surface.height};
         ok &= horizon_gx2_crumbling_upload(crumbling, i, pre, 6, post, 6) == 0;
         ok &= horizon_gx2_crumbling_draw(crumbling, i, rect, &atlas, identity, fog, ranges, 0, 6) == 0;
         ok &= horizon_gx2_crumbling_draw(crumbling, i, rect, &atlas, identity, fog, ranges, 6, 6) == 0;
         ok &= samples(color, i);
         if (i == 0) WHBGfxFinishRenderTV();
         else WHBGfxFinishRenderDRC();
      }
      WHBGfxFinishRender();
   }
   OSReport("GX2 crumbling fixture %s\n", ok ? "passed" : "failed");
   horizon_gx2_crumbling_destroy(crumbling);
   if (atlas.surface.image) GX2RDestroySurfaceEx(&atlas.surface, atlas_flags);
   WHBGfxShutdown();
   WHBProcShutdown();
   return ok ? 0 : 1;
}