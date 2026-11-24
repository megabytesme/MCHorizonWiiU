# Wii U renderer performance programme

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

## Milestones

| Work | Current status |
| --- | --- |
| Capture and live diagnostics | HZP3 preserves the earlier capture layout and adds CPU submission spans for the native world passes, target allocation, effective dimensions and translucent index uploads. Authenticated `cafe_performance` status and serial windows retain host pacing tails, paired and partial counts, and effective world dimensions. The native fixture verified 126 decoded records with phase and display distinctions. |
| Renderer parity | The [coverage inventory](coverage.md) tracks stock passes. Isolated interpreter diagnostics cover selection, crumbling, basic first-person geometry, sorted translucent terrain, stock sky discs, ordinary entities, precipitation, animated atlas cells and repeated terrain UVs on both outputs. Hosted gameplay and unsupported passes remain separately tracked. |
| Fixed scaling and DRS | Fixed world scaling uses a bounded pair of native color/depth surfaces per display and composites before native-size HUD. Host tests drive the original controller through 75–100% on synthetic GPU samples. Without qualified GPU timing on Cafe, automatic mode holds the configured floor. |
| Benchmark | Night calls the original runner for Standard, Village, Forest, Ocean, Mountain and Travel. The benchmark uses stock Full Performance with an owned disposable world and requires terrain on both outputs. Reports identify omitted passes and distinguish configured and effective dimensions. Host queue tests and the Cafe Rust check passed for the benchmark source. |
| World performance | The RC 21 culling loop tests resident sections per display, binds state per visible section and explicitly waits once per display before terrain. The current source batches invariant state once per pass and preserves a same-binary per-section reference. Both binding modes passed the fixture. Measured world impact and synchronized buffer reuse remain open. |

The RC 24 hosted recipe pins client `0.8.0-rc.24`, patch `0.5.0-rc.24`,
builder `0.7.0-rc.4` and assembled source tree
`966bb274f40ad7092e5a8e1c819b0c00e7f5ac79`. See the dated hosted catalog
for the complete patch chain. Native packages and their validation receipts
are tracked separately from this source identity.

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
   Keep the world and resolution identical for the two binding arms. Follow
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

## Hosted benchmark

Open **Settings → Benchmark** in Night, choose a scenario, then use
**Start Game**. The run uses an offline profile, reads the selected
installation's prepared resources and writes report files under its
`benchmarks` directory. A marked `.benchmark-run` directory owns its scratch
world. Minus cancels the benchmark and initiates the hosted save barrier
before returning to Night. The standard version 1 report includes Cafe-specific
metrics for the native world dimensions and partial passes. Desktop A/B
switches and GPU timestamp metrics are unavailable in this launcher flow.

For acceptance, check that the benchmark reaches its selected scene, submits
terrain on both outputs, completes its route, writes a standard report and
restores Night. Repeat with Minus during loading and timed gameplay. Record
CPU mode, package and fixture hashes alongside the report. The focused host
fixture exercises the report and pending-command cancellation independently.
