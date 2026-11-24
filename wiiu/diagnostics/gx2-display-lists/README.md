# GX2 display-list submission probe

This standalone test compares three ways to submit 64 indexed draws with a
28-byte attribute stride and one contiguous 20-word vertex uniform update.
Each arm renders 64 sections and 384 indices per display.

| Mode | Work issued during the timed command build |
| --- | --- |
| Immediate | Write uniforms, bind the section's vertex address and issue the indexed draw for each section. |
| Section lists | Write the same live uniforms and call one immutable list containing the vertex address and indexed draw for each section. Reuse the lists on both displays. |
| Frame list | Call an already recorded list for each display. The list contains its uniforms, vertex addresses and indexed draws in original order. The fixture holds the camera and workload fixed to measure this case. |

The six arms run in immediate, section, frame, frame, section, immediate order.
Each has an untimed first presentation, eight timed samples per display and
repeated presentation during a seven-second visual window. Output binding,
clearing, shader mode, depth, state setup and scan-out stay outside the
display lists and measured build span. The lists contain core GX2 uniform,
attribute and draw commands. The fixture records them once, flushes the
written memory, and keeps their addresses and referenced buffers live.

The serial setup records the guest timebase, list sizes and one-time
recording costs. Each sample reports CPU ticks for building commands,
explicit flush and the subsequent retirement wait. The wait span includes
reading the new watermark before waiting. `auto=1` means the submission
watermark advanced during command construction, so the explicit wait omits
GPU work already retired in that interval. `retired=1` requires a new
submission watermark and confirmation that retirement reached it.
Presentation, flip pacing, logging and pipeline setup are outside these
timings. These samples measure the synthetic workload only.
[WUT GX2 display-list API](https://github.com/devkitPro/wut/blob/v1.9.2/include/gx2/displaylist.h)
· [Cemu command submission implementation](https://github.com/cemu-project/Cemu/blob/main/src/Cafe/OS/libs/gx2/GX2_Command.cpp#L384-L419)
· [Crema hardware display-list findings](https://github.com/vs-sr-dev/wiiu-crema#lessons-learned-cemu-vs-real-hardware)

## Build and verify

Use the pinned devkitPPC, WUT and CafeGLSL. Run from the repository root
with the usual toolchain environment.

```sh
build=$(mktemp -d)
bash wiiu/diagnostics/gx2-display-lists/build.sh "$build"
```

Boot the RPX in a disposable forced-interpreter Cemu lab. Wait for
`GX2 display lists setup=READY`. Capture both outputs at each
`GX2 display lists visual` marker.

- TV shows four red rows above four blue rows, with a green tile at the
  top left.
- GamePad shows four blue rows above four red rows, with a green tile at
  the bottom right.
- Both show an eight-by-eight tile grid. Tiles in even zero-based columns
  are slightly narrower, and gaps stay black.
- Compare outputs across all six arms. Require 96 unique timing samples
  with `retired=1` and the final `result=SUBMITTED` log.

Use the WUHB in a dedicated app directory for the later hardware run.
Record the display mode, serial, TV stills and GamePad observation. Confirm
return to the Wii U Menu. The probe accesses no game files or resources and
uses scan-out stills for visual comparison.

## RC 29 development result

RPX `3e1d897f7edd6262d158cfaf116fe7a29ee7fb52b92b5ba0eea0f26fc8df90aa`
passed in forced-interpreter Cemu. The WUHB
`b02076bfba4d733cfdbf7eec82964b620f7c68a388754bdebccf473506c94c9b`
contains that RPX. The 12 stills were byte-identical within each display
and passed 1,536 expected-color checks at tile centers and alternating
right edges. All 96 samples retired. Recorded section lists used 8,192
bytes total. Each fixed output list used 12,032 bytes. The fixture reserved
66,048 bytes for all list storage including alignment and bookkeeping.

| Mode | Median command build, summed TV and GamePad | Median build, explicit flush and drain, summed TV and GamePad |
| --- | --- | --- |
| Immediate | 83.72 and 83.38 µs | 332.61 and 309.33 µs |
| Section lists | 50.43 and 88.63 µs | 261.82 and 323.88 µs |
| Frame list | 3.27 and 2.85 µs | 185.27 and 133.82 µs |

Each value is the median of eight paired samples in a separate arm. The
immediate and section-list measurements overlap. Those calls submitted
early on both outputs in every timed Cemu sample. The fixed lists reached
the explicit flush before their first new submission in every sample.
Hardware timing and visuals remain pending.

## Applicability to hosted terrain

Section lists match the lifetime of immutable vertex and index data. A
possible production cache would attach them to a resident solid mesh and
its index-buffer generation, with live clip/origin uniforms per display.
Buffer replacement and eviction would retire the cached list alongside the
buffers after their last GPU use. Pooled, bounded list capacity would count
against terrain admission. A failed allocation or record would leave the
immediate path available. Translucent sorted index uploads and section
publication/eviction require separate validation.

A cached full output list also depends on camera-relative transforms,
visible section membership and order, viewport projection, and every
recorded address. Different TV and GamePad projections require independent
qualification. Begin with the section case on hardware and measure the
complete command cost, including flush and retirement. The fixed-list arm
provides an upper-bound comparison for repeat-camera command reuse.
