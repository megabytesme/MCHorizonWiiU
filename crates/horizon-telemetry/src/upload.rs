//! Sends the outbox, from the launcher at start and exit. Before anything else it reads the
//! consent again and deletes what the player no longer allows, so with everything off it
//! returns without opening a connection. Failures back off (a minute, doubling, at most a day)
//! and at most [`MAX_BATCHES_A_DAY`] batches leave a day.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::batch::{self, Batch};
use crate::{Consent, outbox};

pub const MAX_BATCHES_A_DAY: u32 = 8;
const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, PartialEq, Eq)]
pub enum Flush {
    /// Nothing to send, or nothing allowed: no connection was made.
    Nothing,
    /// Waiting after a failure, or for tomorrow's allowance.
    Later,
    Sent {
        items: usize,
        batches: usize,
    },
    /// The server could not be reached or failed; tried again later.
    Failed(String),
}

/// The uploader's own state, beside the consent.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct State {
    day: u32,
    batches: u32,
    failures: u32,
    retry_at: u64,
    deliveries: BTreeMap<String, Delivery>,
}

/// Which destinations have resolved one stable outbox item. Tracking per item keeps acknowledgments
/// valid if a new item changes the batch boundaries on a later run.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Delivery {
    resolved: BTreeSet<String>,
    accepted: bool,
}

fn state_path(home: &Path) -> PathBuf {
    crate::dir(home).join("upload.json")
}

fn load(home: &Path) -> State {
    std::fs::read(state_path(home))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save(home: &Path, state: &State) {
    if let Ok(json) = serde_json::to_vec(state) {
        let _ = crate::write_atomic(&state_path(home), &json);
    }
}

fn agent() -> ureq::Agent {
    let builder = ureq::AgentBuilder::new();
    #[cfg(target_os = "cafe")]
    let builder = builder.timeout_connect(TIMEOUT).timeout_read(TIMEOUT);
    #[cfg(not(target_os = "cafe"))]
    let builder = builder.timeout(TIMEOUT);
    builder.build()
}

/// Sends what the player allows to `endpoint` (`https://host`, without a trailing slash).
pub fn flush(home: &Path, endpoint: &str) -> Flush {
    flush_endpoints(home, &[endpoint.to_owned()])
}

/// Sends what the player allows to every endpoint in the same reporting cycle.
/// A successful destination is persisted per outbox item, so a later destination failure does not
/// cause duplicates at a server that already accepted the item.
pub fn flush_endpoints(home: &Path, endpoints: &[String]) -> Flush {
    if endpoints.is_empty() {
        return Flush::Nothing;
    }
    let consent = Consent::load(home);
    // What was turned off since it was kept never leaves.
    let allowed = outbox::retain_allowed(home);
    let live_items: BTreeSet<_> = allowed.iter().map(|(path, _)| item_key(path)).collect();
    let mut state = load(home);
    state.deliveries.retain(|item, _| live_items.contains(item));
    if allowed.is_empty() {
        save(home, &state);
        return Flush::Nothing;
    }
    let today = crate::today();
    if state.day != today {
        state.day = today;
        state.batches = 0;
    }
    if crate::now_secs() < state.retry_at || state.batches >= MAX_BATCHES_A_DAY {
        return Flush::Later;
    }
    let agent = agent();
    let (mut items, mut batches) = (0, 0);
    for group in batch::split(allowed) {
        if state.batches >= MAX_BATCHES_A_DAY {
            break;
        }
        let paths: Vec<_> = group.iter().map(|(path, _)| path.clone()).collect();
        let keys: Vec<_> = paths.iter().map(|path| item_key(path)).collect();
        let mut errors = Vec::new();
        for endpoint in endpoints {
            let pending: Vec<_> = group
                .iter()
                .filter(|(path, _)| {
                    !state
                        .deliveries
                        .get(&item_key(path))
                        .is_some_and(|delivery| delivery.resolved.contains(endpoint))
                })
                .collect();
            if pending.is_empty() {
                continue;
            }
            let pending_keys: Vec<_> = pending.iter().map(|(path, _)| item_key(path)).collect();
            let list = pending.iter().map(|(_, item)| (*item).clone()).collect();
            let body = batch::encode(&Batch::new(&consent.install_id, list));
            let url = format!("{endpoint}/v1/batch");
            let sent = agent
                .post(&url)
                .set("Content-Type", "application/json")
                .set("Content-Encoding", "zstd")
                .send_bytes(&body);
            match sent {
                Ok(_) => {
                    mark_resolved(&mut state, &pending_keys, endpoint, true);
                    save(home, &state);
                }
                // Refused for what it is (not for load): sending it again would not help.
                Err(ureq::Error::Status(code, _)) if (400..500).contains(&code) && code != 429 => {
                    mark_resolved(&mut state, &pending_keys, endpoint, false);
                    save(home, &state);
                }
                Err(error) => {
                    errors.push(format!("{endpoint}: {error}"));
                }
            }
        }
        if !errors.is_empty() {
            state.failures = (state.failures + 1).min(16);
            state.retry_at = crate::now_secs() + (60u64 << (state.failures - 1)).min(86_400);
            save(home, &state);
            return Flush::Failed(errors.join("; "));
        }
        let complete = keys.iter().all(|key| {
            state.deliveries.get(key).is_some_and(|delivery| {
                endpoints
                    .iter()
                    .all(|endpoint| delivery.resolved.contains(endpoint))
            })
        });
        if complete {
            let accepted_items = keys
                .iter()
                .filter(|key| {
                    state
                        .deliveries
                        .get(*key)
                        .is_some_and(|delivery| delivery.accepted)
                })
                .count();
            state.batches += 1;
            state.failures = 0;
            state.retry_at = 0;
            if accepted_items > 0 {
                items += accepted_items;
                batches += 1;
            }
            paths
                .iter()
                .for_each(|path| drop(std::fs::remove_file(path)));
            keys.iter().for_each(|key| {
                state.deliveries.remove(key);
            });
            save(home, &state);
        }
    }
    save(home, &state);
    if batches == 0 {
        Flush::Later
    } else {
        Flush::Sent { items, batches }
    }
}

fn item_key(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn mark_resolved(state: &mut State, keys: &[String], endpoint: &str, accepted: bool) {
    for key in keys {
        let delivery = state.deliveries.entry(key.clone()).or_default();
        delivery.resolved.insert(endpoint.to_owned());
        delivery.accepted |= accepted;
    }
}

/// Asks the server to delete everything sent under `install_id`.
pub fn delete_remote(endpoint: &str, install_id: &str) -> Result<(), String> {
    if !crate::consent::is_install_id(install_id) {
        return Err("no install ID: nothing was sent".into());
    }
    agent()
        .delete(&format!("{endpoint}/v1/installs/{install_id}"))
        .call()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Asks every endpoint to delete everything sent under `install_id`.
pub fn delete_remotes(endpoints: &[String], install_id: &str) -> Result<(), String> {
    let mut errors = Vec::new();
    for endpoint in endpoints {
        if let Err(error) = delete_remote(endpoint, install_id) {
            errors.push(format!("{endpoint}: {error}"));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

/// The batch the next [`flush`] would send, as readable JSON ("View what would be sent").
pub fn preview(home: &Path) -> String {
    let consent = Consent::load(home);
    let items = outbox::retain_allowed(home)
        .into_iter()
        .map(|(_, item)| item)
        .collect();
    serde_json::to_string_pretty(&Batch::new(&consent.install_id, items)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::payload::{Crash, Payload, Source};
    use crate::{Choices, Item, outbox, test_home};

    use super::*;

    fn mock_server(statuses: Vec<u16>) -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(AtomicUsize::new(0));
        let count = requests.clone();
        std::thread::spawn(move || {
            for status in statuses {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request = String::new();
                reader.read_line(&mut request).unwrap();
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line.trim().is_empty() {
                        break;
                    }
                    if let Some((key, value)) = line.split_once(':')
                        && key.eq_ignore_ascii_case("content-length")
                    {
                        length = value.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                count.fetch_add(1, Ordering::SeqCst);
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} Test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
            }
        });
        (endpoint, requests)
    }

    #[test]
    fn mirrored_upload_does_not_resend_to_an_endpoint_that_already_accepted() {
        let (primary, primary_requests) = mock_server(vec![204]);
        let (secondary, secondary_requests) = mock_server(vec![503, 204]);
        let home = test_home::new("upload-mirror");
        let mut consent = Consent::load(&home);
        consent.answer(Choices::ALL);
        consent.save(&home).unwrap();
        let crash = Item::new(Source::Game, "v1", Payload::Crash(Crash::default()));
        assert!(outbox::push(&home, &crash).unwrap());
        let endpoints = vec![primary, secondary];

        assert!(matches!(
            flush_endpoints(&home, &endpoints),
            Flush::Failed(_)
        ));
        assert_eq!(primary_requests.load(Ordering::SeqCst), 1);
        assert_eq!(secondary_requests.load(Ordering::SeqCst), 1);
        assert_eq!(outbox::pending(&home).len(), 1);

        let mut state = load(&home);
        state.retry_at = 0;
        save(&home, &state);
        assert_eq!(
            flush_endpoints(&home, &endpoints),
            Flush::Sent {
                items: 1,
                batches: 1
            }
        );
        assert_eq!(
            primary_requests.load(Ordering::SeqCst),
            1,
            "the primary already accepted the item and must not see a duplicate"
        );
        assert_eq!(secondary_requests.load(Ordering::SeqCst), 2);
        assert!(outbox::pending(&home).is_empty());
    }

    #[test]
    fn mirrored_upload_tries_secondary_after_primary_failure() {
        let (primary, primary_requests) = mock_server(vec![503, 204]);
        let (secondary, secondary_requests) = mock_server(vec![204]);
        let home = test_home::new("upload-mirror-primary-fails");
        let mut consent = Consent::load(&home);
        consent.answer(Choices::ALL);
        consent.save(&home).unwrap();
        let crash = Item::new(Source::Game, "v1", Payload::Crash(Crash::default()));
        assert!(outbox::push(&home, &crash).unwrap());
        let endpoints = vec![primary, secondary];

        assert!(matches!(
            flush_endpoints(&home, &endpoints),
            Flush::Failed(_)
        ));
        assert_eq!(primary_requests.load(Ordering::SeqCst), 1);
        assert_eq!(
            secondary_requests.load(Ordering::SeqCst),
            1,
            "the secondary must still receive the batch when the primary fails"
        );

        let mut state = load(&home);
        state.retry_at = 0;
        save(&home, &state);
        assert_eq!(
            flush_endpoints(&home, &endpoints),
            Flush::Sent {
                items: 1,
                batches: 1
            }
        );
        assert_eq!(primary_requests.load(Ordering::SeqCst), 2);
        assert_eq!(
            secondary_requests.load(Ordering::SeqCst),
            1,
            "the accepted secondary must not receive a duplicate on retry"
        );
        assert!(outbox::pending(&home).is_empty());
    }
}
