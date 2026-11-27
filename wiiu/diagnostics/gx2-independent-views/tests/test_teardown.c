#include "scene.h"

#include <assert.h>
#include <inttypes.h>
#include <stdio.h>

typedef int64_t OSTime;
typedef uint64_t GX2Semaphore;
enum { CACHE_LINE = 32, GX2_SEMAPHORE_ACTION_SIGNAL = 1 };

#include "stage-observe.inc"

typedef struct {
   bool copy_pending;
} FixtureStage;
typedef struct {
   FixtureStage stage;
} Fixture;

struct Readings {
   OSTime submitted, retired;
};

static struct {
   struct Readings readings[8];
   size_t count, index;
   unsigned flushes, signals, draws, sleeps, retained_logs, retired_logs;
   Stage *stage;
   bool expect_pending;
} gpu;

static OSTime GX2GetLastSubmittedTimeStamp(void)
{
   assert(gpu.index < gpu.count);
   return gpu.readings[gpu.index++].submitted;
}

static OSTime GX2GetRetiredTimeStamp(void)
{
   assert(gpu.index > 0 && gpu.index <= gpu.count);
   return gpu.readings[gpu.index - 1].retired;
}

static void DCFlushRange(void *line, size_t bytes)
{
   assert((uintptr_t)line % CACHE_LINE == 0 && bytes == CACHE_LINE);
   assert(gpu.signals == 0);
}

static void GX2SetSemaphore(GX2Semaphore *line, int mode)
{
   assert((uintptr_t)line % CACHE_LINE == 0);
   assert(mode == GX2_SEMAPHORE_ACTION_SIGNAL);
   ++gpu.signals;
}
static void GX2Flush(void) { assert(gpu.signals == 1); ++gpu.flushes; }
static bool GX2DrawDone(void)
{
   assert(gpu.signals == 1 && gpu.flushes == 1);
   ++gpu.draws;
   return false; // The ABI alone does not prove completion.
}

static OSTime OSMillisecondsToTicks(uint32_t ms) { return ms; }
static void OSSleepTicks(OSTime ticks)
{
   // No resource release or repeat submission can precede the safe sample.
   assert(gpu.stage->copy_pending == gpu.expect_pending);
   assert(ticks == 16 && gpu.signals == 1 && gpu.flushes == 1 && gpu.draws == 1);
   assert(gpu.index < gpu.count);
   ++gpu.sleeps;
}

static void OSReport(const char *format, ...)
{
   if (strstr(format, "retaining")) ++gpu.retained_logs;
   if (strstr(format, "teardown retired")) ++gpu.retired_logs;
}

#include "retire.inc"

static void retry(Stage *stage, const struct Readings *values, size_t count, unsigned sleeps)
{
   memset(&gpu, 0, sizeof(gpu));
   assert(count <= sizeof(gpu.readings)/sizeof(gpu.readings[0]));
   memcpy(gpu.readings, values, count*sizeof(*values));
   gpu.count = count;
   gpu.stage = stage;
   gpu.expect_pending = stage->copy_pending;
   retire_for_teardown(stage, "fixture");
   assert(!stage->copy_pending);
   assert(gpu.index == gpu.count && gpu.sleeps == sleeps);
   assert(gpu.signals == 1 && gpu.flushes == 1 && gpu.draws == 1);
   assert(gpu.retained_logs == (sleeps != 0) && gpu.retired_logs == (sleeps != 0));
}

static unsigned renders;
static unsigned fail_at;

static bool render(Fixture *fixture, unsigned tv_views, unsigned pose, enum Fault fault)
{
   (void)fixture;
   assert(tv_views == 4);
   assert(pose == (fault == GENERAL_SHADER));
   ++renders;
   return renders != fail_at;
}

static bool run_faults(bool ok)
{
   Fixture f = {0};
#include "faults.inc"
   return ok;
}

int main(void)
{
   // Empty startup still needs a real fresh tail, because zero is no fence.
   Stage stage = {0};
   retry(&stage, (struct Readings[]){{0,0}, {1,0}, {1,1}}, 3, 0);

   // The failed copy recorded a larger watermark than the live getter.
   stage = (Stage){.copy_pending = true, .watermark_floor = 100};
   retry(&stage, (struct Readings[]){{90,89}, {98,97}, {98,98}, {101,100}, {101,101}}, 5, 2);

   // Equal stale timestamps cannot retire a copy whose ticket never advanced.
   stage = (Stage){.copy_pending = true, .watermark_floor = 40};
   retry(&stage, (struct Readings[]){{40,40}, {40,40}, {40,40}, {41,40}, {41,41}}, 5, 2);

   // Once observed, a newer submitted tail cannot be replaced by stale data.
   stage = (Stage){.watermark_floor = 102};
   retry(&stage, (struct Readings[]){{102,102}, {103,102}, {104,103},
                                    {103,103}, {104,104}}, 5, 2);
   assert(stage.watermark_floor == 104);

   assert(!run_faults(false) && renders == 0);
   fail_at = 1;
   assert(!run_faults(true) && renders == 1);
   renders = 0;
   fail_at = 4;
   assert(!run_faults(true) && renders == 4);
   renders = 0;
   fail_at = 0;
   assert(run_faults(true) && renders == 6);
   puts("GX2 teardown host checks PASS (fresh proof, stale/invalid copy, empty startup, no extra GPU work, fail-fast)");
   return 0;
}
