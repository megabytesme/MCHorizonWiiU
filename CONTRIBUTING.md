# Contributing to MCHorizonWiiU

Use the [builder guide](docs/builder.md) to reconstruct the exact Horizon source. Keep work on the port in this repository and submit generally applicable Horizon changes upstream. Develop against a scratch Cemu profile and SD. Keep Minecraft files, prepared resources, real saves, account or pairing data, and emulator binaries outside Git.

- Record the exact Horizon commit and patch input and output trees for each source change.
- Bump the client, patch, builder or toolchain version for the component that changed. Update the [compatibility catalog](docs/catalog.md) with its reviewed Git pin and SHA-256.
- Tests should distinguish host, Cafe build, emulator and physical hardware outcomes. Associate runtime results with the exact RPX SHA-256 and state what the test exercised.
- Run the host checks below and include the results with the proposed change.

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
