# Wii U gameplay performance scorecard

## RC 29 physical reference

The console ran client `0.8.0-rc.29`, port commit `c231d779772e` and
engine tree `53dae28084460c2448e22714f6962d8bfb1fa3d8`. The archived
qualification instance used the correctly bound prepared resources. The
[hardware acceptance record](rc29-validation.md) identifies the RPX,
fixed target sizes, lifecycle and original Village report.

The four columns use 15 contiguous live windows per configuration. The
75% and 100% series have the same observed terrain counts and render and
simulation distances of 6/5. The 50% reopen and legacy-uniform series were
separate camera windows. Game-step values are weighted by the iteration
count. Pacing in this table gives the median of the short-window statistics,
plus the maximum observed gap. All recorded submissions were partial with
omission mask `0x8000`.

| Physical measurement | 75% Full Performance | 100% fixed | 50% reopen | 50% legacy uniforms |
| --- | ---: | ---: | ---: | ---: |
| Serial window end timestamps, UTC | 13:45:11–13:45:40 | 13:47:06–13:47:35 | 15:11:30–15:11:59 | 16:27:18–16:27:47 |
| Total sampled window, s | 30.604 | 30.974 | 30.687 | 30.929 |
| Paired submissions, partial / complete | 244 / 0 | 238 / 0 | 300 / 0 | 308 / 0 |
| Paired submissions/s | 7.97 | 7.68 | 9.78 | 9.96 |
| Window elapsed per paired submission, ms | 125.425 | 130.142 | 102.291 | 100.420 |
| Timed game step, ms | 125.048 | 129.763 | 101.923 | 100.049 |
| Idle handler wall time, ms | 0.107 | 0.097 | 0.108 | 0.104 |
| Artwork preparation, ms | 38.760 | 39.098 | 38.525 | 40.454 |
| Redraw outside native submission, ms | 22.267 | 21.542 | 20.238 | 19.266 |
| Native submission outside measured waits, ms | 60.174 | 65.721 | 39.745 | 37.205 |
| Explicit GX2 waits, ms | 3.727 | 3.297 | 3.298 | 3.014 |
| Other game-step work, ms | 0.012 | 0.010 | 0.010 | 0.010 |
| Outside timed game step, ms | 0.379 | 0.379 | 0.368 | 0.371 |
| Median short-window p50 / p95 / p99, ms | 126.09 / 135.05 / 135.05 | 130.32 / 138.15 / 138.15 | 102.85 / 111.24 / 111.24 | 98.99 / 104.05 / 107.75 |
| Largest observed gap, ms | 150.62 | 152.74 | 142.87 | 127.53 |
| Server ticks/s | 20.00 | 20.02 | 19.98 | 20.01 |
| Terrain sections per display at each live window boundary | 294 | 294 | 294 | 275 |
| Meshed / uploaded sections | 24 / 24 | 22 / 20 | 8 / 8 | 12 / 12 |
| Terrain upload bytes | 3,071,424 | 2,257,856 | 1,452,224 | 1,234,368 |
| Valid endpoint pairs within guest-clock bounds | 15 | 15 | 16 | 16 |
| Median TV / GamePad GPU callback spans, ms | 42.59 / 18.45 | 46.34 / 20.36 | 26.95 / 13.21 | 24.98 / 12.39 |
| Median inter-output GPU gap, ms | 1.31 | 1.37 | 1.36 | 1.29 |
| Median TV start to GamePad finish, ms | 62.36 | 67.98 | 41.55 | 38.65 |

The game-step spans count elapsed time on the render thread. They can
include scheduler preemption and synchronous driver work. The idle bucket
times the application's idle handler. `GX2DrawDone` in the RC 29 HUD
path ran inside native submission, outside the explicit-wait bucket.
Worker completion wall time and GPU timestamps can overlap the game step.
The endpoint differences match the corresponding logged TV and GamePad
spans at 27 MHz. For each endpoint pair, the sum of those two spans and
the inter-output gap equals the combined bracket. Per-column medians are
computed independently. The bracket includes inter-output work and may
include GPU starvation, inline waits and CPU submission latency.

Only the retained HZP3 rows can supply pooled p50/p95/p99, geometry volume
and per-pass timings. The 100% HZP3 sample came later in the same run at
the same section count. The 50% near-window series is bounded by the
first and last correlated endpoint sequences. The serial comparisons at
75% and 50% legacy fall outside their respective retained HZP3 ranges.

| HZP3 measurement, medians unless stated | Later 100% fixed | 50% reopen |
| --- | ---: | ---: |
| First–last retained sequence | 26136–26379 | 191118–191418 |
| Paired partial submissions | 244 | 301 |
| Pacing p50 / p95 / p99 / max, ms | 131.05 / 138.88 / 147.78 / 163.13 | 102.75 / 110.26 / 120.07 / 142.85 |
| Texture upload nested in artwork, ms | 40.170 | 39.935 |
| Streaming / terrain upload, ms | 0 / 0 | 0 / 0 |
| Solid terrain TV / GamePad, ms | 1.706 / 1.213 | 1.772 / 1.200 |
| Translucent terrain TV / GamePad, ms | 0.973 / 0.881 | 0.988 / 0.884 |
| HUD TV / GamePad, ms | 42.743 / 17.795 | 23.418 / 10.686 |
| World target setup TV / GamePad, ms | 0 / 0 | 0.043 / 0.024 |
| World composite TV / GamePad, ms | 0 / 0 | 0.007 / 0.003 |
| Sections tested TV / GamePad | 1,335 / 1,335 | 1,335 / 1,335 |
| Combined recorded section draws / indices | 886 / 1,869,564 | 886 / 1,873,716 |
| Rust live / peak requested bytes | 337,439,031 / 373,359,924 | 357,233,074 / 390,845,575 |

Upload medians of zero reflect sparse uploads. The live windows retain the
totals. Full recovery after Save & Quit was 234,880,920 bytes free in the
GX2 bulk heap, with a 234,880,896-byte largest aligned block.

### Bottlenecks and experiments

1. **HUD synchronization and submission.** At 100%, the TV and GamePad HUD
   spans total about 61 ms when adding the two representative medians.
   `hud.c` calls `GX2DrawDone` before updating HUD backing per output,
   and the existing WHB finish also waits for the full pair. The HUD span
   varies with resolution because it can include queued world work. Prepare
   shared HUD data before either display submits, then use the existing
   retirement boundary before reusing backing. Compare the old placement
   and the shared path in one binary, holding camera, geometry and scale
   fixed. Verify that all atlas and vertex consumers have retired before
   the next write. GPU execution continues to cost time after the drains
   move, so the full measured span is an upper bound on removable wall time.
2. **Artwork.** At 75%, artwork consumes 38.76 ms, or 30.9% of the interval.
   The retained 50% HZP3 sample places nearly all of its artwork time
   inside the texture stage. `Gx2LoadingPresenter::prepare_game_artwork`
   advances the shared terrain atlas and uploads dirty cells. Preserve
   stock frame selection, interpolation, padding, pixels and both displays
   while reducing nearest-neighbor expansion work. Compare upload bytes and
   callback time on identical prepared inputs. Even removing the entire
   38.76 ms bucket would leave 86.67 ms per pair, about 11.5 pairs/s.
3. **Redraw outside submission.** At stock 75%, `Game::render_to` and other
   redraw work outside the presenter cost 22.27 ms (17.8%). Profile the
   shared frame construction and repeated geometry on a fixed camera
   before changing cache invalidation across pack, dimension and viewport
   changes. A 50% reduction in this bucket alone would save about 11 ms.
4. **Terrain commands.** The retained solid and translucent call spans add
   to about 4.8 ms at 100%. This makes raw command construction a smaller
   target than HUD retirement or artwork. The 75% scale reduces world
   target area by about 44% and the callback bracket by about 8%. Paired
   cadence improves by about 4%. The observed workload has both
   resolution-dependent GPU cost and substantial resolution-independent
   frame work. GPU active occupancy remains unmeasured.

### Physical display-list diagnostic

Each arm ran eight repeated frames at 64 draws and 384 indices per output.
Two arms exercised each method. All 96 per-output samples retired, and no
sample auto-submitted during the timed build. The photographed grid matched
the expected pattern on both displays and the diagnostic returned to the
Wii U Menu.

| Path | Repeated CPU build TV / GamePad, µs | Flush TV / GamePad, µs | Retirement wait TV / GamePad, µs | One-time construction |
| --- | ---: | ---: | ---: | --- |
| Immediate | 19.85 / 18.34 | 3.12 / 3.17 | 607.82 / 138.07 | None |
| Reusable section lists, live uniforms | 12.91 / 11.41 | 3.15 / 3.31 | 607.83 / 138.16 | 111 µs for all 64 lists |
| Fixed frame list per output | 0.13 / 0.14 | 3.16 / 3.18 | 607.64 / 138.18 | 43 µs for both lists |

The recorder reported 8,192 bytes of section-list payload, 12,032 bytes
per fixed-output list and 66,048 bytes of reserved command-list memory.
Summing TV and GamePad CPU build ticks within each physical pair gives
medians of 38.186 µs direct, 24.334 µs with section lists, and 0.274 µs
with fixed frame lists. The one-time record cost breaks even on the ninth
whole pair for section lists and the second pair for fixed frame lists. The latter rely on immutable output-specific uniform and draw
commands. Real hosted terrain changes projection, residency and translucent
sort order. A simple scaling of the section-list saving to 294 draws per
display is about 64 µs per pair, roughly 0.05% of the stock interval. This
fixture supports a lower-risk focus on the measured synchronization and
artwork costs first.

### Original Village

The route measured 58 paired submissions in 12.146 seconds. Thirty showed
terrain, and 29 adjacent eligible intervals covered 6.834 seconds (56.3%).
The HZP3 range with the same 58 submissions and 30 terrain pairs is
130829–130886. Both outputs declined from 562 visible sections to zero.
The final frame still tested 392 resident sections per output, down from
1,363 at route start. Uploads continued, totaling 6.14 MB across the
28 no-terrain frames. Both displays fall off together while work continues.
The original route travels 192 blocks in 12 seconds at render distance 6.
The trace points to the view outrunning resident terrain. The contribution of server generation, mesh completion, upload admission
and culling of the remaining sections remains unresolved. A matched replay needs
route-position and stage queue evidence before changing streaming policy.
