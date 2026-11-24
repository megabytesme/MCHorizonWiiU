# GX2 GPU timing qualification

Build this standalone diagnostic with the pinned Cafe toolchain and CafeGLSL.
It requires no prepared game resources and makes no changes to Horizon's
launcher, application data or renderer. The build writes an RPX and WUHB.

```sh
bash wiiu/diagnostics/gx2-gpu-timing/build.sh "$scratch/gx2-gpu-timing"
```

## Why this needs a hardware run

WUT 1.9.2 includes both cycle-sample imports in `libwut.a`. Its public GX2
headers declare command retirement and presentation calls and omit the cycle
API prototypes. Cemu and SwitchWakerHD independently use a 64-bit destination
pointer in PowerPC register r3. Cemu immediately writes CPU time for the top
sample and deliberately writes an invalid value for the bottom sample. Both
projects' sample implementations are emulator behavior and cannot establish
console execution times. This fixture calls the imports in isolation and tests
observable ordering and clock behavior before any renderer integration.
[WUT imports](https://github.com/devkitPro/wut/blob/v1.9.2/cafe/gx2.def) ·
[WUT event header](https://github.com/devkitPro/wut/blob/v1.9.2/include/gx2/event.h) ·
[Cemu implementation](https://github.com/cemu-project/Cemu/blob/9cac87595d8f4d038f36de35457298796991a05e/src/Cafe/OS/libs/gx2/GX2.cpp) ·
[SwitchWakerHD implementation](https://github.com/centollOS/SwitchWakerHD/blob/925fbf4fe79da79cc97a62f21da5a5b74f05822a/runtime/src/gx2/gx2_core.cpp)

The independently declared `GX2GetLastSubmittedTimeStamp` and
`GX2GetRetiredTimeStamp` APIs identify command submission and retirement.
Their difference is not elapsed GPU time. Use the retirement watermark to
guard reads from permanent sample storage. The probe also reports wall time
spent polling the watermark and waiting on the selected submission.
[WUT event header](https://github.com/devkitPro/wut/blob/v1.9.2/include/gx2/event.h) ·
[Cemu retirement implementation](https://github.com/cemu-project/Cemu/blob/main/src/Cafe/OS/libs/gx2/GX2_Command.cpp)

## Run and interpret

In forced-interpreter Cemu, launch the RPX in a disposable portable directory
and retain the guest log. Expect both raster readbacks to pass and cycle
samples to report `unavailable`. This verifies the workload, data plumbing
and rejection of the emulator's sentinel.

The initial pinned Cafe build reproduced byte-for-byte in two build
directories. In forced-interpreter Cemu, the 480×270 raster check read red
203 and 0 at the corner and center. At 1920×1080, both reads were 203. All
nine bottom samples were all-ones and had already changed before an explicit
flush, with no automatic submission. The timing source was reported as
unavailable and the fixture exited successfully.

For a console run, use a separate application directory such as
`sd:/wiiu/apps/HorizonGX2Timing/` with `HorizonGX2Timing.wuhb`. Start serial
capture before launching it. It runs automatically, briefly presents a teal
frame and returns to Aroma. No Horizon settings or worlds are accessed.
Keep the full raw log from `GX2 timing raster` through `GX2 TIMING FIXTURE`.

The same pipeline draws 32 blended triangles at full and one-quarter
viewport width and height, in ABBA order twice. This changes fragment
coverage sixteenfold while keeping draw and vertex counts fixed. The
zero-draw batch estimates marker overhead. The probe reads colors outside
the measured batches, runs two drained clock-calibration pairs over 10 and
40 ms CPU sleeps, and samples one independent WHB frame for presentation
cost. The normal game does not need these probe-only drains.

| Log record | Meaning |
| --- | --- |
| `GX2 timing <trial>` | CPU wall microseconds to build the commands, flush the batch, and poll and wait for retirement. `pending=1` means the target was still in flight immediately before the wait. |
| `GX2 cycles` | Raw 64-bit hexadecimal top and bottom values read after retirement and CPU cache invalidation. Zero, all-ones, unchanged poison, and reversed pairs are rejected. |
| `GX2 span` | The 64-bit hexadecimal difference between valid top and bottom values. `early` shows whether either destination changed before the deliberate flush. `auto=1` means command construction had already advanced the submission watermark. |
| `GX2 timing calibration` | Host elapsed microseconds and the GPU counter increase between drained bottom samples. The increase is hexadecimal. Use the two intervals to estimate and cross-check the clock rate. |
| `GX2 timing present` | CPU wall time in WHB begin, clear/copy and finish, plus combined swap and flip counts. |

`GX2 TIMING FIXTURE PASSED` means the workload and retirement path worked.
Qualify the cycle source only if the console repeatedly shows writes after
submission, ordered non-sentinel values, consistent calibration, and an
increase for the high-coverage trials without a similar change in CPU build
time. A qualified span would measure elapsed time between two GPU pipeline
positions and may include pipeline overlap or idle. Test empty and mixed
frames, foreground transitions and asynchronous result handling before using
the source for DRS. Keep any controller disabled for unsupported clocks and
emulator stubs.
