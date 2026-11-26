# Post-RC 35 engineering queue

Use canonical `main`, the locked Nightly source recipe and an isolated
portable Cemu profile. The existing test SD retains RC 35 until a
supervised transfer. [Verified RC 35 fallback](rc35-validation.md)

| Workstream | Next verification |
| --- | --- |
| GX2 fixed-scale gameplay | Run the qualified stationary benchmark at 100% and the RC 35 saved pose at 75%. Match camera, distances, phase, effective scale and display coverage. Compare `4` with `4m` for model sharing, `4w` with `4u` for atlas writeback, and `4` with `4q` for the deferred TV flip wait. Compare `4` with `4c` at fixed 75% with both outputs fully composited. Use 100% as a control for `4c`. Record wall intervals, wait attribution, both display timings, draw counts and bulk heap recovery. |
| Automatic Dynamic Resolution | Run the existing standalone DRS60 WUHB on the Wii U. Its three live phases target 50→60, 60→50 and 50→60. Follow with production target allocation checks, including a real downward transition and mixed CPU/GPU pressure. |
| Loading | Record an entry with fresh preparation caches and a same-candidate reopen of the identical fixture. Join world-ready, first paired terrain and settled markers without adding overlapping stage durations. The candidate's first entry may rebuild versioned caches. |
| Village streaming | Keep the documented route and original acceptance rules. Compare coverage and longest consecutive ineligible run, then use the forward-streaming counters to attribute misses to accepted mesh jobs, pending uploads or resident terrain. |
| Online mode | Repeat the controlled online-mode join, disconnect and reconnect on the Wii U using the existing Night profile and an explicit host and port. Capture both outputs and confirm that the session server accepted the profile. |
| Renderer coverage | Qualify entity hurt, TNT and creeper overlays in a live scene. Continue native mipmaps, clouds and world text with focused dual-output checks, including depth, material blending and resource-pack reload. |
| Build and release | Verify source trees, the exact RPX embedded in WUHB, native relocations and Aroma headroom. Record exact-package Cemu results and independent package comparison. Preserve the RC 35 fallback and keep development status until hardware acceptance. |

[Stationary and Village workloads](benchmarks.md) ·
[Gameplay scorecard](gameplay-scorecard.md) ·
[Loading scorecard](loading-scorecard.json) ·
[Online-mode record](online-mode.md)
