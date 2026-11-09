# Cafe std TCP receive-timeout patch and probe

This scratch subtree is an independently reviewable follow-on to
`wiiu/rust-std-socket/cafe-socket-clone.patch`. `library/std/src/sys/net/connection/socket/cafe.rs`
is the clone-patched Cafe source plus the proposed timeout change;
`cafe-clone-base.rs` is the exact clone-patched base. Review the timeout delta
with:

```sh
git diff --no-index -- cafe-clone-base.rs cafe.rs
```

Run that command from `library/std/src/sys/net/connection/socket/`.

## Proposed behavior

- `SO_RCVTIMEO` is emulated in Cafe std with `select` and per-call
  `MSG_DONTWAIT`; setting `SO_SNDTIMEO` remains unsupported because nsysnet
  exposes no send-timeout mechanism, but clearing it with `None` succeeds as a
  no-op because Cafe never installs a send timeout.
- Timeout is shared by cloned handles, including getter/setter behavior. Each
  blocking receive gets a fresh deadline. `None` uses the ordinary blocking
  syscall. Zero is `InvalidInput`.
- The socket's shared nonblocking state is recorded only after the native
  option succeeds. Concurrent setter calls serialize the native option and
  matching state update. Nonblocking reads keep their immediate `WouldBlock`
  behavior even when a timeout is configured. `peek` retains `MSG_PEEK`.
- The select wait uses the original deadline, at most 50 ms per slice, and
  rejects file descriptors outside the 32-bit nsysnet `fd_set` range. The
  short slices let a timed read observe Cafe-side `Shutdown::Read` and return
  EOF without relying on select being woken by shutdown.
- UDP receives use the same emulation because Rust's std socket `set_timeout`
  path is shared. A zero-length UDP receive still calls `recvfrom`, so it
  consumes/peeks a datagram and obtains its peer address normally.

## Limitations for review

The source patch has been built into Cafe stage-1 `std` and its TCP receive
behavior has passed the Cemu probe below. It has not been verified on Wii U
hardware or in the production Horizon client. nsysnet has no
`SO_RCVTIMEO`/`SO_SNDTIMEO`; this design depends on `select` plus WUT's
`MsgFlags::DONTWAIT` (`0x20`). The WUT binding exposes a 32-bit `FdSet` and
`FD_SETSIZE = 32`, hence the explicit range rejection. Cemu's nsysnet HLE has
been observed to toggle the host socket blocking mode around receive calls;
concurrent reads on the same descriptor may therefore interact even though
`DONTWAIT` is passed. The Cemu result is limited to the tested loopback cases.

The emulation is at each `recv`/`recvfrom` syscall boundary: a higher-level
`read_exact` that makes multiple reads can receive a fresh timeout per read,
as standard-library socket timeouts generally apply per operation. A timeout
update racing an already-started receive does not alter that receive's
snapshot. `Interrupted` from `select` is retried against the same deadline, so
repeated signals cannot extend the configured timeout.

## Bounded target probe

The RPX probe in `src/lib.rs` uses a local TCP pair. It checks connect/accept,
write-timeout clearing and rejection, receive timeout duration, clone-shared
timeout getter, clearing back to a blocking read, nonblocking `WouldBlock`,
`peek`, zero timeout rejection, and read-shutdown cancellation. Potentially
blocking reads and writes run on worker threads with finite channel deadlines;
the probe joins completed workers and
returns a failure without proceeding to later checks if a worker misses its
bounded wait. TCP connect has its own finite timeout; listener accept polls a
nonblocking socket to a deadline. The shutdown test waits for its worker to
announce that it is about to call `read` before issuing shutdown. This narrows
the scheduling race, but does not prove the worker has entered nsysnet's
`select` or receive syscall; the current public std API provides no such
synchronization point. The failure mask bits are declared alongside the
probe. UDP is outside this RPX because Cemu's local `UdpSocket::send_to`
stopped progressing in the earlier combined diagnostic; that behavior remains
unverified.

Build with the isolated Cafe stage-1 compiler/sysroot used for `std`, then run
the RPX once in Cemu:

```sh
export HORIZON_WIIU_STD_RUSTC=/path/to/stage1/bin/rustc
export HORIZON_WIIU_STD_SYSROOT=/path/to/isolated-cafe-sysroot
export HORIZON_WIIU_TIMEOUT_PROBE_BUILD_DIR=/tmp/horizonwiiu-socket-timeout-probe
bash wiiu/rust-std-timeout/build_probe.sh
```

Expected success output:
`Cafe std TcpStream timeout probe: PASS failures=0x00000000`.

## Cemu result (2026-11-08)

The Cafe stage-1 `std` was rebuilt from the matching modified toolchain source,
then this RPX was built in
`C:\Users\ben\AppData\Local\Temp\horizonwiiu-socket-timeout-probe-20261108`
with `HORIZON_WIIU_JOBS=32`. Cemu 2.6 ran it once in the isolated profile at
`E:\temp\horizonwiiu-loading-cemu-20261108\portable`. The RPX returned
`Cafe std TcpStream timeout probe: PASS failures=0x00000000`, called
`coreinit.exit(0)`, and has SHA-256
`5cfd6afeadbd1395b04c7045bddc1240e79c0d13c584b74ecfc9d4f4223ea7fa`.
The captured run log is
`E:\temp\horizonwiiu-loading-cemu-20261108\portable\log.txt`.
Its 120 ms TCP receive timeout returned in about 134 ms. The remaining checks
also completed with a zero failure mask: clone-shared timeout getter, clearing
the timeout to blocking, nonblocking `WouldBlock`, `peek`, zero-duration
validation, and read-shutdown cancellation.

That run verified the tested TCP standard-library behavior in Cemu. The later
production-client result below supersedes its statement about HTTPS. Real
hardware and UDP remain unverified. An earlier combined diagnostic reached the
end of its UDP receive-timeout step, then stopped progressing at
`UdpSocket::send_to` in Cemu. The TCP RPX is therefore deliberately TCP-only;
the UDP send stall remains a separate platform/emulator limitation to
investigate before claiming LAN broadcast discovery.

## Cafe HTTP write-timeout clearing and Horizon live ping (2026-11-09)

The isolated Cafe `std` used for the production RPX now treats
`set_write_timeout(None)` as a no-op. Setting `Some(timeout)` still returns
`Unsupported`, because nsysnet has no send-timeout option. The bounded timeout
probe against this stage-0 sysroot returned `failures=0x00000008`; bits
`0x200` (write-timeout clear) and `0x400` (write-timeout set rejection) were
clear. The remaining bit is the probe's separate receive-timeout result check;
the earlier stage-1 probe above passed that check.

The Horizon Loading RPX was then rebuilt against the patched sysroot with 32
jobs and run in Cemu 2.6. It loaded the title and showed the loading screen
after 124 ms. Cemu logged connection attempts to both telemetry hosts
(`153.52.255.62:443` and `144.21.57.120:443`) and no live-ping failure. After
launch, `horizontelemetry.bytesdirect.com/v1/live` reported one active
`dev 0.1.0` session; that endpoint had reported zero before the run. The
production RPX SHA-256 is
`7EA448E102447B479EB2CFFFCE5BC73CCAD8ADA57D168C72301CCA32AD2E9640`.
The Cemu log is
`E:\temp\horizonwiiu-loading-cemu-20261108\portable\log.txt`.

This verifies Cafe HTTPS live-ping delivery in Cemu, including acceptance by
the secondary endpoint. It does not verify Wii U hardware, consented batch
uploads, multiplayer server connections, or UDP discovery. Startup still logs
an unrelated unsupported operation while opening `horizon/logs/latest.log`.
