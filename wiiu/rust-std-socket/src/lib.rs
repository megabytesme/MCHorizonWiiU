//! Cemu-only behavioral probe for Cafe `TcpStream::try_clone` ownership.
//!
//! The probe uses a loopback TCP listener in the same process. Every socket is
//! nonblocking except the bounded `connect_timeout` operation, and polling
//! loops have finite deadlines so a regression cannot hang the RPX.

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::time::{Duration, Instant};

const DEADLINE: Duration = Duration::from_secs(5);
const MAX_PROBE_PAYLOAD: usize = 16;
const _: () = assert!(b"clone-write".len() <= MAX_PROBE_PAYLOAD);
const _: () = assert!(b"reply".len() <= MAX_PROBE_PAYLOAD);

const FAIL_BIND: u32 = 1 << 0;
const FAIL_LISTENER_NONBLOCKING: u32 = 1 << 1;
const FAIL_CONNECT: u32 = 1 << 2;
const FAIL_ACCEPT: u32 = 1 << 3;
const FAIL_CLONE: u32 = 1 << 4;
const FAIL_STREAM_NONBLOCKING: u32 = 1 << 5;
const FAIL_EMPTY_READ: u32 = 1 << 6;
const FAIL_CLIENT_WRITE: u32 = 1 << 7;
const FAIL_SERVER_READ: u32 = 1 << 8;
const FAIL_SERVER_WRITE: u32 = 1 << 9;
const FAIL_CLIENT_READ: u32 = 1 << 10;
const FAIL_LAST_CLOSE_EOF: u32 = 1 << 11;

fn expired(deadline: Instant) -> bool {
    Instant::now() >= deadline
}

fn accept_bounded(listener: &TcpListener) -> io::Result<TcpStream> {
    let deadline = Instant::now() + DEADLINE;
    loop {
        match listener.accept() {
            Ok((stream, _)) => return Ok(stream),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if expired(deadline) {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "accept deadline"));
                }
                std::hint::spin_loop();
            }
            Err(error) => return Err(error),
        }
    }
}

fn write_all_bounded(stream: &mut TcpStream, bytes: &[u8]) -> io::Result<()> {
    let deadline = Instant::now() + DEADLINE;
    let mut offset = 0;
    while offset < bytes.len() {
        match stream.write(&bytes[offset..]) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "socket write returned zero",
                ))
            }
            Ok(written) => offset += written,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if expired(deadline) {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "write deadline"));
                }
                std::hint::spin_loop();
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn read_exact_bounded(stream: &mut TcpStream, expected: &[u8]) -> io::Result<()> {
    let deadline = Instant::now() + DEADLINE;
    let mut actual = [0u8; MAX_PROBE_PAYLOAD];
    if expected.len() > actual.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "probe payload too large",
        ));
    }
    let mut offset = 0;
    while offset < expected.len() {
        match stream.read(&mut actual[offset..expected.len()]) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "peer closed before payload",
                ))
            }
            Ok(read) => offset += read,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if expired(deadline) {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "read deadline"));
                }
                std::hint::spin_loop();
            }
            Err(error) => return Err(error),
        }
    }
    if &actual[..expected.len()] != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "TCP payload mismatch",
        ));
    }
    Ok(())
}

fn expect_would_block(stream: &mut TcpStream) -> io::Result<()> {
    let mut byte = [0u8; 1];
    match stream.read(&mut byte) {
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(()),
        Err(error) => Err(error),
        Ok(0) => Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "peer closed before data",
        )),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unexpected data before send",
        )),
    }
}

fn expect_eof_bounded(stream: &mut TcpStream) -> io::Result<()> {
    let deadline = Instant::now() + DEADLINE;
    let mut byte = [0u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => return Ok(()),
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unexpected data before EOF",
                ))
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if expired(deadline) {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "EOF deadline"));
                }
                std::hint::spin_loop();
            }
            Err(error) => return Err(error),
        }
    }
}

/// Returns zero only if clone sharing, original-drop survival, nonblocking
/// behavior, bidirectional data, and final-close EOF all pass on Cafe std.
#[no_mangle]
pub extern "C" fn horizon_socket_clone_probe() -> u32 {
    let listener = match TcpListener::bind(("127.0.0.1", 0)) {
        Ok(listener) => listener,
        Err(_) => return FAIL_BIND,
    };
    if listener.set_nonblocking(true).is_err() {
        return FAIL_LISTENER_NONBLOCKING;
    }

    let address = match listener.local_addr() {
        Ok(address) => address,
        Err(_) => return FAIL_BIND,
    };
    let client = match TcpStream::connect_timeout(&address, DEADLINE) {
        Ok(stream) => stream,
        Err(_) => return FAIL_CONNECT,
    };
    let mut server = match accept_bounded(&listener) {
        Ok(stream) => stream,
        Err(_) => return FAIL_ACCEPT,
    };
    if server.set_nonblocking(true).is_err() {
        return FAIL_STREAM_NONBLOCKING;
    }

    let mut clone = match client.try_clone() {
        Ok(clone) => clone,
        Err(_) => return FAIL_CLONE,
    };
    // Change the shared socket option through one handle, then use the clone.
    if client.set_nonblocking(true).is_err() {
        return FAIL_STREAM_NONBLOCKING;
    }
    drop(client);

    if expect_would_block(&mut clone).is_err() {
        return FAIL_EMPTY_READ;
    }
    if write_all_bounded(&mut clone, b"clone-write").is_err() {
        return FAIL_CLIENT_WRITE;
    }
    if read_exact_bounded(&mut server, b"clone-write").is_err() {
        return FAIL_SERVER_READ;
    }
    if write_all_bounded(&mut server, b"reply").is_err() {
        return FAIL_SERVER_WRITE;
    }
    if read_exact_bounded(&mut clone, b"reply").is_err() {
        return FAIL_CLIENT_READ;
    }

    drop(clone);
    if expect_eof_bounded(&mut server).is_err() {
        return FAIL_LAST_CLOSE_EOF;
    }
    let _ = server.shutdown(Shutdown::Both);
    0
}
