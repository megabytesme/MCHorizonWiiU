#include <coreinit/cache.h>
#include <coreinit/debug.h>
#include <coreinit/thread.h>
#include <coreinit/time.h>
#include <gx2/draw.h>
#include <gx2/event.h>
#include <gx2/registers.h>
#include <gx2/shaders.h>
#include <gx2/state.h>
#include <gx2/surface.h>
#include <proc_ui/procui.h>
#include <stdbool.h>
#include <stdint.h>
#include <sysapp/launch.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern const uint8_t _binary_timing_gsh_start[];

// WUT exposes these symbols in its import archive, without public prototypes.
// Both Cemu and SwitchWakerHD receive an aligned 64-bit destination in r3.
// The observed ABI is used only by this qualification fixture.
extern void GX2SampleTopGPUCycle(uint64_t *destination);
extern void GX2SampleBottomGPUCycle(uint64_t *destination);

enum { DRAWS = 32, TRIALS = 8, CACHE_BYTES = 64, VISUAL_HOLD_MS = 8000 };
static const uint64_t poison = UINT64_C(0xa55a2c076ac15a5a);
typedef struct {
   _Alignas(CACHE_BYTES) uint64_t value;
} CycleSlot;
_Static_assert(sizeof(CycleSlot) == CACHE_BYTES, "isolate timestamp cache lines");

static CycleSlot top, bottom;
static _Alignas(CACHE_BYTES) float triangle[][4] = {
   {-1.0f, -1.0f, 0.5f, 1.0f}, {3.0f, -1.0f, 0.5f, 1.0f}, {-1.0f, 3.0f, 0.5f, 1.0f},
};

static void reset_slot(CycleSlot *slot)
{
   slot->value = poison;
   DCFlushRange(slot, sizeof(*slot));
}

static uint64_t read_slot(CycleSlot *slot)
{
   DCInvalidateRange(slot, sizeof(*slot));
   return *(volatile uint64_t *)&slot->value;
}

static bool valid(uint64_t cycle)
{
   return cycle != 0 && cycle != poison && cycle != UINT64_MAX;
}

static unsigned usec(OSTime ticks)
{
   return (unsigned)OSTicksToMicroseconds(ticks);
}

static bool drain(void)
{
   GX2Flush();
   const OSTime submitted = GX2GetLastSubmittedTimeStamp();
   GX2WaitTimeStamp(submitted);
   return GX2GetRetiredTimeStamp() >= submitted;
}

static void viewport(const GX2ColorBuffer *color, bool full)
{
   const uint32_t width = full ? color->surface.width : color->surface.width / 4;
   const uint32_t height = full ? color->surface.height : color->surface.height / 4;
   GX2SetScissor(0, 0, color->surface.width, color->surface.height);
   WHBGfxClearColor(0.0f, 0.0f, 0.0f, 1.0f);
   GX2SetViewport(0.0f, 0.0f, width, height, 0.0f, 1.0f);
   GX2SetScissor(0, 0, width, height);
}

static void draw(unsigned count)
{
   for (unsigned i = 0; i < count; ++i) GX2DrawEx(GX2_PRIMITIVE_MODE_TRIANGLES, 3, 0, 1);
}

static bool targets_ready(void)
{
   // ProcUIInForeground may still be true during release. WHB drains and
   // clears these foreground allocations before returning from its callback.
   return ProcUIInForeground() && WHBGfxGetTVColourBuffer()->surface.image &&
      WHBGfxGetDRCColourBuffer()->surface.image;
}

static bool foreground(void)
{
   return WHBProcIsRunning() && targets_ready();
}

static void bind_pipeline(const WHBGfxShaderGroup *shader)
{
   GX2SetDepthOnlyControl(false, false, GX2_COMPARE_FUNC_ALWAYS);
   GX2SetCullOnlyControl(GX2_FRONT_FACE_CCW, false, false);
   GX2SetColorControl(GX2_LOGIC_OP_COPY, 1, false, true);
   GX2SetTargetChannelMasks(GX2_CHANNEL_MASK_RGBA, 0, 0, 0, 0, 0, 0, 0);
   GX2SetBlendControl(GX2_RENDER_TARGET_0, GX2_BLEND_MODE_SRC_ALPHA,
                      GX2_BLEND_MODE_INV_SRC_ALPHA, GX2_BLEND_COMBINE_MODE_ADD, false,
                      GX2_BLEND_MODE_ONE, GX2_BLEND_MODE_ZERO, GX2_BLEND_COMBINE_MODE_ADD);
   GX2SetShaderMode(GX2_SHADER_MODE_UNIFORM_REGISTER);
   GX2SetFetchShader(&shader->fetchShader);
   GX2SetVertexShader(shader->vertexShader);
   GX2SetPixelShader(shader->pixelShader);
   GX2SetAttribBuffer(0, sizeof(triangle), sizeof(triangle[0]), triangle);
}

static bool show_raster(const WHBGfxShaderGroup *shader, bool full)
{
   if (!foreground()) return false;
   WHBGfxBeginRender();
   WHBGfxBeginRenderTV();
   const GX2ColorBuffer *tv = WHBGfxGetTVColourBuffer();
   bind_pipeline(shader);
   viewport(tv, full);
   draw(DRAWS);
   WHBGfxFinishRenderTV();

   WHBGfxBeginRenderDRC();
   const GX2ColorBuffer *drc = WHBGfxGetDRCColourBuffer();
   bind_pipeline(shader);
   viewport(drc, full);
   draw(DRAWS);
   WHBGfxFinishRenderDRC();
   WHBGfxFinishRender();

   const unsigned divide = full ? 1 : 4;
   OSReport("GX2 timing visual %s tv=%ux%u drc=%ux%u hold_ms=%u\n", full ? "full" : "small",
            tv->surface.width / divide, tv->surface.height / divide,
            drc->surface.width / divide, drc->surface.height / divide, VISUAL_HOLD_MS);
   const OSTime deadline = OSGetSystemTime() + OSMillisecondsToTicks(VISUAL_HOLD_MS);
   do {
      if (!foreground()) return false;
      OSSleepTicks(OSMillisecondsToTicks(16));
   } while (OSGetSystemTime() < deadline);
   return true;
}

static bool workload(GX2ColorBuffer *color, unsigned count, bool full, unsigned trial,
                     bool *saw_valid, bool *saw_unsupported)
{
   const uint32_t width = full ? color->surface.width : color->surface.width / 4;
   const uint32_t height = full ? color->surface.height : color->surface.height / 4;
   viewport(color, full);
   if (!drain()) {
      OSReport("GX2 timing could not drain preflight workload\n");
      return false;
   }

   reset_slot(&top);
   reset_slot(&bottom);
   const uint64_t reset_top = read_slot(&top), reset_bottom = read_slot(&bottom);
   const OSTime submitted_before = GX2GetLastSubmittedTimeStamp();
   const OSTime started = OSGetSystemTime();
   GX2SampleTopGPUCycle(&top.value);
   draw(count);
   GX2SampleBottomGPUCycle(&bottom.value);
   const OSTime built = OSGetSystemTime();
   // Distinguish a destination that the API initializes from a valid sample
   // that appeared before the explicit flush. Observe both raw values.
   const bool auto_submitted = GX2GetLastSubmittedTimeStamp() != submitted_before;
   const uint64_t early_top = read_slot(&top);
   const uint64_t early_bottom = read_slot(&bottom);
   const OSTime flushing = OSGetSystemTime();
   GX2Flush();
   const OSTime flushed = OSGetSystemTime();
   const OSTime submitted = GX2GetLastSubmittedTimeStamp();
   const OSTime retired_before_wait = GX2GetRetiredTimeStamp();
   const bool completed = GX2WaitTimeStamp(submitted);
   const OSTime waited = OSGetSystemTime();
   const OSTime retired = GX2GetRetiredTimeStamp();
   if (retired < submitted) {
      OSReport("GX2 timing fence failed wait=%u submitted=%016llx retired=%016llx\n",
               (unsigned)completed, (unsigned long long)submitted, (unsigned long long)retired);
      return false;
   }

   const uint64_t a = read_slot(&top), b = read_slot(&bottom);
   const bool written = valid(a) && valid(b) && b >= a;
   *saw_valid |= written;
   *saw_unsupported |= !written;
   OSReport("GX2 timing %u %ux%u draws=%u build_us=%u flush_us=%u fence_us=%u pending=%u\n",
            trial, width, height, count, usec(built - started), usec(flushed - flushing),
            usec(waited - flushed), (unsigned)(retired_before_wait < submitted));
   OSReport("GX2 cycles %u top=%016llx bottom=%016llx\n",
            trial, (unsigned long long)a, (unsigned long long)b);
   OSReport("GX2 span %u ticks=%016llx early=%u,%u auto=%u\n", trial,
            (unsigned long long)(written ? b - a : 0), (unsigned)(early_top != poison),
            (unsigned)(early_bottom != poison), (unsigned)auto_submitted);
   OSReport("GX2 timing preflush %u reset=%016llx,%016llx value=%016llx,%016llx\n", trial,
            (unsigned long long)reset_top, (unsigned long long)reset_bottom,
            (unsigned long long)early_top, (unsigned long long)early_bottom);
   return true;
}

// Compare separately drained samples across a CPU sleep. The reported relation
// calibrates the counter's units without assuming the GPU shader clock.
static bool calibrate(unsigned milliseconds)
{
   reset_slot(&top);
   reset_slot(&bottom);
   const OSTime first = OSGetSystemTime();
   GX2SampleBottomGPUCycle(&top.value);
   if (!drain()) return false;
   OSSleepTicks(OSMillisecondsToTicks(milliseconds));
   GX2SampleBottomGPUCycle(&bottom.value);
   if (!drain()) return false;
   const OSTime second = OSGetSystemTime();
   const uint64_t a = read_slot(&top), b = read_slot(&bottom);
   OSReport("GX2 timing calibration sleep_ms=%u cpu_us=%u gpu_ticks=%016llx\n", milliseconds,
            usec(second - first), (unsigned long long)(valid(a) && valid(b) && b > a ? b - a : 0));
   return true;
}

static void measure_presentation(void)
{
   OSTime last_flip, last_vsync;
   uint32_t swaps_before, flips_before, swaps, flips;
   GX2GetSwapStatus(&swaps_before, &flips_before, &last_flip, &last_vsync);
   const OSTime started = OSGetSystemTime();
   WHBGfxBeginRender();
   const OSTime begun = OSGetSystemTime();
   WHBGfxBeginRenderTV();
   WHBGfxClearColor(0.0f, 0.15f, 0.1f, 1.0f);
   WHBGfxFinishRenderTV();
   WHBGfxBeginRenderDRC();
   WHBGfxClearColor(0.0f, 0.15f, 0.1f, 1.0f);
   WHBGfxFinishRenderDRC();
   const OSTime copied = OSGetSystemTime();
   WHBGfxFinishRender();
   const OSTime finished = OSGetSystemTime();
   GX2GetSwapStatus(&swaps, &flips, &last_flip, &last_vsync);
   OSReport("GX2 timing present begin_us=%u copy_us=%u finish_us=%u swaps=%u flips=%u\n",
            usec(begun - started), usec(copied - begun), usec(finished - copied),
            swaps - swaps_before, flips - flips_before);
}

static bool measure_raster(const WHBGfxShaderGroup *shader)
{
   if (!foreground()) return false;
   WHBGfxBeginRender();
   WHBGfxBeginRenderTV();
   bind_pipeline(shader);
   GX2ColorBuffer *color = WHBGfxGetTVColourBuffer();
   bool valid_counter = false, invalid_counter = false;
   if (!workload(color, 0, true, 0, &valid_counter, &invalid_counter)) return false;
   for (unsigned i = 0; i < TRIALS; ++i) {
      const bool full = (i % 4 == 1) || (i % 4 == 2); // ABBA, repeated once
      if (!foreground() || !workload(color, DRAWS, full, i + 1,
                                      &valid_counter, &invalid_counter)) return false;
   }
   if (!calibrate(10) || !calibrate(40) || !foreground()) return false;
   WHBGfxFinishRenderTV();
   WHBGfxBeginRenderDRC();
   WHBGfxClearColor(0.0f, 0.0f, 0.0f, 1.0f);
   WHBGfxFinishRenderDRC();
   WHBGfxFinishRender();
   measure_presentation();
   OSReport("GX2 timing cycle samples state=%s\n", valid_counter ?
            (invalid_counter ? "mixed" : "available") : "unavailable");
   return true;
}

static void return_to_menu(void)
{
   if (WHBProcIsRunning()) {
      OSReport("GX2 timing requesting system menu\n");
      SYSLaunchMenu();
      const OSTime warning = OSGetSystemTime() + OSMillisecondsToTicks(5000);
      bool warned = false;
      while (WHBProcIsRunning()) {
         if (!warned && OSGetSystemTime() >= warning) {
            OSReport("GX2 timing waiting for ProcUI to acknowledge the menu request\n");
            warned = true;
         }
         OSSleepTicks(OSMillisecondsToTicks(16));
      }
   }
   OSReport("GX2 timing ProcUI exit complete\n");
}

int main(void)
{
   WHBProcInit();
   if (!WHBGfxInit()) {
      OSReport("GX2 TIMING FIXTURE FAILED graphics initialization\n");
      return_to_menu();
      WHBProcShutdown();
      return 1;
   }
   WHBGfxShaderGroup shader = {0};
   const bool loaded = WHBGfxLoadGFDShaderGroup(&shader, 0, _binary_timing_gsh_start);
   bool ok = loaded && WHBGfxInitShaderAttribute(&shader, "aPosition", 0, 0,
                        GX2_ATTRIB_FORMAT_FLOAT_32_32_32_32) && WHBGfxInitFetchShader(&shader);
   if (!ok) OSReport("GX2 timing shader setup failed load=%u\n", (unsigned)loaded);
   if (ok) {
      DCFlushRange(triangle, sizeof(triangle));
      ok = show_raster(&shader, false) && show_raster(&shader, true) && measure_raster(&shader);
   }
   if (targets_ready()) GX2DrawDone();
   if (loaded) WHBGfxFreeShaderGroup(&shader);
   OSReport("GX2 TIMING FIXTURE %s\n", ok ? "PASSED" : "FAILED");
   return_to_menu();
   WHBGfxShutdown();
   WHBProcShutdown();
   return ok ? 0 : 1;
}
