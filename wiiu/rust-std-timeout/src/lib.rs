//! Bounded behavioral probe for Cafe std TCP receive timeouts.
//! Export `horizon_socket_timeout_probe` from an RPX harness and print its
//! returned failure mask. Potentially blocking receives run on workers observed
//! through finite channel deadlines, so a regression does not hang the RPX.

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const FAIL_BIND: u32 = 1 << 0;
const FAIL_CONNECT: u32 = 1 << 1;
const FAIL_TIMEOUT_GETTER: u32 = 1 << 2;
const FAIL_TIMEOUT_RESULT: u32 = 1 << 3;
const FAIL_ZERO_TIMEOUT: u32 = 1 << 4;
const FAIL_CLEAR_BLOCKING: u32 = 1 << 5;
const FAIL_NONBLOCKING: u32 = 1 << 6;
const FAIL_PEEK: u32 = 1 << 7;
const FAIL_SHUTDOWN_CANCEL: u32 = 1 << 8;
const FAIL_WRITE_TIMEOUT_CLEAR: u32 = 1 << 9;
const FAIL_WRITE_TIMEOUT_SET: u32 = 1 << 10;
const PROBE_WAIT: Duration = Duration::from_secs(3);

unsafe extern "C" {
    fn horizon_socket_timeout_probe_stage(stage: u32);
}

#[inline(never)]
fn report_stage(stage: u32) {
    // SAFETY: the RPX harness provides this diagnostic callback in main.c.
    unsafe { horizon_socket_timeout_probe_stage(stage) };
}

fn join_bounded(worker: JoinHandle<()>) -> bool {
    let deadline = Instant::now() + PROBE_WAIT;
    while !worker.is_finished() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(1));
    }
    worker.is_finished() && worker.join().is_ok()
}

fn wait_for_worker<T>(receiver: mpsc::Receiver<T>, worker: JoinHandle<()>) -> Option<T> {
    let value = receiver.recv_timeout(PROBE_WAIT).ok();
    join_bounded(worker).then_some(value).flatten()
}

fn timed_out<T>(result: &io::Result<T>) -> bool {
    matches!(result, Err(error) if error.kind() == io::ErrorKind::TimedOut)
}

fn connect_pair() -> io::Result<(TcpStream, TcpStream)> {
    report_stage(1);
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    report_stage(2);
    let client = TcpStream::connect_timeout(&address, PROBE_WAIT)?;
    report_stage(3);
    let deadline = Instant::now() + PROBE_WAIT;
    loop {
        match listener.accept() {
            Ok((server, _)) => {
                report_stage(4);
                return Ok((client, server));
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "accept deadline"));
                }
                thread::yield_now();
            }
            Err(error) => return Err(error),
        }
    }
}

fn worker_read(
    stream: TcpStream,
) -> (
    mpsc::Receiver<io::Result<usize>>,
    mpsc::Receiver<()>,
    JoinHandle<()>,
) {
    let (tx, rx) = mpsc::channel();
    let (started_tx, started_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        let mut stream = stream;
        let mut byte = [0];
        report_stage(20);
        let _ = started_tx.send(());
        let result = stream.read(&mut byte);
        report_stage(21);
        let _ = tx.send(result);
        report_stage(22);
    });
    (rx, started_rx, worker)
}

/// Failure bits: timeout, clone-shared getter, clear-to-blocking, nonblocking,
/// peek, zero duration validation, shutdown cancellation, and write-timeout
/// clear/rejection behavior.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_socket_timeout_probe() -> u32 {
    report_stage(0);
    let (client, server) = match connect_pair() {
        Ok(pair) => pair,
        Err(_) => return FAIL_BIND | FAIL_CONNECT,
    };
    let mut failures = 0;
    let clone = match client.try_clone() {
        Ok(clone) => clone,
        Err(_) => return FAIL_CONNECT,
    };
    // Cafe cannot enforce a send timeout, but clearing the default `None`
    // configuration must succeed for clients that set all timeout kinds.
    if client.set_write_timeout(None).is_err() {
        failures |= FAIL_WRITE_TIMEOUT_CLEAR;
    }
    if !matches!(
        client.set_write_timeout(Some(Duration::from_millis(1))),
        Err(ref error) if error.kind() == io::ErrorKind::Unsupported
    ) {
        failures |= FAIL_WRITE_TIMEOUT_SET;
    }

    let short = Duration::from_millis(120);
    if client.set_read_timeout(Some(short)).is_err()
        || clone.read_timeout().ok() != Some(Some(short))
    {
        failures |= FAIL_TIMEOUT_GETTER;
    }
    report_stage(5);
    let start = Instant::now();
    let (rx, _, worker) = worker_read(client.try_clone().expect("clone was supported"));
    let timeout_result = wait_for_worker(rx, worker);
    report_stage(6);
    if timeout_result.is_none() {
        return failures | FAIL_TIMEOUT_RESULT;
    }
    if !timeout_result.as_ref().is_some_and(timed_out)
        || start.elapsed() < short
        || start.elapsed() > PROBE_WAIT
    {
        failures |= FAIL_TIMEOUT_RESULT;
    }
    report_stage(7);
    if !matches!(
        client.set_read_timeout(Some(Duration::ZERO)),
        Err(ref error) if error.kind() == io::ErrorKind::InvalidInput
    ) {
        failures |= FAIL_ZERO_TIMEOUT;
    }
    report_stage(8);

    // Clearing the timeout must restore a truly blocking read. Verify the
    // worker remains pending briefly, then release it with a byte from peer.
    if clone.set_read_timeout(None).is_err() {
        failures |= FAIL_CLEAR_BLOCKING;
    }
    let (rx, started, worker) = worker_read(clone.try_clone().expect("clone was supported"));
    if !matches!(started.recv_timeout(PROBE_WAIT), Ok(())) {
        return failures | FAIL_CLEAR_BLOCKING;
    }
    let early_result = rx.recv_timeout(Duration::from_millis(80));
    if !matches!(early_result, Err(mpsc::RecvTimeoutError::Timeout)) {
        let _ = join_bounded(worker);
        return failures | FAIL_CLEAR_BLOCKING;
    } else {
        if !matches!(
            worker_write_all(server.try_clone().expect("clone was supported"), b"x"),
            Some(Ok(()))
        ) {
            let _ = client.shutdown(Shutdown::Both);
            let _ = join_bounded(worker);
            return failures | FAIL_CLEAR_BLOCKING;
        }
        let result = rx.recv_timeout(PROBE_WAIT);
        let worker_exited = join_bounded(worker);
        if !matches!(result, Ok(Ok(1))) || !worker_exited {
            return failures | FAIL_CLEAR_BLOCKING;
        }
    }
    report_stage(9);

    // A timeout configured on a nonblocking stream must not turn WouldBlock
    // into a wait. Peek uses the same timeout path but leaves the byte queued.
    if client.set_nonblocking(true).is_err() || clone.set_read_timeout(Some(short)).is_err() {
        return failures | FAIL_NONBLOCKING;
    } else {
        let (rx, _, worker) = worker_read(client.try_clone().expect("clone was supported"));
        match wait_for_worker(rx, worker) {
            None => return failures | FAIL_NONBLOCKING,
            Some(Err(ref e)) if e.kind() == io::ErrorKind::WouldBlock => {}
            _ => failures |= FAIL_NONBLOCKING,
        }
    }
    if client.set_nonblocking(false).is_err()
        || !matches!(
            worker_write_all(server.try_clone().expect("clone was supported"), b"p"),
            Some(Ok(()))
        )
    {
        failures |= FAIL_PEEK;
    } else {
        let peek_stream = clone.try_clone().expect("clone was supported");
        let (tx, rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            let mut byte = [0];
            let _ = tx.send(peek_stream.peek(&mut byte).map(|n| (n, byte[0])));
        });
        match wait_for_worker(rx, worker) {
            Some(Ok((1, b'p'))) => {
                let (rx, _, worker) = worker_read(clone);
                match wait_for_worker(rx, worker) {
                    None => return failures | FAIL_PEEK,
                    Some(Ok(1)) => {}
                    _ => failures |= FAIL_PEEK,
                }
            }
            None => return failures | FAIL_PEEK,
            _ => failures |= FAIL_PEEK,
        }
    }
    report_stage(10);

    // Shutdown(Read) must release a timed read without waiting for its timeout.
    if client.set_read_timeout(Some(PROBE_WAIT)).is_err() {
        failures |= FAIL_SHUTDOWN_CANCEL;
    } else {
        let (rx, started, worker) = worker_read(client.try_clone().expect("clone was supported"));
        if !matches!(started.recv_timeout(PROBE_WAIT), Ok(())) {
            return failures | FAIL_SHUTDOWN_CANCEL;
        }
        // The worker has announced that it is about to read. This narrows the
        // race but cannot prove it has entered nsysnet's select yet.
        if client.shutdown(Shutdown::Read).is_err() {
            failures |= FAIL_SHUTDOWN_CANCEL;
        }
        let result = rx.recv_timeout(Duration::from_millis(500));
        let worker_exited = join_bounded(worker);
        if !matches!(result, Ok(Ok(0))) || !worker_exited {
            failures |= FAIL_SHUTDOWN_CANCEL;
        }
    }
    let _ = server.shutdown(Shutdown::Both);
    report_stage(11);

    failures
}

fn worker_write_all(stream: TcpStream, data: &'static [u8]) -> Option<io::Result<()>> {
    let (tx, rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        let mut stream = stream;
        let _ = tx.send(stream.write_all(data));
    });
    wait_for_worker(rx, worker)
}
