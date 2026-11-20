use std::fs;
use std::path::{Path, PathBuf};

use super::local_wire::*;
use super::night_online::{self, Online};
use super::night_store::{self as store, Preferences, Profile, Server, Store};
use horizon_wiiu_microsoft::{Identity, Provider};

const MANIFEST: &str = "horizon-resources.json";
const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ_- .:[]";
const LIST_ROWS: usize = 3;
// Public client ID verified by the existing UWP port.
const MICROSOFT_CLIENT_ID: &str = "331e08f8-172d-43b9-835a-3108c2fcdbf3";
const ACCOUNT_FIXTURE: bool = option_env!("HORIZON_NIGHT_ACCOUNT_FIXTURE").is_some();

pub struct ResourcePin<'a> {
    pub version: &'a str,
    pub schema: u64,
    pub client: &'a str,
    pub server: &'a str,
    pub exporter: &'a str,
}

#[derive(serde::Deserialize)]
struct PreparedManifest<'a> {
    schema: u64,
    minecraft: &'a str,
    client_sha256: &'a str,
    server_sha256: &'a str,
    exporter_sha256: &'a str,
}

#[derive(Clone)]
pub struct World {
    pub name: String,
    pub detail: String,
    pub unavailable: Option<&'static str>,
}

pub trait GameFiles {
    fn world(&self, path: &Path, identity: &str, online_uuid: Option<u128>) -> Option<World>;
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ResourceStatus {
    Ready,
    Missing,
    Corrupt,
    Incompatible,
}

impl ResourceStatus {
    fn text(self) -> &'static str {
        match self {
            Self::Ready => "Prepared files ready",
            Self::Missing => "Prepared files missing",
            Self::Corrupt => "Prepared manifest or files invalid",
            Self::Incompatible => "Preparation differs from the client pin",
        }
    }
}

fn plain(path: &Path, directory: bool) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        if directory {
            metadata.is_dir()
        } else {
            metadata.is_file() && metadata.len() > 0
        }
    })
}

pub fn resources(path: &Path, pin: &ResourcePin<'_>) -> ResourceStatus {
    let manifest = path.join(MANIFEST);
    if !manifest.exists() {
        return ResourceStatus::Missing;
    }
    if !plain(&manifest, false) {
        return ResourceStatus::Corrupt;
    }
    let Ok(data) = store::read_limited(&manifest, 64 * 1024) else {
        return ResourceStatus::Corrupt;
    };
    let Ok(value) = serde_json::from_slice::<PreparedManifest<'_>>(&data) else {
        return ResourceStatus::Corrupt;
    };
    let PreparedManifest {
        schema,
        minecraft: version,
        client_sha256: client,
        server_sha256: server,
        exporter_sha256: exporter,
    } = value;
    if [client, server, exporter].iter().any(|value| {
        value.len() != 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }) {
        return ResourceStatus::Corrupt;
    }
    if (schema, version, client, server, exporter)
        != (
            pin.schema,
            pin.version,
            pin.client,
            pin.server,
            pin.exporter,
        )
    {
        return ResourceStatus::Incompatible;
    }
    if ["facts.json", "registries.json", "asset-index.json"]
        .iter()
        .any(|relative| !plain(&path.join(relative), false))
        || !plain(&path.join("assets"), true)
        || !plain(&path.join("data"), true)
    {
        return ResourceStatus::Corrupt;
    }
    ResourceStatus::Ready
}

#[derive(Clone)]
enum Target {
    Menu,
    World(String),
    Server(String),
}

pub struct Launch {
    pub game: PathBuf,
    pub resources: PathBuf,
    pub player: String,
    pub world: Option<String>,
    pub server: Option<String>,
    pub swap_ab: bool,
    pub swap_xy: bool,
    pub identity: Option<Identity>,
}

#[derive(Clone, Copy)]
enum List {
    Profiles,
    Instances,
    Worlds,
    Servers,
    Reports,
}

#[derive(Clone)]
enum Edit {
    NewProfile,
    Profile(usize),
    NewServerName,
    ServerName(usize),
    Address { index: Option<usize>, name: String },
}

impl Edit {
    fn caption(&self) -> &'static str {
        match self {
            Self::NewProfile => "New offline identity",
            Self::Profile(_) => "Edit profile label",
            Self::NewServerName | Self::ServerName(_) => "Server name",
            Self::Address { .. } => "Server address",
        }
    }
    fn limit(&self) -> u16 {
        match self {
            Self::NewProfile => 16,
            Self::Address { .. } => 253,
            _ => 24,
        }
    }
    fn list(&self) -> List {
        match self {
            Self::NewProfile | Self::Profile(_) => List::Profiles,
            _ => List::Servers,
        }
    }
}

#[derive(Clone)]
enum Page {
    Home,
    Online,
    Library,
    Settings,
    Versions,
    About(usize),
    Items(List, usize),
    Profile(usize),
    Server(usize),
    Confirm { profile: bool, index: usize },
    Edit(Edit),
    Report(usize, usize),
}

struct Item {
    key: String,
    label: String,
    detail: String,
    state: u8,
}

impl Item {
    fn plain(key: &str, label: &str, detail: &str, state: u8) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            detail: detail.into(),
            state,
        }
    }
}

enum Command {
    Back,
    Online,
    Show(Page),
    Profile(usize),
    Server(usize),
    Instance(String),
    World(String),
    Remove { profile: bool, index: usize },
    Toggle(u8),
    Edit(Edit, String),
    Keyboard,
    AddCharacter,
    EraseCharacter,
    FinishEdit,
    ClearTarget,
}

pub struct LocalService<G> {
    base: PathBuf,
    pin: ResourcePin<'static>,
    pub preferences: Preferences,
    store: Store,
    game: G,
    about: &'static [(&'static str, &'static str)],
    page: Page,
    target: Target,
    commands: Vec<Command>,
    items: Vec<Item>,
    message: &'static str,
    edited: String,
    character: usize,
    keyboard: bool,
    online: Online,
}

impl<G: GameFiles> LocalService<G> {
    pub fn new(
        base: PathBuf,
        pin: ResourcePin<'static>,
        game: G,
        about: &'static [(&'static str, &'static str)],
    ) -> Self {
        let (store, preferences) = Store::open(base.join("launcher"));
        let message = store.notice;
        let target = preferences
            .server
            .map(|i| Target::Server(preferences.servers[i].address.clone()))
            .unwrap_or(Target::Menu);
        let online = Online::new(
            &base.join("private"),
            Provider::for_night(Some(MICROSOFT_CLIENT_ID), ACCOUNT_FIXTURE),
        );
        Self {
            base,
            pin,
            game,
            about,
            store,
            preferences,
            page: Page::Home,
            target,
            commands: Vec::new(),
            items: Vec::new(),
            message,
            edited: String::new(),
            character: 0,
            keyboard: false,
            online,
        }
    }

    fn game_dir(&self, instance: &str) -> PathBuf {
        if instance == "default" {
            self.base.clone()
        } else {
            self.base.join("instances").join(instance)
        }
    }

    fn resource_dir(&self, game: &Path) -> PathBuf {
        game.join(".local")
            .join("horizon-client")
            .join(self.pin.version)
    }

    fn installed(&self, name: &str) -> ResourceStatus {
        if !store::identifier(name) {
            return ResourceStatus::Missing;
        }
        let game = self.game_dir(name);
        if (name != "default"
            && (!plain(&self.base.join("instances"), true) || !plain(&game, true)))
            || !plain(&game.join(".local"), true)
            || !plain(&game.join(".local/horizon-client"), true)
            || !plain(&self.resource_dir(&game), true)
        {
            return ResourceStatus::Missing;
        }
        resources(&self.resource_dir(&game), &self.pin)
    }

    fn world(&self, id: &str) -> Option<World> {
        let saves = self.game_dir(&self.preferences.instance).join("saves");
        let path = saves.join(id);
        if !store::identifier(id) || !plain(&saves, true) || !plain(&path, true) {
            return None;
        }
        let saved = path.join("level.dat");
        if !plain(&saved, false) {
            return plain(&path.join("level.dat_old"), false).then(|| World {
                name: id.into(),
                detail: "Recovery required".into(),
                unavailable: Some("Open recovery in the game menu"),
            });
        }
        if self.online.selected().is_some() {
            let account = self.online.active()?;
            self.game.world(&path, &account.name, Some(account.id.0))
        } else {
            self.game.world(
                &path,
                &self.preferences.profiles[self.preferences.profile].identity,
                None,
            )
        }
    }

    pub fn launch(&self) -> Result<Launch, u8> {
        if self.installed(&self.preferences.instance) != ResourceStatus::Ready {
            return Err(2);
        }
        let identity = self.online.active();
        if self.online.selected().is_some() && (identity.is_none() || ACCOUNT_FIXTURE) {
            return Err(5);
        }
        let game = self.game_dir(&self.preferences.instance);
        let (world, server) = match &self.target {
            Target::Menu => (None, None),
            Target::World(id) if self.world(id).is_some_and(|w| w.unavailable.is_none()) => {
                (Some(id.clone()), None)
            }
            Target::World(_) => return Err(5),
            Target::Server(address) if store::endpoint(address).as_ref() == Some(address) => {
                (None, Some(address.clone()))
            }
            Target::Server(_) => return Err(5),
        };
        Ok(Launch {
            resources: self.resource_dir(&game),
            game,
            player: identity.map_or_else(
                || {
                    self.preferences.profiles[self.preferences.profile]
                        .identity
                        .clone()
                },
                |a| a.name.clone(),
            ),
            world,
            server,
            swap_ab: self.preferences.settings.swap_ab,
            swap_xy: self.preferences.settings.swap_xy,
            identity: identity.map(Identity::duplicate),
        })
    }

    fn save(&mut self, next: Preferences) -> bool {
        if self.store.commit(&next).is_err() {
            self.message =
                "Could not save launcher settings. The previous settings are still active.";
            return false;
        }
        self.preferences = next;
        true
    }

    fn report(&self, key: &str) -> Option<Vec<(String, String)>> {
        if !store::identifier(key) || !key.ends_with(".json") {
            return None;
        }
        let folder = self.game_dir(&self.preferences.instance).join("benchmarks");
        let path = folder.join(key);
        if !plain(&folder, true) || !plain(&path, false) {
            return None;
        }
        let content = store::read_limited(&path, 2 * 1024 * 1024).ok()?;
        let value: serde_json::Value = serde_json::from_slice(&content).ok()?;
        let object = value.as_object()?;
        if value["format"].as_str()? != "horizon-benchmark-report"
            || value["version"].as_u64()? != 1
        {
            return None;
        }
        let mut rows = Vec::new();
        for (key, segment) in object {
            if !matches!(
                key.as_str(),
                "version" | "kind" | "created" | "app" | "conditions" | "metrics" | "label"
            ) {
                continue;
            }
            if let Some(fields) = segment.as_object() {
                for (name, value) in fields {
                    if let Some(text) = scalar(value) {
                        rows.push((format!("{key} / {name}"), text));
                    } else if key == "metrics" {
                        for statistic in ["median", "p95", "mean", "min", "max", "n"] {
                            if let Some(text) = scalar(&value[statistic]) {
                                rows.push((format!("{name} / {statistic}"), text));
                            }
                        }
                    }
                }
            } else if let Some(text) = scalar(segment) {
                rows.push((key.clone(), text));
            }
        }
        Some(rows)
    }

    fn listing(&self, kind: List) -> Vec<Item> {
        let game = self.game_dir(&self.preferences.instance);
        match kind {
            List::Profiles => self
                .preferences
                .profiles
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    Item::plain(
                        "",
                        &p.name,
                        &format!("Identity: {}", p.identity),
                        if i == self.preferences.profile {
                            SELECTED
                        } else {
                            ACTIVE
                        },
                    )
                })
                .collect(),
            List::Servers => self
                .preferences
                .servers
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    Item::plain(
                        "",
                        &s.name,
                        &s.address,
                        if Some(i) == self.preferences.server {
                            SELECTED
                        } else {
                            ACTIVE
                        },
                    )
                })
                .collect(),
            List::Instances => core::iter::once("default".into())
                .chain(
                    directories(&self.base.join("instances"))
                        .into_iter()
                        .filter(|name| !name.eq_ignore_ascii_case("default")),
                )
                .map(|name| {
                    let status = self.installed(&name);
                    Item::plain(
                        &name,
                        &name,
                        status.text(),
                        if status != ResourceStatus::Ready {
                            DISABLED
                        } else if self.preferences.instance == name {
                            SELECTED
                        } else {
                            ACTIVE
                        },
                    )
                })
                .collect(),
            List::Worlds => directories(&game.join("saves"))
                .into_iter()
                .filter_map(|name| {
                    let w = self.world(&name)?;
                    Some(Item::plain(
                        &name,
                        &w.name,
                        w.unavailable.unwrap_or(&w.detail),
                        if w.unavailable.is_some() {
                            DISABLED
                        } else if matches!(&self.target, Target::World(id) if id == &name) {
                            SELECTED
                        } else {
                            ACTIVE
                        },
                    ))
                })
                .collect(),
            List::Reports => entries(&game.join("benchmarks"), false)
                .into_iter()
                .filter(|key| key.ends_with(".json"))
                .map(|key| Item::plain(&key, &key, "Local benchmark report", ACTIVE))
                .collect(),
        }
    }

    fn show(&mut self, page: Page) {
        if let Page::Items(kind, 0) = page {
            self.items = self.listing(kind);
        }
        self.page = page;
    }

    pub fn command(&mut self, action: u8, argument: u8, input: &str) {
        self.message = "";
        if matches!(self.page, Page::Online) {
            let selected = self.online.selected();
            if self.online.command(action, argument, input) {
                self.show(Page::Items(List::Profiles, 0));
            }
            self.account_changed(selected);
            return;
        }
        match action {
            OPEN_PROFILES => self.show(Page::Items(List::Profiles, 0)),
            OPEN_LIBRARY => self.show(Page::Library),
            OPEN_VERSIONS => self.show(Page::Versions),
            OPEN_SETTINGS => self.show(Page::Settings),
            BACK => self.back(),
            LEFT | RIGHT if matches!(self.page, Page::Edit(_)) => {
                let offset = if action == LEFT {
                    ALPHABET.len() - 1
                } else {
                    1
                };
                self.character = (self.character + offset) % ALPHABET.len();
            }
            TEXT_ACCEPTED if matches!(self.page, Page::Edit(_)) => {
                self.edited = input.into();
                self.finish_edit();
            }
            TEXT_CANCELLED => {}
            TEXT_UNAVAILABLE => {
                self.message = "System keyboard unavailable. Use the character picker below."
            }
            ACTIVATE if usize::from(argument) < self.commands.len() => {
                let command = self.commands.remove(usize::from(argument));
                self.execute(command);
            }
            _ => {}
        }
    }

    pub fn tick(&mut self) -> bool {
        let selected = self.online.selected();
        let changed = self.online.tick();
        self.account_changed(selected);
        changed
    }

    pub fn shutdown(&mut self) {
        self.online.shutdown();
    }

    fn account_changed(&mut self, previous: Option<horizon_wiiu_microsoft::ProfileId>) {
        if self.online.selected() != previous && matches!(self.target, Target::World(_)) {
            self.target = Target::Menu;
        }
    }

    fn back(&mut self) {
        let page = match &self.page {
            Page::Items(List::Worlds | List::Servers, _) => Page::Library,
            Page::Items(List::Instances, _) | Page::About(_) => Page::Versions,
            Page::Items(List::Reports, _) => Page::Settings,
            Page::Profile(_) => Page::Items(List::Profiles, 0),
            Page::Server(_) => Page::Items(List::Servers, 0),
            Page::Confirm { profile, index } => {
                if *profile {
                    Page::Profile(*index)
                } else {
                    Page::Server(*index)
                }
            }
            Page::Edit(edit) => Page::Items(edit.list(), 0),
            Page::Report(_, _) => Page::Items(List::Reports, 0),
            _ => Page::Home,
        };
        self.show(page);
    }

    fn execute(&mut self, command: Command) {
        let mut next = self.preferences.clone();
        match command {
            Command::Back => self.back(),
            Command::Online => {
                self.online.open();
                self.show(Page::Online);
            }
            Command::Show(page) => self.show(page),
            Command::Profile(index) => {
                let selected = self.online.selected();
                if let Err(error) = self.online.offline() {
                    self.message = night_online::error_label(error);
                    return;
                }
                self.account_changed(selected);
                next.profile = index;
                if self.save(next) {
                    self.show(Page::Items(List::Profiles, 0));
                }
            }
            Command::Server(index) => {
                next.server = Some(index);
                let address = next.servers[index].address.clone();
                if self.save(next) {
                    self.target = Target::Server(address);
                    self.show(Page::Home);
                }
            }
            Command::Instance(id) => {
                if self.installed(&id) != ResourceStatus::Ready {
                    self.message = "The selected installation is no longer available.";
                    return;
                }
                next.instance = id;
                next.server = None;
                if self.save(next) {
                    self.target = Target::Menu;
                    self.show(Page::Home);
                }
            }
            Command::World(id) => {
                if self
                    .world(&id)
                    .is_some_and(|world| world.unavailable.is_none())
                {
                    if next.server.take().is_some() && !self.save(next) {
                        return;
                    }
                    self.target = Target::World(id);
                    self.show(Page::Home);
                } else {
                    self.message = "The selected world is no longer available.";
                }
            }
            Command::ClearTarget => {
                next.server = None;
                if self.save(next) {
                    self.target = Target::Menu;
                    self.show(Page::Home);
                }
            }
            Command::Remove {
                profile: true,
                index,
            } => {
                if next.profiles.len() <= 1 {
                    return;
                }
                next.profiles.remove(index);
                if next.profile == index {
                    next.profile = 0;
                } else if next.profile > index {
                    next.profile -= 1;
                }
                if self.save(next) {
                    self.show(Page::Items(List::Profiles, 0));
                }
            }
            Command::Remove {
                profile: false,
                index,
            } => {
                next.servers.remove(index);
                let selected = next.server == Some(index);
                next.server =
                    next.server
                        .filter(|&i| i != index)
                        .map(|i| if i > index { i - 1 } else { i });
                if self.save(next) {
                    if selected && matches!(self.target, Target::Server(_)) {
                        self.target = Target::Menu;
                    }
                    self.show(Page::Items(List::Servers, 0));
                }
            }
            Command::Toggle(flag) => {
                match flag {
                    SWAP_AB => next.settings.swap_ab = !next.settings.swap_ab,
                    SWAP_XY => next.settings.swap_xy = !next.settings.swap_xy,
                    _ => next.settings.touch = !next.settings.touch,
                }
                self.save(next);
            }
            Command::Edit(edit, text) => {
                self.edited = text;
                self.character = 0;
                self.show(Page::Edit(edit));
            }
            Command::Keyboard => self.keyboard = true,
            Command::AddCharacter => {
                if let Page::Edit(edit) = &self.page {
                    if self.edited.len() < usize::from(edit.limit()) {
                        self.edited.push(char::from(ALPHABET[self.character]));
                    }
                }
            }
            Command::EraseCharacter => {
                self.edited.pop();
            }
            Command::FinishEdit => self.finish_edit(),
        }
    }

    fn finish_edit(&mut self) {
        let Page::Edit(edit) = self.page.clone() else {
            return;
        };
        let text = self.edited.trim();
        let mut next = self.preferences.clone();
        match &edit {
            Edit::NewProfile => {
                if !store::identity(text)
                    || next.profiles.len() >= store::ENTRIES
                    || next
                        .profiles
                        .iter()
                        .any(|p| p.identity.eq_ignore_ascii_case(text))
                {
                    self.message =
                        "Use a unique identity with 1-16 letters, digits or underscores.";
                    return;
                }
                next.profiles.push(Profile {
                    name: text.into(),
                    identity: text.into(),
                });
                next.profile = next.profiles.len() - 1;
            }
            Edit::Profile(index) if store::label(text, 24) => {
                next.profiles[*index].name = text.into()
            }
            Edit::NewServerName | Edit::ServerName(_) if store::label(text, 24) => {
                let (index, initial) = match edit {
                    Edit::ServerName(index) => (Some(index), next.servers[index].address.clone()),
                    _ => (None, String::new()),
                };
                self.page = Page::Edit(Edit::Address {
                    index,
                    name: text.into(),
                });
                self.edited = initial;
                return;
            }
            Edit::Address { index, name } => {
                let Some(address) = store::endpoint(text) else {
                    self.message = "Enter a hostname or IP and an optional port (1-65535).";
                    return;
                };
                let server = Server {
                    name: name.clone(),
                    address,
                };
                match index {
                    Some(index) => next.servers[*index] = server,
                    None if next.servers.len() < store::ENTRIES => next.servers.push(server),
                    None => {
                        self.message = "The saved server list is full.";
                        return;
                    }
                }
            }
            _ => {
                self.message = "Enter a label with 1-24 bytes and no control characters.";
                return;
            }
        }
        let server_changed = next
            .server
            .and_then(|index| next.servers.get(index))
            .map(|entry| entry.address.clone());
        if self.save(next) {
            if let (Target::Server(_), Some(address)) = (&self.target, server_changed) {
                self.target = Target::Server(address);
            }
            self.show(Page::Items(edit.list(), 0));
        }
    }

    pub fn model(&mut self) -> LocalMenu {
        self.commands.clear();
        let status = self.installed(&self.preferences.instance);
        let online = self.online.selected_label();
        let requires_account = online.is_some() && self.online.active().is_none();
        let report = match self.page {
            Page::Report(index, _) => self
                .items
                .get(index)
                .and_then(|item| self.report(&item.key)),
            _ => None,
        };
        let mut menu = LocalMenu {
            profile: Text::new(online.map_or(
                &self.preferences.profiles[self.preferences.profile].name,
                |(name, _)| name,
            )),
            play_detail: Text::new(if status != ResourceStatus::Ready {
                status.text()
            } else if requires_account {
                online.map_or("", |(_, status)| status)
            } else {
                match &self.target {
                    Target::Menu => "Native game menu",
                    Target::World(id) => id,
                    Target::Server(address) => address,
                }
            }),
            ready: u8::from(
                status == ResourceStatus::Ready
                    && !requires_account
                    && (!ACCOUNT_FIXTURE || online.is_none()),
            ),
            settings: (if self.preferences.settings.swap_ab {
                SWAP_AB
            } else {
                0
            }) | (if self.preferences.settings.swap_xy {
                SWAP_XY
            } else {
                0
            }) | (if self.preferences.settings.touch {
                TOUCH
            } else {
                0
            }),
            ..LocalMenu::default()
        };
        if matches!(self.page, Page::Online) {
            self.online.model(&mut menu);
            return menu;
        }
        let mut row = |label: &str, detail: &str, state, command| {
            if let Some(dest) = menu.rows.get_mut(self.commands.len()) {
                *dest = LocalRow {
                    label: Text::new(label),
                    detail: Text::new(detail),
                    state,
                };
                self.commands.push(command);
            }
        };
        let (title, description) = match self.page.clone() {
            Page::Home => (
                "",
                if requires_account {
                    self.online.status_message()
                } else {
                    ""
                },
            ),
            Page::Online => unreachable!(),
            Page::Library => {
                row(
                    "Local worlds",
                    "From this installation",
                    ACTIVE,
                    Command::Show(Page::Items(List::Worlds, 0)),
                );
                row(
                    "Saved servers",
                    "Offline identity",
                    ACTIVE,
                    Command::Show(Page::Items(List::Servers, 0)),
                );
                row(
                    "Use game menu",
                    "Clear the direct target",
                    if matches!(self.target, Target::Menu) {
                        SELECTED
                    } else {
                        ACTIVE
                    },
                    Command::ClearTarget,
                );
                row("Back", "", ACTIVE, Command::Back);
                (
                    "Worlds & servers",
                    "Choose a target, then select Start Game on Play.",
                )
            }
            Page::Settings => {
                for (title, flag, on) in [
                    ("Swap A and B", SWAP_AB, self.preferences.settings.swap_ab),
                    ("Swap X and Y", SWAP_XY, self.preferences.settings.swap_xy),
                    ("GamePad touch", TOUCH, self.preferences.settings.touch),
                ] {
                    row(
                        title,
                        if on { "On" } else { "Off" },
                        ACTIVE,
                        Command::Toggle(flag),
                    );
                }
                row(
                    "Reports",
                    "Existing benchmark results",
                    ACTIVE,
                    Command::Show(Page::Items(List::Reports, 0)),
                );
                row("Back", "", ACTIVE, Command::Back);
                (
                    "Settings",
                    "Button swaps apply to the game. Touch controls Night navigation.",
                )
            }
            Page::Versions => {
                row(
                    "Installed resources",
                    status.text(),
                    ACTIVE,
                    Command::Show(Page::Items(List::Instances, 0)),
                );
                row(
                    "About this build",
                    "Verified build identity",
                    ACTIVE,
                    Command::Show(Page::About(0)),
                );
                row("Back", "", ACTIVE, Command::Back);
                (
                    "Version & resources",
                    "Select an existing compatible Horizon preparation.",
                )
            }
            Page::About(page) => {
                for (label, value) in self.about.iter().skip(page * 4).take(4) {
                    row(label, value, DISABLED, Command::Back);
                }
                if (page + 1) * 4 < self.about.len() {
                    row(
                        "More build information",
                        "",
                        ACTIVE,
                        Command::Show(Page::About(page + 1)),
                    );
                }
                if page > 0 {
                    row(
                        "Previous build information",
                        "",
                        ACTIVE,
                        Command::Show(Page::About(page - 1)),
                    );
                }
                row("Back", "", ACTIVE, Command::Back);
                ("About Horizon Wii U", "CafeOS / PowerPC")
            }
            Page::Items(kind, page) => {
                let limit = if matches!(kind, List::Profiles) {
                    2
                } else {
                    LIST_ROWS
                };
                let start = page * limit;
                if matches!(kind, List::Profiles) && page == 0 {
                    row(night_online::tr("accounts"), "", ACTIVE, Command::Online);
                }
                for (index, item) in self.items.iter().enumerate().skip(start).take(limit) {
                    let action = match kind {
                        List::Profiles => Command::Show(Page::Profile(index)),
                        List::Servers => Command::Show(Page::Server(index)),
                        List::Instances => Command::Instance(item.key.clone()),
                        List::Worlds => Command::World(item.key.clone()),
                        List::Reports => Command::Show(Page::Report(index, 0)),
                    };
                    row(&item.label, &item.detail, item.state, action);
                }
                let add = match kind {
                    List::Profiles if self.preferences.profiles.len() < store::ENTRIES => {
                        Some(("Create offline profile", Edit::NewProfile))
                    }
                    List::Servers if self.preferences.servers.len() < store::ENTRIES => {
                        Some(("Add server", Edit::NewServerName))
                    }
                    _ => None,
                };
                // Keep six rows on both displays. A later page can go back
                // through the preceding page before reaching the parent.
                if let Some((label, edit)) = add {
                    row(label, "", ACTIVE, Command::Edit(edit, String::new()));
                }
                if self.items.len() > start + limit {
                    row(
                        "Next page",
                        "",
                        ACTIVE,
                        Command::Show(Page::Items(kind, page + 1)),
                    );
                }
                if page > 0 {
                    row(
                        "Previous page",
                        "",
                        ACTIVE,
                        Command::Show(Page::Items(kind, page - 1)),
                    );
                } else {
                    row("Back", "", ACTIVE, Command::Back);
                }
                match kind {
                    List::Profiles => (
                        "Offline profiles",
                        "Choose an offline identity or manage Microsoft accounts.",
                    ),
                    List::Instances => (
                        "Installed resources",
                        "Only compatible preparations can be selected.",
                    ),
                    List::Worlds => (
                        "Local worlds",
                        if self.items.is_empty() {
                            "No saved worlds in this installation."
                        } else {
                            "Only compatible worlds for the selected identity can start here."
                        },
                    ),
                    List::Servers => (
                        "Saved servers",
                        if self.items.is_empty() {
                            "Add a hostname or IP address to get started."
                        } else {
                            "Select an endpoint to use on Play."
                        },
                    ),
                    List::Reports => (
                        "Diagnostic reports",
                        if self.items.is_empty() {
                            "No benchmark JSON reports in this installation."
                        } else {
                            "Read existing local reports. Nothing is uploaded."
                        },
                    ),
                }
            }
            Page::Profile(index) => {
                let profile = &self.preferences.profiles[index];
                row(
                    "Use this profile",
                    &profile.identity,
                    if index == self.preferences.profile {
                        SELECTED
                    } else {
                        ACTIVE
                    },
                    Command::Profile(index),
                );
                row(
                    "Rename label",
                    &profile.name,
                    ACTIVE,
                    Command::Edit(Edit::Profile(index), profile.name.clone()),
                );
                row(
                    "Remove profile",
                    if self.preferences.profiles.len() > 1 {
                        "Confirm removal"
                    } else {
                        "Keep at least one profile"
                    },
                    if self.preferences.profiles.len() > 1 {
                        ACTIVE
                    } else {
                        DISABLED
                    },
                    Command::Show(Page::Confirm {
                        profile: true,
                        index,
                    }),
                );
                row("Back", "", ACTIVE, Command::Back);
                (
                    "Offline profile",
                    "The login identity stays fixed when the launcher label changes.",
                )
            }
            Page::Server(index) => {
                let server = &self.preferences.servers[index];
                row(
                    "Use this server",
                    &server.address,
                    ACTIVE,
                    Command::Server(index),
                );
                row(
                    "Edit server",
                    &server.name,
                    ACTIVE,
                    Command::Edit(Edit::ServerName(index), server.name.clone()),
                );
                row(
                    "Remove server",
                    "Confirm removal",
                    ACTIVE,
                    Command::Show(Page::Confirm {
                        profile: false,
                        index,
                    }),
                );
                row("Back", "", ACTIVE, Command::Back);
                ("Saved server", "Connections use the selected profile.")
            }
            Page::Confirm { profile, index } => {
                row("Cancel", "Keep this entry", ACTIVE, Command::Back);
                row(
                    "Confirm removal",
                    "Remove from Night",
                    ACTIVE,
                    Command::Remove { profile, index },
                );
                (
                    "Confirm removal",
                    if profile {
                        "This removes the local profile entry. World data and UUIDs are preserved."
                    } else {
                        "This removes the endpoint from Night's saved list."
                    },
                )
            }
            Page::Edit(edit) => {
                menu.mode = EDITOR;
                let initial = self
                    .edited
                    .chars()
                    .rev()
                    .take(56)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<String>();
                row(
                    "Open system keyboard",
                    "Type text. Press Minus to close.",
                    ACTIVE,
                    Command::Keyboard,
                );
                row(
                    "Append character",
                    &char::from(ALPHABET[self.character]).to_string(),
                    ACTIVE,
                    Command::AddCharacter,
                );
                row("Delete last character", "", ACTIVE, Command::EraseCharacter);
                row("Continue", &initial, ACTIVE, Command::FinishEdit);
                row("Cancel", "", ACTIVE, Command::Back);
                if std::mem::take(&mut self.keyboard) {
                    menu.input = Text::new(&self.edited);
                    menu.input_limit = edit.limit();
                }
                (
                    edit.caption(),
                    "Use the keyboard or choose a character with Left/Right.",
                )
            }
            Page::Report(index, page) => {
                if let Some(fields) = &report {
                    for (label, value) in fields.iter().skip(page * 3).take(3) {
                        row(label, value, DISABLED, Command::Back);
                    }
                    if (page + 1) * 3 < fields.len() {
                        row(
                            "More",
                            "",
                            ACTIVE,
                            Command::Show(Page::Report(index, page + 1)),
                        );
                    }
                    if page > 0 {
                        row(
                            "Previous",
                            "",
                            ACTIVE,
                            Command::Show(Page::Report(index, page - 1)),
                        );
                    }
                }
                row("Back", "", ACTIVE, Command::Back);
                (
                    "Benchmark report",
                    if report.is_some() {
                        "Recorded measurements from the local report."
                    } else {
                        "This file is missing, invalid, or uses an unsupported format."
                    },
                )
            }
        };
        menu.title = Text::new(title);
        menu.description = Text::new(if self.message.is_empty() {
            description
        } else {
            self.message
        });
        menu.count = self.commands.len() as u8;
        menu
    }
}

fn scalar(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Bool(_) | serde_json::Value::Number(_) => Some(value.to_string()),
        _ => None,
    }
}

fn entries(directory: &Path, directory_entries: bool) -> Vec<String> {
    if !plain(directory, true) {
        return Vec::new();
    }
    let mut names = fs::read_dir(directory)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_ok_and(|kind| {
                if directory_entries {
                    kind.is_dir()
                } else {
                    kind.is_file()
                }
            })
        })
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| store::identifier(name))
        .take(256)
        .collect::<Vec<_>>();
    names.sort_by_key(|name| name.to_ascii_lowercase());
    names
}

fn directories(path: &Path) -> Vec<String> {
    entries(path, true)
}

#[path = "night_tools.rs"]
mod night_tools;
