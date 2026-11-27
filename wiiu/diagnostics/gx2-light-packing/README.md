# GX2 terrain light packing fixture

The fixture compares the stock terrain shader with two alternatives on
independent TV and GamePad targets. The first encodes centered lightmap
coordinates in the existing FP32 vertex word. The second also places the
smooth cutout factor in the unused fourth component of vertex color.
The fixture keeps packed atlas addressing, stock depth and alpha,
Overworld render-distance-six fog, and indexed draw order. It tests both
solid/cutout terrain and the same scene with sorted translucent terrain.

| Program | VS dwords / registers / ALU groups | PS dwords / registers / ALU groups | Smooth parameters |
| --- | --- | --- | --- |
| Stock | 178 / 6 / 18 | 142 / 5 / 24 | 4 |
| Centered light | 162 / 5 / 18 | 142 / 5 / 24 | 4 |
| Combined | 158 / 5 / 17 | 142 / 4 / 24 | 3 |

Each path still samples one vertex light lookup texture and one fragment
atlas texture. The centered-only pixel program and descriptor exactly
match stock. The host oracle checks every 16-bit light pair, including
values above the original clamp.

Build from an assembled source and the pinned compiler. The verified
run used assembled engine tree
393c268a6cf0af5b4bcc2199c0a8482b4d454d13. The builder checks the exact
stock GFD and writes all generated material outside the source.

    bash wiiu/diagnostics/gx2-light-packing/build.sh "$assembled" "$cafeglsl" "$scratch"

Run gx2-light-packing.rpx in a fresh interpreter lab with
[the Cemu fixture runner](../../../tools/gx2_cemu.py). Require 20
per-output pass=1 rows followed by GX2 light packing fixture result=PASS
and coreinit.exit(0). Both positive arms must match all RGBA8 pixels.
The swapped-light arm must change RGB with unchanged alpha and
silhouette. The forced-cutout arm must change coverage.

The interpreter passed all 20 results with exact RGBA8 on both positive
arms in both cohorts. Wrong light changed 800,953 TV and 152,756
GamePad opaque/cutout pixels. With sorted translucency it changed
929,529 TV and 176,614 GamePad pixels. Alpha and coverage remained
identical in both controls. Forced cutout changed coverage on both
displays. This fixture establishes parity for its scenes. Hardware
shader cost and gameplay coverage still need measurement.

The verified RPX SHA-256 is
a9013d7153619d7c94675145256294f63c0925c108736c948103603b8b25a701.
The interpreter verification JSON SHA-256 is
e7070ad846b1194b831be2d2db90cc7411273d9d8c270c5cc448724a6a9f2611.
