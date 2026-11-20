//! OAuth state is serialized by the app owner. Save a complete snapshot
//! before issuing a token that depends on a persistent state transition.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ring::{
    digest, hmac,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{SystemTime, UNIX_EPOCH};
use subtle::ConstantTimeEq;
use url::{Host, Url};
use zeroize::{Zeroize, Zeroizing};

const MAX_CLIENTS: usize = 32;
const MAX_PENDING: usize = 4;
const ACCESS_PER_CLIENT: usize = 2;
const REGISTRATION_LIFE: u64 = 10 * 60;
const CONSENT_LIFE: u64 = 3 * 60;
const CODE_LIFE: u64 = 90;
pub const ACCESS_LIFE: u64 = 15 * 60;
const REFRESH_LIFE: u64 = 90 * 86400;
const RECORD_LIMIT: usize = 64 * 1024;
const RECORD_VERSION: u8 = 1;
type Verifier = [u8; 32];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Scopes(u8);

impl Scopes {
    pub const LAUNCHER_READ: Self = Self(1);
    pub const LAUNCHER_MANAGE: Self = Self(2);
    pub const GAME_OBSERVE: Self = Self(4);
    pub const GAME_CONTROL: Self = Self(8);

    const NAMES: [(&'static str, Self); 4] = [
        ("launcher.read", Self::LAUNCHER_READ),
        ("launcher.manage", Self::LAUNCHER_MANAGE),
        ("game.observe", Self::GAME_OBSERVE),
        ("game.control", Self::GAME_CONTROL),
    ];

    pub fn names() -> impl Iterator<Item = &'static str> {
        Self::NAMES.into_iter().map(|(name, _)| name)
    }

    pub fn parse(value: &str) -> Result<Self, Error> {
        let mut bits = 0;
        for word in value.split_ascii_whitespace() {
            if word == "offline_access" {
                continue;
            }
            let (_, scope) = Self::NAMES
                .iter()
                .find(|(name, _)| *name == word)
                .ok_or(Error::InvalidScope)?;
            bits |= scope.0;
        }
        if bits == 0 {
            return Err(Error::InvalidScope);
        }
        Ok(Self(bits))
    }

    pub const fn contains(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub fn as_text(self) -> String {
        Self::NAMES
            .into_iter()
            .filter_map(|(name, scope)| self.contains(scope).then_some(name))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

pub trait Persistence {
    fn load(&self) -> io::Result<Option<Zeroizing<Vec<u8>>>>;
    fn save(&mut self, data: &[u8]) -> io::Result<()>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidRequest,
    InvalidClient,
    InvalidScope,
    InvalidGrant,
    InvalidTarget,
    Closed,
    Busy,
    Storage,
    Random,
}

#[derive(Clone, Serialize, Deserialize)]
struct Client {
    name: String,
    redirects: Vec<String>,
    registration_expires: u64,
    allows_refresh: bool,
    grant: Option<Grant>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Grant {
    scopes: Scopes,
    family: [u8; 16],
    generation: u64,
    current: Verifier,
    expires: u64,
    last_activity: u64,
    refresh_enabled: bool,
    #[serde(default)]
    access: Vec<SavedAccess>,
}

#[derive(Clone, Serialize, Deserialize)]
struct SavedAccess {
    verifier: Verifier,
    expires: u64,
    scopes: Scopes,
}

#[derive(Clone, Serialize, Deserialize)]
struct Database {
    version: u8,
    resource: String,
    refresh_signing_key: Verifier,
    clients: BTreeMap<String, Client>,
}

impl Drop for Database {
    fn drop(&mut self) {
        self.refresh_signing_key.zeroize();
    }
}

impl SavedAccess {
    fn access_for(&self, id: &str) -> Access {
        Access {
            principal: Principal {
                client_id: id.to_owned(),
                scopes: self.scopes,
                valid: Arc::new(AtomicBool::new(true)),
                expires: self.expires,
            },
            expires: self.expires,
        }
    }
}

struct Access {
    principal: Principal,
    expires: u64,
}

#[derive(Clone)]
pub struct Principal {
    pub client_id: String,
    pub scopes: Scopes,
    valid: Arc<AtomicBool>,
    expires: u64,
}

impl Principal {
    #[cfg(test)]
    pub(crate) fn fixture() -> Self {
        Self {
            client_id: "fixture".into(),
            scopes: Scopes::GAME_CONTROL,
            valid: Arc::new(AtomicBool::new(true)),
            expires: u64::MAX,
        }
    }
    pub fn is_active(&self) -> bool {
        self.valid.load(Ordering::Acquire)
            && SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .is_ok_and(|now| now.as_secs() < self.expires)
    }
}

impl Drop for Access {
    fn drop(&mut self) {
        self.principal.valid.store(false, Ordering::Release);
    }
}

#[derive(Clone)]
pub struct Consent {
    pub id: String,
    pub client_name: String,
    pub redirect_uri: String,
    pub scopes: Scopes,
}

#[derive(Clone)]
pub struct Authorization {
    client_id: String,
    redirect_uri: String,
    scopes: Scopes,
    pkce: Verifier,
    state: String,
}

pub struct AuthorizeRequest<'a> {
    pub client_id: &'a str,
    pub redirect_uri: &'a str,
    pub scope: Option<&'a str>,
    pub challenge: &'a str,
    pub state: &'a str,
    pub resource: Option<&'a str>,
}

struct Pending {
    request: Authorization,
    expires: u64,
    approved: Option<bool>,
}

struct Code {
    request: Authorization,
    expires: u64,
}

#[derive(Serialize)]
pub struct Tokens {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: u64,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
}

pub struct TrustedClient {
    pub client_id: String,
    pub name: String,
    pub scopes: Scopes,
    pub last_activity: u64,
}

pub struct OAuth<P> {
    storage: P,
    db: Database,
    pairing_until: u64,
    disabled: bool,
    pending: BTreeMap<String, Pending>,
    codes: BTreeMap<Verifier, Code>,
    access: BTreeMap<Verifier, Access>,
    activity: BTreeMap<String, u64>,
}

pub(crate) fn random<const N: usize>() -> Result<[u8; N], Error> {
    let mut bytes = [0u8; N];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| Error::Random)?;
    Ok(bytes)
}

fn hash(text: &str) -> Verifier {
    digest::digest(&digest::SHA256, text.as_bytes())
        .as_ref()
        .try_into()
        .unwrap()
}

fn decode<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.len() != (N * 8).div_ceil(6) {
        return None;
    }
    URL_SAFE_NO_PAD.decode(value).ok()?.try_into().ok()
}

fn verifier_shape(verifier: &str) -> bool {
    (43..=128).contains(&verifier.len())
        && verifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
}

/// The native desktop clients use loopback HTTP receivers. Match the
/// registered URI byte-for-byte, including callback path and port.
fn valid_redirect(value: &str) -> bool {
    if value.len() > 512
        || !value.starts_with("http://")
        || value.contains(['\\', '?', '#', '%'])
        || value.bytes().any(|b| !b.is_ascii_graphic())
    {
        return false;
    }
    let Ok(parsed) = Url::parse(value) else {
        return false;
    };
    let loopback = match parsed.host() {
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        Some(Host::Domain(host)) => host == "localhost",
        None => false,
    };
    loopback
        && parsed.port().is_some_and(|port| port > 0)
        && parsed.path().starts_with('/')
        && parsed.path() != "/"
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.as_str() == value
}

impl<P: Persistence> OAuth<P> {
    pub fn open(storage: P, resource: String) -> Result<Self, Error> {
        let db = match storage.load().map_err(|_| Error::Storage)? {
            Some(bytes) if bytes.len() <= RECORD_LIMIT => {
                serde_json::from_slice::<Database>(&bytes)
                    .ok()
                    .filter(|db| {
                        db.version == RECORD_VERSION
                            && db.resource == resource
                            && db.clients.len() <= MAX_CLIENTS
                    })
                    .ok_or(Error::Storage)?
            }
            Some(_) => return Err(Error::Storage),
            None => Database {
                version: RECORD_VERSION,
                resource,
                refresh_signing_key: random()?,
                clients: BTreeMap::new(),
            },
        };
        let access = db
            .clients
            .iter()
            .flat_map(|(id, client)| {
                client.grant.iter().flat_map(move |grant| {
                    grant
                        .access
                        .iter()
                        .take(ACCESS_PER_CLIENT)
                        .map(move |record| (record.verifier, record.access_for(id)))
                })
            })
            .collect();
        Ok(Self {
            storage,
            db,
            pairing_until: 0,
            disabled: false,
            pending: BTreeMap::new(),
            codes: BTreeMap::new(),
            access,
            activity: BTreeMap::new(),
        })
    }

    fn commit(&mut self, db: Database) -> Result<(), Error> {
        let data = Zeroizing::new(serde_json::to_vec(&db).map_err(|_| Error::Storage)?);
        if data.len() > RECORD_LIMIT {
            return Err(Error::Storage);
        }
        self.storage.save(&data).map_err(|_| Error::Storage)?;
        self.db = db;
        Ok(())
    }

    pub fn resource(&self) -> &str {
        &self.db.resource
    }

    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
        if disabled {
            self.access.clear();
            self.pending.clear();
            self.codes.clear();
            self.pairing_until = 0;
        }
    }

    /// Emergency disable revokes bearer verifiers in storage as well as in
    /// memory. Ordinary app shutdown leaves unexpired verifiers reusable.
    pub fn revoke_access_tokens(&mut self) -> Result<(), Error> {
        self.access.clear();
        let mut db = self.db.clone();
        let mut changed = false;
        for grant in db
            .clients
            .values_mut()
            .filter_map(|client| client.grant.as_mut())
        {
            changed |= !grant.access.is_empty();
            grant.access.clear();
        }
        if changed {
            self.commit(db)?;
        }
        Ok(())
    }

    pub fn open_pairing(&mut self, now: u64) {
        if !self.disabled {
            self.pairing_until = now.saturating_add(CONSENT_LIFE);
        }
    }

    fn pairing_open(&self, now: u64) -> bool {
        !self.disabled && now < self.pairing_until
    }

    pub fn register(
        &mut self,
        now: u64,
        name: &str,
        redirects: &[String],
        allows_refresh: bool,
    ) -> Result<String, Error> {
        if !self.pairing_open(now) {
            return Err(Error::Closed);
        }
        if name.is_empty()
            || name.chars().any(char::is_control)
            || name.len() > 96
            || redirects.is_empty()
            || redirects.len() > 4
            || !redirects.iter().all(|uri| valid_redirect(uri))
        {
            return Err(Error::InvalidRequest);
        }
        let mut db = self.db.clone();
        db.clients
            .retain(|_, client| client.grant.is_some() || client.registration_expires > now);
        // Reserve at least one slot for explicit local recovery if registrations
        // flood the current pairing window. Trusted grants are never evicted.
        if db.clients.len() >= MAX_CLIENTS
            || db
                .clients
                .values()
                .filter(|client| client.grant.is_none())
                .count()
                >= MAX_PENDING * 2
        {
            return Err(Error::Busy);
        }
        let id = URL_SAFE_NO_PAD.encode(random::<24>()?);
        db.clients.insert(
            id.clone(),
            Client {
                name: name.to_owned(),
                redirects: redirects.to_vec(),
                allows_refresh,
                grant: None,
                registration_expires: now.saturating_add(REGISTRATION_LIFE),
            },
        );
        self.commit(db)?;
        Ok(id)
    }

    pub fn authorization(
        &self,
        now: u64,
        request: AuthorizeRequest<'_>,
    ) -> Result<Authorization, Error> {
        if !self.pairing_open(now) {
            return Err(Error::Closed);
        }
        let client = self
            .db
            .clients
            .get(request.client_id)
            .ok_or(Error::InvalidClient)?;
        if client.grant.is_none() && client.registration_expires <= now {
            return Err(Error::InvalidClient);
        }
        if !client
            .redirects
            .iter()
            .any(|uri| uri == request.redirect_uri)
            || request.state.is_empty()
            || request.state.len() > 512
            || !request.state.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(Error::InvalidRequest);
        }
        self.check_resource(request.resource)?;
        Ok(Authorization {
            client_id: request.client_id.to_owned(),
            redirect_uri: request.redirect_uri.to_owned(),
            scopes: Scopes::parse(request.scope.unwrap_or("launcher.read"))?,
            pkce: decode(request.challenge).ok_or(Error::InvalidRequest)?,
            state: request.state.to_owned(),
        })
    }

    pub fn begin_consent(&mut self, now: u64, request: Authorization) -> Result<String, Error> {
        if !self.pairing_open(now) || !self.db.clients.contains_key(&request.client_id) {
            return Err(Error::Closed);
        }
        self.pending.retain(|_, item| item.expires > now);
        if self.pending.len() >= MAX_PENDING {
            return Err(Error::Busy);
        }
        let id = URL_SAFE_NO_PAD.encode(random::<32>()?);
        self.pending.insert(
            id.clone(),
            Pending {
                request,
                expires: now.saturating_add(CONSENT_LIFE),
                approved: None,
            },
        );
        Ok(id)
    }

    pub fn pending(&self, now: u64) -> impl Iterator<Item = Consent> + '_ {
        self.pending.iter().filter_map(move |(id, pending)| {
            let request = &pending.request;
            let client = self.db.clients.get(&request.client_id)?;
            (pending.expires > now && pending.approved.is_none()).then(|| Consent {
                id: id.clone(),
                client_name: client.name.clone(),
                redirect_uri: request.redirect_uri.clone(),
                scopes: request.scopes,
            })
        })
    }

    pub fn decide(&mut self, now: u64, id: &str, approved: bool) -> Result<(), Error> {
        let pending = self
            .pending
            .get_mut(id)
            .filter(|p| p.expires > now && p.approved.is_none())
            .ok_or(Error::InvalidRequest)?;
        pending.approved = Some(approved);
        if approved {
            self.pairing_until = 0;
        }
        Ok(())
    }

    /// The browser can poll only its own opaque, unguessable transaction id.
    pub fn finish(&mut self, now: u64, id: &str, issuer: &str) -> Result<Option<Url>, Error> {
        let pending = self
            .pending
            .get(id)
            .filter(|p| p.expires > now)
            .ok_or(Error::InvalidRequest)?;
        let Some(approved) = pending.approved else {
            return Ok(None);
        };
        let mut redirect =
            Url::parse(&pending.request.redirect_uri).map_err(|_| Error::InvalidRequest)?;
        if approved {
            self.codes.retain(|_, code| code.expires > now);
            if self.codes.len() >= MAX_PENDING {
                return Err(Error::Busy);
            }
            let code = URL_SAFE_NO_PAD.encode(random::<32>()?);
            redirect.query_pairs_mut().append_pair("code", &code);
            self.codes.insert(
                hash(&code),
                Code {
                    request: pending.request.clone(),
                    expires: now.saturating_add(CODE_LIFE),
                },
            );
        } else {
            redirect
                .query_pairs_mut()
                .append_pair("error", "access_denied");
        }
        redirect
            .query_pairs_mut()
            .append_pair("state", &pending.request.state)
            .append_pair("iss", issuer);
        self.pending.remove(id);
        Ok(Some(redirect))
    }

    fn check_resource(&self, resource: Option<&str>) -> Result<(), Error> {
        if resource.is_some_and(|uri| uri != self.db.resource) {
            return Err(Error::InvalidTarget);
        }
        Ok(())
    }

    pub fn exchange(
        &mut self,
        now: u64,
        client_id: &str,
        redirect: &str,
        code: &str,
        verifier: &str,
        resource: Option<&str>,
    ) -> Result<Tokens, Error> {
        if self.disabled {
            return Err(Error::Closed);
        }
        self.check_resource(resource)?;
        let key = hash(code);
        let request = &self
            .codes
            .get(&key)
            .filter(|code| code.expires > now)
            .ok_or(Error::InvalidGrant)?
            .request;
        if request.client_id != client_id
            || request.redirect_uri != redirect
            || !verifier_shape(verifier)
            || !bool::from(request.pkce.ct_eq(&hash(verifier)))
        {
            return Err(Error::InvalidGrant);
        }
        let scopes = request.scopes;
        let client = self.db.clients.get(client_id).ok_or(Error::InvalidClient)?;
        let grant = Grant {
            scopes,
            family: random()?,
            generation: 0,
            current: [0; 32],
            expires: now.saturating_add(REFRESH_LIFE),
            last_activity: now,
            refresh_enabled: client.allows_refresh,
            access: Vec::new(),
        };
        let tokens = self.issue(now, client_id, grant, scopes)?;
        self.codes.remove(&key);
        Ok(tokens)
    }

    fn issue(
        &mut self,
        now: u64,
        client_id: &str,
        mut grant: Grant,
        scopes: Scopes,
    ) -> Result<Tokens, Error> {
        let access_token = URL_SAFE_NO_PAD.encode(random::<32>()?);
        let refresh_token = if grant.refresh_enabled {
            grant.generation = grant.generation.checked_add(1).ok_or(Error::InvalidGrant)?;
            let mut payload = [0u8; 56];
            payload[..16].copy_from_slice(&grant.family);
            payload[16..24].copy_from_slice(&grant.generation.to_be_bytes());
            payload[24..].copy_from_slice(&random::<32>()?);
            let signed = self.signature_payload(client_id, &payload);
            let mac = hmac::sign(
                &hmac::Key::new(hmac::HMAC_SHA256, &self.db.refresh_signing_key),
                &signed,
            );
            let token = URL_SAFE_NO_PAD.encode([payload.as_slice(), mac.as_ref()].concat());
            grant.current = hash(&token);
            Some(token)
        } else {
            None
        };
        let record = SavedAccess {
            verifier: hash(&access_token),
            expires: now.saturating_add(ACCESS_LIFE),
            scopes,
        };
        grant.access.retain(|access| access.expires > now);
        grant.access.truncate(ACCESS_PER_CLIENT - 1);
        grant.access.insert(0, record.clone());
        let keep: Vec<_> = grant.access.iter().map(|access| access.verifier).collect();
        grant.last_activity = now;
        let mut db = self.db.clone();
        db.clients
            .get_mut(client_id)
            .ok_or(Error::InvalidClient)?
            .grant = Some(grant);
        self.commit(db)?;
        self.access.retain(|key, token| {
            token.expires > now && (token.principal.client_id != client_id || keep.contains(key))
        });
        self.access
            .insert(record.verifier, record.access_for(client_id));
        Ok(Tokens {
            access_token,
            token_type: "Bearer",
            expires_in: ACCESS_LIFE,
            scope: scopes.as_text(),
            refresh_token,
        })
    }

    fn signature_payload(&self, client_id: &str, payload: &[u8]) -> Vec<u8> {
        [
            b"Horizon Wii U refresh v1\0".as_slice(),
            client_id.as_bytes(),
            b"\0",
            payload,
        ]
        .concat()
    }

    pub fn refresh(
        &mut self,
        now: u64,
        client_id: &str,
        token: &str,
        resource: Option<&str>,
        scope: Option<&str>,
    ) -> Result<Tokens, Error> {
        if self.disabled {
            return Err(Error::Closed);
        }
        self.check_resource(resource)?;
        let grant = self
            .db
            .clients
            .get(client_id)
            .and_then(|client| client.grant.as_ref())
            .filter(|grant| grant.refresh_enabled && grant.expires > now)
            .ok_or(Error::InvalidGrant)?;
        let decoded = decode::<88>(token).ok_or(Error::InvalidGrant)?;
        let (payload, mac) = decoded.split_at(56);
        let key = hmac::Key::new(hmac::HMAC_SHA256, &self.db.refresh_signing_key);
        hmac::verify(&key, &self.signature_payload(client_id, payload), mac)
            .map_err(|_| Error::InvalidGrant)?;
        if payload[..16] != grant.family {
            return Err(Error::InvalidGrant);
        }
        let generation = u64::from_be_bytes(payload[16..24].try_into().unwrap());
        if generation < grant.generation {
            self.revoke_client(client_id)?;
            return Err(Error::InvalidGrant);
        }
        if generation != grant.generation || !bool::from(grant.current.ct_eq(&hash(token))) {
            return Err(Error::InvalidGrant);
        }
        let scopes = scope
            .map(Scopes::parse)
            .transpose()?
            .unwrap_or(grant.scopes);
        if !grant.scopes.contains(scopes) {
            return Err(Error::InvalidScope);
        }
        // A refresh request can narrow the access token. The rotated refresh
        // token keeps the authorization approved by the player (RFC 6749 §6).
        self.issue(now, client_id, grant.clone(), scopes)
    }

    pub fn authenticate(&mut self, now: u64, token: &str) -> Option<Principal> {
        if self.disabled || token.len() != 43 {
            return None;
        }
        let access = self
            .access
            .get(&hash(token))
            .filter(|token| token.expires > now)?;
        self.activity
            .insert(access.principal.client_id.clone(), now);
        Some(access.principal.clone())
    }

    pub fn revoke_client(&mut self, id: &str) -> Result<(), Error> {
        if self.db.clients.contains_key(id) {
            let mut db = self.db.clone();
            db.clients.remove(id);
            // Block active requests even if the vault cannot commit. Report the
            // failed persistent change to the local operator.
            self.access
                .retain(|_, token| token.principal.client_id != id);
            self.pending
                .retain(|_, pending| pending.request.client_id != id);
            self.codes.retain(|_, code| code.request.client_id != id);
            self.activity.remove(id);
            if let Err(error) = self.commit(db) {
                self.set_disabled(true);
                return Err(error);
            }
        }
        Ok(())
    }

    pub fn revoke_token(&mut self, client_id: &str, token: &str) -> Result<(), Error> {
        if self
            .access
            .get(&hash(token))
            .is_some_and(|access| access.principal.client_id == client_id)
        {
            let verifier = hash(token);
            self.access.remove(&verifier);
            let mut db = self.db.clone();
            if let Some(grant) = db
                .clients
                .get_mut(client_id)
                .and_then(|client| client.grant.as_mut())
            {
                grant.access.retain(|access| access.verifier != verifier);
            }
            if let Err(error) = self.commit(db) {
                self.set_disabled(true);
                return Err(error);
            }
        } else if let Some(decoded) = decode::<88>(token) {
            let (payload, mac) = decoded.split_at(56);
            let key = hmac::Key::new(hmac::HMAC_SHA256, &self.db.refresh_signing_key);
            if hmac::verify(&key, &self.signature_payload(client_id, payload), mac).is_ok()
                && self
                    .db
                    .clients
                    .get(client_id)
                    .and_then(|c| c.grant.as_ref())
                    .is_some_and(|grant| payload[..16] == grant.family)
            {
                self.revoke_client(client_id)?;
            }
        }
        Ok(())
    }

    pub fn trusted(&self) -> impl Iterator<Item = TrustedClient> + '_ {
        self.db.clients.iter().filter_map(|(id, client)| {
            let grant = client.grant.as_ref()?;
            Some(TrustedClient {
                client_id: id.clone(),
                name: client.name.clone(),
                scopes: grant.scopes,
                last_activity: self
                    .activity
                    .get(id)
                    .copied()
                    .unwrap_or(grant.last_activity),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Memory(Arc<Mutex<Option<Zeroizing<Vec<u8>>>>>);

    impl Persistence for Memory {
        fn load(&self) -> io::Result<Option<Zeroizing<Vec<u8>>>> {
            Ok(self.0.lock().unwrap().clone())
        }

        fn save(&mut self, data: &[u8]) -> io::Result<()> {
            *self.0.lock().unwrap() = Some(Zeroizing::new(data.to_vec()));
            Ok(())
        }
    }

    const ISSUER: &str = "https://192.168.1.25:8443";
    const RESOURCE: &str = "https://192.168.1.25:8443/mcp";
    const REDIRECT: &str = "http://127.0.0.1:55171/oauth/callback";
    const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

    fn approve<P: Persistence>(
        oauth: &mut OAuth<P>,
        now: u64,
        client: &str,
        scope: &str,
    ) -> String {
        let auth = oauth
            .authorization(
                now,
                AuthorizeRequest {
                    client_id: client,
                    redirect_uri: REDIRECT,
                    scope: Some(scope),
                    challenge: CHALLENGE,
                    state: "sender-state",
                    resource: Some(RESOURCE),
                },
            )
            .unwrap();
        let id = oauth.begin_consent(now, auth).unwrap();
        assert_eq!(oauth.pending(now).next().unwrap().redirect_uri, REDIRECT);
        assert_eq!(oauth.finish(now, &id, ISSUER).unwrap(), None);
        oauth.decide(now, &id, true).unwrap();
        let callback = oauth.finish(now, &id, ISSUER).unwrap().unwrap();
        let query: BTreeMap<_, _> = callback.query_pairs().collect();
        assert_eq!(query["state"], "sender-state");
        assert_eq!(query["iss"], ISSUER);
        query["code"].to_string()
    }

    #[test]
    fn registration_requires_local_window_and_binding_to_pkce_redirect_and_audience() {
        let mut oauth = OAuth::open(Memory::default(), RESOURCE.into()).unwrap();
        let redirect = [REDIRECT.into()];
        assert_eq!(
            oauth.register(1, "Codex", &redirect, true),
            Err(Error::Closed)
        );
        oauth.open_pairing(1);
        for invalid in [
            "https://evil.example:55171/callback",
            "http://localhost.evil.example:55171/callback",
            "http://localhost@evil.example:55171/callback",
            "http://127.0.0.1:55171/%2e%2e/callback",
            "http://127.0.0.1:55171/callback?code=preexisting",
            "http://127.0.0.1:55171/callback#frag",
        ] {
            assert_eq!(
                oauth.register(1, "Codex", &[invalid.into()], true),
                Err(Error::InvalidRequest)
            );
        }
        let id = oauth.register(1, "Codex", &redirect, true).unwrap();
        assert_eq!(oauth.trusted().count(), 0);
        assert!(matches!(
            oauth.exchange(1, &id, REDIRECT, "made-up", VERIFIER, None),
            Err(Error::InvalidGrant)
        ));
        let code = approve(&mut oauth, 2, &id, "launcher.read game.observe");
        assert!(matches!(
            oauth.exchange(3, &id, REDIRECT, &code, VERIFIER, Some("https://other/mcp")),
            Err(Error::InvalidTarget)
        ));
        assert!(matches!(
            oauth.exchange(
                3,
                &id,
                "http://127.0.0.1:55172/oauth/callback",
                &code,
                VERIFIER,
                None
            ),
            Err(Error::InvalidGrant)
        ));
        assert!(matches!(
            oauth.exchange(
                3,
                &id,
                REDIRECT,
                &code,
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                None
            ),
            Err(Error::InvalidGrant)
        ));
        let tokens = oauth
            .exchange(3, &id, REDIRECT, &code, VERIFIER, Some(RESOURCE))
            .unwrap();
        assert!(matches!(
            oauth.exchange(3, &id, REDIRECT, &code, VERIFIER, None),
            Err(Error::InvalidGrant)
        ));
        let principal = oauth.authenticate(3, &tokens.access_token).unwrap();
        assert_eq!(
            principal.scopes,
            Scopes::LAUNCHER_READ.union(Scopes::GAME_OBSERVE)
        );
        assert!(oauth
            .authenticate(3 + ACCESS_LIFE, &tokens.access_token)
            .is_none());
    }

    #[test]
    fn remembered_clients_refresh_across_restarts_and_replay_revokes_the_family() {
        let memory = Memory::default();
        let mut oauth = OAuth::open(memory.clone(), RESOURCE.into()).unwrap();
        let mut devices = Vec::new();
        for (name, scope) in [
            ("Codex", "launcher.read game.control"),
            ("Gemini", "launcher.read"),
        ] {
            oauth.open_pairing(1);
            let id = oauth.register(1, name, &[REDIRECT.into()], true).unwrap();
            let code = approve(&mut oauth, 2, &id, scope);
            let tokens = oauth
                .exchange(2, &id, REDIRECT, &code, VERIFIER, None)
                .unwrap();
            devices.push((id, tokens));
        }
        let (codex_id, codex) = &devices[0];
        let (gemini_id, gemini) = &devices[1];
        let old = codex.refresh_token.as_deref().unwrap();
        let mut oauth = OAuth::open(memory.clone(), RESOURCE.into()).unwrap();
        assert_eq!(oauth.trusted().count(), 2);
        assert!(oauth.authenticate(3, &codex.access_token).is_some());
        oauth.revoke_access_tokens().unwrap();
        let mut oauth = OAuth::open(memory.clone(), RESOURCE.into()).unwrap();
        assert!(oauth.authenticate(3, &codex.access_token).is_none());
        assert!(matches!(
            oauth.refresh(4, gemini_id, old, None, None),
            Err(Error::InvalidGrant)
        ));
        assert!(matches!(
            oauth.refresh(4, codex_id, old, None, Some("launcher.manage")),
            Err(Error::InvalidScope)
        ));
        let new = oauth
            .refresh(4, codex_id, old, Some(RESOURCE), Some("launcher.read"))
            .unwrap();
        assert_ne!(new.refresh_token, codex.refresh_token);
        let mut oauth = OAuth::open(memory.clone(), RESOURCE.into()).unwrap();
        assert_eq!(
            oauth.authenticate(4, &new.access_token).unwrap().scopes,
            Scopes::LAUNCHER_READ
        );
        let restored = oauth
            .refresh(
                4,
                codex_id,
                new.refresh_token.as_deref().unwrap(),
                None,
                None,
            )
            .unwrap();
        assert_eq!(
            oauth
                .authenticate(4, &restored.access_token)
                .unwrap()
                .scopes,
            Scopes::LAUNCHER_READ.union(Scopes::GAME_CONTROL)
        );
        oauth.set_disabled(true);
        assert!(oauth.authenticate(4, &restored.access_token).is_none());
        assert!(matches!(
            oauth.refresh(4, codex_id, old, None, None),
            Err(Error::Closed)
        ));
        let mut oauth = OAuth::open(memory.clone(), RESOURCE.into()).unwrap();
        assert!(matches!(
            oauth.refresh(5, codex_id, old, None, None),
            Err(Error::InvalidGrant)
        ));
        assert!(matches!(
            oauth.refresh(
                5,
                codex_id,
                restored.refresh_token.as_deref().unwrap(),
                None,
                None
            ),
            Err(Error::InvalidGrant)
        ));
        let mut oauth = OAuth::open(memory, RESOURCE.into()).unwrap();
        let new_gemini = oauth
            .refresh(
                6,
                gemini_id,
                gemini.refresh_token.as_deref().unwrap(),
                None,
                None,
            )
            .unwrap();
        oauth.revoke_client(gemini_id).unwrap();
        assert!(oauth.authenticate(6, &new_gemini.access_token).is_none());
        assert_eq!(oauth.trusted().count(), 0);
    }
}
