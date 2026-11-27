# Native terrain format and shader trials

These Cemu fixtures render separate TV and GamePad targets and compare the
full RGBA output. Their scenes exercise stock UVs, lighting, fog, depth or
translucency, depending on the experiment. Gameplay frame time and physical
parity require their own matched captures.

## Exact binary16 position fetch

The eligible format uses binary16 XYZ, the original 32-bit packed light,
two stock UV values and RGBA8 color. The stride falls from 28 to 24 bytes.
The fixture rejects coordinates that fail the exact binary16 check. The
stock and split FP32 arms isolate any shader or attribute rearrangement.

The initial half-position test drew zero terrain pixels. The pinned WHB
attribute helper assigned constant XYZ selectors to the half format. The
corrected fixture supplies the R, G, B, 1 selector before initializing the
fetch shader. The split FP32 and half-position arms then matched stock pixels
under the varying and flat lightmaps. Coverage remained 710,750 TV pixels
and 154,497 GamePad pixels in each arm. Changing the packed light differed
at 355,374 and 77,005 pixels with the varying map and matched with the flat
map.

Corrected RPX SHA-256
`3b53e17fb1cb33d9d8e492a4e5ca9cb40437f00ba8ef913b836aab5137c458b8`.
The production trial will measure exact per-layer eligibility and admission
cost. Water offsets, rotated models and other off-grid positions can require
the original float format.

## Packed integers and material specialization

Explicit R, G, 0, 1 selectors corrected the UINT2 fetch setup. The 24-byte
packed-integer fixture matched all flat-light pixels and retained the same
terrain coverage under varying light. Varying light changed 699,503 TV
pixels and 152,045 GamePad pixels. RPX SHA-256
`b355ef20429d65106ae991af2cc8f16c70db89a0b9c9159dd41dc14aea8c61d1`.

For the packed-atlas fragment program, the generic specialization reduced
the compiled program from 142 to 130 dwords and removed four pre-texture
ALU operations. The fixed solid and translucent variants use 110 and 108
dwords and move fog calculations earlier. All retained the same stock
vertex code and texture lookup count. Solid output matched in the initial
fixture. In the translucent pass, both trial shaders matched the TV and
differed at 85,508 GamePad pixels. A repeat with explicit per-output color
and depth clears returned the same difference count. Initial RPX SHA-256
`4d755971603b216c6a57c0630c2c8f6e18fd0e237a9e92f58b8353ff08690276`.
Explicit-clear RPX SHA-256
`8f9f23cc1c4003882f6ca2bafe036d81ae9b78eb19e423f8119ce3a08f95d20f`.

## Reversed water quad reuse

When consecutive water quads have byte-identical vertices in reversed
winding, the trial keeps one copy of the vertices and both original corner
orders. It preserves stock shaders, logical indices and sort order. In the
fixture, native backing fell from 8,448 to 4,800 bytes. All 12 positive
cases matched and the three injected violations changed the expected
outputs. RPX SHA-256
`3387be4c9240579657e0bf13938fb890e4f25b48b2f69ffa996e8e71a5a1a3f4`.
The default-off gameplay bridge budgets the remap alongside native storage
and falls back when a section would save no bytes. Native gameplay and
physical timing remain open.

## Greedy meshing eligibility

The current packed terrain UVs encode one repeat below 2.0. Greedy runs
spanning more cells need a repeat representation that preserves atlas seams.
The native shaders interpolate fog distances from each original corner.
A host control in the transition region changed the fog factor from zero
to 0.758 when corners were removed. A separate control used the actual
translucent sorter. Combining water cells moved an overlapping glass draw
ahead of the water and changed the controlled red channel from 0.551 to 0.271.

A conservative host predicate found clear-fog opaque candidates while
preserving actual local geometry bounds. Across the synthetic six-chunk
proxy, eligibility removed 13% of quads with an environmental fog start of
64 blocks and 27% with a start of 96. Those figures describe the proxy.
A sparse source-only diagnostic now prepares bounds from uploaded sections
and can report eligible opaque geometry for each output. Use the stock
saved-view captures to measure coverage before selecting a merge format.
