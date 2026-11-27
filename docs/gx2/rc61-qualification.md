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
