# Native game build inputs and migration

The launcher worktree starts at `5a8360e3ee5f9c5c040a13325666459fee480826`.
The native baseline includes these 12 local commits over the tracking ref
`origin/codex/wiiu-platform-api` at `e1e2a43c2bbffc6a7bf738786d1d6cdb50926ced`.

| Commit | Native baseline |
| --- | --- |
| `2a062db6d66` | GamePad screenshots |
| `9f05abae8fc` | HUD depth and effects |
| `339e0880590` | Terrain fog |
| `94613348320` | Audio decoding memory |
| `a1554810a89` | Worldgen facts memory |
| `2b88c7a874c` | Active-session world lock |
| `e1a1dbb45a6` | Entity UUID seeding |
| `4a787152e00` | Terrain streaming memory |
| `b9f7c5b3cab` | Terrain save peak memory |
| `ba8ba8651a2` | Indexed GX2 terrain |
| `948f7b41760` | Authenticated LAN Agent Control |
| `5a8360e3ee5` | Region-file sector extension |

## HorizonSav1 inputs on Ben-Desktop

The physical build record lives at `%TEMP%/HorizonWiiU-region-save-20261119`.
`native-build-overlays.patch` captures these nine tracked overlays.
`build-input-sha256.txt` pins the scratch Cargo manifest, lock, app source,
build/link scripts and archive staging script. `toolchain.txt` fingerprints
the stage1 compiler, Cafe filesystem source, WUT library and TV size adapter.
For another build, copy the scripts and fix their source and target paths in
scratch. Their recorded commands point into the active checkout and shared
targets.

- `crates/horizon-client-hud/src/hud.rs`
- `crates/horizon-client-platform/src/logging.rs`
- `crates/horizon-client/src/lan/io_pool.rs`
- `crates/horizon-client/src/lan/mod.rs`
- `crates/horizon-core/src/prepared_files.rs`
- `crates/horizon-threads/src/jobs.rs`
- `wiiu/loading_app/src/lib.rs`
- `wiiu/loading_host/lib.rs`
- `wiiu/platform/Cargo.lock`

The game uses Cafe Rust 1.101.0-dev with LLVM 23.1.3 under
`/var/tmp/HorizonWiiU-rust-std-tls-20261111/build/x86_64-unknown-linux-gnu/stage1`,
WUT 1.9.2-1 and devkitPPC r51-1. Its offline Cargo build selects that patched
sysroot and the custom getrandom backend. Verify the compiler and its
filesystem, TLS and library patch provenance when reproducing the game
elsewhere. The scratch app source sets `/vol/external01/wiiu/apps/HorizonSav1`.
Linking uses the Winit TV-size adapter at
`%TEMP%/HorizonWiiU-winit-tv-size-empty-20261115.c`. Private prepared resources
are staged separately. The native build enables AX and omits the Cemu entropy
and session-lock wrappers.

| External Cargo overlay | Recorded source path |
| --- | --- |
| Winit 0.30.13 Cafe shim | `$scratch/HorizonWiiU-png-stage-probe-20261111/winit` |
| ring 0.17.14 | `$scratch/HorizonWiiU-ring-patched-touchtrace-20261111` |
| png 0.18.1 | `$scratch/HorizonWiiU-png-stage-probe-20261111/png-0.18.1` |
| ureq 2.12.1 | `$scratch/HorizonWiiU-touchtrace-20261111/ureq-trace` |

The same manifest uses tracked getrandom, wgpu-hal, naga and libmimalloc-sys
patches under `third_party/`. The physical run confirmed authenticated LAN MCP,
the 841-record terrain autosave, and persisted block edits. Save & Quit
encountered the separate `weather.dat_old` replacement issue assigned to the
game agent.

## Migration

1. Keep the launcher client, local recipe and bridge proof independent. Keep
   the current game branch and its overlays available for comparison. Record
   exact source, patch, overlay and toolchain fingerprints in a future game
   recipe with restricted references to private inputs.
2. After the game agent's branch is ready for review, move general changes
   upstream where suitable, keep Cafe platform code under `wiiu/`, and export
   reviewed Horizon-specific deltas as versioned patches over exact upstream
   commits. Split compiler/sysroot and dependency patches by pinned upstream
   project, preserve notices, and verify each assembly in a clean scratch tree.
3. Implement the fallible hosted gameplay adapter before building a combined
   RPX. Prove repeated play/return and resource teardown independently of the
   original candidate. Preserve the hardware-tested game MCP behavior.
4. Record separate build, Cemu and hardware results against exact RPX/WUHB
   hashes. Add catalog signing after trust roots and signature verification
   exist. Keep Minecraft data, manifests of private resources, credentials,
   pairing and saves local.
