#include <EGL/egl.h>
#define GL_GLEXT_PROTOTYPES 1
#include <GL/glcorearb.h>
#include <array>
#include <bit>
#include <cassert>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <string>
#include <utility>
#include <vector>

using uint32 = std::uint32_t;
namespace LatteConst { enum class VertexFetchEndianMode; }
namespace Latte { struct GPU_LIMITS { static constexpr uint32 NUM_VERTEX_BUFFERS = 16; }; }
enum class RendererAPI { OpenGL, Vulkan };
enum class LogType { Force };
constexpr int GLVENDOR_AMD = 1;
struct { int glVendor = 0; } LatteGPUState;
struct Renderer {
    RendererAPI api = RendererAPI::OpenGL;
    RendererAPI GetType() const { return api; }
} renderer;
Renderer *g_renderer = &renderer;

struct LatteDecompilerShader {};
struct LatteParsedFetchShaderAttribute_t {
    uint32 format, nfa, isSigned;
    LatteConst::VertexFetchEndianMode endianSwap;
    uint32 offset = 6, semanticId = 0, attributeBufferIndex = 0;
};
struct StringBuf {
    std::string text;
    void add(const char *source) { text += source; }
    void addFmt(std::string source, uint32 semantic)
    {
        source.replace(source.find("{}"), 2, std::to_string(semantic));
        text += source;
    }
};
unsigned unsupported_count = 0, assert_count = 0;
template<class... Args> void cemuLog_log(LogType, Args...) { ++unsupported_count; }
template<class... Args> void cemuLog_logDebug(LogType, Args...) { ++unsupported_count; }
void cemu_assert_unimplemented() { ++assert_count; }
void cemu_assert_debug(bool passed) { if (!passed) ++assert_count; }

#include "exact_emitter.inc"

using Emitter = decltype(before::LatteDecompiler_emitAttributeDecodeGLSL);
using Endian = LatteConst::VertexFetchEndianMode;
struct Emission {
    std::string text;
    unsigned unsupported, assertions;
    bool operator==(const Emission &) const = default;
};

static Emission emit(Emitter &emitter, LatteParsedFetchShaderAttribute_t attribute)
{
    StringBuf output;
    unsupported_count = assert_count = 0;
    emitter(nullptr, &output, &attribute);
    return {std::move(output.text), unsupported_count, assert_count};
}

static Emission old_emit(LatteParsedFetchShaderAttribute_t attribute)
{
    return emit(before::LatteDecompiler_emitAttributeDecodeGLSL, attribute);
}

static Emission new_emit(LatteParsedFetchShaderAttribute_t attribute)
{
    return emit(after::LatteDecompiler_emitAttributeDecodeGLSL, attribute);
}

static void check_admission()
{
    unsigned cases = 0, changed = 0;
    for (RendererAPI api : {RendererAPI::OpenGL, RendererAPI::Vulkan})
    for (int vendor : {0, GLVENDOR_AMD})
    for (uint32 format = 0; format != 64; ++format)
    for (uint32 nfa = 0; nfa != 4; ++nfa)
    for (uint32 sign = 0; sign != 2; ++sign)
    for (auto endian : {Endian::SWAP_NONE, Endian::SWAP_U16, Endian::SWAP_U32,
                        Endian::SWAP_DEFAULT})
    for (uint32 buffer : {0u, 16u}) {
        renderer.api = api;
        LatteGPUState.glVendor = vendor;
        const LatteParsedFetchShaderAttribute_t a{format, nfa, sign, endian, 6, 0, buffer};
        const auto old = old_emit(a);
        const auto patched = new_emit(a);
        ++cases;
        const bool desired = format == FMT_16 && nfa == 2 && sign == 0 &&
                             endian == Endian::SWAP_U16 && buffer == 0;
        if (desired) {
            assert(old.text.empty() && old.unsupported == 1 && old.assertions == 0);
            assert(!patched.text.empty() && patched.unsupported == 0 && patched.assertions == 0);
            ++changed;
        } else {
            assert(old == patched);
        }
    }
    assert(changed == 4);
    std::printf("exact emitter matrix cases=%u changed=%u (one descriptor)\n", cases, changed);
}

static GLuint compile(GLenum type, const std::string &source)
{
    const GLuint shader = glCreateShader(type);
    const char *pointer = source.c_str();
    glShaderSource(shader, 1, &pointer, nullptr);
    glCompileShader(shader);
    GLint good = 0;
    glGetShaderiv(shader, GL_COMPILE_STATUS, &good);
    if (!good) {
        char log[4096];
        glGetShaderInfoLog(shader, sizeof(log), nullptr, log);
        std::fprintf(stderr, "%s\n%s\n", source.c_str(), log);
        std::abort();
    }
    return shader;
}

static GLuint link(const std::string &emission, char lane, uint32 one)
{
    const std::string vertex = std::string(
        "#version 430 core\n"
        "layout(location=0) in uvec4 attrDataSem0;\n"
        "flat out uvec4 capture;\n"
        "void main() {\n uvec4 attrDecoder;\n") + emission +
        "capture = uvec4(attrDecoder." + lane + ", attrDecoder.z, attrDecoder.w, " +
        std::to_string(one) + "u);\ngl_Position = vec4(0.0);\n}\n";
    const GLuint shader = compile(GL_VERTEX_SHADER, vertex);
    const GLuint program = glCreateProgram();
    glAttachShader(program, shader);
    const char *output = "capture";
    glTransformFeedbackVaryings(program, 1, &output, GL_INTERLEAVED_ATTRIBS);
    glLinkProgram(program);
    GLint good = 0;
    glGetProgramiv(program, GL_LINK_STATUS, &good);
    if (!good) {
        char log[4096];
        glGetProgramInfoLog(program, sizeof(log), nullptr, log);
        std::fprintf(stderr, "%s\n", log);
        std::abort();
    }
    glDeleteShader(shader);
    return program;
}

using Vertex = std::array<std::uint8_t, 20>;
using Capture = std::array<uint32, 4>;
static std::vector<Capture> draw(GLuint input, const Emission &emission,
                                 GLint components, std::uintptr_t offset,
                                 char lane, uint32 one, std::size_t count)
{
    const GLuint program = link(emission.text, lane, one);
    GLuint output = 0;
    glGenBuffers(1, &output);
    glBindBuffer(GL_TRANSFORM_FEEDBACK_BUFFER, output);
    glBufferData(GL_TRANSFORM_FEEDBACK_BUFFER, count * sizeof(Capture), nullptr, GL_STREAM_READ);
    glBindBufferBase(GL_TRANSFORM_FEEDBACK_BUFFER, 0, output);
    glBindBuffer(GL_ARRAY_BUFFER, input);
    glVertexAttribIPointer(0, components, GL_UNSIGNED_SHORT, sizeof(Vertex),
                           reinterpret_cast<const void *>(offset));
    glEnableVertexAttribArray(0);
    glUseProgram(program);
    glEnable(GL_RASTERIZER_DISCARD);
    glBeginTransformFeedback(GL_POINTS);
    glDrawArrays(GL_POINTS, 0, static_cast<GLsizei>(count));
    glEndTransformFeedback();
    std::vector<Capture> actual(count);
    glGetBufferSubData(GL_TRANSFORM_FEEDBACK_BUFFER, 0, count * sizeof(Capture), actual.data());
    assert(glGetError() == GL_NO_ERROR);
    glDeleteBuffers(1, &output);
    glDeleteProgram(program);
    return actual;
}

static uint32 fp(float value) { return std::bit_cast<uint32>(value); }

static void check_readback()
{
    assert(std::endian::native == std::endian::little);
    EGLDisplay display = eglGetDisplay(EGL_DEFAULT_DISPLAY);
    assert(display != EGL_NO_DISPLAY && eglInitialize(display, nullptr, nullptr));
    assert(eglBindAPI(EGL_OPENGL_API));
    const EGLint config_attribs[] = {
        EGL_SURFACE_TYPE, EGL_PBUFFER_BIT, EGL_RENDERABLE_TYPE, EGL_OPENGL_BIT, EGL_NONE
    };
    EGLConfig config = nullptr;
    EGLint available = 0;
    assert(eglChooseConfig(display, config_attribs, &config, 1, &available) && available == 1);
    const EGLint context_attribs[] = {
        EGL_CONTEXT_MAJOR_VERSION, 4, EGL_CONTEXT_MINOR_VERSION, 3,
        EGL_CONTEXT_OPENGL_PROFILE_MASK, EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT, EGL_NONE
    };
    const EGLint surface_attribs[] = {EGL_WIDTH, 1, EGL_HEIGHT, 1, EGL_NONE};
    EGLContext context = eglCreateContext(display, config, EGL_NO_CONTEXT, context_attribs);
    EGLSurface surface = eglCreatePbufferSurface(display, config, surface_attribs);
    assert(context != EGL_NO_CONTEXT && surface != EGL_NO_SURFACE);
    assert(eglMakeCurrent(display, surface, surface, context));
    std::printf("headless renderer=%s\n", glGetString(GL_RENDERER));
    renderer.api = RendererAPI::OpenGL;
    LatteGPUState.glVendor = 0;

    const unsigned count = 65536;
    std::vector<Vertex> vertices(count);
    for (unsigned value = 0; value < count; ++value) {
        const unsigned wrong_lane = value ^ 0xA59E;
        vertices[value][4] = wrong_lane >> 8;
        vertices[value][5] = wrong_lane & 0xFF;
        vertices[value][6] = value >> 8;
        vertices[value][7] = value & 0xFF;
        vertices[value][8] = value & 0xFF;
        vertices[value][9] = value >> 8;
    }
    GLuint input = 0, vao = 0;
    glGenVertexArrays(1, &vao);
    glBindVertexArray(vao);
    glGenBuffers(1, &input);
    glBindBuffer(GL_ARRAY_BUFFER, input);
    glBufferData(GL_ARRAY_BUFFER, count * sizeof(Vertex), vertices.data(), GL_STATIC_DRAW);
    const auto scalar = draw(input, new_emit({FMT_16, 2, 0, Endian::SWAP_U16}), 1, 6, 'x', fp(1.0f), count);
    const auto swap = draw(input, new_emit({FMT_16, 2, 0, Endian::SWAP_U16}), 1, 8, 'x', fp(1.0f), count);
    const auto pair = draw(input, old_emit({FMT_16_16, 2, 0, Endian::SWAP_U16}), 2, 4, 'y', fp(1.0f), count);
    const auto lane = draw(input, old_emit({FMT_16_16, 2, 0, Endian::SWAP_U16}), 2, 4, 'x', fp(1.0f), count);
    const auto integer = draw(input, new_emit({FMT_16, 1, 0, Endian::SWAP_U16}), 1, 6, 'x', 1, count);
    const auto norm = draw(input, new_emit({FMT_16, 0, 0, Endian::SWAP_U16}), 1, 6, 'x', fp(1.0f), count);
    unsigned asymmetric_bytes = 0;
    for (unsigned value = 0; value < count; ++value) {
        assert(scalar[value] == (Capture{fp(static_cast<float>(value)), 0, 0, fp(1.0f)}));
        assert(scalar[value] == pair[value]);
        assert(integer[value] == (Capture{value, 0, 0, 1}));
        assert(norm[value][1] == 0 && norm[value][2] == 0 && norm[value][3] == fp(1.0f));
        assert(std::abs(std::bit_cast<float>(norm[value][0]) - value / 65535.0) <= 1.0e-7);
        assert(lane[value][0] == fp(static_cast<float>(value ^ 0xA59E)));
        assert(lane[value] != scalar[value]);
        const unsigned wrong_endian = ((value & 0xFF) << 8) | (value >> 8);
        assert(swap[value] == (Capture{fp(static_cast<float>(wrong_endian)), 0, 0, fp(1.0f)}));
        if (value != wrong_endian) {
            assert(scalar[value] != swap[value]);
            ++asymmetric_bytes;
        }
    }
    assert(asymmetric_bytes == 65280);
    std::printf("exact emitted GLSL values=%u scalar=pair, wrong-lane=%u, wrong-byte-order=%u"
                ", old integer and normalized preserve results\n", count, count, asymmetric_bytes);
    glDeleteBuffers(1, &input);
    glDeleteVertexArrays(1, &vao);
    assert(eglMakeCurrent(display, EGL_NO_SURFACE, EGL_NO_SURFACE, EGL_NO_CONTEXT));
    assert(eglDestroySurface(display, surface) && eglDestroyContext(display, context));
    assert(eglTerminate(display));
}

int main()
{
    check_admission();
    check_readback();
}
