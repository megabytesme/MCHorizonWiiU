# RC 57 terrain preparation and grouped GX2 submission

| Input or output | Identity |
| --- | --- |
| Port build source | `b76f6278df513d6793d6f9ee34620ace7b019cff` |
| Engine tree | `c52bd63b767ee1502cb6c447c6940bb1554b75f6` |
| Recipe SHA-256 | `36aa01eb869480062b75906ad029e5d4eb4da6c20eca2ca212f8df87c44ff5e2` |
| Cafe Toolchain | `1.1.0` |
| RPX SHA-256 | `21b4ae1736d9e76ad027071a0737dc09a8edc9b8c2b52adbd4b01b7877d7290c` |
| WUHB SHA-256 | `56967e56212d0bc23d5e1f62f46e1321eed210b604066babf29412d6b15dbc8b` |
| Estimated Aroma code-heap headroom | 12,480,960 bytes |

Ben-Desktop assembled and built the exact source in
`/var/tmp/mchorizon-rc57-native.jxpflwax/out` with
`tools/build_cafe_game_isolated.sh --hosted-nightly` and the pinned source
and input caches. Builder output verification and independent file hashes
passed. The 26 builder and 50 GX2 host tool tests passed.

## Direct terrain rows

The mesh worker decodes uniform and four-bit block palettes directly
into the padded grid. Wider palettes retain the bulk decode path.
The scratch block array is allocated when a worker first needs it.
Light input, revision, visibility and upload ownership stay the same.

The host fixture checked every padded block and light value for 4,056
sections in 169 saved columns, including missing neighbors. Synthetic
controls covered wider local and global palettes and mixed column
heights. Fourteen palette tests and two section tests passed.

In 21 alternating host rounds, the median for 1,159 matched non-air
grid fills fell from 12.942 to 11.195 ms, a 13.5% reduction. The p90
fell from 14.255 to 11.986 ms. The four-bit path reached 960 of those
sections and avoided a 16 KiB write and reread for each. Whole-mesh
CPU time, memory traffic and Village coverage need native measurements.

## Grouped command submission

| Physical arm | Profile and markers |
| --- | --- |
| Grouped control | Plain `4`, `gx2-hud-view-lanes.enable`, `gx2-grouped-present.enable` |
| Early submission | Grouped control plus `gx2-grouped-early-flush.enable` |
| Queue observer | Early submission plus `gx2-grouped-retirement.enable` |

The trial can flush the already queued world draws while the exact
prior combined flip remains pending. It still waits before both scan
copies and performs the existing final retirement check. The entry
gate verifies the same swap ticket and that the previous GPU watermark
is covered after both callbacks. The early flush leaves the render
targets and HUD arena with the current synchronous presentation.

Sixteen host timeline cases covered an already-flipped ticket, counter
wrap, a callback failure, foreground loss, timeout recovery and final
timestamp failure. One modeled six-tick scan wait overlapped six GPU
ticks, with the modeled sequence falling from 17 to 11 ticks. This
test checks command order and ownership. Actual FIFO progress and time
savings remain unmeasured.

The matched WUHB is staged for the supervised fixed-75% independent
saved-view A/B. Record the paired cadence, combined flips, native passes
and heap recovery. Run the queue observer separately and repeat the
same stationary window and foreground recovery. See the
[fixed saved-view procedure](benchmarks.md#physical-fixed-75-saved-view).

## Grouped Cemu lifecycle check

The exact RPX ran two multicore recompiler sessions with the same
disposable world, prepared archive and stock fixed-75% settings. Both
sessions used the grouped HUD lanes. One added the early submission
marker. Both captured independently rendered TV and GamePad images
before and after a 30-second dwell.

| Observation | Grouped control | Early submission requested |
| --- | ---: | ---: |
| First returned Game frame | 73.627 s | 76.020 s |
| First paired terrain gate | 76.602 s | 78.917 s |
| Sections per output at the gate | 2 | 2 |
| Durable backward movement | 8.623 blocks | 8.623 blocks |
| Confirmed early flush | — | Unobserved |
| HUD lane destruction | Assertion | Assertion |

The disposable saves contain the same moved position in both runs.
After the Game and world-cache release, both sessions stopped at the
native HUD arena destruction assertion. Each reported a lane-token
fallback before exit. The grouped control reproduces the failure with
the early-flush marker absent. The trace does not establish which
arena state prevented destruction or whether the emulator misreported
the return. The grouped hardware arms are staged and on hold until
retirement and teardown qualify. The stock direct path and its separate
terrain worker validation remain eligible for testing.

Acceptance receipt SHA-256, grouped control then early request:
`31c956c9886897f69156b629e31ad94a6e0dd33342b6ea442e20cfa16235816d` and
`a955d0c54799d780c42a8cdedd6d353e054d8f712c4d3432579182e5a96b212f`.
