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
  --name=Horizon_GX2_Scale_Diagnostic --short-name=GX2_Scale --author=MCHorizonWiiU
sha256sum "$build/gx2_world_scale_transition.rpx" "$build/HorizonGX2Scale.wuhb"
```

Run the RPX in forced-interpreter Cemu or put the WUHB in its own application
directory, `sd:/wiiu/apps/HorizonGX2Scale/`. The app exits automatically.
Capture the serial records from `GX2 transitions` through `GX2 scale
transition diagnostic done`. A 1080p TV and a standard GamePad yield
1440×809 and 641×360 world surfaces. The first direct WHB control checks
both outputs with the same shader and a CPU-filled texture. The six trials
then reuse the scaled target and alternate patterns to test fresh samples.
In forced-interpreter Cemu, both direct controls and all 24 trial readbacks
passed at those dimensions and the process exited successfully.

| Trial | Transition | What a pass indicates when the RC 26 arm fails |
| --- | --- | --- |
| 0 | RC 26. Restore WHB, then invalidate source color and texture. | A failing destination with a passing source reproduces the transition fault on first use. |
| 1 | Invalidate source color and texture while it is bound, then restore WHB. | Moving the same operation before rebinding is sufficient for this workload. |
| 2 | Invalidate color before restoring WHB and texture after restoration. | Separating the producer and consumer cache operations is sufficient for this workload. |
| 3 | Invalidate color and texture before restoring WHB, then wait for retirement. | The extra retirement wait changes the observed result. It also adds synchronization cost. |
| 4 | Invalidate the source, copy to an independent linear texture and sample that copy. | Sampling independent backing changes the result. The copy also changes layout and adds bandwidth and allocation. |
| 5 | RC 26 repeated with the opposite source pattern. | Correct output on one RC 26 arm and stale output on the other calls for repeat and reuse checks. |

`GX2 scale source` describes offscreen pixels. `GX2 scale output` describes
the corresponding composed pixels. The probe reads the destination first
so that copying the source for CPU access cannot fix its transition. Keep
all trial records on failure. A failed direct control calls for checking
basic shader and readback behavior. A failed source calls for checking the
offscreen draw and readback. If every row passes on hardware, the next
comparison must preserve the stock terrain draw and the original frame's
target state changes.

Cemu's implementation models texture invalidation by address. The RetroArch
Wii U blur path invalidates the source texture before rebinding its next
color target. These mechanisms motivate the variants. Physical results are
needed to identify which mechanism affects the hosted renderer.
[Cemu surface-sync handler](https://github.com/cemu-project/Cemu/blob/main/src/Cafe/HW/Latte/Core/LatteCommandProcessor.cpp) ·
[RetroArch GX2 passes](https://github.com/libretro/RetroArch/blob/master/gfx/drivers/gx2_gfx.c)
