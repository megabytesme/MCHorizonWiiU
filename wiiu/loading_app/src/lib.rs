//! Cafe executable driver for Horizon's original Loading session.
//!
//! WUT owns the RPX entry, WHB process and GX2 lifetime in `main.c`; this
//! library only runs on that initialized GX2 thread.

use horizon_client::game::{
    CafeLoadingHost, CafeLoadingSession, CafeLoadingStatus, CafeRunOutcome,
};
use horizon_client_platform::{EventLoopControl, EventLoopCtl};
use horizon_wiiu_loading_host::Gx2LoadingPresenter;
use horizon_wiiu_platform::{touch::TouchMapper, VpadState};
use std::cell::Cell;
use std::time::{Duration, Instant};

unsafe extern "C" {
    fn horizon_wiiu_app_is_running() -> i32;
    fn horizon_wiiu_app_is_focused() -> i32;
    fn horizon_loading_host_dimensions(dimensions: *mut u32) -> i32;
}

/// Status returned to the WUT entry point: 0 finished, 1 title is ready but
/// cannot yet hand off to Game, 2 loading/presentation failed, 3 setup failed.
const FINISHED: i32 = 0;
const READY_WITHOUT_GAME_HANDOFF: i32 = 1;
const LOADING_FAILED: i32 = 2;
const PRESENTER_FAILED: i32 = 3;
const APP_ROOT_FAILED: i32 = 4;

const CAFE_APP_ROOT: &str = "/vol/external01/wiiu/apps/HorizonWiiU";

/// Called by C before WHB/GX2 initialization so all getrandom versions are
/// anchored and the Cafe system RNG is checked before platform startup.
#[no_mangle]
pub extern "C" fn horizon_wiiu_app_entropy_first() {
    horizon_wiiu_entropy::link();
}

/// Called by the WUT driver after WHB and GX2 initialization.
#[no_mangle]
pub extern "C" fn horizon_wiiu_app_run() -> i32 {
    // Cafe std's FS backend mounts the SD card at /vol/external01 when its
    // first filesystem operation initializes the FS client. Set a known app
    // working directory so Options' existing relative .local fallback and
    // game data paths resolve inside this app's isolated SD folder.
    if std::env::set_current_dir(CAFE_APP_ROOT).is_err() {
        return APP_ROOT_FAILED;
    }

    // Detailed startup spans identify UI and core resource stages. Cafe std
    // cannot create the optional trace file, so send them to OSConsole/serial.
    horizon_client::spans::startup::enable_log_sink();

    // SAFETY: C calls this only after WHBGfxInit and keeps GX2 alive until it
    // returns. The presenter is dropped on this same thread before C shuts GX2
    // down.
    let mut presenter = match unsafe { Gx2LoadingPresenter::new_embedded(16_384) } {
        Ok(presenter) => presenter,
        Err(_) => return PRESENTER_FAILED,
    };
    let mut host = LoadingHost::new(&mut presenter);
    let outcome = horizon_client::entry::main_with_cafe_host(Default::default(), &mut host);
    if host.failure.is_some() {
        return LOADING_FAILED;
    }
    if host.exited {
        return match &outcome {
            CafeRunOutcome::Loading(session) if session.status() == CafeLoadingStatus::Failed => {
                LOADING_FAILED
            }
            _ => FINISHED,
        };
    }
    match outcome {
        CafeRunOutcome::Finished => FINISHED,
        CafeRunOutcome::Loading(session) => match session.status() {
            CafeLoadingStatus::Loading => LOADING_FAILED,
            CafeLoadingStatus::ReadyForHandoff => READY_WITHOUT_GAME_HANDOFF,
            CafeLoadingStatus::Failed => LOADING_FAILED,
        },
    }
}

struct LoadingHost<'a> {
    presenter: &'a mut Gx2LoadingPresenter<'static>,
    control: Cell<EventLoopControl>,
    vpad: VpadState,
    touch: TouchMapper,
    focused: bool,
    touch_blocked_until_release: bool,
    pad_blocked_until_neutral: bool,
    exited: bool,
    failure: Option<String>,
}

impl<'a> LoadingHost<'a> {
    fn new(presenter: &'a mut Gx2LoadingPresenter<'static>) -> Self {
        Self {
            presenter,
            control: Cell::new(EventLoopControl::Poll),
            vpad: VpadState::default(),
            touch: TouchMapper::default(),
            focused: true,
            touch_blocked_until_release: false,
            pad_blocked_until_neutral: false,
            exited: false,
            failure: None,
        }
    }

    fn running(&self) -> bool {
        // SAFETY: C owns and initialized WHBProc before entering Rust.
        unsafe { horizon_wiiu_app_is_running() != 0 }
    }

    fn dispatch_touch(&mut self, session: &mut CafeLoadingSession, release: Option<horizon_wiiu_platform::FreshTouchRelease>) {
        if self.touch_blocked_until_release {
            if release.is_none() {
                return;
            }
            self.touch_blocked_until_release = false;
        }
        let mut dimensions = [0u32; 4];
        // SAFETY: dimensions points to four writable u32 values; C queries
        // WHB's current TV/DRC targets synchronously.
        let result = unsafe { horizon_loading_host_dimensions(dimensions.as_mut_ptr()) };
        if result != 0 {
            self.failure = Some("WHB TV/DRC colour buffers are unavailable".into());
            return;
        }
        let viewport = [dimensions[2], dimensions[3]];
        for event in self
            .touch
            .events_with_freshness(&self.vpad, viewport, release)
        {
            // Dispatch each event immediately with the exact poll/release
            // evidence that produced it. Pointer position precedes press.
            session.dispatch_event(self, event);
        }
    }

    fn release_on_exit(&mut self, session: &mut CafeLoadingSession) {
        let release = self.touch.reset(&self.vpad);
        for event in release {
            session.dispatch_event(self, event);
        }
        let neutral = self.vpad.reset();
        self.pad_blocked_until_neutral = true;
        session.dispatch_pad_state(self, neutral);
        session.dispatch_event(
            self,
            horizon_client_platform::ClientWindowEvent::Focused(false),
        );
    }

    fn update_focus(&mut self, session: &mut CafeLoadingSession) -> bool {
        // SAFETY: ProcUI was initialized by WHBProcInit and is advanced by
        // WHBProcIsRunning on this same main thread.
        let focused = unsafe { horizon_wiiu_app_is_focused() != 0 };
        if focused == self.focused {
            return focused;
        }
        self.focused = focused;
        if focused {
            self.touch.focus_gained(&self.vpad);
        } else {
            self.touch_blocked_until_release = true;
            for event in self.touch.focus_lost(&self.vpad) {
                session.dispatch_event(self, event);
            }
            let neutral = self.vpad.reset();
            self.pad_blocked_until_neutral = true;
            session.dispatch_pad_state(self, neutral);
        }
        session.dispatch_event(
            self,
            horizon_client_platform::ClientWindowEvent::Focused(focused),
        );
        focused
    }
}

impl EventLoopCtl for LoadingHost<'_> {
    fn set_control_flow(&self, control: EventLoopControl) {
        self.control.set(control);
    }
}

impl CafeLoadingHost for LoadingHost<'_> {
    fn drive_loading(&mut self, session: &mut CafeLoadingSession) -> horizon_client::Result<()> {
        loop {
            if !self.running() || self.control.get() == EventLoopControl::Exit {
                self.exited = true;
                break;
            }
            let focused = self.update_focus(session);
            if !session.poll(self) {
                self.exited = true;
                break;
            }

            if !focused {
                // WHBGfx frees foreground buffers on a ProcUI release. Keep
                // loader/menu services moving but do not touch GX2 or VPAD.
                std::thread::sleep(Duration::from_millis(16));
                continue;
            }

            let (pad, fresh_sample, fresh_release, terminal_error) = match self.vpad.poll_with_evidence() {
                Ok((pad, fresh_sample, fresh_release)) => {
                    (pad, fresh_sample, fresh_release, false)
                }
                // VPAD busy means no new sample. Retain the previous state and
                // never claim a physical touch release.
                Err(-4) => (self.vpad.update(None), None, None, false),
                // Terminal controller errors reset VpadState in the adapter;
                // release the mapped click, while requiring a fresh physical
                // release before a later touch starts another click.
                Err(_) => {
                    self.pad_blocked_until_neutral = true;
                    (self.vpad.reset(), None, None, true)
                }
            };
            if terminal_error {
                // Deliver neutral immediately to release any menu edge, but
                // do not treat an error-created default as fresh rearm proof.
                session.dispatch_pad_state(self, pad);
            } else if self.pad_blocked_until_neutral {
                let neutral = pad.buttons == 0 && pad.triggers.iter().all(|value| *value == 0.0);
                // Only a successful sample can rearm. Cached neutral state,
                // no-sample/busy polls, and touch-release evidence do not.
                if fresh_sample.is_some() && neutral {
                    self.pad_blocked_until_neutral = false;
                    session.dispatch_pad_state(self, pad);
                }
            } else {
                // Pass the actual canonical Cafe controller sample through
                // Horizon's typed menu path; no keyboard events are synthesized.
                session.dispatch_pad_state(self, pad);
            }
            self.dispatch_touch(session, fresh_release);
            if self.failure.is_some() {
                break;
            }

            if let Err(error) = session.draw(self.presenter) {
                self.failure = Some(error.to_string());
                break;
            }
            match session.status() {
                CafeLoadingStatus::Loading => {}
                // Ready means the original title was accepted by the presenter,
                // not that a world was selected or transferred into Game.
                // Keep the title interactive until the WUT process exits.
                CafeLoadingStatus::ReadyForHandoff => {}
                CafeLoadingStatus::Failed => break,
            }

            // Bound the software loop to the display cadence while honoring
            // host exit. The title callback itself synchronously presents TV
            // and DRC with WHB's own frame begin/finish calls.
            let delay = match self.control.get() {
                EventLoopControl::Exit => Duration::ZERO,
                EventLoopControl::Poll => Duration::from_millis(16),
                EventLoopControl::WaitUntil(deadline) => {
                    deadline.saturating_duration_since(Instant::now())
                }
            };
            if !delay.is_zero() {
                std::thread::sleep(delay.min(Duration::from_millis(16)));
            }
        }
        // Every way out of the host loop releases touch and controller edges
        // while the Loading session and its event loop are still alive.
        self.release_on_exit(session);
        Ok(())
    }
}
