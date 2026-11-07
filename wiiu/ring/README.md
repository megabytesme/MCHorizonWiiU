# Cafe Ring preparation

Ring 0.17.14's `SystemRandom` already calls getrandom 0.2, but its OS allowlist
does not include Cafe. The one-line patch enables that existing implementation
on Cafe; it does not change desktop targets or implement a random generator.
The executable still must link and call the guarded `wiiu/entropy` adapter
before random-dependent initialization, with the custom getrandom backend
configuration described in [its README](../entropy/README.md). The full client
does not yet perform that initialization.

The helper does not download or vendor Ring. It accepts the exact crates.io
`ring-0.17.14.crate` (SHA-256
`a4689e6c2294d81e88dc6261c768b63bc4fcdb852be6d1352498b114f61383b7`), copies
it to a private temporary file, checks that copy, and extracts and patches the
same copy into a new directory. It rejects an existing destination; a partial
destination after an extraction or patch failure is preserved and reported.
All original source and license files remain in the scratch directory.

For an isolated WSL build, locate the already-cached `.crate` archive and use
a new scratch build directory:

```sh
scratch="$(mktemp -d "${TMPDIR:-/tmp}/horizonwiiu-ring-build.XXXXXXXX")"
bash wiiu/ring/prepare.sh /path/to/ring-0.17.14.crate "$scratch/ring"
# In a disposable wrapper or source copy, with Cafe RUSTC and WUT configured:
RUSTFLAGS='--cfg getrandom_backend="custom"' cargo +nightly \
  --config "patch.crates-io.ring.path=\"$scratch/ring\"" \
  check --target powerpc-nintendo-cafe --jobs 32 --keep-going
```

The path override changes Cargo's lock resolution. Use a disposable workspace
and lockfile; do not run this against the working checkout expecting its lock
to remain unchanged. This helper prepares a dependency only; it is not wired
into the game build and does not make the full client compile.

On 2026-11-07, with the checkout at `5a264111818f1080c18c9513ffe0dd8576bf341a`,
the helper's output matched the independently patched crate. A disposable
original-client wrapper using the `--config` override reached
`horizon-server-files` without the earlier Rustls `SecureRandom` errors; it
still failed on unsupported Winit, socket2, if-addrs and mio, and on a missing
64-bit atomic in `horizon-server-files`. The full log is
`%TEMP%\HorizonWiiU-ring-cafe-check-20261107\config-check.log`. This check
does not link or execute the client.

A separate Rust/WUT diagnostic linked the patched Ring and the existing entropy
adapter. On isolated NSS-patched Cemu 2.6 it returned `failure_flags=0` and
`coreinit.exit(0)` for SystemRandom, SHA-256 and AES-GCM known answers, an
Ed25519 verification vector and X25519 two-party agreement. The RPX and source
are under `%TEMP%\HorizonWiiU-ring-cafe-check-20261107`; RPX SHA-256 is
`9d810f5a42948d5829d5cec11240615476fc65e4982faaa1610c449aef4e2bf0`.
This is limited big-endian smoke-test evidence, not a TLS handshake, a security
audit, or real-hardware validation. The same RPX on isolated stock Cemu logged
`Cafe system RNG failed its startup check` and wrote no success status. Stock
Cemu's NSS implementation is known to leave random buffers unchanged, and the
entropy adapter fails closed there.
