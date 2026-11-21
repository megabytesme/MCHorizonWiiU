//! Hosted Cafe wiring. The HTTP workers never hold or access Game.
use super::{local_service, local_wire as wire, night_menu::Launch, CAFE_APP_ROOT};
use horizon_wiiu_mcp::{
    oauth::{Principal, Scopes},
    protocol::{text_result, Fault},
    router::Backend,
    service::{Policy, Service},
    session::{self, Session},
};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[allow(dead_code)]
#[path = "../../../crates/horizon-client/src/bin/horizon-ctl/commands.rs"]
mod commands;
#[path = "night_mcp_menu.rs"]
mod menu;

static HOME: AtomicBool = AtomicBool::new(false);
static GAME_ALLOWED: AtomicBool = AtomicBool::new(false);
static REMOTE_LAUNCH: AtomicBool = AtomicBool::new(false);
static NO_AI: AtomicBool = AtomicBool::new(false);
static GAME_NO_AI: AtomicBool = AtomicBool::new(false);
static LAUNCH: Mutex<Option<(Principal, Value)>> = Mutex::new(None);

fn session() -> &'static Session {
    static SESSION: OnceLock<Session> = OnceLock::new();
    SESSION.get_or_init(Session::default)
}

fn menu() -> &'static Mutex<menu::Menu> {
    static MENU: OnceLock<Mutex<menu::Menu>> = OnceLock::new();
    MENU.get_or_init(|| {
        let menu = menu::Menu::open(Path::new(CAFE_APP_ROOT));
        horizon_client::control::install_hosted_mcp(poll_game, || NO_AI.load(Ordering::Acquire));
        Mutex::new(menu)
    })
}

pub(super) fn change_policy(service: &mut Service<NightTools>, policy: Policy) -> bool {
    let written = service.set_policy(policy).is_ok();
    let disabled = policy.no_ai || !policy.enabled;
    if written || disabled {
        load_policy(policy);
    }
    written
}

fn load_policy(policy: Policy) {
    GAME_ALLOWED.store(
        policy.enabled && policy.allow_game && !policy.no_ai,
        Ordering::Release,
    );
    REMOTE_LAUNCH.store(
        policy.enabled && policy.allow_game && policy.allow_launch && !policy.no_ai,
        Ordering::Release,
    );
    NO_AI.store(policy.no_ai, Ordering::Release);
    if policy.no_ai || !policy.enabled {
        session().withdraw();
        LAUNCH.lock().unwrap().take();
    }
}

pub(super) fn menu_open() -> bool {
    menu().lock().unwrap().is_open()
}

pub(super) fn local_menu(command: u8, index: u8, base: wire::LocalMenu) -> Option<wire::LocalMenu> {
    menu().lock().unwrap().update(command, index, base)
}

#[no_mangle]
pub extern "C" fn horizon_night_mcp_tick(on_home: bool) -> bool {
    let changed = HOME.swap(on_home, Ordering::AcqRel) != on_home;
    if on_home {
        return LAUNCH.lock().unwrap().is_some();
    }
    if changed {
        LAUNCH.lock().unwrap().take();
    }
    false
}

#[no_mangle]
pub extern "C" fn horizon_night_mcp_shutdown() {
    HOME.store(false, Ordering::Release);
    LAUNCH.lock().unwrap().take();
    session().withdraw();
    if let Some(mut menu) = menu().lock().ok() {
        menu.shutdown();
    }
}

pub(super) fn take_launch() -> Result<Option<Launch>, ()> {
    HOME.store(false, Ordering::Release);
    let request = LAUNCH.lock().unwrap().take();
    match request {
        Some((principal, args))
            if principal.is_active() && REMOTE_LAUNCH.load(Ordering::Acquire) =>
        {
            local_service()
                .lock()
                .unwrap()
                .tool_launch(&args)
                .map(Some)
                .map_err(|_| ())
        }
        Some(_) => Err(()),
        None => Ok(None),
    }
}

pub(super) fn prepare_game() -> session::Registration {
    GAME_NO_AI.store(false, Ordering::Release);
    session().prepare(GAME_ALLOWED.load(Ordering::Acquire) && !NO_AI.load(Ordering::Acquire))
}

pub(crate) fn withdraw() {
    session().withdraw();
}

fn poll_game(no_ai: bool, ready: bool) -> (Option<horizon_client::control::Request>, bool) {
    let old = GAME_NO_AI.swap(no_ai, Ordering::AcqRel);
    if old != no_ai {
        if let Some(service) = menu().lock().unwrap().service.as_mut() {
            if let Ok(mut policy) = service.policy() {
                policy.no_ai = no_ai;
                change_policy(service, policy);
            }
        }
    }
    let before = session().live();
    let (request, cancelled) =
        session().poll(ready && !no_ai && GAME_ALLOWED.load(Ordering::Acquire));
    (
        request.map(|r| horizon_client::control::Request {
            line: r.line,
            reply: r.reply,
        }),
        cancelled || (before && !session().live()) || (!old && no_ai),
    )
}

#[derive(Clone, Copy)]
pub(super) struct NightTools;

const READ: &[&str] = &[
    "versions",
    "settings",
    "instance_list",
    "worlds_list",
    "multiplayer_list",
];
const MANAGE: &[&str] = &["instance_select", "multiplayer_add"];
const OBSERVE: &[&str] = &[
    "game_hello",
    "game_state",
    "game_status",
    "game_screen",
    "game_inventory",
    "game_looking_at",
    "game_entities",
    "game_block",
    "game_light",
    "game_blocks",
    "game_find_block",
    "game_ui",
    "game_messages",
    "game_narration",
    "game_wait_ready",
    "game_screenshot",
];
const CONTROL: &[&str] = &[
    "game_move",
    "game_jump",
    "game_look",
    "game_turn",
    "game_look_at",
    "game_look_at_block",
    "game_look_at_entity",
    "game_goto",
    "game_break",
    "game_place",
    "game_interact",
    "game_attack",
    "game_select",
    "game_click_slot",
    "game_close",
    "game_open_inventory",
    "game_pause",
    "game_resume",
    "game_save",
    "game_quit",
];

fn catalog() -> &'static [Value] {
    static CATALOG: OnceLock<Vec<Value>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let inventory: Value = serde_json::from_str(include_str!("pinned_tools.json"))
            .expect("verified pinned MCP tool inventory");
        inventory["tools"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|row| row.get("tool").cloned())
            .filter(|tool| {
                NightTools
                    .required(tool["name"].as_str().unwrap_or_default())
                    .is_some()
            })
            .collect()
    })
}

impl Backend for NightTools {
    fn list(&self, principal: &Principal) -> Vec<Value> {
        let live = session().live();
        catalog()
            .iter()
            .filter(|tool| {
                let name = tool["name"].as_str().unwrap_or_default();
                (!name.starts_with("game_") || live)
                    && (name != "play" || REMOTE_LAUNCH.load(Ordering::Acquire))
                    && self
                        .required(name)
                        .is_some_and(|scope| principal.scopes.contains(scope))
            })
            .cloned()
            .collect()
    }

    fn required(&self, name: &str) -> Option<Scopes> {
        if READ.contains(&name) {
            return Some(Scopes::LAUNCHER_READ);
        }
        if MANAGE.contains(&name) {
            return Some(Scopes::LAUNCHER_MANAGE);
        }
        if name == "play" {
            return Some(Scopes::LAUNCHER_MANAGE.union(Scopes::GAME_CONTROL));
        }
        if OBSERVE.contains(&name) {
            return Some(Scopes::GAME_OBSERVE);
        }
        if CONTROL.contains(&name) {
            return Some(Scopes::GAME_CONTROL);
        }
        None
    }

    fn call(
        &self,
        principal: &Principal,
        name: &str,
        args: &Value,
        disconnected: &dyn Fn() -> bool,
    ) -> Result<Value, Fault> {
        if disconnected() {
            return Err(Fault::new(400, -32000, "Client disconnected"));
        }
        if !principal.is_active() || NO_AI.load(Ordering::Acquire) {
            return Err(Fault::new(
                403,
                -32000,
                "AI features are off: turn off No AI in the launcher's settings",
            ));
        }
        if name == "play" {
            if !REMOTE_LAUNCH.load(Ordering::Acquire) {
                return Ok(text_result(
                    "The player has disabled remote game launches.",
                    true,
                ));
            }
            if !HOME.load(Ordering::Acquire) {
                return Ok(text_result(
                    "Return to the Night home page before starting a game.",
                    true,
                ));
            }
            let mut request = LAUNCH.lock().unwrap();
            if !HOME.load(Ordering::Acquire) {
                return Ok(text_result(
                    "Return to the Night home page before starting a game.",
                    true,
                ));
            }
            if request.is_some() {
                return Ok(text_result("The game is already starting.", true));
            }
            let start = local_service().lock().unwrap().tool_launch(args);
            return Ok(match start {
                Ok(_) => {
                    *request = Some((principal.clone(), args.clone()));
                    text_result(
                        "Horizon is starting. Use game_wait_ready once the game tools appear.",
                        false,
                    )
                }
                Err(err) => text_result(err, true),
            });
        }
        if name.starts_with("game_") {
            return Ok(game_call(principal, name, args, disconnected));
        }
        let result = local_service().lock().unwrap().tool(name, args);
        Ok(match result {
            Ok(output) => text_result(&output.to_string(), false),
            Err(error) => text_result(error, true),
        })
    }
}

fn game_call(
    principal: &Principal,
    full_name: &str,
    args: &Value,
    disconnected: &dyn Fn() -> bool,
) -> Value {
    let no_game = || {
        text_result(
        "No Horizon game is running under Agent Control. Start one with play, or enable game control locally.",
        true,
    )
    };
    if !session().live() {
        return no_game();
    }
    let name = full_name.trim_start_matches("game_");
    let Some(cmd) = commands::COMMANDS
        .iter()
        .find(|cmd| cmd.mcp && commands::tool_name(cmd) == name)
    else {
        return text_result("This game tool is unavailable on Wii U.", true);
    };
    let mut words = match commands::line_for(cmd, args) {
        Ok(words) => words,
        Err(error) => return text_result(&error, true),
    };
    if words
        .iter()
        .any(|word| word.len() > 240 || word.chars().any(char::is_control))
        || words
            .iter()
            .skip(1)
            .any(|word| word.chars().any(char::is_whitespace))
    {
        return text_result("Use one value per game argument.", true);
    }
    if name == "screenshot" {
        words.push("capture.png".into());
    }
    if let Some(wire) = cmd.wire {
        words.splice(0..1, wire.split(' ').map(str::to_owned));
    }
    let wire = words.join(" ");
    let seconds = args
        .get("seconds")
        .and_then(Value::as_f64)
        .unwrap_or(10.0)
        .clamp(0.0, 10.0);
    let deadline = Instant::now() + Duration::from_secs_f64(seconds);
    let mut reply = loop {
        let result = session().call(
            principal,
            if name == "wait_ready" {
                "status".into()
            } else {
                wire.clone()
            },
            disconnected,
        );
        let result = match result {
            Ok(result) => result,
            Err(session::Error::NoGame) => return no_game(),
            Err(_) => return text_result("The game did not accept the request in time.", true),
        };
        let Ok(value) = serde_json::from_str::<Value>(&result) else {
            return text_result("The game returned an invalid control reply.", true);
        };
        if name != "wait_ready"
            || value.get("error").is_some()
            || (!value["player"].is_null()
                && value["pending_chunks"] == 0
                && value["server"]["loaded"] != false)
        {
            break value;
        }
        if Instant::now() >= deadline {
            return text_result(
                "The world is still loading. Call game_wait_ready again.",
                true,
            );
        }
        std::thread::sleep(Duration::from_millis(250));
    };
    if let Some(error) = reply.get("error") {
        return text_result(&json!({"error": error}).to_string(), true);
    }
    if name == "screenshot" {
        let image = reply
            .as_object_mut()
            .and_then(|map| map.remove("png_base64"));
        if let Some(image) =
            image.filter(|v| v.as_str().is_some_and(|s| s.len() <= 7 * 1024 * 1024))
        {
            return json!({
                "content": [{"type": "image", "data": image, "mimeType": "image/png"},
                    {"type": "text", "text": reply.to_string()}],
                "isError": false
            });
        }
        return text_result(
            "The frame was unavailable or exceeded the response limit.",
            true,
        );
    }
    let mut result = text_result(&reply.to_string(), false);
    if commands::reply_schema(cmd).is_some() && reply.is_object() {
        result["structuredContent"] = reply;
    }
    result
}
