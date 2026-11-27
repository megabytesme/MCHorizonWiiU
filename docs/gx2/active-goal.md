# Wii U active engineering goal

Status active. Baseline verified on 26 November 2026. Canonical development is on
`megabytesme/MCHorizonWiiU:main`; the programme started at
`deb2e4dcb73976abcc432ab6986ebceb647c2619`. The measured candidate remains
RC 38, assembled tree `8ecf49ba0a2c12da512ccc803730d79ece8aeb61`, Horizon
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`, Cafe Toolchain 1.1.0.
Both local package hashes match the build receipt. RC 35 is the physical
fallback. [Machine-readable status](active-goal.json) Â· [RC 38 evidence](rc38-validation.md)

## Single-player qualification comes first

Prioritise ordinary single-player gameplay with stock Full Performance,
fixed 75%, separately rendered TV and GamePad views, and the original
settled saved-camera workload. Qualification requires repeated
physical 60 FPS-equivalent windows, correct native shaders and applicable
stock passes, meaningful terrain coverage and stable memory. Keep
optimising when throughput exceeds 60.

Consolidate the major experiments into a small stock and trial package
set. Compare HUD storage, grouped presentation, verified retirement and
terrain bandwidth at matched settings before changing defaults. Measure
loading, traversal and automatic resolution separately. Physical
qualification remains supervised while host, Cemu and source work
continues. [Physical A/B queue](post-rc35-queue.md)

## Independent views and local players

The primary 60 FPS workload requires separately rendered world views on
TV and GamePad. Each output submits its own world geometry, projection and
depth work. Both can reuse immutable terrain, model and texture resources.
Resampling one finished world image across the displays is optional for
compatible single-player views. That mode has its own results and is
excluded from primary acceptance.

The local multiplayer architecture targets four TV seats and one GamePad
seat in a shared world, each with its own camera, controls, player state,
inventory and HUD. Establish independent dual-output performance first,
then qualify two independent local views, two TV seats, four TV seats,
and the fifth GamePad seat. Retain the existing native viewport and
separate sort-order fixtures. After physical single-player qualification,
adapt the smallest practical two-player path from upstream Horizon's
player clients, controller routing and integrated-server connections.
Measure the incremental CPU, GPU, memory, streaming and simulation costs
before expanding that architecture.
[Independent rendering and splitscreen roadmap](independent-views.md)

## Acceptance and current evidence

| Outcome | Completion evidence | Latest verified evidence |
| --- | --- | --- |
| Sustained native gameplay | Repeated settled physical windows of at least 60 seconds on stock Full Performance, fixed 75%, DRS off, render 6 and simulation 5, with the saved 174-section view independently rendered on both outputs. Record paired cadence, unique flip evidence where available, frame-time tails, terrain, TPS, visuals and memory. Keep separate heavy and traversal results. | RC 38 delivered 19.971 paired submissions/s over 98.593 seconds at fixed 75%, with a 54.391 ms p99 interval. Two shorter windows delivered 20.011 and 20.001 pairs/s. The matched fixed 50% sweep reached about 29.92 pairs/s. |
| Persistent authenticated MCP | One owner-approved full-scope grant across Night, games, process and console restarts and compatible updates. Exercise the complete launch, capture, settings, save/reopen, benchmark and multiplayer sequence, plus revocation and rejection of unknown clients. | RC 42 reused the RC 26 disposable grant in Cemu and exercised Night, launch, live graphics, save and return, and benchmark status and report reads. Physical restart and the remaining tool flow are still pending. |
| Usable loading | Below 60 s on physical Wii U. Measure first responsive gameplay, first paired terrain and settled terrain at comparable cache states. Target substantially faster reopening with a 30 s stretch goal. | RC 38 first returned game frame 116.798 s after upgrade and 106.918 s on warm reopen. First paired terrain 124.937 s and 114.999 s. The warm run restored both preparation snapshots. |
| Online mode | Physical account-backed join, terrain, movement, clean disconnect and rejoin. Qualify profile certificates, SRV and bounded, cancellable requests. | RC 43 reused the approved grant and account in Cemu, joined and rejoined a secure online-mode server and moved 28.49 blocks. Physical qualification is pending. |
| Automatic DRS | Qualified physical timing and upward/downward transitions at 30 and 60 targets, stable surface allocation and native HUD on both outputs. | Physical upward transitions at 30 FPS. The corrected 60 FPS probe boots in Cemu and reports unavailable GPU endpoints there. |
| Village streaming | At least 95% eligible terrain time on the original route and rules. | RC 33 measured 62.34%. Separate RC 33 stationary control was 99.64% eligible at fixed 100% and 562 sections per output. |
| Rendering and splitscreen | Improve the 32-pass matrix with dual-output checks. Qualify the five-seat architecture incrementally with independent camera, HUD, controller, world and resource ownership. Test nearby and distant seats and distinguish renderer fixtures from full local gameplay. | Separate TV and GamePad projections are active for one player. The native viewport fixture passed 12 nearby and distant layout cases in Cemu, with five independent cameras at its largest layout. Physical timing and local player integration remain open. [Native fixture](../../wiiu/diagnostics/gx2-independent-views/README.md) |
| World and lifecycle integrity | Save/reopen, foreground and clean exit loops with readable saves, stable memory and no increasing retained allocations. | RC 38 restored the saved pose on reopen and returned the GX2 bulk heap to 234,880,920 free bytes after the first save and exit to Night. |

## Current work

- Keep tonight's changes in independent rendering, resource retirement,
  terrain, loading, DRS and Cemu correctness. Authentication, permissions
  and security controls are on hold for separate review.
- RC 49 halves the native index payload for eligible sorted sections and
  reuses the decoded block entity model document. Exact native packaging
  and host checks passed. [RC 49 qualification](rc49-qualification.md)
- Offline analysis of the settled physical RC 38 capture measured 29.965 ms
  per loop in the two HUD callbacks, including synchronization. RC 33 Village
  continued uploading meshes during the terrain dropout. The capture leaves
  the cause of the dropout unresolved. [Host stage timings](host-stage-profile.md)
- RC 50 frees mesh admission slots held by unstarted work beyond the visible
  area. In the host traversal fixture, one chunk of camera motion admitted
  20 newly visible sections from a full queue. The exact native build passed.
  Village coverage still needs measurement. [RC 50 qualification](rc50-qualification.md)
- The grouped HUD trial reached a verified retired pair in offline Cemu.
  The corrected harness captured TV and GamePad independently before and
  after the dwell, then verified movement, save and GX2 bulk heap recovery.
  The three matched physical arms are staged. [Grouped HUD trial](grouped-hud-trial.md)
- RC 51 consolidates the opt-in renderer arms, memory inventory and GPU/CPU
  profiling into one native package for matched comparison. Its native
  package and combined host checks passed. [RC 51 qualification](rc51-qualification.md)
- RC 52 reduces prepared resource work, retries transient DRS allocation,
  and packages exact compact terrain formats for physical A/B. Cemu's
  multicore recompiler reached gameplay, rendered both displays and saved
  the moved player. [RC 52 qualification](rc52-qualification.md)
- RC 53 packages the forward upload fix and verified warm reopening.
  The bounded host fixture admitted 28 forward sections from a full
  queue and rearmed each parked request. [RC 53 qualification](rc53-qualification.md)
- The native 24-byte half-position fixture matched stock RGBA under two
  lightmaps on both outputs. Reversed water quad reuse also matched the
  original sorted draws. Eligible terrain and memory savings in ordinary
  gameplay still need measurement. [Terrain format trials](terrain-format-trials.md)
- The verified archive trial shares one freshly checked descriptor across
  ordinary resource and HUD preparation. The Cemu warm round reached
  gameplay 80.766 seconds sooner and both rounds restored the original
  saved pose and GX2 bulk heap. [Verified archive replay](verified-archive-reopen.md)

- Prioritise bounded CPU/GPU pipelining, verified retirement, independent
  viewport submission, geometry and visibility work, shaders and memory
  access. The RC 38 capture clusters at three combined refresh periods
  at 75% and two at 50%. The next GPU boundary separates completion through
  solid terrain from the rest of each output callback without adding a
  wait. Compare matched views and preserve the stock graphics workload.
  [Independent rendering roadmap](independent-views.md)
- The RC 39 source recipe keeps owner-approved refresh grants durable and
  migrates active legacy grants on refresh. Real Codex host sessions
  verified read-only and all four scopes through pairing, independent
  restart and token refresh. The exact tool audit and RC 41 graphics
  controls are on main. Complete Night, gameplay, revocation and restart
  qualification requires a packaged run. [MCP comparison](../mcp/nightly-parity.md)
- RC 41 measured 8.018 s of duplicate integrated-server CPU preparation
  in Cemu. A separate source branch stages one-time reuse for the same
  resource generation. Compare first entry, warm reopen and peak heap
  with identical archives and cache states. [Handoff experiment](https://github.com/megabytesme/MCHorizonWiiU/blob/codex/loading-cpu-handoff/docs/gx2/loading-performance.md#integrated-server-cpu-handoff-experiment)
- A host-only stock HUD seed restored a 112 MiB atlas byte-for-byte from
  9.53 MB of retained data. The same verified archive descriptor backed
  the rebuilt font, and a replacement at the same path invalidated the
  seed. Cafe needs the corresponding Night lifetime and mapped upload.
  [Stock HUD seed fixture](../../wiiu/diagnostics/stock-hud-seed/README.md)
- The RC 40 package builds with the opt-in terrain retirement path and
  prepared server resource reuse. Night passed the independent Cemu
  display checks. Both saves and the reopen completed. Forward movement
  met a sand block at the archived starting pose. [RC 40 package](rc40-validation.md)
- The Cemu x64 reservation fix passed carry, operand and allocator probes
  in both JIT modes. Exact RC 38 opened one world, played audio and saved
  in multi-core JIT. Warm reopen still panicked, and the scratch emulator
  duplicated the GamePad capture. Use the established forced interpreter
  for sustained replay and independent pixels. [Cemu record](../../wiiu/cemu/RECOMPILER.md)
- RC 41 adds fixed-scale graphics control through authenticated MCP,
  narrower villager texture discovery, a stock Fusion shortcut and the
  opt-in back-face trial. After recovering the partial copy and verifying
  its inputs, the exact `4b` package rendered terrain and HUD on both
  Cemu displays, moved 4.313 blocks backward and passed PCM, save, heap
  recovery and clean exit. [RC 41 package](rc41-validation.md)
- RC 42 includes benchmark control and report access, axis-facing terrain
  ranges, Village trace sequencing, the Cafe SRV adapter and a scratch
  Cemu GamePad MCP capture. The corrected package passed the native build
  and reused the original Codex grant through a saved-world round. The
  configured and paired scales agreed at 100%, 75%, 50% and 75%.
  Two fresh Codex processes reused the same grant after the app restarted.
  The same grant started and cancelled a benchmark with the normal save
  unchanged. The local reports page blocked the next remote launch. A
  fresh interpreter process started the rerun and reached 26 of the
  90 required columns before the harness cancelled. Teardown restored
  the GX2 bulk heap and exited normally. A timed result is still open.
  [RC 42 qualification](rc42-qualification.md)
- RC 43 adds Night profile selection, the CPU handoff and
  an opt-in solid shader specialization, plus authenticated saved-bookmark
  joins. The standalone Cemu control and shader trial matched both full
  outputs with fog enabled and disabled. The profile-refresh path now
  passes the host fixture using the native module layout. The pinned
  native build and both package hashes passed. The exact package passed
  two Cemu saved-world rounds with both displays, 4.313 blocks of movement,
  audio, pose restoration and complete GX2 bulk heap recovery. The CPU
  fork took 0.493 and 0.528 s. The warm Game frame arrived earlier, and
  the later 10-section sample used a different saved pose.
  [RC 43 qualification](rc43-qualification.md)
- RC 44 stages same-client load cancellation, remote return to Home,
  idle Night close and per-run authenticated GX2 profile captures. It also
  drops queued mesh snapshots once a newer revision supersedes them on
  Cafe. The deterministic host fixture reduced stale work from 157 jobs
  to zero and kept the final revisions identical. The pinned source
  assembly and native package verification passed. The exact Cemu
  package reused the prior grant, cancelled an ordinary launch and two
  consecutive scoped benchmark runs, preserved all 26 ordinary save
  files, and closed Night through MCP. Full timed report publication
  and the original Village route still need qualification.
  [RC 44 qualification](rc44-qualification.md)
- RC 45 extends the stock audit to 62 tools, including bookmark changes
  and isolated owner test worlds. The candidate adds the opt-in `4v`
  solid draw plan and `4j` guarded shared HUD submission. The pinned
  native build and package hashes passed. The exact Cemu package reused
  the original four-scope grant, repeated the RC 44 cancellation flow
  and added, edited and removed a test bookmark. The first disposable
  world reached paired terrain below the four-section gate. The save
  barrier then failed because Cemu on Windows could not create the
  temporary player-data directory at that path length. A shorter
  fixture reached paired terrain and wrote its level, region and player
  files. A stale copied credential triggered reuse detection and
  revoked that disposable grant before the graceful-close checkpoint.
  The writer fix passed seven host tests covering recovery and concurrent
  saves.
  [RC 45 qualification](rc45-qualification.md)
- RC 46 extends the stock audit to 63 tools and adds authenticated
  retrieval of completed benchmark captures. The pinned native build
  passed and both package hashes verified. The exact package reopened
  the shorter fixture with the original approved grant from a separate
  disposable installation. Both displays reached terrain and the durable
  save barrier completed. All 26 ordinary save files matched by SHA-256.
  A second reopen and save passed through the same authenticated session.
  A separate interpreter package completed its Title-only report, and
  the original grant retrieved all six capture pages after an application
  restart. The 281,628 reconstructed bytes matched the saved HZP3.
  The stock gameplay workload still needs a complete timed run.
  [RC 46 qualification](rc46-qualification.md)

RC 47 stages shorter Cafe save paths and repeats authorization checks
after blocked Night operations and game reply delivery. The assembled
recipe preserves all 63 stock registrations and scopes. The host
regression rejects a revoked grant after restart and verifies isolation
of a newly paired read-only grant. The native build and package hashes
passed. [RC 47 qualification](rc47-qualification.md)

RC 48 binds MCP benchmark cancellation to its run and registered client.
The same ownership logic passed two live Cemu cancellations and rejected
the preceding run's ID. RC 48 also resolves the pack-defined default
post chain during asset installation. Both authorization host fixtures
and the verified native build passed. [RC 48 qualification](rc48-qualification.md)

## Physical queue

The last deployment receipt records RC 38, profile marker `4`, the controlled
LAN bookmark and the corrected DRS60 probe on a safely ejected card.
No later launch is established. The existing COM3 monitor was recording at
57600 baud, 8N1. Use stills from the HDMI projector outside timing windows.
Verify package hashes and the existing qualification archive binding before
the supervised session. [Detailed queue](post-rc35-queue.md)

- Capture matched fixed-scale and early-TV A/B runs, including Home and
  foreground return. Preserve the original stationary and Village workloads.
- Run the corrected DRS60 diagnostic once. The earlier DRS60 WUHB failed
  before application entry with Wii U error 160-2203 and is retired.
- Replay controlled online-mode join and reconnect with the existing Night
  profile. Capture terrain and movement on both outputs.
- Compare first entry and warm reopen with a verified candidate. Record
  first usable gameplay, stage overlap, save integrity and heap recovery.
- Qualify persisted full-scope MCP trust and the automated workflow after the
  connected candidate covers those operations.
