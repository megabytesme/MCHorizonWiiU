# Migration validation

The Night packages were built from `38ab6b8c13c724969d0eb2daefe9805050089719`.
The separate port branch starts at `6913bfeaf60ccfedd6c10e6e50010e8d0d669165`.
Its hosted-session baseline is preserved on the launcher branch at
`f4ac01d8915d09cfad7de6d0117246b0d4da5449`.

## Exact source and offline builds

The validated compatibility recipe has SHA-256
`31d859bcfcb6d5401ef83610202731a1421c93e3c738cfe43739cf1946dcc27c`.
Git regenerated the checked-in binary-safe patches and matched their hashes.
The clean assembly produced these trees.

| Source assembly | Git tree |
| --- | --- |
| Canonical port from Horizon `69fab597a6f0e8a6ce524d38d1b73e599e343034` | `e64ecd1968c0057168c2148eef46caf4d79d675c` |
| Opt-in saved-world reopen instrumentation | `251c45490e910f3f3be7cf5738abd65275ea99a3` |
| Stable game with the verified local build overlay | `af17b86aef2d1a2d1c2f0f7ef0f391cf579e64ac` |

The stable recipe used pinned Cafe Rust, devkitPPC, WUT and verified dependency
overlays. Cargo ran offline with the pinned lock. Early builds in different
absolute paths linked successfully with different bytes. The release
comparison gave separate host directories the same compiler-visible path
through Linux mount namespaces. Both game builds were resumed in their own
directories after the Windows system volume filled. The completed binaries
matched using `cmp`, and their independent build records matched package sizes
and hashes. Night and its fixture each also matched across two clean native
builds.

| Package | SHA-256 |
| --- | --- |
| `HorizonLauncher-Night.rpx` | `9fc6399b1be401486a6aefab5003aa5849d3b602952a8d39ff8d0d8cc6f7aeeb` |
| `HorizonLauncher-Night.wuhb` | `f68c8619ef17a6a47849253fc0100f0fbc2299076bfb046f38ddbb2d9aa9d367` |
| `HorizonLauncher-Night-Fixture.rpx` | `2441c8706360c55f3f681f688679319b2c3e4406b58d55f2a09ea9666d85ed0b` |
| `HorizonLauncher-Night-Fixture.wuhb` | `c4a7d97e09649c9d6ec3f28be7c5bc3a6221262dd8ea8a994f37e677fcac197d` |
| `HorizonWiiU-Game.rpx` | `65c1563b75ccc16b1bc205e020209cec8f092175630035f9de4443b5684d6993` |
| `HorizonWiiU-Game.wuhb` | `a421c156dd76c2e4b9cb714951b11ebb371db64096a1d014a569536c13018c41` |

Build commands and local artifact layout are in the
[builder runbook](../wiiu/builder/README.md). On Ben-Desktop the Night build
records, files and Cemu captures are under
`%TEMP%/HorizonWiiU-port-release/{production,fixture}`. The standalone game
records and files are under `$scratch/HorizonWiiU-port-development/game-final`.
The build receipts reference private resource inventories by digest only.

## Acceptance

| Check | Observed result |
| --- | --- |
| Host | Seven builder tests, eleven launcher tests, rustfmt and Clippy passed. The lifecycle tests cover save failure and retry, cancellation, focus rearm, OS exit and release after a worker syncs a scratch save. |
| Night, stock Cemu 2.6 | VPAD navigation, offline profile selection, A/B remapping, disabled services and guest exit passed. Ten TV and GamePad captures. The lifecycle trace had zero session allocations. |
| Night fixture, stock Cemu 2.6 | Mock start, save and drain, return, failed or cancelled startup, repeat launch and guest exit passed. Sixteen captures. |
| Game, stock Cemu 2.6 | The matching RPX stopped at the Cafe entropy startup guard. |
| Game, NSS-patched Cemu 2.6 | The same RPX reached `title.shown` in interpreter mode with a complete private prepared-resource cache. This is a title checkpoint. World entry, gameplay, real save/quit and reopening were outside the test. |
| Physical Wii U | Acceptance remains on the original game artifacts. The migrated outputs await hardware acceptance. |

Night receipts contain the emulator and RPX hashes, the lifecycle markers
and screenshot hashes. Standalone game receipts are in the sibling
`standalone-cemu` and `standalone-cemu-stock` scratch profiles. The emulator
patch and its limitations are documented in [Cemu NSS support](../wiiu/cemu/README.md).
