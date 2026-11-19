# Horizon Night

Horizon Night presents the shared launcher on both outputs. The TV puts Play
beside a compact action panel and leaves the skyline visible. The GamePad puts
Play above a full-width touch list. Both layouts display the profile, resource
and input states supplied by the shared launcher. Only enabled rows accept
touch. Controller focus uses a violet ring, and a white check identifies saved
selection independently of focus.

The sunset follows the dark palette in `crates/horizon-theme/src/night.rs` in the assembled source
and the pixel scene in `launcher/src/sky.rs`. The launcher
frame, typography and translucent controls follow the desktop Night and Glass
design in `docs/design-system.md`,
`launcher/src/glass_home.rs` and
`launcher/src/glass_ui.rs`.

## Assets and builds

`bake.py` generates a 512×400 RGBA atlas and a table of fixed-size glyph
records. The sky is original pixel artwork drawn in Python. The Horizon mark
comes from the repository's `crates/horizon-theme/icons/horizon-64.png`. The
type uses the bundled Bricolage Grotesque Display Bold, Geist Regular and Geist
Mono Regular. The font license notices are in
[OFL-BricolageGrotesque.txt](notices/OFL-BricolageGrotesque.txt)
and [OFL-Geist.txt](notices/OFL-Geist.txt).

The build uses the checked-in atlas and glyph table. To regenerate the assets,
run the following with Python 3.14 and Pillow 12.0.0 in an isolated environment.

```sh
python -m pip install Pillow==12.0.0
python wiiu/pregame/themes/night/bake.py --horizon-source /path/to/assembled/source
```

The launcher builds Horizon Night by default. See the
[builder documentation](../../../builder/README.md) for toolchain setup and
verification. Output packages are named `HorizonLauncher-Night.rpx` and
`HorizonLauncher-Night.wuhb`.

Use the shared Cemu fixture in [tests/cemu.ps1](../../tests/cemu.ps1) to exercise
launch, cancel, return and the diagnostic startup error. The Home Profiles row
contains logical GamePad coordinate (640, 350) on the 1280×720 canvas.
