# Post-RC 35 engineering queue

Use canonical `main`, the locked Nightly source recipe and an isolated
portable Cemu profile. The last SD receipt records RC 38 and the corrected
DRS60 probe, then a safe eject. Verify this state before the next console
session. [RC 38 package](rc38-validation.md) ·
[Verified RC 35 fallback](rc35-validation.md)

| Workstream | Next verification |
| --- | --- |
| GX2 fixed-scale gameplay | Run the qualified stationary benchmark at 100% and the RC 35 saved pose at 75%. Match camera, distances, phase, effective scale and display coverage. Compare `4` with `4m` for model sharing, `4w` with `4u` for atlas writeback, and `4` with `4q` for the deferred TV flip wait. Exercise Home and foreground reacquisition in `4q`. Compare `4` with `4c` at fixed 75% and verify the omission mask. Expect GamePad only on the standard inset TV viewport. Use 100% as a control for `4c`. Record wall intervals, wait attribution, both display timings, draw counts and bulk heap recovery. |
| Automatic Dynamic Resolution | Run the existing standalone DRS60 WUHB on the Wii U. Its three live phases target 50→60, 60→50 and 50→60. Follow with production target allocation checks, including a real downward transition and mixed CPU/GPU pressure. |
| Loading | Record an entry with fresh preparation caches and a same-candidate reopen of the identical fixture. Join world-ready, first paired terrain and settled markers without adding overlapping stage durations. The candidate's first entry may rebuild versioned caches. |
| Village streaming | Keep the documented route and original acceptance rules. Compare coverage and longest consecutive ineligible run, then use the forward-streaming counters to attribute misses to accepted mesh jobs, pending uploads or resident terrain. |
| Online mode | Repeat the controlled online-mode join, disconnect and reconnect on the Wii U using the existing Night profile and an explicit host and port. Capture both outputs and confirm that the session server accepted the profile. |
| Renderer coverage | Qualify entity hurt, TNT and creeper overlays in a live scene. Continue native mipmaps, clouds and world text with focused dual-output checks, including depth, material blending and resource-pack reload. |
| On-demand captures | Run the verified stage-only package with ProcUI handoff and serial checkpoints. Verify explicit staging for both outputs at 720p and 1080p TV settings. Use HDMI stills for current physical comparisons. [Package and Cemu comparison](../../wiiu/diagnostics/gx2-explicit-readback/README.md) |
| Build and release | Verify source trees, the exact RPX embedded in WUHB, native relocations and Aroma headroom. Record exact-package Cemu results and independent package comparison. Preserve the RC 35 fallback and keep development status until hardware acceptance. |

[Stationary and Village workloads](benchmarks.md) ·
[Gameplay scorecard](gameplay-scorecard.md) ·
[Loading scorecard](loading-scorecard.json) ·
[Online-mode record](online-mode.md)

The corrected DRS60 WUHB has SHA-256
`a86766fa02bb505f24873c502d54e672726932324b3d489dd5349343aef91139`
and embeds RPX SHA-256
`1c596cf9a5d4fb8fa8f64bc2c549a1bea010e1e0df4be4320911b8e5424016a3`.
Capture its light, heavy and light transitions. The original package
(`512ecdfb30a52c29d5164e23547426ce9331532fc52b1205f70be9b2f3d480e2`)
failed with error 160-2203 before entry and is retired. [Build and hardware procedure](../../wiiu/diagnostics/gx2-drs60/README.md)

## Physical qualification order

Focus first on the independent fixed-75% single-player reference.
Keep the current RC 38 saved pose and capture settings. The consolidated
RC 55 package stages matched stock and renderer controls. RC 57 stages
the grouped early-submission trial on one verified binary. Record
the marker files for each arm. [RC 55 package](rc55-qualification.md) ·
[RC 57 package](rc57-qualification.md)

RC 60 supersedes the grouped HUD arms that failed arena destruction in
Cemu. The corrected control and early-requested arms passed the full
hosted lifecycle. Begin the physical comparison on the corrected package.
Run the separate bounded TV depth arm after confirming actual MEM1
allocation and foreground release. RC 59 retains its stock-fog
comparison of plain rendering and the opaque no-kill shader. Stock
Overworld fog excludes the clear-fog diagnostic from the reference.
[RC 60 package](rc60-qualification.md) · [RC 59 package](rc59-qualification.md)

RC 61 confirmed the guarded GamePad entry-drain omission in Cemu. Its
same-build grouped control and trial take priority among the queued
renderer comparisons. The separate stock near-fog arm exercises the
shader on both native targets. Its Cemu run admitted about 2% of the
settled solid indices per display. [RC 61 package](rc61-qualification.md)

RC 62 stages a same-build signed atlas comparison and a separate passive
solid draw-list census. Keep the census out of pacing measurements.
Run it both at the saved pose and during a bounded traversal.
[RC 62 package](rc62-qualification.md)

The RC 63 prepared-frustum control remains available after the
presentation and geometry comparisons. Its two startup sampling phases
and automatic DRS recovery checks use separate runs.
[RC 63 package](rc63-qualification.md)

### Paired publication timing

The physical RC 38 long window averaged 50.064 ms per loop, with
5.027 ms waiting for the previous flip, 2.944 ms in terrain waits and
0.387 ms in the terminal finish. After the deadline diagnostic passes its
native and hosted checks, use a separate 174-section saved-view run with
plain `4` and `gx2-present-deadlines.enable`. For the independently tested
grouped path, add `gx2-grouped-retirement.enable` to sample after its
existing flush. Collect the HZP3 file and serial stream from the same run.

```sh
python3 -B tools/gx2_present_deadlines.py CAPTURE.hzp SERIAL.log \
  --first-sequence FIRST --last-sequence LAST --scale 75 --sections 174
```

The reducer reports independent refresh observations for an empty swap
queue before publication, the publication bracket with GPU work pending,
and an already retired pair still waiting for a flip. These sparse
observations describe the combined queue. Use each world's independent
draw counts and timings alongside them. Cemu's aliased refresh and flip
clocks can confirm record handling. A physical capture is needed to
attribute elapsed refreshes.

| Order | Comparison | Evidence |
| --- | --- | --- |
| 1 | RC 49 stock and the trial package with rendering switches absent | Same fixed-75% surfaces, 174 sections per output, native passes, save and heap recovery. Capture at least two settled windows of 60 seconds each. |
| 2 | Same trial package with bounded HUD lanes, then with lanes plus grouped presentation | Record actual per-output lane and fallback counts, paired submissions, flip timing, waits, peak allocations and visible output. Exercise Home and foreground return. |
| 3 | Same RC 60 grouped path with early submission marker absent and present | Match the two independent viewport sizes and saved view. Record paired cadence and combined flips and run the grouped queue observer separately. |
| 4 | Same stock path with the verified first-TV retirement skip forced slow and enabled | Verify ticket validity, fallback counts, comparable draw/coverage and paired wall time. |
| 5 | Cold and warm verified archive loading, original Village route, and 30/60 DRS probes | Record each result separately with the corresponding package and settings. |

Keep screenshots outside the timing windows. Preserve stock view distances,
materials and per-output world rendering in the reference. The viewport and
blend diagnostics remain available as separate correctness checks.

## Additional supervised checks

Keep the RC 38 saved-camera reference for the fixed 75% workload.
The earlier RC 48 lifecycle receipt remains part of the separate review.
[RC 48 qualification](rc48-qualification.md)

- Record at least two separate 60-second-or-longer settled HZP3 windows
  from the same 174-section pose at fixed 75%, DRS off, render 6 and
  simulation 5. Verify independent TV and GamePad terrain draws and depth
  work. Compare with the 98.593-second RC 38 reference.
  [Window procedure](benchmarks.md#physical-fixed-75-saved-view)
- Record an offline save and reopen, including the save barrier, return
  to Night and GX2 bulk heap recovery. Authentication and permission
  qualification remain pending separate review.
- Run the checked standalone readback on both 720p and 1080p TV setups
  before enabling physical staged MCP screenshots. Confirm the readback
  fence and clean menu return on the serial capture. [Readback fixture](../../wiiu/diagnostics/gx2-explicit-readback/README.md)
- Run the standalone independent-view fixture with the same 1080p and
  720p TV setups when practical. Record viewport and depth markers and
  the fault masks. Use the fixed-75% saved world for gameplay timing.
  [Standalone viewport fixture](../../wiiu/diagnostics/gx2-independent-views/README.md)
- Keep the original Village and heavy stationary runs separate. Use the
  hosted DRS decision report alongside the 30/60 physical timing probes.
  [Benchmark definitions](benchmarks.md#stationary-render-diagnostic)
