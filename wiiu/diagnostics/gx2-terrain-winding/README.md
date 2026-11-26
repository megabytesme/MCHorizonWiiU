# GX2 solid terrain winding fixture

The fixture draws one terrain section on TV and GamePad through the
assembled production shader, vertex format and shared quad-index buffer.
It renders with the normal solid-pass state and repeats with GX2 back-face
culling enabled. The test uses a two-pixel atlas and requires no Minecraft
assets.

Build with the pinned Cafe toolchain and an assembled Horizon source tree.

```sh
bash wiiu/diagnostics/gx2-terrain-winding/build.sh "$assembled_source" "$scratch"
```

Prepare a disposable Cemu lab with `terrain_winding.rpx` using the
[fixture runner](../../../tools/gx2_cemu.py). Use the interpreter mode
and wait for `GX2 terrain winding diagnostic result=PASS`. There are 32
RGB checks across the two outputs and two culling states.

- The normal solid pass draws opposite-winding opaque and visible cutout
  quads. Back-face culling retains the front of each material and leaves
  the back-facing samples clear.
- The near cutout's alpha of 127 leaves the farther opaque surface
  visible. A visible cutout at alpha 128 still draws. The opaque quads
  use the same alpha-127 texel without applying a cutout.
- Pairs containing both windings remain visible regardless of their
  submission order.

This fixture uses CPU framebuffer readback in Cemu. A hosted world replay
and physical observations are needed before enabling solid terrain back
culling in gameplay.

The RC 38 terrain source built with the pinned Cafe tools and passed all
32 checks in NSS-patched Cemu 2.6 with the forced interpreter. The
fixture exited normally via `coreinit.exit(0)`. Its RPX SHA-256 was
`37d132dbb588120a6154ad264414f680cf37f037b859e257ca4ef9eabaa56bce`.
The assembled terrain shader SHA-256 was
`c07d3b7557e55341a3bfcd3e36cf2dbd048f8473514a8ac26d400878503f7133`.
