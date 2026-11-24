# Wii U renderer performance programme

The [RC 26 physical record](physical-rc26.md) covers the verified console
boot, live MCP, the visible 100% baseline, save recovery and the scaled
world, readback and Menu handoff failures.

## Reference and validation

The baseline is client `0.8.0-rc.21` at port commit
`c775bf6e9b2328075a810aabcb10f7556c183d11`, assembled tree
`53aaa6b435053ec7da34a2dce0bfe0e35de48649` and upstream Horizon
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`. Its RPX SHA-256 is
`0deb7e60aedae3a90c0887659e763b44057be04251ad00e7297dd3cab1e939d4`.
The WUHB SHA-256 is
`5f61c65a0e6e118b109b5e73f20c121d10db721e6f8f6a183df3916bb2660852`.

The archived forced-interpreter acceptance has three saved-world sessions in
one process. Both displays, GamePad Respawn and inventory, durable saves,
position-preserving reopen, AX PCM and GX2 heap recovery passed. The
stationary distance-12 sample was still streaming, and the median was
0.337 paired display submissions per second across its 39 gameplay windows.
The single-core and multi-core paths did not complete gameplay acceptance.
Physical RC 21 reached Night. Matched physical gameplay performance is
pending, including a measured stock Full Performance baseline.

The stock Full Performance saved-world replay used NSS-patched Cemu in
forced interpreter mode and retained 54 gameplay submissions, all partial.
Median host pacing was 2,943.249 ms, p95 was 3,197.168 ms, and p99/max was
3,402.762 ms. The median frame submitted 8 section draws and 16,152 indices
across both displays. Terrain was still streaming. The replay confirmed
movement, durable save, audible AX PCM in the guest diagnostics, Night
restoration and recovery of 234,880,920 bytes in the GX2 bulk heap. The
archived distance-12 run supplies the separate RC 21 same-process reopen
evidence. Short standalone fixtures overlapped loading in the stock replay,
so its startup wall time is excluded from matched comparisons.

## Full Performance on RC 21

The pinned `graphics_preset.rs` also enables the stock sky, fog, weather and
animated textures. The status below describes the assembled RC 21 renderer.

| Stock setting | Value | Cafe path |
| --- | --- | --- |
| Render and simulation distance | 6 and 5 | Applies to client streaming and the integrated server view and simulation. |
| Resolution scale and DRS | 75%, off | The world draws into the fitted WHB viewport at native resolution. RC 21 ignores the stored scale for GX2. |
| Clouds | Off | The setting gates shared frame data. Cafe has no native cloud draw. |
| Particles | Minimal (2) | Applies to shared particle preparation. Cafe has no native particle draw. |
| Entity shadows and distance | Off and 50% | Settings reach shared preparation and entity distance reaches integrated tracking. Cafe omits the native entity passes. |
| Biome blend, leaf cutout and opaque ice | 0, off, on | These reach the shared mesher used by native solid terrain. |
| Mipmap levels | 0 | The native atlas has one point-sampled level at every setting. |
| Weather radius | 3 | Caps the shared weather geometry. Cafe omits native precipitation. |
| LOD and FXAA | 0 and off | These features are inactive in Cafe. |

The existing Cemu harness defaults to distance 12 for streaming regression.
Run `prepare --keep-fixture-distances` with separately prepared fixture files
to retain the 6/5 reference settings. Use the same options-file hashes, seed,
camera and resources in each matched run.

## RC 23 saved-world acceptance

Client `0.8.0-rc.23` at port commit
`aa6e9ecddd47cf4173707e1de4b07915fd945038` passed one stock Full
Performance session in forced-interpreter Cemu. The RPX SHA-256 is
`c1acd1f090c9873578bbbc5a479206a96b095ada8c0a7edca57bc8fa62e8af7a`.
The WUHB SHA-256 is
`8002f1811d2b82860c296a86a3bd7331e39ffaedf2d6b4675e30e50d850d69c6`.
The WUHB contains the exact standalone RPX and the package passed the
Aroma code-heap check with 13,025,344 bytes of estimated headroom.

The runs used the same prepared inputs, initial save, controller mappings,
graphics options and 130-second stationary window. RC 23 rendered its
world at 1440×809 on TV and 641×360 on GamePad using 11,378,688 bytes of
additional GX2 world surfaces. The HUD drew at output resolution. Movement,
AX PCM, the durable save and return to Night passed. The post-session GX2
bulk heap recovered to 234,880,920 free bytes. The process allocator peaked
at 335,620,266 requested bytes and settled to 44,509,148 after teardown.

| Forced-interpreter capture | RC 21 | RC 23 |
| --- | ---: | ---: |
| Partial gameplay submissions | 54 | 51 |
| Pacing samples | 53 | 49 |
| Median host pacing | 2,943.249 ms | 3,076.834 ms |
| p95 host pacing | 3,197.168 ms | 3,814.640 ms |
| p99 and maximum host pacing | 3,402.762 ms | 4,897.866 ms |
| Median section draws across both outputs | 8 | 4 |

Both captures were still streaming. Rendering stages and effective world
resolution differ, so the timings describe the two candidate workloads.
On RC 21, the median of 51 logged gameplay windows was 2,867.9 ms per
built frame outside native submission, 25.9 ms in submission outside the
measured waits and 10.2 ms inside those waits. This points to CPU-side
redraw work in the interpreter. Hardware timing remains open.

## Milestones

| Work | Current status |
| --- | --- |
| Capture and live diagnostics | HZP3 preserves the earlier capture layout and adds CPU submission spans for the native world passes, target allocation, effective dimensions and translucent index uploads. The internal status and serial windows retain host pacing tails, paired and partial counts, and effective world dimensions. RC 26 shares the sample with authenticated `game_state`. The native fixture verified 126 decoded records with phase and display distinctions. |
| Renderer parity | The [coverage inventory](coverage.md) tracks stock passes. Isolated interpreter diagnostics cover selection, crumbling, basic first-person geometry, sorted translucent terrain, stock sky discs, ordinary entities, precipitation, animated atlas cells and repeated terrain UVs on both outputs. Hosted gameplay and unsupported passes remain separately tracked. |
| Fixed scaling and DRS | Fixed world scaling uses a bounded pair of native color/depth surfaces per display and composites before native-size HUD. Host tests drive the original controller through 75–100% on synthetic GPU samples. Without qualified GPU timing on Cafe, automatic mode holds the configured floor. |
| Benchmark | Night calls the original runner for Standard, Village, Forest, Ocean, Mountain and Travel. The benchmark uses stock Full Performance with an owned disposable world and requires terrain on both outputs. Reports identify omitted passes and distinguish configured and effective dimensions. Host queue tests and the Cafe Rust check passed for the benchmark source. |
| World performance | The RC 21 culling loop tests resident sections per display, binds state per visible section and explicitly waits once per display before terrain. The current source batches invariant state once per pass and preserves a same-binary per-section reference. Both binding modes passed the fixture. Measured world impact and synchronized buffer reuse remain open. |

## RC 24 native packages

The RC 24 hosted recipe pins client `0.8.0-rc.24`, patch `0.5.0-rc.24`,
builder `0.7.0-rc.4`, port commit
`d9724e6ec97de2d3999c10b63571ee14c4734839` and assembled source tree
`966bb274f40ad7092e5a8e1c819b0c00e7f5ac79`. Its hosted recipe SHA-256 is
`eb9738f87fc83aa2a4aa45b7d3cabf94b785d548bd6a9f51b969e396c75be924`.

Two independent native builds produced identical RPX and WUHB bytes.
The RPX SHA-256 is
`7d489ca0676d6c306b6c3b014ca1e12f6626852395c8d18b143b0e631042b009`.
The WUHB SHA-256 is
`2ef0cca2eb04b989749bed7efe8b30964ad804cc3e5bd877ce376f941e1b9885`.
Both WUHBs contain the exact standalone RPX. The Aroma code-heap check
reports 12,986,560 bytes of estimated headroom. Native packages and
detailed receipts are stored privately. Hardware acceptance is pending.

## RC 25 packages and loading cancellation

Client `0.8.0-rc.25` and patch `0.5.0-rc.25` were built at port commit
`6e8ea5bc31e2c92889608960ad863d96d24d5398` and assembled tree
`45e78150bc53cfc1295250f091c67b721d49892d`. Two separate build directories
produced identical packages. One reused compiled dependencies from the RC 24
build. The RPX SHA-256 is
`ba1ac64c187291ca0285097cf3b4c0f83fe0880cb0f92f3d6f687e324ce5360c`.
The WUHB SHA-256 is
`f201c722a7fc160328951306b82f30122ff86f065d07c3b72b454ad0079a314a`.
Both WUHBs embed the exact standalone RPX. The Aroma check reports
12,973,504 bytes of estimated headroom.

The exact RPX returned to Night in 5.235 seconds when Minus was pressed
during early resource loading. A second forced-interpreter run pressed Minus
during PNG decoding and returned in 10.210 seconds. Both runs released the
loading resources, recovered 234,880,920 free GX2 bulk-heap bytes and left
the report list empty. Cancellation during timed gameplay remains pending.

## RC 26 packages and authenticated gameplay

The hosted recipe pins client `0.8.0-rc.26`, patch `0.5.0-rc.26`,
builder `0.7.0-rc.4`, port commit
`aa8d4601cb2d0a4421b10a0aed5342c6f2edd2cb` and assembled tree
`1f405190dbff9428f2585cee478efc0d9fb0307c`. The recipe SHA-256 is
`be7ba74511454c33e76f0acc229a4572fad1f8ccef3cc11afdb4d0d3ee794499`.

Two native build directories produced byte-identical packages. The second
resumed its own build after a host interruption. The RPX SHA-256 is
`7ad154a161e42cde0094ba9e92fe59181a89dfcc65797de8d69f0fcf0882d137`.
The WUHB SHA-256 is
`cf62fef35c9bdf38017190dc33d43bedf263a190dd687ce8a54e722c78a3df03`.
Both WUHBs embed the exact 37,493,740-byte RPX at offset 512. The Aroma
code-heap check reports 12,957,312 bytes of estimated headroom.

In forced-interpreter Cemu, the exact RPX passed Codex MCP pairing with
GamePad consent, authenticated world entry, durable Save & Quit, return to
Night and launcher close. `game_state` returned the live gameplay window,
both output workloads and configured graphics. One captured window had
five terrain sections on each output and one partial paired submission.
This replay used distance 6/5 with custom 100% scale. The fitted world
viewports were 1920×1079 on TV and 854×480 on GamePad, with zero extra
world-target bytes. The guest reported nonzero AX PCM, a peak of 326,518,366
requested Rust bytes and recovery to 234,880,920 free GX2 bulk-heap bytes.

The final HZP3 capture decoded 4,096 retained rows, including 39 partial
gameplay presentations. Across 37 gameplay pacing intervals the median was
7,084.776 ms, p95 was 8,464.491 ms, and p99/max was 8,844.001 ms. Another
emulator and a native rebuild overlapped the replay. Use these timings as
instrumentation evidence. A controlled RC 26 replay and same-process
saved-world reopen remain pending.

Keep stock Full Performance, experimental Wii U tuning and distance-12
regression as different configurations. Record which options affect GX2.
Avoid using interpreter cadence to select a physical resolution floor or
target frame rate. Use a warmed scene and a streaming replay for each
comparison and retain matching output captures outside the timed window.

| Comparison configuration | Settings and purpose |
| --- | --- |
| Stock Full Performance | Use the original settings listed above, with 75% fixed scale and the same gameplay and rendering inputs. |
| Wii U-tuned experiment | Use the stock distances and effects with a 75% floor and a 30 FPS target. Enable DRS for controller validation. Until a GPU signal is qualified, this holds at 75%. Compare fixed 100%, 75%, and 50% on matched hardware scenes before tuning the floor. |
| Distance-12 regression | Keep render and simulation at 12 with the existing disposable save fixture. Exercise streaming, teardown, audio, save and same-process reopen separately from the 6/5 reference. |

## Physical testing queue

1. With the console owner present, prepare a dedicated test SD with the
   existing compatible resources and a disposable copy of the saved-world
   fixture. Keep the normal SD separate. Verify the candidate RPX and WUHB
   against its receipt, place the WUHB at
   `sd:/wiiu/apps/HorizonWiiU/HorizonWiiU.wuhb`, and confirm the version in
   **Night → Versions → About this build** before opening a world. Record
   the TV mode, GamePad output, resource cache state and active graphics
   configuration. Keep the owner-approved RC 21 package available for A/B.
2. Play a warm stock Full Performance scene and the same fixed camera,
   rotation and streaming route on both packages. Capture per-display
   outcomes, complete and partial submissions, GX2 flip deltas, frame-time
   tails, server ticks and memory. Repeat capture-on/off under identical
   conditions using `cafe_performance` for the independent A/B readings.
   Use an empty `gx2-profile.enable` marker for the candidate's normal
   binding and a marker containing `2` for the matched per-section reference.
   For the RC 28 terrain stream comparison, marker `3` restores the two
   redundant buffer binds while keeping per-pass state binding. Keep the
   world and resolution identical within each comparison. Follow
   the [capture setup](benchmarks.md) for collection and decoding.
3. Exercise the existing RC 21 distance-12 regression setup. Verify terrain
   convergence, GamePad touch and controls, audible AX playback, foreground
   return, durable Save & Quit, Night restoration and a position-preserving
   reopen. Exercise authenticated diagnostics only after local consent.
4. Compare matched TV and GamePad reference views. Check
   the outline against full blocks, fences and occluded edges. Verify world
   scale transitions at 100%, 75% and 50%, full-size HUD and correct touch.
   Run **Night → Settings → Benchmark → Village** to completion, then
   repeat with Minus during loading and the timed route. Verify the report
   and resource teardown before the next launch.

Keep emulator and hardware evidence separate. Private receipts, captures,
logs, resources, fixtures and packages stay outside Git.

## Capture cost

The [native capture fixture](../../wiiu/diagnostics/gx2_profile/README.md)
completed four forced-interpreter runs in off/on/on/off order. It recorded
3 and 5 µs per loop inside recorder calls with capture disabled, and 14 and
15 µs enabled. Whole-loop means were 16.80–16.90 ms with WHB pacing.
Both enabled runs wrote a valid 126-record HZP2 capture. The decoder
distinguished Night, loading, complete gameplay, partial gameplay and TV-only
outcomes. This measures a clear-and-copy fixture. Matched in-game and
physical capture overhead are pending.

## Terrain state binding experiment

The native fixture submitted the same 128 solid draws per arm in forced
interpreter Cemu. Rebinding invariant terrain state per section took 604
and 678 µs of CPU submission time. Binding once per pass took 354 and
296 µs. Both outputs passed the separate depth, alpha-discard and blending
readbacks using the split binding API. Completion waits were outside the
timed spans. The result qualifies the command reduction on this fixture.
Matched world frame time and hardware benefit remain pending.

The pinned WHB fetch shader reads all three indexed terrain attributes
from stream 0 at byte offsets 0, 24 and 16. RC 28 binds only that stream
per draw. In a second forced-interpreter fixture, all eight output
comparisons matched their expected pixels with three draws and 30 indices
per output in each arm. The legacy arm bound all three streams on each
draw. Across both outputs, 128 draws took 117–169 µs of submission time
with the redundant binds and 88–109 µs with a single bind per draw.
The isolated result leaves the physical timing impact open.

## Gameplay GPU timing probe

The physical [RC 26 timing trials](physical-rc26.md#physical-gpu-timing-probe)
qualified the top and bottom cycle counter on isolated raster loads. RC 28
adds opt-in sampling for the hosted gameplay path with `4` in
`gx2-profile.enable`. Each output is bracketed after its WHB clear and before
its scan copy. The reader checks completion at the existing WHB frame fence
and adds no waits. Unverified retirement or an invalid counter pair disables
the probe for that process. A forced-interpreter fixture exercised the
normal paired present function and confirmed rejection of Cemu's sentinel.

When valid, serial reports the per-output spans in raw ticks, the nominal
27 MHz clock and the HZP3 sequence once every two seconds. Intermittent CPU
submission can leave GPU idle time between the markers. The spans still
need physical gameplay, foreground and scale-transition trials before use
as a DRS input.

## Hosted benchmark

Open **Settings → Benchmark** in Night, choose a scenario, then use
**Start Game**. The run uses an offline profile, reads the selected
installation's prepared resources and writes report files under its
`benchmarks` directory. A marked `.benchmark-run` directory owns its scratch
world. Minus during a running benchmark initiates the hosted save barrier.
Before Game handoff, the RC 25 source also asks resource workers to stop at
their next safe checkpoint and waits for them to join before returning to
Night. The standard version 1 report includes Cafe-specific metrics for the
native world dimensions and partial passes. Desktop A/B switches and GPU
timestamp metrics are unavailable in this launcher flow.

For acceptance, check that the benchmark reaches its selected scene, submits
terrain on both outputs, completes its route, writes a standard report and
restores Night. Repeat with Minus during loading and timed gameplay. Record
CPU mode, package and fixture hashes alongside the report. The focused host
fixtures exercise the report, pending-command cancellation and cooperative
loader checkpoints. Confirm the loader change on a packaged candidate before
relying on its response time.

### Village diagnostic

The standard Village run on RC 24 reached 42 of its 90 minimum client
columns before the 25-minute interpreter harness timeout. Terrain was still
streaming, and the run had yet to reach the timed route or publish a report.

The RC 26 source has an opt-in Village diagnostic for an owned disposable
benchmark workspace. Put `horizon-benchmark-village-diagnostic.txt` in its
`.benchmark-run` directory, with the line `Horizon Cafe Village diagnostic v1`
terminated by LF. Select Village in Night as usual. Only an existing
owner-marked workspace with that exact opt-in uses the diagnostic. It waits
for paired terrain at the current camera before timing and keeps the stock
seed, camera path and 12-second flight. Its report scenario is
`cafe-diagnostic-village`, with a `Diagnostic` label and an explicit relaxed
streaming-gate metric. Use standard Village for full-view comparisons.

The packaged RC 26 diagnostic entered Village and started the 12-second
route in forced-interpreter Cemu. After the timing marker, one paired
submission logged terrain on both outputs. The following submissions lost
visible terrain as the camera advanced through the streaming area. No
qualifying frame interval was recorded, so the runner rejected the flight
and wrote no benchmark report. The save barrier completed, the workers
released, and Night returned to its reports screen with 234,880,920 free
GX2 bulk-heap bytes. The result verifies entry, timing and failure teardown.
Standard Village report generation and cancellation during a timed route
remain open on the candidate.

## RC 27 package verification

Two isolated `build_cafe_game_isolated.sh --hosted-nightly` build directories
produced matching RC 27 files from port commit
`ac10de0c707ba0e4d121a532c0828a3cdd4619af` and assembled engine tree
`c4a11dd3605e7c32686bad2c9d912caec1caaf93`. Both passed package verification.
The WUHB contains the 37,490,077-byte RPX at offset 512.

| Artifact | SHA-256 |
| --- | --- |
| RPX | `fe252df4c48190a81cb5c0e2916aaa053aa7f64c37dee60cb052094af3395838` |
| WUHB | `1bc6379335fcd92fcc780b6393a36af2da3feaa163f5e8597e89dd1cce2b5306` |

The Aroma code-heap receipt estimates 12,952,640 bytes of headroom against
the measured contiguous allocation baseline. The exact RPX reached Night's
Profiles screen on both outputs in forced-interpreter Cemu. RC 27 remained
a local package. Physical acceptance covered RC 26 and the standalone GX2
probes. The newer RC 28 assembly passed the Cafe cross-check.
