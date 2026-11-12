#include <coreinit/debug.h>
#include <stdint.h>
#include <proc_ui/procui.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern void horizon_wiiu_app_entropy_first(void);
extern int32_t horizon_wiiu_app_run(void);

static volatile int32_t sFocused = 1;

static uint32_t app_acquired(void *context)
{
   (void)context;
   sFocused = 1;
   return 0;
}

static uint32_t app_released(void *context)
{
   (void)context;
   sFocused = 0;
   return 0;
}

int32_t horizon_wiiu_app_is_running(void)
{
   return WHBProcIsRunning() ? 1 : 0;
}

int32_t horizon_wiiu_app_is_focused(void)
{
   return sFocused;
}

int main(void)
{
   /* This must be the first platform operation: it anchors Cafe std entropy. */
   horizon_wiiu_app_entropy_first();
   OSReport("Horizon Wii U startup: entropy ready\n");

   OSReport("Horizon Wii U startup: WHBProcInit begin\n");
   WHBProcInit();
   OSReport("Horizon Wii U startup: WHBProcInit complete\n");
   OSReport("Horizon Wii U startup: WHBGfxInit begin\n");
   if (!WHBGfxInit()) {
      OSReport("Horizon Wii U: WHBGfxInit failed\n");
      WHBProcShutdown();
      return 3;
   }
   OSReport("Horizon Wii U startup: WHBGfxInit complete\n");
   /* WHB owns target allocation callbacks. These callbacks only publish the
    * lifecycle state after WHB's ProcUI pump has processed each transition. */
   OSReport("Horizon Wii U startup: ProcUI callback registration begin\n");
   ProcUIRegisterCallback(PROCUI_CALLBACK_ACQUIRE, app_acquired, NULL, 100);
   ProcUIRegisterCallback(PROCUI_CALLBACK_RELEASE, app_released, NULL, 100);
   OSReport("Horizon Wii U startup: ProcUI callback registration complete\n");

   OSReport("Horizon Wii U startup: Rust app entry begin\n");
   const int32_t result = horizon_wiiu_app_run();
   OSReport("Horizon Wii U startup: Rust app entry returned\n");
   switch (result) {
   case 0:
      OSReport("Horizon Wii U: Cafe entry finished\n");
      break;
   case 1:
      OSReport("Horizon Wii U: original title is ready; Game handoff API is unavailable\n");
      break;
   case 2:
      OSReport("Horizon Wii U: original Loading failed\n");
      break;
   case 4:
      OSReport("Horizon Wii U: cannot enter /vol/external01/wiiu/apps/HorizonWiiU; check that the SD card is mounted and the app is installed there\n");
      break;
   case 5:
      OSReport("Horizon Wii U: original local world startup failed; inspect the Cafe log\n");
      break;
   default:
      OSReport("Horizon Wii U: native loading presenter setup failed\n");
      break;
   }

   /* Rust drops the presenter and Loading outcome before returning here. */
   WHBGfxShutdown();
   WHBProcShutdown();
   return result == 0 ? 0 : 1;
}
