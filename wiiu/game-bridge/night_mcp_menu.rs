use super::{change_policy, load_policy, wire, NightTools};
use horizon_wiiu_mcp::service::Service;
use horizon_wiiu_security::{Protection, StorageMode};
use std::path::Path;
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

const PORT: u16 = 8443;

#[derive(Clone)]
enum Page {
    Main,
    Settings,
    Pin(PinMode),
    NoPin,
    Rotate,
    Requests(usize),
    Clients(usize),
    Client(String, usize),
    Consent(String, usize),
    Identity,
}

#[derive(Clone, Copy)]
enum PinMode {
    Set,
    Unlock,
}

#[derive(Clone)]
enum Action {
    Screen(Page),
    Back,
    Pair,
    AllowPlay,
    AllowGame,
    ToggleService,
    ToggleNoAi,
    Restart,
    Approve(String, bool),
    Revoke(String),
    ConfirmNoPin,
    ConfirmRotation,
    AddDigit,
    EraseDigit,
    SubmitPin(PinMode),
}

pub(super) struct Menu {
    pub(super) service: Option<Service<NightTools>>,
    page: Option<Page>,
    actions: Vec<Option<Action>>,
    rows: Vec<wire::LocalRow>,
    message: Option<String>,
    pin: Zeroizing<Vec<u8>>,
    digit: u8,
    next_unlock: Option<Instant>,
}

impl Menu {
    pub(super) fn open(root: &Path) -> Self {
        let (service, message) = match Service::open(root, NightTools) {
            Ok(service) => (Some(service), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let mut menu = Self {
            service,
            page: None,
            actions: Vec::new(),
            rows: Vec::new(),
            message,
            pin: Zeroizing::new(Vec::with_capacity(16)),
            digit: 0,
            next_unlock: None,
        };
        if let Some(service) = &menu.service {
            if let Ok(policy) = service.policy() {
                load_policy(policy);
                if policy.enabled && !policy.no_ai {
                    menu.connect();
                }
            }
        }
        menu
    }

    pub(super) fn shutdown(&mut self) {
        if let Some(service) = self.service.as_mut() {
            service.shutdown();
        }
        self.erase_pin();
    }

    fn connect(&mut self) {
        let Some(service) = self.service.as_mut() else {
            return;
        };
        let result = (|| {
            let address = horizon_client_platform::cafe_net::assigned_ipv4()?;
            let hostname = optional_hostname()?;
            service.connect(address, PORT, hostname.as_deref(), false)
        })();
        self.message = result.err().map(|error| error.to_string());
    }

    fn erase_pin(&mut self) {
        self.pin.fill(0);
        self.pin.clear();
        self.digit = 0;
    }

    pub(super) fn is_open(&self) -> bool {
        self.page.is_some()
    }

    pub(super) fn update(
        &mut self,
        command: u8,
        row: u8,
        mut base: wire::LocalMenu,
    ) -> Option<wire::LocalMenu> {
        if command == wire::OPEN_AGENT {
            self.page = Some(Page::Main);
        } else {
            let page = self.page.clone()?;
            match command {
                wire::BACK => {
                    self.erase_pin();
                    self.page = match page {
                        Page::Main => None,
                        Page::Pin(_) | Page::NoPin | Page::Rotate => Some(Page::Settings),
                        Page::Client(_, p) => Some(Page::Clients(p)),
                        Page::Consent(_, p) => Some(Page::Requests(p)),
                        _ => Some(Page::Main),
                    };
                    if self.page.is_none() {
                        base.title = wire::Text::default();
                        return Some(base);
                    }
                }
                wire::LEFT | wire::RIGHT if matches!(page, Page::Pin(_)) => {
                    self.digit = (self.digit + if command == wire::RIGHT { 1 } else { 9 }) % 10;
                }
                wire::ACTIVATE => {
                    if let Some(Some(action)) = self.actions.get(row as usize).cloned() {
                        self.apply(action);
                    }
                }
                _ => {}
            }
        }
        let Some(page) = self.page.clone() else {
            base.title = wire::Text::default();
            return Some(base);
        };
        self.rows.clear();
        self.actions.clear();
        let (title, description) = self.render(page);
        base.title = wire::Text::new(title);
        base.description = wire::Text::new(self.message.as_deref().unwrap_or(description));
        base.rows.fill(wire::LocalRow::default());
        for (row, item) in base.rows.iter_mut().zip(self.rows.iter()) {
            *row = *item;
        }
        base.count = self.rows.len() as u8;
        base.mode = if matches!(self.page, Some(Page::Pin(_))) {
            wire::EDITOR
        } else {
            0
        };
        base.input = wire::Text::default();
        base.input_limit = 0;
        Some(base)
    }

    fn row(&mut self, label: &str, detail: &str, action: Option<Action>) {
        if self.rows.len() < wire::ROWS {
            self.rows.push(wire::LocalRow {
                label: wire::Text::new(label),
                detail: wire::Text::new(detail),
                state: if action.is_some() {
                    wire::ACTIVE
                } else {
                    wire::DISABLED
                },
            });
            self.actions.push(action);
        }
    }

    fn page_row(&mut self, label: &str, detail: &str, page: Page) {
        self.row(label, detail, Some(Action::Screen(page)));
    }

    fn render(&mut self, page: Page) -> (&'static str, &'static str) {
        if self.service.is_none() {
            self.row("Back", "", Some(Action::Back));
            return ("Agent Control", "The credential vault is unavailable.");
        }
        let service = self.service.as_ref().unwrap();
        let storage = service.status();
        let policy = service.policy().unwrap_or_default();
        let connection = service
            .connection()
            .map(|(url, hash)| (url.to_owned(), hash.to_owned()));
        match page {
            Page::Main => {
                let status = if storage.cleanup_pending {
                    "SD cleanup pending"
                } else if !storage.initialized {
                    "Choose how to store secrets"
                } else if storage.locked {
                    "PIN protected / locked"
                } else if storage.mode == StorageMode::Unprotected {
                    "Automatic unlock"
                } else {
                    "PIN protected / unlocked"
                };
                if let Some((endpoint, _)) = connection {
                    self.page_row(
                        "HTTPS connection",
                        &format!("{endpoint}/mcp"),
                        Page::Identity,
                    );
                    self.row(
                        "Pair a PC",
                        "Open a three-minute pairing window",
                        Some(Action::Pair),
                    );
                    let pending = self
                        .service
                        .as_ref()
                        .unwrap()
                        .local(|auth, now| auth.pending(now).count())
                        .unwrap_or_default();
                    self.page_row(
                        "Authorization requests",
                        &format!("{pending} waiting"),
                        Page::Requests(0),
                    );
                    self.page_row(
                        "Trusted clients",
                        "Review grants and revoke access",
                        Page::Clients(0),
                    );
                } else if storage.locked {
                    self.page_row(
                        "Unlock Agent Control",
                        "Use the local PIN",
                        Page::Pin(PinMode::Unlock),
                    );
                } else if policy.enabled && !policy.no_ai {
                    self.row(
                        "Try HTTPS again",
                        "Use the console LAN address",
                        Some(Action::Restart),
                    );
                }
                self.page_row("Agent Control settings", status, Page::Settings);
                self.row("Back", "", Some(Action::Back));
                (
                    "Agent Control",
                    if policy.no_ai {
                        "No AI is on."
                    } else {
                        "Connect a trusted MCP client over HTTPS."
                    },
                )
            }
            Page::Settings => {
                if !storage.initialized {
                    self.page_row(
                        "Set a PIN",
                        "Unlock locally after each cold start",
                        Page::Pin(PinMode::Set),
                    );
                    self.page_row(
                        "Use no PIN",
                        "Starts automatically after a restart",
                        Page::NoPin,
                    );
                } else if storage.locked {
                    self.page_row("Unlock", "Local PIN required", Page::Pin(PinMode::Unlock));
                } else {
                    self.row(
                        "Agent Control",
                        on_off(policy.enabled),
                        Some(Action::ToggleService),
                    );
                    self.row(
                        "Allow game control",
                        on_off(policy.allow_game),
                        Some(Action::AllowGame),
                    );
                    self.row(
                        "Allow agent game launch",
                        on_off(policy.allow_launch),
                        Some(Action::AllowPlay),
                    );
                    self.row(
                        "No AI / Emergency disable",
                        on_off(policy.no_ai),
                        Some(Action::ToggleNoAi),
                    );
                    self.page_row(
                        "HTTPS identity and storage",
                        "Trust, PIN and rotation",
                        Page::Identity,
                    );
                }
                self.row("Back", "", Some(Action::Back));
                (
                    "Agent Control settings",
                    "These permissions are controlled on the Wii U.",
                )
            }
            Page::Identity => {
                if let Some((_, digest)) = connection {
                    let (a, b) = digest.split_at(48);
                    self.row("CA SHA-256 1/2", a, None);
                    self.row("CA SHA-256 2/2", b, None);
                }
                self.page_row(
                    "Set or change PIN",
                    "Restart requires a local unlock",
                    Page::Pin(PinMode::Set),
                );
                self.page_row(
                    "Use no PIN",
                    "Confirm SD exposure before switching",
                    Page::NoPin,
                );
                self.page_row(
                    "Rotate HTTPS and revoke all",
                    "Requires installing the new CA",
                    Page::Rotate,
                );
                self.row("Back", "", Some(Action::Back));
                (
                    "HTTPS identity",
                    "Copy launcher/agent-control-ca.pem and verify its SHA-256.",
                )
            }
            Page::Pin(mode) => {
                self.row("PIN", &"*".repeat(self.pin.len()), None);
                self.row(
                    "Add digit",
                    &format!("< {} >", self.digit),
                    Some(Action::AddDigit),
                );
                self.row("Erase last digit", "", Some(Action::EraseDigit));
                self.row("Confirm PIN", "", Some(Action::SubmitPin(mode)));
                self.row("Cancel", "", Some(Action::Back));
                (
                    "Local PIN",
                    "Left and right choose a digit. Use at least six digits.",
                )
            }
            Page::NoPin => {
                self.row(
                    "Confirm automatic unlock",
                    "Secrets are unencrypted on the SD card",
                    Some(Action::ConfirmNoPin),
                );
                self.row("Cancel", "", Some(Action::Back));
                ("Use no PIN", "A copy of the SD card can expose keys and let someone impersonate this console.")
            }
            Page::Rotate => {
                self.row(
                    "Rotate and revoke all",
                    "PCs must install the new CA and pair again",
                    Some(Action::ConfirmRotation),
                );
                self.row("Cancel", "", Some(Action::Back));
                (
                    "Rotate HTTPS identity",
                    "Replace this console's HTTPS identity and revoke its trusted clients.",
                )
            }
            Page::Requests(skip) => {
                let pending = service
                    .local(|auth, now| auth.pending(now).collect::<Vec<_>>())
                    .unwrap_or_default();
                let count = pending.len();
                for consent in pending.into_iter().skip(skip).take(4) {
                    self.page_row(
                        &consent.client_name,
                        &format!("Pairing {}", prefix(&consent.id)),
                        Page::Consent(consent.id, skip),
                    );
                }
                if count == 0 {
                    self.row("No pending requests", "", None);
                }
                self.row("Back", "", Some(Action::Back));
                (
                    "Authorization requests",
                    "Only approve a matching browser request you started.",
                )
            }
            Page::Clients(skip) => {
                let clients = service
                    .local(|auth, _| auth.trusted().collect::<Vec<_>>())
                    .unwrap_or_default();
                let count = clients.len();
                for client in clients.into_iter().skip(skip).take(3) {
                    self.page_row(
                        &client.name,
                        &client.scopes.as_text(),
                        Page::Client(client.client_id, skip),
                    );
                }
                if count == 0 {
                    self.row("No trusted clients", "", None);
                }
                if skip > 0 {
                    self.page_row("Previous clients", "", Page::Clients(skip - 3));
                }
                if count > skip + 3 {
                    self.page_row("More clients", "", Page::Clients(skip + 3));
                }
                self.row("Back", "", Some(Action::Back));
                (
                    "Trusted clients",
                    "Select a PC to inspect or revoke its authorization.",
                )
            }
            Page::Consent(id, _) => {
                let pending = service
                    .local(|auth, now| auth.pending(now).find(|p| p.id == id))
                    .flatten();
                if let Some(pending) = pending {
                    self.row(
                        &pending.client_name,
                        &format!("Pairing {}", prefix(&id)),
                        None,
                    );
                    self.row("Redirect URI", &pending.redirect_uri, None);
                    self.row("Requested scopes", &pending.scopes.as_text(), None);
                    self.row(
                        "Approve on this Wii U",
                        "",
                        Some(Action::Approve(id.clone(), true)),
                    );
                    self.row("Deny", "", Some(Action::Approve(id, false)));
                }
                self.row("Back", "", Some(Action::Back));
                (
                    "Review authorization",
                    "Compare the pairing ID with the one shown in your browser.",
                )
            }
            Page::Client(id, _) => {
                let client = service
                    .local(|auth, now| {
                        auth.trusted()
                            .find(|p| p.client_id == id)
                            .map(|client| (client, now))
                    })
                    .flatten();
                if let Some((client, now)) = client {
                    self.row(&client.name, &client.scopes.as_text(), None);
                    self.row(
                        "Last authenticated activity",
                        &format!("{} seconds ago", now.saturating_sub(client.last_activity)),
                        None,
                    );
                    self.row(
                        "Revoke this client's access",
                        "Access and refresh tokens stop working",
                        Some(Action::Revoke(id)),
                    );
                }
                self.row("Back", "", Some(Action::Back));
                ("Trusted client", "Revocation takes effect immediately.")
            }
        }
    }

    fn apply(&mut self, action: Action) {
        self.message = None;
        match action {
            Action::Screen(page) => {
                self.erase_pin();
                self.page = Some(page);
            }
            Action::Back => {
                let _ = self.update(wire::BACK, 0, wire::LocalMenu::default());
            }
            Action::AddDigit if self.pin.len() < 16 => self.pin.push(b'0' + self.digit),
            Action::EraseDigit => {
                if let Some(last) = self.pin.last_mut() {
                    *last = 0;
                }
                self.pin.pop();
            }
            Action::AddDigit => {}
            Action::SubmitPin(mode) => self.submit_pin(mode),
            Action::Restart => self.connect(),
            other => self.apply_policy_action(other),
        }
    }

    fn submit_pin(&mut self, mode: PinMode) {
        if self.pin.len() < 6 {
            self.message = Some("Enter at least six digits.".into());
            return;
        }
        if matches!(mode, PinMode::Unlock)
            && self.next_unlock.is_some_and(|time| time > Instant::now())
        {
            self.message = Some("Wait briefly before trying another PIN.".into());
            return;
        }
        let Some(service) = self.service.as_mut() else {
            return;
        };
        let new = !service.status().initialized;
        let result = match mode {
            PinMode::Set => service.protect(Protection::PinProtected { new_pin: &self.pin }),
            PinMode::Unlock => service.unlock(&self.pin),
        };
        self.erase_pin();
        match result {
            Ok(()) => self.after_unlock(new),
            Err(error) => {
                self.next_unlock = Some(Instant::now() + Duration::from_secs(3));
                self.message = Some(error.to_string());
            }
        }
    }

    fn after_unlock(&mut self, newly_initialized: bool) {
        if let Some(service) = self.service.as_mut() {
            match service.policy() {
                Ok(mut policy) => {
                    if newly_initialized {
                        policy.enabled = true;
                    }
                    if newly_initialized && !change_policy(service, policy) {
                        self.message = Some("Could not save Agent Control settings.".into());
                        return;
                    }
                    load_policy(policy);
                    self.page = Some(Page::Main);
                    if policy.enabled && !policy.no_ai {
                        self.connect();
                    }
                }
                Err(error) => self.message = Some(error.to_string()),
            }
        }
    }

    fn apply_policy_action(&mut self, action: Action) {
        let Some(service) = self.service.as_mut() else {
            return;
        };
        match action {
            Action::Pair => {
                service.local(|auth, now| auth.open_pairing(now));
                self.message = Some("Pairing is open. Continue in your browser.".into());
            }
            Action::Approve(id, yes) => {
                let result = service.local(|auth, now| auth.decide(now, &id, yes));
                self.message = Some(
                    if result == Some(Ok(())) {
                        "Decision sent to your browser."
                    } else {
                        "The request expired."
                    }
                    .into(),
                );
                self.page = Some(Page::Requests(0));
            }
            Action::Revoke(id) => {
                let revoked = service.local(|auth, _| auth.revoke_client(&id)) == Some(Ok(()));
                self.page = Some(Page::Clients(0));
                self.message = Some(
                    if revoked {
                        "Client access revoked."
                    } else {
                        "Could not save revocation. Agent Control is blocked."
                    }
                    .into(),
                );
            }
            Action::ConfirmNoPin => {
                let new = !service.status().initialized;
                match service.protect(Protection::ConfirmedUnprotected) {
                    Ok(()) => self.after_unlock(new),
                    Err(error) => self.message = Some(error.to_string()),
                }
            }
            Action::ConfirmRotation => match service.rotate() {
                Ok(()) => {
                    self.page = Some(Page::Main);
                    self.connect();
                }
                Err(error) => self.message = Some(error.to_string()),
            },
            Action::AllowPlay | Action::AllowGame | Action::ToggleService | Action::ToggleNoAi => {
                let Ok(mut policy) = service.policy() else {
                    return;
                };
                match action {
                    Action::AllowPlay => policy.allow_launch = !policy.allow_launch,
                    Action::AllowGame => policy.allow_game = !policy.allow_game,
                    Action::ToggleService => policy.enabled = !policy.enabled,
                    Action::ToggleNoAi => policy.no_ai = !policy.no_ai,
                    _ => unreachable!(),
                }
                if !change_policy(service, policy) {
                    self.message = Some("Could not save Agent Control settings.".into());
                } else if policy.enabled && !policy.no_ai {
                    self.connect();
                }
            }
            _ => {}
        }
    }
}

fn optional_hostname() -> std::io::Result<Option<String>> {
    use std::io::Read;
    let path = Path::new(super::CAFE_APP_ROOT).join("launcher/agent-control-hostname.txt");
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut name = String::new();
    file.take(255).read_to_string(&mut name)?;
    Ok(Some(name.trim().to_owned()))
}

fn on_off(value: bool) -> &'static str {
    if value {
        "On"
    } else {
        "Off"
    }
}

fn prefix(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}
