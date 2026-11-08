//! Bounded behavioral probe for Cafe std TCP receive timeouts.
//! Export `horizon_socket_timeout_probe` from an RPX harness and print its
//! returned failure mask. Potentially blocking receives run on workers observed
//! through finite channel deadlines, so a regression does not hang the RPX.

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream, UdpSocket};
use std::sync::mpsc;
use std::thread;
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
const FAIL_UDP_TIMEOUT: u32 = 1 << 9;
const FAIL_UDP_PEEK: u32 = 1 << 10;
const FAIL_UDP_ZERO_DATAGRAM: u32 = 1 << 11;
const PROBE_WAIT: Duration = Duration::from_secs(3);

fn timed_out<T>(result: io::Result<T>) -> bool {
    result.is_err_and(|error| error.kind() == io::ErrorKind::TimedOut)
}

fn connect_pair() -> io::Result<(TcpStream, TcpStream)> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let client = TcpStream::connect_timeout(&address, PROBE_WAIT)?;
    let deadline = Instant::now() + PROBE_WAIT;
    loop {
        match listener.accept() {
            Ok((server, _)) => return Ok((client, server)),
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

fn worker_read(stream: TcpStream) -> (mpsc::Receiver<io::Result<usize>>, mpsc::Receiver<()>) {
    let (tx, rx) = mpsc::channel();
    let (started_tx, started_rx) = mpsc::channel();
    thread::spawn(move || {
        let mut stream = stream;
        let mut byte = [0];
        let _ = started_tx.send(());
        let _ = tx.send(stream.read(&mut byte));
    });
    (rx, started_rx)
}

/// Failure bits: timeout, clone-shared getter, clear-to-blocking, nonblocking,
/// peek, zero duration validation, and shutdown cancellation.
#[no_mangle]
pub extern "C" fn horizon_socket_timeout_probe() -> u32 {
    let (client, mut server) = match connect_pair() {
        Ok(pair) => pair,
        Err(_) => return FAIL_BIND | FAIL_CONNECT,
    };
    let mut failures = 0;
    let clone = match client.try_clone() {
        Ok(clone) => clone,
        Err(_) => return FAIL_CONNECT,
    };

    let short = Duration::from_millis(120);
    if client.set_read_timeout(Some(short)).is_err()
        || clone.read_timeout().ok() != Some(Some(short))
    {
        failures |= FAIL_TIMEOUT_GETTER;
    }
    let start = Instant::now();
    let mut byte = [0];
    let (rx, _) = worker_read(client.try_clone().expect("clone was supported"));
    if !matches!(rx.recv_timeout(PROBE_WAIT), Ok(result) if timed_out(result))
        || start.elapsed() < short
        || start.elapsed() > PROBE_WAIT
    {
        failures |= FAIL_TIMEOUT_RESULT;
    }
    if !matches!(
        client.set_read_timeout(Some(Duration::ZERO)),
        Err(ref error) if error.kind() == io::ErrorKind::InvalidInput
    ) {
        failures |= FAIL_ZERO_TIMEOUT;
    }

    // Clearing the timeout must restore a truly blocking read. Verify the
    // worker remains pending briefly, then release it with a byte from peer.
    if clone.set_read_timeout(None).is_err() {
        failures |= FAIL_CLEAR_BLOCKING;
    }
    let (rx, started) = worker_read(clone.try_clone().expect("clone was supported"));
    if !matches!(started.recv_timeout(PROBE_WAIT), Ok(())) {
        failures |= FAIL_CLEAR_BLOCKING;
    }
    if rx.recv_timeout(Duration::from_millis(80)).is_ok() {
        failures |= FAIL_CLEAR_BLOCKING;
    }
    if !matches!(
        worker_write_all(server.try_clone().expect("clone was supported"), b"x"),
        Some(Ok(()))
    ) || !matches!(rx.recv_timeout(PROBE_WAIT), Ok(Ok(1)))
    {
        failures |= FAIL_CLEAR_BLOCKING;
    }

    // A timeout configured on a nonblocking stream must not turn WouldBlock
    // into a wait. Peek uses the same timeout path but leaves the byte queued.
    if client.set_nonblocking(true).is_err() || clone.set_read_timeout(Some(short)).is_err() {
        failures |= FAIL_NONBLOCKING;
    } else {
        let (rx, _) = worker_read(client.try_clone().expect("clone was supported"));
        if !matches!(rx.recv_timeout(PROBE_WAIT), Ok(Err(ref e)) if e.kind() == io::ErrorKind::WouldBlock)
        {
            failures |= FAIL_NONBLOCKING;
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
        thread::spawn(move || {
            let mut byte = [0];
            let _ = tx.send(peek_stream.peek(&mut byte).map(|n| (n, byte[0])));
        });
        match rx.recv_timeout(PROBE_WAIT) {
            Ok(Ok((1, b'p'))) => {
                let (rx, _) = worker_read(clone);
                if !matches!(rx.recv_timeout(PROBE_WAIT), Ok(Ok(1))) {
                    failures |= FAIL_PEEK;
                }
            }
            _ => failures |= FAIL_PEEK,
        }
    }

    // Shutdown(Read) must release a timed read without waiting for its timeout.
    if client.set_read_timeout(Some(PROBE_WAIT)).is_err() {
        failures |= FAIL_SHUTDOWN_CANCEL;
    } else {
        let (rx, started) = worker_read(client.try_clone().expect("clone was supported"));
        if !matches!(started.recv_timeout(PROBE_WAIT), Ok(())) {
            failures |= FAIL_SHUTDOWN_CANCEL;
        }
        // The worker has announced that it is about to read. This narrows the
        // race but cannot prove it has entered nsysnet's select yet.
        if client.shutdown(Shutdown::Read).is_err()
            || !matches!(rx.recv_timeout(Duration::from_millis(500)), Ok(Ok(0)))
        {
            failures |= FAIL_SHUTDOWN_CANCEL;
        }
    }
    let _ = server.shutdown(Shutdown::Both);

    // UDP shares Cafe std's timeout emulation. Keep the operations local and
    // bounded so a broken timeout cannot stall the RPX.
    if let Err(_) = probe_udp(&mut failures, short) {
        failures |= FAIL_UDP_TIMEOUT | FAIL_UDP_PEEK | FAIL_UDP_ZERO_DATAGRAM;
    }
    failures
}

fn probe_udp(failures: &mut u32, short: Duration) -> io::Result<()> {
    let receiver = UdpSocket::bind(("127.0.0.1", 0))?;
    let sender = UdpSocket::bind(("127.0.0.1", 0))?;
    let destination = receiver.local_addr()?;
    receiver.set_read_timeout(Some(short))?;

    let worker_socket = receiver.try_clone()?;
    let (tx, rx) = mpsc::channel();
    let started = Instant::now();
    thread::spawn(move || {
        let mut data = [0u8; 8];
        let _ = tx.send(worker_socket.recv_from(&mut data));
    });
    if !matches!(rx.recv_timeout(PROBE_WAIT), Ok(Err(ref e)) if e.kind() == io::ErrorKind::TimedOut)
        || started.elapsed() < short
        || started.elapsed() > PROBE_WAIT
    {
        *failures |= FAIL_UDP_TIMEOUT;
    }

    if !matches!(
        worker_send_to(sender.try_clone()?, b"peek", destination),
        Some(Ok(4))
    ) {
        *failures |= FAIL_UDP_PEEK;
    }
    match worker_udp_receive(receiver.try_clone()?, true) {
        Some((Ok((4, _)), data)) if &data[..4] == b"peek" => {
            if !matches!(worker_udp_receive(receiver.try_clone()?, false), Some((Ok((4, _)), data)) if &data[..4] == b"peek")
            {
                *failures |= FAIL_UDP_PEEK;
            }
        }
        _ => *failures |= FAIL_UDP_PEEK,
    }

    if !matches!(
        worker_send_to(sender.try_clone()?, b"", destination),
        Some(Ok(0))
    ) {
        *failures |= FAIL_UDP_ZERO_DATAGRAM;
    }
    match worker_udp_receive(receiver.try_clone()?, true) {
        Some((Ok((0, _)), _)) => match worker_udp_receive(receiver.try_clone()?, false) {
            Some((Ok((0, _)), _)) => {
                if !matches!(
                    worker_send_to(sender.try_clone()?, b"n", destination),
                    Some(Ok(1))
                ) {
                    *failures |= FAIL_UDP_ZERO_DATAGRAM;
                }
                if !matches!(worker_udp_receive(receiver.try_clone()?, false), Some((Ok((1, _)), data)) if data[0] == b'n')
                {
                    *failures |= FAIL_UDP_ZERO_DATAGRAM;
                }
            }
            _ => *failures |= FAIL_UDP_ZERO_DATAGRAM,
        },
        _ => *failures |= FAIL_UDP_ZERO_DATAGRAM,
    }
    Ok(())
}

fn worker_udp_receive(
    socket: io::Result<UdpSocket>,
    peek: bool,
) -> Option<(io::Result<(usize, std::net::SocketAddr)>, [u8; 8])> {
    let socket = socket.ok()?;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut data = [0u8; 8];
        let result = if peek {
            socket.peek_from(&mut data)
        } else {
            socket.recv_from(&mut data)
        };
        let _ = tx.send((result, data));
    });
    rx.recv_timeout(PROBE_WAIT).ok()
}

fn worker_write_all(stream: TcpStream, data: &'static [u8]) -> Option<io::Result<()>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut stream = stream;
        let _ = tx.send(stream.write_all(data));
    });
    rx.recv_timeout(PROBE_WAIT).ok()
}

fn worker_send_to(
    socket: UdpSocket,
    data: &'static [u8],
    destination: std::net::SocketAddr,
) -> Option<io::Result<usize>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(socket.send_to(data, destination));
    });
    rx.recv_timeout(PROBE_WAIT).ok()
}
