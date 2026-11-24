#include <gx2/mem.h>
#include <gx2/state.h>

enum TransitionMode { RC26_ORDER, BEFORE_RESTORE, SPLIT_SYNC, DRAINED_SYNC, COPY_TEXTURE };
static enum TransitionMode transition_mode;

static void composite_invalidate(GX2InvalidateMode mode, void *address, uint32_t bytes)
{
   if (transition_mode == RC26_ORDER) GX2Invalidate(mode, address, bytes);
   if (transition_mode == SPLIT_SYNC)
      GX2Invalidate(GX2_INVALIDATE_MODE_TEXTURE, address, bytes);
}

/* Reuse the simple pattern draw, while interposing only the synchronization
 * call in the compiled production composite. */
#define GX2Invalidate composite_invalidate
#define main world_scale_pixel_fixture_main
#include "../world_scale/main.c"
#undef main
#undef GX2Invalidate

static bool reversed_texture(GX2Texture *pattern)
{
   if (!texture(pattern)) return false;
   const GX2RResourceFlags flags = GX2R_RESOURCE_BIND_TEXTURE | GX2R_RESOURCE_USAGE_CPU_WRITE |
      GX2R_RESOURCE_USAGE_GPU_READ;
   uint8_t *pixels = GX2RLockSurfaceEx(&pattern->surface, 0, flags);
   if (!pixels) {
      GX2RDestroySurfaceEx(&pattern->surface, 0);
      return false;
   }
   const uint8_t blue[] = {0, 0, 255, 255}, red[] = {255, 0, 0, 255};
   memcpy(pixels, blue, sizeof(blue));
   memcpy(pixels + pattern->surface.pitch * 4, red, sizeof(red));
   GX2RUnlockSurfaceEx(&pattern->surface, 0, flags);
   return true;
}

#include "transitions.h"

int main(void)
{
   WHBProcInit();
   if (!WHBGfxInit()) {
      WHBProcShutdown();
      return 1;
   }
   GX2Texture pattern = {0};
   const bool ready = texture(&pattern);
   GX2Surface *outputs[] = {&WHBGfxGetTVColourBuffer()->surface, &WHBGfxGetDRCColourBuffer()->surface};
   const bool ok = ready && check_transitions(outputs, &pattern);
   if (pattern.surface.image) GX2RDestroySurfaceEx(&pattern.surface, 0);
   OSReport("GX2 scale transition diagnostic done result=%s\n", ok ? "PASS" : "FAIL");
   WHBGfxShutdown();
   WHBProcShutdown();
   return ok ? 0 : 3;
}
