use crate::oauth::Principal;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

const MAX_PENDING: usize = 4;
const DEADLINE: Duration = Duration::from_secs(12);
const DISCONNECT_POLL: Duration = Duration::from_millis(30);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    NoGame,
    Busy,
    Cancelled,
    Disabled,
}

#[derive(Clone, Default)]
pub struct Session(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    generation: u64,
    live: bool,
    controllable: bool,
    requests: VecDeque<Queued>,
    active: Option<Guard>,
}

#[derive(Clone)]
struct Guard {
    principal: Principal,
    cancelled: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
    deadline: Instant,
}

impl Guard {
    fn alive(&self) -> bool {
        !self.cancelled.load(Ordering::Acquire)
            && self.deadline > Instant::now()
            && self.principal.is_active()
    }
}

struct Queued {
    generation: u64,
    guard: Guard,
    request: Request,
}

pub struct Request {
    pub line: String,
    pub reply: mpsc::Sender<String>,
}

/// Scoped to one application-owned launch, including loading and save.
pub struct Registration {
    session: Session,
    generation: u64,
}

impl Session {
    pub fn prepare(&self, controllable: bool) -> Registration {
        let mut state = self.lock();
        state.generation = state.generation.wrapping_add(1);
        state.live = false;
        state.controllable = controllable;
        state.requests.clear();
        state.active = None;
        Registration {
            session: self.clone(),
            generation: state.generation,
        }
    }

    /// Called by the game thread. A cancelled deferred request is withdrawn
    /// before another action starts, so it cannot outlive its client.
    pub fn poll(&self, ready: bool) -> (Option<Request>, bool) {
        let mut state = self.lock();
        let was_live = state.live;
        state.live = ready && state.controllable;
        if !state.live {
            state.requests.clear();
            return (None, state.active.take().is_some() || was_live);
        }
        let mut cancelled = false;
        if let Some(active) = state.active.as_ref() {
            if active.finished.load(Ordering::Acquire) {
                state.active = None;
            } else if !active.alive() {
                state.active = None;
                cancelled = true;
            } else {
                return (None, false);
            }
        }
        while let Some(item) = state.requests.pop_front() {
            if item.generation == state.generation && item.guard.alive() {
                state.active = Some(item.guard);
                return (Some(item.request), cancelled);
            }
        }
        (None, cancelled)
    }

    pub fn live(&self) -> bool {
        self.lock().live
    }

    pub fn withdraw(&self) {
        let mut state = self.lock();
        state.live = false;
        state.requests.clear();
        if let Some(active) = state.active.take() {
            active.cancelled.store(true, Ordering::Release);
        }
    }

    pub fn call(
        &self,
        principal: &Principal,
        line: String,
        disconnected: &dyn Fn() -> bool,
    ) -> Result<String, Error> {
        if disconnected() {
            return Err(Error::Cancelled);
        }
        let (reply, result) = mpsc::channel();
        let guard = Guard {
            principal: principal.clone(),
            finished: Arc::default(),
            cancelled: Arc::default(),
            deadline: Instant::now() + DEADLINE,
        };
        {
            let mut state = self.lock();
            if !state.live {
                return Err(Error::NoGame);
            }
            if !principal.is_active() {
                return Err(Error::Disabled);
            }
            if state.requests.len() >= MAX_PENDING {
                return Err(Error::Busy);
            }
            let generation = state.generation;
            state.requests.push_back(Queued {
                generation,
                guard: guard.clone(),
                request: Request { line, reply },
            });
        }
        while guard.alive() && !disconnected() {
            match result.recv_timeout(DISCONNECT_POLL) {
                Ok(value) => {
                    guard.finished.store(true, Ordering::Release);
                    return Ok(value);
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    guard.finished.store(true, Ordering::Release);
                    return Err(Error::NoGame);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
        guard.cancelled.store(true, Ordering::Release);
        Err(Error::Cancelled)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(|error| error.into_inner())
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        let mut state = self.session.lock();
        if state.generation == self.generation {
            state.requests.clear();
            state.live = false;
            state.controllable = false;
            if let Some(active) = state.active.take() {
                active.cancelled.store(true, Ordering::Release);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_requests_and_old_generations_are_withdrawn() {
        let game = Session::default();
        let principal = Principal::fixture();
        assert_eq!(
            game.call(&principal, "status".into(), &|| false),
            Err(Error::NoGame)
        );
        let old = game.prepare(true);
        assert!(game.poll(false).0.is_none());
        game.poll(true);
        let client = game.clone();
        let connected = Arc::new(AtomicBool::new(true));
        let peer = connected.clone();
        let controller = std::thread::spawn(move || {
            client.call(&principal, "move forward 8".into(), &|| {
                !peer.load(Ordering::Acquire)
            })
        });
        let limit = Instant::now() + Duration::from_secs(2);
        let request = loop {
            if let Some(req) = game.poll(true).0 {
                break req;
            }
            assert!(Instant::now() < limit);
            std::thread::yield_now();
        };
        assert_eq!(request.line, "move forward 8");
        connected.store(false, Ordering::Release);
        assert_eq!(controller.join().unwrap(), Err(Error::Cancelled));
        assert!(game.poll(true).1);
        let new = game.prepare(false);
        drop(old);
        assert!(game.poll(true).0.is_none());
        assert!(!game.live());
        drop(new);
    }
}
