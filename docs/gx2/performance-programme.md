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

## Milestones

| Work | Current status |
| --- | --- |
| Capture and live diagnostics | HZP2 extends the existing ring with output outcomes, observed flip progress, output surface sizes and GX2 submission spans. The existing `cafe_performance` authenticated status and serial windows remain available. |
| Renderer parity | The [coverage inventory](coverage.md) tracks stock passes. Initial implementation targets the stock selection outline and preserves the same indexed solid terrain and HUD. |
| Fixed scaling and DRS | The RC 21 world uses the fitted display rectangles at native resolution. Fixed native color/depth targets and an upscale before native HUD are in development. Automatic changes require a separately qualified timing signal. |
| Benchmark | A hosted adaptation of the existing Horizon runner is in development. Benchmark results require a verified render workload and disclosure of omitted passes. |
| World performance | The RC 21 culling loop tests resident sections per display, binds state per visible section and explicitly waits once per display before terrain. Compare measured submission and waits on matched workloads before changing these paths. |

Keep stock Full Performance, experimental Wii U tuning and distance-12
regression as different configurations. Record which options affect GX2.
Avoid using interpreter cadence to select a physical resolution floor or
target frame rate. Use a warmed scene and a streaming replay for each
comparison and retain matching output captures outside the timed window.

## Physical testing queue

1. With the console owner present, use a disposable installation and verify
   the candidate RPX and WUHB hashes. Follow the [capture setup](benchmarks.md)
   and record TV mode, GamePad output, resource cache state and the active
   graphics configuration. Retain the owner-approved RC 21 package for A/B.
2. Play a warm stock Full Performance scene and the same fixed camera,
   rotation and streaming route on both packages. Capture per-display
   outcomes, complete and partial submissions, GX2 flip deltas, frame-time
   tails, server ticks and memory. Repeat capture-on/off under identical
   conditions using `cafe_performance` for the independent A/B readings.
3. Exercise the existing RC 21 distance-12 regression setup. Verify terrain
   convergence, GamePad touch and controls, audible AX playback, foreground
   return, durable Save & Quit, Night restoration and a position-preserving
   reopen. Exercise authenticated diagnostics only after local consent.
4. As passes land, compare matched TV and GamePad reference views. Check
   the outline against full blocks, fences and occluded edges. Verify world
   scale transitions at 100%, 75% and 50%, full-size HUD and correct touch.

Keep emulator and hardware evidence separate. Private receipts, captures,
logs, resources, fixtures and packages stay outside Git.
