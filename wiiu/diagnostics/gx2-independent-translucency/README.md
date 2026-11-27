# GX2 independently sorted alpha blends

Build this standalone fixture with the Cafe Rust and C toolchains used
for the game. Supply the exact assembled source with compact terrain
indices and per-view sort lanes.

    CAFE_RUSTC=/path/to/cafe/rustc CAFEGLSL_COMPILER_EXECUTABLE=/path/to/glslcompiler \
      ./build.sh /path/to/assembled/source /path/to/output

A single 16-vertex terrain section owns two alpha slabs and two compact
sorted-index lanes. The red slab is at +Z and the blue slab at −Z.
Both windings are present on each slab. Stock back-face culling selects
one surface from each opposite 90° perspective camera. The fixture uses
the production translucent shader, LEQUAL depth reads and writes, and
source-alpha color blending over opaque black. A separate four-vertex
opaque quad writes black and far depth over each full output in the
same queue before the alpha draw. The alpha pass reinstates stock
depth state. Successive cases test that the prior blend was cleared.

| Mode | Expected mismatch mask | TV | GamePad |
| --- | ---: | --- | --- |
| 0, independent | 0 | Red near, blue far | Blue near, red far |
| 1, cameras exchanged | 0 | Blue near, red far | Red near, blue far |
| 2, share TV lane | 2 | Red near, blue far | Blue only |
| 3, exchange lanes | 3 | Red only | Blue only |

The correct center and offset pixels have near channel 128, far channel
63 or 64, green 0 and alpha 255. These two far values cover the adjacent
UNORM8 quantizations of 128 × 127 / 255. An incorrect order draws the near
slab first. Its depth write rejects the far slab, leaving one color at
128. Each output also checks a black pixel beyond the projected slabs.

The allocation receipt checks one vertex fill, one GX2R vertex backing
and two equally sized index lanes. Raw payloads are 448 bytes for vertices
and 48 bytes for each 24-entry U16 lane. Actual aligned bytes come from
the WUT buffer estimate and the native creation result. The fixture
records the separate background allocation as well. The renderer
returns an error for an out-of-capacity lane.

Both background and alpha draws must submit through the paired WHB path. The link
wrapper checks that no explicit GX2DrawDone call occurs between draw
submissions and requires a later paired finish. One staged color copy
per output is checked after the paired finish. The fixture produces
pixel, allocation and command-order evidence. It records no frame rate.

In the native output directory run the analytic host oracle as follows.

    rustc --edition 2021 --crate-type lib -C panic=abort -C opt-level=2 \
      --emit obj=projection-host.o projection.rs
    cc -O2 -Wall -Wextra -Werror /path/to/fixture/oracle_test.c projection-host.o -lm -o oracle-test
    ./oracle-test

## Engine input and qualification

Start from the terrain-index engine tree
`b517a37e0175efca91ed3b2c6be2be4daefb8ae5`, assembled by applying the
compact index patch after RC 48. In a disposable engine checkout,
apply [the sort-lane overlay](sort-lanes.patch). The resulting engine
tree is `2eac181990cfcbefda334601f52ad2c44962f430`. The overlay leaves
one lane active for ordinary single-player terrain.

At 1920×1080 TV and 854×480 GamePad in Cemu 2.6 Vulkan, the final
fixture observed the following RGB samples. Center and offset samples
agreed in every case, with both guard pixels black.

| Case | Fault mask | TV | GamePad |
| --- | ---: | --- | --- |
| Independent orders | 0 | 128,0,64 | 64,0,128 |
| Exchanged cameras | 0 | 64,0,128 | 128,0,64 |
| GamePad uses TV order | 2 | 128,0,64 | 0,0,128 |
| Exchanged index lanes | 3 | 128,0,0 | 0,0,128 |
| Independent order repeated | 0 | 128,0,64 | 64,0,128 |

A single 448-byte vertex buffer and two aligned 64-byte compact index
lanes use 576 bytes. The fixture also uses a separate 128-byte opaque
quad to initialise color and depth independently before each blend.
Before that explicit draw was added, later cases composited over
previous GamePad colors. The same oracle passed after the change.

Neither output callback issued a whole-GPU wait. The paired finish
issued one, and both staged copies ran afterwards. The independent
views and blend fixtures use the same guarded teardown, which keeps
resources alive until an independently verified fresh submission
retires and stops the matrix on unexpected failure.

| Artifact | SHA-256 |
| --- | --- |
| RPX | `d286a75fbebd09ff04b7ce6c002cf4267af19c9a0713fc92517ce027d18dfc3c` |
| WUHB | `e986f1b8b1cc973f823d9583105c2c9002ac78f80b627e8e68aaa34a53407ddb` |

The fixture qualifies blend, depth, lane selection and the ordering
of native calls in Cemu. Physical cadence and local gameplay need
separate evidence.
