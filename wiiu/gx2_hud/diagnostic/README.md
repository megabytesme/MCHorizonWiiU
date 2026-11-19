# GX2 HUD diagnostic

This separately built RPX tests the GX2 HUD presenter using synthetic pixels.
It checks atlas upload and patch, alpha ordering, additive and invert,
preview depth and scissor, vignette filtering, item glint wrapping and mask,
the End Portal layer sum, and CIT glint ordering, sampling and blending.
It draws the same test geometry to TV and GamePad and samples each output.

With devkitPro/WUT, CMake, Ninja and CafeGLSL installed, from the project root in
WSL (adjust the tool paths for the machine):

```sh
CAFEGLSL_ROOT=$workspace/HorizonWiiU-toolchain/build-async-default/deps/cafeglsl-1.0.0-rc1-linux-x86_64 \
HORIZON_WIIU_HUD_DIAG_BUILD_DIR=$scratch/HorizonWiiU-hud-diagnostic/build \
bash wiiu/gx2_hud/diagnostic/build.sh
```

The generated `horizon_gx2_hud_diag.rpx` uses `GX2CopySurface` to a
`GX2_TILE_MODE_LINEAR_SPECIAL` surface. Cemu 2.6 reads this copy back.
Set `<AsyncCompile>false</AsyncCompile>` in the isolated Cemu profile so its
first frame can draw. Both outputs must use a 16:9 target.

## Physical depth comparison

1. With the console powered off, copy the two prepared folders
   `HorizonWiiUHudBaseline` and `HorizonWiiUHudDiag` into the SD card's
   `wiiu/apps` directory. Use those new app folders alongside the regular
   installation.
2. Launch `HorizonWiiUHudBaseline.rpx` from the homebrew menu. Photograph
   TV and GamePad while the sample grid is on-screen, then let the app exit
   after 15 seconds. Repeat with `HorizonWiiUHudDiag.rpx`.
3. Compare the second row on both outputs. The expected left depth region
   is red with gray outside its scissor. The next depth region is blue until
   the green overlay covers its right side and extends outside the scissor.
   The first row provides a control for basic color drawing.
4. If written, keep `hud-diagnostic-status.log` from each test app folder.
   The asset-free fixture has no saves. On hardware, use the visible grid
   to judge the draw because the earlier GamePad readback allocation failed.

The baseline fixture omits the two output-restoration calls immediately
after the depth clear. The pair uses the same pre-portal fixture and both
versions passed the Cemu pixel test. Comparing
their displayed grid on the console isolates the state-restore change.

## Recorded Cemu 2.6 run

The additive-blend diagnostic built on 2026-11-07 to RPX SHA-256
`5e625dc1e5a197ad4068498c1f54fbac7db9d1fb11b6d776e522d86d94ed3384`.
An isolated patched Cemu 2.6 run with synchronous shader compilation read back
`(153,102,76,191)` for additive `(102,51,25,128)` over the grey
`(51,51,51,63)` background, and `(0,0,255,255)` from the following alpha run.
All existing texture and transparency samples also passed. This is a diagnostic
fixture, not a live menu, and hardware has not been tested. Evidence is under
`$scratch\HorizonWiiU-hud-additive` on the build machine.

The expanded RGBA diagnostic built on 2026-11-07 to RPX SHA-256
`36811cc0fbc7c9f35a6ac7461ecd158215937ceb5ecf3589191e56e4ae929aa8`.
An isolated patched Cemu 2.6 run with synchronous shader compilation reported
`readback=pass`: texture red `(255,0,0,255)`, texture green `(0,255,0,255)`,
texture blue `(0,0,255,255)`, and the transparent sample matched the background
`(51,51,51,255)`. The original vertex-tinted red and green checks also passed.
This checks fully transparent discard, not partial-alpha blending or real hardware.
A separate temporary build with red and green component selectors deliberately
swapped failed the new texture checks while the original tinted checks passed.

The official portable Cemu release was extracted in the temporary diagnostic
folder, with its own `portable/sdcard` and settings. The existing user SD and
Cemu installation were not used for the test. With the SD status directory
already created, this was the exact PowerShell launch:

```powershell
$base = "$env:TEMP\HorizonWiiU-hud-diagnostic"
Start-Process -FilePath "$base\cemu\Cemu_2.6\Cemu.exe" -ArgumentList @('-g', "`"$base\build\horizon_gx2_hud_diag.rpx`"") -WorkingDirectory "$base\cemu\Cemu_2.6" -WindowStyle Hidden -PassThru
```

The 2026-11-07 diagnostic RPX had SHA-256
`3C885E77FE414C79985BA67F0C0F11ED4BCDD908ED5E4CEA4D8657DFDFCB2B78`;
Cemu identified its module checksum as `0x40bb9ddf` and logged title exit 0.
The isolated SD status reported `readback=pass`, 1920x1080, source/destination
tile modes 4/16, and red `(255,0,0,255)`, green `(0,255,0,255)`, and background
`(51,51,51,255)` samples. The status and Cemu log were preserved in the temp
folder as `readback-pass-status.log` and `readback-pass-cemu.log`.

In the earlier run (RPX SHA-256
`9E0F3B52E2FCACD94177E11A3296A73A32B6DDC460941417C88B550B3402A253`,
Cemu checksum `0xb490389a`), the user directly observed a grey background with
red and green boxes. That version's linear-aligned readback returned all zeroes;
its original evidence is `first-run-status.log` and `first-run-cemu.log` in the
same temp folder. Neither run demonstrates actual Horizon `HudBatch` integration,
menu navigation, GamePad input, or execution on real hardware.
