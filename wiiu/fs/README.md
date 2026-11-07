# Cafe free-space adapter

`available_space` uses Cafe std to resolve relative paths and initialize its
filesystem client, then uses a separate WUT client to query
`FSGetFreeSpaceSize`. It returns an error on a failed query or deregistration;
the size is never returned on failure. If deregistration fails, the client
allocation is retained because the OS may still reference it. This adapter
does not alter Horizon's existing save and low-disk-space logic.

On 2026-11-07, a standalone Rust+WUT diagnostic was built from the working tree
based on commit `ae75e914f2` and run in an isolated stock Cemu 2.6 instance.
It tested an absolute SD directory, the same directory relative to a changed
current directory, missing relative and invalid-volume paths, an embedded NUL,
and 4 concurrent threads each making 8 queries. It returned `failure_flags=0`
and Cemu logged `coreinit.exit(0)`. The earlier direct C probe observed
`32212254720` bytes for the existing directory and status `-6` for missing and
invalid paths; it showed that the underlying call can modify the output on
failure, which the adapter discards. These checks do not cover real hardware or
the failure of `FSDelClient`.

The diagnostic source, isolated SD status and RPX are under
`%TEMP%\HorizonWiiU-fs-probe-20261107`. The concurrent RPX is
`cemu\fs-rust-final.rpx` (SHA-256
`2615ac36f32b2841460e208bbc287a9a34a4a83343acc1d6adcd13b21d34896e`). It is
a diagnostic, not the Horizon client or a title-screen checkpoint.

Build the diagnostic's Rust archive from its `rust-probe` directory on WSL with
`DEVKITPRO=/opt/devkitpro DEVKITPPC=/opt/devkitpro/devkitPPC`, the Cafe stage-1
compiler as `RUSTC`, and
`CARGO_TARGET_DIR=/tmp/horizonwiiu-fs-rust cargo +nightly build --release --lib --target powerpc-nintendo-cafe --jobs 32`.
Configure its `build-rust` directory with
`-DCMAKE_TOOLCHAIN_FILE=/opt/devkitpro/cmake/WiiU.cmake` and
`-DHORIZON_ENTROPY_STATICLIB=/tmp/horizonwiiu-fs-rust/powerpc-nintendo-cafe/release/libhorizon_wiiu_entropy_probe.a`,
then run `cmake --build <build-rust> --parallel 32` with devkitPro, devkitPPC,
and Cargo on `PATH`. Its copied entropy-probe harness calls the filesystem test;
the name of the harness does not imply an entropy test.
