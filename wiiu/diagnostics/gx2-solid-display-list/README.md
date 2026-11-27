# GX2 solid terrain display-list fixture

The fixture uses ordinary terrain state and draws 174 distinct resident
section buffers on each display. TV and GamePad use different projections,
disjoint geometry and separate 64 KiB command buffers. Each solid block
is followed by two depth-tested, alpha-blended draws before the first
fence or readback. The near quad must blend and the far quad must stay
occluded. The fixture queues both outputs and both scan copies, then
uses the ordinary `WHBGfxFinishRender` call. Its submission timestamp
must advance and positively cover both command lists before the
[first readback fence](../gx2-explicit-readback).

The full sequence runs at the native scan sizes and again with the
gameplay scaler and its separate 1440×809 TV and 641×360 GamePad
color and depth targets. Each suite captures and compares all RGBA
bytes at 1920×1080 and 854×480 after its respective presentation path.

The initial and warm replay, rejected key with direct fallback, revoked
closed record and rebuilt replay must match the stock draw. Controls
force a wrong camera, reverse equal-depth draw order and disable the
tail depth test. The guard rejects stale projection, target size,
resource, revision and foreground identities. Both command arenas must
stay protected until a covering timestamp has retired.

Build from a port checkout with an assembled stock terrain source and the
pinned CafeGLSL and devkitPPC tools. Pass a fresh scratch output directory.

```sh
bash wiiu/diagnostics/gx2-solid-display-list/build.sh "$assembled_source" "$scratch"
```

Use [the Cemu fixture runner](../../../tools/gx2_cemu.py) with
`solid-list.rpx` in a fresh interpreter lab. Require eight capacity rows,
six paired retirement rows with `held=3 retired=3`, four closed-record
invalidations, all 36 image and trailing-state results, and the final
`GX2 solid list fixture result=PASS` followed by `coreinit.exit(0)`.

The RC64-source run recorded 32,736 bytes per list and 192 bytes for the
largest single draw. All 24 ordinary comparisons were byte-identical.
The scaled attachments used 11,378,688 bytes. Each fault changed pixels
on both displays and each pair retired under the ordinary finish ticket.
The verified RPX SHA-256 is
`29ecb1227d382798352f50e1630e5bc1d80b2a72d18a6e8f07393649854c2860`.

| Injected fault | Native TV / GamePad pixels changed | Scaled TV / GamePad pixels changed |
| --- | ---: | ---: |
| Wrong camera | 374,730 / 59,584 | 395,216 / 68,442 |
| Reversed equal-depth order | 34,970 / 5,671 | 34,970 / 5,724 |
| Wrong trailing depth test | 5,418 / 840 | 5,632 / 925 |

The tail rebinds its shaders, vertex inputs and blend/depth state. The
fixture can detect changes to the underlying color and depth through
that tail. Gameplay cache invalidation and hardware timing still need
qualification. Each arm paints an explicit background on the bound
attachments to isolate a repeated GamePad clear artifact reproduced in
Cemu.

The earlier RC60 fixture used repeated handles and passed 12 image
comparisons. Its RPX SHA-256 remains
`96000669ad40005c77617e11000d16ea1fdf841dd8408d8d9ae6113c13347a00`.
[RC60 fixture source and evidence](https://github.com/megabytesme/MCHorizonWiiU/tree/8d1e07963fb/wiiu/diagnostics/gx2-solid-display-list)
