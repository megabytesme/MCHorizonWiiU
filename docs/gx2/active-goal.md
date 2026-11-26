# Wii U active engineering goal

Status active. Baseline verified on 26 November 2026. Canonical development is on
`megabytesme/MCHorizonWiiU:main`; the programme started at
`deb2e4dcb73976abcc432ab6986ebceb647c2619`. The measured candidate remains
RC 38, assembled tree `8ecf49ba0a2c12da512ccc803730d79ece8aeb61`, Horizon
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`, Cafe Toolchain 1.1.0.
Both local package hashes match the build receipt. RC 35 is the physical
fallback. [Machine-readable status](active-goal.json) Â· [RC 38 evidence](rc38-validation.md)

## Acceptance and current evidence

| Outcome | Completion evidence | Latest verified evidence |
| --- | --- | --- |
| Sustained native gameplay | Repeated settled physical windows of at least 60 seconds on stock Full Performance, fixed 75%, DRS off, render 6 and simulation 5, with the saved 174-section view on both outputs. Record paired cadence, unique flip evidence where available, frame-time tails, terrain, TPS, visuals and memory. Keep separate heavy and traversal results. | RC 38 delivered 20.011 and 20.001 pairs/s in two shorter windows. The matched scale sweep was about 19.97 pairs/s at 100% and 75%, and 29.92 at 50%. |
| Persistent authenticated MCP | One owner-approved full-scope grant across Night, games, process and console restarts and compatible updates. Exercise the complete launch, capture, settings, save/reopen, benchmark and multiplayer sequence, plus revocation and rejection of unknown clients. | RC 38 verified HTTPS launch, game calls and return to Night. The existing native tool inventory and separate encrypted LAN control expose different capabilities. |
| Usable loading | Below 60 s on physical Wii U. Measure first responsive gameplay, first paired terrain and settled terrain at comparable cache states. Target substantially faster reopening with a 30 s stretch goal. | RC 38 first returned game frame 116.798 s after upgrade and 106.918 s on warm reopen. First paired terrain 124.937 s and 114.999 s. The warm run restored both preparation snapshots. |
| Online mode | Physical account-backed join, terrain, movement, clean disconnect and rejoin. Qualify profile certificates, SRV and bounded, cancellable requests. | RC 36 passed authenticated join and reconnect in Cemu. RC 38 updated profile lookup and signing keys. Physical session pending. |
| Automatic DRS | Qualified physical timing and upward/downward transitions at 30 and 60 targets, stable surface allocation and native HUD on both outputs. | Physical upward transitions at 30 FPS. The corrected 60 FPS probe boots in Cemu and reports unavailable GPU endpoints there. |
| Village streaming | At least 95% eligible terrain time on the original route and rules. | RC 33 measured 62.34%. Separate RC 33 stationary control was 99.64% eligible at fixed 100% and 562 sections per output. |
| Rendering and splitscreen | Improve the 32-pass matrix with dual-output checks. Document independent players, projections, HUDs, controllers and resource ownership, with independent-view measurements when implemented. | Shared world resources already serve separate TV and GamePad projections. Renderer gaps and independent player views remain open. |
| World and lifecycle integrity | Save/reopen, foreground and clean exit loops with readable saves, stable memory and no increasing retained allocations. | RC 38 restored the saved pose on reopen and returned the GX2 bulk heap to 234,880,920 free bytes after the first save and exit to Night. |

## Current work

- GX2 agent investigates flip cadence and resource ownership. The RC 38 capture
  shows that the combined GX2 flip timestamp clusters at three refreshes at
  75% and two at 50%. Next, qualify early TV submission and GPU workload cuts
  separately with matching geometry, scale and capture settings.
- The RC 39 source recipe keeps owner-approved refresh grants durable and
  migrates active legacy grants on refresh. The 13 host tests passed and
  the builder reconstructed the exact tree. The full workflow still needs
  the pinned tool audit, authenticated graphics and Night actions, and
  packaged Cemu and physical qualification.
- Loading agent is tracing the warm critical path through client preparation
  and integrated-server construction. Compare identical archives and cache states.
- The standalone winding fixture passed 32 RGB checks across both outputs
  in forced-interpreter Cemu. Qualify an opt-in solid terrain back-face
  path in a hosted world before moving on to axis-facing index ranges.
  [Winding fixture](../../wiiu/diagnostics/gx2-terrain-winding/README.md)
- The RC 40 package builds with the opt-in terrain retirement path and
  prepared server resource reuse. Night passed the independent Cemu
  display checks. Both saves and the reopen completed. The movement check
  is still open after a terrain-gated replay. [RC 40 package](rc40-validation.md) A subsequent source iteration scopes villager
  texture discovery and exposes authenticated live graphics controls.
- The Cemu x64 reservation fix passed carry, operand and allocator probes
  in both JIT modes. Exact RC 38 opened one world, played audio and saved
  in multi-core JIT. Warm reopen still panicked, and the scratch emulator
  duplicated the GamePad capture. Use the established forced interpreter
  for sustained replay and independent pixels. [Cemu record](../../wiiu/cemu/RECOMPILER.md)
- RC 41 adds fixed-scale graphics control through authenticated MCP,
  narrower villager texture discovery, a stock Fusion shortcut and the
  opt-in solid back-face trial. Its verified package passed the Night
  smoke. The `4b` saved-world replay has a terrain gate and a verified
  backward GamePad binding. Its fixture copy ran out of space before
  Cemu launched. [RC 41 package](rc41-validation.md)
- RC 42 is on the [qualification branch](https://github.com/megabytesme/MCHorizonWiiU/tree/codex/rc42-qualification).
  Its exact source includes benchmark control and report access, axis-facing
  terrain ranges, Village trace sequencing, the Cafe SRV adapter and a
  scratch Cemu GamePad MCP capture. Native packaging needs more free
  space on the desktop's system drive. The saved-world fixture itself
  needs about 860 MiB. The runner now checks this before copying.
- Real Codex host sessions verified read-only and all four scopes through
  pairing, independent restart and token refresh. Hosted graphics,
  benchmarks and complete restart and revocation evidence remain open.

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
