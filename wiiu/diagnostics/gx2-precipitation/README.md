# GX2 precipitation fixture

Compile against an assembled source tree containing the precipitation patch.
Use the same WUT and CafeGLSL tools as the hosted build.

```sh
bash wiiu/diagnostics/gx2-precipitation/build.sh /path/to/assembled/source /tmp/gx2-precipitation
```

Run `precipitation_fixture.rpx` in a disposable Cemu profile with OSConsole
logging enabled. The fixture checks four pixels on each display covering solid
terrain occlusion, overlapping rain and snow with depth writes disabled,
alpha cutoff and sprite-local wrapping. Expect eight `pass=1` records and
`GX2 precipitation fixture passed`.

The shared `weather.rs` unit tests also replay the packed rain and snow phase
parameters against the stock CPU random reference. Those tests cover negative
and extreme ticks, wrap boundaries and partial-frame interpolation.
