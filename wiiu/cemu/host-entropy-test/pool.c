#include <stdint.h>
#include <fcntl.h>
#include <unistd.h>
#include <stdio.h>
#include <stdatomic.h>
#include <string.h>
#include <whb/sdcard.h>
#define POOL_SIZE (1024u * 1024u)
static unsigned char pool[POOL_SIZE];
static size_t used;
static int initialized;
static atomic_flag lock = ATOMIC_FLAG_INIT;
static const char *fresh = "/vol/external01/wiiu/apps/HorizonWiiU/cemu-entropy.pool";
static const char *claimed = "/vol/external01/wiiu/apps/HorizonWiiU/cemu-entropy.claimed";
static void wipe(void *p, size_t n) { volatile unsigned char *v=p; while(n--) *v++=0; }
static void acquire(void) { while (atomic_flag_test_and_set_explicit(&lock,memory_order_acquire)) { } }
static void release(void) { atomic_flag_clear_explicit(&lock,memory_order_release); }
int __wrap_NSSInit(void) {
  int result=-1;
  acquire();
  if (initialized) { result=0; goto done; }
  if (!WHBMountSdCard()) goto done;
  int claim=open(claimed,O_CREAT|O_EXCL|O_WRONLY,0600); if(claim<0) goto done;
  if(close(claim)!=0) goto done;
  FILE *f=fopen(fresh,"rb"); if(!f) goto done;
  size_t count=fread(pool,1,POOL_SIZE,f); int tail=fgetc(f); int failed=ferror(f); int closed=fclose(f);
  if (count!=POOL_SIZE || tail!=EOF || failed || closed!=0 || remove(fresh)!=0 || remove(claimed)!=0) goto done;
  initialized=1; result=0;
done:
  if (result!=0) wipe(pool,POOL_SIZE);
  release(); return result;
}
int __wrap_NSSGetRandom(void *out, int len) {
  if (len<0 || (len>0 && !out)) return -1;
  acquire();
  if (!initialized || (size_t)len>POOL_SIZE-used) { release(); return -1; }
  if (len==0) { release(); return 0; }
  memcpy(out,pool+used,(size_t)len); wipe(pool+used,(size_t)len); used+=(size_t)len;
  release(); return 0;
}
