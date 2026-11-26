#include <coreinit/core.h>
#include <coreinit/debug.h>
#include <coreinit/event.h>
#include <coreinit/memdefaultheap.h>
#include <coreinit/memexpheap.h>
#include <coreinit/thread.h>
#include <coreinit/time.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>
#include <whb/proc.h>

enum { CLASSES = 6, SLOTS = 96, BATCHES = 16, STACK_BYTES = 64 * 1024 };
static const uint32_t sizes[CLASSES] = {64, 224, 704, 2560, 11264, 32768};
static const int32_t alignments[CLASSES] = {4, 8, 8, 16, 32, 64};

typedef struct {
   uint64_t total;
   uint32_t max;
   uint32_t count;
} Timing;

typedef struct {
   _Alignas(64) void *blocks[SLOTS];
   Timing alloc[CLASSES];
   Timing free[CLASSES];
   unsigned core;
   unsigned failures;
   OSTime started;
   OSTime finished;
} Worker;

static Worker workers[2];
static OSEvent start;
static OSEvent allocated[2];
static OSEvent freed[2];
static OSThread partner;
static _Alignas(16) uint8_t stack[STACK_BYTES];

static void record(Timing *timing, uint32_t elapsed)
{
   timing->total += elapsed;
   if (elapsed > timing->max) timing->max = elapsed;
   ++timing->count;
}

static void run(unsigned index, bool exchange)
{
   Worker *self = &workers[index];
   Worker *owner = exchange ? &workers[1 - index] : self;
   self->core = OSGetCoreId();
   self->started = OSGetSystemTime();
   for (unsigned batch = 0; batch < BATCHES; ++batch) {
      for (unsigned i = 0; i < SLOTS; ++i) {
         unsigned size_class = i % CLASSES;
         uint32_t before = (uint32_t)OSGetSystemTick();
         void *block = MEMAllocFromDefaultHeapEx(sizes[size_class], alignments[size_class]);
         uint32_t ticks = (uint32_t)OSGetSystemTick() - before;
         record(&self->alloc[size_class], ticks);
         self->blocks[i] = block;
         if (!block || (uintptr_t)block % (unsigned)alignments[size_class]) ++self->failures;
      }
      if (exchange) {
         OSSignalEvent(&allocated[index]);
         OSWaitEvent(&allocated[1 - index]);
      }
      for (unsigned i = 0; i < SLOTS; ++i) {
         // 37 is coprime with SLOTS. Alternate which blocks stay between gaps.
         unsigned slot = (i * 37 + batch) % SLOTS;
         void *block = owner->blocks[slot];
         if (block) {
            uint32_t before = (uint32_t)OSGetSystemTick();
            MEMFreeToDefaultHeap(block);
            uint32_t ticks = (uint32_t)OSGetSystemTick() - before;
            record(&self->free[slot % CLASSES], ticks);
         }
      }
      if (exchange) {
         OSSignalEvent(&freed[index]);
         OSWaitEvent(&freed[1 - index]);
      }
   }
   self->finished = OSGetSystemTime();
}

static int run_partner(int argc, void *data)
{
   (void)data;
   OSWaitEvent(&start);
   run(1, argc != 0);
   return 0;
}

static unsigned report(const char *mode, unsigned count, MEMHeapHandle heap)
{
   unsigned failures = 0;
   for (unsigned i = 0; i < count; ++i) {
      const Worker *worker = &workers[i];
      failures += worker->failures;
      for (unsigned k = 0; k < CLASSES; ++k) {
         const Timing *a = &worker->alloc[k], *f = &worker->free[k];
         if (a->count != BATCHES * SLOTS / CLASSES || f->count != a->count) ++failures;
         OSReport("Horizon allocator mode=%s core=%u size=%u align=%d "
                  "alloc_n=%u alloc_total=0x%08x%08x alloc_max=%u "
                  "free_n=%u free_total=0x%08x%08x free_max=%u\n",
                  mode, worker->core, sizes[k], alignments[k],
                  a->count, (unsigned)(a->total >> 32), (unsigned)a->total, a->max,
                  f->count, (unsigned)(f->total >> 32), (unsigned)f->total, f->max);
      }
   }
   OSTime overlap = 0;
   if (count == 2) {
      OSTime started = workers[0].started > workers[1].started ? workers[0].started : workers[1].started;
      OSTime finished = workers[0].finished < workers[1].finished ? workers[0].finished : workers[1].finished;
      if (finished > started) overlap = finished - started;
      else ++failures;
   }
   OSReport("Horizon allocator mode=%s failures=%u overlap_us=%u heap_free=%u largest_64=%u\n",
            mode, failures, (unsigned)OSTicksToMicroseconds(overlap), MEMGetTotalFreeSizeForExpHeap(heap),
            MEMGetAllocatableSizeForExpHeapEx(heap, 64));
   return failures;
}

static bool measure_pair(bool exchange, unsigned other_core)
{
   memset(workers, 0, sizeof(workers));
   OSInitEvent(&start, false, OS_EVENT_MODE_AUTO);
   for (unsigned i = 0; i < 2; ++i) {
      OSInitEvent(&allocated[i], false, OS_EVENT_MODE_AUTO);
      OSInitEvent(&freed[i], false, OS_EVENT_MODE_AUTO);
   }
   if (!OSCreateThread(&partner, run_partner, exchange, NULL, stack + STACK_BYTES,
                       STACK_BYTES, 16, (OSThreadAttributes)(1u << other_core))) return false;
   OSResumeThread(&partner);
   OSSignalEvent(&start);
   run(0, exchange);
   int result = 1;
   return OSJoinThread(&partner, &result) && result == 0 && workers[0].core != workers[1].core;
}

int main(void)
{
   WHBProcInit();
   void *probe = MEMAllocFromDefaultHeapEx(64, 64);
   MEMHeapHandle heap = probe ? MEMFindContainHeap(probe) : NULL;
   if (probe) MEMFreeToDefaultHeap(probe);
   bool ok = heap && heap->tag == MEM_EXPANDED_HEAP_TAG;
   OSThread *main_thread = OSGetCurrentThread();
   unsigned affinity = OSGetThreadAffinity(main_thread);
   unsigned core = OSGetCoreId();
   bool pinned = ok && core < 3 && OSSetThreadAffinity(main_thread, 1u << core);
   if (pinned) {
      Timing overhead = {0};
      for (unsigned i = 0; i < 1024; ++i) {
         uint32_t before = (uint32_t)OSGetSystemTick();
         record(&overhead, (uint32_t)OSGetSystemTick() - before);
      }
      OSReport("Horizon allocator timer_hz=%u timer_n=%u timer_total=0x%08x%08x timer_max=%u "
               "heap_free=%u largest_64=%u\n",
               (unsigned)OSTimerClockSpeed, overhead.count,
               (unsigned)(overhead.total >> 32), (unsigned)overhead.total, overhead.max,
               MEMGetTotalFreeSizeForExpHeap(heap), MEMGetAllocatableSizeForExpHeapEx(heap, 64));
      run(0, false);
      unsigned failures = report("single", 1, heap);
      bool contended = measure_pair(false, (core + 1) % 3);
      if (contended) failures += report("contended", 2, heap);
      bool cross_free = contended && measure_pair(true, (core + 1) % 3);
      if (cross_free) failures += report("cross_free", 2, heap);
      ok = OSSetThreadAffinity(main_thread, affinity) && cross_free && failures == 0;
   } else {
      ok = false;
   }
   OSReport("Horizon allocator fixture %s\n", ok ? "PASSED" : "FAILED");
   WHBProcShutdown();
   return ok ? 0 : 1;
}
