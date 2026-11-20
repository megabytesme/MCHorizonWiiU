//! Launcher side of the in-process Cafe game handoff.
use crate::config::{Instance, LaunchConfig, PreparedResources, Resources, WorldOrServer};
use crate::local_wire::LocalMenu;
use crate::session::{
    ExitWork, LiveEvent, PreparePoll, ReturnTo, SaveError, SavePoll, SessionDriver, SessionFactory,
    StartError, StartMode,
};

const ROOT: &str = "/vol/external01/wiiu/apps/HorizonWiiU";
const RESOURCE_DIR: &str = "/vol/external01/wiiu/apps/HorizonWiiU/.local/horizon-client/26.3";

extern "C" {
    fn horizon_night_local(
        output: *mut LocalMenu,
        command: u8,
        index: u8,
        input: *const u8,
        len: usize,
    );
    fn horizon_hosted_game_run() -> i32;
}

pub(crate) fn hosted_instance() -> Instance<'static> {
    Instance {
        game_directory: ROOT,
        target: WorldOrServer::GameMenu,
        resources: Resources::Prepared(PreparedResources {
            directory: RESOURCE_DIR,
            game_version: "26.3",
            // The offline builder records the full resource inventory independently.
            // The game validates the installed preparation manifest on startup.
            manifest_sha256: None,
        }),
    }
}

pub(crate) fn local_menu(command: u8, index: u8, input: &str) -> LocalMenu {
    let mut output = LocalMenu::default();
    unsafe { horizon_night_local(&mut output, command, index, input.as_ptr(), input.len()) };
    output
}

pub(crate) struct CafeFactory;

pub(crate) struct CafeDriver {
    outcome: Option<ReturnTo>,
}

impl SessionFactory for CafeFactory {
    type Driver = CafeDriver;

    fn start(&mut self, _: LaunchConfig<'_>, _: StartMode) -> Result<CafeDriver, StartError> {
        Ok(CafeDriver { outcome: None })
    }
}

impl CafeDriver {
    // The caller has released the shell GPU and CPU buffers. The borrowed
    // foreground loop runs until the game acknowledges save and teardown.
    pub(crate) fn run(&mut self) -> bool {
        let code = unsafe { horizon_hosted_game_run() };
        self.outcome = Some(match code {
            0 => ReturnTo::Cancelled,
            1 => ReturnTo::Saved,
            2 => ReturnTo::StartError(StartError::Resources),
            5 => ReturnTo::StartError(StartError::Target),
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
