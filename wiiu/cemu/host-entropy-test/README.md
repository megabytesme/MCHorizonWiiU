# Isolated Cemu host entropy test harness

This is a development-only experiment. It is not included in normal player
builds and must not be used on a physical Wii U. The normal Cafe NSS startup
check remains enabled; player builds have no host-entropy fallback. This harness
wraps the two NSS imports only in a separately linked test RPX.

`prepare-pool.ps1` creates exactly 1 MiB from Windows' OS cryptographic RNG in
an NTFS profile that passes private ACL and ancestor checks. The guest wrapper
claims it once, verifies its size, removes the file, and serves disjoint ranges
under a lock, wiping consumed bytes. Missing, stale, short, oversized, or
exhausted pools fail closed. It never logs pool contents. Removing a file or
wiping memory does not guarantee that filesystem or host-memory remnants are
securely erased. Administrators and processes running as the same Windows user
are trusted; a modified emulator or profile is not protected by this mechanism.
Never copy a pool, reuse a profile containing one, or package it in an RPX.

Use a fresh, isolated Cemu portable profile and game directory on NTFS. Ensure
that Cemu is stopped, and that its `portable/sdcard/wiiu/apps/HorizonWiiU`
directory and profile parent are private to the current user, SYSTEM, and
Administrators. Run from PowerShell:

```powershell
./wiiu/cemu/host-entropy-test/prepare-pool.ps1 -PortableDirectory C:\path\to\profile\portable
```

The script refuses an existing pool or claim. Prepare a fresh pool before every
run. After a failed or interrupted run, stop Cemu and discard the isolated
pool and claim; do not retry with the same bytes. Do not put either file in a
log, archive, repository, or shared folder.

## Smoke test

Build the actual `wiiu/entropy/probe` archive as described in
[`../../entropy/README.md`](../../entropy/README.md). With devkitPro, WUT,
Ninja, `cargo-wiiu`, and `readrpl` on PATH, configure this directory in a new
scratch build directory using `powerpc-eabi-cmake`, setting
`HORIZON_ENTROPY_STATICLIB` to that archive. Build with `cmake --build`. Set
`HORIZON_TEST_PREINIT=ON` to exercise the real entropy adapter before
`WHBProcInit`; the default instead races four first-time initializers. Both
variants also exercise concurrent requests, the real getrandom adapters,
exhaustion, and a null zero-length request. A passing run reports
`Host pool validation: failure flags=0` and exits 0. Run each variant with a
fresh pool. Before game testing, also verify that missing, short, and existing
claim files fail closed; never inspect or print pool bytes.

## Separately relinking a game test RPX

`link-game.sh` accepts a **new** build directory, a prebuilt Cafe game archive,
the configured Winit `tv_size.c`, existing diagnostic argument and session-lock
objects, their process-ID wrapper symbol, the Cafe GLSL compiler path, and
`ON` or `OFF` for `HORIZON_WIIU_DISABLE_AX_AUDIO`, in that order. Choose the
audio setting used by the original build. Set `DEVKITPRO` and `DEVKITPPC` and
put the required tools on PATH.
It refuses an existing build directory and does not alter the supplied archive
or previous RPXs. Paths containing spaces or commas are not supported by its
linker-flag interface. The result is an emulator test artifact only. Launch it
only in the isolated profile after creating a fresh pool and, if needed, a fresh
control endpoint with `../prepare-control.ps1`.

The test wrapper and profile do not make Cemu's NSS implementation or this
finite pool suitable for normal play, multiplayer security, or production use.
An emulator-side cryptographic NSS implementation and physical-hardware tests
remain separate work.
