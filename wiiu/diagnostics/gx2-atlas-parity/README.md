# GX2 atlas shader parity

This standalone title compares the stock terrain pixel shader, the
packed-only shader with the stock clamp, and the gutter shader. It uses
the assembled RC68 GX2 terrain backend and the exact stock RC69 terrain
GFD. The atlas has the stock 2,046×2,772 dimensions and 64-pixel cells.
All texture data and geometry are synthetic. The title reads no world.

Each case creates a fresh atlas, draws an opaque magenta and black clear,
and verifies the background sentinel in the readback. The TV and GamePad
have different projections and separate render and readback receipts.
The title reads each complete RGBA surface after its ordered copy retires.
Both shader comparisons run with cutout and overlapping translucent
geometry, with repeated UVs, a 127/128 alpha boundary, and two fixed
texture phases. The second phase reverses candidate order after capturing
stock. A second stock draw exercises the same bindings as the candidates.
The wrong-sprite control verifies sensitivity to a changed texture address.

Build with the CafeGLSL and devkitPPC versions pinned by the project.

```sh
bash wiiu/diagnostics/gx2-atlas-parity/build.sh ASSEMBLED_RC68_SOURCE NEW_BUILD_DIRECTORY
```

Windows can build this title using the same assembled C backend and
generated GLSL sources. [Windows diagnostics builder](../WINDOWS.md)

Run `atlas-parity.rpx` in a disposable Cemu interpreter lab or in a
supervised console session. Require 40 distinct `GX2 atlas parity arm=`
records with `clear=1` and `result=PASS`, followed by
`GX2 atlas parity fixture result=PASS` and a normal guest exit. Arms 1–3
must have zero RGBA and alpha differences from arm 0 for every phase,
material and display. Arm 4 must change more than a tenth of each image.

The Windows standalone build produced RPX SHA-256
`ce2b681e212082217a89053734a0495ad530c0f7c8fa48f9268451fdb70a0893`.
Cemu Vulkan and OpenGL each passed all 40 records with that exact binary.
The software receipt has SHA-256
`6c071e65832a593c83cba1ea1697d28f6438b6d9a3abe9547fcc1f371f5341e7`.
This verifies the supplied cases. Native pixel parity in more general
geometry and timing on Wii U remain open.
[Terrain shader qualification](../../../docs/gx2/terrain-format-trials.md#stock-point-atlas-addressing)
