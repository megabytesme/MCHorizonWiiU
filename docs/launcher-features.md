# Horizon Night launcher feature and action matrix

D-pad Up/Down selects enabled rows. A confirms and B backs out with the
default mapping. A GamePad tap activates on release within the same target.
Dragging away, losing focus, or losing the device cancels the gesture.
Disabled rows have no controller or touch action. Core ownership and game
acceptance requirements are in [CORE.md](../wiiu/pregame/CORE.md) and
[INTEGRATION.md](../wiiu/pregame/INTEGRATION.md).

| Control or feature | Service and state | Error, cancellation and input | Verification and status |
| --- | --- | --- | --- |
| Fixture / Start demo | Starts the owned mock driver for the current offline profile. State is per session | Cancel drains preparation; startup failure shows an error. Standard A/tap | Host and Cemu pass |
| Native game launch | Planned real Cafe driver receives a resolved instance and typed profile | Start Game is disabled with a game-bridge explanation. No real launch action yet | Typed lifecycle exercised with mock. Real integration pending |
| Profiles / Player, Guest | Selects a validated built-in offline identity in memory and snapshots it at start | B/Back leaves selection intact. Standard A/tap selects | Host verifies request identity; Cemu selects Guest. Working |
| Microsoft account | Device-code, Xbox/Minecraft identity, entitlement and credential provider contracts | Disabled with provider status. An eventual request can expire, be denied, or be cancelled | Disabled action tested. Protocol integration pending |
| Saved accounts and sign-out | Future store exposes public profile fields and opaque IDs. A protected vault owns refresh credentials | No account-management rows yet. Sign-out will revoke the local session and erase its refresh record | Interface only. Session-only sign-in is the fallback if a vault is unavailable |
| Version & resources | Opens the version page and displays the compiled launcher version | B/Back returns to Home | Host navigation and Cemu capture. Working |
| Fixture / Built-in demo | Displays the active built-in resource option. Selecting the already active row keeps it selected | No writes or downloads. B/Back returns | Host and Cemu. Working |
| Installed game resources | The builder can check a private prepared-resource inventory. The runtime configuration accepts a directory, version and digest | Disabled in the standalone shell. A real driver must reject missing or incompatible assets | Builder inventory verification and host request tests. UI install or runtime selection pending |
| Local worlds and saved servers | Typed target identifies a resolved local world or server. Future service will list and persist entries under the instance | No world or server controls yet. The future selector must validate paths and addresses before play | Host request tests for local target. Game selector pending |
| Settings / A/B swap | Changes the actual shell accept/back mapping, with updated prompts and a launch snapshot | B/Back follows the current mapping. Standard A/tap toggles | Host and Cemu swap and restore. Working in memory |
| Settings / GamePad touch | Enables or disables launcher touch actions for the current launch | A/B/D-pad stay active. With touch disabled, use a button to re-enable | Host tests active and disabled touch. Working in memory |
| Settings persistence / X/Y | Future settings store loads and writes private preferences. X/Y needs a verified game action mapping | No X/Y control yet. Existing controls apply for the present process | Settings handoff snapshot tested. Durable store and X/Y pending |
| Fixture Settings / Test startup error | Fixture acquires a partial session, fails preparation and drains it | Cancel before failure, or dismiss the resulting error using A/B/tap | Host and Cemu. Diagnostic is visible only in the fixture package |
| Agent Control / Pair a PC | Launcher listener and generation-scoped tool registration interfaces. The game retains its existing MCP | Pair control is disabled with an unavailable status. B/Back leaves the page | Host and Cemu show the disabled row. Launcher authorization and pairing pending |
| Benchmarks and reports | Future session targets with a private report destination. Existing Cemu acceptance is a builder fixture | No benchmark or report UI yet | Pending real game sessions and private report handling |
| Build / About / What's New | Versions shows the compiled version. The builder stores source/recipe provenance and package hashes | About and release-note controls are absent until an exact release source is available | Compiled version and package verification work. About and What's New pending |
| Fixture Preparing / Cancel | Polls backend progress; unknown progress uses an indeterminate indicator | A/B/tap can cancel. The action locks while workers drain | Host verifies late ready is discarded. Cemu cancels after resources were acquired |
| Fixture session / Save & Quit | Starts the save or disconnect barrier. Returns after confirmed completion and resource drain | Ignore duplicate requests while saving. Retry is available after a failure on the same session | Host proves retry and worker sync. Cemu proves the mock ordering. Real game save/reopen pending |
| Fixture error / Return to Play | Clears the sanitized startup error after cleanup | A/B/tap dismisses | Host and Cemu pass |
| Back / Close Horizon | Back restores the previous Home action. Close exits through the app host | Available only on the relevant pages. OS exit settles active work first | Host tests exit by phase. Cemu guest exit passes. Physical return pending |
