# GX2 20-byte exact half-position terrain fixture

The pair arm compares the native 28-byte stock terrain vertices, the
qualified 24-byte half-position format, and a 20-byte format with exact
half XYZ and the original unsigned 16-bit light word. The vertex shader
and stock pixel shader are identical in both half arms. Positions, UVs,
color, cutout, material state, both fog ramps, and draw order are held.
The two outputs have separate attachments and projections.

Position fetch reads half4 at byte zero and keeps XYZ. The spare W
component is forced to one. The compact light fetch reads an unsigned
scaled 16_16 pair at byte four and routes its G lane to the scalar
light input. The second halfword is the original light value. Both
fetches explicitly use 8-in-16 endian swap. Numeric scaling exactly
represents each original unsigned value as binary32.

The second RPX draws a hardware-only scalar contrast using unsigned
scaled 16 at byte six. Pinned Cemu's legacy decoder handles unsigned
scaled pairs. The scalar RPX needs the tested decoder patch for Cemu
Vulkan. [Decoder validation](../../cemu/VERTEX-FETCH.md)

Build from exact assembled RC68 engine
b24421b563b1195818039eb6c0f1c49ea3c4bf22 and the pinned compiler.

    bash wiiu/diagnostics/gx2-half20-light/build.sh "$assembled" "$cafeglsl" "$scratch"

Run gx2-half20-pair.rpx in a fresh interpreter lab with
[the Cemu fixture runner](../../../tools/gx2_cemu.py).
Require four valid fetch descriptor rows, 24 per-output pass=1 rows,
GX2 half20 fixture result=PASS and coreinit.exit(0).
With an asymmetric lightmap the wrong lane, byte order and clamp
controls must visibly change RGB on each output. They must preserve
coverage and alpha. Under the flat map all arms match pixel-for-pixel.

All 24 rows passed in Cemu interpreter on the exact pair RPX. Both
positive compact arms matched stock RGBA on TV and GamePad. All three
negative controls fired under the varying map. The host oracle also
compared all 65,536 original light words.

- Pair RPX SHA-256 387cf8030e443cdc70398ebaf1e7aed8786cbafaad5e3c08090b0c8170665416.
- Scalar RPX SHA-256 44407ef5487004b0bb0dfdaa66ff2029763e81367c033c76f4baa20fb68b2adb.
- Interpreter verification JSON SHA-256 2cf1df5448ab6280ccaba56893dc2840ba09a9db733ec5520f407e1713fddfb7.

Physical timing and parity remain pending. The 20-byte format reduces
logical eligible payload by one sixth versus half24. Vertex cache
traffic, allocation alignment, and actual eligible coverage determine
the physical effect.
