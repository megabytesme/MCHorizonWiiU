# Cafe native Loading application draft

This is a standalone WUT RPX driver for the existing Cafe Loading path. Its C
`main` calls the entropy anchor first, then owns `WHBProcInit`, `WHBGfxInit`,
their shutdown calls, and the WUT process loop. The Rust static library calls
`entry::main_with_cafe_host` with the existing `CafeLoadingSession` and
`Gx2LoadingPresenter`. No alternate menu or renderer is introduced.

While loading, Rust polls the session and VPAD on the main thread, consumes
each `VpadState::poll_with_evidence` sample/touch marker in the same iteration,
maps touch to the existing `TouchMapper` pointer events, and dispatches those
events immediately to `CafeLoadingSession`. Each focused frame also sends the
real `PadState` through `CafeLoadingSession::dispatch_pad_state`, which uses
Horizon's typed menu-button path; no keyboard events are synthesized. Terminal
VPAD errors, focus loss, and exit send a neutral pad state. After focus loss or
a terminal error, held buttons/triggers are ignored until a successful fresh
VPAD sample reports them neutral; cached/no-sample defaults and touchscreen
release evidence do not rearm controller input. WUT ProcUI acquire/release
callbacks inform Rust of focus changes. Touch capture is released on focus
loss; touch remains blocked until a fresh physical release after focus returns.
Rendering and VPAD polling pause while backgrounded. The existing presenter
draws both WHB targets after foreground acquisition and is dropped before
`WHBGfxShutdown` on normal returns.

Before constructing the presenter or entering Horizon, the driver sets its
working directory to `/vol/external01/wiiu/apps/HorizonWiiU`. The Cafe std
filesystem backend mounts the SD card at `/vol/external01` when it initializes
its FS client. A missing app root returns to C with an actionable `OSReport`
and still follows the normal WHB/GX2 shutdown path. Resource selection and
missing-resource reporting remain with Horizon's original options and loading
logic, including `--resources` and `--game-dir`. The app root is an explicit
install-layout contract: install the app at that SD path; resource data may be
provided through the normal Horizon options.

## Known upstream seams still missing

- **Loading-to-Game:** `CafeLoadingSession::status()` reports
  `ReadyForHandoff`, but `CafeRunOutcome` only returns `Finished` or the
  retained `Loading` session. There is no public operation to consume
  `Loading.loaded`/`menu.pending` and continue into the original `Game` loop.
  This driver therefore keeps the original title running until exit. It does
  not report Ready as a game handoff.
- **GamePad world handoff:** original Loading now receives actual VPAD button
  and trigger samples through its typed pad-state path. Analog sticks are
  carried in `PadState` but the startup menu currently consumes button edges;
  the eventual Cafe Game host still needs to poll and route movement/look input.
- **Entry error cleanup:** `entry::main_with_cafe_host` preserves the common
  entry logging/crash path, whose error arm calls `std::process::exit`. An
  error before or during host startup can therefore bypass C's explicit WHB/GX2
  shutdown and Rust destructors. A fallible entry seam that returns the same
  error instead of exiting is required for guaranteed native resource cleanup.
- **Background GPU resources:** WHB owns and reacquires the TV/DRC render
  targets; the driver does no drawing while its ProcUI release callback says
  background. The presenter retains its own HUD/panorama resources across that
  transition and refreshes target dimensions on the next prepare/draw. This
  lifetime has not been validated on Cafe or Cemu.
- **Build substrate:** this directory reuses `wiiu/bootstrap/prepare_wut_startup.sh`,
  `wiiu/package_rpx.cmake`, and `wiiu/loading_host/CMakeLists.txt`. It still
  requires a Cafe stage1 Rust compiler/sysroot, devkitPro WUT, Cafe GLSL tools,
  Cargo target dependencies, and scratch overlays/patches for the Horizon
  dependency graph and Cafe std. In particular, the full client currently
  needs the TLS-key virtualization patch in `wiiu/rust-std-tls`; the unpatched
  Cafe std reached by the latest isolated Cemu run aborts with `out of TLS
  keys`. That patch is under target validation. CMake passes WUT headers and
  PowerPC Cafe ABI flags into this directory and child presenter targets, but
  does not install or apply external compiler patches.

Cafe resource selection is already adapted in the original `Options::parse`:
it uses the relative `.local/horizon-client/<version>` default when
`current_exe()` is unavailable and still honors an explicit `--resources`
override. See commit `5d04ef9a5`.

## Build and review

From a devkitPro/WSL shell with the reviewed scratch Cafe compiler setup:

```sh
export HORIZON_WIIU_STD_RUSTC=/path/to/cafe-stage1/bin/rustc
export DEVKITPRO=/opt/devkitpro
powerpc-eabi-cmake -S wiiu/loading_app -B /tmp/horizonwiiu-loading-app -G Ninja
cmake --build /tmp/horizonwiiu-loading-app --parallel
```

The resulting RPX is
`/tmp/horizonwiiu-loading-app/horizon_wiiu_loading_app.rpx`. Native CMake
configure/link was validated with a prebuilt full-client archive. An isolated
Cemu launch entered the WUT host and original Horizon entry, logged
`Horizon 0.1.0`, then aborted in the unpatched Cafe std at TLS-key exhaustion.
This confirms RPX startup integration only; this run did not verify a title
frame. The TLS patch and a rebuilt full-client archive are required before the
next Cemu menu check. ProcUI focus lifetime and actual Cafe hardware behavior
remain unverified. Full acceptance also requires Loading-to-Game handoff and
Cafe Game movement/look input.
