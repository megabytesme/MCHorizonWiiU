# Native terrain atlas subimage update diagnostic

Build this fixture against the exact assembly with the GX2 terrain animation patch applied. It requires no prepared content.

```sh
bash build.sh "$assembled_source" "$scratch/terrain-atlas-animation"
```

Prepare a fresh Cemu lab with that RPX, then launch interpreter mode with the repository's existing `tools/gx2_cemu.py` driver. The fixture draws a two-color atlas sprite and a green neighbor from a single uploaded GX2 terrain section. After the first full TV and GamePad presentation, it patches only the sprite cell, including its border, then draws the same retained terrain section again. The log reports 20 pixel checks before a successful guest exit. Green stays identical while the two target colors change on both displays.

The fixture exercises the native backing update and cache invalidation. Hosted game timing, pack-driven animation transitions and physical hardware visibility remain separate checks.
