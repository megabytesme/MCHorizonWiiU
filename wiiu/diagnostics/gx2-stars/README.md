# GX2 Overworld stars fixture

Run `bash build.sh ASSEMBLED_SOURCE BUILD_DIRECTORY` with the pinned Cafe
toolchain and a source tree containing the native stars pass.

Run the resulting `stars_fixture.rpx` in an isolated Cemu interpreter.
Expect four `pass=1` pixel readbacks per display and the final
`GX2 stars fixture passed` message. The fixture draws from one immutable
triangle buffer with different projection rows and brightness on each output.
It checks the additive SDR brightness curve, the order relative to the lower
dark sky disc, and that pixels outside the geometry stay unchanged.
