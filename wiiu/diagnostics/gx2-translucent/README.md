# Native translucent terrain diagnostic

This RPX draws both WHB outputs with the production GX2 terrain implementation and a two-pixel white atlas. It uses a green opaque surface, alpha-weighted red and blue terrain quads, an invisible near quad, and a yellow opaque surface submitted last. The fixture runs without Minecraft assets.

From WSL after enabling the pinned Cafe toolchain:

```sh
bash wiiu/diagnostics/gx2-translucent/build.sh "$assembled_source" "$scratch/translucent"
```

Prepare an isolated Cemu lab with the resulting RPX using [the fixture runner](../../../tools/gx2_cemu.py), then launch in interpreter mode and wait for `GX2 translucent fixture passed`. The log records two pixel checks per display. Green `(0, 255, 0)` proves opaque occlusion. The second pixel `(64, 0, 96)` (within rounding tolerance) requires far-to-near U32 indices, texture alpha multiplied by tint alpha, discard of the invisible quad before it writes depth, and depth writes from the visible translucent surfaces. The readback checks the emulator's GX2 behavior for this fixture. An integrated saved-world replay and physical console comparison are separate checks.

For host coverage, compile and run `wiiu/gx2_terrain/tests/buffers.c` from the assembled tree with a C compiler and run `cargo test --manifest-path wiiu/gx2_terrain/Cargo.toml --features atlas` in that tree. The host tests exercise resource sizing, allocation rollback, mapped index updates and tint packing.
# Terrain state binding comparison

On an assembly with `horizon_gx2_terrain_bind`, set
`HORIZON_TEST_BOUND_TERRAIN=1` when invoking `build.sh`. The same fixture
submits 128 draws in each arm, alternating per-section and per-pass state
binding. It times CPU submission separately from the completion wait and
repeats the depth and blending assertions using the separated binding API.
