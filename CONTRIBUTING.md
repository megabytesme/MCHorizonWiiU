# Contributing to MCHorizonWiiU

Use the [builder guide](docs/builder.md) to reconstruct the exact Horizon source. Keep work on the port in this repository and submit generally applicable Horizon changes upstream. Develop against a scratch Cemu profile and SD. Keep Minecraft files, prepared resources, real saves, account or pairing data, and emulator binaries outside Git.

## Branches and commits

- Use `main` for routine sequential development. Start from current `origin/main` with `git pull --ff-only`, then push each coherent, tested commit with `git push origin main` as soon as it is ready.
- For concurrent or risky work, use one short-lived descriptive branch and push each tested commit as you make it. Merge it into `main` after acceptance and remove it once its history is reachable from `main` and dependent work has moved.
- Use `codex/nightly-YYYY-MM-DD` only for temporary work tied to that upstream revision. Keep historical refs under `archive/` or `history/`.
- Commit with your configured developer identity. Preserve existing authorship, published history, and commit pins used by the release catalog.

## Source and validation

- Record the exact Horizon commit and patch input and output trees for each source change.
- Bump the client, patch, builder or toolchain version for the component that changed. Update the [compatibility catalog](docs/catalog.md) with its reviewed Git pin and SHA-256.
- Tests should distinguish host, Cafe build, emulator and physical hardware outcomes. Associate runtime results with the exact RPX SHA-256 and state what the test exercised.
- Run the applicable host checks below before committing. The public-source workflow runs on pushes to `main` and on pull requests. It verifies source reconstruction and stages source archives for validation.

```sh
python3 -B -m unittest discover -s wiiu/builder -v
python3 -B tools/check_public_tree.py --ref HEAD
gitleaks git --log-opts=HEAD --redact=100 .
python3 -B tools/check_release.py
cargo fmt --manifest-path wiiu/pregame/Cargo.toml --check
cargo clippy --locked --manifest-path wiiu/pregame/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path wiiu/pregame/Cargo.toml
```

Report reproducible issues with the build identity shown in Night's About page, the target and input device, and a redacted log excerpt.
