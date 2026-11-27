# RC 62 signed terrain atlas cells and solid draw-list census

| Input or output | Identity |
| --- | --- |
| Port build source | `c0e4b10ee5761f5cc0945d623bebe22e69419fdf` |
| Engine tree | `b9c6e27b28de76d72aa4e38f5e01e4c6cc51c938` |
| Recipe SHA-256 | `2fa3e78e212889a8eded93d6d3b0178652d8becc386d864b6fdc4e2c65a9e10f` |
| Cafe Toolchain | `1.1.0` |
| RPX SHA-256 | `510db88bb5ad8bebdbab9511287576d62eeb5bb4f8eb976d1a8708d192267f94` |
| WUHB SHA-256 | `c5aaba5bd5485474f08c22bfb85862bf5479206cb968c96fd36fe9667f3b543d` |
| Estimated Aroma code-heap headroom | 12,443,776 bytes |

The pinned build ran with 32 Cargo and CMake jobs on Ben-Desktop at
`/var/tmp/mchorizon-rc62-native.gqa1o4po/out`. Both package hashes, 26
builder tests, strict native C in both renderer configurations, and the
ordinary-play admission controls passed.

## Signed atlas cells

The optional stream keeps the 28-byte vertex stride. It stores
stock-quantized local UV values and atlas-cell coordinates as signed
16-bit components. The stock shader's packed-float UV selection
introduces rounding at large layer numbers. The new upload reproduces
those values, including the wrap endpoint, across all 16,777,216
input and layer-byte combinations. The specialized vertex program
uses 146 dwords, down from 178, with six GPRs in both programs. The
original 142-dword pixel program and stock vertex program remain
byte-identical.

The reusable Cemu fixture matched every RGBA byte on two outputs
with independent projections, high layer bytes, repeated UVs, cutout
alpha and fog. Twelve output comparisons passed across two lightmaps,
including wrong-cell controls. The marked hosted RPX also loaded the
signed terrain program during ordinary gameplay. Native vertex time,
upload overhead and frame pacing still require same-build A/B.
[Signed atlas fixture](../../wiiu/diagnostics/gx2-signed-atlas-cells/README.md)

## Exact direct solid-draw census

The passive observer checks whether the complete direct solid plan
could reuse the previous same-view commands. It compares raw projection
bits, bindings, the ordered eligible sections and their mesh identities.
It emits one TV and one GamePad record every 120 accepted playing pairs.
The source does not record or replay a list. The separate 174-draw
fixture already proved the 32,736-byte replay and wrong-view controls.
[Display-list fixture](../../wiiu/diagnostics/gx2-solid-display-list/README.md)

Cemu emitted 18 matched windows. Of 2,153 eligible pairs per view,
1,804 would have reused a list. Eleven windows started at 100 or more
visible sections on both outputs. In those windows, 1,206 of 1,320
eligible pairs per view would have reused a list, about 91.4%.
The maximum reached 163 sections per view. Continuous camera movement,
the physical 174-section stock pose and actual command retirement still
need qualification before enabling a cache.

## Hosted lifecycle

Both marked arms ran the exact RC 62 RPX in multicore Cemu with stock
Full Performance, fixed 75%, DRS off, render distance 6 and simulation
distance 5. Each ran a 60-second dwell after reaching 100 sections on
both independently rendered displays.

| Cemu diagnostic | Signed atlas | Passive census |
| --- | ---: | ---: |
| Game frame ready, s | 53.737 | 53.594 |
| First 100 sections per output, s | 90.818 | 90.838 |
| Paired playing rows without screenshots | 2,232 | 2,179 |
| Same rows with no recorded omissions | 1,692 | 1,651 |
| Maximum sections per output | 163 | 163 |
| Structurally readable chunk streams | 378 | 376 |
| Audio entries written and read | 43,442 | 43,629 |

Both arms saved 8.623 blocks of verified movement, wrote six readable
region files, restored 234,880,920 free GX2 bulk bytes and exited
cleanly. The captures remain separate from physical pacing.

| Disposable run | Acceptance JSONL SHA-256 | Session HZP3 SHA-256 |
| --- | --- | --- |
| `r62-2621e3fb` | `b27284d43620db23967b04ac7002ca32c0576c459d56000a28ec899ad1f07740` | `39e7eeaf70703a650ac94ce7aeebfd1fb1f06ad8fde6217e22aa196cbe78c88d` |
| `r62-a3e13bc2` | `c3c88ef31bfbfac31a2cf5862d5481dfbe36f1364fa3bd6d6b74e8b603842184` | `d9293d72fa620b155a12846aa711b8516f6e7d3d64c8f226cdb26f7add1696df` |

The separate model-bake fix avoids creating a missing-material string
when a resolved particle sprite exists. The real-resource host fixture
reduced 179,241 allocation calls by 79,851 and preserved ordered
geometry and the error fallback. Native loading savings remain open.

## Supervised comparisons

Compare plain `4` against `4` plus `gx2-atlas-cells.enable` on the
same WUHB at the fixed 174-section pose, with repeated 60-second windows.
Check cutout, repeated UVs and atlas animation on both displays.
Run `gx2-solid-list-census.enable` separately at the fixed pose and along
a bounded traversal. Join the serial sequence ranges to the session
HZP3 records and retain view changes, mesh mutations and age by output.
[Physical test order](post-rc35-queue.md#physical-qualification-order)
