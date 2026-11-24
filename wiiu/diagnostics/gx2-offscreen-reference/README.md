# GX2 offscreen reference

This standalone diagnostic compares a known display target and an independent
color-only render-to-texture path. It uses original shaders and geometry, no
Horizon resources, and no CPU access to GPU-written images.

Build with the pinned WUT, devkitPPC and CafeGLSL. Set
`CAFEGLSL_COMPILER_EXECUTABLE` to the pinned compiler when it is outside PATH.

```bash
bash wiiu/diagnostics/gx2-offscreen-reference/build.sh "$scratch/reference"
```

The RPX runs in an isolated forced-interpreter Cemu lab. A physical session
uses the independent `GX2 Reference` WUHB. It presents five seven-second still
windows. The serial marker `GX2 offscreen reference still` follows the first
presentation of each trial. Collect HDMI stills and GamePad observations.
The pattern is red over blue with a magenta top-left corner and a yellow
bottom-right corner. A solid green frame identifies the offscreen clear.
Solid grey is the display clear. The final `SUBMITTED` marker confirms that
all presentation commands ran. Use the observed colors to judge rendering.

| Trial | TV | GamePad | Question |
| --- | --- | --- | --- |
| `direct` | Procedural pattern | Procedural pattern | Do the source shader and both displays work? |
| `whb-sample` | Sample the known GamePad color buffer | Procedural pattern | Do the sampler and texture alias work for a visible, WHB-owned producer? |
| `clear-only` | Green | Green | Can clients sample the manually allocated surface's clear? |
| `draw-no-clear` | Procedural pattern | Procedural pattern | Does drawing overwrite the preceding green clear through the dedicated context? |
| `clear-draw` | Procedural pattern | Procedural pattern | Does the dedicated context survive a separate color clear before drawing? |

The manual source is one aligned 320×192 RGBA8 surface with a color view and a
texture view. It has no depth or multisampling attachment. It uses a dedicated
shadowed context and a producer-side invalidate and fence before the sampler
runs. The WHB control samples the actual GamePad color surface while still
using its normal scan copy. No arm performs an arbitrary scan-size copy.
The fixture closes through ProcUI after requesting the Wii U Menu.

The `storage-v5` probe remains the first physical test. This reference
provides a comparison if Horizon's storage/context arms stay green. If
`whb-sample` passes and the manual `clear-only` also passes, a manual
draw that leaves green narrows the discrepancy to drawing to the independent
attachment or the visibility of those draw writes. If both manual draw arms
pass, add the failed production case's GX2R allocation, depth attachment,
combined clear, or context-sharing individually to find the necessary change.
Even a passing reference leaves production scaling on both displays to verify.

The implementation follows the public WUT/libwhb context setup and uses the
command ordering demonstrated by the native Deadline 2024 demo. RetroArch
independently uses manual MEM2 allocation and same-image color/texture views
in its blur passes. These sources motivated the comparison. The fixture has
its own code and shaders.

- [WUT 1.9.2 WHB surface and context handling](https://github.com/devkitPro/wut/blob/v1.9.2/libraries/libwhb/src/gfx.c#L93-L139)
- [We Are Outdated native render buffer, zlib licensed](https://github.com/vurpo/we-are-outdated/blob/b3ce39e07c9a6ee0f57f0f127031a044e06c27c7/renderer/RenderBuffer.cpp#L68-L160)
- [RetroArch GX2 blur passes, GPL-3.0 license](https://github.com/libretro/RetroArch/blob/c5c8407dbfc4b8e185b4e762148e1c4e16a0f8c4/gfx/drivers/gx2_gfx.c#L830-L906)
