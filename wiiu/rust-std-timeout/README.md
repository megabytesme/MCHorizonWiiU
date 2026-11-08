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
  `MSG_DONTWAIT`; `SO_SNDTIMEO` stays explicitly unsupported because nsysnet
  exposes no send-timeout mechanism.
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

This is a proposed source patch, not a validated std implementation. No build
or Cemu run was performed. nsysnet has no `SO_RCVTIMEO`/`SO_SNDTIMEO`; this
design depends on `select` plus WUT's `MsgFlags::DONTWAIT` (`0x20`). The WUT
binding exposes a 32-bit `FdSet` and `FD_SETSIZE = 32`, hence the explicit
range rejection. Cemu's nsysnet HLE has been observed to toggle the host socket
blocking mode around receive calls; concurrent reads on the same descriptor
may therefore interact even though `DONTWAIT` is passed. The probe below is
intended to establish the target behavior before the patch is applied to the
toolchain. Hardware behavior also remains unverified.

The emulation is at each `recv`/`recvfrom` syscall boundary: a higher-level
`read_exact` that makes multiple reads can receive a fresh timeout per read,
as standard-library socket timeouts generally apply per operation. A timeout
update racing an already-started receive does not alter that receive's
snapshot. `Interrupted` from `select` is retried against the same deadline, so
repeated signals cannot extend the configured timeout.

## Bounded target probe

The RPX probe in `src/lib.rs` uses local TCP and UDP sockets. It checks TCP
timeout duration, clone-shared timeout getter, clearing back to a blocking
read, nonblocking `WouldBlock`, `peek`, zero timeout rejection, and read-
shutdown cancellation. UDP coverage checks receive timeout duration, `peek`
retention, and that a zero-length datagram is received as a datagram and
consumed. Potentially blocking TCP and UDP data operations (reads, peeks,
writes, and sends) run on worker threads, and the caller waits only for finite
channel deadlines. TCP connect has its own finite timeout; listener accept
polls a nonblocking socket to a deadline. The shutdown test waits for its
worker to announce that it is about to call `read` before issuing shutdown.
This narrows the scheduling race, but does not prove the worker has entered
nsysnet's `select` or receive syscall; the current public std API provides no
such synchronization point. Failure mask bits are declared alongside the
probe. Workers can remain blocked on a failed case until process exit; the
probe does not join them without a bound. A stuck worker can consume data
intended for a later case, causing additional failures.

Build only after review, with the isolated Cafe stage1 compiler/sysroot used
for std, and run the RPX in Cemu:

```sh
export HORIZON_WIIU_STD_RUSTC=/path/to/stage1/bin/rustc
export HORIZON_WIIU_STD_SYSROOT=/path/to/isolated-cafe-sysroot
export HORIZON_WIIU_TIMEOUT_PROBE_BUILD_DIR=/tmp/horizonwiiu-socket-timeout-probe
bash wiiu/rust-std-timeout/build_probe.sh
```

Expected success output: `Cafe std TcpStream timeout probe: PASS failures=0x00000000`.
This scratch patch and probe have not been built or executed.
