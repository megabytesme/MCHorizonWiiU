use crate::http::{Request, Response};
use crate::oauth::{
    self, Authorization, AuthorizeRequest, Error, OAuth, Persistence, Principal, Scopes,
};
use crate::protocol::{self, Fault, Tools};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const PROTECTED: &str = "/.well-known/oauth-protected-resource/mcp";
const AUTH_METADATA: &str = "/.well-known/oauth-authorization-server";

pub trait Backend {
    fn list(&self, principal: &Principal) -> Vec<Value>;
    fn required(&self, name: &str) -> Option<Scopes>;
    fn call(
        &self,
        principal: &Principal,
        name: &str,
        args: &Value,
        disconnected: &dyn Fn() -> bool,
    ) -> Result<Value, Fault>;
}

struct AuthorizedTools<'a, T> {
    backend: &'a T,
    principal: Principal,
    metadata: &'a str,
    missing: Mutex<Option<Scopes>>,
    disconnected: &'a dyn Fn() -> bool,
}

impl<T: Backend> Tools for AuthorizedTools<'_, T> {
    fn list(&self) -> Vec<Value> {
        self.backend.list(&self.principal)
    }

    fn call(&self, name: &str, args: &Value) -> Result<Value, Fault> {
        let scopes = self
            .backend
            .required(name)
            .ok_or_else(|| Fault::new(200, -32602, "Unknown tool"))?;
        if !self.principal.scopes.contains(scopes) {
            *self.missing.lock().unwrap() = Some(scopes);
            return Err(Fault::new(403, -32000, "Insufficient scope"));
        }
        self.backend
            .call(&self.principal, name, args, self.disconnected)
    }
}

struct BrowserConfirmation {
    expires: u64,
    request: Authorization,
}

struct Inner<P> {
    oauth: OAuth<P>,
    browser: BTreeMap<String, BrowserConfirmation>,
}

/// Owns a single OAuth authority and restricts requests to the configured
/// certificate origin. Policy and consent methods are only exposed in-process.
pub struct Router<P, T> {
    issuer: String,
    authority: String,
    metadata: String,
    inner: Mutex<Inner<P>>,
    backend: T,
}

impl<P: Persistence, T: Backend> Router<P, T> {
    pub fn new(issuer: String, oauth: OAuth<P>, backend: T) -> Result<Self, Error> {
        let authority = issuer
            .strip_prefix("https://")
            .ok_or(Error::InvalidTarget)?
            .to_owned();
        if authority.contains(['/', '\\', '?', '#', '@'])
            || url::Url::parse(&issuer).is_err()
            || oauth.resource() != format!("{issuer}/mcp")
        {
            return Err(Error::InvalidTarget);
        }
        let metadata = format!("{issuer}{PROTECTED}");
        Ok(Self {
            issuer,
            authority,
            metadata,
            backend,
            inner: Mutex::new(Inner {
                oauth,
                browser: BTreeMap::new(),
            }),
        })
    }

    pub fn local<R>(&self, local: impl FnOnce(&mut OAuth<P>, u64) -> R) -> R {
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        local(&mut inner.oauth, now())
    }

    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    pub fn handle(&self, request: Request, disconnected: &dyn Fn() -> bool) -> Response {
        if !request
            .header("host")
            .is_some_and(|host| host.eq_ignore_ascii_case(&self.authority))
        {
            return Response::empty(400);
        }
        if request
            .header("origin")
            .is_some_and(|origin| origin != self.issuer)
        {
            return Response::empty(403);
        }
        let (path, query) = request
            .target
            .split_once('?')
            .unwrap_or((&request.target, ""));
        let now = now();
        match (request.method.as_str(), path) {
            ("GET", PROTECTED | "/.well-known/oauth-protected-resource") if query.is_empty() => {
                Response::json(
                    200,
                    json!({
                        "resource": format!("{}/mcp", self.issuer),
                        "authorization_servers": [&self.issuer],
                        "scopes_supported": Scopes::names().collect::<Vec<_>>(),
                        "bearer_methods_supported": ["header"],
                    }),
                )
            }
            ("GET", AUTH_METADATA) if query.is_empty() => Response::json(
                200,
                json!({
                    "issuer": self.issuer,
                    "authorization_endpoint": format!("{}/oauth/authorize", self.issuer),
                    "registration_endpoint": format!("{}/oauth/register", self.issuer),
                    "token_endpoint": format!("{}/oauth/token", self.issuer),
                    "revocation_endpoint": format!("{}/oauth/revoke", self.issuer),
                    "response_types_supported": ["code"],
                    "grant_types_supported": ["authorization_code", "refresh_token"],
                    "code_challenge_methods_supported": ["S256"],
                    "token_endpoint_auth_methods_supported": ["none"],
                    "revocation_endpoint_auth_methods_supported": ["none"],
                    "authorization_response_iss_parameter_supported": true,
                    "scopes_supported": Scopes::names().collect::<Vec<_>>(),
                }),
            ),
            ("POST", "/oauth/register") if query.is_empty() => self.register(&request, now),
            ("POST", "/oauth/token") if query.is_empty() => self.token(&request, now, false),
            ("POST", "/oauth/revoke") if query.is_empty() => self.token(&request, now, true),
            ("GET", "/oauth/authorize") => self.authorize(query, now),
            ("POST", "/oauth/authorize") if query.is_empty() => self.browser_consent(&request, now),
            ("GET", "/oauth/wait") => self.poll(query, now),
            (_, "/mcp") if query.is_empty() => self.mcp(&request, now, disconnected),
            _ => Response::empty(404),
        }
    }

    fn register(&self, request: &Request, now: u64) -> Response {
        if request.media_type() != Some("application/json") {
            return Response::empty(415);
        }
        let result = (|| {
            let body: Value =
                serde_json::from_slice(&request.body).map_err(|_| Error::InvalidRequest)?;
            let name = body["client_name"].as_str().unwrap_or("MCP client");
            let redirect = body["redirect_uris"]
                .as_array()
                .ok_or(Error::InvalidRequest)?;
            let redirect: Vec<String> = redirect
                .iter()
                .map(|uri| uri.as_str().map(str::to_owned).ok_or(Error::InvalidRequest))
                .collect::<Result<_, _>>()?;
            let grants = body["grant_types"].as_array();
            let refresh = grants.is_none_or(|grants| grants.iter().any(|x| x == "refresh_token"));
            if grants.is_some_and(|g| {
                !g.iter().any(|x| x == "authorization_code")
                    || g.iter().any(|x| {
                        !matches!(x.as_str(), Some("authorization_code" | "refresh_token"))
                    })
            }) || body
                .get("token_endpoint_auth_method")
                .is_some_and(|v| v != "none")
                || body
                    .get("response_types")
                    .is_some_and(|v| v != &json!(["code"]))
            {
                return Err(Error::InvalidRequest);
            }
            let id = self.lock().oauth.register(now, name, &redirect, refresh)?;
            Ok(json!({
                "client_id": id, "client_id_issued_at": now, "client_name": name,
                "redirect_uris": redirect, "token_endpoint_auth_method": "none",
                "grant_types": if refresh { vec!["authorization_code", "refresh_token"] } else { vec!["authorization_code"] },
                "response_types": ["code"],
            }))
        })();
        match result {
            Ok(json) => Response::json(201, json),
            Err(error) => oauth_error(error),
        }
    }

    fn authorize(&self, query: &str, now: u64) -> Response {
        let result = (|| {
            let fields = Fields::parse(query)?;
            if fields.required("response_type")? != "code"
                || fields.required("code_challenge_method")? != "S256"
            {
                return Err(Error::InvalidRequest);
            }
            let mut inner = self.lock();
            let request = inner.oauth.authorization(
                now,
                AuthorizeRequest {
                    client_id: fields.required("client_id")?,
                    redirect_uri: fields.required("redirect_uri")?,
                    scope: fields.get("scope"),
                    challenge: fields.required("code_challenge")?,
                    state: fields.required("state")?,
                    resource: fields.get("resource"),
                },
            )?;
            inner.browser.retain(|_, item| item.expires > now);
            if inner.browser.len() >= 8 {
                return Err(Error::Busy);
            }
            let nonce = URL_SAFE_NO_PAD.encode(oauth::random::<32>()?);
            inner.browser.insert(
                nonce.clone(),
                BrowserConfirmation {
                    expires: now.saturating_add(120),
                    request,
                },
            );
            Ok(html(
                "Connect to Horizon Wii U",
                &format!(
                    r#"<p>Send this request to the Wii U. Review the client and its requested access on the GamePad.</p><form action="/oauth/authorize" method="post"><input type="hidden" name="csrf" value="{nonce}"><button>Request approval</button></form>"#
                ),
            ))
        })();
        result.unwrap_or_else(oauth_error)
    }

    fn browser_consent(&self, request: &Request, now: u64) -> Response {
        if request.header("origin") != Some(&self.issuer)
            || request
                .header("sec-fetch-site")
                .is_some_and(|s| s != "same-origin")
        {
            return Response::empty(403);
        }
        let result = (|| {
            let fields = form(request)?;
            let mut inner = self.lock();
            let nonce = fields.required("csrf")?;
            let request = inner
                .browser
                .remove(nonce)
                .filter(|p| p.expires > now)
                .ok_or(Error::InvalidRequest)?;
            let id = inner.oauth.begin_consent(now, request.request)?;
            Ok(Response::empty(303).header("Location", format!("/oauth/wait?id={id}")))
        })();
        result.unwrap_or_else(oauth_error)
    }

    fn poll(&self, query: &str, now: u64) -> Response {
        let result = (|| {
            let fields = Fields::parse(query)?;
            let id = fields.required("id")?;
            let response = match self.lock().oauth.finish(now, id, &self.issuer)? {
                Some(redirect) => Response::empty(303).header("Location", redirect.to_string()),
                None => html(
                    "Awaiting GamePad approval",
                    &format!(
                        r#"<p>Compare pairing ID <b>{}</b> with the one on your GamePad, then approve or deny the request there.</p><meta http-equiv="refresh" content="2">"#,
                        &id[..8]
                    ),
                ),
            };
            Ok(response)
        })();
        result.unwrap_or_else(oauth_error)
    }

    fn token(&self, request: &Request, now: u64, revoke: bool) -> Response {
        let result = (|| {
            let fields = form(request)?;
            if request.header("authorization").is_some() || fields.get("client_secret").is_some() {
                return Err(Error::InvalidClient);
            }
            let id = fields.required("client_id")?;
            let mut inner = self.lock();
            if revoke {
                inner.oauth.revoke_token(id, fields.required("token")?)?;
                return Ok(Response::empty(200));
            }
            let resource = fields.get("resource");
            let tokens = match fields.required("grant_type")? {
                "authorization_code" => inner.oauth.exchange(
                    now,
                    id,
                    fields.required("redirect_uri")?,
                    fields.required("code")?,
                    fields.required("code_verifier")?,
                    resource,
                )?,
                "refresh_token" => inner.oauth.refresh(
                    now,
                    id,
                    fields.required("refresh_token")?,
                    resource,
                    fields.get("scope"),
                )?,
                _ => {
                    return Ok(Response::json(
                        400,
                        json!({"error":"unsupported_grant_type"}),
                    ))
                }
            };
            Ok(Response::json(
                200,
                serde_json::to_value(tokens).map_err(|_| Error::Storage)?,
            ))
        })();
        result.unwrap_or_else(oauth_error)
    }

    fn mcp(&self, request: &Request, now: u64, disconnected: &dyn Fn() -> bool) -> Response {
        let auth = request.header("authorization").and_then(|value| {
            let (scheme, token) = value.split_once(' ')?;
            scheme.eq_ignore_ascii_case("bearer").then_some(token)
        });
        let principal = auth.and_then(|token| self.lock().oauth.authenticate(now, token));
        let Some(principal) = principal else {
            let comment = if request.header("authorization").is_some() {
                r#"error="invalid_token", "#
            } else {
                ""
            };
            return Response::empty(401).header(
                "WWW-Authenticate",
                format!(
                    r#"Bearer {comment}resource_metadata="{}", scope="launcher.read""#,
                    self.metadata
                ),
            );
        };
        let toolbox = AuthorizedTools {
            backend: &self.backend,
            principal,
            metadata: &self.metadata,
            missing: Mutex::new(None),
            disconnected,
        };
        let response = protocol::dispatch(request, &toolbox);
        let scope = *toolbox.missing.lock().unwrap();
        match scope {
            Some(scope) => response.header(
                "WWW-Authenticate",
                format!(
                    r#"Bearer error="insufficient_scope", resource_metadata="{}", scope="{}""#,
                    toolbox.metadata,
                    scope.as_text()
                ),
            ),
            None => response,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner<P>> {
        self.inner.lock().unwrap_or_else(|error| error.into_inner())
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| time.as_secs())
}

fn html(title: &str, body: &str) -> Response {
    Response::bytes(
        200, "text/html; charset=utf-8",
        format!(r#"<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>{title}</title><main><h1>{title}</h1>{body}</main></html>"#).into_bytes(),
    ).header("Content-Security-Policy", "default-src 'none'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'")
        .header("Referrer-Policy", "no-referrer")
        .header("X-Frame-Options", "DENY")
}

fn oauth_error(error: Error) -> Response {
    let (status, error, description) = match error {
        Error::InvalidRequest => (
            400,
            "invalid_request",
            "The request is malformed or expired.",
        ),
        Error::InvalidClient => (
            401,
            "invalid_client",
            "Register the public client before pairing.",
        ),
        Error::InvalidScope => (
            400,
            "invalid_scope",
            "Request a supported Agent Control scope.",
        ),
        Error::InvalidGrant => (
            400,
            "invalid_grant",
            "The credential is invalid or expired.",
        ),
        Error::InvalidTarget => (
            400,
            "invalid_target",
            "Use the console's configured MCP URL.",
        ),
        Error::Closed => (
            403,
            "access_denied",
            "Enable Agent Control and open pairing on the Wii U.",
        ),
        Error::Busy => (
            429,
            "temporarily_unavailable",
            "The Wii U has reached its client or pairing limit.",
        ),
        Error::Storage | Error::Random => (
            503,
            "temporarily_unavailable",
            "The Wii U could not complete the security operation.",
        ),
    };
    Response::json(
        status,
        json!({"error": error, "error_description": description}),
    )
}

struct Fields(BTreeMap<String, String>);

impl Fields {
    fn parse(query: &str) -> Result<Self, Error> {
        if query.len() > 8192 {
            return Err(Error::InvalidRequest);
        }
        let mut fields = BTreeMap::new();
        for (name, value) in url::form_urlencoded::parse(query.as_bytes()) {
            if fields.len() >= 24
                || fields
                    .insert(name.into_owned(), value.into_owned())
                    .is_some()
            {
                return Err(Error::InvalidRequest);
            }
        }
        Ok(Self(fields))
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }

    fn required(&self, name: &str) -> Result<&str, Error> {
        self.get(name)
            .filter(|v| !v.is_empty())
            .ok_or(Error::InvalidRequest)
    }
}

fn form(request: &Request) -> Result<Fields, Error> {
    if request.media_type() != Some("application/x-www-form-urlencoded") {
        return Err(Error::InvalidRequest);
    }
    Fields::parse(std::str::from_utf8(&request.body).map_err(|_| Error::InvalidRequest)?)
}
