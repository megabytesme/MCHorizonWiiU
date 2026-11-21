# MCHorizonWiiU

MCHorizonWiiU is an unofficial Nintendo Wii U port of [Horizon](https://github.com/horizon-rust/horizon), a Rust client for Minecraft: Java Edition. This repository owns the CafeOS port. The builder recreates the upstream workspace at a pinned Git commit and applies the Wii U patch series.

**Current channel: development.** Horizon Night is the on-console launcher. It manages local offline profiles, prepared installations, saved-world selection, server bookmarks, and controller preferences. The combined RPX enters the native game and returns to Night in the same process. Display and input use the Wii U's TV and GamePad support through WUT and native GX2. The renderer is still under development. See the [GX2 checkpoint](wiiu/RENDERER_CHECKPOINT.md) and the [feature matrix](docs/launcher-features.md) for coverage.

Horizon Night supports offline profiles and Microsoft device sign-in for accounts that own Minecraft: Java Edition. Account refresh credentials are stored per profile with PIN protection by default. Night also hosts its own [HTTPS Agent Control endpoint](docs/mcp/README.md) with local GamePad approval. The original [encrypted LAN control transport](wiiu/game-bridge/agent-control.md) remains available for diagnostics.

## Build and install

1. Read the [Windows and WSL builder guide](docs/builder.md) and [Cafe toolchain requirements](docs/inputs.md).
2. Select a compatible client and patch. Prepare dependencies and supply your own Minecraft: Java Edition 26.3 installation and matching JARs to prepare private resources.
3. Build the RPX and WUHB. Copy the package and prepared resource directory to a scratch SD as described in the builder guide.

The builder is a desktop packaging tool. Gameplay and local launcher operations run on the console. Night can select installed, compatible resource preparations. The project has no in-console compiler or installer for upstream nightlies.

## Validation

The [unified candidate validation](docs/releases/client-v0.7.0.md) tracks the combined application. The [Night validation record](docs/night-local-validation.md) describes exact package hashes and Cemu results from the original baseline. The [migration build record](docs/releases/client-v0.6.0.md) tracks the sanitized source candidate separately. **Reopening a saved world on physical Wii U remains a release blocker.** Earlier hardware observations and the Cemu save cycle are documented in the [hosted validation record](docs/hosted-validation.md).

## Development

[Contributing](CONTRIBUTING.md) · [Release catalog and signing](docs/catalog.md) · [GitHub migration](docs/migration.md) · [Third-party notices](THIRD_PARTY_NOTICES.md) · [MIT license](LICENSE)

Minecraft, Nintendo Wii U and their associated content belong to their respective owners. MCHorizonWiiU is a community project.
