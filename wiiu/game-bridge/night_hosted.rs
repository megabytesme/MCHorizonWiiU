//! Game side of the launcher-owned Cafe session.
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use super::{CafeLoadingStatus, CafeRunOutcome, Gx2LoadingPresenter, LoadingHost, CAFE_APP_ROOT};

#[path = "local_wire.rs"]
mod local_wire;
#[path = "night_menu.rs"]
mod night_menu;
#[path = "night_store.rs"]
mod night_store;

// C/Pregame consume these statuses only after the native game and renderer drop.
const RETURNED: i32 = 0;
const SAVED: i32 = 1;
const RESOURCES: i32 = 2;
const FAILED: i32 = 3;
const OS_SAVE_FAILED: i32 = 4;

struct NativeFiles;

impl night_menu::GameFiles for NativeFiles {
    fn world(&self, path: &Path, identity: &str) -> Option<night_menu::World> {
        use horizon_client::save::{self, Level, SessionLock};
        let Some(level) = Level::read_summary(path) else {
            return Some(night_menu::World {
                name: path.file_name()?.to_str()?.to_owned(),
                detail: "Unreadable world metadata".into(),
                unavailable: Some("Open recovery in the game menu"),
            });
        };
        let unavailable = if SessionLock::is_locked(path) {
            Some("World is locked or inaccessible")
        } else if save::incompatible_version(path).is_some() {
            Some("Incompatible save version")
        } else if save::needs_file_fixes(path) || save::downgraded_from(path).is_some() {
            Some("Open in the game menu for upgrade or backup")
        } else if !horizon_client::game::hosted_world_profile_matches(level.player_uuid, identity) {
            Some("Select the world's original offline identity")
        } else {
            None
        };
        let mode = if level.hardcore {
            "Hardcore"
        } else {
            match level.mode() {
                1 => "Creative",
                2 => "Adventure",
                3 => "Spectator",
                _ => "Survival",
            }
        };
        let version = horizon_client::game::hosted_world_version(path)
            .unwrap_or_else(|| "Version unknown".into());
        Some(night_menu::World {
            name: level.name,
            detail: format!("{mode} / {version}"),
            unavailable,
        })
    }
}

fn local_service() -> &'static Mutex<night_menu::LocalService<NativeFiles>> {
    static LOCAL: OnceLock<Mutex<night_menu::LocalService<NativeFiles>>> = OnceLock::new();
    LOCAL.get_or_init(|| {
        Mutex::new(night_menu::LocalService::new(
            Path::new(CAFE_APP_ROOT).to_owned(),
            night_menu::ResourcePin {
                version: env!("HORIZON_NIGHT_MINECRAFT"),
                schema: env!("HORIZON_NIGHT_PREP_SCHEMA")
                    .parse()
                    .expect("verified resource schema"),
                client: env!("HORIZON_NIGHT_CLIENT_JAR"),
                server: env!("HORIZON_NIGHT_SERVER_JAR"),
                exporter: env!("HORIZON_NIGHT_EXPORTER"),
            },
            NativeFiles,
            &[
                ("Horizon source", env!("HORIZON_NIGHT_UPSTREAM")),
                ("Wii U client", env!("HORIZON_NIGHT_CLIENT")),
                ("Wii U patch series", env!("HORIZON_NIGHT_PATCH")),
                ("Builder", env!("HORIZON_NIGHT_BUILDER")),
                ("Prepared Minecraft", env!("HORIZON_NIGHT_MINECRAFT")),
                ("Application", "Horizon Night / Wii U"),
                ("Build input SHA-256", env!("HORIZON_NIGHT_BUILD_ID")),
            ],
        ))
    })
}

#[no_mangle]
pub unsafe extern "C" fn horizon_night_local(
    output: *mut local_wire::LocalMenu,
    command: u8,
    index: u8,
    input: *const u8,
    len: usize,
) {
    if output.is_null() || input.is_null() || len > 1024 {
        return;
    }
    let Ok(input) = std::str::from_utf8(unsafe { std::slice::from_raw_parts(input, len) }) else {
        return;
    };
    let mut service = local_service().lock().unwrap_or_else(|e| e.into_inner());
    if command != 0 {
        service.command(command, index, input);
    }
    unsafe { output.write(service.model()) };
}

/// The RPX calls this synchronously on the foreground thread. The caller has
/// released the Night HUD and lends the same ProcUI pump and WHB context.
#[no_mangle]
pub extern "C" fn horizon_hosted_game_run() -> i32 {
    let result = local_service()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .launch();
    let launch = match result {
        Ok(launch) => launch,
        Err(code) => return i32::from(code),
    };
    if std::env::set_current_dir(&launch.game).is_err() {
        return RESOURCES;
    }
    horizon_client::spans::startup::enable_log_sink();
    horizon_client_platform::cafe_memory::report("hosted.before_game");
    let result = run(launch);
    if let Err(error) = std::env::set_current_dir(CAFE_APP_ROOT) {
        log::error!("Cafe did not restore the launcher working directory: {error}");
    }
    horizon_client_platform::cafe_memory::report("hosted.after_game_drop");
    result
}

fn run(launch: night_menu::Launch) -> i32 {
    let mut presenter = match unsafe { Gx2LoadingPresenter::new_embedded(16_384) } {
        Ok(presenter) => presenter,
        Err(error) => {
            log::error!("Cafe hosted presenter initialization: {error}");
            return FAILED;
        }
    };
    let mut host = LoadingHost::new(&mut presenter);
    host.hosted = true;
    host.hosted_swap_ab = launch.swap_ab;
    host.pad_blocked_until_neutral = true;
    unsafe { super::OSReport(c"Horizon Wii U startup: hosted game entry\n".as_ptr()) };
    let mut options = vec![
        "--resources".to_owned(),
        launch.resources.to_string_lossy().into_owned(),
        "--name".to_owned(),
        launch.player,
    ];
    if let Some(world) = launch.world {
        options.extend(["--quickPlaySingleplayer".into(), world]);
    }
    if let Some(server) = launch.server {
        options.extend(["--quickPlayMultiplayer".into(), server]);
    }
    let outcome = horizon_client::entry::run_hosted_cafe(
        &mut host,
        options,
        [!launch.swap_ab, !launch.swap_xy],
    );
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
    if host.hosted_saved {
        SAVED
    } else {
        RETURNED
    }
}
