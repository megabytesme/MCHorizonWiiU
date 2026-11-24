#include <coreinit/cache.h>
#include <coreinit/debug.h>
#include <coreinit/memdefaultheap.h>
#include <coreinit/thread.h>
#include <coreinit/time.h>
#include <gx2/clear.h>
#include <gx2/context.h>
#include <gx2/draw.h>
#include <gx2/event.h>
#include <gx2/mem.h>
#include <gx2/registers.h>
#include <gx2/sampler.h>
#include <gx2/shaders.h>
#include <gx2/state.h>
#include <gx2/surface.h>
#include <gx2/texture.h>
#include <gx2/utils.h>
#include <proc_ui/procui.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <sysapp/launch.h>
#include <whb/gfx.h>
#include <whb/proc.h>

extern const uint8_t _binary_pattern_gsh_start[];
extern const uint8_t _binary_sample_gsh_start[];

enum { TARGET_WIDTH = 320, TARGET_HEIGHT = 192, STILL_MS = 7000 };

typedef struct {
    float position[2], uv[2];
} Vertex;

static _Alignas(GX2_VERTEX_BUFFER_ALIGNMENT) Vertex triangle[] = {
    {{-1,  1}, {0, 0}},
    {{-1, -3}, {0, 2}},
    {{ 3,  1}, {2, 0}},
};

typedef struct {
    GX2ColorBuffer color;
    GX2Texture texture;
    GX2ContextState *context;
} Offscreen;

typedef struct {
    WHBGfxShaderGroup group;
    bool loaded;
} Program;

enum Trial { DIRECT, WHB_SAMPLE, CLEAR_ONLY, DRAW_NO_CLEAR, CLEAR_DRAW };

static bool program_init(Program *program, const uint8_t *shader)
{
    program->loaded = WHBGfxLoadGFDShaderGroup(&program->group, 0, shader);
    return program->loaded &&
        WHBGfxInitShaderAttribute(&program->group, "position", 0,
            offsetof(Vertex, position), GX2_ATTRIB_FORMAT_FLOAT_32_32) &&
        WHBGfxInitShaderAttribute(&program->group, "uv", 0,
            offsetof(Vertex, uv), GX2_ATTRIB_FORMAT_FLOAT_32_32) &&
        WHBGfxInitFetchShader(&program->group);
}

static GX2Texture texture_view(const GX2Surface *surface)
{
    GX2Texture texture = {
        .surface = *surface,
        .viewNumMips = 1,
        .viewNumSlices = 1,
        .compMap = GX2_COMP_MAP(GX2_SQ_SEL_R, GX2_SQ_SEL_G, GX2_SQ_SEL_B, GX2_SQ_SEL_A),
    };
    GX2InitTextureRegs(&texture);
    return texture;
}

static bool offscreen_init(Offscreen *target)
{
    target->color.surface = (GX2Surface){
        .dim = GX2_SURFACE_DIM_TEXTURE_2D,
        .width = TARGET_WIDTH,
        .height = TARGET_HEIGHT,
        .depth = 1,
        .mipLevels = 1,
        .format = GX2_SURFACE_FORMAT_UNORM_R8_G8_B8_A8,
        .aa = GX2_AA_MODE1X,
        .use = GX2_SURFACE_USE_TEXTURE | GX2_SURFACE_USE_COLOR_BUFFER,
        .tileMode = GX2_TILE_MODE_DEFAULT,
    };
    GX2Surface *surface = &target->color.surface;
    GX2CalcSurfaceSizeAndAlignment(surface);
    if (!surface->alignment || surface->alignment > INT32_MAX ||
        !surface->imageSize || surface->imageSize > 1024 * 1024) return false;

    surface->image = MEMAllocFromDefaultHeapEx(surface->imageSize, (int32_t)surface->alignment);
    target->context = MEMAllocFromDefaultHeapEx(sizeof(*target->context), GX2_CONTEXT_STATE_ALIGNMENT);
    if (!surface->image || !target->context) return false;
    GX2Invalidate(GX2_INVALIDATE_MODE_CPU, surface->image, surface->imageSize);

    target->color.viewNumSlices = 1;
    GX2InitColorBufferRegs(&target->color);
    target->texture = texture_view(surface);

    GX2SetupContextStateEx(target->context, TRUE);
    GX2SetContextState(target->context);
    GX2SetColorBuffer(&target->color, GX2_RENDER_TARGET_0);
    GX2SetViewport(0, 0, surface->width, surface->height, 0, 1);
    GX2SetScissor(0, 0, surface->width, surface->height);
    GX2SetContextState(NULL);

    OSReport("GX2 offscreen reference layout %ux%u pitch=%u tile=%u use=%08x bytes=%u alignment=%u\n",
        surface->width, surface->height, surface->pitch, surface->tileMode,
        surface->use, surface->imageSize, surface->alignment);
    return true;
}

static void draw(const Program *program, const GX2ColorBuffer *target,
                 const GX2Texture *source, const GX2Sampler *sampler)
{
    const WHBGfxShaderGroup *shader = &program->group;
    GX2SetViewport(0, 0, target->surface.width, target->surface.height, 0.0f, 1.0f);
    GX2SetScissor(0, 0, target->surface.width, target->surface.height);
    GX2SetShaderMode(shader->vertexShader->mode);
    GX2SetVertexShader(shader->vertexShader);
    GX2SetPixelShader(shader->pixelShader);
    GX2SetFetchShader(&shader->fetchShader);
    GX2SetAttribBuffer(0, sizeof(triangle), sizeof(triangle[0]), triangle);

    GX2SetRasterizerClipControlEx(TRUE, TRUE, TRUE);
    GX2SetCullOnlyControl(GX2_FRONT_FACE_CCW, FALSE, FALSE);
    GX2SetDepthOnlyControl(FALSE, FALSE, GX2_COMPARE_FUNC_ALWAYS);
    GX2SetAlphaTest(FALSE, GX2_COMPARE_FUNC_ALWAYS, 0.0f);
    GX2SetTargetChannelMasks(GX2_CHANNEL_MASK_RGBA, 0, 0, 0, 0, 0, 0, 0);
    GX2SetColorControl(GX2_LOGIC_OP_COPY, 1, FALSE, TRUE);
    GX2SetBlendControl(GX2_RENDER_TARGET_0, GX2_BLEND_MODE_ONE, GX2_BLEND_MODE_ZERO,
        GX2_BLEND_COMBINE_MODE_ADD, FALSE, GX2_BLEND_MODE_ONE, GX2_BLEND_MODE_ZERO,
        GX2_BLEND_COMBINE_MODE_ADD);

    if (source) {
        const uint32_t slot = shader->pixelShader->samplerVars[0].location;
        GX2SetPixelTexture(source, slot);
        GX2SetPixelSampler(sampler, slot);
    }
    GX2DrawEx(GX2_PRIMITIVE_MODE_TRIANGLES, 3, 0, 1);
}

static bool displays_ready(void)
{
    return ProcUIInForeground() && WHBGfxGetTVColourBuffer()->surface.image &&
        WHBGfxGetDRCColourBuffer()->surface.image;
}

static void finish_producer(const GX2Surface *surface)
{
    GX2Invalidate(GX2_INVALIDATE_MODE_CPU_TEXTURE | GX2_INVALIDATE_MODE_COLOR_BUFFER,
        surface->image, surface->imageSize);
    GX2DrawDone();
    GX2SetContextState(NULL);
    GX2Flush();
}

static void present(enum Trial trial, Offscreen *target, const Program *pattern,
                    const Program *sample, const GX2Sampler *sampler)
{
    const GX2ColorBuffer *tv = WHBGfxGetTVColourBuffer();
    const GX2ColorBuffer *drc = WHBGfxGetDRCColourBuffer();
    const bool manual_source = trial != DIRECT && trial != WHB_SAMPLE;
    const Program *program = manual_source ? sample : pattern;
    const GX2Texture *source = manual_source ? &target->texture : NULL;
    WHBGfxBeginRender();

    if (manual_source) {
        if (trial != DRAW_NO_CLEAR)
            GX2ClearColor(&target->color, 0.0f, 1.0f, 0.0f, 1.0f);
        GX2SetContextState(target->context);
        if (trial != CLEAR_ONLY)
            draw(pattern, &target->color, NULL, NULL);
        finish_producer(&target->color.surface);
    }

    WHBGfxBeginRenderDRC();
    WHBGfxClearColor(0.18f, 0.18f, 0.18f, 1.0f);
    draw(program, drc, source, sampler);
    WHBGfxFinishRenderDRC();

    GX2Texture known_source;
    if (trial == WHB_SAMPLE) {
        finish_producer(&drc->surface);
        known_source = texture_view(&drc->surface);
        program = sample;
        source = &known_source;
    }
    WHBGfxBeginRenderTV();
    WHBGfxClearColor(0.18f, 0.18f, 0.18f, 1.0f);
    draw(program, tv, source, sampler);
    WHBGfxFinishRenderTV();
    WHBGfxFinishRender();
}

static bool run_trials(Offscreen *target, const Program *pattern,
                       const Program *sample, const GX2Sampler *sampler)
{
    static const struct {
        enum Trial trial;
        const char *name;
    } trials[] = {
        {DIRECT, "direct"},
        {WHB_SAMPLE, "whb-sample"},
        {CLEAR_ONLY, "clear-only"},
        {DRAW_NO_CLEAR, "draw-no-clear"},
        {CLEAR_DRAW, "clear-draw"},
    };
    for (unsigned i = 0; i < sizeof(trials) / sizeof(trials[0]); ++i) {
        const OSTime deadline = OSGetSystemTime() + OSMillisecondsToTicks(STILL_MS);
        bool shown = false;
        do {
            if (!WHBProcIsRunning() || !displays_ready()) return false;
            present(trials[i].trial, target, pattern, sample, sampler);
            if (!shown) {
                OSReport("GX2 offscreen reference still trial=%u mode=%s\n", i, trials[i].name);
                shown = true;
            }
            OSSleepTicks(OSMillisecondsToTicks(16));
        } while (OSGetSystemTime() < deadline);
    }
    return true;
}

static void return_to_menu(void)
{
    if (!WHBProcIsRunning()) return;
    OSReport("GX2 offscreen reference requesting system menu\n");
    SYSLaunchMenu();
    while (WHBProcIsRunning()) OSSleepTicks(OSMillisecondsToTicks(16));
    OSReport("GX2 offscreen reference ProcUI exit complete\n");
}

int main(void)
{
    WHBProcInit();
    const bool graphics = WHBGfxInit();
    Offscreen target = {0};
    Program pattern = {0}, sample = {0};
    GX2Sampler sampler;
    bool submitted = false;

    if (graphics && displays_ready() &&
        program_init(&pattern, _binary_pattern_gsh_start) &&
        program_init(&sample, _binary_sample_gsh_start) &&
        sample.group.pixelShader->samplerVarCount == 1 &&
        sample.group.pixelShader->samplerVars &&
        offscreen_init(&target)) {
        DCFlushRange(triangle, sizeof(triangle));
        GX2InitSampler(&sampler, GX2_TEX_CLAMP_MODE_CLAMP, GX2_TEX_XY_FILTER_MODE_POINT);
        submitted = run_trials(&target, &pattern, &sample, &sampler);
    }

    if (graphics && displays_ready()) GX2DrawDone();
    if (target.context) MEMFreeToDefaultHeap(target.context);
    if (target.color.surface.image) MEMFreeToDefaultHeap(target.color.surface.image);
    if (sample.loaded) WHBGfxFreeShaderGroup(&sample.group);
    if (pattern.loaded) WHBGfxFreeShaderGroup(&pattern.group);
    OSReport("GX2 offscreen reference result=%s\n", submitted ? "SUBMITTED" : "INTERRUPTED");
    return_to_menu();
    if (graphics) WHBGfxShutdown();
    WHBProcShutdown();
    return submitted ? 0 : 1;
}
