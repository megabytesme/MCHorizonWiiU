# RC 68 terrain replay, centered lighting and CPU attribution

| Input or output | Identity |
| --- | --- |
| Port build source | `45a9080e98ca079eb4a6bf35bf6ad9bfffe94b0b` |
| Engine tree | `b24421b563b1195818039eb6c0f1c49ea3c4bf22` |
| Recipe SHA-256 | `050b24eb7ce454a271e2211b13d238e1df7a02f5490b282eb498ea4a15b3429b` |
| Cafe Toolchain | `1.1.0` |
| RPX SHA-256 | `032e2fa2a7e30a1bdb650a3b8e460556ebf08185a6311f33c666a0f93df9d323` |
| WUHB SHA-256 | `753bc30509049ab6e1b311fb6c7e8e3d09ba159dea1f0077f281fcbe098f8994` |
| Estimated Aroma code-heap headroom | 12,409,792 bytes |

The 32-job build on Ben-Desktop ran `python3 -B build.py build` in
`/var/tmp/mchorizon-rc68-renderer-native.s6t9oryy`. Cargo took 16 minutes
32 seconds and reused timestamps for 12,366 unchanged files. The
assembled source and output were under
`/dev/shm/mchorizon-rc68-renderer-native.pfjy2_60/out`. All 26 builder
tests passed. Both artifacts were verified again in their durable
output directory. The WUHB includes the exact RPX at offset 512.

## Display-list retirement and active coverage

The opt-in list recorder now accepts the proven contiguous 20-word
uniform layout and preserves dirty cache writes before checking its
post-record guards. The ordinary finish positively covers each queued
list before the resources it references can be retired.

The paired replay observer counts cold recording and warm replay on
each output. It buffers at most 64 windows of 120 complete consecutive
pairs. At the existing save checkpoint it reports the retained windows
with matching HZP3 sequence and clock edges. The normal timing window
therefore contains no serial write for these coverage reports.

The stock Cemu `r68-5bab2482` and replay `r68-68274a10` held separate
1440×809 and 641×360 world render targets at fixed 75%, render distance
6 and simulation distance 5. Each passed a 60-second dwell after
reaching 100 visible sections per display. The external TV and GamePad
scanout captures were byte-identical within each output across arms.
Both saved movement, played AX audio, recovered all 234,880,920 GX2 bulk
bytes and exited normally.

All 14 replay windows joined the retained capture with matching bounds
and both output dimensions. Both outputs used a warm list in 1,537 of
1,680 paired frames. In the steady 124-section cohort the ratio was
806 of 840. For 477 stock and 741 replay frames with the same 636 tested
sections per display, 262 section draws, 524,208 indices and 936 HUD
vertices, median solid CPU stages were 2.603/2.537 ms on TV/GamePad
stock and 0.027/0.032 ms with replay. Native benefit and foreground
qualification remain pending. The next candidate verifies stock-pair
retirement before the optional renderer can queue its first list.

    python3 -B tools/gx2_solid_replay_coverage.py CAPTURE.hzp SERIAL.log --sections 174

Matched Cemu replay coverage SHA-256
`df4f2dc238b3966e679c5a384da102eb024ef0f266674a2d5d3954e65fdb381a`.

## Centered light terrain shader

Plain profile `4` with `gx2-centered-light.enable` requests the exact
combined shader verified by the standalone [lighting fixture](../../wiiu/diagnostics/gx2-light-packing/README.md).
The terrain resource selects and owns the compatible native shader
and light encoding together. This preserves the stock fixed fog,
material and sorted translucency behavior. The stock shader recompiles
byte-identically. The combined program reduces vertex ALU groups from
18 to 17, smooth parameters from four to three, and pixel registers
from five to four while retaining the same texture lookups.

The Cemu trial `r68-78e97d42` logged successful activation and produced
byte-identical stationary scanouts to the stock run after its independent
fixed-75% gameplay dwell. Save, AX audio, normal exit, and full GX2 heap
recovery passed. Physical shader and frame-time measurements remain
queued and the stock settings retain the primary acceptance workload.

## Prepared pools and CPU profiling

The world generator now shares immutable raw template-pool sources with
the structure registry while keeping each decoded cache and generation
owner distinct. The exact stock fixture covered 245 source documents
and 751,724 input bytes. Per-owner acquisition changed from 702,381
requested bytes and 532 allocations to 72 bytes and one fixture
allocation. The host copy took 36.474 µs median, so this measures memory
relief. Loading-critical-path improvements remain tracked separately.

Cemu `r68-e670ea56` joined 24 client tick records by exact sequence,
clock frequency and start time. Each covered one advancing client
simulation step. Four early paired frames carried a renderer partial
flag. The reducer keeps the 20 omission-free frames. In the 13 records
from the exact 124-section cohort, `Game::tick` took 21.040 ms median
inside a 21.265 ms median
client tick. Each other named group took at most 0.072 ms median. This
makes the game tick the next function-level measurement in the hosted
workload. Its physical cost needs a separate run. The server pump
marker was recognized and generation 1 was admitted. No completed pump
sample was recorded. The GPU bulk heap and save checks passed.

    python3 -B tools/gx2_client_tick_laps.py CAPTURE.hzp SERIAL.log --sections 174

The centered-light and CPU profiling packages use the same staged native
WUHB and isolated marker sets. Test stock and centered light with the
same fixed-75% 174-section pose, stock passes and two independent
outputs. Run the CPU markers separately. [Physical order](post-rc35-queue.md)

Hosted qualification SHA-256
`1214c5105b97a73d3d40f4674d7b350c74efd326b9b7d6859dfd52b2cde437a0`.
