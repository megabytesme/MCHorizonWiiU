# Native terrain format and shader trials

These Cemu fixtures render separate TV and GamePad targets and compare the
full RGBA output. Their scenes exercise stock UVs, lighting, fog, depth or
translucency, depending on the experiment. Gameplay frame time and physical
parity require their own matched captures.

## Exact binary16 position fetch

The eligible format uses binary16 XYZ, the original 32-bit packed light,
two stock UV values and RGBA8 color. The stride falls from 28 to 24 bytes.
The fixture rejects coordinates that fail the exact binary16 check. The
stock and split FP32 arms isolate any shader or attribute rearrangement.

The initial half-position test drew zero terrain pixels. The pinned WHB
attribute helper assigned constant XYZ selectors to the half format. The
corrected fixture supplies the R, G, B, 1 selector before initializing the
fetch shader. The split FP32 and half-position arms then matched stock pixels
under the varying and flat lightmaps. Coverage remained 710,750 TV pixels
and 154,497 GamePad pixels in each arm. Changing the packed light differed
at 355,374 and 77,005 pixels with the varying map and matched with the flat
map.

Corrected RPX SHA-256
`3b53e17fb1cb33d9d8e492a4e5ca9cb40437f00ba8ef913b836aab5137c458b8`.
The production trial will measure exact per-layer eligibility and admission
cost. Water offsets, rotated models and other off-grid positions can require
the original float format.

## Exact binary16 with unsigned light

The 20-byte variant combines the existing exact half XYZ with an unsigned
light halfword. GX2 takes light from the G lane of an aligned two-halfword
fetch. Both half formats matched stock RGBA on independent TV and GamePad
attachments under the varying and flat lightmaps. Wrong lane, byte order
and clamp changed RGB under the varying lightmap while alpha and coverage
stayed identical. This saves eight logical bytes per eligible vertex
against stock. Aligned allocation and real geometry coverage still need
measurement. [Half20 fixture](../../wiiu/diagnostics/gx2-half20-light/README.md)

## RC 69 gameplay draw coverage

The exact RC 69 RPX completed four offline saved-world arms in Cemu.
The draw probe checked the complete GX2 vertex, fragment and fetch
program bytes and the live vertex stride when Vulkan recorded each
indexed draw. Stock 28-byte, opaque no-kill, half24 and half20 each
reached 8,192 matching draws on TV and 8,192 on GamePad. The visible
world viewports were 1440×809 and 641×360, with separate padded backing
extents of 1440×816 and 672×368. The probe verified both dimensions and
kept the viewport identity separate from the attachment padding.

Each arm reached 100 terrain sections per output before its 45-second
stationary dwell, then passed movement, save, audio, bulk heap recovery
and clean exit. Each display capture matched its stock counterpart byte
for byte. Separate uninstrumented runs measured 318,508 eligible compact
vertices at 651 resident sections. The 20-byte layout saved 1,274,032
logical payload bytes over the 24-byte layout.

These observations cover independent gameplay draw selection in Cemu.
The native sparse counter and physical timing remain pending. RC 69 RPX
SHA-256 `b330fe80f86c1b12fa79084ff66be7f8a2cf895bb092f1710e52121ed6250d78`.
Paired draw receipt SHA-256
`43b4a7dee1ad5347c3304a7c2d85dd2b113804c779b4eaec41f049df45240e5a`.

## Packed integers and material specialization

Explicit R, G, 0, 1 selectors corrected the UINT2 fetch setup. The 24-byte
packed-integer fixture matched all flat-light pixels and retained the same
terrain coverage under varying light. Varying light changed 699,503 TV
pixels and 152,045 GamePad pixels. RPX SHA-256
`b355ef20429d65106ae991af2cc8f16c70db89a0b9c9159dd41dc14aea8c61d1`.

For the packed-atlas fragment program, the generic specialization reduced
the compiled program from 142 to 130 dwords and removed four pre-texture
ALU operations. The fixed solid and translucent variants use 110 and 108
dwords and move fog calculations earlier. All retained the same stock
vertex code and texture lookup count. Solid output matched in the initial
fixture. In the translucent pass, both trial shaders matched the TV and
differed at 85,508 GamePad pixels. A repeat with explicit per-output color
and depth clears returned the same difference count. Initial RPX SHA-256
`4d755971603b216c6a57c0630c2c8f6e18fd0e237a9e92f58b8353ff08690276`.
Explicit-clear RPX SHA-256
`8f9f23cc1c4003882f6ca2bafe036d81ae9b78eb19e423f8119ce3a08f95d20f`.

## Reversed water quad reuse

When consecutive water quads have byte-identical vertices in reversed
winding, the trial keeps one copy of the vertices and both original corner
orders. It preserves stock shaders, logical indices and sort order. In the
fixture, native backing fell from 8,448 to 4,800 bytes. All 12 positive
cases matched and the three injected violations changed the expected
outputs. RPX SHA-256
`3387be4c9240579657e0bf13938fb890e4f25b48b2f69ffa996e8e71a5a1a3f4`.
The default-off gameplay bridge budgets the remap alongside native storage
and falls back when a section would save no bytes. Native gameplay and
physical timing remain open.

## Tiled atlas storage

The stock-sized 1D thin-tiled atlas passed exact dual-output RGBA,
padding and animation checks in Cemu. A same-size host fixture streamed
all 1,301 sprite layers through one 17,424-byte cell buffer. The layout
adds 32,768 bytes to the existing single linear allocation. Four
animated cells used 36 dirty spans instead of 264, with more bytes
flushed and more CPU conversion work in the host control. Physical
sampling time and safe gameplay mutation still govern adoption.
[Tiled atlas fixture](../../wiiu/diagnostics/gx2-tiled-terrain-atlas/README.md)

## Greedy meshing eligibility

The current packed terrain UVs encode one repeat below 2.0. Greedy runs
spanning more cells need a repeat representation that preserves atlas seams.
The native shaders interpolate fog distances from each original corner.
A host control in the transition region changed the fog factor from zero
to 0.758 when corners were removed. A separate control used the actual
translucent sorter. Combining water cells moved an overlapping glass draw
ahead of the water and changed the controlled red channel from 0.551 to 0.271.

A conservative host predicate found clear-fog opaque candidates while
preserving actual local geometry bounds. Across the synthetic six-chunk
proxy, eligibility removed 13% of quads with an environmental fog start of
64 blocks and 27% with a start of 96. Those figures describe the proxy.
A sparse source-only diagnostic now prepares bounds from uploaded sections
and can report eligible opaque geometry for each output. Use the stock
saved-view captures to measure coverage before selecting a merge format.

## Clear opaque vertex program

The first paired runtime fixture stopped in the previously introduced
clear pixel loader. Both compiled stages carry a loop descriptor. The
corrected guard matches its count and contents against the corresponding
stock shader. The other reflection and depth checks still apply.

The opt-in vertex program retains the exact pixel binary and the vertex
export IDs. It removes the fog distance work and the section-origin
uniform write inside the existing bounded clear-opaque run. The pinned
compiler reduced the program from 178 to 146 dwords, six to five general
purpose registers and two square roots to zero. The atlas and lightmap
samples and clip rows remain. Directly deleting the unused declarations
moves the atlas varying to another export ID and is rejected.

A standalone native RPX ran in the Cemu interpreter and passed 12
full-output checks across both displays, three shader arms and two lightmaps.
This included 8,294,400 TV bytes and 1,639,680 GamePad bytes per case,
known RGBA samples and the incompatible export control. Its SHA-256 is
`367dddb2621d4b154a487a5146004bfce1c788a4dee9a03d37efd6a4b37d1d8e`.
The [reproduction fixture](../../wiiu/diagnostics/gx2-clear-opaque-vertex/README.md)
builds the same shader binaries.

Stock Overworld atmospheric fog starts at zero. The clear path
requires both starts above one block and admits no sections under
that configuration. Its synthetic fixture exercises the shader and
loader. The primary physical reference retains stock fog.

## Fog reciprocal relocation

The native paired bit probe found exact spans and products for all 40
ranges on both Cemu outputs. CPU-prepared reciprocals matched the GPU
bits for 31 ranges. The other nine differed by one ULP and include the
stock render-distance-6 environment ramp. The guarded render-distance-10
control matched every RGBA pixel. An intentionally wrong reciprocal
changed 2,073,600 TV and 373,440 GamePad pixels. Render distance 6,
render distance 19 and the degenerate range retained their stock path.
Fixture RPX SHA-256
`aaa06813dd9e51ddf96539d99192f3dbae3cb89c733a7daa5c3529d01981266c`.

The alternative flat vertex export reduced the fragment program from
142 to 134 dwords and grew the vertex program from 178 to 186. Both
stages gained one GPR. The physical saved-view median of 1,053,348
indices per pair corresponds to 702,232 submitted quad vertices if
each quad processes its four vertices once. A single layer of the two
75% render targets contains 1,395,720 pixels. This leaves little margin in a simple
ALU issue-group estimate. Actual shader invocations and timings remain
unknown. Keep the stock fragment reciprocal in gameplay.

## Stock solid depth scheduling

A single GFD descriptor byte changes the stock solid shader's fallback
depth-test order. The vertex and fragment programs, sampler layout and
shader alpha discard stay identical. Cemu passed all 28 visual checks
for independent TV and GamePad views across three overdraw levels and
two draw orders. The fixture's separate bottom-cycle comparison and
physical depth/alpha parity remain to be measured. [Same-RPX Re-Z fixture](../../wiiu/diagnostics/gx2-rez/README.md)

## Stock point atlas addressing

The Windows CafeGLSL release reproduced the stock RC 69 terrain GFD
embedded in the native title byte for byte. Its SHA-256 is
`c07d3b7557e55341a3bfcd3e36cf2dbd048f8473514a8ac26d400878503f7133`.
The alternatives preserve the complete 178-dword vertex stage and the
pixel metadata except program length. Each uses one texture instruction,
four pixel inputs and five pixel registers. [CafeGLSL compiler release](https://github.com/Exzap/CafeGLSL/releases/tag/v1.0.0-rc1)

| Pixel address path | Pixel program dwords | Issue groups before texture | ALU lanes before texture |
| --- | ---: | ---: | ---: |
| Stock | 142 | 8 | 19 |
| Packed atlas only, retain clamp | 130 | 7 | 15 |
| Use replicated gutter in place of clamp | 126 | 5 | 12 |
| Packed atlas only with replicated gutter | 118 | 4 | 8 |

The gutter trial keeps stock alpha, fog and blending. The compiler combines
the local coordinate multiply and atlas origin add. Multiplication by the
stock 64-pixel side is exact in the bounded binary32 domain. A host check
matched all 64,875 split and combined sums at side 64. At side 63 it found
1,314 different sums. The gutter also requires coherent sprite layers on
each quad, bounded interpolation and current RGBA edges after animation.
Keep coverage of those conditions in the native pixel control.

The packed-only clamped GFD is byte-identical to the earlier generic
packed variant that changed 85,508 GamePad translucent pixels, including
31,316 alpha values, in each of two standalone Cemu runs. The opaque
results matched. That variant still needs a same-package stock-to-stock
control that isolates the GamePad blend state before any requalification.

An isolated Cemu copy of RC 69 replaced only the stock terrain GFD with
the 126-dword gutter trial. It loaded the original saved view at fixed
75%, reached 100 sections per display and saved the expected movement.
The Cemu shader cache retained the exact compiled trial program. The
first terrain captures matched the stock counterparts byte for byte on
both displays. After the 45-second dwell, the trial showed a rabbit that
was absent from the control. The pixel changes were confined to that
region on both outputs. Save, normal exit and full GX2 heap recovery
passed. This comparison leaves native point precision and physical
frame time open. The receipt SHA-256 is
`e08da8bb6c36568620c89788063009adc838b023a5b1285bcfc0ebb4c9d85f7c`.

The authored boat scene exercised translucent water in separate matched
stock and gutter runs. Both outputs drew terrain and completed the save,
heap recovery and title exit. With ordinary animation active, the settled
captures differed at 571,662 TV and 125,807 GamePad pixels. The two runs
showed different water animation phases and different live entity poses.
A separate diagnostic froze texture animation in both arms. Its settled
differences fell to 54,775 TV and 11,422 GamePad pixels, and the boat and
fish still moved independently. Initial residency also differed. These
runs leave translucent pixel parity unresolved. Use a fixed animation
phase, fixed geometry and explicit per-output draw clear in the next
standalone comparison. Compare complete RGBA and depth surfaces with
stock-to-stock and reversed-order controls. The frozen diagnostic receipt
has SHA-256 `b489c8f6d37117475ab80de40aa160dadd2d8b0dea313109015ee7cffa34bc19`.

The same stock scene allocates 22,708,224 bytes for 1,301 padded 64-pixel
terrain layers. The retained decoded inputs contain 1,263 16-pixel
entries, 47 32-pixel entries and one 64-pixel entry. A mixed-size host
layout for all 1,311 inputs estimates 1,980,416 bytes. Direct remapping
passed a different texel to alpha discard in the host model. Preserving
the original selected address and recentering the compact sample passed
1,473,480 modeled comparisons. The address-coded native fixture's shader
compiled, with actual paired sampling and throughput testing pending.
