//! End-to-end tests: a real gateway on a loopback port in front of a fake provider that speaks both the
//! OpenAI and the Anthropic API, and counts every call it receives.

use super::pricing::Price;
use super::store::{self, NewProvider, Target};
use super::*;
use axum::extract::Json as JsonBody;
use std::sync::atomic::{AtomicBool, Ordering};

struct MapSecrets(HashMap<String, String>);

impl Secrets for MapSecrets {
    fn read(&self, reference: &str) -> Result<Option<String>, String> {
        Ok(self.0.get(reference).cloned())
    }
}

#[derive(Default)]
struct Fake {
    calls: Mutex<HashMap<String, usize>>,
    flaky_failed: AtomicBool,
}

impl Fake {
    fn count(&self, model: &str) -> usize {
        self.calls.lock().unwrap().get(model).copied().unwrap_or(0)
    }
}

/// The fake OpenAI-compatible provider. The model name picks the behaviour.
async fn fake_openai(State(fake): State<Arc<Fake>>, headers: HeaderMap, JsonBody(body): JsonBody<Value>) -> Response {
    let model = body["model"].as_str().unwrap_or("").to_string();
    *fake.calls.lock().unwrap().entry(model.clone()).or_default() += 1;
    if model == "badkey" {
        return (StatusCode::UNAUTHORIZED, Json(json!({ "error": { "message": "invalid api key" } }))).into_response();
    }
    if headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) != Some("Bearer key-1") {
        return (StatusCode::UNAUTHORIZED, Json(json!({ "error": { "message": "bad key" } }))).into_response();
    }
    match model.as_str() {
        "limited" => return ([(header::RETRY_AFTER, "120")], (StatusCode::TOO_MANY_REQUESTS,
            Json(json!({ "error": { "message": "quota" } })))).into_response(),
        "forbidden" => return (StatusCode::FORBIDDEN, Json(json!({ "error": { "message": "not allowed" } }))).into_response(),
        "broken" => return (StatusCode::BAD_REQUEST, Json(json!({ "error": { "message": "bad field" } }))).into_response(),
        "down" => return (StatusCode::SERVICE_UNAVAILABLE, "overloaded").into_response(),
        "flaky" if !fake.flaky_failed.swap(true, Ordering::SeqCst) => {
            return (StatusCode::REQUEST_TIMEOUT, "try again").into_response();
        }
        "slow" => tokio::time::sleep(Duration::from_millis(1500)).await,
        "slow-ok" => tokio::time::sleep(Duration::from_millis(300)).await,
        _ => {}
    }
    let usage = json!({ "prompt_tokens": 1000, "completion_tokens": 500 });
    // Says whether prompt-cache markers survived the trip, for the translation tests.
    let text = if body.to_string().contains("cache_control") { "pong cache:yes" } else { "pong" };
    if body["stream"] == true {
        assert_eq!(body["stream_options"]["include_usage"], true, "the gateway asks for usage");
        let mut sse = String::new();
        if model == "tooler" {
            sse.push_str(&format!("data: {}\n\n", json!({ "choices": [{ "delta": { "tool_calls": [{ "index": 0, "id": "call_1",
                "function": { "name": "read", "arguments": "{\"path\":" } }] } }] })));
            sse.push_str(&format!("data: {}\n\n", json!({ "choices": [{ "delta": { "tool_calls": [{ "index": 0,
                "function": { "arguments": "\"a\"}" } }] }, "finish_reason": "tool_calls" }] })));
        } else {
            sse.push_str(&format!("data: {}\n\n", json!({ "choices": [{ "delta": { "content": "po" } }] })));
            sse.push_str(&format!("data: {}\n\n", json!({ "choices": [{ "delta": { "content": "ng" }, "finish_reason": "stop" }] })));
        }
        sse.push_str(&format!("data: {}\n\ndata: [DONE]\n\n", json!({ "choices": [], "usage": usage })));
        return ([(header::CONTENT_TYPE, "text/event-stream")], sse).into_response();
    }
    Json(json!({ "id": "x", "object": "chat.completion", "model": model,
        "choices": [{ "index": 0, "finish_reason": "stop", "message": { "role": "assistant", "content": text } }],
        "usage": usage })).into_response()
}

/// The fake Anthropic provider: answers with what it received, so passthrough can be checked field by field.
async fn fake_anthropic(State(fake): State<Arc<Fake>>, headers: HeaderMap, JsonBody(body): JsonBody<Value>) -> Response {
    let model = body["model"].as_str().unwrap_or("").to_string();
    *fake.calls.lock().unwrap().entry(model.clone()).or_default() += 1;
    if headers.get("x-api-key").and_then(|v| v.to_str().ok()) != Some("key-1") || headers.get("anthropic-version").is_none() {
        return (StatusCode::UNAUTHORIZED, Json(json!({ "type": "error", "error": { "type": "authentication_error",
            "message": "missing key or version" } }))).into_response();
    }
    if model == "limited" {
        return ([(header::RETRY_AFTER, "30")], (StatusCode::TOO_MANY_REQUESTS, Json(json!({ "type": "error",
            "error": { "type": "rate_limit_error", "message": "slow down" } })))).into_response();
    }
    let seen = format!("cache_control={} thinking={} beta={}", body.to_string().contains("cache_control"),
        !body["thinking"].is_null(), headers.get("anthropic-beta").and_then(|v| v.to_str().ok()).unwrap_or("-"));
    let usage = json!({ "input_tokens": 10, "cache_read_input_tokens": 900, "cache_creation_input_tokens": 90, "output_tokens": 5 });
    if body["stream"] == true {
        let sse = format!(
            "event: message_start\ndata: {}\n\nevent: content_block_delta\ndata: {}\n\nevent: message_delta\ndata: {}\n\nevent: message_stop\ndata: {{\"type\":\"message_stop\"}}\n\n",
            json!({ "type": "message_start", "message": { "id": "msg_1", "usage": usage } }),
            json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": seen } }),
            json!({ "type": "message_delta", "delta": { "stop_reason": "end_turn" }, "usage": { "output_tokens": 7 } }));
        return ([(header::CONTENT_TYPE, "text/event-stream")], sse).into_response();
    }
    Json(json!({ "id": "msg_1", "type": "message", "role": "assistant", "model": model,
        "content": [{ "type": "text", "text": seen }], "stop_reason": "end_turn", "usage": usage })).into_response()
}

struct Rig {
    base: String,
    token: String,
    client_id: String,
    shared: Shared,
    fake: Arc<Fake>,
    http: reqwest::Client,
    _server: Server,
}

fn price() -> Price {
    // $0.10 in, $0.40 out per million tokens: 1000 in + 500 out = $0.0001 + $0.0002.
    Price { input_micro_per_mtok: 100_000, output_micro_per_mtok: 400_000, cached_input_micro_per_mtok: None,
        cache_write_micro_per_mtok: None, source: "user".into(), observed_at: 0 }
}

async fn rig_with(read_timeout: Duration) -> Rig {
    let fake = Arc::new(Fake::default());
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream.local_addr().unwrap();
    let app = Router::new()
        .route("/v1/chat/completions", post(fake_openai))
        .route("/anthropic/v1/messages", post(fake_anthropic))
        .with_state(fake.clone());
    tokio::spawn(async move { axum::serve(upstream, app).await.unwrap() });

    let conn = crate::db::open_in_memory().unwrap();
    let endpoint = format!("http://{upstream_addr}/v1");
    let add = |kind: &str, label: &str, endpoint: &str, billing: &str, wire: &str| {
        store::add_provider(&conn, &NewProvider { kind, label, endpoint, credential_ref: Some("provider-main"),
            default_billing: billing, wire }).unwrap()
    };
    let main = add("custom", "Main", &endpoint, "paid", "openai");
    // Same provider kind as Main, so the same quota group: a sibling key of the same organization.
    let sibling = add("custom", "Sibling", &endpoint, "paid", "openai");
    // Marked as its own organization.
    let backup = add("custom", "Backup", &endpoint, "free", "openai");
    store::update_provider(&conn, &backup.id, None, None, Some(true)).unwrap();
    let router = add("openrouter", "Router", &endpoint, "free", "openai");
    let claude = add("custom-anthropic", "Claude", &format!("http://{upstream_addr}/anthropic/v1"), "paid", "anthropic");
    for model in ["fast", "slow-ok"] {
        store::add_model(&conn, &main.id, model).unwrap();
        store::update_model(&conn, &main.id, model, None, None, Some(Some(price()))).unwrap();
    }
    store::add_model(&conn, &claude.id, "sonnet").unwrap();
    store::update_model(&conn, &claude.id, "sonnet", None, None, Some(Some(price()))).unwrap();
    let combo = |name: &str, members: &[(&str, &str)]| {
        let members: Vec<Target> = members.iter().map(|(p, m)| Target { provider_id: p.to_string(), model_id: m.to_string() }).collect();
        store::save_combo(&conn, None, name, &members).unwrap();
    };
    combo("coding", &[(&main.id, "limited"), (&main.id, "down"), (&backup.id, "steady")]);
    combo("siblings", &[(&main.id, "limited"), (&sibling.id, "steady")]);
    combo("slowcombo", &[(&main.id, "slow"), (&backup.id, "steady")]);
    combo("pinme", &[(&main.id, "flaky"), (&backup.id, "steady")]);
    let _ = router;
    let (client, token) = store::create_client(&conn, "Tests", None).unwrap();

    let secrets = MapSecrets(HashMap::from([("provider-main".to_string(), "key-1".to_string())]));
    let shared = Shared::with_read_timeout(Arc::new(Mutex::new(conn)), Arc::new(secrets), read_timeout);
    let server = start(shared.clone(), 0).await.unwrap();
    Rig { base: format!("http://{}", server.addr), token, client_id: client.id, shared, fake, http: reqwest::Client::new(),
        _server: server }
}

async fn rig() -> Rig {
    rig_with(Duration::from_secs(30)).await
}

impl Rig {
    async fn post(&self, path: &str, body: Value) -> reqwest::Response {
        self.http.post(format!("{}{path}", self.base)).bearer_auth(&self.token).json(&body).send().await.unwrap()
    }

    async fn post_session(&self, path: &str, session: &str, body: Value) -> reqwest::Response {
        self.http.post(format!("{}{path}", self.base)).bearer_auth(&self.token).header(SESSION_HEADER, session)
            .json(&body).send().await.unwrap()
    }

    fn ledger(&self) -> Vec<store::UsageEvent> {
        let mut events = self.shared.with_db(|db| store::usage_summary(db, 0, 100)).unwrap().recent;
        events.reverse();
        events
    }

    fn set_budget(&self, micro: Option<u64>) {
        self.shared.with_db(|db| store::set_client_budget(db, &self.client_id, micro)).unwrap();
    }
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().unwrap()
}

fn chat(model: &str) -> Value {
    json!({ "model": model, "messages": [{ "role": "user", "content": "ping" }] })
}

// ---------- Basics ----------

#[test]
fn requires_a_token_and_a_loopback_host() {
    rt().block_on(async {
        let rig = rig().await;
        let anonymous = rig.http.post(format!("{}/v1/chat/completions", rig.base)).json(&json!({ "model": "x" }))
            .send().await.unwrap();
        assert_eq!(anonymous.status(), 401);
        let rebound = rig.http.get(format!("{}/health", rig.base)).header(header::HOST, "evil.example:80")
            .send().await.unwrap();
        assert_eq!(rebound.status(), 403);
        assert_eq!(rig.http.get(format!("{}/health", rig.base)).send().await.unwrap().status(), 200);
    });
}

#[test]
fn routes_a_connection_model_and_prices_it() {
    rt().block_on(async {
        let rig = rig().await;
        let response = rig.post("/v1/chat/completions", chat("main/fast")).await;
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["x-smarttaskrouter-route"], "main/fast");
        assert_eq!(response.headers()["x-smarttaskrouter-mode"], "native");
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["choices"][0]["message"]["content"], "pong");
        let event = &rig.ledger()[0];
        assert_eq!((event.cost_status.as_str(), event.cost_micro_usd), ("estimated", Some(300)));
        assert_eq!((event.input_tokens, event.output_tokens), (Some(1000), Some(500)));

        // An unpriced model of a paid connection is unknown, not free.
        rig.post("/v1/chat/completions", chat("main/other")).await;
        let event = rig.ledger().pop().unwrap();
        assert_eq!((event.cost_status.as_str(), event.cost_micro_usd), ("unknown", None));
    });
}

#[test]
fn a_bad_request_is_returned_not_retried() {
    rt().block_on(async {
        let rig = rig().await;
        let response = rig.post("/v1/chat/completions", chat("main/broken")).await;
        assert_eq!(response.status(), 400);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["error"]["message"], "bad field");
        let missing = rig.post("/v1/chat/completions", chat("nope")).await;
        assert_eq!(missing.status(), 404);
    });
}

#[test]
fn streams_through_unchanged_and_records_usage() {
    rt().block_on(async {
        let rig = rig().await;
        let response = rig.post("/v1/chat/completions", json!({ "model": "main/fast", "stream": true, "messages": [] })).await;
        assert_eq!(response.status(), 200);
        let text = response.text().await.unwrap();
        assert!(text.contains("\"po\"") && text.contains("[DONE]"));
        // The ledger is written when the stream ends, just after the client has read it.
        tokio::time::sleep(Duration::from_millis(100)).await;
        let event = rig.ledger().pop().unwrap();
        assert_eq!((event.outcome.as_str(), event.cost_micro_usd), ("ok", Some(300)));
    });
}

// ---------- P0-2: error-scoped fallback and cooldown ----------

#[test]
fn a_combo_falls_back_across_groups_and_cools_failing_targets_down() {
    rt().block_on(async {
        let rig = rig().await;
        let response = rig.post("/v1/chat/completions", chat("coding")).await;
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["x-smarttaskrouter-route"], "backup/steady");
        let outcomes: Vec<(String, Option<u16>)> = rig.ledger().into_iter().map(|e| (e.outcome, e.http_status)).collect();
        assert_eq!(outcomes, [("fallback".into(), Some(429)), ("fallback".into(), Some(503)), ("ok".into(), Some(200))]);

        // The 429 asked for two minutes: the next request skips that model without calling it.
        let again = rig.post("/v1/chat/completions", chat("coding")).await;
        assert_eq!(again.status(), 200);
        assert_eq!(rig.fake.count("limited"), 1);
        let health = rig.shared.with_db(|db| store::list_health(db, Some("main"))).unwrap();
        let limited = health.iter().find(|h| h.model_id == "limited").unwrap();
        assert_eq!(limited.state, "rate_limited");
        assert!(limited.until_ms.unwrap() - crate::db::now_ms() > 100_000);
    });
}

#[test]
fn a_quota_refusal_never_moves_to_a_sibling_key_of_the_same_organization() {
    rt().block_on(async {
        let rig = rig().await;
        let response = rig.post("/v1/chat/completions", chat("siblings")).await;
        assert_eq!(response.status(), 429);
        assert_eq!(response.headers()[header::RETRY_AFTER], "120");
        assert_eq!(rig.fake.count("steady"), 0, "the sibling key was not tried");

        // Once the user says the second key is a separate organization, it may answer.
        rig.shared.with_db(|db| store::update_provider(db, "sibling", None, None, Some(true))).unwrap();
        let response = rig.post("/v1/chat/completions", chat("siblings")).await;
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["x-smarttaskrouter-route"], "sibling/steady");
    });
}

#[test]
fn a_rejected_key_blocks_the_connection_until_it_is_fixed() {
    rt().block_on(async {
        let rig = rig().await;
        assert_eq!(rig.post("/v1/chat/completions", chat("main/badkey")).await.status(), 401);
        let health = rig.shared.with_db(|db| store::list_health(db, Some("main"))).unwrap();
        assert_eq!((health[0].state.as_str(), health[0].model_id.as_str(), health[0].until_ms), ("invalid_credentials", "", None));

        // Every model of the connection is skipped now, without a call.
        let blocked = rig.post("/v1/chat/completions", chat("main/fast")).await;
        assert_eq!(blocked.status(), 502);
        assert!(blocked.text().await.unwrap().contains("invalid credentials"));
        assert_eq!(rig.fake.count("fast"), 0);

        rig.shared.with_db(|db| store::clear_health(db, "main")).unwrap();
        assert_eq!(rig.post("/v1/chat/completions", chat("main/fast")).await.status(), 200);
    });
}

#[test]
fn an_ambiguous_timeout_is_not_retried_on_another_model() {
    rt().block_on(async {
        let rig = rig_with(Duration::from_millis(400)).await;
        let response = rig.post("/v1/chat/completions", chat("slowcombo")).await;
        assert_eq!(response.status(), 504);
        assert_eq!(rig.fake.count("steady"), 0, "no second provider was charged");
        let event = rig.ledger().pop().unwrap();
        assert_eq!((event.model.as_str(), event.outcome.as_str(), event.cost_status.as_str()), ("slow", "error", "unknown"));
    });
}

// ---------- P0-3: budgets, concurrency, complete ledger ----------

#[test]
fn a_client_over_its_monthly_limit_is_refused() {
    rt().block_on(async {
        let rig = rig().await;
        rig.post("/v1/chat/completions", chat("main/fast")).await;
        rig.set_budget(Some(300));
        let refused = rig.post("/v1/chat/completions", chat("main/fast")).await;
        assert_eq!(refused.status(), 429);
    });
}

#[test]
fn concurrent_requests_cannot_pass_a_limit_together() {
    rt().block_on(async {
        let rig = rig().await;
        // Each request may cost up to 1M output tokens × $0.40 = $0.40; the limit holds one, not two.
        rig.set_budget(Some(500_000));
        let body = json!({ "model": "main/slow-ok", "max_tokens": 1_000_000, "messages": [] });
        let (a, b) = tokio::join!(rig.post("/v1/chat/completions", body.clone()), rig.post("/v1/chat/completions", body));
        let mut statuses = [a.status().as_u16(), b.status().as_u16()];
        statuses.sort();
        assert_eq!(statuses, [200, 429]);
        // Settled to the real cost, the reservation is gone.
        tokio::time::sleep(Duration::from_millis(50)).await;
        let client = rig.shared.with_db(store::list_clients).unwrap().remove(0);
        assert_eq!((client.month_spend_micro_usd, client.reserved_micro_usd), (300, 0));
    });
}

#[test]
fn an_unknown_price_is_blocked_for_a_client_with_a_limit() {
    rt().block_on(async {
        let rig = rig().await;
        rig.set_budget(Some(10_000_000));
        let refused = rig.post("/v1/chat/completions", chat("main/other")).await;
        assert_eq!(refused.status(), 429);
        assert!(refused.text().await.unwrap().contains("price is unknown"));
        rig.shared.with_db(|db| store::set_client_policy(db, &rig.client_id, None, Some("allow"))).unwrap();
        assert_eq!(rig.post("/v1/chat/completions", chat("main/other")).await.status(), 200);
        // Free connections need no price.
        rig.shared.with_db(|db| store::set_client_policy(db, &rig.client_id, None, Some("block"))).unwrap();
        assert_eq!(rig.post("/v1/chat/completions", chat("backup/anything")).await.status(), 200);
    });
}

#[test]
fn requests_in_flight_are_capped_per_client() {
    rt().block_on(async {
        let rig = rig().await;
        rig.shared.with_db(|db| store::set_client_policy(db, &rig.client_id, Some(1), None)).unwrap();
        let (a, b) = tokio::join!(rig.post("/v1/chat/completions", chat("main/slow-ok")),
            async { tokio::time::sleep(Duration::from_millis(50)).await; rig.post("/v1/chat/completions", chat("main/slow-ok")).await });
        assert_eq!((a.status().as_u16(), b.status().as_u16()), (200, 429));
        // The slot is free again afterwards.
        assert_eq!(rig.post("/v1/chat/completions", chat("main/fast")).await.status(), 200);
    });
}

#[test]
fn a_client_that_leaves_early_still_leaves_a_ledger_entry() {
    rt().block_on(async {
        let rig = rig().await;
        let impatient = reqwest::Client::builder().timeout(Duration::from_millis(100)).build().unwrap();
        let result = impatient.post(format!("{}/v1/chat/completions", rig.base)).bearer_auth(&rig.token)
            .json(&chat("main/slow-ok")).send().await;
        assert!(result.is_err(), "the client gave up");
        tokio::time::sleep(Duration::from_millis(600)).await;
        let event = rig.ledger().pop().expect("the request was recorded");
        assert_eq!((event.model.as_str(), event.cost_micro_usd), ("slow-ok", Some(300)));
    });
}

// ---------- P0-4: explicit session affinity ----------

#[test]
fn an_explicit_session_stays_on_the_model_that_answered_it() {
    rt().block_on(async {
        let rig = rig().await;
        // The first model times out once (408, no cooldown), so the session lands on the backup.
        let first = rig.post_session("/v1/chat/completions", "s-1", chat("pinme")).await;
        assert_eq!(first.headers()["x-smarttaskrouter-route"], "backup/steady");
        assert_eq!(first.headers()["x-smarttaskrouter-session"], "new");

        // A request without a session goes back to the first model, which works again.
        let other = rig.post("/v1/chat/completions", chat("pinme")).await;
        assert_eq!(other.headers()["x-smarttaskrouter-route"], "main/flaky");

        // The session stays where it landed: same model, same cache.
        let again = rig.post_session("/v1/chat/completions", "s-1", chat("pinme")).await;
        assert_eq!(again.headers()["x-smarttaskrouter-route"], "backup/steady");
        assert_eq!(again.headers()["x-smarttaskrouter-session"], "pinned");
        assert_eq!(again.headers()["x-smarttaskrouter-attempts"], "1");
        assert_eq!(rig.ledger().last().unwrap().session_key.as_deref(), Some("s-1"));
    });
}

// ---------- P0-1: same-format passthrough ----------

fn claude_request(stream: bool) -> Value {
    json!({ "model": "claude/sonnet", "max_tokens": 64, "stream": stream, "thinking": { "type": "enabled", "budget_tokens": 1024 },
        "system": [{ "type": "text", "text": "Long instructions", "cache_control": { "type": "ephemeral" } }],
        "messages": [{ "role": "user", "content": "ping" }] })
}

#[test]
fn an_anthropic_request_to_an_anthropic_connection_passes_through_unchanged() {
    rt().block_on(async {
        let rig = rig().await;
        let response = rig.http.post(format!("{}/v1/messages", rig.base)).header("x-api-key", &rig.token)
            .header("anthropic-version", "2023-06-01").header("anthropic-beta", "prompt-caching-test")
            .json(&claude_request(false)).send().await.unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["x-smarttaskrouter-mode"], "native");
        assert!(response.headers().get("x-smarttaskrouter-dropped").is_none());
        let body: Value = response.json().await.unwrap();
        // Cache markers, thinking and the beta header all reached the provider.
        assert_eq!(body["content"][0]["text"], "cache_control=true thinking=true beta=prompt-caching-test");
        assert_eq!(body["usage"]["cache_read_input_tokens"], 900, "the provider's answer came back as it was");
        let event = rig.ledger().pop().unwrap();
        assert_eq!((event.cache_tokens, event.cache_write_tokens, event.input_tokens), (Some(900), Some(90), Some(1000)));
        assert_eq!(event.route_mode.as_deref(), Some("native"));

        let streamed = rig.post("/v1/messages", claude_request(true)).await.text().await.unwrap();
        assert!(streamed.contains("cache_control=true thinking=true") && streamed.contains("message_stop"));
        tokio::time::sleep(Duration::from_millis(100)).await;
        let event = rig.ledger().pop().unwrap();
        assert_eq!((event.cache_tokens, event.output_tokens, event.outcome.as_str()), (Some(900), Some(7), "ok"));
    });
}

#[test]
fn an_openai_request_cannot_reach_an_anthropic_connection_yet() {
    rt().block_on(async {
        let rig = rig().await;
        let response = rig.post("/v1/chat/completions", chat("claude/sonnet")).await;
        assert_eq!(response.status(), 502);
        assert!(response.text().await.unwrap().contains("speaks the anthropic API"));
        assert_eq!(rig.fake.count("sonnet"), 0);
    });
}

#[test]
fn a_translation_keeps_cache_markers_only_where_the_provider_accepts_them() {
    rt().block_on(async {
        let rig = rig().await;
        let request = |model: &str| json!({ "model": model, "max_tokens": 64,
            "system": [{ "type": "text", "text": "Long", "cache_control": { "type": "ephemeral" } }],
            "messages": [{ "role": "user", "content": "ping" }] });

        let kept = rig.post("/v1/messages", request("router/m")).await;
        assert_eq!(kept.headers()["x-smarttaskrouter-mode"], "translated:anthropic->openai");
        assert!(kept.headers().get("x-smarttaskrouter-dropped").is_none(), "nothing was lost");
        let body: Value = kept.json().await.unwrap();
        assert_eq!(body["content"][0]["text"], "pong cache:yes");

        let dropped = rig.post("/v1/messages", request("main/fast")).await;
        assert_eq!(dropped.headers()["x-smarttaskrouter-dropped"], "cache_control");
        let body: Value = dropped.json().await.unwrap();
        assert_eq!(body["content"][0]["text"], "pong");
    });
}

#[test]
fn anthropic_messages_round_trip_through_translation() {
    rt().block_on(async {
        let rig = rig().await;
        let response = rig.http.post(format!("{}/v1/messages", rig.base)).header("x-api-key", &rig.token)
            .json(&json!({ "model": "main/fast", "max_tokens": 64, "messages": [{ "role": "user", "content": "ping" }] }))
            .send().await.unwrap();
        assert_eq!(response.status(), 200);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["type"], "message");
        assert_eq!(body["content"][0]["text"], "pong");
        assert_eq!(body["usage"]["output_tokens"], 500);

        let streamed = rig.post("/v1/messages", json!({ "model": "main/tooler", "stream": true, "max_tokens": 64,
            "messages": [{ "role": "user", "content": "read a" }] })).await.text().await.unwrap();
        assert!(streamed.starts_with("event: message_start"));
        assert!(streamed.contains("\"type\":\"tool_use\"") && streamed.contains("\"partial_json\":\"\\\"a\\\"}\""));
        assert!(streamed.contains("\"stop_reason\":\"tool_use\"") && streamed.trim_end().ends_with("{\"type\":\"message_stop\"}"));

        let missing = rig.post("/v1/messages", json!({ "model": "claude-sonnet", "messages": [] })).await;
        assert_eq!(missing.status(), 404);
        let body: Value = missing.json().await.unwrap();
        assert_eq!(body["error"]["type"], "not_found_error");
    });
}
