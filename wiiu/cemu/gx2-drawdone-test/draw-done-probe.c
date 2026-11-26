#include <coreinit/debug.h>
#include <gx2/event.h>
#include <gx2/state.h>
#include <stdint.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern BOOL draw_done_with_guest_r3(uint32_t seed);

int main(void)
{
   WHBProcInit();
   if (!WHBGfxInit()) {
      OSReport("GX2DrawDone ABI fixture could not initialize GX2\n");
      WHBProcShutdown();
      return 1;
   }

   WHBGfxBeginRender();
   WHBGfxBeginRenderTV();
   WHBGfxClearColor(0.10f, 0.25f, 0.40f, 1.0f);
   WHBGfxFinishRenderTV();
   WHBGfxBeginRenderDRC();
   WHBGfxClearColor(0.40f, 0.25f, 0.10f, 1.0f);
   WHBGfxFinishRenderDRC();

   GX2Flush();
   const OSTime submitted = GX2GetLastSubmittedTimeStamp();
   const BOOL zero_seed = draw_done_with_guest_r3(0);
   const BOOL nonboolean_seed = draw_done_with_guest_r3(0x5a);
   const OSTime retired = GX2GetRetiredTimeStamp();
   const BOOL timestamp_done = submitted > 0 && GX2WaitTimeStamp(submitted);
   const BOOL pass = zero_seed == TRUE && nonboolean_seed == TRUE
      && timestamp_done && retired >= submitted;
   OSReport("GX2DrawDone ABI seed0=%d seed90=%d timestamp=%d submitted=%lld retired=%lld pass=%d\n",
      (int)zero_seed, (int)nonboolean_seed, (int)timestamp_done,
      (long long)submitted, (long long)retired, (int)pass);

   WHBGfxFinishRender();
   WHBGfxShutdown();
   WHBProcShutdown();
   return pass ? 0 : 2;
}
