# Wii U builder

Builder 0.3.0 reads the unsigned
[compatibility recipe](../../catalog/compatibility-v1.json). It verifies an
exact upstream Git object, two Horizon patches, and the Wii U source
inventory, then reconstructs the native source tree in scratch. The CLI
imports reusable source, packaging and native build modules. It can run
with local Git objects, installed tools and a populated Cargo cache.

## Environment

Use Python 3.11 or newer and Git. Host-only compatibility tests work on
Windows and Linux. Run Cafe builds in a Linux/WSL checkout of the port so
Linux Git and the toolchain see the same files. A Windows linked worktree
has a Windows Git directory pointer and needs a local WSL checkout first.

```sh
export DEVKITPRO=/opt/devkitpro
export DEVKITPPC="$DEVKITPRO/devkitPPC"
export PATH="$DEVKITPRO/tools/bin:$DEVKITPRO/portlibs/wiiu/bin:$DEVKITPPC/bin:$HOME/.cargo/bin:/path/to/cafeglsl/bin:$PATH"
export HORIZON_WIIU_STD_RUSTC=/path/to/verified/stage1/bin/rustc
upstream=/path/to/local/Horizon-Git-repository
pin=69fab597a6f0e8a6ce524d38d1b73e599e343034
scratch=$(mktemp -d)
python3 -B wiiu/builder/build.py validate --source "$upstream" --revision "$pin"
python3 -B wiiu/builder/build.py assemble --source "$upstream" --revision "$pin" --output "$scratch/stable"
python3 -B wiiu/builder/build.py build --source "$upstream" --revision "$pin" --output "$scratch/launcher"
```

The launcher build takes the chosen client files from the current committed
port checkout. It produces `HorizonLauncher-Night.rpx` and
`HorizonLauncher-Night.wuhb`. An assembly is a separate receipt and checkout.
To assemble the traced world-reopen source, pass `--diagnostic` and a new
output path. The launcher remains a hosted mock during development of the
combined process.

## Standalone native game

Create a private input directory with the verified `winit/`, `ring/`, `png/`,
`ureq/` and `workspace.patch` inputs. The
[toolchain lock](../../catalog/toolchain-cafe-v1.json) pins their digests
and the installed Cafe tools. The [input inventory](../../docs/inputs.md)
identifies the current snapshots. Use the matching Cargo nightly and its
verified package cache. The builder runs Cargo with `--offline --locked`.
Source export and game outputs use a separate Cargo target and CMake
directory. Native runtime resources stay outside the packages.

```sh
inputs=/path/to/private/verified-cafe-inputs
python3 -B wiiu/builder/build.py build-game --assembly "$scratch/stable" --inputs "$inputs" --output "$scratch/game"
# Verify either target using the path to build-record.json printed by the builder.
python3 -B wiiu/builder/build.py verify-output /path/to/build-record.json
```

The game command produces `HorizonWiiU-Game.rpx` and
`HorizonWiiU-Game.wuhb`. Its record lists exact tools, assembly and private
overlay hashes, commands and package hashes. Keep the assembly directory
while reusing its game build cache. The game has its own acceptance scope.

## Private resources and validation

Use the reconstructed Horizon preparation tool with your legally obtained
Minecraft files. The builder can inventory and reuse a pre-existing cache.
The generated inventory stays private. Pass both options to `build-game`
to record the checked cache identity. Compilation can run without assets
because the standalone client opens them at run time.

```sh
python3 -B wiiu/builder/build.py inventory-resources /path/to/prepared --output "$scratch/private-resources.json"
# Add --resources /path/to/prepared --resource-manifest "$scratch/private-resources.json" to build-game.
python3 -B -m unittest discover -s wiiu/builder -v
cargo test --locked --manifest-path wiiu/pregame/Cargo.toml --target-dir "$scratch/host-target"
```

Run the isolated launcher Cemu fixture from Windows for emulator evidence.

```powershell
./wiiu/pregame/tests/cemu.ps1 -Cemu 'C:/tools/Cemu/Cemu.exe' -Rpx 'C:/scratch/HorizonLauncher-Night.rpx' -Output "$env:TEMP/port-night-cemu"
```

A build record establishes compilation and package integrity. Cemu launcher
records describe the emulated mock lifecycle. Real game behavior and
physical hardware require their own acceptance, keyed to exact RPX hashes.
