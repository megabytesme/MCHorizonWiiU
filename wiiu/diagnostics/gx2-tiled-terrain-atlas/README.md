# GX2 1D tiled terrain atlas

This standalone fixture compares the stock linear atlas with one native
1D thin-tiled backing. Both use the same stock terrain shaders and the
same per-output geometry and projection. The stock-sized 2,046×2,772
atlas uses a 2,048-pixel pitch. The tiled allocation holds 22,740,992
bytes, 32,768 more than the linear allocation.

Both layouts exercise high sprite layers, repeated UV edges, cutout
alpha, fog and four synthetic animation writes. Before mutation the
fixture checks the exact TV and GamePad readback submissions and the
trailing scan-copy retirement. It checks all visible and padded texels.
The tiled updates flush 36 cache spans covering 82,944 bytes. The
wrong-cell control changes the expected output pixels.

A magenta precondition followed by an explicit black and depth draw
sets the same active attachments in every comparison. The omitted
black draw must leave the sampled gap magenta. This makes the test
independent of a repeated GamePad clear artifact seen in both Cemu
backends with the ordinary clear sequence.

Use an assembled RC61 tree and the pinned CafeGLSL and devkitPPC tools.
The build copies the relevant terrain files into the output directory
and applies a scratch-only texture factory.

```sh
bash wiiu/diagnostics/gx2-tiled-terrain-atlas/build.sh "$assembled_rc61_source" "$scratch"
```

Run `atlas-1d-tile-draw-clear.rpx` in a fresh Cemu interpreter lab.
Require both ownership controls, four storage passes, six covering
scan retirements, all ten image comparisons and both omitted-clear
controls. The title prints `GX2 tile atlas diagnostic result=PASS` and
exits with `coreinit.exit(0)`.

The verified RPX SHA-256 is
`e33da35d3e606977b89a574fc76c9de5893e9cb2ef0708656a0d99127a063cbc`.
All normal tiled comparisons matched linear RGBA byte for byte at
1,920×1,080 and 854×480. The four updates changed 478,400 and 74,805
pixels respectively with no neighboring pixel changes. The deliberate
wrong cell changed 1,793,540 and 280,015 pixels. Gameplay upload cost,
animation retirement and GPU sampling time on hardware remain open.
