use std::cell::{Cell, RefCell};
use std::rc::Rc;

use horizon_wiiu_pregame::accounts::{OfflineName, Profile};
use horizon_wiiu_pregame::config::{
    Instance, LaunchConfig, PreparedResources, Resources, Settings, WorldOrServer,
};
use horizon_wiiu_pregame::launcher::{Action, ActionState, Launcher, OfflineProfile, Page, Touch};
use horizon_wiiu_pregame::session::{
    ExitWork, LiveEvent, PreparePoll, ReturnTo, SaveError, SavePoll, SessionDriver, SessionFactory,
    SessionStatus, StartError, StartMode,
};
use horizon_wiiu_pregame::{ACCEPT, BACK, DOWN, UP};

#[derive(Debug, PartialEq)]
enum Event {
    Started(&'static str),
    SaveRequested,
    SaveCompleted,
    StopRequested,
    Drained,
    Dropped,
    Foreground(bool),
    OsExit(ExitWork),
}

struct Probe {
    events: RefCell<Vec<Event>>,
    prepare: Cell<PreparePoll>,
    save: Cell<SavePoll>,
    drained: Cell<bool>,
    exit_error: Cell<Option<SaveError>>,
    game_quit: Cell<bool>,
    native_outcome: Cell<Option<ReturnTo>>,
    observed_input: RefCell<Option<(String, String, Settings)>>,
}

impl Default for Probe {
    fn default() -> Self {
        Self {
            events: RefCell::default(),
            prepare: Cell::new(PreparePoll::Pending { percent: None }),
            save: Cell::new(SavePoll::Pending),
            drained: Cell::new(false),
            exit_error: Cell::new(None),
            game_quit: Cell::new(false),
            native_outcome: Cell::new(None),
            observed_input: RefCell::default(),
        }
    }
}

struct Factory(Rc<Probe>);
struct Driver(Rc<Probe>);

impl SessionFactory for Factory {
    type Driver = Driver;

    fn start(&mut self, config: LaunchConfig<'_>, mode: StartMode) -> Result<Driver, StartError> {
        let Profile::Offline(name) = config.profile else {
            panic!("offline fixture expected")
        };
        let profile = if name.as_str() == "Guest" {
            "Guest"
        } else {
            "Player"
        };
        self.0.events.borrow_mut().push(Event::Started(profile));
        if let Resources::Prepared(resources) = config.instance.resources {
            assert_eq!(
                config.instance.target,
                WorldOrServer::LocalWorld {
                    id: "scratch-world"
                }
            );
            assert_eq!(resources.manifest_sha256, Some([42; 32]));
            assert_eq!(resources.game_version, "26.3");
            *self.0.observed_input.borrow_mut() = Some((
                config.instance.game_directory.to_owned(),
                resources.directory.to_owned(),
                config.settings,
            ));
        }
        if mode == StartMode::TestError {
            self.0.prepare.set(PreparePoll::Failed(StartError::Fixture));
        }
        Ok(Driver(self.0.clone()))
    }
}

impl SessionDriver for Driver {
    fn poll_prepare(&mut self) -> PreparePoll {
        self.0.prepare.get()
    }
    fn poll_live(&mut self) -> LiveEvent {
        if let Some(outcome) = self.0.native_outcome.take() {
            LiveEvent::Ended(outcome)
        } else if self.0.game_quit.replace(false) {
            LiveEvent::QuitRequested
        } else {
            LiveEvent::Continue
        }
    }
    fn begin_save(&mut self) {
        self.0.events.borrow_mut().push(Event::SaveRequested);
    }
    fn poll_save(&mut self) -> SavePoll {
        let state = self.0.save.get();
        if state == SavePoll::Complete {
            self.0.events.borrow_mut().push(Event::SaveCompleted);
        }
        state
    }
    fn begin_stop(&mut self) {
        self.0.events.borrow_mut().push(Event::StopRequested);
    }
    fn poll_stop(&mut self) -> bool {
        if self.0.drained.get() {
            self.0.events.borrow_mut().push(Event::Drained);
        }
        self.0.drained.get()
    }
    fn foreground(&mut self, focused: bool) {
        self.0.events.borrow_mut().push(Event::Foreground(focused));
    }
    fn shutdown_for_exit(&mut self, work: ExitWork) -> Result<(), SaveError> {
        self.0
            .events
            .borrow_mut()
            .extend([Event::OsExit(work), Event::Drained]);
        self.0.exit_error.get().map_or(Ok(()), Err)
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        self.0.events.borrow_mut().push(Event::Dropped);
    }
}

type App = Launcher<'static, Factory>;

fn launcher() -> (App, Rc<Probe>) {
    let probe = Rc::new(Probe::default());
    let mut app = Launcher::new(
        Factory(probe.clone()),
        Some(Instance::DEMO),
        Settings::default(),
    );
    app.sample(0, Touch::Released);
    (app, probe)
}

fn tap(app: &mut App, button: u32) {
    app.sample(button, Touch::Released);
    app.sample(0, Touch::Released);
}

#[test]
fn completed_native_session_returns_and_restarts_without_a_second_save() {
    let (mut app, probe) = launcher();
    for outcome in [ReturnTo::Saved, ReturnTo::Cancelled] {
        make_live(&mut app, &probe);
        probe.events.borrow_mut().clear();
        probe.native_outcome.set(Some(outcome));
        app.advance();
        assert_eq!(app.view().page, Page::Home);
        assert!(app.live_driver().is_none());
        assert_eq!(*probe.events.borrow(), [Event::Dropped]);
    }
}

#[test]
fn a_failed_native_start_returns_to_error_only_after_the_loop_releases() {
    let (mut app, probe) = launcher();
    make_live(&mut app, &probe);
    probe.events.borrow_mut().clear();
    probe
        .native_outcome
        .set(Some(ReturnTo::StartError(StartError::Resources)));
    app.advance();
    assert_eq!(app.view().page, Page::Error);
    assert_eq!(app.view().error, Some(StartError::Resources));
    assert_eq!(*probe.events.borrow(), [Event::Dropped]);
}

#[test]
fn release_launcher_disables_play_and_hides_fixture_controls() {
    let probe = Rc::new(Probe::default());
    let mut app = Launcher::new(Factory(probe.clone()), None, Settings::default());
    app.sample(0, Touch::Released);
    assert_eq!(app.view().focused, Some(Action::Profiles));
    assert_eq!(
        app.view().rows().next().unwrap().state,
        ActionState::Disabled
    );
    tap(&mut app, UP);
    assert_eq!(app.view().focused, Some(Action::Quit));
    tap(&mut app, DOWN);
    assert_eq!(app.view().focused, Some(Action::Profiles));
    app.sample(0, Touch::Pressed(Some(Action::Play)));
    app.sample(0, Touch::Released);
    app.dispatch(Action::Play);
    app.advance();
    assert_eq!(app.view().page, Page::Home);

    app.dispatch(Action::Versions);
    assert!(app
        .view()
        .rows()
        .all(|row| row.action != Action::DemoResources));
    app.dispatch(Action::Back);
    app.dispatch(Action::Settings);
    assert!(app
        .view()
        .rows()
        .all(|row| row.action != Action::TestStartupError));
    app.dispatch(Action::TestStartupError);
    app.advance();
    assert_eq!(app.view().page, Page::Settings);
    assert!(probe.events.borrow().is_empty());
}

fn make_live(app: &mut App, probe: &Probe) {
    probe.prepare.set(PreparePoll::Pending { percent: None });
    app.dispatch(Action::Play);
    app.advance();
    assert_eq!(app.view().session, Some(SessionStatus::Preparing(None)));
    assert!(app.live_driver().is_none());
    probe
        .prepare
        .set(PreparePoll::Pending { percent: Some(61) });
    app.advance();
    assert_eq!(app.view().progress(), Some(61));
    probe.prepare.set(PreparePoll::Ready);
    app.advance();
    assert_eq!(app.view().page, Page::Session);
    assert!(app.live_driver().is_some());
}

#[test]
fn controller_wraps_skips_disabled_rows_and_handles_face_swap() {
    let (mut app, _) = launcher();
    app.sample(UP, Touch::Released);
    app.sample(UP, Touch::Released);
    assert_eq!(app.view().focused, Some(Action::Quit));
    app.sample(0, Touch::Released);
    tap(&mut app, DOWN);
    assert_eq!(app.view().focused, Some(Action::Play));
    app.dispatch(Action::Profiles);
    tap(&mut app, DOWN);
    tap(&mut app, DOWN);
    assert_eq!(app.view().focused, Some(Action::Back));
    tap(&mut app, BACK);
    assert_eq!(app.view().focused, Some(Action::Profiles));
    app.dispatch(Action::Settings);
    tap(&mut app, ACCEPT);
    assert!(app.view().settings.swap_buttons);
    tap(&mut app, ACCEPT);
    assert_eq!(app.view().focused, Some(Action::Settings));
    tap(&mut app, BACK);
    assert_eq!(app.view().page, Page::Settings);
}

#[test]
fn launcher_waits_for_save_and_drain_before_restoring_menu() {
    let (mut app, probe) = launcher();
    app.dispatch(Action::Profiles);
    app.dispatch(Action::Guest);
    assert_eq!(app.view().profile, OfflineProfile::Guest);
    app.dispatch(Action::Back);
    make_live(&mut app, &probe);
    tap(&mut app, BACK);
    tap(&mut app, BACK);
    app.advance();
    assert_eq!(app.view().session, Some(SessionStatus::Saving));
    assert_eq!(app.view().focused, None);
    assert!(app.live_driver().is_none());
    assert_eq!(
        *probe.events.borrow(),
        [Event::Started("Guest"), Event::SaveRequested]
    );

    probe.save.set(SavePoll::Complete);
    app.advance();
    app.advance();
    assert_eq!(app.view().page, Page::Session);
    assert_eq!(
        app.view().session,
        Some(SessionStatus::Stopping(ReturnTo::Saved))
    );
    assert_eq!(
        *probe.events.borrow(),
        [
            Event::Started("Guest"),
            Event::SaveRequested,
            Event::SaveCompleted,
            Event::StopRequested,
        ]
    );

    probe.drained.set(true);
    app.advance();
    assert_eq!(app.view().page, Page::Home);
    assert_eq!(app.view().session, None);
    assert_eq!(
        *probe.events.borrow(),
        [
            Event::Started("Guest"),
            Event::SaveRequested,
            Event::SaveCompleted,
            Event::StopRequested,
            Event::Drained,
            Event::Dropped,
        ]
    );
    probe.prepare.set(PreparePoll::Pending { percent: None });
    make_live(&mut app, &probe);
    assert_eq!(probe.events.borrow().last(), Some(&Event::Started("Guest")));
}

#[test]
fn game_requested_quit_shares_save_barrier_and_can_retry_failure() {
    let (mut app, probe) = launcher();
    make_live(&mut app, &probe);
    probe.game_quit.set(true);
    app.advance();
    assert_eq!(app.view().session, Some(SessionStatus::Saving));
    probe.save.set(SavePoll::Failed(SaveError::Write));
    app.advance();
    assert_eq!(
        app.view().session,
        Some(SessionStatus::SaveFailed(SaveError::Write))
    );
    assert!(app.live_driver().is_none());
    assert_eq!(app.view().focused, Some(Action::SaveAndQuit));
    assert_eq!(
        *probe.events.borrow(),
        [Event::Started("Player"), Event::SaveRequested]
    );

    tap(&mut app, ACCEPT);
    probe.save.set(SavePoll::Complete);
    probe.drained.set(true);
    app.advance();
    app.advance();
    assert_eq!(app.view().page, Page::Home);
    assert_eq!(
        *probe.events.borrow(),
        [
            Event::Started("Player"),
            Event::SaveRequested,
            Event::SaveRequested,
            Event::SaveCompleted,
            Event::StopRequested,
            Event::Drained,
            Event::Dropped,
        ]
    );
}

#[test]
fn cancellation_discards_late_ready_and_start_error_waits_for_cleanup() {
    let (mut app, probe) = launcher();
    app.dispatch(Action::Play);
    app.dispatch(Action::CancelLoading);
    app.advance();
    assert!(probe.events.borrow().is_empty());
    app.dispatch(Action::Play);
    app.advance();
    app.dispatch(Action::CancelLoading);
    probe.prepare.set(PreparePoll::Ready);
    app.advance();
    assert_eq!(
        app.view().session,
        Some(SessionStatus::Stopping(ReturnTo::Cancelled))
    );
    assert!(app.live_driver().is_none());
    probe.drained.set(true);
    app.advance();
    assert_eq!(app.view().page, Page::Home);

    probe.drained.set(false);
    app.dispatch(Action::Settings);
    app.dispatch(Action::TestStartupError);
    app.advance();
    assert_eq!(app.view().page, Page::Loading);
    assert_eq!(app.view().error, None);
    probe.drained.set(true);
    app.advance();
    assert_eq!(app.view().page, Page::Error);
    assert_eq!(app.view().error, Some(StartError::Fixture));
    assert_eq!(
        *probe.events.borrow(),
        [
            Event::Started("Player"),
            Event::StopRequested,
            Event::Drained,
            Event::Dropped,
            Event::Started("Player"),
            Event::StopRequested,
            Event::Drained,
            Event::Dropped,
        ]
    );
    tap(&mut app, ACCEPT);
    assert_eq!(app.view().page, Page::Home);
}

#[test]
fn forced_exit_uses_phase_specific_barrier_even_on_failure() {
    for work in [
        ExitWork::AbortPreparation,
        ExitWork::SaveActive,
        ExitWork::FinishSave,
        ExitWork::FinishStop,
    ] {
        let (mut app, probe) = launcher();
        if work == ExitWork::AbortPreparation {
            app.dispatch(Action::Play);
            app.advance();
        } else {
            make_live(&mut app, &probe);
        }
        if matches!(work, ExitWork::FinishSave | ExitWork::FinishStop) {
            app.dispatch(Action::SaveAndQuit);
        }
        if work == ExitWork::FinishStop {
            probe.save.set(SavePoll::Complete);
            app.advance();
        }
        probe.exit_error.set(Some(SaveError::Shutdown));
        assert_eq!(app.close(), Err(SaveError::Shutdown));
        assert_eq!(app.close(), Ok(()));
        assert_eq!(app.view().page, Page::Exiting);
        let events = probe.events.borrow();
        assert!(events.ends_with(&[Event::OsExit(work), Event::Drained, Event::Dropped]));
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, Event::OsExit(_)))
                .count(),
            1
        );
    }
}

#[test]
fn focus_loss_suspends_owner_and_rearms_controller_after_neutral_input() {
    let (mut app, probe) = launcher();
    make_live(&mut app, &probe);
    app.focus(false);
    app.focus(false);
    app.sample(BACK, Touch::Released);
    app.advance();
    assert!(app.live_driver().is_none());
    app.focus(true);
    app.sample(BACK, Touch::Released);
    assert_eq!(app.view().session, Some(SessionStatus::Live));
    app.sample(0, Touch::Released);
    app.input_lost();
    app.sample(BACK, Touch::Released);
    assert_eq!(app.view().session, Some(SessionStatus::Live));
    app.sample(0, Touch::Released);
    tap(&mut app, BACK);
    assert_eq!(
        *probe.events.borrow(),
        [
            Event::Started("Player"),
            Event::Foreground(false),
            Event::Foreground(true),
            Event::SaveRequested,
        ]
    );
}

#[test]
fn prepared_instance_keeps_paths_and_settings_in_the_launch_request() {
    let probe = Rc::new(Probe::default());
    let instance = Instance {
        game_directory: "/scratch/instance",
        target: WorldOrServer::LocalWorld {
            id: "scratch-world",
        },
        resources: Resources::Prepared(PreparedResources {
            directory: "/scratch/resources",
            game_version: "26.3",
            manifest_sha256: Some([42; 32]),
        }),
    };
    let mut app = Launcher::new(Factory(probe.clone()), Some(instance), Settings::default());
    app.dispatch(Action::Versions);
    assert!(app
        .view()
        .rows()
        .any(|row| row.action == Action::GameResources && row.state == ActionState::Selected));
    app.dispatch(Action::Back);
    app.dispatch(Action::Settings);
    app.dispatch(Action::TestStartupError);
    assert_eq!(app.view().page, Page::Settings);
    app.dispatch(Action::SwapButtons);
    app.dispatch(Action::Back);
    app.dispatch(Action::Play);
    app.advance();
    assert_eq!(
        *probe.observed_input.borrow(),
        Some((
            "/scratch/instance".to_owned(),
            "/scratch/resources".to_owned(),
            Settings {
                swap_buttons: true,
                touch_navigation: true
            },
        ))
    );
}

#[test]
fn touch_acts_on_release_and_rejects_drag_disabled_and_stale_contacts() {
    let (mut app, _) = launcher();
    app.sample(0, Touch::Pressed(Some(Action::Profiles)));
    assert_eq!(app.view().page, Page::Home);
    app.sample(0, Touch::Released);
    assert_eq!(app.view().page, Page::Profiles);
    app.sample(0, Touch::Pressed(Some(Action::MicrosoftSignIn)));
    app.sample(0, Touch::Released);
    assert_eq!(app.view().focused, Some(Action::Player));
    app.sample(0, Touch::Pressed(Some(Action::Guest)));
    app.sample(0, Touch::Pressed(Some(Action::Player)));
    app.sample(0, Touch::Released);
    assert_eq!(app.view().profile, OfflineProfile::Player);
    app.focus(false);
    app.focus(true);
    app.sample(0, Touch::Pressed(Some(Action::Guest)));
    app.sample(0, Touch::Released);
    assert_eq!(app.view().profile, OfflineProfile::Player);
    app.sample(0, Touch::Pressed(Some(Action::Guest)));
    app.sample(0, Touch::Released);
    assert_eq!(app.view().profile, OfflineProfile::Guest);
    app.dispatch(Action::Back);
    app.dispatch(Action::Settings);
    app.dispatch(Action::ToggleTouch);
    app.sample(0, Touch::Released);
    app.sample(0, Touch::Pressed(Some(Action::Back)));
    app.sample(0, Touch::Released);
    assert_eq!(app.view().page, Page::Settings);
}

#[test]
fn offline_profile_validates_player_names() {
    assert_eq!(
        OfflineName::parse("Cafe_Player42").unwrap().as_str(),
        "Cafe_Player42"
    );
    for bad in ["", "user name", "café", "name_longer_than_16", "../Player"] {
        assert!(OfflineName::parse(bad).is_none());
    }
}
