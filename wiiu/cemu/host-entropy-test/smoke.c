#include <stdint.h>
#include <string.h>
#include <coreinit/debug.h>
#include <coreinit/thread.h>
#include <whb/proc.h>
extern uint32_t horizon_entropy_probe(void);
extern int __wrap_NSSInit(void);
extern int __wrap_NSSGetRandom(void *,int);
static OSThread threads[4];
static _Alignas(16) unsigned char stacks[4][32768];
static unsigned char outputs[4][256];
static unsigned char big[1024*1024];
static int worker(int argc, void *unused) { (void)unused; if (__wrap_NSSInit()!=0) return 1; return __wrap_NSSGetRandom(outputs[argc],256)!=0; }
int main(void) {
  uint32_t flags=0;
#ifdef HORIZON_TEST_PREINIT
  flags |= horizon_entropy_probe();
#endif
  WHBProcInit();
  int created[4]={0};
  for(int i=0;i<4;i++) { if(!OSCreateThread(&threads[i],worker,i,0,stacks[i]+sizeof(stacks[i]),sizeof(stacks[i]),16,OS_THREAD_ATTRIB_AFFINITY_ANY)) flags|=2; else { created[i]=1; OSResumeThread(&threads[i]); } }
  for(int i=0;i<4;i++) { int result=1; if(!created[i] || !OSJoinThread(&threads[i],&result) || result) flags|=4; }
#ifndef HORIZON_TEST_PREINIT
  flags |= horizon_entropy_probe();
#endif
  for(int i=0;i<4;i++) for(int j=0;j<i;j++) if(!memcmp(outputs[i],outputs[j],256)) flags|=8;
  memset(big,0x5a,sizeof(big)); if(__wrap_NSSGetRandom(big,sizeof(big))==0) flags|=16;
  for(size_t i=0;i<sizeof(big);i++) if(big[i]!=0x5a) { flags|=16; break; }
  if(__wrap_NSSGetRandom(0,0)!=0) flags|=32;
  memset(outputs,0,sizeof(outputs)); memset(big,0,sizeof(big));
  OSReport("Host pool validation: failure flags=%u\n",flags); WHBProcShutdown(); return (int)flags;
}
