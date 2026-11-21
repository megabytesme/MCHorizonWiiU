use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use horizon_wiiu_security::{Protection, Secret, StorageMode};

use crate::fixture::Fixture;
use crate::protocol::{self, Payload, Poll, Transport};
use crate::{Accounts, Error, Event, ProfileId, ProfileState, Provider, Verified, Work};

const CLIENT: &str = "fake-registered-app";
const PIN: &[u8] = b"a test PIN passphrase";

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "night-microsoft-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn ms(transport: &dyn Transport) -> protocol::MicrosoftTokens {
    let code = protocol::begin(transport, CLIENT).unwrap();
    match protocol::poll(transport, CLIENT, &code).unwrap() {
        Poll::Completed(tokens) => tokens,
        _ => panic!("the synthetic device code did not authorize"),
    }
}

fn verified(transport: &dyn Transport) -> Verified {
    Verified(protocol::authorize(transport, ms(transport), || false).unwrap())
}

fn result(work: Work) -> Result<Verified, Error> {
    let limit = Instant::now() + Duration::from_secs(4);
    loop {
        match work.poll() {
            Some(Event::Verified(verified)) => return Ok(verified),
            Some(Event::Failed(error)) => return Err(error),
            _ => {}
        }
        assert!(Instant::now() < limit, "the account worker did not finish");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn refresh_replaces_token_and_reopens_each_protection_mode() {
    let storage = Scratch::new();
    let fixture = Fixture::default();
    let mut accounts = Accounts::open(&storage.0).unwrap();
    accounts
        .accept(
            verified(&fixture),
            None,
            Some(Protection::PinProtected { new_pin: PIN }),
        )
        .unwrap();
    accounts
        .accept(
            verified(&fixture),
            None,
            Some(Protection::ConfirmedUnprotected),
        )
        .unwrap();
    assert_eq!(accounts.selected(), Some(ProfileId(2)));
    drop(accounts);

    let mut accounts = Accounts::open(&storage.0).unwrap();
    assert_eq!(accounts.profiles().count(), 2);
    assert_eq!(accounts.state(ProfileId(1)), ProfileState::Locked);
    assert_eq!(
        accounts.state(ProfileId(2)),
        ProfileState::Saved(StorageMode::Unprotected)
    );
    assert!(accounts.active().is_none());
    assert_eq!(accounts.selected(), Some(ProfileId(2)));
    let provider = Provider::mocked(MapFixture::new(Behavior::Rotate));
    let renewed = result(accounts.refresh(ProfileId(2), &provider).unwrap()).unwrap();
    accounts.accept(renewed, Some(ProfileId(2)), None).unwrap();
    assert_eq!(
        accounts.active().unwrap().access_token(),
        "fake-minecraft-2"
    );
    let observing = Provider::mocked(MapFixture::new(Behavior::RequireRotated));
    assert!(result(accounts.refresh(ProfileId(2), &observing).unwrap()).is_ok());

    accounts.unlock(ProfileId(1), PIN).unwrap();
    let provider = Provider::mocked(Fixture::default());
    let renewed = result(accounts.refresh(ProfileId(1), &provider).unwrap()).unwrap();
    accounts.accept(renewed, Some(ProfileId(1)), None).unwrap();
    assert_eq!(accounts.active().unwrap().name, "TestPlayer1");
    accounts.sign_out(ProfileId(2)).unwrap();
    assert!(accounts.profile(ProfileId(2)).is_none());
    let mut accounts = Accounts::open(&storage.0).unwrap();
    assert_eq!(accounts.selected(), Some(ProfileId(1)));
    assert_eq!(accounts.profiles().count(), 1);
    assert_eq!(
        accounts.refresh(ProfileId(2), &provider).err(),
        Some(Error::Reauthenticate)
    );
}

#[test]
fn revoked_refresh_and_denied_ownership_cannot_produce_an_identity() {
    let storage = Scratch::new();
    let mut accounts = Accounts::open(&storage.0).unwrap();
    let fixture = Fixture::default();
    accounts
        .accept(verified(&fixture), None, Some(Protection::SessionOnly))
        .unwrap();
    let revoked = Provider::mocked(MapFixture::new(Behavior::Revoke));
    let error = result(accounts.refresh(ProfileId(1), &revoked).unwrap())
        .err()
        .unwrap();
    assert_eq!(error, Error::Reauthenticate);
    accounts.record_failure(ProfileId(1), error);
    assert!(accounts.active().is_none());
    assert_eq!(accounts.state(ProfileId(1)), ProfileState::Reauthenticate);
    assert_eq!(fs::read_dir(&storage.0).unwrap().count(), 0);
    assert_eq!(Accounts::open(&storage.0).unwrap().profiles().count(), 0);

    let denial = MapFixture::new(Behavior::NoOwnership);
    assert_eq!(
        protocol::authorize(&denial, ms(&denial), || false).err(),
        Some(Error::OwnershipMissing)
    );
}

#[test]
fn expired_sessions_are_rejected_and_expired_device_codes_terminate() {
    let old = MapFixture::new(Behavior::ExpiringAccess);
    let authorized = verified(&old);
    assert!(!authorized
        .identity()
        .is_current_at(Instant::now() + Duration::from_secs(2)));
    let device = Provider::mocked(MapFixture::new(Behavior::ExpiredCode));
    assert_eq!(
        result(device.start().unwrap()).err(),
        Some(Error::CodeExpired)
    );
}

#[derive(Clone, Copy)]
enum Behavior {
    Rotate,
    RequireRotated,
    Revoke,
    NoOwnership,
    ExpiringAccess,
    ExpiredCode,
}

struct MapFixture {
    behavior: Behavior,
    transport: Fixture,
}

impl MapFixture {
    fn new(behavior: Behavior) -> Self {
        Self {
            behavior,
            transport: Fixture::default(),
        }
    }
}

fn body(status: u16, response: &str) -> Result<(u16, Secret), Error> {
    Ok((status, Secret::new(response.as_bytes().to_vec())))
}

impl Transport for MapFixture {
    fn send(&self, url: &str, payload: Payload<'_>) -> Result<(u16, Secret), Error> {
        match self.behavior {
            Behavior::Rotate if url.ends_with("/token") => {
                let Payload::Form(form) = payload else {
                    panic!("expected OAuth form")
                };
                assert!(form.contains(&("grant_type", "refresh_token")));
                assert!(form.contains(&("refresh_token", "fake-refresh-2")));
                return body(
                    200,
                    r#"{"refresh_token":"fake-rotated-2","access_token":"fake-microsoft-2"}"#,
                );
            }
            Behavior::RequireRotated if url.ends_with("/token") => {
                let Payload::Form(form) = payload else {
                    panic!("expected OAuth form")
                };
                assert!(form.contains(&("refresh_token", "fake-rotated-2")));
                return body(200, r#"{"access_token":"fake-microsoft-2"}"#);
            }
            Behavior::Revoke if url.ends_with("/token") => {
                return body(
                    400,
                    r#"{"error":"invalid_grant","error_description":"contains a credential placeholder"}"#,
                );
            }
            Behavior::NoOwnership if url.ends_with("/entitlements/mcstore") => {
                return body(200, r#"{"items":[{"name":"game_minecraft_bedrock"}]}"#);
            }
            Behavior::ExpiringAccess if url.ends_with("/login_with_xbox") => {
                return body(
                    200,
                    r#"{"access_token":"fake-minecraft-1","expires_in":61}"#,
                );
            }
            Behavior::ExpiredCode if url.ends_with("/token") => {
                return body(400, r#"{"error":"expired_token"}"#);
            }
            _ => {}
        }
        self.transport.send(url, payload)
    }
}
