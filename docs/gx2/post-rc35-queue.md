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

## Next supervised application run

Use the verified RC 47 package for the authenticated lifecycle and save
checks. Keep the RC 38 saved-camera reference for the fixed 75% workload.
The RC 47 build record gives the RPX, WUHB and pinned source identities.
[RC 47 qualification](rc47-qualification.md)

- Record at least two separate 60-second-or-longer settled HZP3 windows
  from the same 174-section pose at fixed 75%, DRS off, render 6 and
  simulation 5. Verify independent TV and GamePad terrain draws and depth
  work. Compare with the 98.593-second RC 38 reference.
  [Window procedure](benchmarks.md#physical-fixed-75-saved-view)
- Reuse the approved four-scope grant through Night, live settings, an
  isolated save and reopen, and a console restart. Use the same client
  profile through paired transitions. [Disposable-world runner](benchmarks.md#disposable-world-lifecycle)
- Run the checked standalone readback on both 720p and 1080p TV setups
  before enabling physical staged MCP screenshots. Confirm the readback
  fence and clean menu return on the serial capture. [Readback fixture](../../wiiu/diagnostics/gx2-explicit-readback/README.md)
- Keep the original Village and heavy stationary runs separate. Use the
  hosted DRS decision report alongside the 30/60 physical timing probes.
  [Benchmark definitions](benchmarks.md#stationary-render-diagnostic)
