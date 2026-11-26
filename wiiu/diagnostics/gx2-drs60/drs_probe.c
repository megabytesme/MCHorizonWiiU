#define main timing_fixture_main
#include "main.c"
#undef main

#include "gx2_profile.h"
#include <stddef.h>
#include <coreinit/memdefaultheap.h>

enum { FLOOR = 50, TARGET = 60, LIGHT_DRAWS = 2, MAX_DRAWS = 256 };
static CycleSlot endpoints[2][2];

typedef struct {
   HorizonGx2GpuSample sample;
   uint64_t frame_ns, submission_ns, wait_ns;
   uint32_t drawn[2], scale, outputs;
} Frame;

_Static_assert(sizeof(Frame) == 112 && offsetof(Frame, frame_ns) == 72, "Rust Frame ABI");

extern float drs_timing(const Frame *frame, uint32_t target);
extern uint32_t drs_feed(const Frame *frame);
extern bool drs_self_check(const Frame *light, const Frame *heavy);
extern void drs_forget(void);

static bool self_check(void)
{
   const Frame light = {
      .sample = {
         .endpoints = {{1000, 109000}, {109000, 217000}},
         .submitted = 10, .retired = 10, .status = GX2_GPU_VALID,
      },
      .frame_ns = 24000000, .submission_ns = 23000000, .wait_ns = 18000000,
      .drawn = {2, 2}, .scale = 50, .outputs = 3,
   };
   Frame heavy = light;
   heavy.scale = 60;
   heavy.sample.endpoints[0][1] = 298000;
   heavy.sample.endpoints[1][0] = 298000;
   heavy.sample.endpoints[1][1] = 595000;
   return drs_self_check(&light, &heavy);
}

void *drs_alloc(size_t bytes, size_t alignment)
{
   return MEMAllocFromDefaultHeapEx(bytes, alignment);
}

void drs_free(void *ptr)
{
   MEMFreeToDefaultHeap(ptr);
}

static unsigned microseconds(uint64_t nanos)
{
   return (unsigned)(nanos / 1000);
}

static void output(const WHBGfxShaderGroup *shader, const GX2ColorBuffer *color,
                   unsigned index, unsigned scale, unsigned draws)
{
   const unsigned width = color->surface.width * scale / 100;
   const unsigned height = color->surface.height * scale / 100;
   GX2SetScissor(0, 0, color->surface.width, color->surface.height);
   WHBGfxClearColor(0.0f, 0.0f, 0.0f, 1.0f);
   GX2SampleTopGPUCycle(&endpoints[index][0].value);
   bind_pipeline(shader);
   GX2SetViewport(0, 0, width, height, 0.0f, 1.0f);
   GX2SetScissor(0, 0, width, height);
   draw(draws);
   GX2SampleBottomGPUCycle(&endpoints[index][1].value);
}

static bool measure_frame(const WHBGfxShaderGroup *shader, unsigned scale,
                          unsigned draws, unsigned cpu_delay_ms, Frame *frame)
{
   const OSTime started = OSGetTime();
   if (!foreground()) return false;
   const OSTime cpu_until = OSGetTime() + OSMillisecondsToTicks(cpu_delay_ms);
   while (OSGetTime() < cpu_until) {}
   const OSTime submission_started = OSGetTime();
   WHBGfxBeginRender();
   const OSTime flip_wait = OSGetTime() - submission_started;
   const OSTime before = GX2GetLastSubmittedTimeStamp();
   for (unsigned output = 0; output < 2; ++output) {
      reset_slot(&endpoints[output][0]);
      reset_slot(&endpoints[output][1]);
   }

   WHBGfxBeginRenderTV();
   output(shader, WHBGfxGetTVColourBuffer(), 0, scale, draws);
   WHBGfxFinishRenderTV();
   WHBGfxBeginRenderDRC();
   output(shader, WHBGfxGetDRCColourBuffer(), 1, scale, draws);
   WHBGfxFinishRenderDRC();

   const OSTime finish_started = OSGetTime();
   WHBGfxFinishRender();
   const OSTime finish_wait = OSGetTime() - finish_started;

   const OSTime submitted = GX2GetLastSubmittedTimeStamp();
   const OSTime retired = GX2GetRetiredTimeStamp();
   *frame = (Frame){
      .sample = {
         .submitted = submitted, .retired = retired,
         .status = GX2_GPU_NOT_RETIRED,
      },
      .drawn = {draws, draws}, .scale = scale, .outputs = 3,
   };
   // Retire the last command of the pair before reading or reusing the slots.
   if (submitted > before && retired >= submitted) {
      frame->sample.status = GX2_GPU_VALID;
      for (unsigned i = 0; i < 2; ++i) {
         const uint64_t top = read_slot(&endpoints[i][0]);
         const uint64_t bottom = read_slot(&endpoints[i][1]);
         frame->sample.endpoints[i][0] = top;
         frame->sample.endpoints[i][1] = bottom;
         if (!valid(top) || !valid(bottom) || bottom <= top)
            frame->sample.status = GX2_GPU_INVALID_ENDPOINTS;
      }
   }
   const OSTime ended = OSGetTime();
   frame->frame_ns = OSTicksToNanoseconds(ended - started);
   frame->submission_ns = OSTicksToNanoseconds(ended - submission_started);
   frame->wait_ns = OSTicksToNanoseconds(flip_wait + finish_wait);
   return true;
}

static void report(const char *stage, unsigned number, const Frame *frame, unsigned next)
{
   const float ms = drs_timing(frame, TARGET);
   const uint64_t (*e)[2] = frame->sample.endpoints;
   OSReport("GX2 DRS %s #%u scale=%u next=%u draws=%u status=%u gpu_us=%d "
            "wall_us=%u submit_us=%u wait_us=%u cpu_us=%u "
            "submitted=%016llx retired=%016llx tv=%016llx/%016llx drc=%016llx/%016llx\n",
            stage, number, frame->scale, next, frame->drawn[0], frame->sample.status,
            ms < 0.0f ? -1 : (int)(ms * 1000.0f),
            microseconds(frame->frame_ns), microseconds(frame->submission_ns),
            microseconds(frame->wait_ns), microseconds(frame->frame_ns - frame->wait_ns),
            (unsigned long long)frame->sample.submitted, (unsigned long long)frame->sample.retired,
            (unsigned long long)e[0][0], (unsigned long long)e[0][1],
            (unsigned long long)e[1][0], (unsigned long long)e[1][1]);
}

static unsigned select_load(const WHBGfxShaderGroup *shader)
{
   unsigned best = 0;
   for (unsigned draws = 16; draws <= MAX_DRAWS; draws *= 2) {
      unsigned above = 0;
      for (unsigned i = 1; i <= 3; ++i) {
         Frame frame;
         if (!measure_frame(shader, 60, draws, 0, &frame)) return 0;
         report("calibration", i, &frame, 60);
         if (frame.sample.status != GX2_GPU_VALID) return 0;
         if (frame.frame_ns >= UINT64_C(120000000)) return best;
         if (drs_timing(&frame, TARGET) > 20.0f) ++above;
      }
      best = draws;
      if (above >= 2) break;
   }
   return best;
}

static bool stage(const WHBGfxShaderGroup *shader, const char *name, unsigned draws,
                  unsigned scale, unsigned goal)
{
   const OSTime deadline = OSGetTime() + OSMillisecondsToTicks(14500);
   OSTime next_report = 0;
   unsigned frames = 0, accepted = 0, next = scale;
   while (OSGetTime() < deadline && frames < 4000) {
      Frame frame;
      if (!measure_frame(shader, scale, draws, 0, &frame)) break;
      ++frames;
      if (frame.sample.status != GX2_GPU_VALID) {
         report(name, frames, &frame, scale);
         break;
      }
      accepted += drs_timing(&frame, TARGET) >= 0.0f;
      next = drs_feed(&frame);
      if (next != scale || OSGetTime() >= next_report) {
         report(name, frames, &frame, next);
         next_report = OSGetTime() + OSMillisecondsToTicks(1000);
      }
      if (next != scale) break;
   }
   OSReport("GX2 DRS stage %s frames=%u accepted=%u from=%u to=%u goal=%u\n",
            name, frames, accepted, scale, next, goal);
   return next == goal;
}

static const char *run(const WHBGfxShaderGroup *shader)
{
   Frame frame;
   if (!measure_frame(shader, FLOOR, LIGHT_DRAWS, 0, &frame)) return "foreground_lost";
   report("preflight", 1, &frame, FLOOR);
   if (frame.sample.status == GX2_GPU_NOT_RETIRED) return "retirement_unavailable";
   if (frame.sample.status == GX2_GPU_INVALID_ENDPOINTS) return "endpoints_unavailable";

   if (!measure_frame(shader, FLOOR, LIGHT_DRAWS, 20, &frame)) return "foreground_lost";
   report("cpu_guard", 1, &frame, FLOOR);
   OSReport("GX2 DRS CPU guard target60=%u target30=%u\n",
            (unsigned)(drs_timing(&frame, 60) >= 0.0f),
            (unsigned)(drs_timing(&frame, 30) >= 0.0f));
   if (frame.sample.status != GX2_GPU_VALID) return "cpu_control_endpoints_unavailable";
   if (drs_timing(&frame, TARGET) >= 0.0f) return "cpu_guard_failed";

   const unsigned heavy = select_load(shader);
   OSReport("GX2 DRS chosen draws light=%u heavy=%u\n", LIGHT_DRAWS, heavy);
   if (!heavy) return "load_calibration_unavailable";
   if (!stage(shader, "light", LIGHT_DRAWS, FLOOR, 60)) return "upshift_unproven";
   if (!stage(shader, "heavy", heavy, 60, FLOOR)) return "downshift_unproven";
   if (!stage(shader, "recovery", LIGHT_DRAWS, FLOOR, 60)) return "recovery_unproven";
   return "60fps_policy_transitions_observed";
}

int main(void)
{
   OSReport("GX2 DRS entry before WHB\n");
   WHBProcInit();
   OSReport("GX2 DRS WHB process initialized\n");
   if (!WHBGfxInit()) {
      OSReport("GX2 DRS result=gfx_init_failed\n");
      return_to_menu();
      WHBProcShutdown();
      return 1;
   }
   OSReport("GX2 DRS WHB graphics initialized\n");
   WHBGfxShaderGroup shader = {0};
   const bool loaded = WHBGfxLoadGFDShaderGroup(&shader, 0, _binary_timing_gsh_start);
   const bool ready = loaded &&
      WHBGfxInitShaderAttribute(&shader, "aPosition", 0, 0, GX2_ATTRIB_FORMAT_FLOAT_32_32_32_32) &&
      WHBGfxInitFetchShader(&shader);
   const char *result = "shader_init_failed";
   if (ready) {
      DCFlushRange(triangle, sizeof(triangle));
      OSReport("GX2 DRS begin source=gx2-top-bottom hz=27000000 floor=%u target=%u\n", FLOOR, TARGET);
      const bool policy_ok = self_check();
      OSReport("GX2 DRS policy_abi=%s source=synthetic\n", policy_ok ? "pass" : "fail");
      result = policy_ok ? run(&shader) : "policy_abi_failed";
      drs_forget();
   }
   if (targets_ready()) GX2DrawDone();
   if (loaded) WHBGfxFreeShaderGroup(&shader);
   OSReport("GX2 DRS result=%s\n", result);
   return_to_menu();
   WHBGfxShutdown();
   WHBProcShutdown();
   return ready ? 0 : 1;
}
