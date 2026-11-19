# Native Wii U pre-game shell

Horizon Night is the launcher's built-in presentation on the TV and GamePad. Use D-pad Up/Down
to navigate, A to select and B to return with the default input mapping.
The GamePad also accepts a tap on a visible enabled action. Home includes
Play, Profiles, Version & resources, Settings and Agent Control. Play is
unavailable until the game bridge is ready. The explicit fixture package
includes Start demo, Save & Quit, and a partial-startup cleanup diagnostic.

See the [launcher core](CORE.md), [real-client integration requirements](INTEGRATION.md),
and [builder instructions](../builder/README.md). The [first prototype](PROTOTYPE.md)
and the two visual candidates from the Cemu comparison are preserved separately.

## Architecture decision

Use one CafeOS application with an outer host and a switchable launcher/game
session. The MIT-licensed MCHorizonUWP implementation at local revision
`39ab7f19bff3856df0df3d4a05bbb4b9b4ef947b` prepares the game from XAML, switches
presentation in the same CoreWindow, and runs Horizon on a worker. When the
game bridge returns, the host dispatches cleanup and restores the launcher.
The Wii U shell uses the same ownership boundary with GX2 and the ProcUI
pump on Cafe's main thread.
[UWP account and shell README](https://github.com/megabytesme/MCHorizonUWP/blob/39ab7f19bff3856df0df3d4a05bbb4b9b4ef947b/uwp/pregame/README.md),
[XAML preparation](https://github.com/megabytesme/MCHorizonUWP/blob/39ab7f19bff3856df0df3d4a05bbb4b9b4ef947b/uwp/pregame/host/XamlShell.cpp#L3538-L3551),
[UWP App.cpp](https://github.com/megabytesme/MCHorizonUWP/blob/39ab7f19bff3856df0df3d4a05bbb4b9b4ef947b/uwp/host/App.cpp#L1011-L1078).

A two-title design could use `SYSLaunchTitle`. WUT declares that the call
requests exit through ProcUI and starts the specified title after the current
one exits. Caller IDs describe identity, and `SYSRelaunchTitle` restarts the
current title. A pair of titles would need verified installed title IDs,
explicit reverse launch, a fresh launcher process and non-secret resume data.
The APIs provide no suspended-launcher return contract. WUT's legacy HBL
exception requests a relaunch explicitly and needs its own device test.
[WUT title launch](https://github.com/devkitPro/wut/blob/v1.9.2/include/sysapp/launch.h),
[caller IDs](https://github.com/devkitPro/wut/blob/v1.9.2/include/sysapp/title.h),
[WHB process wrapper](https://github.com/devkitPro/wut/blob/v1.9.2/libraries/libwhb/src/proc.c).

- The C host owns one ProcUI pump and one WHB GX2 initialization. On
  `RELEASE_FOREGROUND` it suspends the active session and acknowledges via
  `ProcUIDrawDoneRelease`. It skips input and drawing in the background and
  requires a fresh neutral VPAD sample on reacquire. WHB's callbacks drain GX2
  and release or reconstruct scan, color and depth targets. The host obtains
  target pointers anew on each frame.
  [ProcUI](https://github.com/devkitPro/wut/blob/v1.9.2/include/proc_ui/procui.h),
  [WHB GX2](https://github.com/devkitPro/wut/blob/v1.9.2/libraries/libwhb/src/gfx.c).
- WHB uses the foreground heap for scan buffers and MEM1 for color/depth.
  Ordinary HUD texture and vertex storage uses MEM2 via the default heap. The
  prototype keeps a small reusable renderer and UI batch and drops the 64 KiB
  dummy session allocation before the first frame back at the menu. Shutdown
  destroys the HUD, shuts down GX2 and ProcUI, then returns through WUT startup.
  [WHB heaps](https://github.com/devkitPro/wut/blob/v1.9.2/libraries/libwhb/src/gfx_heap.c),
  [native host](../host/cafe_host.c).
- The prototype acquires no AX or network resources. The production audio
  bridge acquires AX only when `AXIsInit` is false. The owning session must stop
  audio callbacks, release voices/buffers, call `AXQuit` only if it owns AX, and
  close session sockets and join network workers. The existing Cafe Loading
  driver already owns WHB and ProcUI. A future adapter must borrow the outer
  host's platform services and route the full typed VPAD state.
  [WUT AX](https://github.com/devkitPro/wut/blob/v1.9.2/include/sndcore2/core.h),
  [audio bridge](../audio/audio.c), [Loading driver](../game-bridge/main.c),
  [VPAD bridge](../platform/src/vpad_bridge.c), [GX2 Loading host](../loading_host/loading_host.c).

## Session, account and MCP boundaries

The session factory receives a typed configuration snapshot and returns an
owned driver. The host polls the adapter through preparation, live gameplay,
save/disconnect completion and resource drain. The launcher shows a retry
when saving fails and holds the same session. It returns to Play after the
driver quiesces and drops. The Cafe fixture exercises this protocol with a mock session. See the [session contract](CORE.md) and
[game bridge changes](INTEGRATION.md).

The account traits cover offline name validation, device-code initiate,
poll and cancel, account listing and selection, Minecraft entitlement,
protected refresh storage and sign-out. UI receives a verification URL,
user code and polling timing. The provider retains private polling credentials
and identities through opaque in-memory types. Launch and profile records
carry identifiers and public display fields. Keep sign-in session-only if
secure persistence is unavailable. Microsoft/Xbox/Minecraft transport and
account, world and server menus remain future work. CafeOS protected storage
support is unverified. [Account contracts](src/accounts.rs).

The future app host would own one MCP listener and its authenticated PC
pairing. Expose launcher tools in the menu and publish existing gameplay tools
with a session-scoped dispatch handle only while a game is live. Withdraw these
tools, reject stale handles and drain requests before destroying the session.
Preserve the physically verified LAN game MCP behavior and validate pairing
persistence across restarts. Standard MCP HTTP and an optional Aroma discovery
companion can follow. Normal launches must work on their own. This prototype
opens no endpoint.

The Cemu fixture verifies emulated VPAD and both displays through guest
`coreinit.exit(0)`. The emulator process can stay open after the guest exits.
Cafe HOME transitions and foreground heap reclaim, actual controller timing,
Aroma launch/return, AX, and real shutdown require hardware acceptance.
