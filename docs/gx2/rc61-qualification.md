# RC 61 GamePad overlap, stock fog and saved settings

| Input or output | Identity |
| --- | --- |
| Port build source | `e55f64fb238183a3fc501f3282dcb3b353a4621b` |
| Engine tree | `89af6abc06928903379aef057fd692fa1ec945ed` |
| Recipe SHA-256 | `20b5b0e12b35845a5ed33e7819668745b01a6720409f7b13d5971712570d6c41` |
| Cafe Toolchain | `1.1.0` |
| RPX SHA-256 | `e448a21739306d7070a4f0adf7889a97663cc4f10c92eb14f279dcbd37e9102d` |
| WUHB SHA-256 | `35799ea2e51768b89eb9efc9dc0a6d34ea086d430506046cdcb55b0e7fc25c65` |
| Estimated Aroma code-heap headroom | 12,450,432 bytes |

Ben-Desktop assembled and built both packages in
`/var/tmp/mchorizon-rc61-native.b9s6268h/out`. The package hashes, 26
builder tests, strict native C in both renderer configurations and the
two exact Rust 2021 GamePad entry-wait tests passed.

## Grouped GamePad terrain entry

The default-off arm retains the actual TV entry drain, a successful
native TV HUD lane, the exact earlier retirement token, both scan
copies and the final retirement. The GamePad can skip one terrain-entry
drain when camera, lightmap, resource and viewport identity match and
the native foreground checks hold. Per-output dynamic buffers remain
per output. With the same camera and section origin, the translucent
path reuses a verified sort key before considering an index write. A
direct HUD fallback still waits before it writes shared backing.

The exact multicore Cemu trial printed the positive no-entry-drain
receipt and completed a 30-second dual-output dwell, movement, save
and clean exit. Among complete screenshot-free rows with terrain
counts present in both the RC 60 grouped control and RC 61 trial and
at least 20 sections per output, median terrain-entry wait was
2.148 ms over 94 rows and 0.172 ms over 93 rows. At 70 or more sections
the medians were 0.654 and 0.151 ms over 46 and 39 rows. This is a
diagnostic comparison across Cemu package revisions, with a growing
terrain workload. Native pair cadence is open.

## Same-build GamePad overlap check

A follow-up ran the same RC 61 RPX twice, with only the GamePad
entry-wait marker changed. Both arms started their 60-second independent
dwell after reaching 100 visible sections per display and later reached
163. Both saved six readable region files and 376 readable chunk streams,
drained their audio queues, recovered the GX2 heap and exited cleanly.
The trial logged positive grouped retirement without the entry wait.

The comparison keeps complete screenshot-free pairs at 100 or more
sections per output and includes only matching section-count cohorts
from both runs. It retains 18 common section counts.

| Cemu diagnostic | Grouped control | GamePad overlap |
| --- | ---: | ---: |
| Matched complete pairs | 573 | 605 |
| Median terrain-entry wait, ms | 2.1569 | 0.1671 |
| Median final retirement, ms | 17.6032 | 17.5642 |
| Median flip wait, ms | 0.0019 | 0.0015 |

This isolates the wait change under the emulator. The physical queue
includes a separate retirement observer with the grouped and GamePad
markers. Use its HZP3 and same-run serial logs with
`tools/gx2_grouped_retirement.py` to identify pending queue work and the
native final-retirement cost. Keep it separate from normal pace windows.

| Disposable run | Acceptance JSONL SHA-256 | Session HZP3 SHA-256 |
| --- | --- | --- |
| `r61-f743f64b` | `9157d9385f30f2134efd3512afd08833aecb206ae643ae48882122787b8fa122` | `dcea5f66c4acd7c7233c5a54f1c0367eb637b3bf90e59659ef7fe9fe9d058a07` |
| `r61-cec2f65f` | `8f638d47f2aeaf5912536d2d86dd31db8db3bef7e878a360ef68e0c0dcbb863c` | `bd0a5439fbb142f1c0e6bf8ab56a72a9bdc7f0f1890804f90964062cfcc4c938` |

## Stock near fog

The optional pixel program keeps atmospheric fog and standard alpha
material behavior. It omits the render-distance ramp for an ordered
run only where each submitted section's bounds prove the ramp is zero.
It uses the same vertex program and texture fetch. The pinned machine
code shrank from 142 to 124 pixel dwords. The independent fixture
matched all TV and GamePad RGBA bytes with flat and varying light
and detected the forced-far control.
[Native fixture](../../wiiu/diagnostics/gx2-near-fog/README.md)

The hosted marked arm selected the shader on both displays during
ordinary stock Overworld play. Early samples peaked at 76,584 eligible
indices per output. In three matched settled samples with 124 sections
per output, the bounded run used 5,262 of 262,128 reported solid indices
on each display, about 2.0%. This supports testing actual admission on
hardware and gives the trial lower priority than removing a draw drain.

| Hosted Cemu evidence | GamePad overlap | Stock near fog |
| --- | ---: | ---: |
| Independent dwell, s | 30 | 60 |
| Game frame ready, s | 68.344 | 68.273 |
| Paired playing rows | 388 | 1,156 |
| Rows with no recorded omissions | 129 | 905 |
| Maximum sections per output | 116 | 163 |
| Backward movement saved, blocks | 8.160 | 8.623 |
| Structurally readable chunk streams | 379 | 377 |
| Audio entries written and read | 24,829 | 49,054 |

Both arms produced six readable region files, restored the bulk GX2
heap to 234,880,920 free bytes and exited normally. TV and GamePad
captures remained distinct. The original physical workload remains
the separate 174-section, fixed-75% reference.

| Disposable run | Acceptance JSONL SHA-256 | Session HZP3 SHA-256 |
| --- | --- | --- |
| `r61-f26f8630` | `cdbdc70723141e0a89c63395e93ecff3b2a1fb95e23a6d4e299659d86fc9c535` | `8d83550b09866f63cdae24ae05ff90c419b69402887378df0e6d7354140552b2` |
| `r61-a8a41de9` | `5bf3db2ba30c1d6d08fd9a6bc22777c2151296fbbe5fbb54b2b07c5e5ce9bfec` | `33d04b243adf0ff31c2c6b8c49c7feb5ef1f41dd55f40b1c159ecc2e04c4ee05` |

## Open saved generator settings once

World entry takes a fresh snapshot for that construction rather than
reopening the same level and sidecar seven times. The eight host
scenarios preserved normalized settings and dimension stems across
world roots, same-path replacement, legacy saves, flat and debug
generators, and missing or malformed input. The isolated host check
measured 109.723 to 16.717 ms on the mounted fixture and 1.457 to
0.209 ms on native WSL storage. The portable evidence is fourteen file
opens and decodes reduced to two. Native loading time is open.

## Same-process reopening and memory

The exact RC 61 RPX ran on the corrected multicore Cemu build with the
existing bounded mob-data warm marker enabled. In the two-entry movement
fixture, the first Game frame took 57.173 s and reopening took 55.761 s.
The first paired terrain took 60.021 s and 58.358 s. Reopening preserved
the position saved after moving 8.709 blocks. The warm mob bundle held
635,674 bytes. Its decode span fell from 0.664390 s to 0.218007 s.

A second run saved and reopened five times in one process. The warm
entries reached Game in 53.970–55.044 s and paired terrain in
56.362–57.645 s. Every save preserved the same fixed position and
restored 234,880,920 bulk GX2 bytes, with the same 234,880,896-byte
largest free block. Retained Rust memory was 58,198,457 bytes at the
second complete teardown and 58,232,469 bytes at the fifth, an increase
of 34,012 bytes. The largest sampled queued CPU terrain charge was
5,877,516 bytes against a 24 MiB cap. Both runs exited cleanly.

The default native heap was unavailable in this Cemu capture. Physical
loading, long traversal and the owner of the small retained Rust
increase remain open.

| Disposable run | Acceptance JSONL SHA-256 |
| --- | --- |
| `r61-warm-93fbf946` | `47c0e214f88305881b1c3668472c615d1093ac1d3330759ef48399d57e4c16d7` |
| `r61-memory-53fb83c4` | `8d2610d2ac47d3f51574c8e309cf65d340915cb92a4453f1fc612444d501551c` |

## Supervised comparisons

Use plain `4`, two independently drawn views, and the existing fixed-75%
reference with repeated windows of at least 60 seconds. Leave other trial
markers absent. On the same verified RC 61 package, compare grouped HUD
against grouped HUD plus `gx2-grouped-drc-no-entry-drain.enable`. Confirm
the positive retirement receipt outside timing and record direct HUD
fallbacks. In the separate fog comparison, keep the direct path and add
`gx2-near-fog.enable`. Confirm shader index coverage with the sparse
observer and report native pacing after the correctness checks.
[Physical test order](post-rc35-queue.md#physical-qualification-order)
