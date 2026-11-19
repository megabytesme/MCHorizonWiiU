//! Launcher side of the in-process Cafe game handoff.
use core::ffi::c_char;

use crate::config::{Instance, LaunchConfig, PreparedResources, Resources, WorldOrServer};
use crate::session::{
    ExitWork, LiveEvent, PreparePoll, ReturnTo, SaveError, SavePoll, SessionDriver, SessionFactory,
    StartError, StartMode,
};

const ROOT: &str = "/vol/external01/wiiu/apps/HorizonWiiU";
const RESOURCE_DIR: &str = "/vol/external01/wiiu/apps/HorizonWiiU/.local/horizon-client/26.3";

extern "C" {
    fn horizon_hosted_resources_ready() -> bool;
    fn horizon_hosted_game_run(player: *const c_char) -> i32;
}

pub(crate) fn available_instance() -> Option<Instance<'static>> {
    unsafe { horizon_hosted_resources_ready() }.then_some(Instance {
        game_directory: ROOT,
        target: WorldOrServer::GameMenu,
        resources: Resources::Prepared(PreparedResources {
            directory: RESOURCE_DIR,
            game_version: "26.3",
            // The offline builder records the full resource inventory independently.
            // The game validates the installed preparation manifest on startup.
            manifest_sha256: [0; 32],
        }),
    })
}

pub(crate) struct CafeFactory;

pub(crate) struct CafeDriver {
    player: &'static core::ffi::CStr,
    outcome: Option<ReturnTo>,
}

impl SessionFactory for CafeFactory {
    type Driver = CafeDriver;

    fn start(&mut self, config: LaunchConfig<'_>, _: StartMode) -> Result<CafeDriver, StartError> {
        if !unsafe { horizon_hosted_resources_ready() } {
            return Err(StartError::Resources);
        }
        let crate::accounts::Profile::Offline(name) = config.profile else {
            return Err(StartError::Unavailable);
        };
        let player = match name.as_str() {
            "Player" => c"Player",
            "Guest" => c"Guest",
            _ => return Err(StartError::Unavailable),
        };
        Ok(CafeDriver {
            player,
            outcome: None,
        })
    }
}

impl CafeDriver {
    // The caller has released the shell GPU and CPU buffers. The borrowed
    // foreground loop runs until the game acknowledges save and teardown.
    pub(crate) fn run(&mut self) -> bool {
        let code = unsafe { horizon_hosted_game_run(self.player.as_ptr()) };
        self.outcome = Some(match code {
            0 => ReturnTo::Cancelled,
            1 => ReturnTo::Saved,
            2 => ReturnTo::StartError(StartError::Resources),
            _ => ReturnTo::StartError(StartError::Engine),
        });
        code == 4
    }
}

impl SessionDriver for CafeDriver {
    fn poll_prepare(&mut self) -> PreparePoll {
        PreparePoll::Ready
    }

    fn poll_live(&mut self) -> LiveEvent {
        self.outcome
            .take()
            .map_or(LiveEvent::Continue, LiveEvent::Ended)
    }

    fn begin_save(&mut self) {}
    fn poll_save(&mut self) -> SavePoll {
        SavePoll::Failed(SaveError::Shutdown)
    }
    fn begin_stop(&mut self) {}
    fn poll_stop(&mut self) -> bool {
        true
    }
    fn foreground(&mut self, _: bool) {}
    fn shutdown_for_exit(&mut self, _: ExitWork) -> Result<(), SaveError> {
        Ok(())
    }
}
