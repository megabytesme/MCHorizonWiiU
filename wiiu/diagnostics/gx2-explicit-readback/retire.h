#pragma once

#include "stage.h"
#include <coreinit/thread.h>
#include <gx2/semaphore.h>

static void retire_for_teardown(Stage *stage, const char *fixture)
{
   // Submit a new command on the same lane even if the old copy ticket never
   // advanced or no frame reached the renderer.
   static _Alignas(CACHE_LINE) GX2Semaphore fence_line[CACHE_LINE / sizeof(GX2Semaphore)];
   const OSTime before = GX2GetLastSubmittedTimeStamp();
   stage_observe(stage, before, GX2GetRetiredTimeStamp());
   const OSTime floor = stage->watermark_floor;
   DCFlushRange(fence_line, sizeof(fence_line));
   GX2SetSemaphore(fence_line, GX2_SEMAPHORE_ACTION_SIGNAL);
   GX2Flush();
   stage_observe(stage, GX2GetLastSubmittedTimeStamp(), 0);

   // Keep the existing synchronization, including Cemu's Vulkan full sync.
   GX2DrawDone();
   bool reported = false;
   for (;;) {
      const OSTime submitted = GX2GetLastSubmittedTimeStamp();
      const OSTime retired = GX2GetRetiredTimeStamp();
      if (submitted > floor && submitted >= stage->watermark_floor && retired >= submitted) {
         stage->copy_pending = false;
         if (reported)
            OSReport("%s teardown retired submitted=%lld retired=%lld\n",
                     fixture, (long long)submitted, (long long)retired);
         return;
      }
      stage_observe(stage, submitted, 0);
      if (!reported) {
         reported = true;
         OSReport("%s teardown retaining GPU resources floor=%lld pending_copy=%u\n",
                  fixture, (long long)floor, (unsigned)stage->copy_pending);
      }
      // One fence is outstanding. Retain the process and all allocations
      // while polling instead of enqueuing more work or pumping release.
      OSSleepTicks(OSMillisecondsToTicks(16));
   }
}
