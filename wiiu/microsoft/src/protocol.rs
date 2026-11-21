use std::io::Read;
use std::time::{Duration, Instant};

use horizon_wiiu_security::Secret;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

const DEVICE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBOX: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const LOGIN: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const ENTITLEMENTS: &str = "https://api.minecraftservices.com/entitlements/mcstore";
const PROFILE: &str = "https://api.minecraftservices.com/minecraft/profile";
const SCOPE: &str = "XboxLive.signin offline_access";
const RESPONSE_LIMIT: u64 = 256 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Unconfigured,
    Network,
    Protocol,
    Reauthenticate,
    Denied,
    CodeExpired,
    XboxProfile,
    XboxFamily,
    XboxUnavailable,
    Registration,
    OwnershipMissing,
    ProfileMissing,
    IdentityChanged,
    Storage,
    Vault(horizon_wiiu_security::VaultError),
}

impl From<horizon_wiiu_security::VaultError> for Error {
    fn from(value: horizon_wiiu_security::VaultError) -> Self {
        Self::Vault(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProfileId(pub u128);

impl ProfileId {
    pub fn parse(value: &str) -> Option<Self> {
        (value.len() == 32 && value.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| u128::from_str_radix(value, 16).ok())
            .flatten()
            .filter(|uuid| *uuid != 0)
            .map(Self)
    }
}

/// Credentials are never serialized or part of the Debug output.
pub struct Identity {
    pub id: ProfileId,
    pub name: String,
    token: Secret,
    expires: Instant,
}

impl Identity {
    pub fn access_token(&self) -> &str {
        expose(&self.token)
    }

    pub fn is_current(&self) -> bool {
        self.is_current_at(Instant::now())
    }

    pub(crate) fn is_current_at(&self, now: Instant) -> bool {
        self.expires.saturating_duration_since(now) > Duration::from_secs(60)
    }

    pub fn duplicate(&self) -> Self {
        Self {
            id: self.id,
            name: self.name.clone(),
            token: Secret::new(self.token.expose().to_vec()),
            expires: self.expires,
        }
    }
}

pub(crate) struct Authorized {
    pub(crate) identity: Identity,
    pub(crate) refresh: Secret,
}

pub(crate) struct DeviceCode {
    pub(crate) user_code: Secret,
    pub(crate) verification_uri: String,
    pub(crate) expires: Instant,
    pub(crate) interval: Duration,
    code: Secret,
}

pub(crate) enum Poll {
    Pending,
    SlowDown,
    Completed(MicrosoftTokens),
}

pub(crate) struct MicrosoftTokens {
    access: Secret,
    refresh: Secret,
}

pub(crate) enum Payload<'a> {
    Form(&'a [(&'a str, &'a str)]),
    Json(&'a [u8]),
    Bearer(&'a str),
}

pub(crate) trait Transport: Send + Sync {
    fn send(&self, url: &str, payload: Payload<'_>) -> Result<(u16, Secret), Error>;
}

pub(crate) struct Https(ureq::Agent);

impl Https {
    pub(crate) fn new() -> Self {
        // ureq's rustls provider uses its bundled WebPKI roots. Keep the
        // endpoint allowlist fixed and reject all redirects.
        let timeout = Duration::from_secs(12);
        let agent = ureq::AgentBuilder::new()
            .https_only(true)
            .redirects(0)
            .timeout_connect(timeout)
            .timeout_read(timeout);
        // Cafe std supports connect and read timeouts. ureq's overall timeout
        // also sets SO_SNDTIMEO, which Cafe rejects before the TLS handshake.
        #[cfg(not(target_os = "cafe"))]
        let agent = agent.timeout(timeout);
        Self(agent.build())
    }
}

impl Transport for Https {
    fn send(&self, url: &str, payload: Payload<'_>) -> Result<(u16, Secret), Error> {
        let result = match payload {
            Payload::Form(form) => self.0.post(url).send_form(form),
            Payload::Json(json) => self
                .0
                .post(url)
                .set("Content-Type", "application/json")
                .set("Accept", "application/json")
                .send_bytes(json),
            Payload::Bearer(token) => {
                let header = Zeroizing::new(format!("Bearer {token}"));
                self.0.get(url).set("Authorization", &header).call()
            }
        };
        let response = match result {
            Ok(response) | Err(ureq::Error::Status(_, response)) => response,
            Err(_) => return Err(Error::Network),
        };
        let status = response.status();
        let mut body = Zeroizing::new(Vec::with_capacity(RESPONSE_LIMIT as usize + 1));
        response
            .into_reader()
            .take(RESPONSE_LIMIT + 1)
            .read_to_end(&mut body)
            .map_err(|_| Error::Network)?;
        if body.len() as u64 > RESPONSE_LIMIT {
            return Err(Error::Protocol);
        }
        Ok((status, Secret::new(std::mem::take(&mut *body))))
    }
}

fn request<T: DeserializeOwned>(
    transport: &dyn Transport,
    url: &str,
    payload: Payload<'_>,
) -> Result<(u16, Result<T, Error>), Error> {
    let (status, body) = transport.send(url, payload)?;
    if status == 429 || status >= 500 {
        return Err(Error::Network);
    }
    let value = serde_json::from_slice(body.expose()).map_err(|_| Error::Protocol);
    Ok((status, value))
}

fn json<T: DeserializeOwned>(
    transport: &dyn Transport,
    url: &str,
    body: impl Serialize,
) -> Result<(u16, Result<T, Error>), Error> {
    let body = Zeroizing::new(serde_json::to_vec(&body).map_err(|_| Error::Protocol)?);
    request(transport, url, Payload::Json(&body))
}

fn expose(secret: &Secret) -> &str {
    std::str::from_utf8(secret.expose()).unwrap_or("")
}

fn parse_secret<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Secret, D::Error> {
    let value = Secret::new(String::deserialize(d)?.into_bytes());
    if valid_token(expose(&value)) {
        Ok(value)
    } else {
        Err(serde::de::Error::custom("invalid credential"))
    }
}

fn parse_optional_secret<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<Secret>, D::Error> {
    parse_secret(d).map(Some)
}

fn valid_token(value: &str) -> bool {
    !value.is_empty() && value.len() <= 32 * 1024 && value.bytes().all(|b| b.is_ascii_graphic())
}

pub(crate) fn valid_player(value: &str) -> bool {
    (1..=16).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

#[derive(Deserialize)]
struct DeviceResponse {
    #[serde(deserialize_with = "parse_secret")]
    user_code: Secret,
    #[serde(deserialize_with = "parse_secret")]
    device_code: Secret,
    verification_uri: String,
    expires_in: u64,
    #[serde(default = "default_interval")]
    interval: u64,
}

fn default_interval() -> u64 {
    5
}

pub(crate) fn begin(transport: &dyn Transport, client: &str) -> Result<DeviceCode, Error> {
    let (status, response) = request::<DeviceResponse>(
        transport,
        DEVICE,
        Payload::Form(&[("client_id", client), ("scope", SCOPE)]),
    )?;
    if status != 200 {
        return Err(Error::Registration);
    }
    let response = response?;
    if !matches!(
        response.verification_uri.as_str(),
        "https://www.microsoft.com/link"
            | "https://microsoft.com/link"
            | "https://microsoft.com/devicelogin"
            | "https://www.microsoft.com/devicelogin"
    ) || !(1..=3600).contains(&response.expires_in)
        || !(1..=60).contains(&response.interval)
        || !(4..=16).contains(&response.user_code.expose().len())
        || !response
            .user_code
            .expose()
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'-')
    {
        return Err(Error::Protocol);
    }
    Ok(DeviceCode {
        user_code: response.user_code,
        verification_uri: response.verification_uri,
        expires: Instant::now() + Duration::from_secs(response.expires_in),
        interval: Duration::from_secs(response.interval),
        code: response.device_code,
    })
}

#[derive(Deserialize)]
struct OAuth {
    #[serde(default, deserialize_with = "parse_optional_secret")]
    access_token: Option<Secret>,
    #[serde(default, deserialize_with = "parse_optional_secret")]
    refresh_token: Option<Secret>,
    error: Option<String>,
}

impl OAuth {
    fn tokens(self, fallback: Option<Secret>) -> Result<MicrosoftTokens, Error> {
        Ok(MicrosoftTokens {
            access: self.access_token.ok_or(Error::Protocol)?,
            refresh: self.refresh_token.or(fallback).ok_or(Error::Protocol)?,
        })
    }
}

pub(crate) fn poll(
    transport: &dyn Transport,
    client: &str,
    device: &DeviceCode,
) -> Result<Poll, Error> {
    let (status, response) = request::<OAuth>(
        transport,
        TOKEN,
        Payload::Form(&[
            ("client_id", client),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("device_code", expose(&device.code)),
        ]),
    )?;
    let response = response?;
    if status == 200 {
        return response.tokens(None).map(Poll::Completed);
    }
    match response.error.as_deref() {
        Some("authorization_pending") => Ok(Poll::Pending),
        Some("slow_down") => Ok(Poll::SlowDown),
        Some("authorization_declined" | "access_denied") => Err(Error::Denied),
        Some("expired_token" | "bad_verification_code") => Err(Error::CodeExpired),
        Some("invalid_client" | "unauthorized_client") => Err(Error::Registration),
        _ => Err(Error::Protocol),
    }
}

pub(crate) fn refresh(
    transport: &dyn Transport,
    client: &str,
    credential: Secret,
) -> Result<MicrosoftTokens, Error> {
    if !valid_token(expose(&credential)) {
        return Err(Error::Reauthenticate);
    }
    let (status, response) = request::<OAuth>(
        transport,
        TOKEN,
        Payload::Form(&[
            ("client_id", client),
            ("grant_type", "refresh_token"),
            ("scope", SCOPE),
            ("refresh_token", expose(&credential)),
        ]),
    )?;
    let response = response?;
    if status == 200 {
        return response.tokens(Some(credential));
    }
    match response.error.as_deref() {
        Some("invalid_grant" | "interaction_required" | "invalid_token") => {
            Err(Error::Reauthenticate)
        }
        Some("invalid_client" | "unauthorized_client") => Err(Error::Registration),
        _ => Err(Error::Protocol),
    }
}

#[derive(Deserialize)]
struct XboxToken {
    #[serde(rename = "Token", default, deserialize_with = "parse_optional_secret")]
    token: Option<Secret>,
    #[serde(rename = "DisplayClaims", default)]
    claims: Option<XboxClaims>,
    #[serde(rename = "XErr", default)]
    error: Option<u64>,
}

#[derive(Deserialize)]
struct XboxClaims {
    xui: Vec<XboxUser>,
}

#[derive(Deserialize)]
struct XboxUser {
    uhs: String,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct XboxRequest<'a, P> {
    properties: P,
    relying_party: &'a str,
    token_type: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct XboxUserProperties<'a> {
    auth_method: &'a str,
    site_name: &'a str,
    rps_ticket: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct XboxXstsProperties<'a> {
    sandbox_id: &'a str,
    user_tokens: [&'a str; 1],
}

#[derive(Deserialize)]
struct MinecraftToken {
    #[serde(deserialize_with = "parse_secret")]
    access_token: Secret,
    expires_in: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MinecraftRequest<'a> {
    identity_token: &'a str,
}

#[derive(Deserialize)]
struct Entitlements {
    items: Vec<Entitlement>,
}

#[derive(Deserialize)]
struct Entitlement {
    name: String,
}

#[derive(Deserialize)]
struct MinecraftProfile {
    id: String,
    name: String,
}

pub(crate) fn authorize(
    transport: &dyn Transport,
    tokens: MicrosoftTokens,
    cancelled: impl Fn() -> bool,
) -> Result<Authorized, Error> {
    let check = || {
        if cancelled() {
            Err(Error::Denied)
        } else {
            Ok(())
        }
    };
    check()?;
    let ticket = Zeroizing::new(format!("d={}", expose(&tokens.access)));
    let (status, xbox) = json::<XboxToken>(
        transport,
        XBOX,
        XboxRequest {
            properties: XboxUserProperties {
                auth_method: "RPS",
                site_name: "user.auth.xboxlive.com",
                rps_ticket: &ticket,
            },
            relying_party: "http://auth.xboxlive.com",
            token_type: "JWT",
        },
    )?;
    drop(ticket);
    drop(tokens.access);
    check()?;
    let xbox = xbox?
        .token
        .filter(|_| status == 200)
        .ok_or(Error::XboxUnavailable)?;
    let (status, xsts) = json::<XboxToken>(
        transport,
        XSTS,
        XboxRequest {
            properties: XboxXstsProperties {
                sandbox_id: "RETAIL",
                user_tokens: [expose(&xbox)],
            },
            relying_party: "rp://api.minecraftservices.com/",
            token_type: "JWT",
        },
    )?;
    drop(xbox);
    check()?;
    let xsts = xsts?;
    if let Some(code) = xsts.error {
        return Err(match code {
            2148916233 => Error::XboxProfile,
            2148916236..=2148916238 => Error::XboxFamily,
            _ => Error::XboxUnavailable,
        });
    }
    let xsts_token = xsts
        .token
        .filter(|_| status == 200)
        .ok_or(Error::XboxUnavailable)?;
    let user = xsts
        .claims
        .as_ref()
        .and_then(|claims| claims.xui.first())
        .filter(|user| {
            !user.uhs.is_empty()
                && user.uhs.len() <= 32
                && user.uhs.bytes().all(|b| b.is_ascii_digit())
        })
        .ok_or(Error::Protocol)?;
    let xbox_identity = Zeroizing::new(format!("XBL3.0 x={};{}", user.uhs, expose(&xsts_token)));
    let (status, session) = json::<MinecraftToken>(
        transport,
        LOGIN,
        MinecraftRequest {
            identity_token: &xbox_identity,
        },
    )?;
    drop(xbox_identity);
    drop(xsts_token);
    check()?;
    if status == 403 {
        return Err(Error::Registration);
    }
    if status != 200 {
        return Err(Error::Protocol);
    }
    let session = session?;
    if !(61..=604_800).contains(&session.expires_in) {
        return Err(Error::Protocol);
    }
    let expires = Instant::now() + Duration::from_secs(session.expires_in);
    let bearer = Payload::Bearer(expose(&session.access_token));
    let (status, entitlements) = request::<Entitlements>(transport, ENTITLEMENTS, bearer)?;
    check()?;
    if status != 200 {
        return Err(Error::Protocol);
    }
    if !entitlements?
        .items
        .iter()
        .any(|item| matches!(item.name.as_str(), "game_minecraft" | "product_minecraft"))
    {
        return Err(Error::OwnershipMissing);
    }
    let (status, profile) = request::<MinecraftProfile>(
        transport,
        PROFILE,
        Payload::Bearer(expose(&session.access_token)),
    )?;
    check()?;
    if status == 404 {
        return Err(Error::ProfileMissing);
    }
    if status != 200 {
        return Err(Error::Protocol);
    }
    let profile = profile?;
    let id = ProfileId::parse(&profile.id).ok_or(Error::Protocol)?;
    if !valid_player(&profile.name) {
        return Err(Error::Protocol);
    }
    Ok(Authorized {
        identity: Identity {
            id,
            name: profile.name,
            token: session.access_token,
            expires,
        },
        refresh: tokens.refresh,
    })
}
