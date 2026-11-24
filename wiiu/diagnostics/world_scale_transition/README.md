# GX2 world-target transition probe

This standalone app uses the production world-scale compositor from the
assembled source and the existing pixel fixture's pattern draw. It needs no
game resources. It opens no files and does not access Horizon settings,
saves, network services, accounts or certificates.

Build with the pinned WUT, devkitPPC and CafeGLSL. For the RC 26 comparison,
use assembled engine tree `1f405190dbff9428f2585cee478efc0d9fb0307c`.

```sh
powerpc-eabi-cmake -S wiiu/diagnostics/world_scale_transition -B "$build" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DHORIZON_WIIU_ASSEMBLED="$assembled_source" \
  -DCAFEGLSL_COMPILER_EXECUTABLE="$cafeglsl_bin"
cmake --build "$build" --parallel "$(nproc)"
wuhbtool "$build/gx2_world_scale_transition.rpx" "$build/HorizonGX2Scale.wuhb" \
  --name='Horizon GX2 Scale Visual' --short-name='GX2 Scale Visual' --author=MCHorizonWiiU
sha256sum "$build/gx2_world_scale_transition.rpx" "$build/HorizonGX2Scale.wuhb"
```

Run the RPX in forced-interpreter Cemu or put the WUHB in its own application
directory, `sd:/wiiu/apps/HorizonGX2Scale/`. The physical build presents
seven seven-second still windows and requests the Wii U Menu through ProcUI
when finished. It performs no surface copy or CPU readback.

A 1080p TV and a standard GamePad yield 1440×809 and 641×360 world
surfaces. Capture one output still per `GX2 scale still` serial record.
`reverse=0` expects red above blue and `reverse=1` expects blue above red.
The `SUBMITTED` line confirms all arms ran. Check the stills for pixels.
The first visual probe found that synchronizing before restoring WHB,
splitting synchronization, and waiting for retirement all preserved green
on physical HDMI. This follow-up holds synchronization constant and varies
the draw's depth state and the attachment order. A failed direct control
invalidates the physical comparison.

| Trial | Transition | What a pass indicates when the RC 26 arm fails |
| --- | --- | --- |
| 0 | Direct WHB draw of the CPU-filled pattern. | The control validates the shader, pattern and output. |
| 1 | RC 26. Restore WHB, then invalidate source color and texture. | A failure isolates the difference to the scaled path in this fixture. |
| 2 | Disable depth testing and depth writes for the pattern. | The offscreen draw passes with depth disabled. |
| 3 | Enable depth testing and writes with `ALWAYS` comparison. | Changing the depth comparison is sufficient. |
| 4 | Bind the color and depth attachments before the RC 26 clear. | Binding both attachments before clearing is sufficient. |
| 5 | Bind before clear and disable depth for the pattern. | The combination is sufficient. |
| 6 | RC 26 repeated with the opposite source pattern. | Compare the two RC 26 arms for stale output and reuse effects. |

If every row passes on hardware, repeat with stock terrain and the original
frame's target-state changes. For emulator-only source and destination
readbacks, use a separate build with `-DHORIZON_SCALE_CEMU_READBACK=ON`.
The original readback build passed all 26 Cemu controls and crashed during
the first TV readback on physical hardware. Keep it in Cemu.

Cemu's implementation models texture invalidation by address. The RetroArch
Wii U blur path invalidates the source texture before rebinding its next
color target. These mechanisms motivate the variants. Physical results are
needed to identify which mechanism affects the hosted renderer.
[Cemu surface-sync handler](https://github.com/cemu-project/Cemu/blob/main/src/Cafe/HW/Latte/Core/LatteCommandProcessor.cpp) ·
[RetroArch GX2 passes](https://github.com/libretro/RetroArch/blob/master/gfx/drivers/gx2_gfx.c)
