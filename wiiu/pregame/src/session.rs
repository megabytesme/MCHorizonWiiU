//! Hosted engine lifecycle. The driver is owned by one launcher session and
//! is only polled on the host thread. World operations can use owned workers.

use crate::config::LaunchConfig;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartError {
    Unavailable,
    OutOfMemory,
    Fixture,
    Resources,
    Engine,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveError {
    Write,
    Disconnect,
    Shutdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartMode {
    Normal,
    TestError,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparePoll {
    Pending { percent: Option<u8> },
    Ready,
    Failed(StartError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SavePoll {
    Pending,
    Complete,
    Failed(SaveError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveEvent {
    Continue,
    QuitRequested,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitWork {
    AbortPreparation,
    SaveActive,
    FinishSave,
    FinishStop,
}

pub trait SessionFactory {
    type Driver: SessionDriver;

    /// Begin preparation. Keep any partially acquired resources in the driver
    /// to support cancellation. An immediate error means nothing remains live.
    /// Copy resolved launch inputs needed after this call to backend-owned state.
    fn start(
        &mut self,
        config: LaunchConfig<'_>,
        mode: StartMode,
    ) -> Result<Self::Driver, StartError>;
}

pub trait SessionDriver {
    fn poll_prepare(&mut self) -> PreparePoll;

    /// Report a quit from the game's own UI. The adapter then starts the same
    /// save/disconnect barrier used by a host request.
    fn poll_live(&mut self) -> LiveEvent;

    /// Begin one save attempt. Pause gameplay and finish outstanding writes.
    fn begin_save(&mut self);

    /// Complete means all local writes have succeeded or the remote connection
    /// has closed cleanly. A failed attempt must allow a retry.
    fn poll_save(&mut self) -> SavePoll;

    /// Cancel or stop workers, input, audio and networking. This may leave
    /// cleanup to poll_stop. The adapter calls it once per stopping phase.
    fn begin_stop(&mut self);

    /// True only when all session resources are safe to drop and no callbacks
    /// or workers can still reach them. Release real world locks by Drop.
    fn poll_stop(&mut self) -> bool;

    /// Release foreground-only resources before acknowledging the OS release.
    /// Rebind against current host targets after the acquire message.
    fn foreground(&mut self, focused: bool);

    /// Terminal OS exit barrier. Settle the indicated work and drain resources
    /// even on error. This call must not pump ProcUI or present frames. The
    /// launcher reports failures and drops the driver before ending graphics.
    fn shutdown_for_exit(&mut self, work: ExitWork) -> Result<(), SaveError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReturnTo {
    Cancelled,
    StartError(StartError),
    Saved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionStatus {
    Preparing(Option<u8>),
    Live,
    Saving,
    SaveFailed(SaveError),
    Stopping(ReturnTo),
    Released(ReturnTo),
}

pub struct HostedSession<D> {
    driver: D,
    status: SessionStatus,
}

impl<D: SessionDriver> HostedSession<D> {
    pub fn new(driver: D) -> Self {
        Self {
            driver,
            status: SessionStatus::Preparing(None),
        }
    }

    pub fn status(&self) -> SessionStatus {
        self.status
    }

    pub fn live_driver(&mut self) -> Option<&mut D> {
        (self.status == SessionStatus::Live).then_some(&mut self.driver)
    }

    pub fn foreground(&mut self, focused: bool) {
        self.driver.foreground(focused);
    }

    pub fn cancel(&mut self) {
        if matches!(self.status, SessionStatus::Preparing(_)) {
            self.stop(ReturnTo::Cancelled);
        }
    }

    pub fn save_and_quit(&mut self) {
        if matches!(
            self.status,
            SessionStatus::Live | SessionStatus::SaveFailed(_)
        ) {
            self.driver.begin_save();
            self.status = SessionStatus::Saving;
        }
    }

    fn stop(&mut self, outcome: ReturnTo) {
        self.driver.begin_stop();
        self.status = SessionStatus::Stopping(outcome);
    }

    pub fn advance(&mut self) {
        match self.status {
            SessionStatus::Preparing(_) => match self.driver.poll_prepare() {
                PreparePoll::Pending { percent } => {
                    self.status = SessionStatus::Preparing(percent.map(|value| value.min(100)));
                }
                PreparePoll::Ready => self.status = SessionStatus::Live,
                PreparePoll::Failed(error) => self.stop(ReturnTo::StartError(error)),
            },
            SessionStatus::Live => {
                if self.driver.poll_live() == LiveEvent::QuitRequested {
                    self.save_and_quit();
                }
            }
            SessionStatus::Saving => match self.driver.poll_save() {
                SavePoll::Pending => {}
                SavePoll::Complete => self.stop(ReturnTo::Saved),
                SavePoll::Failed(error) => self.status = SessionStatus::SaveFailed(error),
            },
            SessionStatus::Stopping(outcome) => {
                if self.driver.poll_stop() {
                    self.status = SessionStatus::Released(outcome);
                }
            }
            SessionStatus::SaveFailed(_) | SessionStatus::Released(_) => {}
        }
    }

    /// Consumes this session so even an exit error is followed by Drop.
    pub fn shutdown_for_exit(mut self) -> Result<(), SaveError> {
        let work = match self.status {
            SessionStatus::Preparing(_) => ExitWork::AbortPreparation,
            SessionStatus::Live | SessionStatus::SaveFailed(_) => ExitWork::SaveActive,
            SessionStatus::Saving => ExitWork::FinishSave,
            SessionStatus::Stopping(_) => ExitWork::FinishStop,
            SessionStatus::Released(_) => return Ok(()),
        };
        self.driver.shutdown_for_exit(work)
    }
}
