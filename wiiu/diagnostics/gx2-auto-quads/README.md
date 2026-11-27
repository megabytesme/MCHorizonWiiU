# GX2 automatic-index terrain quads

This standalone title compares the stock indexed triangle draws with
GX2's automatic-index quad primitive. Both modes draw the same resident
terrain vertices with the same stock shader, shader inputs, material,
winding, order and retirement. TV and GamePad use separate projections,
depth targets and explicit color readbacks. The test draws synthetic
geometry and opens no save.

The scene includes nonplanar and partially clipped quads, repeated
terrain UVs, varying light, the alpha 127/128 threshold and a trailing
depth witness. The first view pair crosses the 65,536-vertex batch
boundary with culling disabled. The second starts at that offset with
culling enabled. Every case draws an independently checked opaque
clear before the terrain.

| Arm | Expected result |
| --- | --- |
| 5 | Retain both cold stock readbacks before establishing a reference. |
| 0 | Capture separate indexed RGBA references for TV and GamePad. |
| 1 | Repeat the same indexed draw. Each RGBA byte matches its reference. |
| 2 | Draw automatic-index quads. Each RGBA byte matches its reference. |
| 3 | Change the triangle diagonal. Each image must differ. |
| 4 | Suppress terrain depth writes. The trailing depth witness must differ. |

## Build and verify

Use the pinned CafeGLSL and devkitPPC toolchains. The supplied `windows-recipe.json`
pins the exact terrain backend files from RC69 engine tree
`a569b1d884bbc6e68c4e8bb9eb4472eba88a6503`. The same input files are identical
in the staged RC70 tree `8fb25bfa7cf09cb45895fe56205e74a52b379312`.

```sh
bash wiiu/diagnostics/gx2-auto-quads/build.sh ASSEMBLED_SOURCE NEW_BUILD_DIRECTORY
```

The [Windows diagnostics builder](../WINDOWS.md) produces the RPX and packages
that exact RPX in a WUHB. From the port root with the four toolchain paths set:

```powershell
$port = (Get-Location).Path
$fixture = Join-Path $port 'wiiu/diagnostics/gx2-auto-quads'
python -B "$port/wiiu/diagnostics/build_windows_diagnostic.py" `
  --sdk-root $sdkRoot --sdk-receipt $sdkReceipt `
  --toolchain-spec "$port/catalog/toolchain-cafe-v1.1.0.json" `
  --cafeglsl $windowsCafeGlsl --recipe "$fixture/windows-recipe.json" `
  --output $newOutput --root fixture $fixture `
  --root engine "$assembledSource/wiiu/gx2_terrain" --root port $port
```

In a disposable Cemu lab or supervised console session, use the matching
standalone package and collect the complete guest log. Run
`python3 -B wiiu/diagnostics/gx2-auto-quads/verify.py SERIAL.log` on Cemu
logs. The fixture must report 22 successful output records followed by
`GX2 auto quad fixture result=PASS` and a normal guest exit. Arms 1 and 2
must match their respective reference digest. The cold control illustrates
the first-use state separately from the pixel comparison.

The Windows RPX SHA-256 is
`1c91ccf4bdb21dedec2c28a4facdbe439b0eae36025ce35a800f33d39c59516c`.
The WUHB SHA-256 is
`2ce25e47fbbda0b9495140c91e83374dd8cc37afac48c6cd6c7563f89249fe22`.
The Cemu interpreter passed all 22 records on both Vulkan and OpenGL.
OpenGL's cold TV digest differed from the warm indexed reference. The
warm indexed repeat and automatic quads matched each other exactly in
both scenes and on both displays. The Cemu receipt SHA-256 is
`8faa894967bbd5b85f1e80aa95ae3d970c208b50eeabad2824dd14ce50f0b407`.

## Logical workload accounting

| Workload | Quad draws per display | Logical indices omitted per pair | Bytes at 16 bits per index |
| --- | --- | ---: | ---: |
| First fixture scene | Two batches containing 16,392 quads total | 196,704 | 393,408 |
| Offset fixture scene | One batch containing 8 quads | 96 | 192 |
| RC38 fixed-75% physical reference | Solid subset of the measured terrain draws | At most 1,053,348 | At most 2,106,696 |

The first fixture scene uses 16,376 degenerate quads to reach the stock
batch boundary. These counts describe submitted indices and the capacity
case. The RC38 ceiling includes translucent terrain, which would retain
its sorted indices. The shared 196,608-byte quad index allocation is still
used by fallback and other native draws. The fixture measures no GX2
bandwidth, GPU time or frame rate. Native primitive assembly and a matched
physical throughput comparison remain open.
