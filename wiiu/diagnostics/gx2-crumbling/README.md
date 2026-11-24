# GX2 block-breaking diagnostic

This standalone RPX links the native crumbling pass from a verified hosted
source assembly. It needs WUT, devkitPPC and CafeGLSL. Set
`CAFEGLSL_COMPILER_EXECUTABLE` if `glslcompiler` is outside PATH.

```sh
bash wiiu/diagnostics/gx2-crumbling/build.sh "$assembled_source" "$scratch_build"
```

Use `tools/gx2_cemu.py` with a fresh standalone lab. No world or prepared
resources are needed. The fixture draws both overlay ranges on each display,
using two stage sprites on a synthetic atlas. It verifies negative UV wrap,
alpha discard, stage selection and multiplicative blending with eight pixel
checks. A passing run ends with `GX2 crumbling fixture passed`.