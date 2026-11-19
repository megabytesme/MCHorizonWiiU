# Preserved first launcher checkpoint

The first native launcher and builder remain on the local
`codex/wiiu-launcher-prototype` branch at
`e1232f76313b13de673e814a57c393864385067f`. The native launcher implementation
was committed as `a79f665f2590ce3af294c0e03e8772f77c3f786a`.
Use the `wiiu/pregame/README.md` and `wiiu/builder/README.md` at that commit to
rebuild the checkpoint. The current branch advances the shared core for the
two alternative themes.

The checkpoint builder used a clean Linux Git checkout on Ben-Desktop, Cafe
stage1 Rust, WUT 1.9.2-1 and devkitPPC r51-1. CMake/Ninja configured cleanly
and an incremental build had no work to do. Two independent build directories
produced these byte-identical packages.

| Package | SHA-256 |
| --- | --- |
| `horizon_wiiu_pregame.rpx` | `e42ae0e5e892d77f6d8724ed81e563af4e914a529090339bd283c132661401ae` |
| `horizon_wiiu_pregame.wuhb` | `216fa15eff239240c03923631ab2c9f314aaebb0fe8f3fa4a3906d1bd0119114` |

The local artifacts and build record are in
`%TEMP%/HorizonWiiU-pregame-foundation/artifacts`. The stock Cemu 2.6 fixture
used its own portable directory at
`%TEMP%/HorizonWiiU-pregame-foundation/checkpoint-cemu`. Emulated VPAD opened
the dummy session, returned to the menu and exercised the startup error.
The guest freed the session on return and the partial allocation on failure.
After Close Horizon the guest reported HUD, GX2 and ProcUI cleanup and exited
through `coreinit.exit(0)`. The fixture captured TV and GamePad images for
menu, session and error. Physical hardware was outside the run.
