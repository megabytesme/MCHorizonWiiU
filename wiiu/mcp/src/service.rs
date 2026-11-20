use crate::oauth::{self, OAuth, Persistence};
use crate::router::{Backend, Router};
use crate::tls::{Identity, Listener};
use horizon_wiiu_security::{
    Protection, Secret, SecretStore, StorageMode, StorageStatus, Vault, VaultError,
};
use serde::{Deserialize, Serialize};
use std::io;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

const IDENTITY: &str = "https_identity";
const AUTHORIZATION: &str = "oauth_trust";
const POLICY: &str = "agent_policy";

#[derive(Clone)]
pub struct SharedVault(Arc<Mutex<Vault>>);

impl SharedVault {
    fn with<R>(&self, use_vault: impl FnOnce(&mut Vault) -> R) -> R {
        let mut vault = self.0.lock().unwrap_or_else(|error| error.into_inner());
        use_vault(&mut vault)
    }
}

impl Persistence for SharedVault {
    fn load(&self) -> io::Result<Option<Zeroizing<Vec<u8>>>> {
        self.with(|vault| vault.get(AUTHORIZATION))
            .map(|value| value.map(|secret| Zeroizing::new(secret.expose().to_vec())))
            .map_err(io::Error::other)
    }

    fn save(&mut self, data: &[u8]) -> io::Result<()> {
        self.with(|vault| vault.put(AUTHORIZATION, Secret::new(data.to_vec())))
            .map_err(io::Error::other)
    }
}

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub enabled: bool,
    pub allow_launch: bool,
    #[serde(default)]
    pub allow_game: bool,
    pub no_ai: bool,
}

struct Running<T> {
    listener: Listener,
    router: Arc<Router<SharedVault, T>>,
    fingerprint: String,
}

pub struct Service<T> {
    root: PathBuf,
    vault: SharedVault,
    backend: T,
    running: Option<Running<T>>,
}

impl<T: Backend + Clone + Send + Sync + 'static> Service<T> {
    pub fn open(root: &Path, backend: T) -> Result<Self, VaultError> {
        let vault = Vault::open(&root.join("private"), "mcp")?;
        Ok(Self {
            root: root.to_owned(),
            vault: SharedVault(Arc::new(Mutex::new(vault))),
            backend,
            running: None,
        })
    }

    pub fn status(&self) -> StorageStatus {
        self.vault.with(|vault| vault.status())
    }

    pub fn unlock(&mut self, pin: &[u8]) -> Result<(), VaultError> {
        self.vault.with(|vault| vault.unlock(pin))
    }

    pub fn protect(&mut self, protection: Protection<'_>) -> Result<(), VaultError> {
        self.vault.with(|vault| vault.set_protection(protection))
    }

    pub fn policy(&self) -> Result<Policy, VaultError> {
        match self.vault.with(|vault| vault.get(POLICY))? {
            Some(value) => serde_json::from_slice(value.expose()).map_err(|_| VaultError::Corrupt),
            None => Ok(Policy::default()),
        }
    }

    pub fn set_policy(&mut self, policy: Policy) -> Result<(), VaultError> {
        // Enforce disabling immediately even if the SD write fails.
        let revoked = if policy.no_ai || !policy.enabled {
            self.running.as_ref().map_or(Ok(()), |running| {
                running.router.local(|oauth, _| {
                    oauth.set_disabled(true);
                    oauth.revoke_access_tokens()
                })
            })
        } else {
            Ok(())
        };
        let bytes = serde_json::to_vec(&policy).map_err(|_| VaultError::Corrupt)?;
        self.vault
            .with(|vault| vault.put(POLICY, Secret::new(bytes)))?;
        revoked.map_err(|_| VaultError::StorageUnavailable)?;
        if let Some(running) = &self.running {
            running
                .router
                .local(|oauth, _| oauth.set_disabled(policy.no_ai || !policy.enabled));
        }
        Ok(())
    }

    pub fn connect(
        &mut self,
        address: Ipv4Addr,
        port: u16,
        hostname: Option<&str>,
        fixture: bool,
    ) -> io::Result<()> {
        if self.connection().is_some() {
            return Ok(());
        }
        self.shutdown();
        let status = self.status();
        if !status.initialized || status.locked || status.mode == StorageMode::SessionOnly {
            return Ok(());
        }
        let policy = self.policy().map_err(io::Error::other)?;
        if !policy.enabled || policy.no_ai {
            return Ok(());
        }
        if port == 0 || !crate::tls::allowed_bind(address, fixture) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Agent Control needs a private IPv4 address and a fixed port",
            ));
        }
        if hostname.is_some_and(|host| !safe_hostname(host)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Use an ASCII LAN DNS name under .home.arpa or .local",
            ));
        }
        let host = hostname
            .map(str::to_owned)
            .unwrap_or_else(|| address.to_string());
        let issuer = if port == 443 {
            format!("https://{host}")
        } else {
            format!("https://{host}:{port}")
        };
        let identity = self.identity(address, hostname, &host)?;
        let auth = OAuth::open(self.vault.clone(), format!("{issuer}/mcp")).map_err(|error| {
            let message = match error {
                oauth::Error::Storage => {
                    "The OAuth records belong to a different endpoint or could not be read"
                }
                _ => "OAuth initialization failed",
            };
            io::Error::other(message)
        })?;
        let router = Arc::new(
            Router::new(issuer, auth, self.backend.clone())
                .map_err(|_| io::Error::other("Invalid OAuth endpoint"))?,
        );
        let net = router.clone();
        let pem = identity.public_root_pem();
        let directory = self.root.join("launcher");
        std::fs::create_dir_all(&directory)?;
        std::fs::write(directory.join("agent-control-ca.pem"), pem)?;
        let listener = Listener::start(
            SocketAddrV4::new(address, port),
            fixture,
            identity.server_config().map_err(io::Error::other)?,
            move |request, _peer, disconnected| net.handle(request, disconnected),
        )?;
        self.running = Some(Running {
            listener,
            router,
            fingerprint: identity.fingerprint(),
        });
        Ok(())
    }

    fn identity(
        &self,
        address: Ipv4Addr,
        hostname: Option<&str>,
        requested: &str,
    ) -> io::Result<Identity> {
        match self
            .vault
            .with(|vault| vault.get(IDENTITY))
            .map_err(io::Error::other)?
        {
            Some(secret) => {
                let identity: Identity =
                    serde_json::from_slice(secret.expose()).map_err(io::Error::other)?;
                if !identity.supports_name(requested) {
                    return Err(io::Error::other("The address changed. Rotate the HTTPS identity locally, or restore the original address."));
                }
                Ok(identity)
            }
            None => {
                let identity = Identity::generate(address, hostname).map_err(io::Error::other)?;
                let bytes = serde_json::to_vec(&identity).map_err(io::Error::other)?;
                self.vault
                    .with(|vault| vault.put(IDENTITY, Secret::new(bytes)))
                    .map_err(io::Error::other)?;
                Ok(identity)
            }
        }
    }

    pub fn shutdown(&mut self) {
        if let Some(running) = self.running.take() {
            running.router.local(|oauth, _| oauth.set_disabled(true));
            drop(running);
        }
    }

    pub fn rotate(&mut self) -> Result<(), VaultError> {
        self.shutdown();
        self.vault.with(|vault| {
            vault.remove(AUTHORIZATION)?;
            vault.remove(IDENTITY)
        })
    }

    pub fn connection(&self) -> Option<(&str, &str)> {
        let running = self
            .running
            .as_ref()
            .filter(|running| running.listener.is_running())?;
        Some((running.router.issuer(), &running.fingerprint))
    }

    pub fn local<R>(&self, action: impl FnOnce(&mut OAuth<SharedVault>, u64) -> R) -> Option<R> {
        self.running
            .as_ref()
            .map(|running| running.router.local(action))
    }
}

fn safe_hostname(host: &str) -> bool {
    if host.len() > 253 || !(host.ends_with(".home.arpa") || host.ends_with(".local")) {
        return false;
    }
    host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oauth::{Principal, Scopes};
    use crate::protocol::{text_result, Fault};
    use serde_json::Value;
    use std::net::TcpListener;

    #[derive(Clone)]
    struct TestBackend;

    impl Backend for TestBackend {
        fn list(&self, _: &Principal) -> Vec<Value> {
            Vec::new()
        }
        fn required(&self, _: &str) -> Option<Scopes> {
            None
        }
        fn call(
            &self,
            _: &Principal,
            _: &str,
            _: &Value,
            _: &dyn Fn() -> bool,
        ) -> Result<Value, Fault> {
            Ok(text_result("test", false))
        }
    }

    #[test]
    fn cold_restart_requires_pin_or_automatically_restores_https_identity() {
        let root = std::env::temp_dir().join(format!(
            "horizon-mcp-storage-{}",
            u64::from_be_bytes(crate::oauth::random::<8>().unwrap())
        ));
        std::fs::create_dir(&root).unwrap();
        let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let mut service = Service::open(&root, TestBackend).unwrap();
        service
            .connect(Ipv4Addr::LOCALHOST, port, None, true)
            .unwrap();
        assert!(service.connection().is_none());
        service.protect(Protection::ConfirmedUnprotected).unwrap();
        service
            .set_policy(Policy {
                enabled: true,
                ..Policy::default()
            })
            .unwrap();
        service
            .connect(Ipv4Addr::LOCALHOST, port, None, true)
            .unwrap();
        let first = service.connection().unwrap().1.to_owned();
        drop(service);

        let mut service = Service::open(&root, TestBackend).unwrap();
        assert_eq!(service.status().mode, StorageMode::Unprotected);
        service
            .connect(Ipv4Addr::LOCALHOST, port, None, true)
            .unwrap();
        assert_eq!(service.connection().unwrap().1, first);
        service
            .protect(Protection::PinProtected {
                new_pin: b"fixture-246810",
            })
            .unwrap();
        drop(service);

        let mut service = Service::open(&root, TestBackend).unwrap();
        assert!(service.status().locked);
        service
            .connect(Ipv4Addr::LOCALHOST, port, None, true)
            .unwrap();
        assert!(service.connection().is_none());
        assert!(service.unlock(b"fixture-invalid").is_err());
        service.unlock(b"fixture-246810").unwrap();
        service
            .connect(Ipv4Addr::LOCALHOST, port, None, true)
            .unwrap();
        assert_eq!(service.connection().unwrap().1, first);
        drop(service);
        std::fs::remove_dir_all(root).unwrap();
    }
}
