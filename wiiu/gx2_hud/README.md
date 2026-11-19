# GX2 HUD presenter

This standalone backend consumes Horizon `HudVertex` values, `HudBatch::runs()` ranges,
RGBA atlas pixels, font patches, the physical viewport, and `HudBatch::scale`. Its caller
must initialize GX2 before creating the presenter, call it on the render thread, set the
GX2 color target, and provide a GFD from `hud.vert`/`hud.frag`. Keep the GFD
bytes alive until the presenter is destroyed. Atlas input is copied; patches and frame
data are borrowed only for the duration of a draw. The draw changes GX2 render state.
The Rust bridge in `wiiu/platform/src/hud.rs` accepts Horizon's original `Blend`
and font `Patch` values. It maps supported blends explicitly because Rust's
`Blend` enum and its `Depth`/`CitGlint` payloads have no C ABI.

The Cafe client uses the presenter for loading, title and live game HUD.
It handles alpha, additive, invert, depth, vignette, End Portal, vanilla item
foil and CIT glint. A depth run requires a WHB output and runs after the
world passes. The game supplies the portal rectangles and time, the foil
texture and style, and the active resource pack's CIT slot data.

Resource-pack text shaders return `HORIZON_HUD_UNSUPPORTED` before GPU
submission. Invalid ranges, missing data, capacity overflow and
out-of-bounds patches return `HORIZON_HUD_INVALID`. C and Rust assert the
PowerPC bridge layout. See the [diagnostic](diagnostic/README.md) for
pixel checks and the [renderer checkpoint](../RENDERER_CHECKPOINT.md)
for live validation.

Build the PowerPC archive and GFD in WSL with devkitPro WUT and CafeGLSL on PATH:

```sh
CAFEGLSL_ROOT=/path/to/cafeglsl bash wiiu/gx2_hud/build.sh
```

The default output is `/tmp/horizonwiiu-gx2-hud`; set `HORIZON_WIIU_HUD_BUILD_DIR`
to choose another directory. Exercise the portable data validation separately:

```sh
cc -std=c11 -Wall -Wextra -Werror wiiu/gx2_hud/hud_data.c wiiu/gx2_hud/tests/hud_data_test.c -lm -o /tmp/horizonwiiu-hud-data-test
/tmp/horizonwiiu-hud-data-test
```
