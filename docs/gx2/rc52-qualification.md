# RC 52 prepared resources and compact terrain

RC 52 reduces stock preparation work and adds two opt-in native terrain
formats. Automatic DRS retries a failed surface allocation when the presenter
has retired the previous pair. Fixed-resolution runs keep the selected scale.
Startup failure telemetry uses a fixed set of stage and reason labels.

| Input or output | Identity |
| --- | --- |
| Port build source | `b2c58af8eca2970f32d41bbf01b6106b46340a29` |
| Engine tree | `e4135f0f77fa6abf26d2bcbe86b135bda3f95fa5` |
| Recipe SHA-256 | `d02322905d17fb8217604432bcbce30c7dccf20b6a7ff13a2cadf4bebb0f84db` |
| Cafe Toolchain | `1.1.0` |
| RPX SHA-256 | `69c282f1e82df9d07139d25d712315856c88baf101fbdfdd444e757007efbbc5` |
| WUHB SHA-256 | `3d2c54081d50c2fc5fe1b6dcc75721bd2cb7e5511ad76a04789462a9f408d796` |
| Estimated Aroma code-heap headroom | 12,589,888 bytes |

Ben-Desktop built the exact recipe in
`/var/tmp/mchorizon-rc52-native-performance.e7M0SBq1/out` and verified both
package hashes and the code-heap estimate with
`python3 -B wiiu/builder/build.py verify-output BUILD_RECORD`.

## Host controls

- Model-state matching borrows ordered properties instead of allocating a
  map for every state. It preserved 35,723 bindings and 7,880 alternatives.
  Host median preparation fell from 370 to 250 ms and allocator requests
  from 665,433 to 275,397.
- Lighting reuses the visibility faces already computed during startup.
  All 35,723 states kept the same face masks, including fallback shapes.
  Host median fell from 454 to 34.7 ms and measured peak from roughly 13
  to 1 MB.
- Typed prepared biome quantization preserves both tested vanilla dimensions,
  including 16,384 lookups in each. Host median fell from 68 to 37 ms and
  measured peak live allocation from 26.38 to 17.35 MB.
- The geometry control compared stock, half-position solids, exact reverse
  translucent quads, and both together. Decoded vertices and the 192
  ordered translucent indices matched across two eyes. Accounted backing
  was 6,144, 5,888, 4,608 and 4,352 bytes respectively. Allocation-failure
  controls returned to the original path without leaked buffers.
- Twelve DRS host tests cover upward and downward change, fixed resolution
  and recovery after a transient allocation failure. The synthetic 60 FPS
  target changed 75→85→95→100→90→80→75 with one owned surface pair.

These loading figures are timings on the host. Measure startup phases on
Wii U with matching archive and cache state.

## Offline Cemu multicore recompiler

Corrected Cemu `8cf988a9ce8b2fb9f3a42eea04b4eef911e80f6cc38749da93e2d3e2c86383dd`
reported `CPU-Mode: 3 (gameprofile)`. The exact RC 52 RPX reached gameplay in
50.211 s and paired terrain in 52.851 s on a disposable fixed-75% world with
render distance 6 and simulation distance 5. The 1920×1080 TV and 854×480
GamePad captures were distinct before and after a 60 s dwell. Movement of
8.623 blocks persisted to the world save. Audio drained all 31,956 items.
Peak tracked Rust allocations were 358,133,061 bytes. Save and clean exit
restored 234,880,920 free bytes in the GX2 bulk heap with a largest block of
234,880,896 bytes.

The session HZP3 SHA-256 is
`0f54a4c3c651923ab718306f503d48220e6c9f2f51016337d8297a7335511f70`.
It records 1,236 paired gameplay rows, 783 with no reported partial passes,
and up to 163 sections per output.

## Recompiler failure isolation

One exact single-core run reached gameplay at 117.373 s and paired terrain
at 129.471 s, then rendered both outputs through the dwell. The replay
timed out opening Pause. Captures and serial playback still showed active
gameplay with loops taking about 0.8 s. The harness now holds single-core
inputs across that frame interval.

A fresh single-core start and a multicore start with both geometry markers
failed during core resource work before the host Game handoff. The same
failure-only trace appeared in both runs. Farmland state 6875 supplied a
zero-length borrowed base name. Both the registry lookup and bounded linear
check found the literal `minecraft:dirt` at state 10. The data identifies
an invalid lookup input. The origin of the invalid input still needs a
matched trace before and after loading the landing definitions.

Prior RC 50 runs ended in resource preparation in both modes. Run existing
RC 50, RC 51 and RC 52 packages from one hash-locked disposable fixture to
separate differences in the inputs from a layout-sensitive recompiler
failure. Reliable repeated starts, single-core save and same-process
reopening remain open.

## Supervised native comparison

Use the [fixed-75% reference](benchmarks.md#physical-fixed-75-saved-view)
with plain `4` in `gx2-profile.enable`. Repeat the same input and settings
with each geometry marker by itself and both together.

| Arm | Additional marker |
| --- | --- |
| Stock | None |
| Exact half-position solids | `gx2-half-position.enable` |
| Exact reversed translucent quads | `gx2-reverse-translucent.enable` |
| Both formats | Both above |

Record eligible sections, resident and upload bytes, cadence and GPU pass
timing on both outputs. Keep the stock artwork and render passes active.
Use a separate automatic-DRS workload for upward and downward scale changes.
[Terrain format controls](terrain-format-trials.md)
