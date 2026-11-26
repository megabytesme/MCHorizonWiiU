# RC 37 development candidate

RC 37 adds the loading and model follow-ons to the
[RC 36 integration](rc36-validation.md). Its port recipe is in
`a91c67873336` and the assembled engine tree is
`140d6c4e3a28d5cb26fe8e53714823d638b1797e`. The Horizon upstream and Cafe Toolchain 1.1.0
pins carry forward from RC 36.

| Component | Identity |
| --- | --- |
| Client and patch | `0.8.0-rc.37` and `0.5.0-rc.37` |
| Recipe SHA-256 | `1e42c70ed4d3e0550912120cb346a389c8f014717ce907ae0c74a4d1d4027b0d` |
| RPX SHA-256 | `e916d7c10c5ab3ce5f55e26e0193d06c5dd520566d5fd3f902ffc0655f38e18b` |
| WUHB SHA-256 | `5472d7fb0f9530360464dec1bb6bf197a0450fe379491e479406af946356f36c` |
| Estimated Aroma code-heap headroom | 12,819,008 bytes |

Ben-Desktop built the package in WSL at
`/var/tmp/mchorizon-rc36-root.BTvLBV0f/build-rc37` with
`tools/build_cafe_game_isolated.sh --hosted-nightly` and 32 Cargo jobs.
Cargo completed in 20 minutes 6 seconds. The WUHB embeds the exact RPX
at offset 512. Output verification checked imports, relocations and hashes.

## Loading

- Registry and behaviour snapshot keys now share the content hash of
  unchanged resource files. This can remove a second streamed read and
  SHA-256 pass over the 10,126,974-byte stock facts file. Files without
  usable modification timestamps are read again. The patched world crate
  passed Cafe type checking and all six stock-backed snapshot tests.
- The HUD and terrain decoders reuse packed RGBA8 output and copy square
  animation crops by row. The source-bound host probe matched all 3,903
  stock images and 21 crop cases. The fast path avoided a second
  allocation for 2,510 PNGs and 29,079,772 bytes of copying. Across 21
  alternating in-memory inventory passes, the median moved from 49.918
  to 38.098 ms. This measures the decoder kernel.

## Native models

Hurt red, TNT white and the creeper flash use a separate shader variant
when the world batch needs them. The ordinary model shader remained
byte-identical. The vertex layout stays at 32 bytes. The full modified
native loading-host crate passed the Cafe type check.

The forced-interpreter Cemu fixture passed all 28 sampled TV and GamePad
pixels, including cutout, cardinal shading, first-person depth reset,
custom hit color and shared uploads. Its RPX SHA-256 is
`6ff7a44dfc622ce754ec1812a5a14d283058fb7b8ab614753e2ee6ee0c82ce68`.
[Fixture and expected colors](../../wiiu/diagnostics/gx2-hand/README.md)

Live RC 37 cold and warm times, physical materials and physical
frame timing remain open. [Hardware qualification queue](post-rc35-queue.md)
