# Cafe getrandom adapters

This crate adapts Horizon's existing getrandom 0.2.17, 0.3.4 and 0.4.3
dependencies to the Cafe Rust standard library's `SystemRng`. It does not
replace Horizon game code or invent a random-number generator. All getrandom
dependencies and callbacks are Cafe-only; desktop targets retain their usual
getrandom backends. The executable must call `horizon_wiiu_entropy::link()` as
its first platform step, before other random-dependent initialization, so the
static linker retains all three hooks and the system RNG is checked early.
The crate is not yet connected to the full Horizon client.

Use the pinned Cafe compiler and set `RUSTFLAGS='--cfg getrandom_backend="custom"'`
for a Cafe build containing getrandom 0.3 or 0.4. The crate enables getrandom
0.2's `custom` feature on Cafe. The 0.3 and 0.4 callbacks have distinct names
and their own versions' Rust error types: upstream 0.4.3 is copied under
`third_party/getrandom-0.4.3`, retaining its MIT and Apache-2.0 licenses,
with a Cafe-only link-name change and the corresponding check-cfg declaration.
One blank changelog line also has trailing whitespace removed.
Other targets keep upstream's original link name. Every Cargo workspace that
depends on getrandom 0.4 must use the same path patch, including standalone
Wii U workspaces; Rust-ABI callbacks and dependencies must be built with the
same compiler. Audit the patch when changing getrandom versions.

The 0.3/0.4 adapters initialize possibly uninitialized output storage before
forming a byte slice for `SystemRng`. The current Cafe std initializes NSS on
first use through its own process-wide `Once`; it aborts the process when NSS
reports a failure, rather than returning a getrandom error. This is fail-closed
but is not equivalent to upstream's recoverable error behavior. The current
fork's `std::random` API requires its nightly `random` feature. No fallback,
fixed seed, or synthetic entropy is used.

The startup anchor, and each callback if the anchor was not called first,
also run a once-per-process check on two differently initialized 64-byte
buffers. It aborts if either buffer is unchanged or all-zero, or if the two
outputs are identical. This detects the observed stock-Cemu failure and some
other obvious defects, but cannot prove that output is unpredictable or detect
all broken generators. It does not check random uses made directly by std
before the executable calls `link()`.

Stock Cemu 2.6 returns success for these NSS imports without filling the
buffer, and must not be treated as a working entropy source. The isolated
experimental Cemu patch and its limitations are in `wiiu/cemu/README.md`.
Real Wii U entropy and error handling have not been tested on hardware.

## Link and runtime probe

The standalone `probe` links this actual crate into a Cafe static library,
calls `link()`, exercises all three getrandom versions plus 0.3/0.4's
`fill_uninit`, and reuses the WUT diagnostic entry/packaging. Build it in WSL:

```sh
TMPDIR=$workspace/HorizonWiiU-cemu-nss-20261107/temp \
HORIZON_WIIU_STD_RUSTC=$workspace/HorizonWiiU-toolchain/rust-wiiu-cafe/build/host/stage1/bin/rustc \
HORIZON_WIIU_ENTROPY_BUILD_DIR=$workspace/HorizonWiiU-cemu-nss-20261107/production_probe_build \
HORIZON_WIIU_JOBS=1 bash wiiu/entropy/probe/build.sh
```

The script sets the custom backend cfg, uses WUT startup, and emits
`cmake/horizon_wiiu_entropy_probe.rpx`. It writes a failure bitmask to
`wiiu/apps/HorizonWiiU/std-entropy-status.log` on the selected SD card without
logging random bytes. Bits 1, 2, 4 indicate 0.2, 0.3, 0.4 call errors; bits
8, 16, 32 indicate obviously unchanged output; 64 indicates duplicate buffers;
and 128/256 indicate a failing or all-zero 0.3/0.4 uninitialized-buffer fill.
A zero mask rules out only these failures, not weak or predictable entropy.

On 2026-11-07, the Cafe probe built and its PowerPC ELF contained
`__getrandom_custom`, `__getrandom_v03_custom`, and `__getrandom_v04_custom`.
Before the startup guard was added, RPX SHA-256
`E701BACE982296F4DE3C85C923B5A27159B696F2CB4D5E90670D48B2A6909AAC`
loaded as checksum `0xb818489e` in both isolated Cemu 2.6 runs. Stock Cemu
returned `failure_flags=504`; the locally patched Cemu returned
`failure_flags=0`, with the guest calling `coreinit.exit(0)`. Evidence is in
`$workspace\HorizonWiiU-cemu-nss-20261107\run-production-stock` and
`run-production-patched`.

After adding the guard, RPX SHA-256
`35C95371FDBEC3455239769A239DB647AA5064E3A225B6801ADA0CBDE74BC66E`
loaded with Cemu checksum `0x0fc2d476`. With Coreinit logging enabled, stock
Cemu logged `Cafe system RNG failed its startup check` and did not write a
success status; patched Cemu wrote `failure_flags=0` and the guest called
`coreinit.exit(0)`. The processes belonged to the isolated tests and were
stopped after their results were captured. Logs and status are in
`$workspace\HorizonWiiU-cemu-nss-20261107\run-guard-stock-logging` and
`run-guard-patched`. This is Cemu-only smoke-test evidence, not a cryptographic
or real-hardware validation.
