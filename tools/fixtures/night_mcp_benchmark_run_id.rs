use super::*;
use horizon_wiiu_mcp::oauth::{AuthorizeRequest, OAuth, Persistence};
use horizon_wiiu_mcp::{http, router::Router};
use zeroize::Zeroizing;

const ISSUER: &str = "https://horizon.test";
const REDIRECT: &str = "http://127.0.0.1:55173/oauth/callback";
const FULL: &str = "launcher.read launcher.manage game.observe game.control";
const START: &str = "wiiu_benchmark_start";
const STATUS: &str = "wiiu_benchmark_status";
const CANCEL: &str = "wiiu_benchmark_cancel";

#[derive(Default)]
struct Memory(Option<Zeroizing<Vec<u8>>>);

impl Persistence for Memory {
    fn load(&self) -> std::io::Result<Option<Zeroizing<Vec<u8>>>> {
        Ok(self.0.clone())
    }

    fn save(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.0 = Some(Zeroizing::new(bytes.to_vec()));
        Ok(())
    }
}

fn grant(router: &Router<Memory, NightTools>, scopes: &str) -> Zeroizing<String> {
    router.local(|auth, now| {
        auth.open_pairing(now);
        let id = auth
            .register(now, "Benchmark fixture", &[REDIRECT.into()], true)
            .unwrap();
        let request = auth
            .authorization(
                now,
                AuthorizeRequest {
                    client_id: &id,
                    redirect_uri: REDIRECT,
                    scope: Some(scopes),
                    challenge: "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
                    state: "benchmark-run-id-fixture",
                    resource: None,
                },
            )
            .unwrap();
        let consent = auth.begin_consent(now, request).unwrap();
        auth.decide(now, &consent, true).unwrap();
        let url = auth.finish(now, &consent, ISSUER).unwrap().unwrap();
        let code = url.query_pairs().find(|(key, _)| key == "code").unwrap().1;
        let tokens = auth
            .exchange(
                now,
                &id,
                REDIRECT,
                &code,
                "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
                None,
            )
            .unwrap();
        Zeroizing::new(tokens.access_token)
    })
}

fn invoke(
    router: &Router<Memory, NightTools>,
    token: &str,
    tool: &str,
    args: Value,
) -> http::Response {
    let body = json!({"jsonrpc":"2.0", "id":2, "method":"tools/call",
        "params": {"name": tool, "arguments": args}})
    .to_string();
    let request = format!(
        "POST /mcp HTTP/1.1\r\nHost: horizon.test\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len(),
    );
    router.handle(http::read(request.as_bytes()).unwrap(), &|| false)
}

fn tool_result(response: &http::Response) -> Value {
    assert_eq!(response.status, 200);
    let body: Value = serde_json::from_slice(&response.body).unwrap();
    body["result"].clone()
}

fn state(response: http::Response) -> Value {
    let result = tool_result(&response);
    assert_eq!(
        result["isError"], false,
        "Unexpected tool response {result}"
    );
    serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
}

fn denied(response: http::Response) {
    assert_eq!(tool_result(&response)["isError"], true);
}

fn assert_state(value: &Value, id: &str, phase: &str) {
    assert_eq!(value["instance"], "default");
    assert_eq!(value["scenario"], "cafe-stationary");
    assert_eq!(value["profile"], "4q");
    assert_eq!(value["run_id"], id);
    assert_eq!(value["phase"], phase);
}

fn run_id(value: &Value) -> String {
    let id = value["run_id"].as_str().expect("missing benchmark run ID");
    assert_eq!(id.len(), 32);
    assert!(id
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    id.into()
}

fn foreground_handoff() {
    assert!(horizon_night_mcp_tick(true));
    let launch = match take_launch() {
        Ok(Some(launch)) => launch,
        _ => panic!("Night did not consume the queued benchmark"),
    };
    let benchmark = launch.launch.benchmark.unwrap();
    benchmark_loading(benchmark.scenario, &benchmark.instance, benchmark.profile);
}

#[test]
fn benchmark_run_id_survives_foreground_handoff_and_scopes_cancel() {
    let game = std::env::temp_dir().join("game");
    let resources = game.join(".local/horizon-client/26.3");
    for directory in ["assets", "data"] {
        std::fs::create_dir_all(resources.join(directory)).unwrap();
    }
    for file in ["facts.json", "registries.json", "asset-index.json"] {
        std::fs::write(resources.join(file), "{}").unwrap();
    }
    let hash = "1".repeat(64);
    std::fs::write(
        resources.join("horizon-resources.json"),
        json!({"schema": 7, "minecraft": "26.3", "client_sha256": hash,
              "server_sha256": hash, "exporter_sha256": hash})
        .to_string(),
    )
    .unwrap();
    crate::FIXTURE_ROOT.set(game).unwrap();
    load_policy(Policy {
        enabled: true,
        allow_game: true,
        allow_launch: true,
        no_ai: false,
    });
    let router = Router::new(
        ISSUER.into(),
        OAuth::open(Memory::default(), format!("{ISSUER}/mcp")).unwrap(),
        NightTools,
    )
    .unwrap();
    let owner = grant(&router, FULL);
    let other = grant(&router, FULL);
    let read_only = grant(&router, "launcher.read");
    let launch = || {
        state(invoke(
            &router,
            &owner,
            START,
            json!({
                "scenario": "cafe-stationary", "profile": "4q",
            }),
        ))
    };
    let status = || state(invoke(&router, &owner, STATUS, json!({})));
    let cancel = |token, args| invoke(&router, token, CANCEL, args);

    assert!(!horizon_night_mcp_tick(true));
    let first = launch();
    let first_id = run_id(&first);
    assert_state(&first, &first_id, "queued");
    assert_state(&status(), &first_id, "queued");
    foreground_handoff();
    assert_state(&status(), &first_id, "loading");
    benchmark_started();
    assert_state(&status(), &first_id, "running");
    benchmark_closing();
    assert_state(&status(), &first_id, "closing");
    let report = "game-20261126003000-cafe-stationary.json";
    let capture = "game-20261126003000-cafe-stationary-4q.hzp";
    benchmark_finished(BenchmarkOutcome::Completed, Some(report), Some(capture));
    let completed = status();
    assert_state(&completed, &first_id, "completed");
    assert_eq!(completed["report_id"], report);
    assert_eq!(completed["capture_id"], capture);
    assert!(!horizon_night_mcp_tick(true));
    assert_eq!(status()["night_home_ready"], true);

    let second = launch();
    let second_id = run_id(&second);
    assert_ne!(first_id, second_id);
    assert_state(&second, &second_id, "queued");
    denied(cancel(&owner, json!({"run_id": first_id})));
    denied(cancel(&other, json!({"run_id": second_id})));
    denied(cancel(&owner, json!({"run_id": null})));
    denied(cancel(&owner, json!({"run_id": second_id, "other": true})));
    assert_eq!(cancel(&read_only, json!({"run_id": second_id})).status, 403);
    assert_state(&status(), &second_id, "queued");
    assert_state(
        &state(cancel(&owner, json!({"run_id": second_id}))),
        &second_id,
        "cancelled",
    );
    assert!(!horizon_night_mcp_tick(true));

    let third_id = run_id(&launch());
    foreground_handoff();
    benchmark_started();
    denied(cancel(&other, json!({"run_id": third_id})));
    denied(cancel(&owner, json!({"run_id": second_id})));
    assert!(!take_benchmark_cancel());
    let requested = state(cancel(&owner, json!({"run_id": third_id})));
    assert_state(&requested, &third_id, "running");
    assert_eq!(requested["cancel_requested"], true);
    assert!(take_benchmark_cancel());
    assert!(!take_benchmark_cancel());
    benchmark_finished(BenchmarkOutcome::Cancelled, None, None);
    assert_state(&status(), &third_id, "cancelled");
    assert!(!horizon_night_mcp_tick(true));

    let legacy_id = run_id(&launch());
    assert_state(&state(cancel(&other, json!({}))), &legacy_id, "cancelled");
    assert!(!horizon_night_mcp_tick(true));

    // A new identical run may be queued before the earlier cancel replies.
    let cancel_id = run_id(&launch());
    let principal = router
        .local(|auth, now| auth.authenticate(now, &owner))
        .unwrap();
    let cancelled = cancel_benchmark(&principal, Some(&cancel_id)).unwrap();
    let replacement = state(invoke(
        &router,
        &other,
        START,
        json!({
            "scenario": "cafe-stationary", "profile": "4q",
        }),
    ));
    let replacement_id = run_id(&replacement);
    assert_ne!(replacement_id, cancel_id);
    assert_state(&status(), &replacement_id, "queued");
    assert_state(&cancelled, &cancel_id, "cancelled");
    assert_state(
        &state(cancel(&other, json!({"run_id": replacement_id}))),
        &replacement_id,
        "cancelled",
    );
    assert!(!horizon_night_mcp_tick(true));

    // A Night-local run has no remote ID and cannot inherit the old owner.
    benchmark_loading(
        "cafe-stationary",
        "default",
        Some(BenchmarkProfile::EarlyTv),
    );
    let local = status();
    assert_eq!(local["phase"], "loading");
    assert!(local["run_id"].is_null());
    denied(cancel(&other, json!({"run_id": replacement_id})));
    assert!(!take_benchmark_cancel());
}
