use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::time::{Duration, Instant};

use horizon_wiiu_security::Secret;

use crate::protocol::{self, Authorized, Error, Https, Poll, Transport};

pub struct DevicePrompt {
    verification_uri: String,
    code: Secret,
    expires: Instant,
}

impl DevicePrompt {
    pub fn instructions(&self) -> Option<(&str, &str)> {
        let code = std::str::from_utf8(self.code.expose()).ok()?;
        (Instant::now() < self.expires).then_some((&self.verification_uri, code))
    }
}

pub struct Verified(pub(crate) Authorized);

impl Verified {
    pub fn identity(&self) -> &crate::Identity {
        &self.0.identity
    }
}

pub enum Event {
    Prompt(DevicePrompt),
    Verifying,
    Verified(Verified),
    Failed(Error),
}

#[derive(Clone)]
pub struct Provider {
    client: Option<&'static str>,
    transport: Arc<dyn Transport>,
}

impl Provider {
    pub fn new(client: Option<&'static str>) -> Self {
        Self {
            client: client.filter(|client| !client.is_empty()),
            transport: Arc::new(Https::new()),
        }
    }

    pub fn configured(&self) -> bool {
        self.client.is_some()
    }

    pub fn for_night(client: Option<&'static str>, fixture: bool) -> Self {
        if fixture {
            #[cfg(feature = "fixture")]
            return Self::mocked(crate::fixture::Fixture::default());
            #[cfg(not(feature = "fixture"))]
            return Self::new(None);
        }
        Self::new(client)
    }

    #[cfg(any(test, feature = "fixture"))]
    pub(crate) fn mocked(transport: impl Transport + 'static) -> Self {
        Self {
            client: Some("test-only-public-application"),
            transport: Arc::new(transport),
        }
    }

    pub fn start(&self) -> Result<Work, Error> {
        let client = self.client.ok_or(Error::Unconfigured)?;
        let transport = self.transport.clone();
        Work::spawn(move |events, stop| {
            let device = protocol::begin(&*transport, client)?;
            let mut interval = device.interval;
            if !events.post(Event::Prompt(DevicePrompt {
                verification_uri: device.verification_uri.clone(),
                code: Secret::new(device.user_code.expose().to_vec()),
                expires: device.expires,
            })) {
                return Err(Error::Denied);
            }
            loop {
                let remaining = device.expires.saturating_duration_since(Instant::now());
                if stop.recv_timeout(interval.min(remaining))
                    != Err(mpsc::RecvTimeoutError::Timeout)
                {
                    return Err(Error::Denied);
                }
                if Instant::now() >= device.expires || events.cancelled() {
                    return Err(Error::CodeExpired);
                }
                match protocol::poll(&*transport, client, &device) {
                    Ok(Poll::Pending) => {}
                    Ok(Poll::SlowDown) => interval += Duration::from_secs(5),
                    Ok(Poll::Completed(tokens)) => {
                        if !events.post(Event::Verifying) {
                            return Err(Error::Denied);
                        }
                        drop(device);
                        return protocol::authorize(&*transport, tokens, || events.cancelled());
                    }
                    Err(Error::Network) => {
                        interval = (interval * 2).min(Duration::from_secs(120));
                    }
                    Err(error) => return Err(error),
                }
            }
        })
    }

    pub fn renew(&self, credential: Secret) -> Result<Work, Error> {
        let client = self.client.ok_or(Error::Unconfigured)?;
        let transport = self.transport.clone();
        Work::spawn(move |events, _| {
            if events.cancelled() {
                return Err(Error::Denied);
            }
            let tokens = protocol::refresh(&*transport, client, credential)?;
            protocol::authorize(&*transport, tokens, || events.cancelled())
        })
    }
}

struct Events {
    sender: mpsc::Sender<Event>,
    cancelled: Arc<AtomicBool>,
}

impl Events {
    fn post(&self, event: Event) -> bool {
        !self.cancelled() && self.sender.send(event).is_ok()
    }

    fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

/// Dropping work cancels further polling and discards any queued result.
/// In-progress HTTPS calls drain under their timeout without blocking Night.
pub struct Work {
    events: mpsc::Receiver<Event>,
    stop: mpsc::Sender<()>,
    cancelled: Arc<AtomicBool>,
}

impl Work {
    fn spawn(
        task: impl FnOnce(&Events, mpsc::Receiver<()>) -> Result<Authorized, Error> + Send + 'static,
    ) -> Result<Self, Error> {
        let (event_sender, events) = mpsc::channel();
        let (stop, stop_receiver) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let outgoing = Events {
            sender: event_sender,
            cancelled: cancelled.clone(),
        };
        std::thread::Builder::new()
            .name("Night accounts".into())
            .stack_size(512 * 1024)
            .spawn(move || {
                let result = task(&outgoing, stop_receiver);
                outgoing.post(match result {
                    Ok(result) => Event::Verified(Verified(result)),
                    Err(error) => Event::Failed(error),
                });
            })
            .map_err(|_| Error::Network)?;
        Ok(Self {
            events,
            stop,
            cancelled,
        })
    }

    pub fn poll(&self) -> Option<Event> {
        match self.events.try_recv() {
            Ok(event) => Some(event),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Event::Failed(Error::Network)),
        }
    }
}

impl Drop for Work {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        let _ = self.stop.send(());
    }
}
