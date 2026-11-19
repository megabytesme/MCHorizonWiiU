# Port-only structure and releases

The migration follows the MCHorizonUWP port-only checkout at
`39ab7f19bff3856df0df3d4a05bbb4b9b4ef947b`. Its builder UI and tests are
separate from selection, catalog verification, source assembly and build
execution. The UWP bridge also keeps app-local directories separate from
runtime resources when entering the game. The Wii U port uses a CLI and
CafeOS-native services for those boundaries.
[UWP builder](https://github.com/megabytesme/MCHorizonUWP/tree/39ab7f19bff3856df0df3d4a05bbb4b9b4ef947b/uwp/builder),
[UWP catalog](https://github.com/megabytesme/MCHorizonUWP/blob/39ab7f19bff3856df0df3d4a05bbb4b9b4ef947b/docs/catalog.md),
[UWP candidate export](https://github.com/megabytesme/MCHorizonUWP/blob/39ab7f19bff3856df0df3d4a05bbb4b9b4ef947b/tools/prepare_candidate.py).

| Area | Implemented | Later |
| --- | --- | --- |
| `catalog/` | Independent builder, client, patch and toolchain pins. SHA-256 manifests and exact Git trees | Publisher signing and verification, update trust |
| `patches/<revision>/` | Two binary-safe Horizon deltas and a separate diagnostic patch | Smaller reviewed upstream submissions and formal patch releases |
| `tools/` | Patch regeneration, Cafe input inventory, public-tree and native packaging checks | Publisher candidate automation |
| `wiiu/builder/` | Reusable Python core, source assembler, native builder, thin CLI and host tests | Desktop front end and toolchain provisioning |
| `wiiu/pregame/` and `wiiu/host/` | Official Night UI, shared ProcUI/GX2 host and borrowed game renderer | Physical foreground and repeated-world acceptance |
| `wiiu/game-bridge/` | Original standalone Cafe entry, fallible hosted entry and explicit save barrier | Physical and repeated-session validation |
| Wii U platform modules | Tracked renderer, input, audio, networking, filesystem, entropy and standard-library adapters | Independently distributed dependency snapshots after review |
| `.github/workflows/` | Read-only compatibility and host checks | Release and hardware acceptance automation |

The port worktree starts an independent history. The original full-fork game
worktree, launcher foundation and both earlier visual candidate branches
remain available. The port assembly restores paths expected by the full
Horizon workspace. In particular `wiiu/game-bridge/` installs its pinned
game files at `wiiu/loading_app/` in the reconstructed source. The official
Night host remains separately buildable from the port checkout.

## Current release tuple

| Component | Version |
| --- | --- |
| Builder CLI and core | 0.3.0 |
| Wii U client source release | 0.4.0 |
| Night launcher crate | 0.3.0 |
| Horizon patch series | 0.2.0 |
| Cafe toolchain and external input spec | 1.0.0 |

[Compatibility recipe](../catalog/compatibility-v1.json) ties the tuple to
`powerpc-nintendo-cafe`, upstream `69fab597a6f0e8a6ce524d38d1b73e599e343034`
and standalone game source `1a0a2948c33d11744eb403247cb566b0aa3ee858`.
The series first applies the Cafe platform range through
`e1e2a43c2bbffc6a7bf738786d1d6cdb50926ced`, then the runtime and save range
through the native pin. Each patch stores its exact original range and
byte hash. [Native file inventory](../catalog/native-files-v1.json) verifies
the separately maintained Wii U files before installing them. The resulting
full Git tree must equal the pinned native commit tree.

The result preserves authenticated LAN Agent Control, indexed terrain,
streaming and memory changes, region writes and metadata replacement,
Cafe session locking, AX audio, UUID/entropy, and existing gameplay.
`7c8ad024b49c5d813862c826931b91ff20fc6d83` is a diagnostic with a distinct
patch and tree. Use `assemble --diagnostic` to inspect it. The stable build
accepts the baseline assembly and keeps that instrumentation out. The
diagnostic does not establish a save-reopen fix.

Toolchain and private overlays have an independent
[hash lock](../catalog/toolchain-cafe-v1.json). It includes the patched
Cafe compiler/target libraries, Rust source revision and tracked delta
fingerprint, WUT and devkitPPC tools, pinned Cargo, Winit, ring, PNG, ureq,
the recorded nine-file workspace overlay, and native link inputs. Tool
paths resolve from the installed environment and explicit input root.
The legacy bootstrap's separate target-spec gitlink is pinned and optional
for the current stage1 build.

Build and Cemu records describe exact RPX hashes. Reconstructed source,
successful compilation, Cemu behavior and physical acceptance are separate
evidence. Previous hardware tests remain attached to their original RPX.
The current catalog has no signing trust root.

The [combined recipe](../catalog/hosted-v1.json) adds client version 0.5.0
on top of the unchanged standalone recipe and pinned workspace overlay.
It verifies a separate host delta, Horizon patch and final source tree.
The historical standalone packages keep their own hashes and acceptance.
