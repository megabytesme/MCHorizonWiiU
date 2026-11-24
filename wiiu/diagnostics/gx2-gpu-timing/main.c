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
#include <gx2r/surface.h>
#include <stdbool.h>
#include <stdint.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern const uint8_t _binary_timing_gsh_start[];

// WUT exposes these symbols in its import archive, without public prototypes.
// Both Cemu and SwitchWakerHD receive an aligned 64-bit destination in r3.
// The observed ABI is used only by this qualification fixture.
extern void GX2SampleTopGPUCycle(uint64_t *destination);
extern void GX2SampleBottomGPUCycle(uint64_t *destination);

enum { DRAWS = 32, TRIALS = 8, CACHE_BYTES = 64 };
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
   return slot->value;
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
   WHBGfxClearColor(0.0f, 0.0f, 0.0f, 1.0f);
   GX2SetViewport(0.0f, 0.0f, width, height, 0.0f, 1.0f);
   GX2SetScissor(0, 0, width, height);
}

static void draw(unsigned count)
{
   for (unsigned i = 0; i < count; ++i) GX2DrawEx(GX2_PRIMITIVE_MODE_TRIANGLES, 3, 0, 1);
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
   const OSTime submitted_before = GX2GetLastSubmittedTimeStamp();
   const OSTime started = OSGetSystemTime();
   GX2SampleTopGPUCycle(&top.value);
   draw(count);
   GX2SampleBottomGPUCycle(&bottom.value);
   const OSTime built = OSGetSystemTime();
   // An immediate write before GX2Flush identifies an emulator stub when the
   // queued batch did not submit implicitly during command construction.
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
   return true;
}

static bool check_raster(GX2ColorBuffer *color, bool full)
{
   const GX2Surface *source = &color->surface;
   GX2Surface copy = {
      .dim = GX2_SURFACE_DIM_TEXTURE_2D, .width = source->width, .height = source->height,
      .depth = 1, .mipLevels = 1, .format = source->format,
      .aa = GX2_AA_MODE1X, .tileMode = GX2_TILE_MODE_LINEAR_SPECIAL,
   };
   const GX2RResourceFlags flags = GX2R_RESOURCE_BIND_TEXTURE |
      GX2R_RESOURCE_USAGE_CPU_READ | GX2R_RESOURCE_USAGE_GPU_WRITE;
   if (!GX2RCreateSurface(&copy, flags)) return false;
   GX2CopySurface(source, 0, 0, &copy, 0, 0);
   bool ok = false;
   if (drain()) {
      const uint8_t *pixels = GX2RLockSurfaceEx(&copy, 0, flags);
      if (pixels) {
         const size_t corner = ((size_t)(copy.height / 8) * copy.pitch + copy.width / 8) * 4;
         const size_t center = ((size_t)(copy.height / 2) * copy.pitch + copy.width / 2) * 4;
         ok = pixels[corner] > 140 && (full ? pixels[center] > 140 : pixels[center] == 0);
         OSReport("GX2 timing raster full=%u corner_r=%u center_r=%u pass=%u\n",
                  (unsigned)full, pixels[corner], pixels[center], (unsigned)ok);
         GX2RUnlockSurfaceEx(&copy, 0, flags);
      }
   }
   GX2RDestroySurfaceEx(&copy, 0);
   return ok;
}

// Compare separately drained samples across a CPU sleep. The reported relation
// calibrates the counter's units without assuming the GPU shader clock.
static void calibrate(unsigned milliseconds)
{
   reset_slot(&top);
   reset_slot(&bottom);
   const OSTime first = OSGetSystemTime();
   GX2SampleBottomGPUCycle(&top.value);
   if (!drain()) return;
   OSSleepTicks(OSMillisecondsToTicks(milliseconds));
   GX2SampleBottomGPUCycle(&bottom.value);
   if (!drain()) return;
   const OSTime second = OSGetSystemTime();
   const uint64_t a = read_slot(&top), b = read_slot(&bottom);
   OSReport("GX2 timing calibration sleep_ms=%u cpu_us=%u gpu_ticks=%016llx\n", milliseconds,
            usec(second - first), (unsigned long long)(valid(a) && valid(b) && b > a ? b - a : 0));
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

int main(void)
{
   WHBProcInit();
   if (!WHBGfxInit()) {
      WHBProcShutdown();
      return 1;
   }
   WHBGfxShaderGroup shader = {0};
   const bool loaded = WHBGfxLoadGFDShaderGroup(&shader, 0, _binary_timing_gsh_start);
   bool ok = loaded && WHBGfxInitShaderAttribute(&shader, "aPosition", 0, 0,
                        GX2_ATTRIB_FORMAT_FLOAT_32_32_32_32) && WHBGfxInitFetchShader(&shader);
   if (!ok) OSReport("GX2 timing shader setup failed load=%u\n", (unsigned)loaded);
   if (ok) {
      GX2ColorBuffer *color = WHBGfxGetTVColourBuffer();
      WHBGfxBeginRenderTV();
      GX2SetDepthOnlyControl(false, false, GX2_COMPARE_FUNC_ALWAYS);
      GX2SetCullOnlyControl(GX2_FRONT_FACE_CCW, false, false);
      GX2SetColorControl(GX2_LOGIC_OP_COPY, 1, false, true);
      GX2SetBlendControl(GX2_RENDER_TARGET_0, GX2_BLEND_MODE_SRC_ALPHA,
                         GX2_BLEND_MODE_INV_SRC_ALPHA, GX2_BLEND_COMBINE_MODE_ADD, false,
                         GX2_BLEND_MODE_ONE, GX2_BLEND_MODE_ZERO, GX2_BLEND_COMBINE_MODE_ADD);
      GX2SetShaderMode(GX2_SHADER_MODE_UNIFORM_REGISTER);
      GX2SetFetchShader(&shader.fetchShader);
      GX2SetVertexShader(shader.vertexShader);
      GX2SetPixelShader(shader.pixelShader);
      DCFlushRange(triangle, sizeof(triangle));
      GX2SetAttribBuffer(0, sizeof(triangle), sizeof(triangle[0]), triangle);

      viewport(color, false);
      draw(DRAWS);
      ok = drain() && check_raster(color, false);
      if (ok) {
         viewport(color, true);
         draw(DRAWS);
         ok = drain() && check_raster(color, true);
      }

      bool valid_counter = false, invalid_counter = false;
      ok = ok && workload(color, 0, true, 0, &valid_counter, &invalid_counter);
      for (unsigned i = 0; i < TRIALS && ok; ++i) {
         const bool full = (i % 4 == 1) || (i % 4 == 2); // ABBA, repeated once
         ok = WHBProcIsRunning() && workload(color, DRAWS, full, i + 1,
                                            &valid_counter, &invalid_counter);
      }
      if (ok) {
         calibrate(10);
         calibrate(40);
         measure_presentation();
         OSReport("GX2 timing cycle samples state=%s\n", valid_counter ?
                  (invalid_counter ? "mixed" : "available") : "unavailable");
      }
      GX2DrawDone();
   }
   if (loaded) WHBGfxFreeShaderGroup(&shader);
   OSReport("GX2 TIMING FIXTURE %s\n", ok ? "PASSED" : "FAILED");
   WHBGfxShutdown();
   WHBProcShutdown();
   return ok ? 0 : 1;
}
