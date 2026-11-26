# GX2 first-person diagnostic

This standalone RPX links the native hand pass against a verified hosted
source assembly with the outline and first-person patches. It needs WUT,
devkitPPC, CafeGLSL, CMake and Ninja.

```sh
PATH="$CAFEGLSL_ROOT/bin:$PATH" \
  bash wiiu/diagnostics/gx2-hand/build.sh "$assembled_source" "$scratch_build"
```

Use `tools/gx2_cemu.py` with a fresh isolated lab. No world or prepared assets
are needed. The fixture draws a red world quad in front of deliberately
farther hand geometry. A patch changes a texel in the resident HUD atlas
before the hand pass samples it. The left pixel verifies depth reset, the
updated atlas and lightmap modulation. The right pixel verifies alpha discard
and preservation of the world color. A passing run ends with
`GX2 hand fixture passed`.
## World model overlays

Set HORIZON_TEST_MODEL_OVERLAYS=1 when building against an assembled engine
containing the GX2 world model overlay patch. The default test still builds
against the older source and exercises the first-person depth reset and
transparent cutout.

The optional fixture draws ordinary shading, hurt red, primed TNT white,
the creeper white row at column 7, cardinal-shaded hurt and a transparent
quad in the same indexed world batch. It runs once with the stock hurt
texel and direct model uploads, then with custom hit blue and a shared
TV and GamePad vertex upload. An explicitly drawn backdrop covers
the test position under the transparent quad.

NSS-patched Cemu 2.6 forced interpreter passed all 28 TV and GamePad pixel
checks. The fixture RPX SHA-256 is
6ff7a44dfc622ce754ec1812a5a14d283058fb7b8ab614753e2ee6ee0c82ce68.
The ordinary model shader is byte-identical to its RC 36
preimage, with GFD SHA-256
2930c94012a9358cb06fde0595be0a6ebcb7d78f31a1e377c27ea6a839646bd5.
Live world-scene coverage and physical timing remain open.
