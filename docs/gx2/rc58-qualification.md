# RC 58 shader and preparation experiments

| Input or output | Identity |
| --- | --- |
| Port build source | `dd6e02b8b3b47d0cc3cd4db1f9c18789498be2ab` |
| Engine tree | `c1f06f14cbf876661e33978591f82da74bd0ba6f` |
| Recipe SHA-256 | `49604a06815e9e70d856a285be16712a3a8b6b5b0a2d74ee765f9ac5cd027ee0` |
| Cafe Toolchain | `1.1.0` |
| RPX SHA-256 | `ed12eb99628b29326cdb853c6db1376da88cd8de5e09a958797188b73f4d3459` |
| WUHB SHA-256 | `813a194c62f552f907eab3a16e6e09a2b394eba44bfd95c017c42c0cd69a1215` |
| Estimated Aroma code-heap headroom | 12,460,352 bytes |

Ben-Desktop built the exact staged engine in
`/var/tmp/mchorizon-rc58-native.tgtwjmto/out` with
`tools/build_cafe_game_isolated.sh --hosted-nightly` and the pinned caches.
The native package and independent file hash checks passed. The package
remains an engineering input. The optional opaque-tile observer needed
additional isolation for screenshots and benchmark captures before
supervised use.

## Clear opaque draw

The clear pixel loader compares the compiler's loop descriptor with
the corresponding stock shader. It keeps the other reflection, sampler
and depth checks. The separate vertex experiment retains the shader
interface and cuts 32 dwords, one register and two square roots.
The independent native C cross-build passed with and without the feature.
Both standalone Cemu fixtures passed all 12 readback cases, including
the output-semantic negative. [Shader and fixture evidence](terrain-format-trials.md#clear-opaque-vertex-program)

Use `4o` with `gx2-clear-fog.enable` for a separate clear pixel
diagnostic. Add `gx2-clear-fog-vertex.enable` for the vertex arm.
Verify both ready messages for the vertex arm and actual per-output
clear opaque submitted indices. The ordinary stock Full Performance
Overworld reference keeps fog on and is measured separately.

The exact package's Cemu multicore replay reached both ready messages
and independent TV and GamePad gameplay. Its 35-second dwell and save
recorded 360 pairs, 162 without a partial flag, and up to 118 sections
per output. The three sampled timing rows available for clear-fog
reporting occurred during terrain growth. The replay recorded shader
loading and a successful save. Live clear-run draw counts remain open.
The GX2 bulk heap recovered to 234,880,920 free bytes after Game drop.
One queued audio item remained at the teardown checkpoint.

The exact prepared stock Overworld sets no atmospheric fog-start
override. The default starts at zero and rain can move it lower.
The clear geometry predicate requires both fog starts above one block,
so this workload cannot admit clear draws. Treat a fog-disabled
positive control as a parity diagnostic. The native stock-fog shader
remains in the performance reference.

Session HZP3 SHA-256 is
`30d46e007e8e3b5a0687f881fdd24d0f51edd4e12df572238ef45378f6cd8af8`.
Acceptance receipt SHA-256 is
`a5ba804c80771ae6a8d83707f116456d0cc12a9cdece52b37a461f26ec48d034`.

## GPU overlap, geometry and loading

- The separate GamePad prepass computes section visibility and draw
  order while submitted TV commands can advance. The existing GamePad
  wait still gates native vertex/index, texture and HUD mutations.
  Its 26 timeline controls retain both scan copies, final retirement,
  explicit recovery and identical per-output ordering. The prepass
  requires grouped plain `4` and a separate marker. The early-flush
  marker takes precedence. Grouped physical work awaits qualified HUD
  teardown. [Grouped lifecycle evidence](rc57-qualification.md#grouped-cemu-lifecycle-check)
- The opaque-tile observer uses bounded footprints from uploaded
  uncutout solid geometry. It collects separate output candidates
  without draw filtering. The saved-region host proxy covered 22 of
  234 projected surface sections on both views. The exact saved
  physical reference still needs native resident input and raster
  comparisons.
- Borrowing repeated stock gameplay fact strings reduced host
  allocation requests from 355,158 to 69,374 and reduced peak
  requested bytes by about 6.44 MB. The fixture matched all 35,723
  state and behavior projections, with 12 focused tests passing.
  Host elapsed changes were small. Native startup timing remains
  pending. Full archive verification and loose-file fallback remained
  in the path.
