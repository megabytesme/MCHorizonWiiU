#include <coreinit/debug.h>
#include <stdint.h>
#include <whb/proc.h>

extern uint32_t horizon_tls_probe(void);

int main(void)
{
   WHBProcInit();
   const uint32_t failures = horizon_tls_probe();
   OSReport("Cafe std TLS virtualization probe: %s failures=0x%08x\n",
            failures == 0 ? "PASS" : "FAIL", failures);
   WHBProcShutdown();
   return failures == 0 ? 0 : 1;
}
