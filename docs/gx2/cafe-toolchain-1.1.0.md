# Cafe Rust toolchain 1.1.0

The hosted Nightly recipe records its toolchain version and lock SHA-256.
The [1.1.0 lock](../../catalog/toolchain-cafe-v1.1.0.json) adds an inventory
fingerprint for the host libraries used by Cargo build scripts and
procedural macros. The builder checks both host and Cafe inventories,
compiler and driver, LLVM, Cargo, CafeGLSL, devkitPPC and WUT before the
native build and repeats the check before writing its build receipt.
Historical recipes continue to use their archived 1.0.0 lock.

## Recovery

The archived 1.0.0 Cafe target-library inventory matched its original
fingerprint. No retained compiler or matching driver was found in the
historical stage1, surviving installation, or local archive search.
The retained Rust checkout was at `62606b70e351edb297db1fdd4b99c8b8ca440d38`.
Its tracked Cafe delta, filesystem source and Backtrace revision matched
the recorded inputs. The bootstrap compiler and LLVM archives came from
the retained cache. The LLVM binaries, Cargo, WUT, devkitPPC and shader
compiler matched the existing native-tool hashes.

Rebuilding stage1 produced a different launcher and driver fingerprint.
The historical fingerprint doesn’t record enough build configuration to
isolate the cause of the original compiler byte difference. The replacement
uses the exact recorded source, bootstrap archive hashes and flags. A full
`library` build installs matching host procedural-macro libraries alongside
the Cafe target libraries. The archive records the known Cafe warning about
the unsupported dynamic std library and the flag that registers the
optional TLS probe configuration. That flag leaves the probe hook disabled
in the production standard library.

The shipping std passed the existing TLS virtualization and TCP socket
clone probes in forced-interpreter Cemu. The TLS run exercised concurrent
key creation, thread-local isolation, allocation, destructor dispatch and
table reuse across 300 sequential threads. The mixed-width C-to-Rust call
also verified big-endian byte order and the 64-bit return ABI. The PowerPC
carry diagnostic completed one million calls. The NSS-patched emulator's
entropy smoke test returned zero failure flags. The recorded target
specification uses 32-bit atomics and panic-abort. Physical hardware and
Internet networking remain outside these emulator results.

## Restore the private compiler

The private backup on Ben-Desktop is under
`%LOCALAPPDATA%/MCHorizonWiiU/Toolchains/1.1.0`. It includes stage1, the
shader compiler, cached bootstrap archives, the Rust source delta and
configuration, the target specification, the historical target libraries,
and SHA-256 receipts. Stage1 and the shader compiler were extracted into
an isolated WSL directory. The restored sysroot matched all 124 entries
and the full builder fingerprint.

In WSL set `cafe_archive` to that directory, then run:

```sh
(cd "$cafe_archive" && sha256sum -c SHA256SUMS)
cafe_install=$(mktemp -d)
tar --same-permissions --zstd -xf "$cafe_archive/stage1.tar.zst" -C "$cafe_install"
tar --same-permissions --zstd -xf "$cafe_archive/cafeglsl.tar.zst" -C "$cafe_install"
export HORIZON_WIIU_STD_RUSTC="$cafe_install/stage1/bin/rustc"
export PATH="$cafe_install/cafeglsl-1.0.0-rc1-linux-x86_64/bin:$PATH"
```

Provision the pinned devkitPro packages and Cargo first using the
[input guide](../inputs.md). The two absolute source browsing symlinks in
stage1 refer to the original Rust checkout. If compiler source browsing is
needed, restore that checkout at the recorded path from the pinned revisions
and archived patch. The native build uses the restored executable and
libraries directly. Use the [hosted build command](../builder.md#build-the-november-20-hosted-candidate)
to recheck the installation against the versioned recipe.
