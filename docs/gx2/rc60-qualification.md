# RC 60 grouped HUD retirement and TV depth

| Input or output | Identity |
| --- | --- |
| Port build source | `2082249113e46620d985826c58f3dfaaa64f42a5` |
| Engine tree | `a3635a8f561846bb82ed6891ab301c8b9f66bc97` |
| Recipe SHA-256 | `f65cad093e45c72ddeeca9746d6cdc14e360f9b26e1cdbdc864ce54349021554` |
| Cafe Toolchain | `1.1.0` |
| RPX SHA-256 | `45d53ed9c92e31e3b41c56e647e75e34c756661c56d4ce57ecb50db71011513c` |
| WUHB SHA-256 | `555a1f0bfb4dc61694fffd3e13c03f5baae470c0bc008307b975e1b081a193d9` |
| Estimated Aroma code-heap headroom | 12,457,920 bytes |

Ben-Desktop assembled, built and verified both outputs in
`/var/tmp/mchorizon-rc60-native.9g2dm8tv/out` using the hosted-nightly
builder. The first composition used a Rust 2024 let-chain in a Rust 2021
crate and failed to compile. The corrected feature patch replayed from
the same base and the assembly cache restored 12,375 unchanged mtimes.
All 26 builder tests and 52 GX2 host-tool tests passed.

## Full hosted lifecycle

The original RC 59 grouped control reached both independent outputs in
multicore Cemu. After Save & Quit, the native destructor reported its
arena still active and explicitly refused destruction, matching the
standalone reducer. Cemu's condition-register fix then let the unchanged
RC 59 RPX complete the same lifecycle. RC 60 separately fixes the game
wrapper and passed on the baseline emulator.
[Reducer and condition-register evidence](hud-retirement-reducer.md)

All three RC 60 arms used the exact RPX above, the same disposable
starting world and plain profile `4`. Full Performance, fixed 75%,
render 6 and simulation 5 remained in effect. Each reached distinct
1920×1080 TV and 854×480 GamePad captures, with 30 seconds of gameplay
between screenshot checks, and completed the ordinary save barrier.

| Lifecycle evidence | Grouped control | Grouped, early requested | Direct TV depth in MEM1 |
| --- | ---: | ---: | ---: |
| First Game frame, s | 67.660 | 67.468 | 70.280 |
| First paired terrain, s | 70.454 | 70.203 | 73.041 |
| Playing pairs in session HZP3 | 331 | 306 | 344 |
| Pairs without omissions | 106 | 88 | 129 |
| Maximum drawn sections per output | 117 | 104 | 112 |
| Backward movement in the durable save, blocks | 8.526 | 7.818 | 8.623 |
| Structurally readable region chunk streams | 371 | 380 | 371 |
| Delivered and drained audio queue entries | 23,991 | 24,112 | 24,353 |
| Rust live requested bytes after Game drop | 56,074,793 | 57,549,437 | 57,573,223 |
| Rust peak requested bytes | 346,285,535 | 347,367,984 | 345,564,038 |

Every arm produced six structurally readable region files, recovered
234,880,920 free GX2 bulk bytes with a largest block of 234,880,896
bytes, returned to Night and exited with `coreinit.exit(0)`.
Both grouped arms printed positive grouped retirement. The early arm
did not emit the positive submission-before-prior-scan-wait marker.
These runs qualify lifecycle on Cemu. Their camera and terrain counts
differ from the physical 174-section workload.

| Disposable run | Acceptance JSONL SHA-256 | Session HZP3 SHA-256 |
| --- | --- | --- |
| `r60-2fafe92f` | `e0ceb3c30c7ca5ea6d354c69cb5e221c9a3a9ccd3a77defdf93f6da8382b67e9` | `d1ed71376dac65bca17b7d0d47aedbf894884265601d61cf4341ec16a4b07bc6` |
| `r60-f8e6c0ba` | `81a43528a1b1aca6a20d5f05ef68c6b09ee1e431cb7a1495e54d5ce451562566` | `c10109eee3e754975fe6a2c9a0f9356962d890931bdd082c57bbac2bf1653b02` |
| `r60-21f73823` | `9036fffd2ec8d694a2795a988b648160cbc2eb764bb6644e7fc71f2138d5e9bb` | `c9b794f3f33924338e39568104faae282b9acef1f850a3533b0da0e60599feb6` |

## Supervised physical arms

Use the same verified RC 60 WUHB for each arm. Reference runs use two
independent native outputs, the saved 174-section view and repeat
windows of at least 60 seconds. Keep captures outside pacing windows.

| Arm | Markers in addition to plain profile `4` |
| --- | --- |
| Direct control | None |
| Bounded HUD | `gx2-hud-view-lanes.enable` |
| Grouped HUD | Bounded HUD plus `gx2-grouped-present.enable` |
| Grouped early | Grouped HUD plus `gx2-grouped-early-flush.enable` |
| Grouped retirement observer | Grouped early plus `gx2-grouped-retirement.enable` |
| TV depth | `gx2-mem1-tv-depth.enable` with the direct control |

The depth arm retains the ordinary MEM2 standby and permits a bounded
foreground MEM1 allocation only for fixed-75 TV D32. It validates
both display attachments and ownership, limits the additional depth
allocation to 5 MiB and keeps a 4 MiB free reserve. Cemu logged an active
1440×809 depth surface with 4,700,160 bytes and 2,048-byte alignment.
Record the positive activation on hardware before timing, and test
Home release and foreground reacquisition before extending the run.
Grouped queue observation and TV depth are separate conditions.
[Physical queue and reference](post-rc35-queue.md#physical-qualification-order)
