# RC 55 worker timing, GX2 retirement and warm mob data

| Input or output | Identity |
| --- | --- |
| Port build source | `0b79ccb14272d3f9343aea4725900e73ee166a83` |
| Engine tree | `e79a57757c4520d5e1452e2528d4767d76b12cde` |
| Recipe SHA-256 | `8ebbaf85857d4e234406dffd5fd2f0e72de69a132fc609a6bda4ea336f480e5d` |
| Cafe Toolchain | `1.1.0` |
| RPX SHA-256 | `8097ca9385602633b0704744a2daff2bc3018b61bb8d6acf0fe69aecbcea0a2b` |
| WUHB SHA-256 | `71d3527ebc089eb88bf8fb6d5c061c0b8686ec876721ac681378c8bb53537db8` |
| Estimated Aroma code-heap headroom | 12,487,552 bytes |

The pinned native build completed on Ben-Desktop in
`/var/tmp/mchorizon-rc55-native.K8x7x0Id/out`. Output verification passed.

## Supervised worker and queue observations

Use plain `4` in `gx2-profile.enable`. Select one diagnostic at a time
and keep the primary pacing run separate.

| Workload | Additional markers |
| --- | --- |
| Grouped control | `gx2-hud-view-lanes.enable`, `gx2-grouped-present.enable` |
| Grouped queue observer | Grouped control markers and `gx2-grouped-retirement.enable` |
| Worker schedule | `gx2-worker-schedule.enable` |
| Warm stock mob data | `mob-data-warm.enable` |

The worker observer records bounded samples of streaming jobs, mesh jobs,
integrated ticks and save work. Each sample includes its wall time,
allowed core mask, endpoint cores and base priority. CPU time and complete
core residence are unavailable. Foreground loss disables new samples before
the release acknowledgement. Work crossing an untracked phase is excluded.
Exact marker, foreground and bounded host tests passed.

The grouped observer keeps the existing swap, flush, DrawDone and display
enable order and retains the existing lifecycle checks. It records up to
32 queue samples and emits them at the profile checkpoint after game drop.
These samples can establish whether the new queue token was still pending
after the flush. Individual display scanout and earlier resource reuse
require separate evidence. Host tests cover rejected pairs, inconsistent
tokens and failed DrawDone. Physical queue timing remains open.

    python tools/gx2_grouped_retirement.py CAPTURE SERIAL

For UWP, add `--serial-capture --serial-first-seq FIRST --serial-last-seq LAST`.
The join uses exact HZP3 sequence and start ticks and reports invalid and
unmatched observations.

## Stock mob preparation

The compact decoder matched 35,723 state rows, 161 placements and 74
animations. In host controls, peak requested allocation fell from 5.46 to
3.89 MB with the same retained bytes. Host timing showed no consistent
CPU improvement.

The optional Night seed reuses one stock mob base after save and worker
drain. It parks the base only after recovering exclusive ownership and
checks the selected export bytes and facts before adoption. Live pack and
mod overlays are prepared again. Host tests reused the same allocations
through 21 cycles, with 703,314 estimated retained bytes. Median host
preparation fell from 9.335 ms cold to 2.991 ms warm. Actual Cafe reopening,
whole-game memory and SD timing remain queued.
