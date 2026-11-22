# GitHub cutover

The GitHub default branch is [main](https://github.com/megabytesme/MCHorizonWiiU/tree/main). It contains the CafeOS port, builder and catalog. [PR #4](https://github.com/megabytesme/MCHorizonWiiU/pull/4) merged the unified application in `ce5ed5d132a45db6fc45ba5218f917b6830eb9bc` with the exact candidate tree and both source parents.

The full fork's former default tip is preserved on [archive/legacy-full-horizon](https://github.com/megabytesme/MCHorizonWiiU/tree/archive/legacy-full-horizon) at `63cf4b0b4a92c36c7944adcf56da84cba2885b43`. The published Cafe platform development snapshot is preserved on [archive/wiiu-platform-api](https://github.com/megabytesme/MCHorizonWiiU/tree/archive/wiiu-platform-api) at `e1e2a43c2bbffc6a7bf738786d1d6cdb50926ced`.

The exact original port-only baseline is `cf2ac8cbbad809a878e59aa4b20288a551294283`, with 18 commits rooted at `6913bfeaf60ccfedd6c10e6e50010e8d0d669165`. That history remains in its original local branch. The published port history replays all 18 commits with machine-specific path prefixes replaced, then adds the release changes. The sanitized baseline is `bd02f62b4de34d3750e4395b21f27f11cafe1ef0`. Some path examples were inside the native patch and source inventory, so the public recipe pins the recalculated trees.

The [MCHorizonUWP reference](https://github.com/megabytesme/MCHorizonUWP/tree/f67b82e354e3ccd098a4d31e8bcdbe35f8afcffe) separates its root signed catalog, exact upstream patches, builder core and GUI, signer, host and bridge. MCHorizonWiiU keeps the same responsibilities in `catalog/`, `patches/`, `wiiu/builder/`, `tools/catalog-signer/`, `wiiu/host/` and `wiiu/game-bridge/`. Night, the GX2 renderer and the Cafe platform code stay under `wiiu/`.

## Working after cutover

GitHub branch renames kept the legacy fork and port histories separate. For active port development, use a checkout that tracks the new `origin/main` and follow [CONTRIBUTING.md](../CONTRIBUTING.md). Use a separate checkout or worktree for the legacy fork, especially when it contains local changes. An administrator can select `archive/legacy-full-horizon` as the default if the old project needs to be restored.

Run the [host validation checks](../CONTRIBUTING.md#source-and-validation) for changes on `main`. The [catalog and release procedure](catalog.md) covers signing and publication.
