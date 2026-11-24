# GX2 Overworld sun and moon fixture

Run `bash build.sh ASSEMBLED_SOURCE BUILD_DIRECTORY` with the pinned Cafe
toolchain. Pass the assembled source directory containing the native sky
bodies. The fixture uses the production sky and HUD atlas code with synthetic
sprite pixels.

Run the resulting `celestial_fixture.rpx` in the isolated Cemu interpreter.
Expect five `pass=1` pixel readbacks per display and the final
`GX2 sky bodies fixture passed` message. The samples verify additive
premultiplication, sprite transparency, rain attenuation, sunrise and lower
dark disc ordering, and output alpha.
