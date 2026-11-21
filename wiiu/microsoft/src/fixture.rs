//! Synthetic transport used only by host tests and the separately named Cemu fixture.

use std::sync::atomic::{AtomicUsize, Ordering};

use horizon_wiiu_security::Secret;
use serde_json::json;

use crate::protocol::{Error, Payload, Transport};

#[derive(Default)]
pub(crate) struct Fixture {
    attempts: AtomicUsize,
}

impl Transport for Fixture {
    fn send(&self, url: &str, payload: Payload<'_>) -> Result<(u16, Secret), Error> {
        let mut account = self.attempts.load(Ordering::Relaxed).max(1);
        let (status, body) = if url.ends_with("/devicecode") {
            account = self.attempts.fetch_add(1, Ordering::Relaxed) + 1;
            (
                200,
                json!({
                    "user_code": "MOCK-ONLY",
                    "device_code": format!("fake-device-{account}"),
                    "verification_uri": "https://www.microsoft.com/link",
                    "interval": 2,
                    "expires_in": 300
                }),
            )
        } else if url.ends_with("/token") {
            if let Payload::Form(fields) = payload {
                account = fields
                    .iter()
                    .find(|(key, _)| matches!(*key, "refresh_token" | "device_code"))
                    .and_then(|(_, value)| value.rsplit('-').next())
                    .and_then(|index| index.parse().ok())
                    .ok_or(Error::Protocol)?;
            }
            (
                200,
                json!({ "refresh_token": format!("fake-refresh-{account}"),
                "access_token": format!("fake-microsoft-{account}") }),
            )
        } else if url.ends_with("/user/authenticate") {
            let Payload::Json(body) = payload else {
                return Err(Error::Protocol);
            };
            let body: serde_json::Value =
                serde_json::from_slice(body).map_err(|_| Error::Protocol)?;
            account = body["Properties"]["RpsTicket"]
                .as_str()
                .and_then(fixture_id)
                .ok_or(Error::Protocol)?;
            (200, json!({ "Token": format!("fake-xbox-{account}") }))
        } else if url.ends_with("/xsts/authorize") {
            let Payload::Json(body) = payload else {
                return Err(Error::Protocol);
            };
            let body: serde_json::Value =
                serde_json::from_slice(body).map_err(|_| Error::Protocol)?;
            account = body["Properties"]["UserTokens"][0]
                .as_str()
                .and_then(fixture_id)
                .ok_or(Error::Protocol)?;
            (
                200,
                json!({
                    "Token": format!("fake-xsts-{account}"),
                    "DisplayClaims": { "xui": [{ "uhs": "12" }] }
                }),
            )
        } else if url.ends_with("/login_with_xbox") {
            let Payload::Json(body) = payload else {
                return Err(Error::Protocol);
            };
            let body: serde_json::Value =
                serde_json::from_slice(body).map_err(|_| Error::Protocol)?;
            account = body["identityToken"]
                .as_str()
                .and_then(fixture_id)
                .ok_or(Error::Protocol)?;
            (
                200,
                json!({ "access_token": format!("fake-minecraft-{account}"), "expires_in": 3600 }),
            )
        } else if url.ends_with("/entitlements/mcstore") {
            (200, json!({ "items": [{ "name": "game_minecraft" }] }))
        } else if url.ends_with("/minecraft/profile") {
            let Payload::Bearer(token) = payload else {
                return Err(Error::Protocol);
            };
            account = fixture_id(token).ok_or(Error::Protocol)?;
            (
                200,
                json!({ "id": format!("{account:032x}"), "name": format!("TestPlayer{account}") }),
            )
        } else {
            return Err(Error::Protocol);
        };
        Ok((status, Secret::new(body.to_string().into_bytes())))
    }
}

fn fixture_id(value: &str) -> Option<usize> {
    value.rsplit('-').next()?.parse().ok()
}
