# GX2 stock End sky fixture

Run `bash build.sh ASSEMBLED_SOURCE BUILD_DIRECTORY` with the pinned Cafe
toolchain and a source tree containing the GX2 End sky pass.

Run `end_sky_fixture.rpx` in the isolated Cemu interpreter. Expect six
`pass=1` readbacks per output and `GX2 End sky fixture passed`. A synthetic
atlas tests sprite-local repeat, edges next to other atlas cells, additive
tint, alpha coverage and updating atlas bounds without rebuilding geometry.
The GamePad arm mirrors the same buffer using its own projection rows.
The shared Rust geometry test compares the resident cube's outer corners
against all six faces of the desktop tiled mesh.
