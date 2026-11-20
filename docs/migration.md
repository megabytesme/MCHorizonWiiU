# GitHub cutover

The public full fork remains on `main`. Its archival reference is [archive/legacy-full-horizon](https://github.com/megabytesme/MCHorizonWiiU/tree/archive/legacy-full-horizon) at `63cf4b0b4a92c36c7944adcf56da84cba2885b43`.

The exact original port-only baseline is `cf2ac8cbbad809a878e59aa4b20288a551294283`, with 18 commits rooted at `6913bfeaf60ccfedd6c10e6e50010e8d0d669165`. That history remains in its original local branch. The review branch replays all 18 commits with machine-specific path prefixes replaced, then adds the release changes. The sanitized baseline is `bd02f62b4de34d3750e4395b21f27f11cafe1ef0`. Some path examples were inside the native patch and source inventory, so the public recipe pins the recalculated trees.

The [MCHorizonUWP reference](https://github.com/megabytesme/MCHorizonUWP/tree/f67b82e354e3ccd098a4d31e8bcdbe35f8afcffe) separates its root signed catalog, exact upstream patches, builder core and GUI, signer, host and bridge. MCHorizonWiiU keeps the same responsibilities in `catalog/`, `patches/`, `wiiu/builder/`, `tools/catalog-signer/`, `wiiu/host/` and `wiiu/game-bridge/`. Night, the GX2 renderer and the Cafe platform code stay under `wiiu/`.

## Maintainer procedure

1. Verify the review branch's tip and the archival SHA. Check the [Actions results](https://github.com/megabytesme/MCHorizonWiiU/actions), source tree receipts and redacted history audit for that exact tip.
2. Have the maintainer approve the port-only source and default-branch change. GitHub compares the two roots as unrelated histories. Review the port-only commits on their branch and review release changes against its sanitized baseline.
3. In repository **Settings → General → Default branch**, choose the approved port-only branch. An authenticated administrator can instead use `gh repo edit megabytesme/MCHorizonWiiU --default-branch codex/wiiu-canonical-release`.
4. Confirm `git ls-remote --symref origin HEAD refs/heads/main refs/heads/archive/legacy-full-horizon` reports the new default and both legacy references at the expected commit. Update branch protection and release automation to target the new default. Audit open issues and PRs before changing their references.
5. Retain both legacy references. If necessary, roll back the setting to `main` and investigate using the review branch.

Canonical source cutover and production artifact publication are separate approvals. See the [signing and release procedure](catalog.md). GitHub's [default branch settings guide](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-branches-in-your-repository/changing-the-default-branch) describes required permissions.
