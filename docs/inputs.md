# Cafe build input inventory

The compatibility recipe pins the exact
[toolchain input specification](../catalog/toolchain-cafe-v1.json).
The builder resolves the installed devkitPro tools and stage1 path from
the environment and verifies content before compiling. It uses the
installed Cafe Rust 1.101.0-dev compiler and target library inventory,
WUT 1.9.2 and devkitPPC r51. Cargo nightly and its release lock are also
pinned. The release does not redistribute the installed toolchain.

Supply a private input root with these exact byte inventories. The paths
below identify the existing Ben-Desktop snapshots. Copy them once to the
named input root. The builder verifies hashes, so replacing or relocating
a snapshot cannot silently change the build.

| Role in input root | Current local source |
| --- | --- |
| `winit/` | `$scratch/HorizonWiiU-png-stage-probe-20261111/winit`, Winit 0.30.13 Cafe shim |
| `ring/` | `$scratch/HorizonWiiU-ring-patched-touchtrace-20261111`, ring 0.17.14 |
| `png/` | `$scratch/HorizonWiiU-png-stage-probe-20261111/png-0.18.1` |
| `ureq/` | `$scratch/HorizonWiiU-touchtrace-20261111/ureq-trace`, ureq 2.12.1 |
| `workspace.patch` | `%TEMP%/HorizonWiiU-region-save-20261119/native-build-overlays.patch`, the recorded nine-file snapshot |

The workspace overlay applies after the canonical public source assembly.
It is a private build input with its own fingerprint. Its raw CRLF archive
bytes are pinned before the builder normalizes transport endings for Git.
The nine tracked paths and the earlier game context are listed in the
[baseline inventory](../wiiu/builder/MIGRATION.md). The release uses the
exact recorded workspace snapshot. It does not read uncommitted files
from another worktree.

`getrandom 0.4.3` comes from the native source inventory and retains its
dual license. The reconstructed source already carries the upstream
`wgpu-hal`, `naga` and `libmimalloc-sys` overrides. The additional offline
Cargo lock and the Winit TV-size shim are under `wiiu/game-bridge/offline/`.
The Wii U linker helper, verifier and PowerPC relocation normalization
remain tracked in the port checkout.

The [selected Cafe Rust source patch](../patches/toolchain/cafe-rust-62606b70/source.json)
comes from the installed Rust checkout over
`62606b70e351edb297db1fdd4b99c8b8ca440d38`. It includes the target, socket,
TLS and unwind changes and applies to that exact source in a clean Git
index. The live setup's local `library/backtrace` substitution is recorded
in the toolchain delta fingerprint and remains an external provisioning
step. The source patch is application evidence. The build itself uses
the separately verified installed compiler and sysroot. Rebuilding that
compiler from a clean machine requires reconstructing the backtrace and
stage1 build inputs.

Use `tools/record_cafe_inputs.py` to emit a candidate lock for review.
Use `tools/export_patch_series.py` to regenerate the checked-in Horizon
patches from their exact local history. Neither signs a release.
