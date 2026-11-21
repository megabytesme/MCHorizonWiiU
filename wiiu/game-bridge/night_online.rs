use std::path::Path;

use horizon_wiiu_microsoft::{
    Accounts, DevicePrompt, Error, Event, Identity, ProfileId, ProfileState, Provider, Verified,
    Work,
};
use horizon_wiiu_security::{Protection, Secret, StorageMode, VaultError};

use super::local_wire::*;

const PROFILES_PER_PAGE: usize = 2;

#[derive(Clone, Copy)]
enum Destination {
    New,
    Existing(ProfileId),
}

#[derive(Clone, Copy)]
enum AfterUnlock {
    Select,
    Manage,
    Replace,
}

#[derive(Clone, Copy)]
enum Pin {
    Unlock(ProfileId, AfterUnlock),
    Set(Destination),
    Confirm(Destination),
}

#[derive(Clone, Copy)]
enum Page {
    List(usize),
    Manage(ProfileId),
    Progress,
    Store,
    Warning(Destination),
    Pin(Pin),
    SignOut(ProfileId),
}

#[derive(Clone, Copy)]
enum Action {
    Show(Page),
    Back,
    Start(Option<ProfileId>),
    Offline,
    Select(ProfileId),
    SignOut(ProfileId),
    SaveSession,
    AllowPlain(Destination),
    Keyboard,
    Manage(ProfileId),
}

struct Task {
    work: Work,
    refresh: Option<ProfileId>,
    expected: Option<ProfileId>,
    visible: bool,
}

pub struct Online {
    accounts: Result<Accounts, Error>,
    provider: Provider,
    task: Option<Task>,
    prompt: Option<DevicePrompt>,
    verified: Option<Verified>,
    first_pin: Option<Secret>,
    page: Page,
    actions: Vec<Action>,
    message: &'static str,
    progress: &'static str,
    keyboard: bool,
    automatic_attempted: bool,
}

impl Online {
    pub fn new(root: &Path, provider: Provider) -> Self {
        Self {
            accounts: Accounts::open(root),
            provider,
            task: None,
            prompt: None,
            verified: None,
            first_pin: None,
            page: Page::List(0),
            actions: Vec::new(),
            message: "",
            progress: "",
            keyboard: false,
            automatic_attempted: false,
        }
    }

    fn accounts(&mut self) -> Result<&mut Accounts, Error> {
        self.accounts.as_mut().map_err(|error| *error)
    }

    pub fn selected(&self) -> Option<ProfileId> {
        self.accounts.as_ref().ok().and_then(Accounts::selected)
    }

    pub fn active(&self) -> Option<&Identity> {
        self.accounts.as_ref().ok().and_then(Accounts::active)
    }

    pub fn selected_label(&self) -> Option<(&str, &'static str)> {
        let accounts = self.accounts.as_ref().ok()?;
        let id = accounts.selected()?;
        let profile = accounts.profile(id)?;
        Some((profile.name.as_str(), state_label(accounts.state(id))))
    }

    pub fn open(&mut self) {
        self.page = Page::List(0);
        self.message = "";
    }

    pub fn offline(&mut self) -> Result<(), Error> {
        if let Ok(accounts) = self.accounts.as_mut() {
            accounts.select_offline()?;
        }
        self.abandon();
        Ok(())
    }

    fn abandon(&mut self) {
        self.task = None;
        self.prompt = None;
        self.verified = None;
        self.first_pin = None;
        self.keyboard = false;
    }

    pub fn shutdown(&mut self) {
        self.abandon();
        self.accounts = Err(Error::Storage);
    }

    pub fn tick(&mut self) -> bool {
        if let Some(task) = &self.task {
            if let Some(event) = task.work.poll() {
                let (refresh, expected, visible) = (task.refresh, task.expected, task.visible);
                match event {
                    Event::Prompt(prompt) => self.prompt = Some(prompt),
                    Event::Verifying => {
                        self.prompt = None;
                        self.progress = tr("verifying");
                    }
                    Event::Failed(error) => {
                        self.task = None;
                        self.prompt = None;
                        if let Some(id) = refresh {
                            if let Ok(accounts) = self.accounts() {
                                accounts.record_failure(id, error);
                            }
                        }
                        self.message = error_label(error);
                        if visible {
                            self.page = refresh.map_or(Page::List(0), Page::Manage);
                        }
                    }
                    Event::Verified(verified) => {
                        self.task = None;
                        self.prompt = None;
                        let id = verified.identity().id;
                        if expected.is_some_and(|expected| expected != id) {
                            self.page = Page::Manage(expected.unwrap());
                            self.message = error_label(Error::IdentityChanged);
                        } else if let Some(id) = refresh {
                            let result = self
                                .accounts()
                                .and_then(|accounts| accounts.accept(verified, refresh, None));
                            if visible {
                                self.page = Page::Manage(id);
                            }
                            self.message = result.map_or_else(error_label, |()| tr("verified"));
                        } else {
                            self.verified = Some(verified);
                            match self.accounts.as_ref().ok().map(|a| a.state(id)) {
                                Some(ProfileState::Locked) => {
                                    self.page = Page::Pin(Pin::Unlock(id, AfterUnlock::Replace));
                                }
                                Some(ProfileState::SignedIn(
                                    StorageMode::PinProtected | StorageMode::Unprotected,
                                ))
                                | Some(ProfileState::Saved(_))
                                | Some(ProfileState::Reauthenticate)
                                    if self.accounts.as_ref().ok().and_then(|a| a.mode(id))
                                        != Some(StorageMode::SessionOnly) =>
                                {
                                    let result = self.finish(None);
                                    self.message =
                                        result.map_or_else(error_label, |()| tr("verified"));
                                }
                                _ => self.page = Page::Store,
                            }
                        }
                    }
                }
                return true;
            }
            if self
                .prompt
                .as_ref()
                .is_some_and(|prompt| prompt.instructions().is_none())
            {
                self.abandon();
                self.page = Page::List(0);
                self.message = tr("expired");
                return true;
            }
            return false;
        }
        if self.active().is_some() {
            self.automatic_attempted = false;
            return false;
        }
        if !self.automatic_attempted && self.verified.is_none() && self.provider.configured() {
            if let Some(id) = self.selected() {
                self.automatic_attempted = true;
                if self
                    .accounts
                    .as_ref()
                    .is_ok_and(|accounts| matches!(accounts.state(id), ProfileState::Saved(_)))
                {
                    match self.refresh(id, false) {
                        Ok(()) => return true,
                        Err(error) => {
                            if let Ok(accounts) = self.accounts() {
                                accounts.record_failure(id, error);
                            }
                            self.message = error_label(error);
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Returns true when the account page should close.
    pub fn command(&mut self, action: u8, index: u8, value: &str) -> bool {
        self.message = "";
        let result = match action {
            BACK => return self.back(),
            TEXT_ACCEPTED => self.accept_pin(value.as_bytes()),
            TEXT_CANCELLED => Ok(()),
            TEXT_UNAVAILABLE => {
                self.message = tr("keyboard_unavailable");
                Ok(())
            }
            ACTIVATE => {
                let Some(action) = self.actions.get(index as usize).copied() else {
                    return false;
                };
                match action {
                    Action::Back => return self.back(),
                    Action::Offline => {
                        let result = self.offline();
                        if result.is_ok() {
                            return true;
                        }
                        result
                    }
                    action => self.apply(action),
                }
            }
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.message = error_label(error);
        }
        false
    }

    fn back(&mut self) -> bool {
        let previous = self.page;
        match previous {
            Page::List(0) => return true,
            Page::List(page) => self.page = Page::List(page - 1),
            Page::Warning(Destination::New)
            | Page::Pin(Pin::Set(Destination::New))
            | Page::Pin(Pin::Confirm(Destination::New)) => {
                self.first_pin = None;
                self.page = Page::Store;
            }
            Page::SignOut(id)
            | Page::Pin(Pin::Unlock(id, AfterUnlock::Select | AfterUnlock::Manage))
            | Page::Warning(Destination::Existing(id))
            | Page::Pin(Pin::Set(Destination::Existing(id)))
            | Page::Pin(Pin::Confirm(Destination::Existing(id))) => {
                self.first_pin = None;
                self.page = Page::Manage(id);
            }
            _ => {
                self.abandon();
                self.page = Page::List(0);
            }
        }
        false
    }

    fn refresh(&mut self, id: ProfileId, visible: bool) -> Result<(), Error> {
        let provider = self.provider.clone();
        let task = self.accounts()?.refresh(id, &provider)?;
        self.task = Some(Task {
            work: task,
            refresh: Some(id),
            expected: Some(id),
            visible,
        });
        if self.selected() == Some(id) {
            self.automatic_attempted = true;
        }
        if visible {
            self.progress = tr("refreshing");
            self.page = Page::Progress;
        }
        Ok(())
    }

    fn select(&mut self, id: ProfileId) -> Result<(), Error> {
        if self.accounts()?.state(id) == ProfileState::Locked {
            self.page = Page::Pin(Pin::Unlock(id, AfterUnlock::Select));
        } else if self.accounts()?.select_current(id)? {
            self.message = tr("verified");
        } else {
            self.refresh(id, true)?;
        }
        Ok(())
    }

    fn apply(&mut self, action: Action) -> Result<(), Error> {
        match action {
            Action::Show(page) => self.page = page,
            Action::Start(expected) => {
                self.abandon();
                let work = self.provider.start()?;
                self.task = Some(Task {
                    work,
                    refresh: None,
                    expected,
                    visible: true,
                });
                self.progress = tr("starting");
                self.page = Page::Progress;
            }
            Action::Select(id) => return self.select(id),
            Action::Manage(id) => {
                self.page = if self.accounts()?.state(id) == ProfileState::Locked {
                    Page::Pin(Pin::Unlock(id, AfterUnlock::Manage))
                } else {
                    Page::Manage(id)
                };
            }
            Action::AllowPlain(destination) => {
                self.protect(destination, Protection::ConfirmedUnprotected)?;
            }
            Action::SaveSession => self.finish(Some(Protection::SessionOnly))?,
            Action::SignOut(id) => {
                self.abandon();
                self.accounts()?.sign_out(id)?;
                self.page = Page::List(0);
                self.message = tr("signed_out");
            }
            Action::Keyboard => self.keyboard = true,
            Action::Offline | Action::Back => {}
        }
        Ok(())
    }

    fn accept_pin(&mut self, pin: &[u8]) -> Result<(), Error> {
        let Page::Pin(task) = self.page else {
            return Ok(());
        };
        match task {
            Pin::Unlock(id, after) => {
                self.accounts()?.unlock(id, pin)?;
                self.page = Page::Manage(id);
                match after {
                    AfterUnlock::Select => self.select(id),
                    AfterUnlock::Manage => Ok(()),
                    AfterUnlock::Replace => self.finish(None),
                }
            }
            Pin::Set(destination) => {
                if !(6..=128).contains(&pin.len()) {
                    return Err(VaultError::InvalidPinLength.into());
                }
                self.first_pin = Some(Secret::new(pin.to_vec()));
                self.page = Page::Pin(Pin::Confirm(destination));
                Ok(())
            }
            Pin::Confirm(destination) => {
                let first = self.first_pin.take().ok_or(Error::Storage)?;
                if first.expose() != pin {
                    self.page = Page::Pin(Pin::Set(destination));
                    self.message = tr("pin_mismatch");
                    return Ok(());
                }
                self.protect(destination, Protection::PinProtected { new_pin: pin })
            }
        }
    }

    fn protect(&mut self, destination: Destination, policy: Protection<'_>) -> Result<(), Error> {
        match destination {
            Destination::New => self.finish(Some(policy)),
            Destination::Existing(id) => {
                self.page = Page::Manage(id);
                self.message = if self.accounts()?.change_protection(id, policy)? {
                    tr("cleanup")
                } else {
                    tr("protection_saved")
                };
                Ok(())
            }
        }
    }

    fn finish(&mut self, policy: Option<Protection<'_>>) -> Result<(), Error> {
        let verified = self.verified.take().ok_or(Error::Storage)?;
        let id = verified.identity().id;
        self.page = Page::List(0);
        self.accounts()?.accept(verified, None, policy)?;
        self.page = Page::Manage(id);
        self.message = tr("verified");
        Ok(())
    }

    pub fn model(&mut self, menu: &mut LocalMenu) {
        self.actions.clear();
        let accounts = self.accounts.as_ref().ok();
        let selected = accounts.and_then(Accounts::selected);
        let mut row = |label: &str, detail: &str, state, action| {
            let index = self.actions.len();
            if let Some(dest) = menu.rows.get_mut(index) {
                *dest = LocalRow {
                    label: Text::new(label),
                    detail: Text::new(detail),
                    state,
                };
                self.actions.push(action);
            }
        };
        let (title, description) = match self.page {
            Page::List(page) => {
                if let Some(accounts) = accounts {
                    for profile in accounts
                        .profiles()
                        .skip(page * PROFILES_PER_PAGE)
                        .take(PROFILES_PER_PAGE)
                    {
                        row(
                            &profile.name,
                            state_label(accounts.state(profile.id)),
                            if selected == Some(profile.id) {
                                SELECTED
                            } else {
                                ACTIVE
                            },
                            Action::Show(Page::Manage(profile.id)),
                        );
                    }
                    if accounts.profiles().count() > (page + 1) * PROFILES_PER_PAGE {
                        row(tr("next"), "", ACTIVE, Action::Show(Page::List(page + 1)));
                    }
                }
                if page == 0 {
                    row(
                        tr("sign_in"),
                        "",
                        if self.provider.configured() && accounts.is_some() {
                            ACTIVE
                        } else {
                            DISABLED
                        },
                        Action::Start(None),
                    );
                }
                if selected.is_some() && page == 0 {
                    row(tr("offline"), "", ACTIVE, Action::Offline);
                }
                row(tr("back"), "", ACTIVE, Action::Back);
                (
                    tr("accounts"),
                    if accounts.is_none() {
                        tr("storage_error")
                    } else if !self.provider.configured() {
                        tr("unconfigured")
                    } else {
                        tr("choose_profile")
                    },
                )
            }
            Page::Manage(id) => {
                let profile = accounts.and_then(|accounts| accounts.profile(id));
                let state = accounts.map_or(ProfileState::Unavailable, |a| a.state(id));
                let bad = state == ProfileState::Unavailable;
                if state == ProfileState::Reauthenticate {
                    row(
                        tr("sign_in_again"),
                        tr("use_same_browser_account"),
                        ACTIVE,
                        Action::Start(Some(id)),
                    );
                } else {
                    row(
                        tr("use_account"),
                        profile.map_or("", |p| &p.name),
                        if bad { DISABLED } else { ACTIVE },
                        Action::Select(id),
                    );
                }
                match accounts.and_then(|a| a.mode(id)) {
                    Some(StorageMode::PinProtected) => {
                        if state == ProfileState::Locked {
                            row(tr("unlock_to_manage"), "", ACTIVE, Action::Manage(id));
                        } else {
                            row(
                                tr("change_pin"),
                                "",
                                ACTIVE,
                                Action::Show(Page::Pin(Pin::Set(Destination::Existing(id)))),
                            );
                            row(
                                tr("disable_pin"),
                                "",
                                ACTIVE,
                                Action::Show(Page::Warning(Destination::Existing(id))),
                            );
                        }
                    }
                    Some(StorageMode::Unprotected) => {
                        row(
                            tr("enable_pin"),
                            "",
                            ACTIVE,
                            Action::Show(Page::Pin(Pin::Set(Destination::Existing(id)))),
                        );
                    }
                    _ => {}
                }
                row(
                    tr("sign_out"),
                    tr("remove_refresh"),
                    if bad { DISABLED } else { ACTIVE },
                    Action::Show(Page::SignOut(id)),
                );
                row(tr("back"), "", ACTIVE, Action::Back);
                (tr("account"), state_label(state))
            }
            Page::Progress => {
                if let Some((uri, code)) = self.prompt.as_ref().and_then(DevicePrompt::instructions)
                {
                    row(tr("open_on_phone"), uri, DISABLED, Action::Back);
                    row(tr("code"), code, DISABLED, Action::Back);
                }
                row(tr("cancel"), "", ACTIVE, Action::Back);
                (
                    tr("microsoft"),
                    if self.prompt.is_some() {
                        tr("waiting")
                    } else {
                        self.progress
                    },
                )
            }
            Page::Store => {
                row(
                    tr("pin"),
                    tr("pin_detail"),
                    ACTIVE,
                    Action::Show(Page::Pin(Pin::Set(Destination::New))),
                );
                row(
                    tr("no_pin"),
                    tr("no_pin_detail"),
                    ACTIVE,
                    Action::Show(Page::Warning(Destination::New)),
                );
                row(
                    tr("session"),
                    tr("session_detail"),
                    ACTIVE,
                    Action::SaveSession,
                );
                row(tr("cancel"), "", ACTIVE, Action::Back);
                (tr("save_account"), tr("verified"))
            }
            Page::Warning(destination) => {
                row(
                    tr("sd_warning"),
                    tr("sd_warning_detail"),
                    DISABLED,
                    Action::Back,
                );
                row(
                    tr("impersonation"),
                    tr("impersonation_detail"),
                    DISABLED,
                    Action::Back,
                );
                row(
                    tr("plaintext"),
                    tr("can_enable_later"),
                    DISABLED,
                    Action::Back,
                );
                row(tr("cancel"), "", ACTIVE, Action::Back);
                row(
                    tr("confirm_plain"),
                    "",
                    ACTIVE,
                    Action::AllowPlain(destination),
                );
                (tr("without_pin"), tr("review_warning"))
            }
            Page::Pin(task) => {
                row(tr("keyboard"), "", ACTIVE, Action::Keyboard);
                row(tr("cancel"), "", ACTIVE, Action::Back);
                if std::mem::take(&mut self.keyboard) {
                    menu.input_limit = 128;
                    menu.mode |= PRIVATE_INPUT;
                }
                match task {
                    Pin::Unlock(..) => (tr("unlock"), tr("enter_existing_pin")),
                    Pin::Set(_) => (tr("set_pin"), tr("pin_warning")),
                    Pin::Confirm(_) => (tr("confirm_pin"), tr("repeat_pin")),
                }
            }
            Page::SignOut(id) => {
                row(tr("cancel"), "", ACTIVE, Action::Back);
                row(tr("confirm_sign_out"), "", ACTIVE, Action::SignOut(id));
                (tr("sign_out"), tr("sign_out_detail"))
            }
        };
        menu.title = Text::new(title);
        menu.description = Text::new(if self.message.is_empty() {
            description
        } else {
            self.message
        });
        menu.count = self.actions.len() as u8;
    }

    pub fn status_message(&self) -> &str {
        self.message
    }
}

fn state_label(state: ProfileState) -> &'static str {
    match state {
        ProfileState::Locked => tr("locked"),
        ProfileState::Saved(StorageMode::PinProtected) => tr("pin_saved"),
        ProfileState::Saved(StorageMode::Unprotected) => tr("plain_saved"),
        ProfileState::Saved(StorageMode::SessionOnly) => tr("session"),
        ProfileState::SignedIn(StorageMode::PinProtected) => tr("signed_pin"),
        ProfileState::SignedIn(StorageMode::Unprotected) => tr("signed_plain"),
        ProfileState::SignedIn(StorageMode::SessionOnly) => tr("signed_session"),
        ProfileState::Reauthenticate => tr("reauthenticate"),
        ProfileState::Unavailable => tr("storage_error"),
    }
}

pub fn error_label(error: Error) -> &'static str {
    match error {
        Error::Unconfigured => tr("unconfigured"),
        Error::Network => tr("network_error"),
        Error::Protocol => tr("service_error"),
        Error::Reauthenticate => tr("reauthenticate"),
        Error::Denied => tr("denied"),
        Error::CodeExpired => tr("expired"),
        Error::XboxProfile => tr("xbox_profile"),
        Error::XboxFamily => tr("xbox_family"),
        Error::XboxUnavailable => tr("xbox_unavailable"),
        Error::Registration => tr("registration"),
        Error::OwnershipMissing => tr("no_ownership"),
        Error::ProfileMissing => tr("no_profile"),
        Error::IdentityChanged => tr("identity_changed"),
        Error::Storage => tr("storage_error"),
        Error::Vault(error) => match error {
            VaultError::AuthenticationFailed => tr("pin_failure"),
            VaultError::Corrupt => tr("corrupt"),
            VaultError::Locked => tr("locked"),
            VaultError::InvalidPinLength => tr("pin_size"),
            VaultError::EntropyUnavailable => tr("entropy"),
            VaultError::UnsupportedFormat => tr("vault_version"),
            _ => tr("storage_error"),
        },
    }
}

// English fallback for Night's account interface. Keep keys independent of
// the rendered labels so this table can be supplied by a locale pack.
pub fn tr(key: &str) -> &'static str {
    match key {
        "account" => "Microsoft account",
        "accounts" => "Microsoft accounts",
        "back" => "Back",
        "can_enable_later" => "You can enable PIN protection later",
        "cancel" => "Cancel",
        "change_pin" => "Change PIN",
        "choose_profile" => "Choose a profile or add a Microsoft account.",
        "cleanup" => "Protection saved. An old copy remains on the SD card.",
        "code" => "Enter this code",
        "confirm_pin" => "Confirm PIN or passphrase",
        "confirm_plain" => "Save without encryption",
        "confirm_sign_out" => "Sign out and remove credential",
        "corrupt" => "The saved credential is damaged.",
        "denied" => "Microsoft sign-in was declined.",
        "disable_pin" => "Disable PIN",
        "enable_pin" => "Enable PIN",
        "enter_existing_pin" => "Enter this account's PIN or passphrase.",
        "entropy" => "Secure randomness is unavailable.",
        "expired" => "The device code expired. Start sign-in again.",
        "identity_changed" => "Microsoft returned a different profile. Sign in again.",
        "impersonation" => "A copy can enable impersonation",
        "impersonation_detail" => "Someone could act as your account or MCP server",
        "keyboard" => "Open system keyboard",
        "keyboard_unavailable" => "The system keyboard is unavailable.",
        "locked" => "Locked. Enter PIN to use this account.",
        "microsoft" => "Microsoft sign-in",
        "network_error" => "Cannot reach sign-in services. Check the network and console time.",
        "next" => "Next page",
        "no_ownership" => "This account has no Minecraft Java entitlement.",
        "no_pin" => "No PIN",
        "no_pin_detail" => "Restore after restart with a readable SD credential",
        "no_profile" => "Create a Java profile on minecraft.net, then sign in again.",
        "offline" => "Use offline profile",
        "open_on_phone" => "Open this page on another device",
        "pin" => "PIN protection",
        "pin_detail" => "Recommended. Enter your PIN after restarting",
        "pin_failure" => "The PIN or protected credential could not be verified.",
        "pin_mismatch" => "The entries differed. Enter the PIN or passphrase again.",
        "pin_saved" => "Saved with PIN protection",
        "pin_size" => "Use 6 to 128 bytes for your PIN or passphrase.",
        "pin_warning" => {
            "Use a long passphrase. Short PINs can be guessed offline without attempt limits."
        }
        "plain_saved" => "Saved. No PIN",
        "plaintext" => "No application-level encryption",
        "protection_saved" => "Storage protection updated.",
        "reauthenticate" => "Sign-in expired or was revoked. Sign in again.",
        "refreshing" => "Refreshing and checking Minecraft Java ownership.",
        "registration" => "Microsoft or Minecraft rejected this app registration.",
        "remove_refresh" => "Remove the stored credential from this console",
        "repeat_pin" => "Re-enter the same PIN or passphrase.",
        "review_warning" => "Review what saving without a PIN means for SD storage.",
        "save_account" => "Save Microsoft account",
        "sd_warning" => "SD card copies can reveal secrets",
        "sd_warning_detail" => "Authentication tokens or server private keys",
        "service_error" => "Unexpected response from a sign-in service.",
        "session" => "This session only",
        "session_detail" => "Sign in again after closing the app",
        "set_pin" => "Set PIN or passphrase",
        "sign_in" => "Sign in with Microsoft",
        "sign_in_again" => "Sign in again",
        "sign_out" => "Sign out",
        "sign_out_detail" => "Remove the saved account and refresh credential from this console.",
        "signed_out" => "Signed out. Stored credential removed.",
        "signed_pin" => "Signed in. PIN protected",
        "signed_plain" => "Signed in. No PIN",
        "signed_session" => "Signed in. This session only",
        "starting" => "Requesting a device code.",
        "storage_error" => "Account storage is unavailable or damaged.",
        "unconfigured" => "This build has no Microsoft application registration.",
        "unlock" => "Unlock account",
        "unlock_to_manage" => "Unlock to manage protection",
        "use_account" => "Use this account",
        "use_same_browser_account" => "Use the same Microsoft account in your browser",
        "vault_version" => "This vault needs a different application version.",
        "verified" => "Minecraft Java ownership verified.",
        "verifying" => "Checking Xbox and Minecraft Java ownership.",
        "waiting" => "Waiting for approval on the other device.",
        "without_pin" => "Save without a PIN?",
        "xbox_family" => "Xbox requires age verification or family approval.",
        "xbox_profile" => "Create an Xbox profile before signing in here.",
        "xbox_unavailable" => "Xbox could not authorize this account.",
        _ => "",
    }
}
