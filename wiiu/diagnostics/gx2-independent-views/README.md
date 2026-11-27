# Independent GX2 view fixture

Build from an assembled Cafe source tree and a separate output directory.
Use the same Cafe Rust compiler and GLSL compiler as the game build.

    CAFE_RUSTC=/path/to/cafe/rustc CAFEGLSL_COMPILER_EXECUTABLE=/path/to/glslcompiler \
      ./build.sh /path/to/assembled/source /path/to/output

The RPX exercises one full-width TV view, two TV halves, and four TV
quadrants. Every layout presents an independent full GamePad view with
seat ID four. Each rectangle uses its own camera, aspect, terrain
viewport, column scissor, depth-tested world and preuploaded HUD marker.
The production WHB callback owns both scan copies. Each case checks
one completed pair, both physical outputs, all expected view IDs and
draw submissions. Both camera poses and both residency arms run.

- Nearby views share one immutable 174-section mesh site. Distant views
  draw separate 174-section sites spaced 1,024 blocks apart.
- Geometry has 69,206 quads and 415,236 indices per view. Five views
  issue 870 indexed draws and 2,076,180 indices per pair. Raw section
  vertices occupy 7,751,072 bytes per site. The native allocator
  reports exact aligned bytes and accepts up to 48 MiB of section
  backing. One guarded stage and one reusable packed RGBA buffer cap
  readback payload at 17,731,584 bytes.
- Readback checks the near stripe, its shift after camera movement,
  the farther background, both horizontal guard edges and the HUD.
  Distinct far fog colors identify which TV row owns the last and first
  pixels at a quadrant seam. The test requires correct adjacent content
  on both sides at the actual seam, without an inset vertical moat.
  A distant white terrain witness appears only in the GamePad site.
  Sampling an actual centered crop of the TV result fails both GamePad
  terrain points even if a separate HUD is available.
- Fault arms require exact results per seat. Reusing camera zero
  fails both terrain samples in the other four seats. Disabling depth
  fails both terrain samples in all seats. Removing the column scissor
  fails the edge class in all seats. Extending the lower viewports into
  the upper row must fail both upper seam checks and keep other samples.
  Using the wrong site fails both GamePad terrain samples. Each fault
  preserves the HUD samples.

The optional analytic host test links the generated projection object
for the host and checks the camera geometry and masks at three TV sizes.
In the output directory run the following after the native build.

    rustc --edition 2021 --crate-type lib -C panic=abort -C opt-level=2 \
      --emit obj=projection-host.o projection.rs
    cc -O2 -Wall -Wextra -Werror /path/to/fixture/oracle_test.c projection-host.o -o oracle-test
    ./oracle-test

The synthetic 1×1 atlas and flat lightmap qualify the opaque scheduling
and pixel oracle. Stock/custom packs, translucent draws, terrain
streaming, gameplay, native render time and memory stability each
require their own evidence.

## Qualification

The production C renderer, shader inputs and Rust projection match RC 47
engine tree `499ba0705dce6703c521417ba92e2542380e074a`. The build used
assembled tree `38396809d676ba527c6f01f6aae6eb0e729eed51` and the pinned
Cafe 1.1.0 toolchain. An independent clean output reproduced both hashes.

| Artifact | SHA-256 |
| --- | --- |
| RPX | `783bb38ce8530243aa436e9c904d8738c62fddf6645c33d9bd8c78a4904464e4` |
| WUHB | `4d4e087a27ad7718fc01945e435837e70ac4cafdd83e1cd9adc0a918b673cb9f` |

Cemu 2.6 Vulkan passed all 12 positive combinations at 1920×1080 TV and
854×480 GamePad, returned the expected masks for all five injected faults,
and rejected each of the three sampled TV crops. The native allocator
reported 7,751,104 aligned section bytes for one site and 38,755,520 for
five sites. The analytic host test also covered 1280×720 and 1281×721 TV
sizes. The run ended with `GX2 independent views fixture PASS`.

An earlier inset oracle expected four clear rows inside the lower TV
quadrants. Cemu drew to the y540 seam in both variants of that oracle.
The current fixture checks actual adjacent ownership at the seam and
detects a deliberate 16-row overlap. The original inset behavior,
physical timing, full material parity and local player controls remain
open.
