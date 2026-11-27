# Independent GX2 views and local splitscreen

The primary single-player target is sustained 60 FPS-equivalent native Wii U
gameplay in repeated physical windows of at least 60 seconds. Stock Full
Performance uses fixed 75% world resolution, DRS off, render distance 6,
simulation distance 5 and the saved 174-section camera. TV and GamePad
each render an independent world view with that display's projection,
depth, composition and HUD. Record the package and active submission
path with each capture. Report mirrored final-world color separately.
[Saved-view procedure](benchmarks.md#physical-fixed-75-saved-view)

## Local player milestones

| Milestone | Layout and ownership | Evidence to keep |
| --- | --- | --- |
| 1. Independent single player | One player and two separately rendered views. | Repeat the stock physical reference at sustained 60 FPS-equivalent with matching quality, terrain, pacing, server ticks and memory. Keep wider workloads separate. |
| 2. Two local views | One player on TV and a second on GamePad. Each joins one integrated world with its own camera, controls, inventory and HUD. | Move each player independently, exercise focused inputs and UI, save and reopen both player records, and measure nearby and distant sessions. |
| 3. Two TV seats | Two separate TV viewports with distinct local players. Use an independently rendered companion GamePad view in the three-view reference. | Check viewport bounds, camera aspect, independent HUD and depth, controller assignment and per-seat pacing. Compare the same scene in the native multi-camera fixture. |
| 4. Four TV seats | Four independent TV quadrants and an independently rendered companion GamePad view. | Test nearby and distant seats, joins and disconnects, separate view coverage and GPU memory high-water. Check isolation when one player opens a menu or crosses a chunk boundary. |
| 5. Four TV seats and one GamePad seat | Five independent local players in one world. | Exercise all five cameras, controls, inventories and HUDs simultaneously. Save and reopen players and terrain. Measure paired display cadence, per-view work, server TPS and memory recovery. |

Thirty FPS is an intermediate engineering milestone. Continue toward 60
and record higher achieved throughput where the workload and display
permit it. A multi-camera fixture qualifies renderer work. Full player
support also requires the gameplay and lifecycle checks in the table.

## Shared resources and per-view work

- Key immutable terrain by dimension, section, resource generation and
  mesh revision. Share model data, atlas backing and animation preparation
  across views. Stream the union of player tickets. Charge active and
  retiring mesh generations to one budget.
- Give each seat a viewport, camera and camera uniforms, HUD, first-person
  state and input slot. Use reusable per-view cull results and draw plans
  when their generations match. Keep separate sorted translucent indices
  for overlapping commands. Route GamePad touch and software keyboard to
  a single UI owner while keeping other controllers independently mapped.
- Separate CPU frame preparation from GX2 submission. First qualify
  bounded mutable staging and timestamp-based reuse through foreground
  release, swap timeout, screenshots, pack reload, world close and recovery.
  Then measure overlap and move scan publication only when the ownership
  and scan-buffer sequence can be validated. Pin render submission to its
  supported core and schedule independent preparation work with measured
  server and mesh-worker headroom. [GX2 event interface](https://github.com/devkitPro/wut/blob/v1.9.2/include/gx2/event.h) · [WHB presentation](https://github.com/devkitPro/wut/blob/v1.9.2/libraries/libwhb/src/gfx.c) · [Cafe thread affinity](https://github.com/devkitPro/wut/blob/v1.9.2/include/coreinit/thread.h)
- Measure geometry submission, axis-facing rejection, occlusion, greedy
  meshing and compatible water merges with original material, light, fog
  and sort behavior. Compare overdraw, shader work, native vertex and
  texture access, allocation contention and multicore preparation against
  the same render workload.

The current Cafe presenter accepts one shared-game frame and renders the
two displays separately. A host terrain fixture compares up to five
frustums and their translucent sorts with the existing Cafe decision
paths. Nearby poses reuse a resident mesh, while distant poses require
separate resident meshes. A separate native viewport fixture passes in
Cemu. Hosted integration, player input and physical qualification remain
open. [Horizon and NX comparison](horizon-nx-comparison.md) ·
[Independent-view terrain fixture](../../wiiu/diagnostics/gx2-split-terrain/README.md)

## Horizon and NX source audit

The pinned Horizon runtime owns a host `Game`, boxed guest clients,
per-seat controller assignments and a shared presentation schedule. The
layout admits sixteen logical seats. Guests have their own profiles,
input edges, UI state and client world view. Their memory connections
join the same integrated server, which owns the save barrier and player
records. In the desktop implementation, each guest still creates a
renderer and loads `UiResources`. Cafe needs an independent view packet
per seat and shared immutable backing. [Pinned split runtime](https://github.com/arismoko/horizon/blob/0a875da91dbb0fdcb7ba8c42c11341aa229af51a/crates/horizon-client/src/game/runtime/split/mod.rs) · [Pinned layout](https://github.com/arismoko/horizon/blob/0a875da91dbb0fdcb7ba8c42c11341aa229af51a/crates/horizon-client-render/src/split_layout.rs) · [Pinned guest setup](https://github.com/arismoko/horizon/blob/0a875da91dbb0fdcb7ba8c42c11341aa229af51a/crates/horizon-client/src/game/runtime/split/players.rs#L450)

The layout, player-join and pad-router files match the pinned Wii U
upstream in KoHaku's NX revision and in the audited public Horizon fork
head `de8b4e2526df9d7f611515b78123a26a3914176f`. The current official
upstream was inaccessible during the comparison. NX adds a system
controller dialog with four assignments and a split handheld GPU clock
policy. Those platform choices establish neither seat capacity on Cafe
nor Wii U frame rates. [Audited public head](https://github.com/arismoko/horizon/commit/de8b4e2526df9d7f611515b78123a26a3914176f) · [NX controller dialog](https://github.com/KoHaku743/horizon-nx/blob/de477953c3958551a60f011fa447a99229f9fc43/crates/horizon-nx/src/controllers.rs#L44) · [NX clock policy](https://github.com/KoHaku743/horizon-nx/blob/de477953c3958551a60f011fa447a99229f9fc43/crates/horizon-nx/src/clocks.rs#L75)

RC 47 already encodes overlapping level columns once per server batch.
The decoded-section sharing hint is coordinate-keyed and consumed by
one client. The bulk-edit LOD hint also has one drain. A multi-seat path
must check session, dimension and generation and deliver updates to each
interested client before claiming duplicate decode or mesh work removed.
Cafe currently reads one VPAD channel. The four TV seats need separate
KPAD controller assignments, disconnect state and neutral rearming.
[Native KPAD interface](https://github.com/devkitPro/wut/blob/v1.9.2/include/padscore/kpad.h)

## Native viewport fixture

Use independent cameras with at least one identifying world feature in
each viewport. Use the native draw path with the same projection,
depth, clipping, texture and color rules as the ordinary renderer. At
1920 × 1080 the four 960 × 540 TV quadrants sum to 2,073,600 output pixels,
equal to the full TV image. The GamePad adds 409,920 output pixels at
854 × 480. Measure actual world-target dimensions after scaling, screen
insets and pitch alignment. A quadrant preserves its own depth and
visibility costs even when its output pixel count is one quarter.

The RC 38 fixed-75% targets total 1,395,720 world pixels. Tiling the
1440 × 809 TV lattice into 404- and 405-row quadrants preserves that
total with the 641 × 360 GamePad target. In a calibrated fixture with
255 section-pass draws per view, five views would submit 1,275 draws
and 2,633,370 indices. The estimated workload uses the aggregate RC 38
submission record. Record actual per-view work and pitch-aligned bytes
when the fixture runs. [Physical reference](rc38-validation.md#physical-fixed-75-gameplay)

The native Cemu fixture covers 1+1, 2+1 and 4+1 view layouts and two poses
in both residency modes. It submits 870 opaque indexed draws and
2,076,180 indices at its five-camera layout. One 174-section site uses
7,751,104 aligned bytes, and five disjoint sites use 38,755,520. Each
case checks separate projection, depth, seam ownership and HUD samples.
A terrain feature exclusive to the distant GamePad site distinguishes
its output from the three tested TV crops. [Viewport fixture and exact package hashes](../../wiiu/diagnostics/gx2-independent-views/README.md)

- For nearby cameras, overlap streaming ticket regions and use distinct
  headings. Include shared terrain visible to several views and nearby
  geometry visible to only one view. Compare the sum of each view's
  section and index submissions with the unique resident mesh count.
- For distant cameras, separate the ticket centers until their regions no
  longer overlap. Use comparable terrain density and the same distances.
  Report the union of loaded columns, live and retiring vertex/index
  bytes, upload pressure and missed terrain in each viewport.
- Compare stand-alone references for each camera against the same frame
  in the multi-view fixture. Check viewport edges, letterboxing, depth,
  cutout, overlapping water and glass, and HUD isolation. Record pixels
  outside timing windows. Use separate controls for static and moving
  cameras and preserve stock terrain and materials in both.
- Capture frame pacing, combined flips and available independent scanout
  evidence, CPU work and waits, GPU completion boundaries, sections,
  indices, integrated-server TPS, live and high-water memory, and
  post-close heap recovery. Keep independent render results, optional
  mirrored results and full gameplay acceptance individually identified.
