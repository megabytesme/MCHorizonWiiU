# GX2 solid terrain display-list fixture

The fixture uses ordinary terrain state and draws 174 distinct resident
section buffers on each display. TV and GamePad use different projections,
disjoint geometry and separate 64 KiB command buffers. Each solid block
is followed by two depth-tested, alpha-blended draws before the first
fence or readback. The near quad must blend and the far quad must stay
occluded. The [explicit readback stage](../gx2-explicit-readback)
compares all RGBA bytes on both displays.

The initial and warm replay, rejected key with direct fallback, revoked
closed record and rebuilt replay must match the stock draw. Controls
force a wrong camera, reverse equal-depth draw order and disable the
tail depth test. The guard rejects stale projection, resource, revision
and foreground identities. Each replay must positively retire before its
arena can be rewritten.

Build from a port checkout with an assembled stock terrain source and the
pinned CafeGLSL and devkitPPC tools. Pass a fresh scratch output directory.

```sh
bash wiiu/diagnostics/gx2-solid-display-list/build.sh "$assembled_source" "$scratch"
```

Use [the Cemu fixture runner](../../../tools/gx2_cemu.py) with
`solid-list.rpx` in a fresh interpreter lab. Require four capacity rows,
six verified retirements, both closed-record invalidations, all 18 image
and trailing-state results, and the final
`GX2 solid list fixture result=PASS` followed by `coreinit.exit(0)`.

The RC62 interpreter run recorded 32,736 bytes per list and 192 bytes
for the largest single draw. All ordinary arms matched both displays.
The wrong camera changed 374,730 TV and 59,584 GamePad pixels. Reversing
order changed 34,970 and 5,671. Forcing the tail depth comparison changed
5,418 and 840. The verified RPX SHA-256 is
`2f8585710766668e09977e522dc9c51b2cbfbfd6624bbf70111ac31d214bb4b6`.

The tail rebinds its shaders, vertex inputs and blend/depth state. The
fixture can detect changes to the underlying color and depth through
that tail. State overwritten by those rebinds, real scaled attachments
and pair-wide retirement still require checks. Each arm paints an
explicit background on the bound attachments to isolate a repeated
GamePad clear artifact reproduced in Cemu.

The earlier RC60 fixture used repeated handles and passed 12 image
comparisons. Its RPX SHA-256 remains
`96000669ad40005c77617e11000d16ea1fdf841dd8408d8d9ae6113c13347a00`.
[RC60 fixture source and evidence](https://github.com/megabytesme/MCHorizonWiiU/tree/8d1e07963fb/wiiu/diagnostics/gx2-solid-display-list)
