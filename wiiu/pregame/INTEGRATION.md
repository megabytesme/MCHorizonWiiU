# Native Horizon Night handoff

The combined RPX starts in Horizon Night with one C owner of ProcUI and WHB.
The game loop and shell run on the same Cafe thread. The standalone recipe
retains the original entrypoint and the world-reopening instrumentation
remains a separate opt-in patch.

## Ownership and execution

1. Night checks the selected installation's preparation against the hosted
   catalog and snapshots the selection before handing over. The game reads
   its original settings and uses the selected profile's immutable offline
   identity. A renamed display label never changes the game's login name.
2. On Start Game, the shell releases its GX2 HUD and CPU vertex storage.
   The game constructs its original GX2 presenter, resource loader and native
   game session. The launcher retains its small logical view and preferences.
3. The game polls the same application ProcUI pump. The borrowed native loop
   dispatches the original VPAD state, calibrated touch, USB keyboard and
   mouse, and CafeOS keyboard results. On release it neutralizes input before
   the shared host acknowledges foreground release. It obtains current WHB
   surfaces again after reacquisition.
4. The bridge uses the pinned client's `--quickPlaySingleplayer` or
   `--quickPlayMultiplayer` options when Night selects a target. It rechecks
   local world metadata, ownership and availability at handoff. The native
   world and multiplayer paths still own entry, connections and shutdown.
   Night passes A/B and X/Y preferences as session-only Cafe input mappings.
   Ordinary in-world presses remain game input.
5. The original Save & Quit operation requests the hosted close barrier.
   Pending saves keep polling and a failed attempt retains its session and
   shows an error. A retries the retained save stage. Persistent writer or
   server errors can still require OS exit, which records the failure.
   Game shutdown clears input and closes the session. Rust drops Game, joins
   the screenshot encoder and loader, and drops the borrowed presenter before
   the next launcher frame. Night rebuilds its HUD after it regains foreground.

Loading can be cancelled with B during the initial resource load. Quitting
from the original title also returns to Night. The host handles a CafeOS exit
separately and reports any save failure on that path. The hosted entry
does not use the standalone process-exit error branch. Mod hooks, logger
and other entry setup are installed once, with the normal resource and
settings paths used during each new game session.

## Pinned build and data

The [hosted recipe](../../catalog/hosted-v2.json) checks the exact stable game
tree after the archived workspace overlay. It adds the Horizon-source patch,
the port glue patch and the port-owned bridge source, then verifies the final
staged Git tree. Builder provenance records the shell source hashes separately.
Use the [builder runbook](../builder/README.md) for build and local test commands.

The app root is `/vol/external01/wiiu/apps/HorizonWiiU`. The default game
instance uses that root. Additional instances are immediate directories under
`instances/` and each uses `.local/horizon-client/26.3` under its own root.
The discovery check compares the preparation schema, exact version, client
and server jar digests, and exporter digest with the compatibility pin. It
checks sentinel files and directories before the game's full load. Use the
builder's private inventory for a whole-cache hash check. Files on disk
provide preparation information, independent of account ownership.

Night keeps global settings in `launcher/settings.0.json` and
`launcher/settings.1.json` under the app root. It writes and flushes a
`.pending` file for the inactive slot, removes its prior destination, then
renames to an absent destination. It selects the greatest valid committed
generation on startup. Interrupted pending files are ignored. Schema 1
`launcher/settings.json` is read when no valid generation exists, and the
next successful edit writes schema 2. Unreadable storage or a newer schema
blocks writes and retains the readable configuration. The file contains
offline identities, preferences and configured endpoints. No credentials
are stored. Existing worlds stay with their instance, and world selections
are transient. Night can only fast-launch a world with a matching saved
offline UUID and compatible, unlocked metadata. Other saves can use the
game's own menus for review and recovery.

## Integration acceptance

Validate real world entry, save completion, file flush and reopening the same
scratch world. Verify normal exit, resource failure, loading cancellation,
disconnect and a repeated session. Record the RPX and emulator hashes and
explicitly identify the game settings used. Confirm repeated allocator usage,
AX and input cleanup, HOME release and foreground reacquisition, and CafeOS
exit separately on a physical Wii U before deployment. The launcher feature
matrix records the outstanding launcher services.
