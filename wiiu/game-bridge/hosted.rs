//! Game side of the launcher-owned Cafe session.
use std::ffi::{CStr, c_char};
use std::path::{Path, PathBuf};

use super::{CAFE_APP_ROOT, CafeLoadingStatus, CafeRunOutcome, Gx2LoadingPresenter, LoadingHost};

// C/Pregame consume these statuses only after the native game and renderer drop.
const RETURNED: i32 = 0;
const SAVED: i32 = 1;
const RESOURCES: i32 = 2;
const FAILED: i32 = 3;
const OS_SAVE_FAILED: i32 = 4;

fn resources() -> PathBuf {
    Path::new(CAFE_APP_ROOT).join(".local/horizon-client/26.3")
}

#[no_mangle]
pub extern "C" fn horizon_hosted_resources_ready() -> bool {
    horizon_client::game::hosted_cafe_resources_ready(&resources())
}

/// The RPX calls this synchronously on the foreground thread. The caller has
/// released the Night HUD and lends the same ProcUI pump and WHB context.
#[no_mangle]
pub unsafe extern "C" fn horizon_hosted_game_run(player: *const c_char) -> i32 {
    if player.is_null() {
        return FAILED;
    }
    let Ok(player) = (unsafe { CStr::from_ptr(player) }).to_str() else {
        return FAILED;
    };
    if !horizon_hosted_resources_ready() || std::env::set_current_dir(CAFE_APP_ROOT).is_err() {
        return RESOURCES;
    }
    horizon_client::spans::startup::enable_log_sink();
    horizon_client_platform::cafe_memory::report("hosted.before_game");
    let result = run(player);
    horizon_client_platform::cafe_memory::report("hosted.after_game_drop");
    result
}

fn run(player: &str) -> i32 {
    let mut presenter = match unsafe { Gx2LoadingPresenter::new_embedded(16_384) } {
        Ok(presenter) => presenter,
        Err(error) => {
            log::error!("Cafe hosted presenter initialization: {error}");
            return FAILED;
        }
    };
    let mut host = LoadingHost::new(&mut presenter);
    host.hosted = true;
    host.pad_blocked_until_neutral = true;
    unsafe { super::OSReport(c"Horizon Wii U startup: hosted game entry\n".as_ptr()) };
    let outcome = horizon_client::entry::run_hosted_cafe(&mut host, &resources(), player);
    if host.hosted_exit_save_failed {
        return OS_SAVE_FAILED;
    }
    if let Err(error) = &outcome {
        log::error!("Cafe hosted game error: {error}");
        return FAILED;
    }
    if host.failure.is_some()
        || matches!(&outcome, Ok(CafeRunOutcome::Loading(session))
            if session.status() == CafeLoadingStatus::Failed)
    {
        return FAILED;
    }
    if host.hosted_saved { SAVED } else { RETURNED }
}
