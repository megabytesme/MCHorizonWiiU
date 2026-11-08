# Cafe `TcpStream::try_clone` patch

Base: `rust-wiiu-cafe` commit `62606b70e351edb297db1fdd4b99c8b8ca440d38` (`cafe: update to cafe-sys`, 2026-10-13).

Apply `cafe-socket-clone.patch` at that repository root. The patch changes only `library/std/src/sys/net/connection/socket/cafe.rs`.

Cafe's exposed `cafe_sys::nsys::net::socket` surface has no `dup`/`dup2` or descriptor-control API. The patch implements the standard-library internal `Socket::duplicate` by cloning an `Arc` around the single raw socket handle. All handles retain the same fd; `close` runs exactly once when the last handle drops. New and accepted sockets still enter through their existing OS creation paths and are immediately wrapped. Read/write/connect/options/shutdown continue to issue the same WUT calls against that fd. This matches `TcpStream::try_clone`'s shared-stream behavior: bytes and socket options are shared, and shutdown affects both handles.

This is not a public descriptor duplication facility. It is safe for the standard library's owned `Socket` handles because the raw fd is private to this module and every access keeps a live `Socket` borrow. Review for any downstream internal caller that assumes `Socket::duplicate` returns a separately closeable OS descriptor before applying to another std revision.

The patch was generated and built in an isolated source copy; the installed toolchain was not changed. The target build used the existing Cafe compiler's panic-abort configuration and a separately assembled sysroot. It does not establish panic-unwind support.

## Suggested validation

Run the existing Rust std networking tests for clone and close/shutdown behavior in a Cafe toolchain build, in particular `tcp_clone_smoke`, `tcp_clone_two_read`, `tcp_clone_two_write`, `clone_while_reading`, `shutdown_smoke`, and `close_read_wakes_up`. The small `clone_lifetime_harness.rs` records the key Cafe-specific lifetime contract for inclusion/adaptation in the std test suite. Also verify nonblocking toggles are shared across cloned handles, as WUT implements them as socket options on the shared endpoint.

Known Cafe socket limitations remain: only IPv4 is supported; socket timeout options (`set_timeout`/`timeout`) are unsupported; true `dup` is unavailable. The `TcpStream::connect_timeout` method itself is implemented on Cafe using nonblocking connect plus `select`, so the source-only harness uses it to bound connection attempts. This patch does not change those limitations.

## Cemu-only RPX probe

`src/lib.rs`, `main.c`, `CMakeLists.txt`, and `build_probe.sh` make a small RPX
that uses the patched Cafe `std::net::TcpStream` against a loopback listener in
the same process. It does not copy Horizon's multiplayer protocol or run the
game. The assertions cover: a clone remains usable after dropping the original,
the nonblocking option set through the original is visible through the clone,
both handles see the same TCP byte stream, and dropping the last clone produces
EOF at the peer. Poll loops and `connect_timeout` are bounded to five seconds;
there are no blocking read/accept calls.

Build it with the same stage1 `rustc` used for the std compile and explicitly
point Cargo at that compile's isolated sysroot:

```sh
export HORIZON_WIIU_STD_RUSTC=/home/ben/HorizonWiiU-toolchain/rust-wiiu-cafe/build/x86_64-unknown-linux-gnu/stage1/bin/rustc
export HORIZON_WIIU_STD_SYSROOT=/mnt/e/temp/horizonwiiu-rust-cafe-socket-overlay-20261108
export HORIZON_WIIU_SOCKET_PROBE_BUILD_DIR=/mnt/e/temp/horizonwiiu-socket-clone-patched-probe-20261108
export HORIZON_WIIU_JOBS=1
bash wiiu/rust-std-socket/build_probe.sh
```

The build requires devkitPro/WUT, `cargo-wiiu`, the Cafe cross tools, `readrpl`,
and a rustup `nightly` Cargo. The RPX is written under
`$HORIZON_WIIU_SOCKET_PROBE_BUILD_DIR/cmake/`.

For Cemu only, launch the RPX directly with `Cemu.exe -g <absolute-path-to-rpx>`
and inspect Cemu's OSReport output. A passing run prints
`Cafe std TcpStream clone probe: PASS failures=0x00000000`; a nonzero mask
identifies the first failed step. The patched build printed PASS in Cemu; the
RPX SHA-256 was `0b14a3ac5d49226684d02fabb20061f4118c1e221ea2497c73de2e2ab144a9fb`.
The unpatched sysroot returned failure bit `0x10` at `try_clone`; archive
inspection confirmed the passing build linked the new patched std rather than
that older library. The log is `E:\dev\HorizonWiiU-checkpoints\2026-11-08-live-title\cemu-cafe-std-socket-clone-pass.log`.
Five additional lifetime/option harness tests passed on the Linux host. These
tests do not demonstrate internet access, Horizon server interoperability,
Wii U hardware behavior, blocking-read shutdown, or panic-unwind support.
