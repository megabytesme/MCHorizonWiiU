# Share world color across compatible Wii U views

This standalone GX2 fixture compares independent scaled world draws with
one world draw to the wider TV projection and a centered crop into the
GamePad viewport. Both arms draw separate native HUD batches. Both
outputs retain their own viewport, clear and scan copy. The shared arm
uses the same scene camera and vertical field of view for both displays.

Apply `crop-tv-world-for-gamepad.patch` to a disposable assembled RC 44
tree `633a3a9427e98e5d93a45ea2f896753d652aa8aa`. Add the pinned CafeGLSL
compiler to `PATH` and build the fixture in a temporary directory.

```sh
git -C "$assembled_source" apply "$port/wiiu/diagnostics/gx2-shared-world/crop-tv-world-for-gamepad.patch"
scratch=$(mktemp -d)
bash "$port/wiiu/diagnostics/gx2-shared-world/build.sh" "$assembled_source" "$scratch"
```

Launch `shared_world_fixture.rpx` in an isolated Cemu fixture with the
established forced interpreter. Check four `GX2 shared world` comparisons
and `GX2 shared world fixture PASS`. Each comparison uses caller-owned
staged readback after both outputs have been submitted and retired.

| GamePad viewport | Changed TV pixels | Changed GamePad pixels | Changed pixels outside synthetic edges |
| --- | ---: | ---: | ---: |
| Full 854 × 480 | 0 of 2,073,600 | 3,546 of 409,920 | 0 |
| Inset 714 × 480 | 0 of 2,073,600 | 2,104 of 409,920 | 0 |

The independent arm submitted two world draws and the shared arm
submitted one. Both preserved the foreground depth probe and per-display
HUD positions. Every pixel difference was within three pixels of a
synthetic scene edge. The fixture keeps both draw callbacks in the same
submission sequence before any readback drain.

The tested RPX SHA-256 was
`e271f297183ce00e663878ff786a554dedfc895c66c0ee69868deea88e7a3bf2`.
The repository build reproduced the runtime sections and relocations.
Its source filename and symbol metadata changed the package hash.

Gameplay adoption needs a same-pair TV export token and a fallback for
incompatible cameras and viewports. Screen-space outlines require their
native displayed width and depth relationship. First-person geometry,
water and other world passes need matched pixels on both displays before
sharing their world target. Physical GPU and paired cadence remain open.
