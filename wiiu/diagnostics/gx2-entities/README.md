# GX2 ordinary entity diagnostic

Build against a verified hosted source assembly with the entity-model draft, or
build directly against the sparse native source in this directory.

```sh
PATH="$CAFEGLSL_ROOT/bin:$PATH" bash build.sh "$assembled_source" "$scratch_build"
```

Use `tools/gx2_cemu.py` with an isolated standalone fixture. No prepared
resources or world are needed. A red foreground reference, green indexed model
and later blue background test three depth and material outcomes on both
displays. Green comes from the HUD atlas multiplied by the resident terrain
lightmap. A passing run ends with `GX2 entity fixture passed`.
