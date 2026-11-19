//! Exercises the quit barrier with a real worker and an independent scratch file.
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

use horizon_wiiu_pregame::config::{Instance, LaunchConfig, Settings};
use horizon_wiiu_pregame::launcher::{Action, Launcher, Page};
use horizon_wiiu_pregame::session::{
    ExitWork, LiveEvent, PreparePoll, SaveError, SavePoll, SessionDriver, SessionFactory,
    SessionStatus, StartError, StartMode,
};

const EXPECTED_SAVE: &[u8] = b"synthetic world revision 7\n";

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.0.join("snapshot"));
        let _ = fs::remove_file(self.0.join("worker.lock"));
        let _ = fs::remove_dir(&self.0);
    }
}

struct Factory {
    path: PathBuf,
    gate: Option<Receiver<()>>,
}

struct Driver {
    path: PathBuf,
    lock: Option<File>,
    gate: Option<Receiver<()>>,
    worker: Option<JoinHandle<Result<(), SaveError>>>,
}

impl SessionFactory for Factory {
    type Driver = Driver;

    fn start(&mut self, _config: LaunchConfig<'_>, _mode: StartMode) -> Result<Driver, StartError> {
        let lock =
            File::create_new(self.path.join("worker.lock")).map_err(|_| StartError::Engine)?;
        Ok(Driver {
            path: self.path.clone(),
            lock: Some(lock),
            gate: self.gate.take(),
            worker: None,
        })
    }
}

impl SessionDriver for Driver {
    fn poll_prepare(&mut self) -> PreparePoll {
        PreparePoll::Ready
    }

    fn poll_live(&mut self) -> LiveEvent {
        LiveEvent::Continue
    }

    fn begin_save(&mut self) {
        let gate = self.gate.take().expect("one save attempt for this fixture");
        let path = self.path.join("snapshot");
        self.worker = Some(thread::spawn(move || {
            gate.recv_timeout(Duration::from_secs(5))
                .map_err(|_| SaveError::Write)?;
            let mut file = File::create_new(path).map_err(|_| SaveError::Write)?;
            file.write_all(b"synthetic world revision 7\n")
                .map_err(|_| SaveError::Write)?;
            file.sync_all().map_err(|_| SaveError::Write)
        }));
    }

    fn poll_save(&mut self) -> SavePoll {
        if !self.worker.as_ref().is_some_and(JoinHandle::is_finished) {
            return SavePoll::Pending;
        }
        match self.join_worker() {
            Ok(()) => SavePoll::Complete,
            Err(error) => SavePoll::Failed(error),
        }
    }

    fn begin_stop(&mut self) {}

    fn poll_stop(&mut self) -> bool {
        self.worker.is_none()
    }

    fn foreground(&mut self, _focused: bool) {}

    fn shutdown_for_exit(&mut self, work: ExitWork) -> Result<(), SaveError> {
        if work == ExitWork::SaveActive {
            self.begin_save();
        }
        self.join_worker()
    }
}

impl Driver {
    fn join_worker(&mut self) -> Result<(), SaveError> {
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| SaveError::Shutdown)??;
        }
        Ok(())
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        let _ = self.join_worker();
        drop(self.lock.take());
        let _ = fs::remove_file(self.path.join("worker.lock"));
    }
}

#[test]
fn launcher_returns_after_worker_sync_and_world_lease_release() {
    let created = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "horizon-wiiu-hosted-{}-{created}",
        std::process::id()
    ));
    fs::create_dir(&path).unwrap();
    let scratch = Scratch(path);
    let (permit, gate) = mpsc::channel();
    let mut app = Launcher::new(
        Factory {
            path: scratch.0.clone(),
            gate: Some(gate),
        },
        Instance::DEMO,
        Settings::default(),
    );
    app.dispatch(Action::Play);
    app.advance();
    assert_eq!(app.view().session, Some(SessionStatus::Live));
    app.dispatch(Action::SaveAndQuit);
    app.advance();
    assert_eq!(app.view().session, Some(SessionStatus::Saving));
    assert!(scratch.0.join("worker.lock").exists());
    assert!(!scratch.0.join("snapshot").exists());

    permit.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.view().page != Page::Home && Instant::now() < deadline {
        app.advance();
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(app.view().page, Page::Home);
    assert_eq!(fs::read(scratch.0.join("snapshot")).unwrap(), EXPECTED_SAVE);
    assert!(!scratch.0.join("worker.lock").exists());
}
