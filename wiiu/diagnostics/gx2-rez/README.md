# Independent GX2 Re-Z diagnostic

The same RC64-derived RPX switches between stock and candidate pixel
shader descriptors. Both use byte-identical stock VS/PS code, stock
render-6 fog and the native indexed terrain draw path. The candidate
changes only SHA-pinned GFD byte 0x697 from 0x50 to 0x70, retaining KILL
and requesting Re-Z on fallback. The fixture checks the exact runtime
descriptor field and executable bytes before drawing.

The fixture binds separate TV and GamePad render contexts and native-size
colour buffers, plus a separate stock-format D32 MEM2 depth surface for
each output. The depth pair is capped at 14 MiB. It draws different
projection and background controls per display and copies each output
explicitly to its respective scan target. The official WHB begin wait
is followed by a positive scan-counter check. Both resources and cycle
samples remain owned until positive final retirement.

## Same-package parity and timing

1. The parity stage compares full RGBA8 output from stock and Re-Z in
   both independent displays across 0, 8 and 24 seeded far draws with
   early-front or interleaved depth order. Each arm submits the same
   vertex/index sequence and order for a given case. The expected draw
   counts are 2, 12 and 32 per output, excluding the full-surface clear.
2. Direct pixel guards test front alpha127 rejection followed by a later
   visible depth write, alpha128 acceptance over a hidden later draw,
   the last of two equal-depth quads, and opaque alpha-zero color/depth
   behavior. The wrong-KILL arm must change the alpha127 hole on both
   displays and more than 1/30 of each full image. Distinct background
   colors confirm outside geometry and scan-target identity. The stage
   uses explicit positively retired staging and exercises both guard bytes.
3. Successful parity gates a separate timing stage. A warm clock check
   precedes four interleaved A/B repeats per scene. The 27 MHz bottom
   counters bracket the same selected draw stream, outside clear or
   scan-copy commands. The timed stage has no readback or in-bracket
   synchronization. Markers use separate aligned cache lines, read only
   after normal positive paired retirement. Driver queue starvation and
   event scheduling can still affect intervals.
4. Require `GX2 ReZ parity stage result=PASS` and final
   `GX2 ReZ fixture result=PASS`. The timing comparison also requires
   `GX2 ReZ timing complete=1` and all 48 positive per-output paired samples.
   Missing clocks preserve only visual qualification. Compare the
   high-overdraw deltas against the zero-far control per output.
   Stop on pixel mismatches or failed retirement.

The independently drawn fixture uses scan-sized targets. Physical
hardware is required to establish whether the scheduling hint changes
Latte work. Use the fixed-75% gameplay workload for any later speed claim.

The documented Re-Z exclusions include depth writes with `NOT_EQUAL`.
Mesa also records a lockup with Re-Z and fixed-function alpha testing on
R6xx/R7xx. This fixture uses `LEQUAL`, disables fixed-function alpha
testing and retains shader discard. Qualify the standalone fixture under
supervision before considering a gameplay change.
[AMD register guide](https://www.x.org/docs/AMD/old/R6xx_3D_Registers.pdf) ·
[AMD programming guide](https://www.x.org/docs/AMD/old/R6xx_R7xx_3D.pdf) ·
[Mesa depth-state implementation](https://chromium.googlesource.com/chromiumos/third_party/mesa/+/a11d1bd247df18057ecb9945e01bf39ff85d8d9d/src/gallium/drivers/r600/r600_state.c#2813)

```sh
python3 -B reduce.py SERIAL.log
```

## Reproduction

Build against the stock terrain source in RC64 tree
`a22797fa38ddd43b1cd23f5435a43d718ae5f8f5`.
The compile uses the pinned RC1 CafeGLSL SHA-256
`8268622fbaade63707d649c212e990ef871a4d1657450d03dc2ff85f8fa4ec90`
with the devkitPPC and WUT from the native recipe. Pass a new output
directory.

```sh
bash build.sh "$assembled_source" "$cafeglsl" "$build_directory"
python3 -B test_draw_stream.py
```

Strict Cafe C compilation, reproducible packaging, Wii U relocations and
host draw-stream controls passed. Cemu interpreter passed all 28 visual
records for the exact RPX. The wrong shader changed 97,682 TV pixels and
17,542 GamePad pixels. This Cemu build supplied no usable GPU timestamps.

| Artifact | SHA-256 |
| --- | --- |
| RPX | `447cebc7da487815f9d6055173fd625e543f3a224007720df079ccc2cac2f93e` |
| WUHB | `6d02dc235b9477daa6f53881eabc03eaaa04ea0bf44a460a10b35aa071a01995` |
