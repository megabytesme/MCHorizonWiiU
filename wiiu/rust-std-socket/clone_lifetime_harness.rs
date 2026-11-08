//! Proposed bounded behavioral tests for the Cafe std `Socket::duplicate` implementation.
//! Adapt into `library/std/src/net/tcp/tests.rs` and `udp/tests.rs` in a Cafe
//! rust-wiiu checkout. Source only; these tests have not been built or run.

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream, UdpSocket};
use std::thread;
use std::time::{Duration, Instant};

const DEADLINE: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(1);

fn until<T>(mut operation: impl FnMut() -> io::Result<Option<T>>) -> T {
    let deadline = Instant::now() + DEADLINE;
    loop {
        if let Some(value) = operation().unwrap() {
            return value;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for socket activity"
        );
        thread::sleep(POLL);
    }
}

fn accept_bounded(listener: &TcpListener) -> (TcpStream, std::net::SocketAddr) {
    listener.set_nonblocking(true).unwrap();
    until(|| match listener.accept() {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(None),
        Err(error) => Err(error),
    })
}

fn read_exact_bounded(stream: &mut TcpStream, expected: &mut [u8]) {
    stream.set_nonblocking(true).unwrap();
    let mut offset = 0;
    until(|| {
        if offset == expected.len() {
            return Ok(Some(()));
        }
        match stream.read(&mut expected[offset..]) {
            Ok(0) => Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "peer closed early",
            )),
            Ok(n) => {
                offset += n;
                Ok((offset == expected.len()).then_some(()))
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(error),
        }
    });
}

fn read_eof_bounded(stream: &mut TcpStream) {
    stream.set_nonblocking(true).unwrap();
    let mut byte = [0];
    until(|| match stream.read(&mut byte) {
        Ok(0) => Ok(Some(())),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unexpected data before EOF",
        )),
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(None),
        Err(error) => Err(error),
    });
}

fn recv_from_bounded(socket: &UdpSocket, byte: &mut [u8]) -> (usize, std::net::SocketAddr) {
    socket.set_nonblocking(true).unwrap();
    until(|| match socket.recv_from(byte) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(None),
        Err(error) => Err(error),
    })
}

#[test]
fn tcp_clone_keeps_stream_alive_after_original_drop_and_shares_shutdown() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let addr = listener.local_addr().unwrap();
    let peer = thread::spawn(move || {
        let (mut stream, _) = accept_bounded(&listener);
        let mut byte = [0];
        read_exact_bounded(&mut stream, &mut byte);
        assert_eq!(byte, [0x42]);
        stream.write_all(&[0x24]).unwrap();
        read_eof_bounded(&mut stream);
    });

    let original = TcpStream::connect_timeout(&addr, DEADLINE).unwrap();
    let clone = original.try_clone().unwrap();
    drop(original);

    let mut clone = clone;
    clone.write_all(&[0x42]).unwrap();
    let mut response = [0];
    read_exact_bounded(&mut clone, &mut response);
    assert_eq!(response, [0x24]);
    clone.shutdown(Shutdown::Write).unwrap();
    peer.join().unwrap();
}

#[test]
fn final_stream_clone_drop_closes_peer_connection() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let addr = listener.local_addr().unwrap();
    let peer = thread::spawn(move || {
        let (mut stream, _) = accept_bounded(&listener);
        read_eof_bounded(&mut stream);
    });

    let original = TcpStream::connect_timeout(&addr, DEADLINE).unwrap();
    let clone = original.try_clone().unwrap();
    drop(original);
    drop(clone);
    peer.join().unwrap();
}

#[test]
fn cloned_listener_accepts_after_original_drop() {
    let original = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let addr = original.local_addr().unwrap();
    let clone = original.try_clone().unwrap();
    drop(original);

    let client = thread::spawn(move || TcpStream::connect_timeout(&addr, DEADLINE).unwrap());
    let (mut accepted, _) = accept_bounded(&clone);
    let mut client = client.join().unwrap();
    accepted.write_all(&[0x6a]).unwrap();
    let mut byte = [0];
    read_exact_bounded(&mut client, &mut byte);
    assert_eq!(byte, [0x6a]);
}

#[test]
fn cloned_udp_socket_receives_after_original_drop() {
    let original = UdpSocket::bind(("127.0.0.1", 0)).unwrap();
    let addr = original.local_addr().unwrap();
    let clone = original.try_clone().unwrap();
    drop(original);

    let sender = UdpSocket::bind(("127.0.0.1", 0)).unwrap();
    assert_eq!(sender.send_to(&[0x39], addr).unwrap(), 1);
    let mut byte = [0];
    let (len, _) = recv_from_bounded(&clone, &mut byte);
    assert_eq!(len, 1);
    assert_eq!(byte, [0x39]);
}

#[test]
fn clone_observes_shared_nonblocking_option() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let stream = TcpStream::connect_timeout(&listener.local_addr().unwrap(), DEADLINE).unwrap();
    let clone = stream.try_clone().unwrap();
    stream.set_nonblocking(true).unwrap();
    let mut byte = [0];
    let error = (&clone).read(&mut byte).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    clone.set_nonblocking(false).unwrap();
}
