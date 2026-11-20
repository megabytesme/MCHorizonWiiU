# Cafe toolchain and private inputs

The [toolchain lock](../catalog/toolchain-cafe-v1.json) pins compiler and linker binaries, WUT headers and library, the Cafe sysroot inventory and Cargo. The [dependency recipe](../catalog/dependencies-cafe-v1.json) adds archive hashes for the official devkitPro packages and reproducible crate overlays. These source inputs can be verified on any host supporting the builder.

| Input | Provisioning |
| --- | --- |
| Linux x86-64 or WSL 2 | Python 3.11+, Git, CMake 3.28.3, Ninja 1.11.1, Rustup and JDK 25 for preparation. |
| devkitPPC r51 / WUT 1.9.2 | Use the official devkitPro pacman distribution. Retrieve the exact signed package archives named in the dependency recipe. Review and verify package signatures through pacman. If those revisions have aged out, obtain the signed originals from the publisher or a verified cache. |
| Cafe Rust 1.101.0-dev | Supply a stage1 compiler and target sysroot matching the lock. The [Cafe Rust delta](../patches/toolchain/cafe-rust-62606b70/source.json) applies over [rust-wiiu/rust at 62606b70](https://github.com/rust-wiiu/rust/tree/62606b70e351edb297db1fdd4b99c8b8ca440d38) and includes recorded target, TLS, socket and unwind changes. The installed stage1 used [backtrace-rs at 148c9de](https://github.com/rust-lang/backtrace-rs/tree/148c9de856542400168883f69437affbbe6f6743) for `library/backtrace`. The exact bootstrap configuration and complete locked source tree still need clean-host verification. |
| Cargo nightly and shader compiler | Provide the Cargo build named by the lock, `cargo-wiiu` and the verified `glslcompiler` from the Cafe GLSL toolchain. Install them separately and check the hashes. |
| Crate source overlays | **Prepare dependencies** downloads the pinned ring, png and ureq source archives and reconstructs their exact overlays. The Winit facade and workspace patch are tracked. |
| Minecraft 26.3 | Use your own legal installation and exact client and server JARs. The upstream preparation step uses its assets and exporters. The builder verifies the JAR and preparation pins. |

Set these variables in the Linux / WSL login environment. Choose the actual paths for the installed tools.

```sh
export DEVKITPRO=/opt/devkitpro
export DEVKITPPC="$DEVKITPRO/devkitPPC"
export HORIZON_WIIU_STD_RUSTC=/path/to/cafe-stage1/bin/rustc
export PATH="$DEVKITPRO/tools/bin:$DEVKITPRO/portlibs/wiiu/bin:$DEVKITPPC/bin:$HOME/.cargo/bin:/path/to/cafeglsl/bin:$PATH"
python3 -B wiiu/builder/workflow.py --allow-unsigned inventory-toolchain
```

Add `--package-cache PATH` to check the devkitPro package archive SHA-256 values. This is an inventory command. Install or repair the official packages and custom compiler in your chosen environment before building. Toolchain drift requires a new reviewed recipe, build and evidence.

Minecraft assets, decompiled source, Nintendo proprietary SDK components and user data must stay private. The project distributes public source and deltas. Tools and installed libraries remain separate from builder and client bundles until their individual licenses, corresponding-source obligations and redistribution provenance have been reviewed. Cemu test patches are source-only. See [third-party notices](../THIRD_PARTY_NOTICES.md).
