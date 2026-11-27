# GX2 solid terrain display-list fixture

The fixture uses ordinary terrain state and draws 174 section submissions
on each display. TV and GamePad use different projections and separate
geometry and command buffers. Each display retains at most 64 KiB for the
list. The fixture uses synthetic geometry, atlas and lighting and compares
all RGBA bytes with the [explicit readback stage](../gx2-explicit-readback).

The stock, list replay, rejected key with direct fallback, and repeat list
arms must agree on both displays. A deliberately wrong camera and a reversed
equal-depth draw order must change pixels. The guard rejects stale projection,
resource, revision and foreground identities. An unsubmitted or unretired
list cannot be overwritten. Teardown waits for explicit positive retirement.

Build from a port checkout with an assembled stock terrain source and the
pinned CafeGLSL and devkitPPC tools. Pass a fresh scratch output directory.

```sh
bash wiiu/diagnostics/gx2-solid-display-list/build.sh "$assembled_source" "$scratch"
```

Use [the Cemu fixture runner](../../../tools/gx2_cemu.py) with
`solid-list.rpx` in a fresh interpreter lab. Require both capacity rows,
four verified retirements, all 12 distinct image rows, and the final
`GX2 solid list fixture result=PASS` followed by `coreinit.exit(0)`.

The RC60 interpreter run recorded 32,736 bytes per list and a maximum
192 bytes for one draw. All four exact arms matched every TV and GamePad
RGBA byte. The camera control changed 204,880 TV and 32,595 GamePad pixels.
Reversing draw order changed 34,970 TV and 21,359 GamePad pixels. The
accepted RPX SHA-256 is
`96000669ad40005c77617e11000d16ea1fdf841dd8408d8d9ae6113c13347a00`.
Both capacity rows were below the 65,536-byte cap. This experiment checks
command encoding, output parity and retirement guards in the interpreter.
Hosted cache eligibility and hardware frame cadence remain separate.
