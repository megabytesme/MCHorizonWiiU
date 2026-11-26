use super::*;
use horizon_wiiu_mcp::oauth::{AuthorizeRequest, OAuth, Persistence};
use horizon_wiiu_mcp::{http, router::Router};
use zeroize::Zeroizing;

#[derive(Clone, Default)]
struct GrantSnapshot(Arc<Mutex<Option<Zeroizing<Vec<u8>>>>>);

impl GrantSnapshot {
    fn copy(&self) -> Self {
        Self(Arc::new(Mutex::new(self.0.lock().unwrap().clone())))
    }
}

impl Persistence for GrantSnapshot {
    fn load(&self) -> std::io::Result<Option<Zeroizing<Vec<u8>>>> {
        Ok(self.0.lock().unwrap().clone())
    }

    fn save(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        *self.0.lock().unwrap() = Some(Zeroizing::new(bytes.to_vec()));
        Ok(())
    }
}

#[test]
fn copied_grant_revocation_closes_queued_write_and_reply() {
    let game = std::env::temp_dir().join("game");
    std::fs::create_dir(&game).unwrap();
    crate::FIXTURE_ROOT.set(game).unwrap();
    load_policy(Policy {
        enabled: true,
        allow_game: true,
        allow_launch: true,
        no_ai: false,
    });
    const ISSUER: &str = "https://horizon.test";
    const REDIRECT: &str = "http://127.0.0.1:55173/oauth/callback";
    const FULL: &str = "launcher.read launcher.manage game.observe game.control";
    let backend = NightTools;
    let open = |store: GrantSnapshot| {
        Router::new(
            ISSUER.into(),
            OAuth::open(store, format!("{ISSUER}/mcp")).unwrap(),
            backend,
        )
        .unwrap()
    };
    let generate = |router: &Router<GrantSnapshot, NightTools>, scopes| {
        router.local(|auth, now| {
            auth.open_pairing(now);
            let id = auth
                .register(now, "Disposable client", &[REDIRECT.into()], true)
                .unwrap();
            let request = auth
                .authorization(
                    now,
                    AuthorizeRequest {
                        client_id: &id,
                        redirect_uri: REDIRECT,
                        scope: Some(scopes),
                        challenge: "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
                        state: "revocation-fixture",
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
            (
                id,
                Zeroizing::new(tokens.access_token),
                Zeroizing::new(tokens.refresh_token.unwrap()),
            )
        })
    };
    let http_request = |path: &str, content: &str, token: Option<&str>, body: &str| {
        let auth = token.map_or(String::new(), |value| {
            format!("Authorization: Bearer {value}\r\n")
        });
        let request = format!(
            "POST {path} HTTP/1.1\r\nHost: horizon.test\r\n{auth}Content-Type: {content}\r\nContent-Length: {}\r\n\r\n{body}",
            body.len(),
        );
        http::read(request.as_bytes()).unwrap()
    };
    let tools = |router: &Router<GrantSnapshot, NightTools>, token: &str| {
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
        router.handle(
            http_request("/mcp", "application/json", Some(token), body),
            &|| false,
        )
    };
    let names = |response: http::Response| {
        assert_eq!(response.status, 200);
        let body: Value = serde_json::from_slice(&response.body).unwrap();
        body["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["name"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    let invoke = |router: &Router<GrantSnapshot, NightTools>,
                  token: &str,
                  name,
                  args,
                  disconnected: &dyn Fn() -> bool| {
        let body = json!({"jsonrpc":"2.0", "id":2, "method":"tools/call",
            "params":{"name":name,"arguments":args}})
        .to_string();
        router.handle(
            http_request("/mcp", "application/json", Some(token), &body),
            disconnected,
        )
    };

    let source = GrantSnapshot::default();
    let initial = open(source.clone());
    let (client, access, refresh) = generate(&initial, FULL);
    let (_, reader, _) = generate(&initial, "launcher.read");
    assert_eq!(tools(&initial, &access).status, 200);
    drop(initial);
    let disposable = source.copy();
    let router = open(disposable.clone());
    let registration = session().prepare(true);
    assert!(session().poll(true).0.is_none());
    let full = names(tools(&router, &access));
    for name in [
        "versions",
        "multiplayer_add",
        "game_state",
        "game_options_set",
    ] {
        assert!(
            full.iter().any(|entry| entry == name),
            "missing expected scope {name}"
        );
    }
    let principal = router
        .local(|auth, now| auth.authenticate(now, &access))
        .unwrap();
    let before = local_service()
        .lock()
        .unwrap()
        .tool("multiplayer_list", &json!({}))
        .unwrap();

    let (local_reply, game_reply, stale_catalog) = std::thread::scope(|scope| {
        let service = local_service().lock().unwrap();
        let (notice, queued) = mpsc::channel();
        *crate::BEFORE_NIGHT_LOCK.lock().unwrap() = Some(notice);
        let local = std::thread::Builder::new()
            .name("night-revocation-mutation".into())
            .spawn_scoped(scope, || {
                invoke(
                    &router,
                    &access,
                    "multiplayer_add",
                    json!({"name":"Revoked write", "address":"revoked-bookmark.invalid"}),
                    &|| false,
                )
            })
            .unwrap();
        queued.recv_timeout(Duration::from_secs(2)).unwrap();
        let checks = std::sync::atomic::AtomicUsize::new(0);
        let (game_waiting, waiting) = mpsc::sync_channel(1);
        let (resume, resume_rx) = mpsc::sync_channel(1);
        let resume_rx = Mutex::new(resume_rx);
        let (router_ref, access_ref) = (&router, &access);
        let game = scope.spawn(move || {
            invoke(router_ref, access_ref, "game_state", json!({}), &|| {
                if checks.fetch_add(1, Ordering::Relaxed) == 2 {
                    game_waiting.send(()).unwrap();
                    resume_rx
                        .lock()
                        .unwrap()
                        .recv_timeout(Duration::from_secs(2))
                        .unwrap();
                }
                false
            })
        });
        waiting.recv_timeout(Duration::from_secs(2)).unwrap();
        let request = session().poll(true).0.unwrap();
        assert_eq!(request.line, "state");
        router.local(|auth, _| auth.revoke_client(&client)).unwrap();
        request
            .reply
            .send(r#"{"fixture_private_game_state":"never returned after revoke"}"#.into())
            .unwrap();
        resume.send(()).unwrap();
        drop(service);
        (
            local.join().unwrap(),
            game.join().unwrap(),
            backend.list(&principal),
        )
    });
    let game_body: Value = serde_json::from_slice(&game_reply.body).unwrap();
    let after = local_service()
        .lock()
        .unwrap()
        .tool("multiplayer_list", &json!({}))
        .unwrap();
    assert_eq!(
        (
            local_reply.status,
            game_body["result"]["isError"].as_bool(),
            stale_catalog.len()
        ),
        (403, Some(true), 0)
    );
    assert_eq!(before, after);
    assert!(!String::from_utf8_lossy(&game_reply.body).contains("fixture_private_game_state"));
    assert!(!principal.is_active());
    assert_eq!(tools(&router, &access).status, 401);
    drop(router);

    let restarted = open(disposable);
    assert_eq!(tools(&restarted, &access).status, 401);
    let token_request = format!(
        "grant_type=refresh_token&client_id={client}&refresh_token={}",
        refresh.as_str()
    );
    let response = restarted.handle(
        http_request(
            "/oauth/token",
            "application/x-www-form-urlencoded",
            None,
            &token_request,
        ),
        &|| false,
    );
    assert_eq!(response.status, 400);
    let rejected: Value = serde_json::from_slice(&response.body).unwrap();
    assert_eq!(rejected["error"], "invalid_grant");
    assert!(restarted
        .local(|auth, now| auth.register(now, "Closed pairing", &[REDIRECT.into()], true))
        .is_err());
    let (replacement, new_access, _) = generate(&restarted, "launcher.read");
    assert_ne!(replacement, client);
    for credential in [&reader, &new_access] {
        let tools = names(tools(&restarted, credential));
        assert!(tools.iter().any(|entry| entry == "versions"));
        for name in ["multiplayer_add", "game_state", "game_options_set"] {
            assert!(!tools.iter().any(|entry| entry == name));
            assert_eq!(
                invoke(&restarted, credential, name, json!({}), &|| false).status,
                403
            );
        }
    }
    assert_eq!(tools(&open(source), &access).status, 200);
    drop(registration);
}
