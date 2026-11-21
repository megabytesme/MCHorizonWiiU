# MCHorizonWiiU Builder

The Windows desktop builder uses Python 3.11+ with Tk and an x86-64 Linux environment under WSL 2. The same CLI runs directly in Linux. The [Cafe toolchain guide](inputs.md) lists the host prerequisites, package hashes and components that need maintainer provision.

Clone [MCHorizonWiiU](https://github.com/megabytesme/MCHorizonWiiU) inside the Linux filesystem of your WSL distribution. Its default branch, `main`, contains the CafeOS port and builder. Keep the build cache there. Windows package destinations can use WSL paths such as `/mnt/c/Builds/HorizonWiiU`, although builds on the Linux filesystem are faster. Start the desktop window from a Windows checkout of the same revision.

```powershell
pythonw.exe wiiu/builder/desktop.py --allow-unsigned
```

1. Select the release combination. The display includes the client, patch, toolchain and upstream revision.
2. Enter the WSL distribution, absolute port checkout and output paths. Supply the devkitPro root, verified Cafe Rust compiler and shader tools directory or configure them in the WSL login environment.
3. Run **Check toolchain**. The reported hashes must match the recipe. Run **Prepare / repair dependencies** to reconstruct the overlays and fill the locked Cargo cache.
4. Set the Minecraft installation and client/server JAR paths and run **Prepare resources**. Supply the original Mojang server bundle. Horizon verifies the bundle before unpacking the game JAR, and the builder checks the finished resource manifest. Keep the preparation and inventory private. An existing preparation can be reused from the CLI.
5. Run **Build RPX / WUHB**. The activity log streams output from Cargo and CMake. Cancel interrupts the process group. Launch the same operation again to reuse verified inputs.

Use **Offline** after the upstream object, source assembly, crate archives and Cargo cache are populated. A checksum failure identifies the artifact that needs repair. For a corrupt staged source or overlay, move the named directory aside while the builder is idle and re-run preparation. Interrupted downloads and dependency extraction are staged before promotion. Builds check available space on the destination and cache volumes. An incomplete build has no verified output receipt.

## Direct CLI

```sh
python3 -B wiiu/builder/workflow.py --allow-unsigned inventory-toolchain
python3 -B wiiu/builder/workflow.py --allow-unsigned prepare
python3 -B wiiu/builder/workflow.py --allow-unsigned prepare-resources \
  --minecraft "$minecraft_install" --client-jar "$client_jar" --server-jar "$server_jar" \
  --output "$scratch/prepared"
python3 -B wiiu/builder/workflow.py --allow-unsigned --offline build --fixed-path \
  --output "$scratch/combined" --resources "$scratch/prepared" \
  --resource-manifest "$scratch/prepared.inventory.json"
python3 -B wiiu/builder/build.py verify-output "$scratch/combined/build-record.json"
```

Pass `--upstream PATH` before the subcommand to use a local object store. The fixed-path profile uses an unprivileged Linux mount namespace to stabilize compiler paths. If it is unavailable, omit `--fixed-path` or clear **Fixed compiler path**. Use fixed-path output for byte-for-byte comparison. The [lower-level CLI](../wiiu/builder/README.md) also covers standalone and diagnostic packages.

## SD layout

Copy `HorizonWiiU.wuhb` for Aroma or `HorizonWiiU.rpx` for an RPX-capable homebrew launcher to `wiiu/apps/HorizonWiiU/`. Place the prepared 26.3 directory under `.local/horizon-client/26.3/` inside that app directory. To test with Cemu, use a portable profile with its own scratch SD. Back up existing worlds before testing on hardware. Resource packs and native mods require separate compatibility evidence. The current desktop builder applies the catalogued port patches.

The native build record lists output hashes, exact source trees, input hashes and tool versions. It can include local paths, so share a redacted evidence summary. Do not share the private resource inventory.

## Build the November 20 hosted candidate

The candidate source recipe uses the exact Horizon Nightly commit
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`. Check out that commit in a
disposable upstream checkout and use the verified external Cafe dependency
inputs from the stable builder. Run this command in WSL from the port checkout.

```sh
python3 wiiu/builder/build.py assemble-hosted-nightly \
  --source "$nightly_checkout" \
  --inputs "$cafe_inputs" \
  --output "$candidate_output"
```

The source assembly contains the staged source, Cargo manifest, lockfile and
`nightly-source-record.json` with the exact trees and SHA-256 values.
Cafe Winit comes from the Nightly patch series.

Use the same arguments with `build-hosted-nightly` to package the hosted
candidate. Enable the documented Cafe toolchain environment first. The builder
verifies the compiler and SDK inputs against Toolchain 1.0.0 and uses the
locked, offline dependency graph. Use a fresh output directory for each
independent build.

```sh
python3 wiiu/builder/build.py build-hosted-nightly \
  --source "$nightly_checkout" \
  --inputs "$cafe_inputs" \
  --output "$candidate_output"
python3 wiiu/builder/build.py verify-output "$candidate_output/build-record.json"
```

For independent byte comparisons, run both builds through a Linux mount
namespace so they see the same source and output paths.

```sh
bash tools/build_cafe_game_isolated.sh --hosted-nightly "$candidate_output" \
  --source "$nightly_checkout" --inputs "$cafe_inputs"
```

The local directory contains `HorizonWiiU.rpx`, `HorizonWiiU.wuhb` and
`build-record.json`. The recipe records its own candidate versions.
The active `build-hosted` command still uses the 0.7.0 release recipe.
