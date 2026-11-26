//! Host-only interoperability fixture. It binds loopback, and local
//! approval is driven via stdin by the automated test operator.
use horizon_wiiu_mcp::{
    oauth::{Principal, Scopes},
    protocol::{text_result, Fault},
    router::Backend,
    service::{Policy, Service},
};
use horizon_wiiu_security::Protection;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::net::Ipv4Addr;
use std::path::Path;

#[derive(Clone)]
struct TestTools;

const TOOLS: &[(&str, &str, Scopes)] = &[
    ("versions", "List fixture versions", Scopes::LAUNCHER_READ),
    (
        "fixture_manage",
        "Call a launcher management fixture",
        Scopes::LAUNCHER_MANAGE,
    ),
    (
        "fixture_observe",
        "Call a gameplay observation fixture",
        Scopes::GAME_OBSERVE,
    ),
    (
        "fixture_control",
        "Call a gameplay control fixture",
        Scopes::GAME_CONTROL,
    ),
];

impl Backend for TestTools {
    fn list(&self, principal: &Principal) -> Vec<Value> {
        TOOLS
            .iter()
            .filter(|(_, _, scope)| principal.scopes.contains(*scope))
            .map(|(name, description, _)| {
                json!({
                    "name": name,
                    "description": description,
                    "inputSchema": {"type": "object", "properties": {}},
                })
            })
            .collect()
    }
    fn required(&self, name: &str) -> Option<Scopes> {
        TOOLS
            .iter()
            .find(|(tool, _, _)| *tool == name)
            .map(|(_, _, scope)| *scope)
    }
    fn call(
        &self,
        _: &Principal,
        _: &str,
        _: &Value,
        _: &dyn Fn() -> bool,
    ) -> Result<Value, Fault> {
        Ok(text_result("Horizon Wii U HTTPS fixture", false))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let profile = Path::new(&args[1]);
    let port = args[2].parse::<u16>()?;
    let mut service = Service::open(profile, TestTools)?;
    if !service.status().initialized {
        service.protect(Protection::ConfirmedUnprotected)?;
        service.set_policy(Policy {
            enabled: true,
            ..Policy::default()
        })?;
    }
    service.connect(Ipv4Addr::LOCALHOST, port, None, true)?;
    let (url, fingerprint) = service.connection().ok_or("fixture could not listen")?;
    println!("READY {url} {fingerprint}");
    io::stdout().flush()?;
    for command in io::stdin().lock().lines() {
        match command?.as_str() {
            "pair" => {
                service.local(|auth, now| auth.open_pairing(now));
            }
            "approve" => {
                service.local(|auth, now| {
                    let id = auth.pending(now).next().map(|consent| consent.id);
                    if let Some(id) = id {
                        auth.decide(now, &id, true).unwrap();
                    }
                });
            }
            "revoke" => {
                service.local(|auth, _| {
                    let id = auth.trusted().next().map(|client| client.client_id);
                    if let Some(id) = id {
                        auth.revoke_client(&id).unwrap();
                    }
                });
            }
            "expire-access" => {
                service.local(|auth, _| auth.revoke_access_tokens().unwrap());
            }
            "quit" => break,
            _ => {}
        }
        println!("OK");
        io::stdout().flush()?;
    }
    service.shutdown();
    Ok(())
}
