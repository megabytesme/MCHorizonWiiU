# RC 66 world preparation and solid terrain display lists

| Input or output | Identity |
| --- | --- |
| Port build source | `6576e4b02442e45dddd572d4b227c6ede719067c` |
| Engine tree | `393c268a6cf0af5b4bcc2199c0a8482b4d454d13` |
| Recipe SHA-256 | `2af688929d1673be46e57aa6a4a36092c413c5af18ff030031ad21c34ee18e5e` |
| Cafe Toolchain | `1.1.0` |
| RPX SHA-256 | `4a6ea3453fb6b71690aba15ad5ffa5bc392e40f6028557e1b5ccac9396d2116c` |
| WUHB SHA-256 | `57b2f019037fefa5b046752371236d5ff5a6af486f233cad6699851afa5a0cec` |
| Estimated Aroma code-heap headroom | 12,411,008 bytes |

The 32-job native build used `/dev/shm/mchorizon-rc66-native.9xn76ax9/out`
on Ben-Desktop. Cargo took 12 minutes 52 seconds and reused timestamps
for 12,371 unchanged files. All 26 builder tests passed. Both artifacts
match the build receipt. The RPX appears once at offset 512 in the WUHB.

## World preparation

The prepared-registry path memoizes classification of live placed
features within the existing ordered pass. The focused host fixture
retained all 46 stock decisions and reduced the isolated pass from 31.119
to 5.686 ms, with about 10.7 KiB of temporary map storage. Worldgen also
shares immutable feature bindings when the two stages use the same
`GeneratorData`. The concurrent host fixture reduced peak binding
allocation from 6.76 to 3.38 MB. Wall time remained about 99 ms across
the isolated two-stage host trial.

The matched cold Cemu probe `r66-41706a02` reached first Game in
51.340 s, paired three-section terrain in 53.918 s, generator data in
10.916 s and streaming assembly in 4.805 s. RC 65 registered 51.538,
54.212, 10.925 and 4.823 s in the corresponding run. One measurement
per candidate leaves ordinary timing variation unresolved. The tracked
Rust peak was 349,390,583 bytes, and save restored 234,880,920 free GX2
bulk bytes with a 234,880,896-byte largest block.

## Independently rendered terrain lists

The trial uses a distinct bounded arena and an exact view key for each
display. A cached list preserves the stock solid submission order and
uses the same shaders and resident meshes. Fog, atlas and target state
are established per output before replay. Camera, target and mesh
changes invalidate the affected eligibility. The two lists retire
through the existing ordinary finish and require a positive timestamp
before their storage can be reused.

The same RPX ran stock `r66-06691b01` and opt-in `r66-206f9d14` on the
[CR-liveness-fixed emulator](../../wiiu/cemu/RECOMPILER.md#condition-register-liveness). Both used Full
Performance, render 6, simulation 5, fixed 75%, DRS off and independent
1440×809 and 641×360 world targets. Each held gameplay for 60 seconds
after both outputs reached 100 sections and later reached 163. The
native log confirmed a cold list and warm replay on each output. Saved
movement, AX playback, bulk heap recovery and ordinary exit passed.

External Cemu captures after the dwell were byte-identical within each
output across both arms. The TV and GamePad captures have independent
1920×1080 and 854×480 dimensions. The run logs confirm that warm replay
occurred. They do not identify the exact frame associated with the
external scanout captures. A separate controlled list fixture established
per-frame parity and exercised wrong-state negative controls.

Across five exact work cohorts, median timed solid processing changed
from 1.999–2.455 ms to 0.027–0.0274 ms on TV and from 2.079–2.440 ms to
0.031–0.0313 ms on GamePad. The cohorts agree on tested and drawn
sections, total indices and draws, HUD vertices and target sizes. These
are Cemu CPU-side stage durations. Native cadence, active list coverage,
moving-view benefit and GPU costs still need measurement.

Replay is opt-in through `gx2-solid-list-replay.enable` with plain `4`
and supported fixed-75 settings. The compact first physical sweep uses
the separately staged grouped HUD and GamePad overlap arms. Queue a
physical replay arm after the updated recorder guard is qualified on the
native toolchain and through the hosted lifecycle. Home and foreground
return need explicit supervised checks. Treat any unresolved retirement
as a failed trial and preserve ownership until it can be proven safe.

Hosted receipt SHA-256 `8a6e058fe20c8d7d1641e529a8ec1dd1e0b767059d2153695d22b92eed40bfd6`.
Scanout comparison receipt SHA-256 `ae08a4d461aedd16fa7fa7a66652228eb7a703de208c9547c2c52c80877d1b0f`.
