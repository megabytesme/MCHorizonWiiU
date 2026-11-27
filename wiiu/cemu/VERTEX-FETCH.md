# Cemu 2.6 unsigned scaled scalar U16 vertex fetch

The local upstream tag `v2.6` resolves to
`a6fb0a48eb437a8a41c13b782ac8ae0433bf8f98`, tree
`544b732829be0843e8c6a038da29f59aabe7103e`. The one-file patch outputs
tree `b91700c8eac82ef641cffef5a06817ae787e9edd`. The decoder has the same
starting blob in the pinned checkout and local upstream tag.

WUT 1.9.2 defines `TYPE_16 = 0x02`, `FLAG_SCALED = 0x800` and
`SWAP_8_IN_16 = 1`. Cemu's `GX2_Shader.cpp` maps this to
`FMT_16 = 0x05`, `NFA = 2`, unsigned and `SWAP_U16`. Default swap for
this vertex format also resolves to `SWAP_U16`. Vulkan
`VulkanPipelineCompiler.cpp` selects `VK_FORMAT_R16_UINT`, and
`OpenGLRendererCore.cpp` selects one `GL_UNSIGNED_SHORT` integer
component. Both feed the common shader decoder.

The new decoder branch reuses the scalar byte swap and zero-fill, then
converts the unsigned result to IEEE-754 single precision and stores
its bits. Every U16 value is exactly representable as float. The new
branch follows the existing unsigned scaled U16x2 operation. The
destination selectors are applied in `LatteDecompilerEmitGLSL.cpp`
after decode, including the float bit pattern for selector constant 1.
The patch is limited to unsigned scalar scaled U16 with 8-in-16 byte
swap. Existing scalar integer, normalized and all other descriptor
branches retain their original behavior.

Primary source references

- [WUT GX2 format and endian enums](https://raw.githubusercontent.com/devkitPro/wut/v1.9.2/include/gx2/enum.h)
- [Cemu 2.6 GX2 fetch construction](https://github.com/cemu-project/Cemu/blob/v2.6/src/Cafe/OS/libs/gx2/GX2_Shader.cpp)
- [Cemu 2.6 GLSL attribute decoder](https://github.com/cemu-project/Cemu/blob/v2.6/src/Cafe/HW/Latte/LegacyShaderDecompiler/LatteDecompilerEmitGLSLAttrDecoder.cpp)

## Host controls

Run `python3 wiiu/cemu/vertex-fetch-test/run.py /absolute/path/to/cemu/source` with
Python 3, a C++20 compiler, OpenGL and EGL development libraries and
headless Mesa software rendering. The harness reads exact pinned source
from Git and applies the patch in an automatically cleaned temporary
directory. It executes both full emitter bodies using a minimal adapter,
compares 16,384 descriptor/backend inputs and then compiles actual
emitted GLSL on llvmpipe. Transform feedback compares all 65,536 U16
values at 20-byte stride, including correct scalar and paired fetches,
a wrong-lane control, a swapped-byte readback and integer/normalized
legacy controls.

## Vulkan guest checks

Ben-Desktop compiled the exact decoder object with the prior executable's
CMake command and linked both private objects with its existing static
archives. Only the Cemu shader decoder object was added to the previous
CR-liveness-fixed link inputs. The build directory was
`E:/Temp/mchorizon-cemu-scalar-u16-20261127` and the resulting executable
SHA-256 was `e6a3a311fb94ec7282e7b42cd0d3552c2ef370bc33e0a2f9fd06d2aafe248cb5`.

The [native terrain fixture](../diagnostics/gx2-half20-light/README.md)
uses distinct attachments and projections for TV and GamePad. Fresh
Vulkan labs used the same scalar RPX in all three test arms.

| Cemu arm | Result | Receipt SHA-256 |
| --- | --- | --- |
| Prior decoder, interpreter | Scalar light changed 748,077 TV and 144,753 GamePad pixels on the varying map. Coverage and alpha matched. The fixture exited with status 1. | `c7982399150c103c114f04c2d142657c455aa7fcb4b31451af1e8b914bbabe82` |
| Updated decoder, interpreter | All 24 checks passed and exit status was 0. | `bf009a200704fc0bade9f83b21f0dd283cb43bd85cdd552c9028298a79fb15b0` |
| Updated decoder, multicore | All 24 checks passed and exit status was 0. | `a7ef7d87a7b18f40133ec1e1c70a6f797db56956104f57c3644c360a4ce383c1` |

The two updated reports have the same pixel, alpha and coverage counts
as the paired-fetch RPX. Wrong lane, byte order and clamp still changed
RGB on the varying map. This is a decoder correctness check. Physical
performance for both fetch descriptors remains pending.
