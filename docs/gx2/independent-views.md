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
| 3. Two TV seats | Two separate rectangular TV viewports with distinct local players. Record the GamePad's role when it is active. | Check viewport bounds, camera aspect, independent HUD and depth, controller assignment and per-seat pacing. Compare the same scene in the native two-camera fixture. |
| 4. Four TV seats | Four independent TV quadrants in one scanout image. | Test nearby and distant seats, joins and disconnects, separate view coverage and GPU memory high-water. Check isolation when one player opens a menu or crosses a chunk boundary. |
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
two displays separately. A host terrain fixture already compares two
moving frustums and their translucent sorts with the existing Cafe
decision paths. The saved player, input and viewport paths still need
native integration. [Horizon and NX comparison](horizon-nx-comparison.md) ·
[Two-view terrain fixture](../../wiiu/diagnostics/gx2-split-terrain/README.md)

## Native viewport fixture

Use independent cameras with at least one identifying world feature in
each viewport. Use the native draw path with the same projection,
depth, clipping, texture and color rules as the ordinary renderer. At
1920 × 1080 the four 960 × 540 TV quadrants sum to 2,073,600 output pixels,
equal to the full TV image. The GamePad adds 409,920 output pixels at
854 × 480. Measure actual world-target dimensions after scaling, screen
insets and pitch alignment. A quadrant preserves its own depth and
visibility costs even when its output pixel count is one quarter.

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
