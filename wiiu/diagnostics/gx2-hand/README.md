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