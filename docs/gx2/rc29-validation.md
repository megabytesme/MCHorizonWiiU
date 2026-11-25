# RC 29 hosted validation

RC 29 uses the corrected engine tree
`53dae28084460c2448e22714f6962d8bfb1fa3d8`, upstream Horizon
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`, and
[Cafe Toolchain 1.1.0](cafe-toolchain-1.1.0.md). The original
[physical evidence](physical-rc26.md#physical-post-clear-state-test)
isolated draw state that changes during the offscreen clear. RC 29 restores
the display's WHB context before rebinding the scaled color and depth buffers.
Normal launches use 100% world resolution pending hosted physical qualification.

## Hosted package

Two isolated builds from frozen port commit
`c231d779772efe095bb2ef1d96663a2ccc9163de` produced byte-identical RPX
and WUHB files. The first compilation completed, but the builder withheld
its receipt when the port checkout advanced. Build A reran from the frozen
commit using its existing objects. Build B compiled from an empty build
directory with a separate Cargo target directory. Both used the same
isolated build mount and passed `verify-output`.

The package uses client `0.8.0-rc.29`, patch `0.5.0-rc.29` and builder
`0.7.0-rc.5`. The exact recipe SHA-256 is
`7e684176da0f3a9dfb7caeef51fee8d1c676eee78633e6ff98ffd6fcde0c95ff`.
Both receipts record build ID
`bcc218c88984a73d26cbc989ef4b605361ee51c18b9167c0a5f8bb83e5c31aa4`
and toolchain lock SHA-256
`72ee75b21b158e90333afd070d4a4add4c6059a695fb0eb32ac832e3a6818221`.

| Package | Bytes | SHA-256 |
| --- | ---: | --- |
| HorizonWiiU.rpx | 37,517,719 | `713bf355957f935283304b2dff06cee02db02cadef389825fdf69a89a9f3af92` |
| HorizonWiiU.wuhb | 37,518,540 | `9d20070f5af0cda36842d5d6ca0f1a831b20993c66778bbf1cf25e7e1eda6b13` |

The WUHB contains the exact RPX at byte 512. The Aroma loader estimate
leaves 12,926,720 bytes of contiguous code-heap headroom. The package
booted to Night on both displays in the forced interpreter.

The first Cemu world run requested 75% and used the normal 100% fallback.
Both paused displays showed terrain, the HUD and the Game Menu. Native
AX reported its first nonzero PCM. The harness ended before Save & Quit
because it expected the previous pause log format. It now recognizes the
native cadence marker.

The experimental run used the same RPX and the Full Performance fixture
with render distance 6 and simulation distance 5. Cemu recorded requested
and effective 75%, a TV world buffer of 1440×809, a GamePad world buffer
of 641×360, and 11,378,688 bytes in scaled surfaces. Gameplay and pause
stills showed terrain, the held hand and the native HUD on both displays.
AX delivered nonzero PCM. Profiling mode `4` reported an invalid GPU cycle
pair in Cemu. Console timing and the default 100% fallback decision still
depend on the physical checks below.

Three experimental rounds completed in one Cemu process. All three used
effective 75%, saved and returned to Night. The first save recorded GamePad
movement. Both reopens preserved the exact saved position. During round two
the first emulated save touch was missed. A second touch while the game was
still paused completed the save. The harness now has a bounded retry for
that case.

Both sound decoders joined and the AX callback consumed nonzero PCM in each
round. After every save, the GX2 bulk heap recovered to 234,880,920 free
bytes with a largest free block of 234,880,896 bytes. Post-drop Rust live
bytes were 44,424,467, 44,714,536 and 44,674,609. All three session HZP3
captures decoded. The first retained 4,096 records with zero valid GPU
timing matches. Close Horizon released the launcher view, GX2 and ProcUI,
persisted the final profile and returned through `coreinit.exit(0)`. The
Cemu raw RPX launcher used its placeholder title exit path. Aroma Menu
handoff and relaunch remain console checks.

## Standalone diagnostics

| App | Exact RPX SHA-256 | Exact WUHB SHA-256 |
| --- | --- | --- |
| GX2 Scale Check | `c8ad5161a9299d9097a0d916d846ff19c1fd48beb9c22e697bcea25aa355c8e8` | `4591ecdf6e3fe47e850a5e50012e506cecbb1e6d11bb2cba860d58bedfd71479` |
| GX2 Display Lists | `3e1d897f7edd6262d158cfaf116fe7a29ee7fb52b92b5ba0eea0f26fc8df90aa` | `b02076bfba4d733cfdbf7eec82964b620f7c68a388754bdebccf473506c94c9b` |

The Scale Check verified 40 TV and GamePad pixels in forced-interpreter
Cemu. The sequence is 100%, 75%, 50%, 95%, and 75%. The separate readback
RPX passed ten Cemu checks. Only the readback-free WUHB is in the hardware
queue. The [display-list probe](../../wiiu/diagnostics/gx2-display-lists/README.md)
retired all 96 timed samples and passed 1,536 color checks in Cemu.

### Physical Scale Check

The verified readback-free WUHB completed all five trials on Wii U. The
allocation-budget check passed. Every HDMI still matched the red top,
blue bottom, black margin and small green HUD bar. The owner observed the
same pattern on the GamePad throughout. Serial recorded the final draw,
the Wii U Menu launch and the ProcUI exit event.

| Trial | Scale | TV pattern | GamePad pattern | Extra surface bytes |
| --- | ---: | --- | --- | ---: |
| 1 | 100% | 1898×1054 | 832×454 | 0 |
| 2 | 75% | 1424×791 | 624×341 | 11,018,240 |
| 3 | 50% | 949×527 | 416×227 | 4,853,760 |
| 4 | 95% | 1803×1001 | 790×431 | 17,473,536 |
| 5 | 75% | 1424×791 | 624×341 | 11,018,240 |

The diagnostic deliberately insets its pattern from the display edges.
These dimensions apply to the test pattern. Hosted gameplay records its
own world-target sizes.

### Physical Display Lists

The Wii U retired all 96 samples across six passes, with 64 draws and
384 indices per output in every sample. The standalone app returned to
the Wii U Menu. The TV still and GamePad photo show the expected
eight-by-eight grid with alternating tile widths. The GamePad reverses the
red and blue rows and moves the green tile from top left to bottom right.
The photos document the pattern on both outputs. Serial covers all passes.

| Submission | Output | Median command construction (µs) | Median flush (µs) | Median retirement wait (µs) | Median total (µs) |
| --- | --- | ---: | ---: | ---: | ---: |
| Direct | TV | 19.85 | 3.12 | 607.82 | 630.92 |
| Section lists | TV | 12.91 | 3.15 | 607.83 | 624.16 |
| Frame list | TV | 0.13 | 3.16 | 607.64 | 610.88 |
| Direct | GamePad | 18.34 | 3.17 | 138.07 | 159.62 |
| Section lists | GamePad | 11.41 | 3.31 | 138.16 | 152.86 |
| Frame list | GamePad | 0.14 | 3.18 | 138.18 | 141.64 |

The probe used 66,048 bytes of reserved command-list memory. Frame lists
nearly eliminated CPU command construction for this fixed grid. Flush and
retirement waits stayed close across all three methods on each display.
The CPU retirement wait includes synchronization. Gameplay throughput
requires a separate measurement with terrain and changing state.

### Hosted preparation and fixed-scale measurements

The first physical launch reached the game handoff after 67 minutes. The
test instance had received an archive bound to the default install path.
The block-model and sprite loader made 20,583 loose-file accesses without
opening the archive. For the retry, the archive was rebound to the test
instance. Its manifest and payload bytes matched the original. The same
candidate then opened the archive and recorded one loose-file access by
the end of that stage. The measured UI atlas decode fell from 769.0 to
5.4 seconds. The verified executable and disposable save stayed in place.

| Setting | Requested and effective scale | TV world target | GamePad world target | Additional surfaces |
| --- | ---: | ---: | ---: | ---: |
| Direct | 100% | 1920×1079 | 854×480 | 0 bytes |
| Scaled | 75% | 1440×809 | 641×360 | 11,378,688 bytes |
| Scaled | 50% | 960×540 | 427×240 | 5,038,080 bytes |
| Return to direct | 100% | 1920×1079 | 854×480 | 0 bytes |

The 100% TV still shows terrain and HUD. The owner observed gameplay on
the GamePad at 75% and 50%. Window captures of the Blackmagic projector
produced black stills during the sweep and again after the return to
100%. Capturing the visible projector during stock Village and the
same-process fixture reopen confirmed textured terrain and a native HUD on
TV at 75% and 50%, respectively. Authenticated state confirmed the 50%
world dimensions on the reopen.

The owner explicitly applied Full Performance. At its stock 75% scale,
244 pairs were submitted in 30.60 seconds (7.97/s). After changing only
the scale to 100%, 238 pairs were submitted in 30.97 seconds (7.69/s).
Both windows drew 294 terrain sections per output and carried the
`0x8000` post-effects omission. Separate 50% windows reached about 9.7
pairs/s with different terrain coverage. These numbers measure paired
submission cadence.

Serial reported positive TV and GamePad GPU intervals during hosted
gameplay. In 878 complete serial reports across the sweep, the logged
differences matched their endpoints and the GamePad start followed the
TV end. Across sixteen reports in each matched sample, the median span
from TV start through GamePad end fell from 68.06 ms at 100% to 62.44 ms
at 75%. The bracket can include pipeline idle periods. Endpoint capture
and ordering passed. A qualifying GPU load signal for DRS remains open.

For the measured steady windows at 75% and 100%, the exclusive CPU wall
buckets accounted for the loop budget to within 0.03 ms per iteration
at serial precision and reported zero invalid nesting steps. Native
submission outside measured waits was about 60 and 66 ms per iteration,
respectively. Artwork preparation was about 39 ms at both settings.

Both Save & Quit paths passed the durable barrier and returned to Night.
The GX2 bulk heap recovered to 234,880,920 free bytes with a largest
aligned block of 234,880,896 bytes after each session. The authenticated
launcher reopened the selected fixture in the same process and exposed
its saved 100% scale. The first gameplay checkpoint and the complete
temporary checkpoint from the reopen each decoded as 4,096 HZP3 records.
The second file occupied the temporary name because RC 29's rename could
not overwrite the existing checkpoint. Close Horizon wrote its final
capture and ProcUI returned to the Wii U Menu.

### Stock Village

The standard Village run used Full Performance at 75% and the stock
readiness gates. Spawn and the Village scene became ready after 170.0
and 175.0 seconds, respectively. Village had 213 resident columns,
zero pending chunks and 562 submitted terrain sections on each output
at readiness. A projector still during the run confirmed visible TV
terrain and HUD.

The 12-second route presented 58 pairs over 12.146 seconds. Thirty of
those pairs contained terrain. The 29 eligible adjacent intervals covered
6.834 seconds, or 56.3% of the measured wall time. Night returned and
opened the written report. The report recovered from the SD confirmed
the GX2 backend, stock seed `1234`, distances 6/5, both world target sizes,
and eligible frame-time p50 of 236.0 ms and p95 of 347.5 ms. The measured
terrain gaps limit comparisons against runs with steadier coverage.

## Console procedure

Perform this queue with the owner present. Stage the approved hosted package
and standalone diagnostics together on the disposable SD after checking their
receipts. Keep the existing normal installation and saves on the owner's
regular SD. Use projector stills, GamePad observation and the existing serial
monitor for evidence.

The test SD was staged with all three WUHB hashes checked and safely
ejected. The `RC29-Qualification` instance has its own prepared resources,
graphics settings and `Reopen Fixture` save. It starts at fixed 100% with
render distance 6 and simulation distance 5. The app-wide experimental
marker enables the **Graphics → Quality → Resolution Scale** control for
the fixed-scale sweep. Profile mode `4` records GPU endpoints.
The previous RC 26 executable and its profile captures were backed up
off-card. The scale diagnostic is the first console launch.

1. Launch GX2 Scale Check. For each serial trial record TV and GamePad
   orientation, margins, the small native green HUD bar and upscaling.
   Confirm the second 75% trial and return to the Wii U Menu.
2. Launch GX2 Display Lists. Verify the tile pattern on both outputs,
   collect all 96 timing samples with retirement, and confirm the Wii U
   Menu handoff. Compare command construction, flush and retirement for
   equivalent workloads.
3. Launch the hosted candidate and confirm its version in Night. Select
   `RC29-Qualification` under **Version & resources → Installed resources**,
   then select `Reopen Fixture` under **Worlds & servers → Local worlds**.
   Verify direct 100%, both screens, audio, controller input, touch, Save &
   Quit and exact saved-position recovery. Record resource retirement and
   do a same-process reopen.
4. After the standalone inspection passes, use
   `gx2-world-scale-experimental.enable` for the controlled hosted sweep.
   Hold render distance 6, simulation distance 5 and every other setting
   fixed. Confirm 100%, 75%, 50% and a return to 100% in one session,
   then verify both screens again after foreground transitions and reopening.
   Record requested and effective scale and actual dimensions from the
   authenticated graphics endpoint. Record visible terrain and native HUD in
   the stills for each setting.
5. With stable terrain, repeat the single-setting sweep on the warmed fixed
   camera, then measure rotation, traversal, and denser scenes. Use profiling
   marker `4` for endpoints. Preserve each HZP3 gameplay checkpoint with the
   serial log from the same run. Report paired submission cadence, terrain
   coverage, frame tails and exclusive CPU buckets. Use the
   [correlation command](benchmarks.md#capture-format-and-interpretation)
   to compare endpoint order and overlap before interpreting the per-output
   GPU spans. Run each existing same-binary terrain comparison separately.
6. Run the original Village scenario with its stock gates and verify the
   saved report after return to Night. Verify the distance-12 lifecycle
   separately. Confirm profile persistence, Close Horizon, the Wii U Menu
   handoff and relaunch.

Automatic resolution stays gated until both hosted fixed scaling and
scale-sensitive gameplay GPU timing pass on the console. The RC 26 direct
reference was 6.15 paired submissions per second and approximately 20 TPS.
The alternate preset and partially covered frames make it a separate
reference from the stock Full Performance sweep above.
