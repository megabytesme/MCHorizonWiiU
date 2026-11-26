#pragma once

#include <coreinit/cache.h>
#include <coreinit/debug.h>
#include <gx2/event.h>
#include <gx2/mem.h>
#include <gx2/state.h>
#include <gx2/surface.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

enum {
   CACHE_LINE = 32,
   STAGE_ALIGN = 4096,
   STAGE_CAP = 9 * 1024 * 1024,
   RGBA_CAP = 1920 * 1080 * 4,
   GUARD = 0xa5,
};

typedef struct {
   uint8_t *storage;
   bool copy_pending;
} Stage;

enum ReadStatus {
   READ_OK, READ_SOURCE, READ_LENGTH, READ_LAYOUT, READ_BUSY, READ_PRODUCER,
   READ_COPY_FENCE, READ_COPY_GUARD,
};

static bool guard_matches(const uint8_t *bytes)
{
   for (unsigned i = 0; i < CACHE_LINE; ++i)
      if (bytes[i] != GUARD) return false;
   return true;
}

static bool finish_commands(OSTime *submitted, OSTime *retired)
{
   GX2Flush();
   *submitted = GX2GetLastSubmittedTimeStamp();
   // Cemu's GX2DrawDone import has no return value. It still supplies Vulkan's full sync.
   GX2DrawDone();
   *retired = GX2GetRetiredTimeStamp();
   return *submitted > 0 && *retired >= *submitted;
}

static enum ReadStatus stage_read(Stage *stage, const GX2ColorBuffer *color,
                                  uint8_t *rgba, size_t length)
{
   if (stage->copy_pending) return READ_BUSY;
   const GX2Surface *src = &color->surface;
   if (!src->image || !src->width || !src->height ||
       src->width > 1920 || src->height > 1080 ||
       src->dim != GX2_SURFACE_DIM_TEXTURE_2D ||
       src->depth != 1 || src->pitch < src->width ||
       src->aa != GX2_AA_MODE1X || color->viewMip || color->viewFirstSlice ||
       color->viewNumSlices != 1 ||
       src->format != GX2_SURFACE_FORMAT_UNORM_R8_G8_B8_A8 ||
       (uint64_t)src->pitch * src->height * 4 > src->imageSize) return READ_SOURCE;
   const size_t row_bytes = src->width * 4;
   if (!rgba || length < row_bytes * src->height) return READ_LENGTH;

   GX2Surface dst = {
      .dim = GX2_SURFACE_DIM_TEXTURE_2D, .width = src->width, .height = src->height,
      .depth = 1, .mipLevels = 1, .format = src->format, .aa = GX2_AA_MODE1X,
      .use = GX2_SURFACE_USE_TEXTURE, .tileMode = GX2_TILE_MODE_LINEAR_SPECIAL,
   };
   GX2CalcSurfaceSizeAndAlignment(&dst);
   if (!dst.alignment || STAGE_ALIGN % dst.alignment ||
       dst.tileMode != GX2_TILE_MODE_LINEAR_SPECIAL || dst.mipmapSize ||
       dst.pitch < dst.width || (uint64_t)dst.pitch * dst.height * 4 > dst.imageSize ||
       dst.imageSize > STAGE_CAP - STAGE_ALIGN - 2 * CACHE_LINE) return READ_LAYOUT;

   uint8_t *pixels = stage->storage + STAGE_ALIGN;
   dst.image = pixels;
   const uint32_t cache_span = (dst.imageSize + CACHE_LINE - 1) & ~(CACHE_LINE - 1);
   uint8_t *guard_start = pixels - CACHE_LINE;
   uint8_t *guard_end = pixels + cache_span;
   const uint32_t checked_span = cache_span + 2 * CACHE_LINE;
   memset(guard_start, GUARD, checked_span);
   DCFlushRange(guard_start, checked_span);
   // Prime one cached line so the post-copy invalidation is exercised.
   const volatile uint8_t old_first_byte = *(volatile uint8_t *)pixels;
   if (old_first_byte != GUARD) return READ_COPY_GUARD;

   // Order the producer's color writes before the copy.
   GX2Invalidate(GX2_INVALIDATE_MODE_COLOR_BUFFER, src->image, src->imageSize);
   OSTime submitted, retired;
   if (!finish_commands(&submitted, &retired)) return READ_PRODUCER;
   const OSTime before = GX2GetLastSubmittedTimeStamp();
   stage->copy_pending = true;
   GX2CopySurface(src, 0, 0, &dst, 0, 0);
   const bool done = finish_commands(&submitted, &retired);
   OSReport("GX2 stage fence before=%lld submitted=%lld retired=%lld complete=%u bytes=%u pitch=%u\n",
            (long long)before, (long long)submitted, (long long)retired,
            (unsigned)done, dst.imageSize, dst.pitch);
   if (!done || submitted <= before || retired < submitted) return READ_COPY_FENCE;
   stage->copy_pending = false;

   DCInvalidateRange(guard_start, checked_span);
   OSMemoryBarrier();
   if (!guard_matches(guard_start) || !guard_matches(guard_end)) return READ_COPY_GUARD;
   for (uint32_t y = 0; y < dst.height; ++y)
      memcpy(rgba + y * row_bytes, pixels + (size_t)y * dst.pitch * 4, row_bytes);
   return READ_OK;
}
