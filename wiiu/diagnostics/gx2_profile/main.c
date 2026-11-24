#include "gx2_profile.h"

#include <coreinit/debug.h>
#include <coreinit/time.h>
#include <stdint.h>
#include <whb/gfx.h>
#include <whb/proc.h>
#include <whb/sdcard.h>

enum { PHASE_FRAMES = 42, LOAD_PHASE = 2, PLAY_PHASE = 5, PARTIAL_COUNTER = 1 };

int main(void)
{
   WHBProcInit();
   if (!WHBMountSdCard() || !WHBGfxInit()) return 1;
   horizon_gx2_profile_init();
   GX2ColorBuffer *tv = WHBGfxGetTVColourBuffer(), *drc = WHBGfxGetDRCColourBuffer();
   uint32_t dimensions[] = {tv->surface.width, tv->surface.height,
                            drc->surface.width, drc->surface.height};
   const uint32_t phases[] = {GX2_PROFILE_NIGHT, LOAD_PHASE, PLAY_PHASE};
   bool enabled = false;
   OSTime recorder = 0;
   const OSTime started = OSGetSystemTime();
   for (uint32_t i = 0; i < sizeof(phases) / sizeof(phases[0]); ++i) {
      for (uint32_t j = 0; j < PHASE_FRAMES; ++j) {
         if (!WHBProcIsRunning()) return 1;
         OSTime start = OSGetSystemTime();
         enabled = horizon_gx2_profile_begin(phases[i]);
         uint64_t stamp = horizon_gx2_profile_stamp();
         recorder += OSGetSystemTime() - start;
         WHBGfxBeginRender();
         WHBGfxBeginRenderTV();
         WHBGfxClearColor(0.25f, 0.5f, 0.75f, 1.0f);
         WHBGfxFinishRenderTV();
         WHBGfxBeginRenderDRC();
         WHBGfxClearColor(0.25f, 0.5f, 0.75f, 1.0f);
         WHBGfxFinishRenderDRC();
         WHBGfxFinishRender();
         start = OSGetSystemTime();
         horizon_gx2_profile_end(GX2_PROFILE_FINISH, stamp);
         uint32_t outputs = phases[i] == PLAY_PHASE && j % 3 == 2 ? 1 : 3;
         horizon_gx2_profile_outputs(outputs, dimensions);
         if (outputs == 3) horizon_gx2_profile_count(GX2_PROFILE_PRESENTED, 1);
         if (phases[i] == PLAY_PHASE && j % 3 == 1)
            horizon_gx2_profile_count(PARTIAL_COUNTER, 1);
         horizon_gx2_profile_finish();
         recorder += OSGetSystemTime() - start;
      }
   }
   const OSTime elapsed = OSGetSystemTime() - started;
   uint32_t snapshot[(28 + 256) / sizeof(uint32_t)];
   uint32_t bytes = horizon_gx2_profile_snapshot(snapshot, sizeof(snapshot));
   bool valid = enabled ? bytes == sizeof(snapshot) && snapshot[0] == 0x485A5032 &&
      snapshot[5] == 1 && snapshot[6] == 3 * PHASE_FRAMES : bytes == 0;
   if (enabled) horizon_gx2_profile_flush();
   OSReport("Horizon GX2 diagnostic enabled=%u loop_us=%u recorder_us=%u snapshot=%u\n",
            (unsigned)enabled, (unsigned)(OSTicksToMicroseconds(elapsed) / (3 * PHASE_FRAMES)),
            (unsigned)(OSTicksToMicroseconds(recorder) / (3 * PHASE_FRAMES)), bytes);
   OSReport("GX2 CAPTURE TEST %s\n", valid ? "PASSED" : "FAILED");
   WHBGfxShutdown();
   WHBUnmountSdCard();
   WHBProcShutdown();
   return valid ? 0 : 1;
}
