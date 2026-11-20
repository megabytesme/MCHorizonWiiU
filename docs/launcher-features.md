# Horizon Night launcher actions

D-pad Up and Down move among enabled rows. A selects and B goes back by default.
The A/B setting swaps those controls immediately. GamePad touch triggers on
release over the same row. Dragging away, losing focus or losing input cancels
the gesture. The built-in character picker uses Left and Right when the system
keyboard is unavailable.

| Control | Actual effect | Availability and errors |
| --- | --- | --- |
| Start Game | Starts the pinned native client with the selected installation, offline identity, target and face-button mapping. Returns through the existing game save and resource-release barrier. | Requires a compatible preparation. A missing or stale selected target gives a startup error. |
| Profiles | Create an offline login identity, select it, rename its local display label, or remove the profile after confirmation. | Login identity is immutable and is the input to Horizon's existing offline UUID calculation. At least one profile is required. Saves are never deleted by profile management. |
| Worlds & servers / Local worlds | Lists save metadata under the selected installation, then resolves a chosen world as a direct launch target. | Night disables saves with missing ownership evidence, a different offline UUID, a lock, corrupt metadata or a compatibility warning. Native game menus handle recovery and upgrades. Direct selection does not edit any world file. |
| Worlds & servers / Saved servers | Add, edit, select or confirm removal of an endpoint. Start Game uses the client's standard direct multiplayer entry. | Validates names, hostnames, IPv4, IPv6 and ports before saving. Actual connectivity and authentication are handled by the native client. |
| Worlds & servers / Use game menu | Clears the direct target and lets the game open its usual title menu. | A failed preference write retains the previous target. |
| Version & resources / Installed resources | Shows the default directory and additional installed instances. Selection changes the exact resource directory and game folder handed to the game. | The hosted recipe pins the prepared version, schema and build input digests. Missing, incompatible and invalid preparations have distinct states. Cache installation and updates are pending. |
| Version & resources / About | Shows the upstream revision, client, patch and builder versions, application, architecture, preparation version and build identity embedded by the builder. | The build receipt identifies the RPX and WUHB hashes independently. What's New awaits verified release notes. |
| Settings / Swap A and B | Persists the swap and updates Night's Select and Back controls and the real game's Cafe face input. | A failed write retains the old mapping. |
| Settings / Swap X and Y | Persists the swap and remaps the real game's Cafe X/Y input for the session. | Night has no normal X/Y navigation actions. |
| Settings / GamePad touch | Persists whether touch can activate Night navigation. | D-pad and face buttons remain available. The original game owns its touch behaviour. |
| Settings / Reports | Opens compatible existing benchmark JSON under the current instance `benchmarks/` and displays recorded fields and measurements. | Malformed or unsupported reports give a clear error. Running benchmarks from Night and uploading reports are pending. |
| Agent Control | Shows that launcher pairing is unavailable. | No new listener or pairing service. Existing in-game authenticated Agent Control stays in the game. |
| Microsoft accounts | Shows that Microsoft sign-in is unavailable. | Microsoft/Xbox/Minecraft protocols, ownership, account management, sign-out and protected refresh storage are pending. No launcher file contains authentication material. |
| Close Horizon | Exits via the shared Cafe lifecycle. | Game teardown remains authoritative while a session is running. |

The standalone Night build remains a diagnostic shell with the Start Game
action unavailable. The emulated demo and startup-error actions exist only in
the explicit fixture. See [storage and ownership](../wiiu/pregame/INTEGRATION.md)
and the [hosted recipe](../catalog/hosted-v2.json).
