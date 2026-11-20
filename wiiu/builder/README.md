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
output path. The default launcher leaves game launch unavailable. `build --fixture`
packages the separate `HorizonLauncher-Night-Fixture` for lifecycle tests.

## Unified Horizon Night and game

The hosted builder profile `0.4.0` uses [hosted recipe v2](../../catalog/hosted-v2.json)
with Wii U client `0.6.0` and hosted patch release `0.3.0`. It extends the
exact standalone assembly with pinned Horizon-source patches, Cafe bridge
deltas, hosted Rust sources and a hosted-only offline dependency lock.
Make a clean local Linux checkout
of the integration branch before building. Set `inputs` to the verified
private Cafe input directory. The combined build emits
`HorizonWiiU.rpx`, `HorizonWiiU.wuhb` and a hash-checked build record. Its
`embedded_build` fields are the values shown in Night's About page. The
read-only standalone recipe and earlier hosted release use their own pins.

```sh
python3 -B wiiu/builder/build.py build-hosted --assembly "$scratch/stable" \
  --inputs "$inputs" --output "$scratch/hosted"
# Optional stable compiler mount for comparisons between build directories.
bash tools/build_cafe_game_isolated.sh --hosted "$scratch/hosted-rebuild" \
  --assembly "$scratch/stable" --inputs "$inputs"
```

Use a dedicated output directory for each staged hosted game tree. A relink
can reuse it when only the shell source changes. The builder verifies the
staged game tree and records the exact launcher inputs for each package.

`build-hosted` accepts the same optional verified private resource inventory
arguments as `build-game`. The compiled packages contain no runtime resource
bytes. Run the combined RPX in the NSS-patched Cemu interpreter with a fresh
portable SD under `wiiu/apps/HorizonWiiU` and a complete prepared cache there.
The wrapper requires unprivileged Linux mount namespaces.

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
while reusing its game build cache. Cargo sees the same relative path for
each local package in every fresh build root, including the verified
dependency overlays. The pinned Rust toolchain also embeds absolute workspace paths in parts
of the runtime and symbol table. For byte-for-byte comparison across
independent directories, use the Linux mount-namespace wrapper. It presents
`/var/tmp/horizon-wiiu-build` separately to each compiler while keeping each
host build directory and target isolated. The kernel must allow unprivileged
user namespaces. A container can provide the same mount with the pinned
installed tools. The wrapper prints the host-facing record path.

```sh
for run in first second; do
  bash tools/build_cafe_game_isolated.sh "$scratch/$run" \
    --assembly "$scratch/stable" --inputs "$inputs" > "$scratch/$run.record"
  python3 -B wiiu/builder/build.py verify-output "$(cat "$scratch/$run.record")"
done
first=$(dirname "$(cat "$scratch/first.record")")
second=$(dirname "$(cat "$scratch/second.record")")
cmp "$first/HorizonWiiU-Game.rpx" "$second/HorizonWiiU-Game.rpx"
cmp "$first/HorizonWiiU-Game.wuhb" "$second/HorizonWiiU-Game.wuhb"
```

The standalone game reads its app directory from
`/vol/external01/wiiu/apps/HorizonWiiU` and finds the prepared cache at
`.local/horizon-client/26.3` inside it by default. For an emulator run, use
a dedicated portable Cemu profile with a scratch SD directory and create
that app layout there. Keep real saves and credentials outside the profile.
No runtime resource tree is required to boot the Night launcher fixture.

## Private resources and validation

Use the reconstructed Horizon preparation tool with your legally obtained
Minecraft files. The builder can inventory and reuse a pre-existing cache.
The generated inventory stays private. Pass both options to `build-game`
to record the checked cache identity. Compilation can run without assets
because the standalone client opens them at run time. The inventory checks
file content and guards reuse. A bootable cache also needs the pinned game's
`horizon-resources.json` and its required files. A detached data pack alone
can pass an inventory check and still fail runtime resource validation.

```sh
python3 -B wiiu/builder/build.py inventory-resources /path/to/prepared --output "$scratch/private-resources.json"
# Add --resources /path/to/prepared --resource-manifest "$scratch/private-resources.json" to build-game.
python3 -B -m unittest discover -s wiiu/builder -v
cargo test --locked --manifest-path wiiu/pregame/Cargo.toml --target-dir "$scratch/host-target"
```

Run the isolated launcher Cemu fixture from Windows for emulator evidence.

```powershell
./wiiu/pregame/tests/cemu.ps1 -Production -Cemu 'C:/tools/Cemu/Cemu.exe' -Rpx 'C:/scratch/HorizonLauncher-Night.rpx' -Output "$env:TEMP/port-night-cemu"
./wiiu/pregame/tests/cemu.ps1 -Cemu 'C:/tools/Cemu/Cemu.exe' -Rpx 'C:/scratch/HorizonLauncher-Night-Fixture.rpx' -Output "$env:TEMP/port-night-fixture-cemu"
```

A build record establishes compilation and package integrity. Cemu launcher
records describe the emulated mock lifecycle. Real game behavior and
physical hardware require their own acceptance, keyed to exact RPX hashes.
