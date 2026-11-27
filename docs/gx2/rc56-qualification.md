# RC 56 loading cancellation and visibility diagnostics

| Input or output | Identity |
| --- | --- |
| Port build source | `eaa90d5199c9941bc3549b9bab2590a51189a6f4` |
| Engine tree | `e75e7cd66e1afa3af0e26e0974e592537e2eedb7` |
| Recipe SHA-256 | `b9cb5af122498fb210943331ec2351490a26a25f119af141305b542c9546fe3c` |
| Cafe Toolchain | `1.1.0` |
| RPX SHA-256 | `8ae70e01ae47f183abadfc4add17641cf71046fca9ebd3371458e7e68919533b` |
| WUHB SHA-256 | `8b267bc72b759c6ce0e70bf467a1e6fc2e13e6355c4b5d3f3b0dfc4d60c8418c` |
| Estimated Aroma code-heap headroom | 12,481,024 bytes |

Ben-Desktop assembled and built the exact source in
`/var/tmp/mchorizon-rc56-native.VbNE6Eit/out` with
`tools/build_cafe_game_isolated.sh --hosted-nightly` and the pinned source
and input caches. Builder output verification and independent file hashes
passed. The 26 builder and 49 GX2 host tool tests passed.

## Terrain visibility observation

Plain profile `4` with `gx2-cave-visibility.enable` samples a bounded
asynchronous cave graph once per 120 eligible pairs after a stationary eye.
The graph uses at most 512 columns and reports solid and translucent
candidate, eligible and unknown counts separately for TV and GamePad.
The observation leaves geometry, residency and stock draw counts intact.
Run it separately from primary pacing.

The exact graph rejected a directly visible stone face in a two-block-high
air corridor from `(8.5, 72.5, 8.5)` to `(20.5, 72.5, 104)` even
with fresh topology and a bounded, known world. The distant-ray heuristic
traced from a section corner through a different path than the optical ray.
Keep this implementation observational.

Disabling that heuristic in an independent host experiment retained the
visible corridor at Z56, Z104 and Z135, rejected the blocked control, and
invalidated the witness when a former barrier became unknown. For each
display's projected frustum, an independent voxel traversal confirmed 74
clear lines out of 533 surface candidates and 85 out of 679 cave candidates.
The connectivity-only graph rejected none of those clear lines.

| Saved-region proxy | Stock to connectivity-eligible solid | Stock to connectivity-eligible translucent |
| --- | ---: | ---: |
| Surface | 234 to 128 | 12 to 5 |
| Connected cave | 246 to 141 | 13 to 2 |

The proxy uses saved raw terrain and prepared stock block facts with
approximate shell geometry. The candidate reductions need native proof
that the blocking faces are present, depth-writing and usable after upload
or eviction. An active trial also needs finite contained target geometry
and an optical camera origin consistent with bob, spin and clipping.

## Loading and Cemu

The optional warm mob seed stops before cold preparation if the load was
cancelled while checking its bounded input. Completed and cancelled seed
selection remain covered by focused host checks.

The fixed-field Cemu probes track the stock farmland base through enum
and struct construction, immediate insertion, the completed map, the
constructor and the borrowed lookup. A valid stock value and a deliberate
empty value produced the expected first-difference position in the
reduced host control.

The exact RC 56 package passed one matched disposable world replay in
each Cemu recompiler mode. Both runs drew TV and GamePad independently,
saved movement, drained queued audio and returned to Night with
234,880,920 GX2 bulk bytes free and a largest free block of 234,880,896 bytes.

| Observation | Single-core compiler | Multicore compiler |
| --- | ---: | ---: |
| Game returned | 153.335 s | 71.051 s |
| First paired terrain gate | 167.229 s | 73.797 s |
| Gameplay pairs in the session profile | 28 | 63 |
| Pairs without a partial flag | 0 | 1 |
| Maximum sections per output | 7 | 9 |
| Durable backward movement | 8.623 blocks | 8.784 blocks |
| Audio buffers written and read | 31,541 | 14,328 |

The failure-only trace did not fire in either run. The earlier intermittent
zero-length lookup remains open. These sessions used a small terrain gate
and serve as startup and lifecycle checks.

Session HZP3 SHA-256, single then multicore:
`f7115b8a2bd142d9a5aa14ead5337d622a9235f7df7f2430fd32522bc4b604a0` and
`e7790aa32074ad63803ede356b7dff2c5eb5f4a2719677a678a60716946cb004`.
