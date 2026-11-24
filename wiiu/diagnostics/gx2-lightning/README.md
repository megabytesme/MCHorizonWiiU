# GX2 lightning fixture

Build with the pinned toolchain and an assembled tree containing the GX2
lightning pass.

```sh
bash build.sh ASSEMBLED_SOURCE BUILD_DIRECTORY
```

Run `lightning_fixture.rpx` in a disposable forced-interpreter Cemu
instance. Capture the two displays after `GX2 lightning visual ready`.
Expect one red-plus-blue region on the left of the TV and on the right of
the GamePad. The clear color is RGB (16, 24, 32). Sample the region's
center for approximately (116, 24, 132), and sample the opposite side and
the output center for the unchanged clear color.

A shared upload contains two overlapping bolts in front of an opaque
terrain quad and a yellow bolt behind it. Both front colors contribute only when
the pass uses source-alpha additive blending and keeps depth writes off.
The occluded bolt and the unchanged background check depth testing and
coverage. The opposite output projection reuses the same uploaded buffer.
The fixture redraws from one upload during its seven-second capture window
and performs no CPU surface readback.
