use horizon_wiiu_pregame::local_wire;
use local_wire::*;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

#[path = "../../game-bridge/night_menu.rs"]
mod night_menu;
#[path = "../../game-bridge/night_store.rs"]
mod night_store;

use night_menu::{resources, GameFiles, LocalService, ResourcePin, ResourceStatus, World};
use night_store::{endpoint, Preferences, Store};

const CLIENT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SERVER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const EXPORTER: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn pin() -> ResourcePin<'static> {
    ResourcePin {
        version: "26.3",
        schema: 7,
        client: CLIENT,
        server: SERVER,
        exporter: EXPORTER,
    }
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "night-local-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn prepare(app: &Path) -> PathBuf {
    let root = app.join(".local/horizon-client/26.3");
    for path in ["assets", "data"] {
        fs::create_dir_all(root.join(path)).unwrap();
    }
    for name in ["facts.json", "registries.json", "asset-index.json"] {
        fs::write(root.join(name), "{}").unwrap();
    }
    fs::write(
        root.join("horizon-resources.json"),
        json!({"minecraft":"26.3","schema":7,
        "client_sha256":CLIENT,"server_sha256":SERVER,"exporter_sha256":EXPORTER})
        .to_string(),
    )
    .unwrap();
    root
}

struct Game;

impl GameFiles for Game {
    fn world(&self, path: &Path, identity: &str) -> Option<World> {
        let title = fs::read_to_string(path.join("level.dat")).ok()?;
        Some(World {
            name: title,
            detail: "Survival / 26.3".into(),
            unavailable: if path.join("guard").exists() {
                Some("World is locked")
            } else if identity != "Player" {
                Some("Select the original identity")
            } else {
                None
            },
        })
    }
}

fn open(root: &Scratch) -> LocalService<Game> {
    LocalService::new(root.0.clone(), pin(), Game, &[("Builder", "0.4.0")])
}

fn action(service: &mut LocalService<Game>, label: &str) {
    let menu = service.model();
    let index = menu.rows[..usize::from(menu.count)]
        .iter()
        .position(|row| row.label.as_str() == label)
        .unwrap_or_else(|| panic!("{label} missing on {}", menu.title.as_str()));
    assert_ne!(menu.rows[index].state, DISABLED);
    service.command(ACTIVATE, index as u8, "");
}

#[test]
fn missing_configuration_and_single_valid_older_generation_recover() {
    let root = Scratch::new();
    let (mut store, mut initial) = Store::open(root.0.clone());
    assert_eq!(initial, Preferences::default());
    initial.settings.touch = false;
    store.commit(&initial).unwrap();
    let mut newer = initial.clone();
    newer.settings.swap_xy = true;
    store.commit(&newer).unwrap();
    assert_eq!(Store::open(root.0.clone()).1, newer);
    fs::write(root.0.join("settings.1.json"), b"{\"schema\":2").unwrap();
    fs::write(
        root.0.join("settings.1.pending"),
        fs::read(root.0.join("settings.0.json")).unwrap(),
    )
    .unwrap();
    let (restored, actual) = Store::open(root.0.clone());
    assert!(restored.notice.contains("last valid"));
    assert_eq!(actual, initial);
}

#[test]
fn legacy_version_migrates_and_future_schema_is_preserved() {
    let root = Scratch::new();
    fs::write(
        root.0.join("settings.json"),
        r#"{"schema":1,"player":"Guest","swap_buttons":true,"touch_navigation":false}"#,
    )
    .unwrap();
    let (mut store, value) = Store::open(root.0.clone());
    assert_eq!(value.profiles[0].identity, "Guest");
    assert!(value.settings.swap_ab && !value.settings.touch);
    store.commit(&value).unwrap();
    assert_eq!(Store::open(root.0.clone()).1, value);
    let newer = br#"{"schema":3}"#;
    fs::write(root.0.join("settings.1.json"), newer).unwrap();
    let (mut store, restored) = Store::open(root.0.clone());
    assert!(store.commit(&restored).is_err());
    assert_eq!(fs::read(root.0.join("settings.1.json")).unwrap(), newer);
}

#[test]
fn failed_replacement_keeps_the_previous_interactive_settings() {
    let root = Scratch::new();
    let mut service = open(&root);
    service.command(OPEN_SETTINGS, 0, "");
    action(&mut service, "Swap A and B");
    fs::create_dir(root.0.join("launcher/settings.1.pending")).unwrap();
    action(&mut service, "GamePad touch");
    let menu = service.model();
    assert!(menu.description.as_str().contains("Could not save"));
    assert_eq!(menu.settings, SWAP_AB | TOUCH);
    assert_eq!(open(&root).model().settings, SWAP_AB | TOUCH);
    let invalid = Scratch::new();
    fs::write(invalid.0.join("settings.0.json"), b"truncated").unwrap();
    assert_eq!(Store::open(invalid.0.clone()).1, Preferences::default());
}

#[test]
fn profile_create_rename_select_and_confirmed_removal_preserve_identity() {
    let root = Scratch::new();
    prepare(&root.0);
    let mut service = open(&root);
    service.command(OPEN_PROFILES, 0, "");
    action(&mut service, "Create offline profile");
    service.command(TEXT_ACCEPTED, 0, "Player");
    assert!(service.model().description.as_str().contains("unique"));
    service.command(TEXT_ACCEPTED, 0, "Alex_26");
    action(&mut service, "Alex_26");
    action(&mut service, "Rename label");
    service.command(TEXT_ACCEPTED, 0, "Alex the builder");
    assert_eq!(service.launch().unwrap().player, "Alex_26");
    let mut reopened = open(&root);
    assert_eq!(reopened.model().profile.as_str(), "Alex the builder");
    reopened.command(OPEN_PROFILES, 0, "");
    action(&mut reopened, "Alex the builder");
    action(&mut reopened, "Remove profile");
    action(&mut reopened, "Cancel");
    assert_eq!(reopened.launch().unwrap().player, "Alex_26");
    action(&mut reopened, "Remove profile");
    action(&mut reopened, "Confirm removal");
    assert_eq!(open(&root).launch().unwrap().player, "Player");
}

#[test]
fn preparation_checks_manifest_and_files_and_remembers_selected_instance() {
    let root = Scratch::new();
    let path = root.0.join("instances/Lab");
    let prepared = prepare(&path);
    assert!(resources(&prepared, &pin()) == ResourceStatus::Ready);
    let original = fs::read_to_string(prepared.join("horizon-resources.json")).unwrap();
    fs::write(
        prepared.join("horizon-resources.json"),
        original.replace(CLIENT, SERVER),
    )
    .unwrap();
    assert!(resources(&prepared, &pin()) == ResourceStatus::Incompatible);
    fs::write(
        prepared.join("horizon-resources.json"),
        original.replacen('{', "{\"schema\":7,", 1),
    )
    .unwrap();
    assert!(resources(&prepared, &pin()) == ResourceStatus::Corrupt);
    fs::write(prepared.join("horizon-resources.json"), original).unwrap();
    let mut service = open(&root);
    assert_eq!(service.model().ready, 0);
    service.command(OPEN_VERSIONS, 0, "");
    action(&mut service, "Installed resources");
    action(&mut service, "Lab");
    assert_eq!(open(&root).launch().unwrap().resources, prepared);
    fs::remove_file(prepared.join("facts.json")).unwrap();
    assert_eq!(open(&root).launch().err(), Some(2));
}

#[test]
fn world_picker_is_read_only_and_rechecks_lock_and_presence_at_launch() {
    let root = Scratch::new();
    prepare(&root.0);
    let path = root.0.join("saves/Folder ID");
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("level.dat"), "World title from metadata").unwrap();
    let mut service = open(&root);
    service.command(OPEN_LIBRARY, 0, "");
    action(&mut service, "Local worlds");
    action(&mut service, "World title from metadata");
    let launch = service.launch().unwrap();
    assert_eq!(launch.world.as_deref(), Some("Folder ID"));
    assert_eq!(launch.game, root.0);
    assert!(!path.join("session.lock").exists());
    fs::write(path.join("guard"), []).unwrap();
    assert_eq!(service.launch().err(), Some(5));
    fs::remove_file(path.join("level.dat")).unwrap();
    assert_eq!(service.launch().err(), Some(5));
    for unsafe_id in ["..", ".", "a/../b", "C:\\Windows", "a\0b"] {
        assert!(!night_store::identifier(unsafe_id));
    }
}

#[test]
fn saved_servers_and_settings_round_trip_to_the_launch_request() {
    let root = Scratch::new();
    prepare(&root.0);
    for invalid in [
        "https://mc.example",
        "bad host",
        "x:0",
        "x:65536",
        "127.0.0.999",
        "a..b",
        "-a.com",
        "[::1]:",
    ] {
        assert!(endpoint(invalid).is_none(), "{invalid}");
    }
    assert_eq!(
        endpoint("[2001:db8::1]:25566").as_deref(),
        Some("[2001:db8::1]:25566")
    );
    let mut service = open(&root);
    service.command(OPEN_LIBRARY, 0, "");
    action(&mut service, "Saved servers");
    action(&mut service, "Add server");
    service.command(TEXT_ACCEPTED, 0, "LAN");
    service.command(TEXT_ACCEPTED, 0, "Mc.EXAMPLE:25566");
    action(&mut service, "LAN");
    action(&mut service, "Use this server");
    service.command(OPEN_SETTINGS, 0, "");
    action(&mut service, "Swap A and B");
    action(&mut service, "Swap X and Y");
    let mut service = open(&root);
    let launch = service.launch().unwrap();
    assert_eq!(launch.server.as_deref(), Some("mc.example:25566"));
    assert!(launch.swap_ab && launch.swap_xy);
    service.command(OPEN_LIBRARY, 0, "");
    action(&mut service, "Saved servers");
    action(&mut service, "LAN");
    action(&mut service, "Edit server");
    service.command(TEXT_ACCEPTED, 0, "Updated");
    service.command(TEXT_ACCEPTED, 0, "192.0.2.1");
    assert_eq!(
        service.launch().unwrap().server.as_deref(),
        Some("192.0.2.1")
    );
    action(&mut service, "Updated");
    action(&mut service, "Remove server");
    action(&mut service, "Confirm removal");
    assert!(open(&root).launch().unwrap().server.is_none());
}

#[test]
fn reports_read_existing_metrics_without_mutating_files() {
    let root = Scratch::new();
    let directory = root.0.join("benchmarks");
    fs::create_dir(&directory).unwrap();
    let file = directory.join("run.json");
    let report = br#"{"format":"horizon-benchmark-report","version":1,"metrics":{"latency_ms":{"p95":11.7}}}"#;
    fs::write(&file, report).unwrap();
    let mut service = open(&root);
    service.command(OPEN_SETTINGS, 0, "");
    action(&mut service, "Reports");
    action(&mut service, "run.json");
    assert_eq!(service.model().rows[0].detail.as_str(), "11.7");
    assert_eq!(fs::read(file).unwrap(), report);
}
