# PowerPC HUD wrapper control

This CPU-only fixture extracts the compiler-emitted HUD presentation wrapper
from two assembled engine trees. The C driver uses bounded stubs for
foreground, presentation and GPU retirement. The original wrapper carries
its active-arena comparison over the plain call. The repaired wrapper
seals active arenas by control flow.

Build with the pinned Cafe toolchain and fresh output directory. Use the
original RC 59 engine `f5224f3d354acfdf9daccf2273249daa544580a0`.
The first repaired engine is `ea48074052cecdd38720528980056e64097df618`.
An engine containing the same wrapper correction also works with the
fixture's pinned assembly checks.

```sh
bash wiiu/diagnostics/ppc-hud-wrapper/build.sh "$original" "$repaired" "$scratch"
```

Run `hud-wrapper-cpu.rpx` with the [disposable Cemu runner](../../../tools/gx2_cemu.py)
in the interpreter and both recompilers. Each variant exercises 12,288 calls
per case across 192 epochs. Case 0 is active plain presentation, case 1 is
active grouped presentation, case 2 has no arena, and case 3 is an active
arena at background rejection. Arm 0 is the original and arm 1 is the repair.

The interpreter recorded zero errors. The pinned Cemu recompiler recorded
12,224 errors in Single and 12,160 in Multi, only in arm 0, case 0.
The ticket and output mask were correct in the first failure. The
active arena was left unsealed. Both modes passed the repaired arm
and the other cases. An emulator correction must pass both arms.

The pinned build reproduced RPX SHA-256
`3c589f086839274407ef744633d32078bc77768e33f2e47042bb889bd5c994af`.

See the [hosted and native reducer evidence](../../../docs/gx2/hud-retirement-reducer.md).
