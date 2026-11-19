//! Cafe fixture backend for the hosted session contract.
use core::ffi::c_void;
use core::ptr::NonNull;

use crate::accounts::Profile;
use crate::cafe::trace;
use crate::config::{Instance, LaunchConfig, Resources, WorldOrServer};
use crate::session::{
    ExitWork, LiveEvent, PreparePoll, SaveError, SavePoll, SessionDriver, SessionFactory,
    StartError, StartMode,
};

const PREPARE_POLLS: u8 = 90;
const SAVE_POLLS: u8 = 12;
const STOP_POLLS: u8 = 6;

extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
}

pub struct MockFactory;

pub struct MockDriver {
    scratch: NonNull<c_void>,
    mode: StartMode,
    prepare: u8,
    save: u8,
    stop: u8,
}

impl SessionFactory for MockFactory {
    type Driver = MockDriver;

    fn start(
        &mut self,
        config: LaunchConfig<'_>,
        mode: StartMode,
    ) -> Result<MockDriver, StartError> {
        if !matches!(
            config,
            LaunchConfig {
                profile: Profile::Offline(_),
                instance: Instance {
                    target: WorldOrServer::Demo,
                    resources: Resources::BuiltInDemo,
                    ..
                },
                ..
            }
        ) {
            return Err(StartError::Unavailable);
        }
        let scratch = NonNull::new(unsafe { malloc(64 * 1024) }).ok_or(StartError::OutOfMemory)?;
        unsafe { scratch.as_ptr().cast::<u8>().write_volatile(1) };
        trace(c"session.allocated");
        Ok(MockDriver {
            scratch,
            mode,
            prepare: PREPARE_POLLS,
            save: 0,
            stop: 0,
        })
    }
}

impl SessionDriver for MockDriver {
    fn poll_prepare(&mut self) -> PreparePoll {
        self.prepare = self.prepare.saturating_sub(1);
        if self.prepare != 0 {
            return PreparePoll::Pending {
                percent: Some(
                    (u16::from(PREPARE_POLLS - self.prepare) * 100 / u16::from(PREPARE_POLLS))
                        as u8,
                ),
            };
        }
        if self.mode == StartMode::TestError {
            PreparePoll::Failed(StartError::Fixture)
        } else {
            trace(c"session.ready");
            PreparePoll::Ready
        }
    }

    fn poll_live(&mut self) -> LiveEvent {
        LiveEvent::Continue
    }

    fn begin_save(&mut self) {
        self.save = SAVE_POLLS;
        trace(c"session.save_requested");
    }

    fn poll_save(&mut self) -> SavePoll {
        self.save = self.save.saturating_sub(1);
        if self.save != 0 {
            return SavePoll::Pending;
        }
        trace(c"session.saved");
        SavePoll::Complete
    }

    fn begin_stop(&mut self) {
        self.stop = STOP_POLLS;
        trace(c"session.stop_requested");
    }

    fn poll_stop(&mut self) -> bool {
        self.stop = self.stop.saturating_sub(1);
        if self.stop != 0 {
            return false;
        }
        trace(c"session.quiescent");
        true
    }

    fn foreground(&mut self, _focused: bool) {}

    fn shutdown_for_exit(&mut self, work: ExitWork) -> Result<(), SaveError> {
        if work == ExitWork::SaveActive {
            self.begin_save();
        }
        let result = if matches!(work, ExitWork::SaveActive | ExitWork::FinishSave) {
            loop {
                match self.poll_save() {
                    SavePoll::Pending => {}
                    SavePoll::Complete => break Ok(()),
                    SavePoll::Failed(error) => break Err(error),
                }
            }
        } else {
            Ok(())
        };
        if work != ExitWork::FinishStop {
            self.begin_stop();
        }
        while !self.poll_stop() {}
        result
    }
}

impl Drop for MockDriver {
    fn drop(&mut self) {
        unsafe { free(self.scratch.as_ptr()) };
        trace(c"session.released");
    }
}
