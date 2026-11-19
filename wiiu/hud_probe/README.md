# Original Horizon HUD GX2 diagnostic

This diagnostic constructs Horizon's original `HudAtlas::boot` and `HudBatch`, submits their real vertices and atlas through `HorizonPresenter`, and reads back the TV colour buffer. It draws two diagnostic rectangles; it is **not** the Horizon title screen or a playable client. No Minecraft resources are included or required.

Build with the Cafe Rust standard-library toolchain, devkitPro/WUT, `cargo-wiiu`, and `cafeglsl` installed as described in `PORTING.md`:

```sh
export HORIZON_WIIU_STD_RUSTC=/path/to/cafe/stage1/bin/rustc
export CAFEGLSL_ROOT=/path/to/cafeglsl
export HORIZON_WIIU_HUD_PROBE_BUILD_DIR=/path/to/scratch/horizonwiiu-hud-probe
bash wiiu/hud_probe/build.sh
```

The RPX is written to `$HORIZON_WIIU_HUD_PROBE_BUILD_DIR/cmake/horizon_wiiu_hud_probe.rpx`. `HORIZON_WIIU_JOBS` optionally limits build parallelism. The build uses the Cafe custom getrandom backend and links `horizon-wiiu-entropy`.

For Cemu, use an isolated portable installation and SD card, create `portable/sdcard/wiiu/apps/HorizonWiiU`, and launch `Cemu.exe -g <absolute-path-to-rpx>`. Set `<AsyncCompile>false</AsyncCompile>` in that installation's `portable/settings.xml`; this is a one-frame readback probe and asynchronous shader compilation can leave its first draw blank even when the presenter is correct. The result is written to `portable/sdcard/wiiu/apps/HorizonWiiU/rust-hud-status.log` and to `OSReport`. The readback uses a Cemu-specific linear surface mode, so it is not a hardware test.

The entropy provider requires a working Cafe system RNG. Stock Cemu 2.6's NSS implementation did not supply bytes in our tests and the probe deliberately fails closed there. With a locally patched Cemu 2.6 NSS implementation and synchronous shader compilation, an isolated fresh-cache run of the final RPX (SHA-256 `4670b356c8a64d105aa39ef3ca99d63b54064fd73189579d80084ef890c7c9a8`) reported:

```text
readback=pass target=1920x1080 red=255,0,0,255 green=0,255,0,255 background=51,51,51,255
```

A separate temporary test of an atlas patch observed it on the following frame; Cemu 2.6 checks CPU texture changes at most once per GPU frame, so this does not establish same-frame visibility or hardware behavior. The main probe verifies these diagnostic quads and atlas sampling in the patched emulator, not the original main menu, animation patches, other rendering modes, or real hardware. A mismatch also runs a tiny white control atlas with the same Horizon vertices and logs their first position, UV, colour and CPU atlas texel to aid diagnosis.
