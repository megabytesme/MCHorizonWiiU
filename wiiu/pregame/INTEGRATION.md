# Connecting the native Horizon client

The launcher side owns the hosted lifecycle. The explicit Night fixture
runs `MockFactory`. The default package leaves game launch unavailable.
Connect a real `SessionFactory` after the native game
branch publishes a reviewed game pin and save/reopen result. Build and test
the bridge in a fresh checkout with scratch worlds and privately prepared
resources.

## Game bridge changes required

The standalone game recipe pins `1a0a2948c33d11744eb403247cb566b0aa3ee858`.
Use the assembled source to review and implement the following changes.

1. **Fallible hosted entry.** The existing `entry::main_with_cafe_host` uses
   `main_with_runner`, which can call `std::process::exit` on error. Expose
   a host entry that returns errors and separates app-wide entropy,
   allocator, logging, hooks and registration from per-session startup.
   Consume the explicit `LaunchConfig` for the prepared resources, game
   directory, profile and local-world ID or server endpoint. Keep
   process-global arguments, logging and paths consistent across sessions.
   If any existing path still depends on the working directory, resolve or
   scope it before workers start and keep private app data outside the
   resource cache. Inspect `crates/horizon-client/src/entry.rs` in the assembled game tree.
2. **Retain an owned startup driver.** Adapt `CafeLoadingSession` and
   `start_pending_game`. The driver must retain Loading and any
   `CafeGameSession` created while opening a world or server. Poll the
   existing loading work and report actual progress where available. Signal
   `Ready` only when the game can take over rendering and input. Review the
   readiness boundary around `world_loading_complete` and the first
   successfully presented frame. Stop and join partial work on cancel or
   failure, including the case where a worker completes after cancellation.
   See `crates/horizon-client/src/game.rs` in the assembled tree and the
   [existing host loop](../game-bridge/src/lib.rs).
3. **Keep one native host.** Reuse the presenter's borrowed GX2 frame/backend
   and input adapters. Use the launcher's one ProcUI pump and WHB
   initialization. Split preparation, tick, input and presentation out of the
   standalone Loading loop. Forward typed VPAD, touch, keyboard and mouse
   input from the host only to the owner of the foreground frame. Never
   interpret a regular in-game B press as a launcher quit request. On
   background release, neutralize devices and drain any foreground-dependent
   GPU work before acknowledging ProcUI. Reacquire dimensions and current
   surfaces after WHB restores them. Use one AX owner. Stop callbacks and
   free voices before release, and call `AXQuit` only if the session
   acquired AX. Review process and foreground heap budgets across multiple
   session cycles.
4. **Expose a durable save outcome.** `CafeGameSession::frame` returns
   `Ok(false)` while `pending_save` exists, and `shutdown` logs failures
   without returning them. Neither provides the completion signal for this
   host. Reuse the game's `begin_closing_save` and `poll_closing_save`
   barrier, and export an explicit pending/succeeded/failed result for the
   hosted Save & Quit path. Preserve the game's existing integrated-server
   stop, remote disconnect, metadata updates, player writes, chunk writes,
   synchronization and world lock. Route the game's own Save & Quit action
   to the hosted adapter so the launcher can take over after cleanup. The
   game layer must define a safe retry of a partially completed save.
   Retain the same state and world reservation after a failed save. Surface
   a typed error that the launcher can present without exposing credentials
   or paths. See `crates/horizon-client/src/game/saving.rs` in the assembled tree.
5. **Separate drain from successful save.** Stop and join loading, chunk,
   save, integrated server, networking, render upload and screenshot work.
   Release input capture and audio, drain GX2 work, then destroy session GPU
   resources and drop the game and its world reservation. Emit `poll_stop`
   success only when Drop is safe and callbacks can no longer touch the
   session. The launcher does not render Play before this point. On
   CafeOS exit, settle the current phase using `shutdown_for_exit`. A
   forced-exit error must be observable and still arrange cleanup. Keep
   the ProcUI save-ready notification distinct from world-save success.
   Check callback ordering, the time available to settle an OS exit, and
   HOME release/reacquire on hardware before enabling the real driver.

## Instance, account, settings and Agent Control

- Resolve local world IDs inside the explicit instance root. Validate
  server addresses and ports. Validate the prepared resource directory,
  expected game version and exact private manifest digest before use.
  `PreparedResources` carries the expected digest. The mock neither reads
  resource bytes nor asserts that a configured directory is verified. Reuse
  the prepared cache, and stage mutable settings and saves separately.
- The current UI selects two built-in offline identities. A future profile
  service supplies saved names and opaque account IDs. Resolve entitlement
  and deliver the selected game identity in memory. Use the existing
  device-code and refresh-vault interfaces. Keep request and refresh
  credentials out of launch args, logs, instance data and resource
  manifests. Use session-only Microsoft sign-in until a protected vault is
  available. Sign-out must revoke the launcher session and remove its
  protected refresh record.
- Load settings via the host before constructing `Launcher`. Persist input
  changes through a dedicated settings store with atomic replacement.
  Each session gets a settings snapshot. `touch_navigation` is a shell
  preference. Feed gameplay's own typed input and game settings to its
  adapter.
- Keep the hardware-tested game MCP in place. A future app-owned listener
  publishes launcher tools in its own scope. Once the game enters Live,
  it can register existing game tools under an opaque generation handle.
  Withdraw the registration and drain in-flight requests before game
  teardown. Preserve authenticated pairing across restarts in a reviewed
  store. Coordinate the transition to one endpoint before replacing the
  standalone transport. Standard MCP HTTP and optional Aroma discovery
  can follow the integrated driver.

## Packaging and acceptance

The [compatibility recipe](../../catalog/compatibility-v1.json) pins the
source and inputs for the standalone Night and game packages. The first
combined RPX needs a reviewed game revision, a declared launcher/game
compatibility pair, verified Cafe toolchain and overlays, and a native link
that uses one process host for both components. Private runtime resources
are prepared and verified separately from the packages.

Validate a real adapter first with a fresh private world. Start, edit an
observable block, Save & Quit, verify completion and release, reopen the
same world and verify the edit, then repeat. Run analogous local failure
and remote disconnect checks. Record the exact artifact hash and separate
host, Cemu and hardware acceptance. Physical saved-world reopening remains
with the native game investigation. This branch provides mock lifecycle
evidence and a standalone Night package.
