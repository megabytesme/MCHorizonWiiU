# RC 54 stock screen effects and opaque shader trials

RC 54 adds independent scaled camera overlays, a clear-fog opaque shader,
and sparse host frame timings. It also borrows prepared model-state strings
and records fixed-field provenance for a failed farmland key lookup.

| Input or output | Identity |
| --- | --- |
| Port build source | `d951acc1af08ae2d28f072bdeb5b85cffbde85a2` |
| Engine tree | `784d41f558c29868b67620d0bd496aeda0fd2178` |
| Recipe SHA-256 | `ad3a4019bfc98f18350b74281fff477c366ef626e744ece37ca0b0bec8262ce3` |
| Cafe Toolchain | `1.1.0` |
| RPX SHA-256 | `ea9998df0b05ad7326620843e55441c86ee763f7d49d00a484682b09090e0c9d` |
| WUHB SHA-256 | `90d18707575cfa1d649edaa971d871d65426da25e3537b979986a7e74fd3535b` |
| Estimated Aroma code-heap headroom | 12,506,048 bytes |

Ben-Desktop built the exact recipe in
`/var/tmp/mchorizon-rc54-native.Yo38C2A0/out` and independently verified the
hashes and code-heap estimate. The targeted combined C build regenerated
both pinned experimental shaders and compiled the full native host archive.

## Controls

| Workload | `gx2-profile.enable` bytes | Additional marker |
| --- | --- | --- |
| Independent direct control | `4` | None |
| Scaled stock screen effects | `4` | `gx2-screen-overlays.enable` |
| Full-precision opaque control | `4o` | None |
| Full-precision opaque with proven clear fog | `4o` | `gx2-clear-fog.enable` |
| Sparse game-thread timings | `4f` | None |

For pacing, keep the [fixed 75% saved view](benchmarks.md#physical-fixed-75-saved-view).
Compare the two `4o` arms with each other. Clear fog accepts one contiguous
run of up to 32 opaque draws per output in the original order. The entire
geometry bounds must stay on the clear side of both fog ramps. The pinned pixel program
uses 64 dwords versus 112, with the same vertex binary, atlas sample,
sampled alpha and depth controls. Host geometry, state and compiled shader
checks passed. Native pixel and timing comparisons remain queued.

The separate stock underwater fixture uses two independent scaled outputs
and an identical camera in both arms. It measures absence or presence of
the actual water overlay. The test uses a disposable world and keeps the
stationary performance workload separate. The screen presenter borrows
the live atlas inside the guarded pair callback, retires it before release,
and declines unsupported, bootstrapping or isolated benchmark frames.
Strict C and host failure/foreground controls passed.

## Matched Cemu underwater replay

The exact RC 54 RPX ran both arms in the Cemu interpreter with the same
prepared archive, ocean fixture, options and graphics inputs. Both saved
the camera at `(46.5, 56.0, 148.5)`. This parity fixture disables the
vignette to isolate the stock water overlay and retains one stationary
terrain section per display. The 75% world targets measured 1440×809 and
641×360, with separate 1920×1080 TV and 854×480 GamePad captures.

| Observation | Control | Scaled screen effects |
| --- | ---: | ---: |
| Stable capture windows | 5 | 5 |
| Paired rows bracketed by those windows | 9 | 7 |
| Rows with just the screen-effect omission (`0x200`) | 9 | 0 |
| Rows reporting zero omissions | 0 | 7 |
| Rows with terrain uploads | 0 | 0 |

The live marker arm reported the scaled screen effects submitted on both
outputs. Of 1,344 stable pixel locations per display, 1,326 changed by
at least two channel values on each. The measured changes fit the
advisory stock-water blend envelope at 99.18% of stable TV locations
and 97.99% of stable GamePad locations. The envelope uses the stock
texture's channel and alpha bounds, a fitted brightness, and a two-level
rounding allowance. Hardware overlay parity, the other live overlay
modes and settled performance remain queued.

The control and trial acceptance receipts have SHA-256
`ba1065d2175b239b4b807eb67c29861e39f12fce4fe469681038f434d1623c09` and
`55623d7c903388e3dbaa7c51057e0307df32958e8bd0610ce3738748bce62d03`.

## Frame timing and loading

The `4f` sampler emits game-thread wall spans on every 120th eligible
gameplay frame. Join the same run's HZP3 capture and serial lines by sequence
and exact start ticks. Paired frames with reported partial passes are
retained. A missing output or missing group has separate accounting.

    python tools/gx2_frame_laps.py CAPTURE SERIAL

For UWP, add `--serial-capture --serial-first-seq FIRST --serial-last-seq LAST`.
The renderer and preparation spans contain nested work and waits. Interpret
worker times as completion-interval accounting. Keep sparse timing separate
from the unsampled pacing run.

Borrowed model-state input matched all 35,723 stock bindings. Host peak
allocation for parsing and binding fell from 33.18 to 23.81 MB. The
farmland trace records lengths before insertion and after a failed borrow
while preserving the original lookup and error.

## Matched Cemu recompiler replay

The exact package entered gameplay in 57.642 s in the multicore compiler.
The two displays drew paired terrain at 60.284 s. Backward input moved
8.526 blocks and persisted to the disposable save. Audio drained all
12,763 queued items and the GX2 bulk heap returned to 234,880,920 free
bytes. The session HZP3 SHA-256 is
`362d68d1407205fccee3618d48ff1f53a51b63cdb7159591acc1746f85ae3027`.
It records 98 paired gameplay rows with up to 34 sections per display.

The matched single-core start terminated during resource preparation.
The origin trace recorded the correct 14-byte `minecraft:dirt` string
before insertion. At the failing lookup, the origin state and selected map
entry were both the stock farmland definition. The owned string and its
borrowed view were both zero-length. The literal and the linear registry
check still resolved dirt state 10. This bounds the failure between
construction of the owned definition and its use through the map. Check
immediately after insertion and before interpretation of the completed map
to narrow that interval. Successful multicore startup and physical pacing
have separate acceptance requirements.

The additional multicore clear-fog trial requested `4o` with
`gx2-clear-fog.enable`. It stopped before Game handoff at the same
farmland state and recorded the same origin and lookup lengths. The
shader path was unreachable in that run. The receipt SHA-256 is
`89de557b490b2f05836613f27b7b38e176ae0981f0fead082c8ee36120b8db8b`.

A later runtime fixture reproduced a separate loader rejection. The
pinned clear pixel shader carries the same loop descriptor as the stock
opaque shader, while the RC 54 guard expected zero descriptors. Use a
package with the corrected guard and verify its ready message and
submitted-index counts before timing a clear-fog arm.
[Clear opaque shader correction](terrain-format-trials.md#clear-opaque-vertex-program)
