use crate::http::{self, ReadError, Request, Response};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use rcgen::{
    BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose, PKCS_ECDSA_P256_SHA256,
};
use ring::digest::{digest, SHA256};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use zeroize::Zeroize;

const WORKERS: usize = 4;
const READ_BUDGET: Duration = Duration::from_secs(5);
const WRITE_BUDGET: Duration = Duration::from_secs(10);

/// Serialized as one secret-store entry so a restart never mixes two
/// generations of certificates and private key.
#[derive(Serialize, Deserialize)]
pub struct Identity {
    names: Vec<String>,
    root_der: Vec<u8>,
    leaf_der: Vec<u8>,
    private_key: Vec<u8>,
}

impl Drop for Identity {
    fn drop(&mut self) {
        self.private_key.zeroize();
    }
}

impl Identity {
    pub fn generate(address: Ipv4Addr, hostname: Option<&str>) -> Result<Self, rcgen::Error> {
        let mut names = vec![address.to_string()];
        if let Some(hostname) = hostname {
            names.push(hostname.to_owned());
        }
        let mut root = CertificateParams::default();
        root.distinguished_name
            .push(DnType::CommonName, "Horizon Wii U trust root");
        root.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
        root.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        let issuer_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
        let root_cert = root.self_signed(&issuer_key)?;
        let issuer = Issuer::from_params(&root, issuer_key);
        let mut leaf = CertificateParams::new(names.clone())?;
        leaf.use_authority_key_identifier_extension = true;
        leaf.distinguished_name
            .push(DnType::CommonName, "Horizon Wii U Agent Control");
        leaf.key_usages.push(KeyUsagePurpose::DigitalSignature);
        leaf.extended_key_usages
            .push(ExtendedKeyUsagePurpose::ServerAuth);
        let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
        let cert = leaf.signed_by(&key, &issuer)?;
        Ok(Self {
            names,
            root_der: root_cert.der().to_vec(),
            leaf_der: cert.der().to_vec(),
            private_key: key.serialize_der(),
        })
    }

    pub fn supports_name(&self, name: &str) -> bool {
        self.names.iter().any(|n| n.eq_ignore_ascii_case(name))
    }

    /// This is the fingerprint of the public root the player installs.
    pub fn fingerprint(&self) -> String {
        digest(&SHA256, &self.root_der)
            .as_ref()
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(":")
    }

    pub fn public_root_pem(&self) -> String {
        let mut pem = String::from("-----BEGIN CERTIFICATE-----\n");
        let encoded = STANDARD.encode(&self.root_der);
        for chunk in encoded.as_bytes().chunks(64) {
            pem.push_str(std::str::from_utf8(chunk).expect("base64 ASCII"));
            pem.push('\n');
        }
        pem.push_str("-----END CERTIFICATE-----\n");
        pem
    }

    pub fn server_config(&self) -> Result<ServerConfig, rustls::Error> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut config = ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()?
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(self.leaf_der.clone())],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.private_key.clone())),
            )?;
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        Ok(config)
    }
}

/// Only the caller's verified LAN interface may be selected. Loopback is
/// separate for host fixtures. The service never binds a wildcard listener.
pub fn allowed_bind(address: Ipv4Addr, fixture: bool) -> bool {
    address.is_private() || (fixture && address.is_loopback())
}

pub struct Listener {
    address: SocketAddr,
    shutdown: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl Listener {
    pub fn start(
        address: SocketAddrV4,
        fixture: bool,
        config: ServerConfig,
        handler: impl Fn(Request, SocketAddr, &dyn Fn() -> bool) -> Response + Send + Sync + 'static,
    ) -> io::Result<Self> {
        if !allowed_bind(*address.ip(), fixture) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "MCP requires a private IPv4 interface",
            ));
        }
        let listener = TcpListener::bind(address)?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let shutdown = Arc::new(AtomicBool::new(false));
        let mut service = Self {
            address,
            shutdown,
            threads: Vec::with_capacity(WORKERS + 1),
        };
        let (tx, rx) = mpsc::sync_channel::<(TcpStream, SocketAddr, Instant)>(WORKERS);
        let rx = Arc::new(Mutex::new(rx));
        let config = Arc::new(config);
        let handler = Arc::new(handler);
        for _ in 0..WORKERS {
            let jobs = Arc::clone(&rx);
            let config = Arc::clone(&config);
            let handler = Arc::clone(&handler);
            let shutdown = Arc::clone(&service.shutdown);
            service
                .threads
                .push(
                    thread::Builder::new()
                        .name("mcp-http".into())
                        .spawn(move || {
                            while !shutdown.load(Ordering::Acquire) {
                                let job = jobs
                                    .lock()
                                    .unwrap_or_else(|e| e.into_inner())
                                    .recv_timeout(Duration::from_millis(100));
                                match job {
                                    Ok((socket, peer, deadline))
                                        if !shutdown.load(Ordering::Acquire) =>
                                    {
                                        let _ = serve(
                                            socket,
                                            config.clone(),
                                            peer,
                                            deadline,
                                            &*handler,
                                        );
                                    }
                                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                                    _ => {}
                                }
                            }
                        })?,
                );
        }
        let shutdown = Arc::clone(&service.shutdown);
        service.threads.push(thread::Builder::new().name("mcp-listener".into()).spawn(move || {
            while !shutdown.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((socket, peer)) if matches!(peer.ip(), std::net::IpAddr::V4(ip) if allowed_bind(ip, fixture)) => {
                        let _ = tx.try_send((socket, peer, Instant::now() + READ_BUDGET));
                    }
                    Ok(_) => {},
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(25)),
                    Err(_) => break,
                }
            }
            shutdown.store(true, Ordering::Release);
        })?);
        Ok(service)
    }

    pub fn is_running(&self) -> bool {
        !self.shutdown.load(Ordering::Acquire)
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

struct Deadline {
    socket: TcpStream,
    end: Instant,
}

impl Deadline {
    fn remaining(&self) -> io::Result<Duration> {
        self.end
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| io::Error::from(io::ErrorKind::TimedOut))
    }
}

impl Deadline {
    // Cafe nsysnet does not implement a send timeout. Keep the stream
    // non-blocking and enforce an absolute budget in both directions.
    fn io<T>(&self, mut attempt: impl FnMut(&TcpStream) -> io::Result<T>) -> io::Result<T> {
        loop {
            let remaining = self.remaining()?;
            match attempt(&self.socket) {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(remaining.min(Duration::from_millis(5)));
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                result => return result,
            }
        }
    }
}

impl Read for Deadline {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.io(|mut socket| socket.read(buf))
    }
}

impl Write for Deadline {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.io(|mut socket| socket.write(buf))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.socket.flush()
    }
}

type RequestHandler<'a> = dyn Fn(Request, SocketAddr, &dyn Fn() -> bool) -> Response + 'a;

fn serve(
    socket: TcpStream,
    config: Arc<ServerConfig>,
    peer: SocketAddr,
    deadline: Instant,
    handler: &RequestHandler<'_>,
) -> io::Result<()> {
    socket.set_nonblocking(true)?;
    let connection = ServerConnection::new(config).map_err(io::Error::other)?;
    let mut stream = StreamOwned::new(
        connection,
        Deadline {
            socket,
            end: deadline,
        },
    );
    let response = match http::read(&mut stream) {
        Ok(request) => handler(request, peer, &|| {
            let socket = &stream.sock.socket;
            let mut data = [0];
            match socket.peek(&mut data) {
                Ok(_) => true,
                Err(error) => matches!(
                    error.kind(),
                    io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted
                ),
            }
        }),
        Err(ReadError::Http(status)) => Response::empty(status),
        Err(ReadError::Io(error)) => return Err(error),
    };
    stream.sock.end = Instant::now() + WRITE_BUDGET;
    http::write(&mut stream, response)?;
    stream.conn.send_close_notify();
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustls::pki_types::ServerName;
    use rustls::{ClientConfig, ClientConnection, RootCertStore};

    const HEALTH_REQUEST: &[u8] = b"GET /health HTTP/1.1\r\nHost: loopback\r\n\r\n";

    fn client(
        identity: &Identity,
        address: SocketAddr,
        name: &str,
        request: &[u8],
    ) -> io::Result<String> {
        let mut root = RootCertStore::empty();
        root.add(CertificateDer::from(identity.root_der.clone()))
            .unwrap();
        let config =
            ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_root_certificates(root)
                .with_no_client_auth();
        let name = ServerName::try_from(name.to_owned()).unwrap();
        let conn = ClientConnection::new(Arc::new(config), name).unwrap();
        let mut stream = StreamOwned::new(conn, TcpStream::connect(address)?);
        stream.sock.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.write_all(request)?;
        let mut response = String::new();
        stream.read_to_string(&mut response)?;
        Ok(response)
    }

    #[test]
    fn genuine_tls_persisted_identity_validates_ip_and_dns() {
        let identity = Identity::generate(Ipv4Addr::LOCALHOST, Some("horizon.home.arpa")).unwrap();
        let saved = serde_json::to_vec(&identity).unwrap();
        let restored: Identity = serde_json::from_slice(&saved).unwrap();
        assert_eq!(restored.fingerprint(), identity.fingerprint());
        let listener = Listener::start(
            SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0),
            true,
            restored.server_config().unwrap(),
            |req, _, _| Response::bytes(200, "text/plain", req.target.into_bytes()),
        )
        .unwrap();
        let address = listener.address();
        for name in ["127.0.0.1", "horizon.home.arpa"] {
            assert!(client(&identity, address, name, HEALTH_REQUEST)
                .unwrap()
                .ends_with("/health"));
        }
        assert!(client(&identity, address, "unrelated.home.arpa", HEALTH_REQUEST).is_err());
        let unrelated = Identity::generate(Ipv4Addr::LOCALHOST, None).unwrap();
        assert!(client(&unrelated, address, "127.0.0.1", HEALTH_REQUEST).is_err());
    }

    #[test]
    fn listener_recovers_from_interrupted_tls_and_malformed_http() {
        let identity = Identity::generate(Ipv4Addr::LOCALHOST, None).unwrap();
        let listener = Listener::start(
            SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0),
            true,
            identity.server_config().unwrap(),
            |_, _, _| Response::bytes(200, "text/plain", b"ready".to_vec()),
        )
        .unwrap();
        let address = listener.address();
        let mut broken = TcpStream::connect(address).unwrap();
        broken.write_all(&[0x16, 0x03, 0x03]).unwrap();
        drop(broken);

        let malformed = b"POST /mcp HTTP/1.1\r\nHost: loopback\r\nContent-Length: 999999\r\n\r\n";
        assert!(client(&identity, address, "127.0.0.1", malformed)
            .unwrap()
            .starts_with("HTTP/1.1 413 "));
        assert!(client(&identity, address, "127.0.0.1", HEALTH_REQUEST)
            .unwrap()
            .ends_with("ready"));
    }

    #[test]
    fn refuses_wildcard_and_external_binding() {
        for ip in [
            Ipv4Addr::UNSPECIFIED,
            Ipv4Addr::new(8, 8, 8, 8),
            Ipv4Addr::LOCALHOST,
        ] {
            assert!(!allowed_bind(ip, false));
        }
        assert!(allowed_bind(Ipv4Addr::new(192, 168, 1, 25), false));
    }
}
