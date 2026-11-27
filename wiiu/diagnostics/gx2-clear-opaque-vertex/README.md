# GX2 clear opaque shader fixture

The fixture compares the normal opaque shader pair, the clear pixel shader
with the ordinary vertex shader, and the clear vertex and pixel pair. Each
draws independent TV and GamePad targets. The two outputs use different
projection rows. A four-tile RGBA atlas exercises depth, atlas coordinates,
and cutout alpha at 127 and 128. It runs with a flat and a varying lightmap
and compares all RGBA bytes on each output. The fixture uses synthetic assets.

Build against an assembled source tree that includes the clear vertex
variant, using the pinned Cafe tools.

```sh
bash wiiu/diagnostics/gx2-clear-opaque-vertex/build.sh "$assembled_source" "$scratch"
```

Use [the Cemu fixture runner](../../../tools/gx2_cemu.py) with
`clear_opaque_vertex.rpx` in a fresh lab and interpreter mode. Require
`GX2 clear vertex loader guard=PASS`, 12 distinct
`GX2 opaque screen ... result=PASS` cases, and
`GX2 clear vertex fixture result=PASS` followed by `coreinit.exit(0)`.
The ordinary pixel program is rejected by the opaque loader. A second
negative removes a vertex export and must be rejected before any draw.

The pinned compiler's ordinary, clear-pixel and clear-vertex shaders retain
the same loop descriptor for their respective stages. The paired loader
compares it with the stock shader. The clear vertex shader writes constant
values for the unused fog and cutout outputs to preserve semantic IDs.
Removing those declarations renumbers the surviving vertex outputs.

Readbacks check the shader and binding contract. The stock saved-view
workload separately measures native coverage, fixed-scale throughput and
actual shader adoption. [Terrain shader trials](../../../docs/gx2/terrain-format-trials.md)

The published fixture source built against the RC 58 engine and passed
all 12 checks in Cemu's interpreter. It compared 8,294,400 TV bytes and
1,639,680 GamePad bytes per case. The RPX SHA-256 is
`292409bac20e0d5d8c344a28590fb3f22aeee30e9f0e38eb4a6f4e767258e824`.
The accepted clear vertex GFD SHA-256 is
`2cf73355c2ca0b2890da4d739be6ac34b8123707b5e18953c2827a04e6d062fa`.
