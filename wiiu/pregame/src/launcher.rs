//! Portable launcher state and presentation contract. The native host owns
//! frame timing and provides input samples and a concrete session factory.

use crate::accounts::{OfflineName, Profile};
use crate::config::{Instance, LaunchConfig, Resources, Settings};
use crate::local_wire::{self, LocalMenu};
use crate::session::{
    HostedSession, ReturnTo, SaveError, SessionFactory, SessionStatus, StartError, StartMode,
};
use crate::{ACCEPT, BACK, DOWN, UP};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Page {
    Home,
    Profiles,
    Versions,
    Settings,
    AgentControl,
    Local,
    Loading,
    Session,
    Error,
    Exiting,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Play,
    Profiles,
    Library,
    Versions,
    Settings,
    AgentControl,
    Quit,
    Player,
    Guest,
    MicrosoftSignIn,
    DemoResources,
    GameResources,
    SwapButtons,
    ToggleTouch,
    TestStartupError,
    PairAgent,
    Back,
    CancelLoading,
    SaveAndQuit,
    DismissError,
    Local(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionState {
    Available,
    Selected,
    Disabled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Row<'a> {
    pub action: Action,
    pub label: &'a str,
    pub detail: &'a str,
    pub state: ActionState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OfflineProfile {
    Player,
    Guest,
}

impl OfflineProfile {
    pub fn name(self) -> &'static str {
        match self {
            Self::Player => "Player",
            Self::Guest => "Guest",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceSelection {
    Unavailable,
    BuiltInDemo,
    Prepared,
}

impl ResourceSelection {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unavailable => "Unavailable",
            Self::BuiltInDemo => "Built-in demo",
            Self::Prepared => "Prepared game",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentStatus {
    Unavailable,
    Offline,
    Paired,
}

/// A future app-scope implementation runs one listener. Gameplay registrations
/// use opaque generation handles that the host must withdraw before game Drop.
/// No transport or pairing is installed by the launcher prototype.
pub trait AgentControl {
    type Registration;
    type Tools: ?Sized;
    type Error;

    fn start(&mut self) -> Result<(), Self::Error>;
    fn status(&self) -> AgentStatus;
    fn register_session(&mut self, tools: &Self::Tools) -> Result<Self::Registration, Self::Error>;
    fn withdraw_session(&mut self, registration: Self::Registration);
    fn shutdown(&mut self);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct View {
    pub page: Page,
    pub focused: Option<Action>,
    pub profile: OfflineProfile,
    pub settings: Settings,
    pub resources: ResourceSelection,
    pub agent_status: AgentStatus,
    pub error: Option<StartError>,
    pub session: Option<SessionStatus>,
    pub local: Option<LocalMenu>,
}

impl View {
    pub fn title(&self) -> &str {
        match self.page {
            Page::Home => "Play",
            Page::Profiles => "Profiles",
            Page::Versions => "Version & resources",
            Page::Settings => "Settings",
            Page::AgentControl => "Agent Control",
            Page::Local => self.local.as_ref().map_or("", |menu| menu.title.as_str()),
            Page::Loading if matches!(self.session, Some(SessionStatus::Stopping(_))) => {
                "Releasing session"
            }
            Page::Loading => "Preparing session",
            Page::Session => match self.session {
                Some(SessionStatus::Saving) => "Saving session",
                Some(SessionStatus::SaveFailed(_)) => "Could not finish saving",
                Some(SessionStatus::Stopping(_)) => "Releasing session",
                _ if self.resources == ResourceSelection::BuiltInDemo => "Demo session",
                _ => "Horizon session",
            },
            Page::Error => "Could not start",
            Page::Exiting => "Closing Horizon",
        }
    }

    pub fn description(&self) -> &str {
        if let Some(menu) = &self.local {
            let description = menu.description.as_str();
            if self.page == Page::Local || (self.page == Page::Home && !description.is_empty()) {
                return description;
            }
        }
        match self.page {
            Page::Local => "",
            Page::Home
                if self.resources == ResourceSelection::Unavailable
                    && cfg!(horizon_wiiu_hosted) =>
            {
                "Prepared Horizon 26.3 resources are required."
            }
            Page::Home if self.resources != ResourceSelection::Prepared => {
                "Native game bridge pending."
            }
            Page::Home => "Review the selected Horizon instance.",
            Page::Profiles => "Select a local offline profile.",
            Page::Versions => match self.resources {
                ResourceSelection::Unavailable if cfg!(horizon_wiiu_hosted) => {
                    "Install the prepared Horizon 26.3 resource directory."
                }
                ResourceSelection::Unavailable => {
                    "Game resources will be available when the native bridge is ready."
                }
                ResourceSelection::BuiltInDemo => "The demo uses built-in resources.",
                ResourceSelection::Prepared => "This instance uses prepared game resources.",
            },
            Page::Settings => "Input preferences apply during this launch.",
            Page::AgentControl => "Launcher pairing is unavailable in this build.",
            Page::Loading if matches!(self.session, Some(SessionStatus::Stopping(_))) => {
                "Waiting for startup workers and resources to close."
            }
            Page::Loading if self.resources == ResourceSelection::BuiltInDemo => {
                "Preparing the built-in demo."
            }
            Page::Loading => "Preparing the selected Horizon instance.",
            Page::Session => match self.session {
                Some(SessionStatus::Saving) => "Waiting for the session to finish saving.",
                Some(SessionStatus::SaveFailed(SaveError::Write)) => {
                    "The save failed. Retry before returning to the launcher."
                }
                Some(SessionStatus::SaveFailed(SaveError::Disconnect)) => {
                    "The connection did not close cleanly. Try again."
                }
                Some(SessionStatus::SaveFailed(SaveError::Shutdown)) => {
                    "The session could not finish closing. Try again."
                }
                Some(SessionStatus::Stopping(_)) => "Closing session workers and resources.",
                _ if self.resources == ResourceSelection::BuiltInDemo => {
                    "Prototype gameplay session. No world is loaded."
                }
                _ => "The selected Horizon instance is active.",
            },
            Page::Error => match self.error {
                Some(StartError::Fixture) => "The startup error test released its partial session.",
                Some(StartError::OutOfMemory) => "Session memory is unavailable.",
                Some(StartError::Resources) => "The prepared resources failed verification.",
                Some(StartError::Engine) => "Horizon could not prepare the selected instance.",
                Some(StartError::Target) => "The world or server selection is no longer available.",
                _ => "The profile or launch target is unavailable.",
            },
            Page::Exiting => "Releasing launcher resources.",
        }
    }

    pub fn progress(self) -> Option<u8> {
        match self.session {
            Some(SessionStatus::Preparing(percent)) => percent,
            _ => None,
        }
    }

    pub fn footer(self) -> &'static str {
        if self.settings.swap_buttons {
            "D-pad Move    B Select    A Back"
        } else {
            "D-pad Move    A Select    B Back"
        }
    }

    pub fn rows(&self) -> impl ExactSizeIterator<Item = Row<'_>> {
        self.actions(self.page)
            .iter()
            .copied()
            .map(move |action| self.row(action))
    }

    fn row(&self, action: Action) -> Row<'_> {
        use Action::*;
        use ActionState::*;
        let (label, detail, state) = match action {
            Local(index) => self
                .local
                .as_ref()
                .and_then(|menu| menu.rows.get(usize::from(index)))
                .map_or(("", "", Disabled), |row| {
                    (
                        row.label.as_str(),
                        row.detail.as_str(),
                        match row.state {
                            local_wire::ACTIVE => Available,
                            local_wire::SELECTED => Selected,
                            _ => Disabled,
                        },
                    )
                }),
            Play if self.resources == ResourceSelection::BuiltInDemo => {
                ("Start demo", "Built-in fixture", Available)
            }
            Play => (
                "Start Game",
                self.local
                    .as_ref()
                    .map_or(self.resources.label(), |menu| menu.play_detail.as_str()),
                if self.resources == ResourceSelection::Unavailable {
                    Disabled
                } else {
                    Available
                },
            ),
            Profiles => (
                "Profiles",
                self.local
                    .as_ref()
                    .map_or(self.profile.name(), |m| m.profile.as_str()),
                Available,
            ),
            Library => ("Worlds & servers", "Select a launch target", Available),
            Versions if self.local.is_some() => {
                ("Version & resources", "Installations and About", Available)
            }
            Versions => ("Version & resources", env!("CARGO_PKG_VERSION"), Available),
            Settings => ("Settings", "Controller and touch", Available),
            #[cfg(horizon_wiiu_native_mcp)]
            AgentControl => ("Agent Control", "HTTPS and trusted clients", Available),
            #[cfg(not(horizon_wiiu_native_mcp))]
            AgentControl => ("Agent Control", "Unavailable", Available),
            Quit => ("Close Horizon", "", Available),
            Player => (
                "Player",
                "Offline profile",
                self.profile_state(OfflineProfile::Player),
            ),
            Guest => (
                "Guest",
                "Offline profile",
                self.profile_state(OfflineProfile::Guest),
            ),
            MicrosoftSignIn => ("Microsoft account", "Sign-in provider pending", Disabled),
            DemoResources if self.resources == ResourceSelection::BuiltInDemo => {
                ("Built-in demo", "Ready", Selected)
            }
            DemoResources => ("Built-in demo", "Unavailable for this instance", Disabled),
            GameResources if self.resources == ResourceSelection::Prepared => {
                ("Installed game resources", "Configured", Selected)
            }
            GameResources => ("Installed game resources", "Game bridge pending", Disabled),
            SwapButtons => (
                "Swap A and B",
                on_off(self.settings.swap_buttons),
                Available,
            ),
            ToggleTouch => (
                "GamePad touch",
                on_off(self.settings.touch_navigation),
                Available,
            ),
            TestStartupError if self.resources == ResourceSelection::BuiltInDemo => {
                ("Test startup error", "Run diagnostic", Available)
            }
            TestStartupError => ("Test startup error", "Demo only", Disabled),
            PairAgent => ("Pair a PC", "Launcher pairing pending", Disabled),
            Back => ("Back", "", Available),
            CancelLoading if matches!(self.session, Some(SessionStatus::Stopping(_))) => {
                ("Cancel", "Please wait", Disabled)
            }
            CancelLoading => ("Cancel", "", Available),
            SaveAndQuit => match self.session {
                Some(SessionStatus::Live) => ("Save & Quit", "Return to launcher", Available),
                Some(SessionStatus::SaveFailed(_)) => {
                    ("Retry Save & Quit", "Return to launcher", Available)
                }
                _ => ("Save & Quit", "Please wait", Disabled),
            },
            DismissError => ("Return to Play", "", Available),
        };
        Row {
            action,
            label,
            detail,
            state,
        }
    }

    fn actions(&self, page: Page) -> &'static [Action] {
        use Action::*;
        match page {
            Page::Home if self.local.is_some() => &[
                Play,
                Profiles,
                Library,
                Versions,
                Settings,
                AgentControl,
                Quit,
            ],
            Page::Home => &[Play, Profiles, Versions, Settings, AgentControl, Quit],
            Page::Local => {
                const ACTIONS: [Action; local_wire::ROWS] =
                    [Local(0), Local(1), Local(2), Local(3), Local(4), Local(5)];
                ACTIONS
                    .get(..self.local.as_ref().map_or(0, |m| usize::from(m.count)))
                    .unwrap_or(&[])
            }
            Page::Profiles => &[Player, Guest, MicrosoftSignIn, Back],
            Page::Versions if self.resources == ResourceSelection::BuiltInDemo => {
                &[DemoResources, GameResources, Back]
            }
            Page::Versions => &[GameResources, Back],
            Page::Settings if self.resources == ResourceSelection::BuiltInDemo => {
                &[SwapButtons, ToggleTouch, TestStartupError, Back]
            }
            Page::Settings => &[SwapButtons, ToggleTouch, Back],
            Page::AgentControl => &[PairAgent, Back],
            Page::Loading => &[CancelLoading],
            Page::Session => &[SaveAndQuit],
            Page::Error => &[DismissError],
            Page::Exiting => &[],
        }
    }

    fn profile_state(self, profile: OfflineProfile) -> ActionState {
        if self.profile == profile {
            ActionState::Selected
        } else {
            ActionState::Available
        }
    }
}

fn on_off(enabled: bool) -> &'static str {
    if enabled {
        "On"
    } else {
        "Off"
    }
}

/// A hardware sample after DRC calibration and theme hit testing. An invalid
/// touch breaks a gesture. A missing hardware sample is handled by the host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Touch {
    Released,
    Pressed(Option<Action>),
    Invalid,
}

pub struct Launcher<'a, F: SessionFactory> {
    factory: F,
    instance: Option<Instance<'a>>,
    session: Option<HostedSession<F::Driver>>,
    view: View,
    pending: Option<StartMode>,
    foreground: bool,
    previous_buttons: Option<u32>,
    touch_ready: bool,
    touch_held: bool,
    touch_action: Option<Action>,
    local_intent: Option<(u8, u8)>,
    local_return: Option<Action>,
}

impl<'a, F: SessionFactory> Launcher<'a, F> {
    pub fn new(factory: F, instance: Option<Instance<'a>>, settings: Settings) -> Self {
        let resources = match instance.map(|instance| instance.resources) {
            None => ResourceSelection::Unavailable,
            Some(Resources::BuiltInDemo) => ResourceSelection::BuiltInDemo,
            Some(Resources::Prepared(_)) => ResourceSelection::Prepared,
        };
        let focus = if instance.is_some() {
            Action::Play
        } else {
            Action::Profiles
        };
        Self {
            factory,
            instance,
            session: None,
            view: View {
                page: Page::Home,
                focused: Some(focus),
                profile: OfflineProfile::Player,
                settings,
                resources,
                agent_status: AgentStatus::Unavailable,
                error: None,
                session: None,
                local: None,
            },
            pending: None,
            foreground: true,
            previous_buttons: None,
            touch_ready: false,
            touch_held: false,
            touch_action: None,
            local_intent: None,
            local_return: None,
        }
    }

    pub fn view(&self) -> View {
        self.view
    }

    pub fn install_local_menu(&mut self, local: LocalMenu) {
        let first = self.view.local.is_none();
        let previous = self.view.local.map(|menu| menu.title);
        self.view.local = Some(local);
        self.view.settings.swap_buttons = local.settings & local_wire::SWAP_AB != 0;
        self.view.settings.touch_navigation = local.settings & local_wire::TOUCH != 0;
        self.view.resources = if local.ready != 0 {
            ResourceSelection::Prepared
        } else {
            ResourceSelection::Unavailable
        };
        if local.title.as_str().is_empty() {
            if self.view.page == Page::Local || first {
                let focus = self.local_return.take();
                self.show(Page::Home, focus);
            }
        } else if self.view.page != Page::Local
            || previous != Some(local.title)
            || !self.view.focused.is_some_and(|action| self.enabled(action))
        {
            self.show(Page::Local, None);
        }
    }

    pub fn take_local_intent(&mut self) -> Option<(u8, u8)> {
        self.local_intent.take()
    }

    /// The host lends a foreground frame and gameplay input to this driver.
    /// A preparation, save or teardown frame belongs to the shell.
    pub fn live_driver(&mut self) -> Option<&mut F::Driver> {
        if !self.foreground {
            return None;
        }
        self.session.as_mut()?.live_driver()
    }

    pub fn focus(&mut self, foreground: bool) {
        if self.foreground != foreground {
            self.foreground = foreground;
            self.input_lost();
            if let Some(session) = self.session.as_mut() {
                session.foreground(foreground);
            }
        }
    }

    pub fn input_lost(&mut self) {
        self.previous_buttons = None;
        self.touch_ready = false;
        self.touch_held = false;
        self.touch_action = None;
    }

    pub fn sample(&mut self, held: u32, touch: Touch) {
        if !self.foreground || self.view.page == Page::Exiting {
            return;
        }
        let Some(previous) = self.previous_buttons else {
            if held == 0 {
                self.previous_buttons = Some(0);
            }
            self.handle_touch(touch);
            return;
        };
        self.previous_buttons = Some(held);
        let pressed = held & !previous;
        if pressed != 0 {
            self.touch_action = None;
            self.controller(pressed);
        }
        self.handle_touch(touch);
    }

    fn handle_touch(&mut self, touch: Touch) {
        if !self.view.settings.touch_navigation {
            return;
        }
        match touch {
            Touch::Invalid => {
                self.touch_ready = false;
                self.touch_held = false;
                self.touch_action = None;
            }
            Touch::Released => {
                self.touch_ready = true;
                self.touch_held = false;
                if let Some(action) = self.touch_action.take() {
                    self.dispatch(action);
                }
            }
            Touch::Pressed(action) if self.touch_ready => {
                if self.touch_held {
                    if action != self.touch_action {
                        self.touch_action = None;
                    }
                } else {
                    self.touch_held = true;
                    self.touch_action = action.filter(|id| self.enabled(*id));
                    if self.touch_action.is_some() {
                        self.view.focused = self.touch_action;
                    }
                }
            }
            Touch::Pressed(_) => {}
        }
    }

    fn controller(&mut self, pressed: u32) {
        let (accept, back) = if self.view.settings.swap_buttons {
            (BACK, ACCEPT)
        } else {
            (ACCEPT, BACK)
        };
        if pressed & back != 0 {
            match self.view.page {
                Page::Home => {}
                Page::Loading => self.dispatch(Action::CancelLoading),
                Page::Session => self.dispatch(Action::SaveAndQuit),
                Page::Error => self.dispatch(Action::DismissError),
                Page::Local => self.local_intent = Some((local_wire::BACK, 0)),
                _ => self.dispatch(Action::Back),
            }
            return;
        }
        if self.view.page == Page::Local
            && self
                .view
                .local
                .is_some_and(|menu| menu.mode == local_wire::EDITOR)
        {
            match pressed & (crate::LEFT | crate::RIGHT) {
                crate::LEFT => self.local_intent = Some((local_wire::LEFT, 0)),
                crate::RIGHT => self.local_intent = Some((local_wire::RIGHT, 0)),
                _ => {}
            }
        }
        match pressed & (UP | DOWN) {
            UP => self.move_focus(false),
            DOWN => self.move_focus(true),
            _ => {}
        }
        if pressed & accept != 0 {
            if let Some(action) = self.view.focused {
                self.dispatch(action);
            }
        }
    }

    fn move_focus(&mut self, forward: bool) {
        let options = self.view.actions(self.view.page);
        let Some(current) = options
            .iter()
            .position(|action| Some(*action) == self.view.focused)
        else {
            return;
        };
        let next = if forward {
            options
                .iter()
                .copied()
                .cycle()
                .skip(current + 1)
                .take(options.len())
                .find(|action| self.enabled(*action))
        } else {
            options
                .iter()
                .rev()
                .copied()
                .cycle()
                .skip(options.len() - current)
                .take(options.len())
                .find(|action| self.enabled(*action))
        };
        if next.is_some() {
            self.view.focused = next;
        }
    }

    fn enabled(&self, action: Action) -> bool {
        self.view.actions(self.view.page).contains(&action)
            && self.view.row(action).state != ActionState::Disabled
    }

    fn show(&mut self, page: Page, focus: Option<Action>) {
        self.view.page = page;
        self.view.focused = focus.filter(|action| self.enabled(*action)).or_else(|| {
            self.view
                .actions(page)
                .iter()
                .copied()
                .find(|action| self.enabled(*action))
        });
        self.view.error = None;
        self.touch_action = None;
        self.touch_ready = false;
    }

    /// Shared transition entry point. Gestures and renderers cannot activate a
    /// disabled action or an action belonging to a different page.
    pub fn dispatch(&mut self, action: Action) {
        if !self.foreground || !self.enabled(action) {
            return;
        }
        use Action::*;
        match action {
            Profiles | Library | Versions | Settings if self.view.local.is_some() => {
                self.local_return = Some(action);
                let kind = match action {
                    Profiles => local_wire::OPEN_PROFILES,
                    Library => local_wire::OPEN_LIBRARY,
                    Versions => local_wire::OPEN_VERSIONS,
                    _ => local_wire::OPEN_SETTINGS,
                };
                self.local_intent = Some((kind, 0));
            }
            Local(index) => self.local_intent = Some((local_wire::ACTIVATE, index)),
            Play | TestStartupError => {
                let mode = if action == Play {
                    StartMode::Normal
                } else {
                    StartMode::TestError
                };
                self.pending = Some(mode);
                self.show(Page::Loading, None);
            }
            Profiles => self.show(
                Page::Profiles,
                Some(match self.view.profile {
                    OfflineProfile::Player => Player,
                    OfflineProfile::Guest => Guest,
                }),
            ),
            Versions => self.show(Page::Versions, None),
            Settings => self.show(Page::Settings, None),
            #[cfg(horizon_wiiu_native_mcp)]
            AgentControl if self.view.local.is_some() => {
                self.local_return = Some(AgentControl);
                self.local_intent = Some((local_wire::OPEN_AGENT, 0));
            }
            AgentControl => self.show(Page::AgentControl, None),
            // Close Horizon is available only on Home, after all sessions are gone.
            Quit => {
                self.show(Page::Exiting, None);
            }
            Player => self.view.profile = OfflineProfile::Player,
            Guest => self.view.profile = OfflineProfile::Guest,
            SwapButtons => self.view.settings.swap_buttons = !self.view.settings.swap_buttons,
            ToggleTouch => {
                self.view.settings.touch_navigation = !self.view.settings.touch_navigation;
                self.touch_ready = false;
            }
            Back => {
                let focus = match self.view.page {
                    Page::Profiles => Profiles,
                    Page::Versions => Versions,
                    Page::Settings => Settings,
                    _ => AgentControl,
                };
                self.show(Page::Home, Some(focus));
            }
            CancelLoading => {
                self.pending = None;
                if let Some(session) = self.session.as_mut() {
                    session.cancel();
                    self.update_session_view();
                } else {
                    self.show(Page::Home, None);
                }
            }
            SaveAndQuit => {
                if let Some(session) = self.session.as_mut() {
                    session.save_and_quit();
                    self.update_session_view();
                }
            }
            DismissError => self.show(Page::Home, None),
            Library | DemoResources | GameResources | PairAgent | MicrosoftSignIn => {}
        }
    }

    /// Poll one unit of work per foreground host-loop iteration, including
    /// iterations when a live game has no frame to present.
    pub fn advance(&mut self) {
        if !self.foreground {
            return;
        }
        if let (Some(mode), Some(instance)) = (self.pending.take(), self.instance) {
            let config = LaunchConfig {
                profile: Profile::Offline(OfflineName::for_builtin(self.view.profile.name())),
                instance,
                settings: self.view.settings,
            };
            match self.factory.start(config, mode) {
                Ok(driver) => self.session = Some(HostedSession::new(driver)),
                Err(error) => {
                    self.show(Page::Error, None);
                    self.view.error = Some(error);
                    return;
                }
            }
        }
        if let Some(session) = self.session.as_mut() {
            session.advance();
            self.update_session_view();
        }
    }

    fn update_session_view(&mut self) {
        let Some(status) = self.session.as_ref().map(HostedSession::status) else {
            return;
        };
        if let SessionStatus::Released(outcome) = status {
            drop(self.session.take());
            self.view.session = None;
            match outcome {
                ReturnTo::Cancelled | ReturnTo::Saved => self.show(Page::Home, None),
                ReturnTo::StartError(error) => {
                    self.show(Page::Error, None);
                    self.view.error = Some(error);
                }
            }
            return;
        }

        self.view.session = Some(status);
        let page = match status {
            SessionStatus::Preparing(_)
            | SessionStatus::Stopping(ReturnTo::Cancelled | ReturnTo::StartError(_)) => {
                Page::Loading
            }
            _ => Page::Session,
        };
        let focus_valid = match self.view.focused {
            Some(action) => self.enabled(action),
            None => !self
                .view
                .actions(page)
                .iter()
                .any(|action| self.view.row(*action).state != ActionState::Disabled),
        };
        if page != self.view.page || !focus_valid {
            self.show(page, None);
        }
    }

    pub fn close(&mut self) -> Result<(), SaveError> {
        self.pending = None;
        let result = self
            .session
            .take()
            .map_or(Ok(()), HostedSession::shutdown_for_exit);
        self.view.session = None;
        self.show(Page::Exiting, None);
        result
    }
}
