#include "loading_host.h"
#include "swkbd_bridge.h"

#include <whb/gfx.h>
#include <gx2/surface.h>

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
