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

impl Backend for TestTools {
    fn list(&self, _: &Principal) -> Vec<Value> {
        vec![
            json!({"name": "versions", "description": "List fixture versions",
            "inputSchema": {"type":"object", "properties": {}}}),
        ]
    }
    fn required(&self, name: &str) -> Option<Scopes> {
        (name == "versions").then_some(Scopes::LAUNCHER_READ)
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
