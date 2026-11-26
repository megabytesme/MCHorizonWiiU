# MCHorizonWiiU Builder

The Windows desktop builder uses Python 3.11+ with Tk and an x86-64 Linux environment under WSL 2. The same CLI runs directly in Linux. The [Cafe toolchain guide](inputs.md) lists the host prerequisites, package hashes and components that need maintainer provision.

Clone [MCHorizonWiiU](https://github.com/megabytesme/MCHorizonWiiU) inside the Linux filesystem of your WSL distribution. Keep the build cache there. Windows package destinations can use WSL paths such as `/mnt/c/Builds/HorizonWiiU`. The November 20 candidate uses the [hosted Nightly CLI below](#build-the-november-20-hosted-candidate) from `main`.

## Build the verified 0.7.0 release

The desktop and direct workflows for Builder 0.6.0 verify that their local
source matches the release inventory. Apply the commands in this section
from the rollback tag in Linux or WSL.

```sh
git fetch origin tag rollback/client-0.7.0-unified-main
git worktree add --detach "$rollback_checkout" rollback/client-0.7.0-unified-main
cd "$rollback_checkout"
```

Start the desktop window from a Windows checkout of the same rollback tag.

```powershell
pythonw.exe wiiu/builder/desktop.py --allow-unsigned
```

1. Select the release combination. The display includes the client, patch, toolchain and upstream revision.
2. Enter the WSL distribution, absolute port checkout and output paths. Supply the devkitPro root, verified Cafe Rust compiler and shader tools directory or configure them in the WSL login environment.
3. Run **Check toolchain**. The reported hashes must match the recipe. Run **Prepare / repair dependencies** to reconstruct the overlays and fill the locked Cargo cache.
4. Set the Minecraft installation and client/server JAR paths and run **Prepare resources**. Supply the original Mojang server bundle. Horizon verifies the bundle before unpacking the game JAR, and the builder checks the finished resource manifest. Keep the preparation and inventory private. An existing preparation can be reused from the CLI.
5. Run **Build RPX / WUHB**. The activity log streams output from Cargo and CMake. Cancel interrupts the process group. Launch the same operation again to reuse verified inputs.

Use **Offline** after the upstream object, source assembly, crate archives and Cargo cache are populated. A checksum failure identifies the artifact that needs repair. For a corrupt staged source or overlay, move the named directory aside while the builder is idle and re-run preparation. Interrupted downloads and dependency extraction are staged before promotion. Builds check available space on the destination and cache volumes. An incomplete build has no verified output receipt.

### Direct CLI

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
checks the lock selected by the source recipe and uses the locked, offline
dependency graph. The RC 29 recipe uses [Toolchain 1.1.0](gx2/cafe-toolchain-1.1.0.md).
Use a fresh output directory for each independent build.

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


### Reuse a fixed-path Cargo cache during development

Assemble the new recipe into a fresh build directory. Seed its Cargo cache
from an earlier local build made with the same fixed-path wrapper. The
helper verifies both staged engine trees and preserves the earlier mtime
only for unchanged Git entries and byte-identical dependency files. Cargo
can then reuse their fingerprints and rebuild changed inputs.

```sh
python3 -B wiiu/builder/build.py assemble-hosted-nightly \
  --source "$nightly_checkout" --inputs "$cafe_inputs" --output "$candidate_output"
python3 -B tools/seed_cafe_build_cache.py "$previous_output" "$candidate_output"
bash tools/build_cafe_game_isolated.sh --hosted-nightly "$candidate_output" \
  --source "$nightly_checkout" --inputs "$cafe_inputs"
```

Run both builds from the same pinned toolchain. Use separate output
directories and independent inputs for candidate reproducibility. For
development on a crowded drive, pass `--move-cache` to transfer the
Cargo target between idle outputs on the same filesystem. The previous
RPX, WUHB, build receipt and staged assembly stay in place.

Inspect an RPX against the measured Aroma loader budget before hardware use.
The command verifies the mapped code span against the RPL file info. It
estimates the simultaneous allocations for executable text and the stored
relocation buffer, using the allocation alignment and pre-relocation overhead
observed on hardware. It rejects estimates above the 67,096,512-byte
contiguous heap of the Aroma wrapper.
The Nightly builder runs this check before writing its successful build
receipt and saves the measurements in `code-heap.json`. A passing estimate
still needs a console boot test.

```sh
python3 wiiu/builder/build.py inspect-code-heap "$candidate_output/HorizonWiiU.rpx"
```

### Prepare Minecraft 26.3 for November 20

Use the November 20 preparer with a disposable copy of the staged source.
Provide the existing 26.3 client and server JARs and the player's asset
directory with `indexes/34.json` and `objects/`. The Nightly core needs the
new fluid-state definitions in `block-behaviours.json`. The earlier
0.7.0 preparation predates them.

```sh
cargo +nightly run --offline --locked \
  --manifest-path "$prepared_source/Cargo.toml" -p horizon-prepare -- \
  --prepare --root "$prepared_source" --client-jar "$client_jar" \
  --server-jar "$server_jar" --assets-dir "$minecraft_assets" \
  --output "$private_preparation"
```

The verification run built `horizon-data.bin` with
`horizon_core::prepared_files::write_with_readers` from this same tree.
It then rebound the private archive to the isolated CafeOS game path.
The resulting preparation includes the asset index, fluid-state
definitions and the data-fix tables for older saves.

### Prepare a copied archive for an isolated game path

The prepared archive includes the absolute resource path it is bound to.
When the archived path differs from the path seen by CafeOS in a disposable
Cemu profile, create a separate copy with the correct root.

```sh
python3 tools/rebind_cafe_prepared_archive.py "$prepared_copy" \
  --output "$scratch/horizon-data.bin"
```

The command's default guest path is the standard Horizon Wii U installation.
For a qualification instance, pass its full CafeOS resource directory as
`--guest-root` when rebinding and checking the archive.

```sh
python3 tools/rebind_cafe_prepared_archive.py "$prepared_copy" \
  --output "$scratch/horizon-data.bin" --guest-root "$qualification_guest_root"
python3 tools/rebind_cafe_prepared_archive.py "$prepared_copy" \
  --check --archive "$scratch/horizon-data.bin" --guest-root "$qualification_guest_root" \
  --payload-from "$prepared_copy/horizon-data.bin"
```

The preflight checks the exact guest root, embedded manifest, selected hosted
recipe pins, archive index and payload identity. Save its JSON output with the
qualification receipt. Place the verified archive alongside the matching
manifest. Before safely ejecting the SD card, point the check at the installed
resource directory and pass `--sha256` with the receipt's `archive_sha256` to
confirm the transferred bytes. Use `--recipe` when packaging from a different
hosted source recipe. A failed check invalidates a prepared-resource benchmark.
