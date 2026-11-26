# RC 42 source qualification

The staged recipe applies patches 0136 through 0148 after the RC 38 source.
The last six patches apply in order after RC 41 and produce engine tree
`9d7e310c2b2ba9dbbe16bd432a810dec9c4e00ef`.
The package and exact Cemu gameplay run are pending.

| Increment | Verified so far |
| --- | --- |
| Authenticated benchmark reports and lifecycle | Ten host tests cover runtime tool discovery, report isolation, ownership, launch cancellation and revocation before foreground dispatch. The original routes and local foreground save path are reused. |
| GX2 axis-facing terrain runs | Both synthetic images matched per output. The fixture submitted 42 indices per display with `4r` versus 72 with the same back-face control `4b`. Host checks cover conservative selection, projection and range validation. |
| Village frame correlation | HZP3 sequence transport passed targeted native syntax and C/Rust checks without GX2 timing. Thirty host GX2 tests cover the report analyzer and original trace contracts. |
| Cafe Minecraft SRV resolver | Native bridge compiled without static NetConf getter imports. Four host cases cover dynamic export failure, initialization failure, filtering and interface fallback. The exact bridge fixture reached zero assigned resolvers and exited in Cemu. |
| Cemu GamePad MCP capture | Eleven adapter tests pass with the scratch guard both off and on. GamePad observation reuses the existing capture path. Physical GamePad capture awaits a separate safe staging implementation and qualification. |

## Integrated tests

Build with the locked Cafe 1.1.0 toolchain and verify the RPX, WUHB,
imports, relocations and code-heap estimate. Use the established NSS
interpreter for independent display evidence. Test the exact package
through Night, an authenticated local world, a live 100 → 75 → 50 → 75
settings cycle, both display captures through supported paths, save and
reopen, then an isolated benchmark. Compare `4r` against `4b` with
matching section counts and verify visible terrain before the Cemu dwell.

Use the existing 174-section physical pose and stock Village route for
performance qualification. Keep paired timing, combined flips, visibility,
save integrity and GX2 memory recovery in the evidence.
