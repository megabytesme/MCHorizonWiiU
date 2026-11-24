# GX2 block-outline diagnostic

This small RPX links the native outline draw from a verified hosted source assembly.
It needs WUT, devkitPPC and CafeGLSL. Set `CAFEGLSL_COMPILER_EXECUTABLE` if
`glslcompiler` is outside PATH.

```sh
bash wiiu/diagnostics/gx2-outline/build.sh "$assembled_source" "$scratch_build"
```

Use `tools/gx2_cemu.py` with a fresh standalone lab and that RPX. The diagnostic
logs four readback samples per output. Red verifies depth occlusion, cyan the
primary stroke, black the secondary stroke, and white the surrounding clear.
A passing run ends with `GX2 outline fixture passed`. No world or prepared
resources are needed. The standalone pixels exercise the GX2 shader and state.
Host geometry tests exercise projection, linewidth and near-plane clipping.

```sh
rustc --edition=2021 --test "$assembled_source/wiiu/loading_host/outline_vertices.rs" \
  -o "$scratch_build/outline-vertices-test"
"$scratch_build/outline-vertices-test"
```