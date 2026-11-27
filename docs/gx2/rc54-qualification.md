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
