#include <coreinit/debug.h>
#include <coreinit/dynload.h>
#include <coreinit/thread.h>
#include <coreinit/time.h>
#include <nn/ac/ac_c.h>
#include <nsysnet/netconfig.h>
#include <stdbool.h>
#include <stdint.h>
#include <sysapp/launch.h>
#include <whb/proc.h>

static void *resolve_export(OSDynLoad_Module module, const char *name)
{
   void *address = NULL;
   return OSDynLoad_FindExport(module, OS_DYNLOAD_EXPORT_FUNC, name, &address) == OS_DYNLOAD_OK
          ? address : NULL;
}

static unsigned check_interface(__typeof__(&netconf_get_assigned_dns) assigned,
                                NetConfInterfaceType interface)
{
   uint32_t primary = 0, secondary = 0;
   const int status = assigned(interface, &primary, &secondary);
   const bool valid_primary = primary != 0 && primary != UINT32_MAX;
   const bool valid_secondary = secondary != 0 && secondary != UINT32_MAX && secondary != primary;
   const unsigned count = status == 0 ? (unsigned)valid_primary + (unsigned)valid_secondary : 0;
   OSReport("Horizon DNS probe interface=%s status=%d nameservers=%u\n",
            interface == NET_CONF_INTERFACE_TYPE_WIFI ? "wifi" : "ethernet", status, count);
   return count;
}

static unsigned check_netconf(OSDynLoad_Module module)
{
   const __typeof__(&netconf_init) init =
      (__typeof__(init))resolve_export(module, "netconf_init");
   const __typeof__(&netconf_close) close =
      (__typeof__(close))resolve_export(module, "netconf_close");
   const __typeof__(&netconf_get_assigned_dns) assigned =
      (__typeof__(assigned))resolve_export(module, "netconf_get_assigned_dns");
   if (!init || !close || !assigned) {
      OSReport("Horizon DNS probe exports init=%u close=%u assigned_dns=%u\n",
               (unsigned)(init != NULL), (unsigned)(close != NULL), (unsigned)(assigned != NULL));
      return 0;
   }

   const NNResult ac = ACInitialize();
   if (!NNResult_IsSuccess(ac)) {
      OSReport("Horizon DNS probe ACInitialize status=%08x\n", (unsigned)ac.value);
      return 0;
   }

   unsigned count = 0;
   BOOL connected = false;
   NNResult state = ACIsApplicationConnected(&connected);
   if (NNResult_IsSuccess(state) && !connected) state = ACConnect();
   if (NNResult_IsSuccess(state)) {
      const int status = init();
      OSReport("Horizon DNS probe netconf_init=%d\n", status);
      if (!status) {
         count += check_interface(assigned, NET_CONF_INTERFACE_TYPE_WIFI);
         count += check_interface(assigned, NET_CONF_INTERFACE_TYPE_ETHERNET);
         OSReport("Horizon DNS probe netconf_close=%d\n", close());
      }
   } else {
      OSReport("Horizon DNS probe connection status=%08x\n", (unsigned)state.value);
   }
   ACFinalize();
   return count;
}

int main(void)
{
   WHBProcInit();
   OSDynLoad_Module module = NULL;
   unsigned count = 0;
   if (OSDynLoad_Acquire("nsysnet.rpl", &module) == OS_DYNLOAD_OK) {
      count = check_netconf(module);
      OSDynLoad_Release(module);
   } else {
      OSReport("Horizon DNS probe nsysnet unavailable\n");
   }
   OSReport("Horizon DNS probe result=%s\n", count ? "assigned" : "unavailable");

   if (WHBProcIsRunning()) {
      SYSLaunchMenu();
      while (WHBProcIsRunning()) {
         OSSleepTicks(OSMillisecondsToTicks(16));
      }
   }
   OSReport("Horizon DNS probe ProcUI exit complete\n");
   WHBProcShutdown();
   return 0;
}
