#include "loading_host.h"
#include "swkbd_bridge.h"

#include <whb/gfx.h>
#include <gx2/event.h>
#include <gx2/surface.h>
#include <gx2r/surface.h>
#include <string.h>

extern const uint8_t _binary_hud_gsh_start[];
extern const uint8_t _binary_hud_gsh_end[];
extern const uint8_t _binary_panorama_gsh_start[];
extern const uint8_t _binary_panorama_gsh_end[];
extern const uint8_t _binary_terrain_gsh_start[];
extern const uint8_t _binary_terrain_gsh_end[];

const uint8_t *horizon_loading_host_hud_gfd(size_t *length)
{
   if (!length) return NULL;
   *length = (size_t)((uintptr_t)_binary_hud_gsh_end -
                      (uintptr_t)_binary_hud_gsh_start);
   return _binary_hud_gsh_start;
}

const uint8_t *horizon_loading_host_panorama_gfd(size_t *length)
{
   if (!length) return NULL;
   *length = (size_t)((uintptr_t)_binary_panorama_gsh_end -
                      (uintptr_t)_binary_panorama_gsh_start);
   return _binary_panorama_gsh_start;
}
const uint8_t *horizon_loading_host_terrain_gfd(size_t *length)
{
   if (!length) return NULL;
   *length = (size_t)((uintptr_t)_binary_terrain_gsh_end -
                      (uintptr_t)_binary_terrain_gsh_start);
   return _binary_terrain_gsh_start;
}

int horizon_loading_host_dimensions(uint32_t dimensions[4])
{
   if (!dimensions) return -1;
   GX2ColorBuffer *tv = WHBGfxGetTVColourBuffer();
   GX2ColorBuffer *drc = WHBGfxGetDRCColourBuffer();
   if (!tv || !drc || !tv->surface.width || !tv->surface.height ||
       !drc->surface.width || !drc->surface.height) return -2;
   dimensions[0] = tv->surface.width;
   dimensions[1] = tv->surface.height;
   dimensions[2] = drc->surface.width;
   dimensions[3] = drc->surface.height;
   return 0;
}

/* Match the verified diagnostic copy shape, but return the complete TV image
 * to a caller-provided buffer instead of sampling fixed diagnostic pixels. */
int horizon_loading_host_read_tv_rgba(uint8_t *out, size_t length)
{
   if (!out) return -1;
   GX2ColorBuffer *tv = WHBGfxGetTVColourBuffer();
   if (!tv) return -2;
   GX2Surface *src = &tv->surface;
   if (!src->width || !src->height ||
       src->format != GX2_SURFACE_FORMAT_UNORM_R8_G8_B8_A8 ||
       src->aa != GX2_AA_MODE1X || src->pitch < src->width) return -3;
   if ((size_t)src->pitch > SIZE_MAX / (size_t)src->height / 4 ||
       (size_t)src->imageSize < (size_t)src->pitch * src->height * 4) return -4;
   if ((size_t)src->width > SIZE_MAX / (size_t)src->height / 4) return -5;
   const size_t row_bytes = (size_t)src->width * 4;
   const size_t required = row_bytes * (size_t)src->height;
   if (length < required) return -6;

   GX2Surface dst = {0};
   dst.dim = GX2_SURFACE_DIM_TEXTURE_2D;
   dst.width = src->width;
   dst.height = src->height;
   dst.depth = 1;
   dst.mipLevels = 1;
   dst.format = src->format;
   dst.aa = GX2_AA_MODE1X;
   dst.tileMode = GX2_TILE_MODE_LINEAR_SPECIAL;
   const GX2RResourceFlags flags = GX2R_RESOURCE_BIND_TEXTURE |
      GX2R_RESOURCE_USAGE_CPU_READ | GX2R_RESOURCE_USAGE_GPU_WRITE;
   if (!GX2RCreateSurface(&dst, flags)) return -7;

   int result = 0;
   if (dst.pitch < dst.width ||
       (size_t)dst.pitch > SIZE_MAX / (size_t)dst.height / 4 ||
       (size_t)dst.imageSize < (size_t)dst.pitch * dst.height * 4) {
      result = -8;
      goto cleanup;
   }

   /* Ensure the TV draw has completed before copying, then ensure the copy
    * has completed before locking its CPU-readable destination. */
   GX2DrawDone();
   GX2CopySurface(src, 0, 0, &dst, 0, 0);
   GX2DrawDone();
   const uint8_t *pixels = GX2RLockSurfaceEx(&dst, 0, flags);
   if (!pixels) {
      result = -9;
      goto cleanup;
   }
   const size_t source_pitch = (size_t)dst.pitch * 4;
   for (uint32_t y = 0; y < dst.height; ++y) {
      memcpy(out + (size_t)y * row_bytes, pixels + (size_t)y * source_pitch, row_bytes);
   }
   GX2RUnlockSurfaceEx(&dst, 0, flags);

cleanup:
   GX2RDestroySurfaceEx(&dst, flags);
   return result;
}

int horizon_loading_host_present(HorizonLoadingDrawTarget draw, void *context)
{
   const float clear[4] = {0.0f, 0.0f, 0.0f, 1.0f};
   return horizon_loading_host_present_with_clear(draw, context, clear);
}

int horizon_loading_host_present_with_clear(HorizonLoadingDrawTarget draw, void *context,
                                            const float rgba[4])
{
   if (!draw || !rgba) return -1;
   uint32_t dimensions[4];
   if (horizon_loading_host_dimensions(dimensions) != 0) return -4;
   int result = 0;
   WHBGfxBeginRender();

   WHBGfxBeginRenderTV();
   WHBGfxClearColor(rgba[0], rgba[1], rgba[2], rgba[3]);
   if (draw(context, 0, dimensions[0], dimensions[1]) != 0 ||
       horizon_swkbd_draw(0) != 0) result = -2;
   WHBGfxFinishRenderTV();

   WHBGfxBeginRenderDRC();
   WHBGfxClearColor(rgba[0], rgba[1], rgba[2], rgba[3]);
   if (draw(context, 1, dimensions[2], dimensions[3]) != 0 ||
       horizon_swkbd_draw(1) != 0) result = -3;
   WHBGfxFinishRenderDRC();

   WHBGfxFinishRender();
   return result;
}
