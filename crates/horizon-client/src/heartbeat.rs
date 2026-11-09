//! The anonymous "I'm playing" ping (`horizon_telemetry::live`, docs/telemetry.md) behind the
//! launcher's "N playing now". While the game runs, a thread of its own sends a random session
//! ID made here and kept only in memory, the version, the platform and whether a LAN or
//! play-with-friends world is open, every minute and at once when that last one changes.
//! It sends nothing when sending is turned off (`HORIZON_TELEMETRY_URL=off`) or with
//! `HORIZON_NO_HEARTBEAT=1` (automated runs).
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::Thread;
use std::time::{Duration, Instant};

use horizon_telemetry::live::{self, Ping};

/// A world is open to LAN or to friends.
static LAN: AtomicBool = AtomicBool::new(false);
static PINGER: OnceLock<Thread> = OnceLock::new();

/// After a failure the next ping waits longer, doubling up to this.
const MAX_WAIT: Duration = Duration::from_secs(600);

/// Starts the pings, once. Call when the game itself starts (not a dedicated server, not a
/// benchmark or an automated run).
pub fn start() {
    if PINGER.get().is_some() || std::env::var_os("HORIZON_NO_HEARTBEAT").is_some() {
        return;
    }
    let Some(endpoints) = horizon_telemetry::endpoints() else {
        return;
    };
    let spawned = std::thread::Builder::new()
        .name("Heartbeat".into())
        .spawn(move || run(&endpoints));
    match spawned {
        Ok(handle) => {
            let _ = PINGER.set(handle.thread().clone());
        }
        Err(e) => log::warn!("Horizon: no live count ping: {e}"),
    }
}

/// Whether a world is open to LAN or friends; a change is sent at once.
pub fn set_lan(open: bool) {
    if LAN.swap(open, Ordering::Relaxed) != open
        && let Some(pinger) = PINGER.get()
    {
        pinger.unpark();
    }
}

fn run(endpoints: &[String]) {
    let session = live::session_id();
    let version = crate::telemetry::version();
    let mut retry_waits = vec![live::INTERVAL; endpoints.len()];
    let mut failed = vec![false; endpoints.len()];
    let mut next_pings = vec![Instant::now(); endpoints.len()];
    let mut last_lan = LAN.load(Ordering::Relaxed);
    loop {
        let lan = LAN.load(Ordering::Relaxed);
        let lan_changed = lan != last_lan;
        last_lan = lan;
        let now = Instant::now();
        for (index, endpoint) in endpoints.iter().enumerate() {
            if !lan_changed && now < next_pings[index] {
                continue;
            }
            let ping = Ping::new(&session, &version, lan);
            match live::send(endpoint, &ping) {
                Ok(()) => {
                    retry_waits[index] = live::INTERVAL;
                    failed[index] = false;
                    next_pings[index] = Instant::now() + live::INTERVAL;
                }
                Err(error) => {
                    // A quiet endpoint shouldn't slow down or suppress the other mirror.
                    if !failed[index] {
                        log::info!(
                            "Horizon: the live count ping failed for {endpoint} ({error}); trying again later"
                        );
                    }
                    failed[index] = true;
                    retry_waits[index] = (retry_waits[index] * 2).min(MAX_WAIT);
                    next_pings[index] = Instant::now() + retry_waits[index];
                }
            }
        }
        // A world-state change wakes all mirrors immediately; otherwise each keeps its own cadence.
        let now = Instant::now();
        let next = next_pings
            .iter()
            .min()
            .copied()
            .unwrap_or(now + live::INTERVAL);
        std::thread::park_timeout(next.saturating_duration_since(now));
    }
}
