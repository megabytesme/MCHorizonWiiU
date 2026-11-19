# Native Horizon Night handoff

The combined RPX starts in Horizon Night with one C owner of ProcUI and WHB.
The game loop and shell run on the same Cafe thread. The standalone recipe
retains the original entrypoint and the world-reopening instrumentation
remains a separate opt-in patch.

## Ownership and execution

1. The launcher checks the installed preparation manifest for Horizon 26.3.
   The previous instance directory remains the app root. The game opens its
   original settings and uses the offline profile selected in Night.
2. On Start Game, the shell releases its GX2 HUD and CPU vertex storage.
   The game constructs its original GX2 presenter, resource loader and native
   game session. The launcher retains its small logical view and preferences.
3. The game polls the same application ProcUI pump. The borrowed native loop
   dispatches the original VPAD state, calibrated touch, USB keyboard and
   mouse, and CafeOS keyboard results. On release it neutralizes input before
   the shared host acknowledges foreground release. It obtains current WHB
   surfaces again after reacquisition.
4. Horizon's normal world and server menus remain in the game's UI. Rendering,
   world generation, gameplay settings, and the existing authenticated LAN
   Agent Control stay with Horizon. B is interpreted by the active game while
   it owns input.
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

The [hosted recipe](../../catalog/hosted-v1.json) checks the exact stable game
tree after the archived workspace overlay. It adds the Horizon-source patch,
the port glue patch and the port-owned bridge source, then verifies the final
staged Git tree. Builder provenance records the shell source hashes separately.
Use the [builder runbook](../builder/README.md) for build and local test commands.

The fixed active app root is `/vol/external01/wiiu/apps/HorizonWiiU`. The
prepared cache is `.local/horizon-client/26.3` inside that root. The installed
manifest supplies the game version at startup. The original loader still
validates and reads the actual preparation. A private builder/resource
inventory can record complete file integrity for a given test or artifact.
Game settings, logs and saves belong to the app root. Use a fresh portable
Cemu SD directory and private lawful copies when testing.

## Integration acceptance

Validate real world entry, save completion, file flush and reopening the same
scratch world. Verify normal exit, resource failure, loading cancellation,
disconnect and a repeated session. Record the RPX and emulator hashes and
explicitly identify the game settings used. Confirm repeated allocator usage,
AX and input cleanup, HOME release and foreground reacquisition, and CafeOS
exit separately on a physical Wii U before deployment. The launcher feature
matrix records the outstanding launcher services.
