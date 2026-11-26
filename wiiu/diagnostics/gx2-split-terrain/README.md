# Two-view terrain planning

This host fixture uses the exact `0865ec25196ea2a96a6a330009a930d9c0ce16ea`
intermediate engine tree in the RC 45 source recipe. It extracts the
single-view projection, rank, residency and translucency decisions by
Git object ID. The two-view planner compares its results against separate
calls to those decisions for each camera.

```sh
scratch=$(mktemp -d)
cp -R "$port/wiiu/diagnostics/gx2-split-terrain/." "$scratch/"
cd "$scratch"
python3 extract.py "$assembled_source"
cargo test --offline --locked
```

All 11 tests passed. The two cameras use different poses and aspects.
The fixture verifies shared and unique visible sections, followed by
entirely disjoint visibility after movement. Solid draw keys, origins
and clip rows match independent single-view decisions bit-for-bit.
The views sort shared translucent geometry in opposite orders, preserve
their own last sort for sub-block movement, and invalidate both caches
after a bounds-changing replacement. One bounded resident map serves
both views and preserves the old generation on an oversized replacement.

## Native integration

Start with separate local players in one dimension and one view on each
physical display. Stream the union of the views' ticket regions and keep
one immutable vertex and quad-center allocation per mesh generation.
Use independent sorted GPU indices while the two view commands can
overlap. A second unsigned 32-bit stream adds 24 payload bytes per
translucent quad plus GX2 alignment. Charge both streams and any retiring
generation to the resident budget.

Build the HUD and first-person batches from each seat's own camera,
viewport and player state. Share the atlas and publish both views'
patches before drawing. Assign GamePad touch and the software keyboard
to one UI owner, and keep independent button-edge and disconnect state
for a second controller. Two views on one display need their own
viewport and scissor within that display's frame.

Native two-player rendering, GPU memory high-water and cadence still
need qualification.
