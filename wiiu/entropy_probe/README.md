# Cafe std entropy diagnostic

This opt-in RPX calls the Cafe Rust standard library's `SystemRng` directly.
It does not implement a random source, use a getrandom custom backend, or log
entropy. It is separate from the client and the world-renderer diagnostic.

With the Cafe stage-1 compiler, devkitPro/WUT, Ninja and `cargo-wiiu` available,
run from the repository root in WSL:

```sh
HORIZON_WIIU_STD_RUSTC=$workspace/HorizonWiiU-toolchain/rust-wiiu-cafe/build/host/stage1/bin/rustc \
HORIZON_WIIU_ENTROPY_BUILD_DIR=/tmp/horizonwiiu-entropy-probe \
HORIZON_WIIU_JOBS=1 bash wiiu/entropy_probe/build.sh
```

The RPX is `cmake/horizon_wiiu_entropy_probe.rpx` in the build directory. The
build uses `prepare_wut_startup.sh` to select WUT startup and heap setup, and
`cargo wiiu rpx` to preserve Cafe import sections. The host validator test is:

```sh
CARGO_TARGET_DIR=/tmp/horizonwiiu-entropy-host cargo +nightly test \
  --manifest-path wiiu/entropy_probe/Cargo.toml --jobs 1
```

The diagnostic returns a bitmask and reports it via `OSReport` and, if the SD
card and status directory are available, to
`wiiu/apps/HorizonWiiU/std-entropy-status.log`:

- Bit 1: first zero-filled buffer stayed entirely zero.
- Bit 2: second zero-filled buffer stayed entirely zero.
- Bit 4: the two buffers are identical.
- Bit 8: the buffer initially filled with `0x55` stayed unchanged.

A nonzero result is a failure. A zero result rules out those specific failures;
it does **not** establish cryptographic security. `SystemRng` may abort before
reporting if the system reports an error. A missing status file is not a pass.

On 2026-11-07, the host validator passed 1/1 and the PowerPC RPX built. In an
isolated stock Cemu 2.6 installation, RPX SHA-256
`443f6cd1fb1b7da459c1202700bfbe692ca2307abe0b7850c025933d83073a6e`
loaded with Cemu checksum `0x5966ba5e`, reached `Run title`, and exited with
`failure_flags=15`. All three buffers remained unchanged. This is a Cemu
failure of the current Cafe std RNG path; it is not a real-hardware result.
The RPX, portable SD status and Cemu log were preserved in
`$workspace\builds\HorizonWiiU-entropy-cemu-48eda107e40340e4b353fd44f7fd31c8`.
Temporary Cemu copies were disappearing during this session, so the isolated
emulator was placed in that new non-OneDrive build directory.

The PowerShell launch used that directory's own portable SD and settings:

```powershell
$base = '$workspace\builds\HorizonWiiU-entropy-cemu-48eda107e40340e4b353fd44f7fd31c8'
$cemu = Join-Path $base 'Cemu_2.6'
Start-Process -FilePath "$cemu\Cemu.exe" -ArgumentList @('-g', "`"$base\entropy.rpx`"") -WorkingDirectory $cemu -WindowStyle Hidden -PassThru
```

For a new run, create an isolated portable Cemu directory and its
`portable/sdcard/wiiu/apps/HorizonWiiU` directory, copy the built RPX into it,
and adapt `$base` and the RPX path. Read its status file and `portable/log.txt`;
the installed user's Cemu data is not needed.
