# RC 42 source qualification

The staged recipe applies patches 0136 through 0150 after the RC 38 source.
The last eight patches apply in order after RC 41 and produce engine tree
`9c843d3b8b82cb4ecc809eab90edd9c01b90287e`.
The corrected source built with Cafe Toolchain 1.1.0 and completed the
authenticated saved-world round below.

| Native build | Exact value |
| --- | --- |
| Port source commit | `b2e4a08296b2ba72ce492970f3628105c081705b` |
| Recipe SHA-256 | `1208ea18a43730ee9510f055a042efa94f0d481b032bffaf53c78c72e7973b5f` |
| RPX SHA-256 | `8188bbb334da899e9ecc995ebe1089c3713d8228ff110f4bb52a5d4ad02d98c1` |
| WUHB SHA-256 | `acf3132551b9a9b728e63afd1c7c73e19b8c16883a4f07a66b1e898d15734b3d` |
| Estimated Aroma code heap headroom | 12,762,880 bytes |

Release Cargo ran for 21m 33s with 32 jobs on Ben-Desktop. Native
relocations and both package hashes verified. The WUHB embeds the exact
standalone RPX once at byte 512.

| Increment | Verified so far |
| --- | --- |
| Authenticated benchmark reports and lifecycle | Twelve host tests cover runtime tool discovery, report recovery after instance switch and restart, ownership, launch cancellation and authorization changes while waiting to dispatch. The original routes and local foreground save path are reused. |
| GX2 axis-facing terrain runs | Both synthetic images matched per output. The fixture submitted 42 indices per display with `4r` versus 72 with the same back-face control `4b`. Host checks cover conservative selection, projection and range validation. |
| Village frame correlation | HZP3 sequence transport passed targeted native syntax and C/Rust checks without GX2 timing. A focused event-ordering test confirms that unrecorded rows cannot consume a coverage transition. The host analyzer checks coverage and timing against the capture and preserves the historical RC 33 results. |
| Cafe Minecraft SRV resolver | Native bridge compiled without static NetConf getter imports. Four host cases cover dynamic export failure, initialization failure, filtering and interface fallback. The exact bridge fixture reached zero assigned resolvers and exited in Cemu. |
| Cemu GamePad MCP capture | Eleven adapter tests pass with the scratch guard both off and on. GamePad observation reuses the existing capture path. Physical GamePad capture awaits a separate safe staging implementation and qualification. |

The first full Cafe build caught the protocol crate's `deny(unsafe_code)`
when the target-only DNS adapter was compiled. Patch 0150 scopes the
exception to the bounded FFI wrapper. The corrected package passed the
native build and package checks.

## Exact-package authenticated Cemu replay

The established forced interpreter reused the previously owner-approved
four-scope RC 26 disposable Codex grant and its HTTPS endpoint. Two fresh
Codex processes called Night before the same grant launched the saved
world. No new pairing or permission changes were required. The game
reported its first ready frame at 598.484 s in Cemu.

Authenticated option writes confirmed paired effective scales of 100%,
75%, 50% and 75%. The final 75% setting persisted after the save. Separate
1920×1080 TV and 854×480 GamePad captures showed terrain and HUD. Serial
confirmed the opt-in `4r` route. The session capture contains 75 playing
pairs, including 61 with terrain on both outputs, with up to eight
sections per output. Its SHA-256 is
`ecd629dc68ccf0aef994d60c3b676679b3eba84cf01de520efc9cf9a549ad661`.

`game_quit` completed the save barrier and restored Night. The saved pose
matched the starting fixture. AX delivered nonzero PCM and the GX2 bulk
heap recovered 234,880,920 free bytes. The existing grant read idle
benchmark status and the installation's empty report list. The title
exited through `coreinit.exit(0)`.

A second Cemu application process restarted the exact RPX with the same
disposable installation and Codex profile. Two new Codex processes called
Night again on that grant. Benchmark status returned idle and the title
again exited through `coreinit.exit(0)`.

The same grant started and cancelled `cafe-stationary` during loading.
The launcher reported `cancelled`, the normal save remained byte-identical,
and the selected installation's report list was unchanged. The app then
opened Night's local reports page. The next remote start received the
Home-page requirement. A fresh Cemu process can start the next run while
the remote return-to-Home fix is qualified.

The replay used distances 12/12 and the emulator's own dual-display
capture path. A distant plant differed from the RC 41 stills with a
different terrain section count. Matched control pixels and throughput,
actual benchmark execution and physical console restart remain open.

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
