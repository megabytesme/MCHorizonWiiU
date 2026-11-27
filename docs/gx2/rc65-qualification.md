# RC 65 prepared state rows and paired publication timing

| Input or output | Identity |
| --- | --- |
| Port build source | `ff49b34f51825935da8d6dac508349193c9f2f85` |
| Engine tree | `42164dee2b839c0cf7a741590737d573f6ee4029` |
| Recipe SHA-256 | `081155a32de712d37c6b8765d058062c0ce60431e8d9f8dfd45cb24f7a9c216e` |
| Cafe Toolchain | `1.1.0` |
| RPX SHA-256 | `631818e62cc4a1c576b47c1861ed759e9fe904ff33134d2ee1437df9fbe19375` |
| WUHB SHA-256 | `9289113bf374e8e422ed54bbbc1efbd6d4a4b8a00509664581519b5d006e5015` |
| Estimated Aroma code-heap headroom | 12,421,760 bytes |

The 32-job build used `/dev/shm/mchorizon-rc65-recovery.s7xptjov/out`
on Ben-Desktop after the first assembly encountered a filesystem write
error. The cache handoff restored timestamps for 12,378 unchanged files.
Cargo took 15 minutes 31 seconds. All 26 builder tests passed, and both
package hashes verified in the scratch and durable directories.

## Prepared registry rows

The validated prepared-table path compares common ASCII state rows
directly against the live registry. It charges the original byte, node
and depth budgets. Other encodings, ordering differences outside the
direct shape and malformed rows use the existing NBT reader. The stock
host controls reproduced all 35,723 state identities across all six
tested tables with identical decoded results and consumed lengths.

The paired cold-start probes used the same disposable Reopen Fixture,
verified prepared archive, fixed 75%, Full Performance, render distance
6 and simulation distance 5. The Cemu stage marker and emulator were
identical. Both reached the small paired terrain gate, saved movement,
returned the GX2 bulk heap to 234,880,920 free bytes and exited cleanly.

| Multicore Cemu stage | RC 64 `r64-b053b8d4` | RC 65 `r65-783a4903` |
| --- | ---: | ---: |
| First Game frame | 54.451 s | 51.538 s |
| First small paired terrain gate | 57.191 s | 54.212 s |
| Core collision | 2.672 s | 1.302 s |
| Core motion | 3.030 s | 2.479 s |
| Worldgen collision | 2.948 s | 1.514 s |
| Worldgen support | 7.139 s | 5.679 s |
| Worldgen properties | 7.294 s | 5.411 s |
| Worldgen data | 13.428 s | 10.925 s |
| World streaming assembly | 4.934 s | 4.823 s |
| Peak tracked Rust bytes | 343,418,936 | 349,677,509 |

Stage durations overlap. This single matched run records a 2.913-second
Cemu first-frame reduction. The physical cold and warm loading criteria
still need timed qualification.

The stock saved-world run `r65-dde769fb` held independent TV and GamePad
gameplay for 60 seconds after each display reached 100 visible sections.
It later reached 163 sections and recorded 2,092 screenshot-free fixed
75% pairs at 1440×809 and 641×360 world sizes. Save, AX PCM, full GX2 bulk
recovery and ordinary exit passed. A native build was active during this
lifecycle run. Use it for lifecycle validation.

## Steady-state terrain maintenance

Empty terrain-column eviction calls now bypass resident mesh, deferred
mesh and block-entity scans. The existing retired-batch threshold is
still checked. Six host controls cover shared snapshots, an empty input,
the retirement limit and unchanged nonempty ordering.

## Publication timing

The optional plain-`4` `gx2-present-deadlines.enable` probe samples
the existing submit and retirement paths without adding a GX2 command
or wait. It emits a bounded serial sidecar keyed to HZP3 sequences and
frame-start ticks. Use a supervised physical run to distinguish a refresh
with an empty swap queue, GPU work outstanding at publication and an
already retired pair waiting for its flip.
[Physical queue and reducer](post-rc35-queue.md#paired-publication-timing)

The live deadline run `r65-19006d5c` passed the same 60-second lifecycle,
and all 32 serial records joined exact HZP3 rows. The 160 observations
covered 4–124 sections per output. Cemu's refresh and flip clocks were
aliased throughout. The samples filled before the scene reached its
163-section maximum. The section-gated follow-up will reserve the
diagnostic slots for the physical 174-section cohort.

`r65-27078d8e` also completed the longer settled and save lifecycle
with the server-pump marker. Neither hosted pump run recorded a complete
sample window. The original moving Village workload remains queued for
separate native attribution. Hosted receipt SHA-256
`5df98e9394a81d977af76fbd6253b9747dd742414ea18b6cf87815fd6282c3e3`.
