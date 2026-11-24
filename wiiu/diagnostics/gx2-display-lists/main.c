#include <coreinit/cache.h>
#include <coreinit/debug.h>
#include <coreinit/thread.h>
#include <coreinit/time.h>
#include <gx2/displaylist.h>
#include <gx2/draw.h>
#include <gx2/event.h>
#include <gx2/registers.h>
#include <gx2/shaders.h>
#include <gx2/state.h>
#include <gx2/surface.h>
#include <proc_ui/procui.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>
#include <sysapp/launch.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern const uint8_t _binary_commands_gsh_start[];

enum {
   SIDE = 8,
   SECTIONS = SIDE * SIDE,
   TARGETS = 2,
   UNIFORM_WORDS = 20,
   ALIGNMENT = 256,
   SECTION_LIST_BYTES = 256,
   FRAME_LIST_BYTES = SECTIONS * 256,
   SAMPLES_PER_ARM = 8,
   ARM_HOLD_SECONDS = 7,
};

typedef struct {
   float position[4];
   uint32_t unused[3];
} Vertex;

typedef struct {
   _Alignas(ALIGNMENT) Vertex vertices[4];
} Mesh;

typedef struct {
   _Alignas(ALIGNMENT) uint8_t bytes[SECTION_LIST_BYTES];
   uint32_t used;
} SectionList;

typedef struct {
   _Alignas(ALIGNMENT) uint8_t bytes[FRAME_LIST_BYTES];
   uint32_t used;
} FrameList;

enum SubmitMode {
   IMMEDIATE,
   SECTION_LISTS,
   FRAME_LIST,
};

static const char *const mode_names[] = {"immediate", "section-lists", "frame-list"};
static const enum SubmitMode arms[] = {IMMEDIATE, SECTION_LISTS, FRAME_LIST,
                                      FRAME_LIST, SECTION_LISTS, IMMEDIATE};
static Mesh meshes[SECTIONS];
static _Alignas(ALIGNMENT) uint16_t indices[] = {0, 1, 2, 0, 2, 3};
static _Alignas(16) float uniforms[TARGETS][SECTIONS][UNIFORM_WORDS];
static SectionList section_lists[SECTIONS];
static FrameList frame_lists[TARGETS];
static bool proc_running = true;

_Static_assert(sizeof(Vertex) == 28, "match indexed terrain stride");

static bool outputs_ready(void)
{
   proc_running = WHBProcIsRunning();
   return proc_running && ProcUIInForeground() &&
      WHBGfxGetTVColourBuffer()->surface.image && WHBGfxGetTVDepthBuffer()->surface.image &&
      WHBGfxGetDRCColourBuffer()->surface.image && WHBGfxGetDRCDepthBuffer()->surface.image;
}

static void prepare_meshes(void)
{
   for (unsigned i = 0; i < SECTIONS; ++i) {
      // Alternating widths make a bad attribute address visible in neighbouring tiles.
      const float right = i % 2 ? 0.10f : 0.075f;
      const float points[4][2] = {{-0.1f, -0.1f}, {right, -0.1f}, {right, 0.1f}, {-0.1f, 0.1f}};
      for (unsigned v = 0; v < 4; ++v) {
         meshes[i].vertices[v] = (Vertex){
            .position = {points[v][0], points[v][1], 0.5f, 1.0f},
         };
      }
      for (unsigned target = 0; target < TARGETS; ++target) {
         const unsigned column = i % SIDE, row = i / SIDE;
         const bool top = row >= SIDE / 2;
         const bool red = target == 0 ? top : !top;
         const bool marker = (target == 0 && row == SIDE - 1 && column == 0) ||
            (target == 1 && row == 0 && column == SIDE - 1);
         const float x = -0.875f + 0.25f * column;
         const float y = -0.875f + 0.25f * row;
         const float values[UNIFORM_WORDS] = {
            1, 0, 0, x, 0, 1, 0, y, 0, 0, 1, 0, 0, 0, 0, 1,
            !marker && red, marker, !marker && !red, 0,
         };
         memcpy(uniforms[target][i], values, sizeof(values));
      }
   }
   DCFlushRange(meshes, sizeof(meshes));
   DCFlushRange(indices, sizeof(indices));
}

static bool reflected_uniforms(const GX2VertexShader *shader, uint32_t *start)
{
   const char *const names[] = {"uClipX", "uClipY", "uClipZ", "uClipW", "uTileColor"};
   uint32_t offsets[5];
   for (unsigned i = 0; i < sizeof(offsets) / sizeof(offsets[0]); ++i) {
      const GX2UniformVar *match = NULL;
      for (uint32_t j = 0; j < shader->uniformVarCount; ++j) {
         const GX2UniformVar *value = &shader->uniformVars[j];
         if (strcmp(value->name, names[i]) == 0) match = value;
      }
      if (!match) return false;
      offsets[i] = match->offset;
      if (offsets[i] != offsets[0] + i * 4) return false;
   }
   *start = offsets[0];
   return true;
}

static void draw_mesh(unsigned section)
{
   // Keep GX2R resource operations outside lists. The raw commands reference
   // immutable GPU-visible addresses until the final draw completes.
   GX2SetAttribBuffer(0, sizeof(meshes[section].vertices), sizeof(Vertex), meshes[section].vertices);
   GX2DrawIndexedEx(GX2_PRIMITIVE_MODE_TRIANGLES, 6, GX2_INDEX_TYPE_U16, indices, 0, 1);
}

static void submit(unsigned target, uint32_t offset, enum SubmitMode mode)
{
   if (mode == FRAME_LIST) {
      GX2CallDisplayList(frame_lists[target].bytes, frame_lists[target].used);
      return;
   }
   for (unsigned i = 0; i < SECTIONS; ++i) {
      GX2SetVertexUniformReg(offset, UNIFORM_WORDS, uniforms[target][i]);
      if (mode == SECTION_LISTS)
         GX2CallDisplayList(section_lists[i].bytes, section_lists[i].used);
      else
         draw_mesh(i);
   }
}

static bool finish_list(void *storage, uint32_t capacity, uint32_t *used)
{
   const bool recording = GX2GetDisplayListWriteStatus();
   *used = GX2EndDisplayList(storage);
   if (!recording || GX2GetDisplayListWriteStatus() || !*used || *used % 4 || *used > capacity)
      return false;
   DCFlushRange(storage, *used);
   return true;
}

static bool record_lists(uint32_t offset)
{
   const OSTime section_start = OSGetSystemTime();
   uint32_t section_bytes = 0;
   for (unsigned i = 0; i < SECTIONS; ++i) {
      SectionList *list = &section_lists[i];
      GX2BeginDisplayListEx(list->bytes, sizeof(list->bytes), FALSE);
      if (!GX2GetDisplayListWriteStatus()) return false;
      draw_mesh(i);
      if (!finish_list(list->bytes, sizeof(list->bytes), &list->used)) return false;
      section_bytes += list->used;
   }
   const OSTime section_done = OSGetSystemTime();
   for (unsigned target = 0; target < TARGETS; ++target) {
      FrameList *list = &frame_lists[target];
      GX2BeginDisplayListEx(list->bytes, sizeof(list->bytes), FALSE);
      if (!GX2GetDisplayListWriteStatus()) return false;
      submit(target, offset, IMMEDIATE);
      if (!finish_list(list->bytes, sizeof(list->bytes), &list->used)) return false;
   }
   const OSTime frame_done = OSGetSystemTime();
   OSReport("GX2 display lists records sections=%u section_bytes=%u section_capacity=%u section_us=%u "
            "tv_frame_bytes=%u drc_frame_bytes=%u frame_capacity=%u pair_record_us=%u reserved_bytes=%u\n",
            SECTIONS, section_bytes, SECTIONS * SECTION_LIST_BYTES,
            (unsigned)OSTicksToMicroseconds(section_done - section_start),
            frame_lists[0].used, frame_lists[1].used, TARGETS * FRAME_LIST_BYTES,
            (unsigned)OSTicksToMicroseconds(frame_done - section_done),
            (unsigned)(sizeof(section_lists) + sizeof(frame_lists)));
   return true;
}

static void start_output(unsigned target, const WHBGfxShaderGroup *shader)
{
   const GX2ColorBuffer *color;
   if (target == 0) {
      WHBGfxBeginRenderTV();
      color = WHBGfxGetTVColourBuffer();
   } else {
      WHBGfxBeginRenderDRC();
      color = WHBGfxGetDRCColourBuffer();
   }
   WHBGfxClearColor(0.0f, 0.0f, 0.0f, 1.0f);
   GX2SetViewport(0, 0, color->surface.width, color->surface.height, 0, 1);
   GX2SetScissor(0, 0, color->surface.width, color->surface.height);
   GX2SetShaderMode(shader->vertexShader->mode);
   GX2SetVertexShader(shader->vertexShader);
   GX2SetPixelShader(shader->pixelShader);
   GX2SetFetchShader(&shader->fetchShader);
   GX2SetDepthOnlyControl(TRUE, TRUE, GX2_COMPARE_FUNC_LEQUAL);
   GX2SetCullOnlyControl(GX2_FRONT_FACE_CCW, FALSE, FALSE);
   GX2SetAlphaTest(FALSE, GX2_COMPARE_FUNC_ALWAYS, 0);
   GX2SetColorControl(GX2_LOGIC_OP_COPY, 0, FALSE, TRUE);
   GX2SetTargetChannelMasks(GX2_CHANNEL_MASK_RGBA, 0, 0, 0, 0, 0, 0, 0);
}

static unsigned ticks32(OSTime elapsed)
{
   return elapsed > UINT32_MAX ? UINT32_MAX : (unsigned)elapsed;
}

static void present(const WHBGfxShaderGroup *shader, uint32_t offset, unsigned arm, unsigned sample)
{
   const enum SubmitMode mode = arms[arm];
   WHBGfxBeginRender();
   for (unsigned target = 0; target < TARGETS; ++target) {
      start_output(target, shader);
      if (sample > 0 && sample <= SAMPLES_PER_ARM) {
         // Separate the fixture's state setup from the measured command stream.
         GX2DrawDone();
         const OSTime before = GX2GetLastSubmittedTimeStamp();
         const OSTime start = OSGetSystemTime();
         submit(target, offset, mode);
         const OSTime built = OSGetSystemTime();
         const bool auto_submitted = GX2GetLastSubmittedTimeStamp() != before;
         const OSTime flush_start = OSGetSystemTime();
         GX2Flush();
         const OSTime flushed = OSGetSystemTime();
         const OSTime watermark = GX2GetLastSubmittedTimeStamp();
         GX2WaitTimeStamp(watermark);
         const OSTime retired = OSGetSystemTime();
         OSReport("GX2 display lists sample arm=%u frame=%u target=%u draws=%u indices=%u "
                  "build=%u flush=%u drain=%u auto=%u retired=%u\n",
                  arm, sample, target, SECTIONS, SECTIONS * 6,
                  ticks32(built - start), ticks32(flushed - flush_start), ticks32(retired - flushed),
                  auto_submitted, watermark != before && GX2GetRetiredTimeStamp() >= watermark);
      } else {
         submit(target, offset, mode);
      }
      if (target == 0) WHBGfxFinishRenderTV();
      else WHBGfxFinishRenderDRC();
   }
   WHBGfxFinishRender();
}

int main(void)
{
   WHBProcInit();
   const bool graphics = WHBGfxInit();
   WHBGfxShaderGroup shader = {0};
   const bool loaded = graphics && outputs_ready() &&
      WHBGfxLoadGFDShaderGroup(&shader, 0, _binary_commands_gsh_start);
   uint32_t offset = 0;
   bool ok = loaded && WHBGfxInitShaderAttribute(&shader, "aPosition", 0, 0,
      GX2_ATTRIB_FORMAT_FLOAT_32_32_32_32) && WHBGfxInitFetchShader(&shader) &&
      shader.vertexShader->mode == GX2_SHADER_MODE_UNIFORM_REGISTER &&
      reflected_uniforms(shader.vertexShader, &offset);
   if (ok) {
      prepare_meshes();
      GX2SetShaderMode(GX2_SHADER_MODE_UNIFORM_REGISTER);
      ok = record_lists(offset);
   }
   OSReport("GX2 display lists setup=%s offset=%u clock_hz=%u\n",
            ok ? "READY" : "FAILED", offset, (unsigned)OSSecondsToTicks(1));
   for (unsigned arm = 0; ok && arm < sizeof(arms) / sizeof(arms[0]); ++arm) {
      const OSTime deadline = OSGetSystemTime() + OSSecondsToTicks(ARM_HOLD_SECONDS);
      unsigned sample = 0;
      do {
         ok = outputs_ready();
         if (!ok) break;
         present(&shader, offset, arm, sample);
         if (sample == 0)
            OSReport("GX2 display lists visual arm=%u mode=%s hold_seconds=%u\n",
                     arm, mode_names[arms[arm]], ARM_HOLD_SECONDS);
         ++sample;
         OSSleepTicks(OSMillisecondsToTicks(16));
      } while (sample <= SAMPLES_PER_ARM || OSGetSystemTime() < deadline);
   }
   if (loaded) WHBGfxFreeShaderGroup(&shader);
   OSReport("GX2 display lists diagnostic result=%s\n", ok ? "SUBMITTED" : "FAILED");
   if (proc_running && WHBProcIsRunning()) {
      SYSLaunchMenu();
      while (WHBProcIsRunning()) OSSleepTicks(OSMillisecondsToTicks(20));
   }
   if (graphics) WHBGfxShutdown();
   return ok ? 0 : 1;
}
