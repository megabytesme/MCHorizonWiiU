# GX2 stock near-fog fixture

The fixture draws the ordinary terrain shader, the near variant with the
ordinary shader restored for far terrain, and a negative that deliberately
uses the near shader for a far draw. All three retain the environmental
fog. Each arm draws and checks independent TV and GamePad outputs and runs
with flat and varying lightmaps.

The synthetic atlas checks cutout alpha 127, alpha 128, depth overlap,
and distinct near and far fog. The fixture compares every RGBA byte using
the [explicit readback stage](../gx2-explicit-readback), including a
retired copy before any CPU read.

Build from the port with an assembled engine source that includes the
near-fog trial and the pinned CafeGLSL and devkitPPC tools. Pass a fresh
scratch output directory.

```sh
bash wiiu/diagnostics/gx2-near-fog/build.sh "$assembled_source" "$scratch"
```

Use [the Cemu fixture runner](../../../tools/gx2_cemu.py) with
`near-fog.rpx` in a fresh interpreter lab. Require a passing shader
guard, all 12 distinct output/arm/lightmap rows, a passing fixture result,
and `coreinit.exit(0)`. The guarded arm must match the stock image byte for
byte. The forced-far negative must change the right side while leaving the
near side unchanged. The stock shader is also a negative loader input.

The RC60 interpreter run passed all 12 cases and exited normally.
The guarded arm matched all 8,294,400 TV and 1,639,680 GamePad bytes on both
lightmaps. The forced-far arm changed 1,036,800 TV pixels and 186,720 GamePad
pixels in each lightmap. The accepted near-fog GFD SHA-256 is
`1c909a456ad0fba1efa39ac17e12a86817bbd123018a6b14bab19c469c631704`.
The tested RPX SHA-256 is
`46408356134a5b802651b6341898e1a7ac2ed5945d7c923e6fa5a66d54a9bd9e`.

The fixture checks shader binding and pixels in Cemu. The stock saved-view
workload separately measures hosted shader use and physical frame cadence.
[Terrain shader trials](../../../docs/gx2/terrain-format-trials.md)
