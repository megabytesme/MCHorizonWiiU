# Cafe std TLS virtualization patch and probes

This subtree is a scratch patch for the Cafe Rust standard library. It does not
modify the installed toolchain. `library/std/src/sys/thread_local/key/cafe.rs`
is the proposed replacement, and `cafe-base.rs` is copied from the stage1
source checkout for review. `library/std/src/sys/thread/cafe.rs` contains the
thread-exit table release, while `library/std/src/sys/thread_local/mod.rs`
re-exports that release hook only in the Cafe key branch. Both original source
files are preserved as `thread/cafe-base.rs` and `thread_local/mod-base.rs`.

Review the exact deltas from `wiiu/rust-std-tls/library/std/src/sys`:

```sh
git diff --no-index -- thread_local/key/cafe-base.rs thread_local/key/cafe.rs
git diff --no-index -- thread/cafe-base.rs thread/cafe.rs
git diff --no-index -- thread_local/mod-base.rs thread_local/mod.rs
```

## Design and limits

WUT's `coreinit/thread.h` declares `OS_THREAD_SPECIFIC_0` through
`OS_THREAD_SPECIFIC_13` application-available and explicitly reserves slots 14
and 15 for WUT (`OSThreadSpecificID`, lines 60–80). This backend reserves app
slot 13 as the per-thread table root. The current loading app's native C,
GX2-presenter and VPAD bridge sources do not call `OSGetThreadSpecific` or
`OSSetThreadSpecific`; the current linked Rust static library's only such
references are from Cafe std. Any other code linked into an application using
this backend must honor the same process-wide reservation of slot 13.

Rust logical keys are generation-tagged and independent of OS slot numbers.
There are at most 511 live logical keys and 256 live per-thread tables. Key
indices are reusable after `destroy`; the generation stored with each thread's
cell prevents a raced-away key's stale value or destructor from being reused
under a later key. Generation exhaustion retires that key index. The table
pool has no allocator, standard mutex or TLS dependency: it is fixed BSS and
claimed with atomics. It occupies about 1 MiB on 32-bit Cafe (256 × 512 cells,
each carrying a generation and pointer). Exhaustion aborts with a specific
runtime error instead of aliasing another thread's table.

The key allocator uses a per-index atomic state transition. A concurrent
`LazyKey` loser can destroy its unpublished candidate and return the index to
the free pool. `destroy` clears the destructor before publishing the free
state; per-thread values are invalidated by generation and cleared when tables
are reclaimed. Destruction still follows the std key contract: callers must
not continue using a key after destroying it. Destructor dispatch clears a
cell before calling its callback and repeats up to four passes. The std Cafe
thread wrapper runs destructors, then runtime thread cleanup, then releases the
table so `thread::current()` remains available during destructors and runtime
cleanup. Values left after the four rounds are cleared from the table and
leaked, matching the existing Cafe backend's four-round behavior.

The WUT slot reservation is based on the WUT enum and an audit of this app's
native bridges and linked Rust static library. It is an embedding contract,
not an OS-enforced reservation. A different app that already uses slot 13
must select another application-owned slot and audit all linked dependencies.

## Host-side semantic tests

The host test crate includes the production `cafe.rs` directly and supplies
only a thread-local shim for `OSGetThreadSpecific`/`OSSetThreadSpecific` plus
the minimal `std` reexports that Cafe std normally provides. It therefore
exercises the actual key allocator, table pool and destructor loop on the host;
only the underlying OS TSD storage is replaced. Tests cover 128 live keys,
generation-safe reuse, contended create/destroy candidates, thread isolation,
table reclamation and reuse, exactly four self-resurrection rounds,
cross-key resurrection, dtor access to `thread::current`, and bounded table
pool exhaustion/recovery.

```sh
cargo test --manifest-path wiiu/rust-std-tls/host-tests/Cargo.toml
```

## Cafe target probe

`src/lib.rs` exports `horizon_tls_probe` for a WUT RPX wrapper. It calls a
test-only hook compiled into the isolated Cafe std to check exactly four
self-resurrection rounds in the production destructor loop. It also races
first-use of 32 `thread_local!` keys, holds 128 distinct parent/capacity keys
live simultaneously (32 parent plus 96 capacity statics), checks
parent/worker value isolation, checks a destructor that initializes an earlier
key for the next destructor round, and creates more sequential Rust threads
than the table pool to check reclamation. A worker TLS destructor calls
`std::thread::current()` before Cafe runtime cleanup. Worker completion and
joins are bounded. It does not intentionally overflow target
key or live-thread limits because those paths abort the process; the host tests
exercise bounded overflow instead.

Apply all three source files to an **isolated source clone** of the Cafe Rust
checkout, then rebuild that clone's target `std` into a separate sysroot. The
third file is required because Cafe's `thread_local::key` branch previously
re-exported only `run_dtors`; the thread backend now also needs its scoped
`pub(crate)` table-release hook. For example, with `STD_SOURCE` set to the
isolated source clone:

```sh
cp wiiu/rust-std-tls/library/std/src/sys/thread_local/key/cafe.rs \
  "$STD_SOURCE/library/std/src/sys/thread_local/key/cafe.rs"
cp wiiu/rust-std-tls/library/std/src/sys/thread/cafe.rs \
  "$STD_SOURCE/library/std/src/sys/thread/cafe.rs"
cp wiiu/rust-std-tls/library/std/src/sys/thread_local/mod.rs \
  "$STD_SOURCE/library/std/src/sys/thread_local/mod.rs"
```

Build that source clone's target `std` into a separate sysroot with
`--cfg cafe_tls_probe --check-cfg=cfg(cafe_tls_probe)` so the probe-only
self-rearm hook is present. Do not point the build at the installed stage1
source or sysroot. Build and run the probe only after independent review:

```sh
export HORIZON_WIIU_STD_RUSTC=/path/to/isolated-stage1/bin/rustc
export HORIZON_WIIU_STD_SYSROOT=/path/to/isolated-cafe-sysroot
export HORIZON_WIIU_TLS_PROBE_BUILD_DIR=/tmp/horizonwiiu-tls-probe
bash wiiu/rust-std-tls/build_probe.sh
```

Expected success output: `Cafe std TLS virtualization probe: PASS failures=0x00000000`.

## Validation record

Validated 2026-11-08:

- WSL host tests: 8 passed, 0 failed, including production key capacity and
  thread-table exhaustion/recovery.
- Isolated Cafe std build: `RUSTFLAGS="--cfg=cafe_tls_probe
  --check-cfg=cfg(cafe_tls_probe)" python3 x.py build --stage 0 library/std
  --target powerpc-nintendo-cafe` completed successfully. The source clone,
  build output and sysroot overlay were kept separate from the installed
  toolchain.
- `build_probe.sh` built a 239,167-byte RPX; SHA-256:
  `6D3A6D759DADA8F98D42E0A29A9C15C4300B91356F780B3043FA360C8E72D02D`.
- The RPX ran in the isolated patched Cemu build (`a6fb0a4`) and reported
  `Cafe std TLS virtualization probe: PASS failures=0x00000000`, then exited
  through `coreinit.exit(0)`. The captured log is in
  `C:\Users\ben\AppData\Local\Temp\HorizonWiiU-winit-subset-cee3342fba1e412e8b0f4dbea08c090a\title-original-cemu\portable\log.txt`.

This validates the patched std TLS behavior in Cemu. It does not yet validate
Wii U hardware or the full Horizon client's startup. The full client and every
crate with inlined std TLS calls still need rebuilding against this sysroot
before repeating the Horizon Cemu startup check.
