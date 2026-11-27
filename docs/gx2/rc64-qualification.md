# RC 64 boat water mask and stage attribution

| Input or output | Identity |
| --- | --- |
| Port build source | `32e2bd6659d7128f532ae068252d222eca95ef53` |
| Engine tree | `a22797fa38ddd43b1cd23f5435a43d718ae5f8f5` |
| Recipe SHA-256 | `5336350457f0b361b0b57308b5d42acf03b9b4b9e8347536fa2fdd5d558f4ece` |
| Cafe Toolchain | `1.1.0` |
| RPX SHA-256 | `08d257cad32c824b8f83bb98967937d51e72582d3b8221d723037121bfd4bdaf` |
| WUHB SHA-256 | `b18c28fdd8b1324c309b54a46a09b2f595b44adf8a3f1b96f593eff0a414551e` |
| Estimated Aroma code-heap headroom | 12,425,088 bytes |

The 32-job Ben-Desktop build used the pinned hosted Nightly recipe.
The initial archive write on `E:` failed. The verified source and
timestamped Cargo cache were copied to
`/dev/shm/mchorizon-rc64-recovery.j20ov0td/out` and the normal isolated
build completed there. The recovered Cargo step took 7 minutes
30 seconds. `verify-output` verified the packages in both the build and
durable output directories. All 26 builder tests and the strict
standalone Cafe C checks passed.

## Boat depth mask

GX2 draws the shared boat mask as a depth-only pass after ordinary
entities and before translucent terrain. The mask preserves the stock
camera projection and each view's depth attachment. A failed mask
allocation or draw reports the existing world-entity pass omission and
allows both outputs to finish.

The deterministic offline scene starts at `(-1.5, 64.0, 70.5)`, yaw
180°, pitch 26.75°. The control save differs only by the absence of one
oak boat. The Cemu replay used multicore mode, fixed 75%, Full Performance,
DRS off, render distance 6 and simulation distance 5. Both outputs were
captured separately at 1920×1080 and 854×480 before movement or save.

Preselected interior regions changed from 2,096 blue pixels to 2,096
wood pixels on TV and from 450 blue pixels to 450 wood pixels on GamePad.
The water beside the boat stayed blue in all 2,040 TV and 414 GamePad
control pixels. The existing visible hull stayed wood-colored in the
sampled regions. Structural checks confirmed the fixture boat survived
the save. With the boat absent, every sampled interior and surrounding
water pixel stayed blue. All three runs saved movement, played AX PCM,
returned the GX2 bulk heap to 234,880,920 free bytes and exited cleanly.
The RC 63 boat run reported the world-entity omission. Both RC 64 runs
cleared that warning in this scene.

| Cemu arm | Disposable run | First Game frame | Early sections per output |
| --- | --- | ---: | ---: |
| RC 63 boat | `boat-hosted-fcbcd6bc` | 54.509 s | 55 |
| RC 64 boat | `boat-hosted-d23b4083` | 53.616 s | 49 |
| RC 64 without boat | `boat-hosted-e2be2d83` | 54.570 s | 40 |

The first RC 64 no-boat replay joined three sound decoders and completed
the save and heap recovery. The lifecycle harness had assumed exactly two
decoder threads and stopped. It now accepts a positive join count and
records the observed value. The rerun completed with three threads.
Color and lifecycle receipt SHA-256
`80455873a188d9b29e7b539280d74768fada4693251b1e3cd349a7708bb8741c`.
This scene qualifies the boat mask in Cemu. The staged physical check and
the ordinary 174-section timing runs remain separate.

## Village and startup probes

The existing Village trace now records accepted worker meshes, native
upload and eviction outcomes, and admissions carried across rejected
presentations. Its raw-route gate preserves the original traversal and
coverage requirements. A host control verifies that a camera-less
presentation cannot discard pending eviction outcomes.

Use `worldgen-load-profile.enable` for a separate cold/warm trace of
generator and collision preparation. Use `gx2-server-pump-profile.enable`
for a separate measurement of integrated-server work between fixed
simulation ticks. Keep the load and pump markers out of the timed primary
saved-view run.

The fresh generator probe `r64-b053b8d4` reached Game in 54.451 s and
the first small paired terrain gate in 57.191 s in multicore Cemu. The
marker recorded 13.428 s for generator data, including overlapping
collision, support and property stages at 2.948, 7.139 and 7.294 s.
The core motion stage took 3.030 s and ended 2.695 s after the light
stage. The item model stage ended 2.454 s before block facts, which
bounds the potential overlap available to a motion preparation trial.
These stages overlap and their times should be read on one timeline.

The separate pump run `r64-d62abce7` saved successfully after the initial
15-second gameplay dwell. It yielded no complete pump sample window.
Use a longer settled saved-view run and keep the new measurements
separate from the primary timing comparison.
