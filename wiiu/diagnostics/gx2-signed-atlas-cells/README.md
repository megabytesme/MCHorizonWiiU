# Stock-quantized atlas cells on GX2

This fixture draws the same 31×42 padded atlas through the existing
stock path and the signed 16-bit atlas-cell format. It uses the hosted C
shader loader and both vertex uploads. Each display has its own
projection, color, depth and readback. Each of two lightmaps runs stock,
signed and a wrong-cell control. The terrain uses high layer bytes,
repeated UV seams, cutout alpha and atmospheric fog.

The packed-float stock vertex shader lowers its UV selection to two
separately rounded multiply-add instructions. With large packed layer
components, those instructions reduce UV precision and can move the
result across the texture repeat threshold. The trial reproduces their
output with integer arithmetic during upload. The stock shader remains
byte-identical. This diagnostic measures the optimization at unchanged
visual behavior.

Build from an assembled engine that includes the signed atlas cell
trial, with the pinned Cafe Toolchain 1.1.0 and CafeGLSL compiler.

```sh
bash wiiu/diagnostics/gx2-signed-atlas-cells/build.sh ASSEMBLED_SOURCE NEW_BUILD_DIRECTORY
```

Run the RPX in a disposable Cemu interpreter lab. The shader loader
prints `GX2 atlas-cell quantized hosted loader=ready`. Each signed arm
must match all stock RGBA bytes at 1920×1080 and 854×480. The wrong
cell changes over half the pixels on each display. The title prints
`GX2 atlas cell fixture result=PASS` and exits with `coreinit.exit(0)`.

The reproducible RPX SHA-256 is
`84f58e72fb42b29d85999363e9fb139dc5c33aa88c2c2098fca8b1ac5ba01603`.
It passed all 12 cases. The stock and signed vertex programs use 178
and 146 dwords and the same six GPRs. Both use the original 142-dword
pixel program. Native performance requires a matched gameplay run.
