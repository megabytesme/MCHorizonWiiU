# GX2 stock sky fixture

The fixture compiles and links the stock sky shader and WUT draw path from an
assembled source checkout. Use the same pinned Cafe tools as the hosted build.

```sh
bash wiiu/diagnostics/gx2-stock-sky/build.sh /path/to/assembled/source /tmp/gx2-stock-sky
```

Run `stock_sky_fixture.rpx` in a disposable Cemu 2.6 profile with OSConsole
logging enabled. The fixture reads five pixels on each display covering the
top disc colour, spherical fog, the cylindrical fog threshold, the blended
sunrise fan and the dark disc's order. Expect ten `pass=1` records and
`GX2 stock sky fixture passed`. Host unit tests in
`gfx/stock_sky.rs` independently check the disc geometry and packed layer
ranges.
