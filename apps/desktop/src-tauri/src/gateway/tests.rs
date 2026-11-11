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
        // Overloaded once, and ready again at once (Retry-After: 0, so no cooldown is left behind).
        "flaky" if !fake.flaky_failed.swap(true, Ordering::SeqCst) => {
            return ([(header::RETRY_AFTER, "0")], (StatusCode::SERVICE_UNAVAILABLE, "try again")).into_response();
        }
        "timeout408" => return (StatusCode::REQUEST_TIMEOUT, "generation took too long").into_response(),
        "gateway504" => return (StatusCode::GATEWAY_TIMEOUT, "upstream timed out").into_response(),
        "broke" => return (StatusCode::INTERNAL_SERVER_ERROR, "boom").into_response(),
        "noquota" => return (StatusCode::TOO_MANY_REQUESTS, Json(json!({ "error": { "message": "You exceeded your current quota",
            "type": "insufficient_quota", "code": "insufficient_quota" } }))).into_response(),
        "daylong" => return ([(header::RETRY_AFTER, "86400")], (StatusCode::TOO_MANY_REQUESTS,
            Json(json!({ "error": { "message": "daily quota" } })))).into_response(),
        "slow" => tokio::time::sleep(Duration::from_millis(1500)).await,
        "slow-ok" => tokio::time::sleep(Duration::from_millis(300)).await,
        _ => {}
    }
    // A stream that takes 1.5 s, for the stop-while-streaming tests (review LC-02).
    if model == "drip" {
        let chunks = futures_util::stream::unfold(0u32, |i| async move {
            if i >= 15 {
                return None;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
            let data = if i == 14 {
                format!("data: {}\n\ndata: [DONE]\n\n", json!({ "choices": [], "usage": { "prompt_tokens": 10, "completion_tokens": 15 } }))
            } else {
                format!("data: {}\n\n", json!({ "choices": [{ "delta": { "content": "x" } }] }))
            };
            Some((Ok::<_, std::io::Error>(Bytes::from(data)), i + 1))
        });
        return ([(header::CONTENT_TYPE, "text/event-stream")], Body::from_stream(chunks)).into_response();
    }
    // A model that reports more than any estimate held for it: 1M prompt tokens.
    let usage = if model == "heavy" { json!({ "prompt_tokens": 1_000_000, "completion_tokens": 500 }) }
        else { json!({ "prompt_tokens": 1000, "completion_tokens": 500 }) };
    // Says whether prompt-cache markers survived the trip, for the translation tests.
    let text = if body.to_string().contains("cache_control") { "pong cache:yes" } else { "pong" };
    if model == "keyorder" {
        // Echoes the key order of the first tool's schema properties, for the passthrough test.
        let order: Vec<String> = body["tools"][0]["function"]["parameters"]["properties"].as_object()
            .map(|m| m.keys().cloned().collect()).unwrap_or_default();
        return ([("x-ratelimit-remaining-requests", "41"), ("x-request-id", "req_7")],
            Json(json!({ "choices": [{ "index": 0, "finish_reason": "stop", "message": { "role": "assistant",
                "content": order.join(",") } }], "usage": usage }))).into_response();
    }
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
    let limits = [("anthropic-ratelimit-requests-remaining", "99"), ("request-id", "req_a1")];
    if body["stream"] == true {
        let sse = format!(
            "event: message_start\ndata: {}\n\nevent: content_block_delta\ndata: {}\n\nevent: message_delta\ndata: {}\n\nevent: message_stop\ndata: {{\"type\":\"message_stop\"}}\n\n",
            json!({ "type": "message_start", "message": { "id": "msg_1", "usage": usage } }),
            json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": seen } }),
            json!({ "type": "message_delta", "delta": { "stop_reason": "end_turn" }, "usage": { "output_tokens": 7 } }));
        return ([(header::CONTENT_TYPE, "text/event-stream")], limits, sse).into_response();
    }
    (limits, Json(json!({ "id": "msg_1", "type": "message", "role": "assistant", "model": model,
        "content": [{ "type": "text", "text": seen }], "stop_reason": "end_turn", "usage": usage }))).into_response()
}

struct Rig {
    base: String,
    token: String,
    client_id: String,
    shared: Shared,
    fake: Arc<Fake>,
    http: reqwest::Client,
    server: tokio::sync::Mutex<Option<Server>>,
}

fn price() -> Price {
    // $0.10 in, $0.40 out per million tokens: 1000 in + 500 out = $0.0001 + $0.0002.
    Price { input_micro_per_mtok: 100_000, output_micro_per_mtok: 400_000, cached_input_micro_per_mtok: None,
        cache_write_micro_per_mtok: None, source: "user".into(), observed_at: 0 }
}

async fn rig_with(read_timeout: Duration) -> Rig {
    rig_on(read_timeout, None).await
}

/// A rig on a store file, so the gateway takes the store's owner lock (reviews PR-01, LC-02).
async fn rig_on_store(path: &std::path::Path) -> Rig {
    rig_on(Duration::from_secs(30), Some(path)).await
}

async fn rig_on(read_timeout: Duration, store_path: Option<&std::path::Path>) -> Rig {
    let fake = Arc::new(Fake::default());
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream.local_addr().unwrap();
    let app = Router::new()
        .route("/v1/chat/completions", post(fake_openai))
        .route("/anthropic/v1/messages", post(fake_anthropic))
        .with_state(fake.clone());
    tokio::spawn(async move { axum::serve(upstream, app).await.unwrap() });

    let conn = match store_path { Some(path) => crate::db::open(path).unwrap(), None => crate::db::open_in_memory().unwrap() };
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
    let shared = match store_path { Some(path) => Shared::for_store(shared.db.clone(), shared.secrets.clone(), path), None => shared };
    let server = start(shared.clone(), 0).await.unwrap();
    Rig { base: format!("http://{}", server.addr), token, client_id: client.id, shared, fake, http: reqwest::Client::new(),
        server: tokio::sync::Mutex::new(Some(server)) }
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

    fn combo(&self, name: &str, members: &[(&str, &str)]) {
        let members: Vec<Target> = members.iter().map(|(p, m)| Target { provider_id: p.to_string(), model_id: m.to_string() }).collect();
        self.shared.with_db(|db| store::save_combo(db, None, name, &members)).unwrap();
    }

    fn priced(&self, provider: &str, model: &str) {
        self.shared.with_db(|db| {
            store::add_model(db, provider, model).unwrap();
            store::update_model(db, provider, model, None, None, Some(Some(price()))).unwrap();
        });
    }

    fn client(&self) -> store::Client {
        self.shared.with_db(store::list_clients).unwrap().into_iter().find(|c| c.id == self.client_id).unwrap()
    }

    /// Stops the gateway, giving open responses `grace` to finish (review LC-02).
    async fn stop_server(&self, grace: Duration) {
        if let Some(server) = self.server.lock().await.take() {
            server.stop_within(grace).await;
        }
    }

    /// Starts the gateway again with the same store; returns its new base URL.
    async fn start_server(&self) -> std::io::Result<String> {
        let server = start(self.shared.clone(), 0).await?;
        let base = format!("http://{}", server.addr);
        *self.server.lock().await = Some(server);
        Ok(base)
    }

    /// Waits until this process no longer owns the store: every attempt of a stopped gateway is settled.
    async fn until_released(&self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.shared.owns_store() {
            assert!(Instant::now() < deadline, "the store was never released");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Opts the test client into trying further models after a server error (review FR-02; the default stops).
    fn allow_paid_retries(&self, n: u32) {
        self.shared.with_db(|db| store::set_client_paid_retries(db, &self.client_id, n)).unwrap();
    }

    fn journal_len(&self) -> i64 {
        self.shared.with_db(|db| db.query_row("SELECT COUNT(*) FROM budget_reservations", [], |r| r.get(0))).unwrap()
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
        rig.allow_paid_retries(1);
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
        rig.allow_paid_retries(1);
        // The first model is overloaded once (503 with Retry-After: 0), so the session lands on the backup.
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

// ---------- Counter-review CR-01: quota scope across requests ----------

#[test]
fn a_cooldown_on_one_key_keeps_its_siblings_off_that_model_in_later_requests() {
    rt().block_on(async {
        let rig = rig().await;
        assert_eq!(rig.post("/v1/chat/completions", chat("main/limited")).await.status(), 429);
        // A later request straight to the sibling key, same model: skipped without a call.
        let sibling = rig.post("/v1/chat/completions", chat("sibling/limited")).await;
        assert_eq!(sibling.status(), 429);
        assert!(sibling.headers().contains_key(header::RETRY_AFTER));
        assert!(sibling.text().await.unwrap().contains("shares limits with Main"));
        assert_eq!(rig.fake.count("limited"), 1);
        // Another model of the sibling is not covered by a per-model cooldown.
        assert_eq!(rig.post("/v1/chat/completions", chat("sibling/steady")).await.status(), 200);
        // A connection confirmed as independent is not held back.
        rig.shared.with_db(|db| store::update_provider(db, "sibling", None, None, Some(true))).unwrap();
        assert_eq!(rig.post("/v1/chat/completions", chat("sibling/limited")).await.status(), 429);
        assert_eq!(rig.fake.count("limited"), 2, "tried, and refused by the provider itself");
        let sibling = rig.shared.with_db(|db| store::get_provider(db, "sibling")).unwrap().unwrap();
        assert!(sibling.separate_quota && sibling.quota_confirmed_at.is_some());
    });
}

#[test]
fn an_account_out_of_credit_cools_the_whole_connection_down() {
    rt().block_on(async {
        let rig = rig().await;
        let response = rig.post("/v1/chat/completions", chat("main/noquota")).await;
        assert_eq!(response.status(), 429);
        let health = rig.shared.with_db(|db| store::list_health(db, Some("main"))).unwrap();
        assert_eq!((health[0].model_id.as_str(), health[0].state.as_str()), ("", "payment_required"));
        assert!(health[0].until_ms.unwrap() - crate::db::now_ms() > 3_000_000);
        // Every model of the connection, and of its siblings, waits.
        assert_eq!(rig.post("/v1/chat/completions", chat("main/fast")).await.status(), 429);
        assert_eq!(rig.post("/v1/chat/completions", chat("sibling/fast")).await.status(), 429);
        assert_eq!(rig.fake.count("fast"), 0);
    });
}

// ---------- CR-04: stated resets ----------

#[test]
fn a_reset_a_day_away_is_kept_not_cut_to_hours() {
    rt().block_on(async {
        let rig = rig().await;
        rig.post("/v1/chat/completions", chat("main/daylong")).await;
        let health = rig.shared.with_db(|db| store::list_health(db, Some("main"))).unwrap();
        let wait = health[0].until_ms.unwrap() - crate::db::now_ms();
        assert!(wait > 86_000_000 && wait <= 86_400_000, "{wait}");
    });
}

// ---------- CR-05: statuses that may come after the provider ran the request ----------

#[test]
fn a_timeout_status_is_not_retried_elsewhere_and_its_cost_is_unknown() {
    rt().block_on(async {
        let rig = rig().await;
        rig.combo("t408", &[("main", "timeout408"), ("backup", "steady")]);
        rig.combo("t504", &[("main", "gateway504"), ("backup", "steady")]);
        for (combo, status) in [("t408", 408), ("t504", 504)] {
            let response = rig.post("/v1/chat/completions", chat(combo)).await;
            assert_eq!(response.status(), status);
            let event = rig.ledger().pop().unwrap();
            assert_eq!((event.outcome.as_str(), event.cost_status.as_str()), ("error", "unknown"), "{combo}");
        }
        assert_eq!(rig.fake.count("steady"), 0, "no second provider was charged");
    });
}

#[test]
fn a_server_error_counts_against_the_limit_at_the_amount_held() {
    rt().block_on(async {
        let rig = rig().await;
        rig.priced("main", "broke");
        rig.priced("main", "broken");
        rig.set_budget(Some(10_000_000));
        assert_eq!(rig.post("/v1/chat/completions", chat("main/broke")).await.status(), 500);
        let event = rig.ledger().pop().unwrap();
        assert_eq!((event.cost_status.as_str(), event.cost_micro_usd), ("unknown", None));
        let held = event.at_risk_micro_usd.expect("the held amount is counted");
        assert!(held > 0);
        let client = rig.client();
        assert_eq!((client.month_at_risk_micro_usd, client.counted_micro_usd()), (held, held));
        // A 4xx rejection is not billed and holds nothing back.
        rig.post("/v1/chat/completions", chat("main/broken")).await;
        assert_eq!(rig.ledger().pop().unwrap().at_risk_micro_usd, None);
    });
}

// ---------- CR-03: the limit is checked against an estimate ----------

#[test]
fn spend_above_the_estimate_is_recorded_and_stops_the_next_request() {
    rt().block_on(async {
        let rig = rig().await;
        rig.priced("main", "heavy");
        // A tiny prompt holds a few cents; the provider then reports 1M prompt tokens ($0.10).
        rig.set_budget(Some(50_000));
        assert_eq!(rig.post("/v1/chat/completions", chat("main/heavy")).await.status(), 200);
        let client = rig.client();
        assert_eq!(client.month_spend_micro_usd, 100_200, "the real cost is recorded, above the limit");
        let refused = rig.post("/v1/chat/completions", chat("main/fast")).await;
        assert_eq!(refused.status(), 429);
        assert!(refused.text().await.unwrap().contains("monthly limit"));
    });
}

#[test]
fn the_estimate_counts_non_latin_text_and_cache_writes() {
    let latin = json!({ "messages": [{ "content": "hello world ".repeat(100) }] });
    let cjk = json!({ "messages": [{ "content": "你好世界".repeat(100) }] });
    let vietnamese = json!({ "messages": [{ "content": "không có gì ".repeat(100) }] });
    // English is about four characters a token; the estimate stays above that.
    assert!(estimate_input_tokens(&latin) >= 1200 / 4);
    // CJK is at least a token a character; bytes / 4 would give 300 for these 400 characters.
    assert!(estimate_input_tokens(&cjk) >= 400);
    assert!(estimate_input_tokens(&vietnamese) > "không có gì ".repeat(100).len() as u64 / 4);
    let mut p = price();
    let plain = estimate_micro(&latin, &p);
    p.cache_write_micro_per_mtok = Some(125_000);
    assert!(estimate_micro(&latin, &p) > plain, "a prompt may be written to the cache at the higher price");
}

// ---------- CR-06: the journal and the ledger ----------

#[test]
fn every_attempt_is_settled_and_leaves_no_journal_entry() {
    rt().block_on(async {
        let rig = rig().await;
        rig.set_budget(Some(10_000_000));
        rig.shared.with_db(|db| store::set_client_policy(db, &rig.client_id, None, Some("allow"))).unwrap();
        rig.allow_paid_retries(1);
        // Two refusals and an answer, a stream, and a native Anthropic answer: five attempts.
        rig.post("/v1/chat/completions", chat("coding")).await;
        rig.post("/v1/chat/completions", json!({ "model": "main/fast", "stream": true, "messages": [] })).await.text().await.unwrap();
        rig.post("/v1/messages", claude_request(false)).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(rig.journal_len(), 0);
        assert_eq!(rig.ledger().len(), 5);
    });
}

#[test]
fn nothing_is_sent_when_the_ledger_cannot_be_written() {
    rt().block_on(async {
        let rig = rig().await;
        rig.shared.with_db(|db| db.execute_batch("PRAGMA query_only = ON")).unwrap();
        let refused = rig.post("/v1/chat/completions", chat("main/fast")).await;
        assert_eq!(refused.status(), 503);
        assert!(refused.text().await.unwrap().contains("nothing was sent"));
        assert_eq!(rig.fake.count("fast"), 0);
        let health: Value = rig.http.get(format!("{}/health", rig.base)).send().await.unwrap().json().await.unwrap();
        assert_eq!(health["accounting"], "degraded");

        rig.shared.with_db(|db| db.execute_batch("PRAGMA query_only = OFF")).unwrap();
        assert_eq!(rig.post("/v1/chat/completions", chat("main/fast")).await.status(), 200);
        let health: Value = rig.http.get(format!("{}/health", rig.base)).send().await.unwrap().json().await.unwrap();
        assert_eq!(health["accounting"], "ok");
    });
}

#[test]
fn an_attempt_never_settled_becomes_an_interrupted_entry() {
    let conn = crate::db::open_in_memory().unwrap();
    let (client, _) = store::create_client(&conn, "Tests", Some(1_000_000)).unwrap();
    let start = store::AttemptStart { client_id: &client.id, provider_id: "main", model_id: "fast", requested_model: "coding",
        session_key: Some("s-9"), route_mode: "native", hold_micro: 4_000, owner_generation: "g1" };
    let id = store::open_attempt(&conn, &start).unwrap().unwrap();
    // The generation that journaled it is still live: its entries are left alone.
    assert_eq!(store::settle_interrupted(&conn, store::Unsettled::NotOf("g1")).unwrap(), 0);
    // A gateway of a new generation took the store: g1 has finished or died.
    assert_eq!(store::settle_interrupted(&conn, store::Unsettled::NotOf("g2")).unwrap(), 1);
    assert_eq!(store::settle_interrupted(&conn, store::Unsettled::NotOf("g3")).unwrap(), 0, "turned exactly once");
    let event = store::usage_summary(&conn, 0, 10).unwrap().recent.remove(0);
    assert_eq!((event.outcome.as_str(), event.cost_status.as_str(), event.at_risk_micro_usd), ("interrupted", "unknown", Some(4_000)));
    assert_eq!((event.provider.as_str(), event.session_key.as_deref()), ("main", Some("s-9")));
    // Settling it afterwards writes nothing twice.
    let cost = pricing::Cost::Estimated { micro_usd: 1, source: "x".into() };
    store::settle(&conn, &id, &UsageRecord::failed(&client.id, "main", "fast", "coding", &cost, "ok", None, 0, "")).unwrap();
    assert_eq!(store::usage_summary(&conn, 0, 10).unwrap().recent.len(), 1);
    assert_eq!(store::list_clients(&conn).unwrap()[0].counted_micro_usd(), 4_000);
}

// ---------- CR-07: sessions ----------

#[test]
fn a_session_id_is_scoped_to_its_client_and_validated() {
    rt().block_on(async {
        let rig = rig().await;
        rig.allow_paid_retries(1);
        let first = rig.post_session("/v1/chat/completions", "shared-id", chat("pinme")).await;
        assert_eq!(first.headers()["x-smarttaskrouter-route"], "backup/steady");
        // Another client naming the same session is not sent to the first client's pin.
        let (_, other) = rig.shared.with_db(|db| store::create_client(db, "Other", None)).unwrap();
        let theirs = rig.http.post(format!("{}/v1/chat/completions", rig.base)).bearer_auth(&other)
            .header(SESSION_HEADER, "shared-id").json(&chat("pinme")).send().await.unwrap();
        assert_eq!(theirs.headers()["x-smarttaskrouter-route"], "main/flaky");
        assert_eq!(theirs.headers()["x-smarttaskrouter-session"], "new");
        // An id that is too long is ignored, not truncated into someone else's.
        let long = rig.post_session("/v1/chat/completions", &"x".repeat(200), chat("main/fast")).await;
        assert!(long.headers().get("x-smarttaskrouter-session").is_none());
    });
    let mut headers = HeaderMap::new();
    headers.insert(SESSION_HEADER, HeaderValue::from_bytes("phiên-1".as_bytes()).unwrap());
    assert_eq!(session_key(&headers), None, "non-ASCII ids are ignored");
    headers.insert(SESSION_HEADER, HeaderValue::from_static("a b"));
    assert_eq!(session_key(&headers), None);
}

#[test]
fn expired_session_pins_are_deleted() {
    let conn = crate::db::open_in_memory().unwrap();
    let (client, _) = store::create_client(&conn, "Tests", None).unwrap();
    let provider = store::add_provider(&conn, &NewProvider { kind: "custom", label: "Main", endpoint: "http://x.test",
        credential_ref: None, default_billing: "paid", wire: "openai" }).unwrap();
    let target = Target { provider_id: provider.id.clone(), model_id: "m".into() };
    store::pin(&conn, &client.id, "old", "m", &target).unwrap();
    conn.execute("UPDATE session_affinity SET last_used_at = 0", []).unwrap();
    store::pin(&conn, &client.id, "new", "m", &target).unwrap();
    let left: i64 = conn.query_row("SELECT COUNT(*) FROM session_affinity", [], |r| r.get(0)).unwrap();
    assert_eq!(left, 1);
}

// ---------- CR-08: what a native passthrough keeps ----------

#[test]
fn a_native_request_keeps_its_key_order_and_the_providers_limit_headers() {
    rt().block_on(async {
        let rig = rig().await;
        let response = rig.post("/v1/chat/completions", json!({ "model": "main/keyorder", "messages": [],
            "tools": [{ "type": "function", "function": { "name": "f", "parameters": { "type": "object",
                "properties": { "zeta": {}, "alpha": {}, "mid": {} } } } }] })).await;
        assert_eq!(response.headers()["x-ratelimit-remaining-requests"], "41");
        assert_eq!(response.headers()["x-request-id"], "req_7");
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["choices"][0]["message"]["content"], "zeta,alpha,mid");

        let claude = rig.post("/v1/messages", claude_request(false)).await;
        assert_eq!(claude.headers()["anthropic-ratelimit-requests-remaining"], "99");
        assert_eq!(claude.headers()["request-id"], "req_a1");
    });
}

// ---------- Review FR-01: a client's connection list ----------

#[test]
fn a_restricted_client_never_reaches_a_connection_outside_its_list() {
    rt().block_on(async {
        let rig = rig().await;
        rig.allow_paid_retries(1);
        rig.combo("private", &[("main", "limited"), ("backup", "steady")]);
        // Client A may use Main only; client B may use Main and Backup.
        rig.shared.with_db(|db| store::set_client_connections(db, &rig.client_id, Some(&["main".to_string()]))).unwrap();
        let (b, b_token) = rig.shared.with_db(|db| store::create_client(db, "Vetted", None)).unwrap();
        rig.shared.with_db(|db| store::set_client_connections(db, &b.id, Some(&["main".to_string(), "backup".to_string()])))
            .unwrap();

        let a = rig.post("/v1/chat/completions", json!({ "model": "private",
            "messages": [{ "role": "user", "content": "secret-prompt-text" }] })).await;
        assert_eq!(a.status(), 429, "Main's refusal is returned; Backup is never tried");
        assert!(!a.text().await.unwrap().contains("secret-prompt-text"), "no prompt in the diagnostics");
        assert_eq!(rig.fake.count("steady"), 0);

        // A session pin cannot route A there either.
        rig.shared.with_db(|db| store::pin(db, &rig.client_id, "s-a", "private",
            &Target { provider_id: "backup".into(), model_id: "steady".into() })).unwrap();
        rig.post_session("/v1/chat/completions", "s-a", chat("private")).await;
        assert_eq!(rig.fake.count("steady"), 0);
        // Nor a direct request, and the refusal says why.
        let direct = rig.post("/v1/chat/completions", chat("backup/steady")).await;
        assert_eq!(direct.status(), 502);
        assert!(direct.text().await.unwrap().contains("not allowed to use Backup"));
        assert_eq!(rig.fake.count("steady"), 0);

        // B falls back as the combo says (Main is cooling down, so B goes straight to Backup).
        let vetted = rig.http.post(format!("{}/v1/chat/completions", rig.base)).bearer_auth(&b_token).json(&chat("private"))
            .send().await.unwrap();
        assert_eq!(vetted.headers()["x-smarttaskrouter-route"], "backup/steady");

        // The model list shows A only what it may use.
        let models: Value = rig.http.get(format!("{}/v1/models", rig.base)).bearer_auth(&rig.token).send().await.unwrap()
            .json().await.unwrap();
        let ids: Vec<&str> = models["data"].as_array().unwrap().iter().filter_map(|m| m["id"].as_str()).collect();
        assert!(ids.iter().any(|id| id.starts_with("main/")) && !ids.iter().any(|id| id.starts_with("claude/")));

        // An empty list allows nothing.
        rig.shared.with_db(|db| store::set_client_connections(db, &rig.client_id, Some(&[]))).unwrap();
        assert_eq!(rig.post("/v1/chat/completions", chat("main/fast")).await.status(), 502);
        assert_eq!(rig.fake.count("fast"), 0);
    });
}

// ---------- Review FR-02: no paid retry by default ----------

#[test]
fn a_server_error_stops_by_default_and_retries_only_when_the_client_allows_it() {
    rt().block_on(async {
        let rig = rig().await;
        rig.combo("overloaded", &[("main", "broke"), ("backup", "steady")]);
        let stopped = rig.post("/v1/chat/completions", chat("overloaded")).await;
        assert_eq!(stopped.status(), 500, "the provider's own error is returned");
        assert_eq!(rig.fake.count("steady"), 0, "no second provider was charged");
        let event = rig.ledger().pop().unwrap();
        assert_eq!((event.outcome.as_str(), event.cost_status.as_str()), ("error", "unknown"));

        // Opted in: one retry is allowed, and both attempts are in the ledger.
        rig.shared.with_db(|db| store::clear_health(db, "main")).unwrap();
        rig.allow_paid_retries(1);
        let retried = rig.post("/v1/chat/completions", chat("overloaded")).await;
        assert_eq!(retried.headers()["x-smarttaskrouter-route"], "backup/steady");
        let outcomes: Vec<String> = rig.ledger().into_iter().rev().take(2).map(|e| e.outcome).collect();
        assert_eq!(outcomes, ["ok", "fallback"]);

        // A free or local label does not make a replay safe: without the opt-in, a free connection's 5xx stops
        // too (review PR-02).
        rig.allow_paid_retries(0);
        rig.combo("free-first", &[("router", "broke"), ("backup", "steady")]);
        let free = rig.post("/v1/chat/completions", chat("free-first")).await;
        assert_eq!(free.status(), 500);
        assert_eq!(rig.fake.count("steady"), 1, "only the opted-in retry above reached Backup");

        // Retries are bounded by the setting, not only by the combo: one retry over three failing models.
        rig.shared.with_db(|db| store::clear_health(db, "main")).unwrap();
        rig.shared.with_db(|db| store::clear_health(db, "router")).unwrap();
        rig.combo("three", &[("main", "broke"), ("router", "broke"), ("backup", "steady")]);
        rig.allow_paid_retries(1);
        let before = rig.fake.count("broke");
        let bounded = rig.post("/v1/chat/completions", chat("three")).await;
        assert_eq!(bounded.status(), 500);
        assert_eq!((rig.fake.count("broke") - before, rig.fake.count("steady")), (2, 1), "two calls, then stop");
    });
}

// ---------- Review FR-03: no paid admission while a settlement waits ----------

#[test]
fn a_settlement_that_cannot_be_written_holds_paid_requests_until_it_is() {
    rt().block_on(async {
        let rig = rig().await;
        // The store turns read-only while the answer is on its way, so its ledger entry cannot be written.
        let (answer, _) = tokio::join!(rig.post("/v1/chat/completions", chat("main/slow-ok")), async {
            tokio::time::sleep(Duration::from_millis(120)).await;
            rig.shared.with_db(|db| db.execute_batch("PRAGMA query_only = ON")).unwrap();
        });
        assert_eq!(answer.status(), 200, "the client still gets its answer");
        assert_eq!(rig.shared.pending_settlements(), 1);
        let health: Value = rig.http.get(format!("{}/health", rig.base)).send().await.unwrap().json().await.unwrap();
        assert_eq!(health["accounting"], "degraded");

        // No paid request is sent while it waits.
        let held = rig.post("/v1/chat/completions", chat("main/fast")).await;
        assert_eq!(held.status(), 503);
        assert!(held.text().await.unwrap().contains("wait until the usage ledger"));
        assert_eq!(rig.fake.count("fast"), 0);

        // Once the store accepts writes, the entry is written exactly as it was, once, and requests flow again.
        rig.shared.with_db(|db| db.execute_batch("PRAGMA query_only = OFF")).unwrap();
        assert_eq!(rig.post("/v1/chat/completions", chat("main/fast")).await.status(), 200);
        assert_eq!(rig.shared.pending_settlements(), 0);
        let ledger = rig.ledger();
        let slow: Vec<_> = ledger.iter().filter(|e| e.model == "slow-ok").collect();
        assert_eq!(slow.len(), 1);
        assert_eq!((slow[0].outcome.as_str(), slow[0].cost_micro_usd), ("ok", Some(300)));
        assert_eq!(rig.journal_len(), 0);
        let health: Value = rig.http.get(format!("{}/health", rig.base)).send().await.unwrap().json().await.unwrap();
        assert_eq!(health["accounting"], "ok");
    });
}

// ---------- Review PR-01: one gateway per store ----------

#[test]
fn a_second_gateway_on_the_same_store_is_refused_until_the_first_stops() {
    rt().block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.sqlite3");
        let open = || {
            let conn = crate::db::open(&path).unwrap();
            Shared::for_store(Arc::new(Mutex::new(conn)), Arc::new(MapSecrets(HashMap::new())), &path)
        };
        let first = start(open(), 0).await.expect("the first gateway owns the store");
        let second = start(open(), 0).await;
        let error = second.err().expect("a second gateway is refused");
        assert_eq!(error.kind(), std::io::ErrorKind::ResourceBusy);
        assert!(error.to_string().contains("Another gateway is already serving this store"));
        // Stopping releases the store for the other one.
        first.stop().await;
        let third = start(open(), 0).await.expect("free again once the first stopped");
        third.stop().await;
    });
}

#[test]
fn owning_the_store_turns_every_older_unsettled_attempt_at_once() {
    rt().block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.sqlite3");
        let conn = crate::db::open(&path).unwrap();
        let (client, _) = store::create_client(&conn, "Tests", None).unwrap();
        // An entry left a moment ago by a gateway of an earlier generation that died: no one else can own it.
        let start_row = store::AttemptStart { client_id: &client.id, provider_id: "main", model_id: "fast",
            requested_model: "main/fast", session_key: None, route_mode: "native", hold_micro: 0, owner_generation: "dead" };
        store::open_attempt(&conn, &start_row).unwrap().unwrap();
        let shared = Shared::for_store(Arc::new(Mutex::new(conn)), Arc::new(MapSecrets(HashMap::new())), &path);
        let server = start(shared.clone(), 0).await.unwrap();
        let events = shared.with_db(|db| store::usage_summary(db, 0, 10)).unwrap().recent;
        assert_eq!((events.len(), events[0].outcome.as_str()), (1, "interrupted"));
        server.stop().await;
    });
}

// ---------- Review LC-01 to LC-04: gateway lifecycle ----------

fn other_gateway(path: &std::path::Path) -> Shared {
    let conn = crate::db::open(path).unwrap();
    Shared::for_store(Arc::new(Mutex::new(conn)), Arc::new(MapSecrets(HashMap::new())), path)
}

fn count_events(rig: &Rig, model: &str) -> (usize, usize) {
    let ledger = rig.ledger();
    let mine: Vec<_> = ledger.iter().filter(|e| e.model == model).collect();
    (mine.len(), mine.iter().filter(|e| e.outcome == "interrupted").count())
}

#[test]
fn stopping_keeps_the_store_owned_until_a_request_in_flight_is_settled() {
    rt().block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.sqlite3");
        let rig = rig_on_store(&path).await;
        // A 1.5 s answer, stopped 300 ms in with a 100 ms grace: the listener ends long before the answer.
        let request = { let (http, base, token) = (rig.http.clone(), rig.base.clone(), rig.token.clone());
            tokio::spawn(async move { http.post(format!("{base}/v1/chat/completions")).bearer_auth(token)
                .json(&chat("main/slow")).send().await.map(|r| r.status().as_u16()) }) };
        tokio::time::sleep(Duration::from_millis(300)).await;
        rig.stop_server(Duration::from_millis(100)).await;

        // The attempt still holds the store: another gateway cannot take it over a live attempt.
        assert!(rig.shared.owns_store());
        let refused = start(other_gateway(&path), 0).await.err().expect("refused while the attempt runs");
        assert_eq!(refused.kind(), std::io::ErrorKind::ResourceBusy);

        let _ = request.await;
        rig.until_released().await;
        let next = start(other_gateway(&path), 0).await.expect("free once the attempt is settled");
        // Settled once, by its own gateway, with its real outcome; not turned into an interrupted entry.
        assert_eq!(count_events(&rig, "slow"), (1, 0));
        assert_eq!(rig.ledger().iter().find(|e| e.model == "slow").unwrap().outcome, "ok");
        assert_eq!(rig.journal_len(), 0);
        next.stop().await;
    });
}

#[test]
fn stopping_while_streaming_keeps_the_store_until_the_stream_is_settled() {
    rt().block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.sqlite3");
        let rig = rig_on_store(&path).await;
        let stream = { let (http, base, token) = (rig.http.clone(), rig.base.clone(), rig.token.clone());
            tokio::spawn(async move {
                let response = http.post(format!("{base}/v1/chat/completions")).bearer_auth(token)
                    .json(&json!({ "model": "main/drip", "stream": true, "messages": [] })).send().await;
                match response { Ok(r) => r.text().await.map(|t| t.len()).unwrap_or(0), Err(_) => 0 }
            }) };
        tokio::time::sleep(Duration::from_millis(400)).await;
        rig.stop_server(Duration::from_millis(100)).await;
        assert!(rig.shared.owns_store(), "the stream pump still holds the store");
        assert_eq!(start(other_gateway(&path), 0).await.err().map(|e| e.kind()), Some(std::io::ErrorKind::ResourceBusy));

        let _ = stream.await;
        rig.until_released().await;
        assert_eq!(count_events(&rig, "drip"), (1, 0), "one entry, by the stream itself");
        assert_eq!(rig.journal_len(), 0);
        start(other_gateway(&path), 0).await.expect("free once the stream is settled").stop().await;
    });
}

#[test]
fn restarting_in_the_same_process_recovers_orphans_once_and_never_interrupts_live_work() {
    rt().block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.sqlite3");
        let rig = rig_on_store(&path).await;
        // An orphan of an earlier generation (journaled, never settled, its holder gone).
        let orphan = store::AttemptStart { client_id: &rig.client_id, provider_id: "main", model_id: "orphan",
            requested_model: "main/orphan", session_key: None, route_mode: "native", hold_micro: 700,
            owner_generation: "earlier" };
        rig.shared.with_db(|db| store::open_attempt(db, &orphan)).unwrap().unwrap();

        // Stop and start several times in a row, quickly: the orphan becomes exactly one interrupted entry.
        let mut base = rig.base.clone();
        for _ in 0..4 {
            rig.stop_server(Duration::from_millis(50)).await;
            rig.until_released().await;
            base = rig.start_server().await.expect("restarts in the same process");
        }
        assert_eq!(count_events(&rig, "orphan"), (1, 1));
        assert_eq!(rig.ledger().iter().find(|e| e.model == "orphan").unwrap().at_risk_micro_usd, Some(700));

        // Restarting while an attempt is in flight joins its ownership: the live attempt is not interrupted.
        let request = { let (http, token) = (rig.http.clone(), rig.token.clone());
            tokio::spawn(async move { http.post(format!("{base}/v1/chat/completions")).bearer_auth(token)
                .json(&chat("main/slow")).send().await.map(|r| r.status().as_u16()) }) };
        tokio::time::sleep(Duration::from_millis(300)).await;
        rig.stop_server(Duration::from_millis(50)).await;
        assert!(rig.shared.owns_store());
        rig.start_server().await.expect("joins the ownership that is still settling");
        let _ = request.await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(count_events(&rig, "slow"), (1, 0), "settled by its own attempt, not interrupted");
        assert_eq!(rig.journal_len(), 0);
        rig.stop_server(Duration::from_millis(50)).await;
    });
}

#[test]
fn a_failed_startup_recovery_refuses_to_start_and_succeeds_once_the_store_is_writable() {
    rt().block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.sqlite3");
        let shared = other_gateway(&path);
        let client = shared.with_db(|db| store::create_client(db, "Tests", None)).unwrap().0;
        let orphan = store::AttemptStart { client_id: &client.id, provider_id: "main", model_id: "orphan",
            requested_model: "main/orphan", session_key: None, route_mode: "native", hold_micro: 0, owner_generation: "dead" };
        shared.with_db(|db| store::open_attempt(db, &orphan)).unwrap().unwrap();

        shared.with_db(|db| db.execute_batch("PRAGMA query_only = ON")).unwrap();
        let failed = start(shared.clone(), 0).await.err().expect("no gateway over unaccounted attempts");
        assert!(failed.to_string().contains("could not record the attempts a previous gateway left unsettled"));
        assert!(!shared.owns_store(), "the lock is released after a failed start");

        shared.with_db(|db| db.execute_batch("PRAGMA query_only = OFF")).unwrap();
        let server = start(shared.clone(), 0).await.expect("starts once the store is writable");
        let health: Value = reqwest::get(format!("http://{}/health", server.addr)).await.unwrap().json().await.unwrap();
        assert_eq!(health["accounting"], "ok");
        let events = shared.with_db(|db| store::usage_summary(db, 0, 10)).unwrap().recent;
        assert_eq!((events.len(), events[0].outcome.as_str()), (1, "interrupted"));
        server.stop().await;
    });
}

#[test]
fn a_settlement_waiting_at_stop_keeps_the_store_until_it_is_written() {
    rt().block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.sqlite3");
        let rig = rig_on_store(&path).await;
        let (answer, _) = tokio::join!(rig.post("/v1/chat/completions", chat("main/slow-ok")), async {
            tokio::time::sleep(Duration::from_millis(120)).await;
            rig.shared.with_db(|db| db.execute_batch("PRAGMA query_only = ON")).unwrap();
        });
        assert_eq!(answer.status(), 200);
        assert_eq!(rig.shared.pending_settlements(), 1);

        // Stopped with a settlement waiting: the store is not handed over, so no other gateway can turn the
        // entry into an interrupted one while its exact figures are still in memory.
        rig.stop_server(Duration::from_millis(50)).await;
        assert!(rig.shared.owns_store());
        assert_eq!(start(other_gateway(&path), 0).await.err().map(|e| e.kind()), Some(std::io::ErrorKind::ResourceBusy));

        // Started again once the store is writable: the entry is written exactly, and the store can be released.
        rig.shared.with_db(|db| db.execute_batch("PRAGMA query_only = OFF")).unwrap();
        rig.start_server().await.unwrap();
        assert_eq!(rig.shared.pending_settlements(), 0);
        assert_eq!(count_events(&rig, "slow-ok"), (1, 0));
        assert_eq!(rig.ledger().iter().find(|e| e.model == "slow-ok").unwrap().cost_micro_usd, Some(300));
        rig.stop_server(Duration::from_millis(50)).await;
        rig.until_released().await;
    });
}

// ---------- ROADMAP P0-5: rule-based Auto routing ----------

fn set_caps(rig: &Rig, provider: &str, model: &str, tier: &str, tools: Option<bool>) {
    rig.shared.with_db(|db| {
        let _ = store::add_model(db, provider, model);
        store::set_capabilities(db, provider, model, &store::Capabilities { tier: Some(tier.into()),
            context_tokens: Some(128_000), max_output_tokens: Some(8192), supports_tools: tools, supports_vision: Some(false),
            ..store::Capabilities::default() })
    }).unwrap();
}

#[test]
fn auto_routes_to_the_lowest_eligible_tier_and_says_why() {
    rt().block_on(async {
        let rig = rig().await;
        set_caps(&rig, "main", "fast", "T2", Some(true));
        set_caps(&rig, "backup", "steady", "T2", None);
        let response = rig.post("/v1/chat/completions", chat("auto")).await;
        assert_eq!(response.status(), 200);
        // Same tier, and Backup is free: it comes first.
        assert_eq!(response.headers()["x-smarttaskrouter-route"], "backup/steady");
        let why = response.headers()["x-smarttaskrouter-auto"].to_str().unwrap().to_string();
        assert!(why.starts_with("auto; backup/steady ranked 1 of 2 (lowest eligible tier first): tier T2 (minimum T2); free"), "{why}");
        assert!(rig.ledger().pop().unwrap().route_reason.unwrap().contains("tier T2"));

        // A request with tools needs a model known to take them: Backup's support is unknown.
        let tools = rig.post("/v1/chat/completions", json!({ "model": "auto", "messages": [],
            "tools": [{ "type": "function", "function": { "name": "f", "parameters": { "type": "object" } } }] })).await;
        assert_eq!(tools.headers()["x-smarttaskrouter-route"], "main/fast");

        // auto/cheap: the free one, by estimated cost.
        let cheap = rig.post("/v1/chat/completions", chat("auto/cheap")).await;
        assert_eq!(cheap.headers()["x-smarttaskrouter-route"], "backup/steady");

        // The aliases are listed with the models.
        let models: Value = rig.http.get(format!("{}/v1/models", rig.base)).bearer_auth(&rig.token).send().await.unwrap()
            .json().await.unwrap();
        let ids: Vec<&str> = models["data"].as_array().unwrap().iter().filter_map(|m| m["id"].as_str()).collect();
        assert!(ids.contains(&"auto") && ids.contains(&"auto/cheap"));
    });
}

#[test]
fn auto_refuses_with_every_reason_when_nothing_is_eligible() {
    rt().block_on(async {
        let rig = rig().await;
        // Models exist but nothing is known about them.
        let refused = rig.post("/v1/chat/completions", chat("auto")).await;
        assert_eq!(refused.status(), 422);
        let body: Value = refused.json().await.unwrap();
        assert_eq!(body["error"]["type"], "no_eligible_model");
        let message = body["error"]["message"].as_str().unwrap();
        assert!(message.contains("main/fast: no tier set"), "{message}");
        assert_eq!(rig.fake.count("fast"), 0, "nothing was sent");
        // An alias that does not exist is not guessed at.
        assert_eq!(rig.post("/v1/chat/completions", chat("auto/fast")).await.status(), 404);
    });
}

#[test]
fn an_auto_session_stays_on_its_model_when_a_better_one_appears() {
    rt().block_on(async {
        let rig = rig().await;
        set_caps(&rig, "main", "fast", "T2", Some(true));
        let first = rig.post_session("/v1/chat/completions", "s-auto", chat("auto")).await;
        assert_eq!(first.headers()["x-smarttaskrouter-route"], "main/fast");
        // A free T2 model becomes eligible.
        set_caps(&rig, "backup", "steady", "T2", None);
        let kept = rig.post_session("/v1/chat/completions", "s-auto", chat("auto")).await;
        assert_eq!(kept.headers()["x-smarttaskrouter-route"], "main/fast", "no switch inside a session");
        assert!(kept.headers()["x-smarttaskrouter-auto"].to_str().unwrap().ends_with("kept for this session"));
        // A new session or no session gets the better choice.
        let fresh = rig.post("/v1/chat/completions", chat("auto")).await;
        assert_eq!(fresh.headers()["x-smarttaskrouter-route"], "backup/steady");
    });
}

#[test]
fn auto_respects_the_client_minimum_tier_and_its_connection_list() {
    rt().block_on(async {
        let rig = rig().await;
        set_caps(&rig, "main", "fast", "T3", Some(true));
        set_caps(&rig, "backup", "steady", "T1", None);
        assert_eq!(rig.post("/v1/chat/completions", chat("auto")).await.headers()["x-smarttaskrouter-route"], "main/fast");
        rig.shared.with_db(|db| store::set_client_auto_min_tier(db, &rig.client_id, "T1")).unwrap();
        assert_eq!(rig.post("/v1/chat/completions", chat("auto")).await.headers()["x-smarttaskrouter-route"], "backup/steady");
        rig.shared.with_db(|db| store::set_client_connections(db, &rig.client_id, Some(&["main".to_string()]))).unwrap();
        assert_eq!(rig.post("/v1/chat/completions", chat("auto")).await.headers()["x-smarttaskrouter-route"], "main/fast");
    });
}

#[test]
fn a_listed_model_keeps_what_the_user_set() {
    let conn = crate::db::open_in_memory().unwrap();
    let p = store::add_provider(&conn, &NewProvider { kind: "openrouter", label: "Router", endpoint: "https://o.test",
        credential_ref: None, default_billing: "paid", wire: "openai" }).unwrap();
    let listed = |context: u64| store::Listed { id: "vendor/m".into(), context_tokens: Some(context), max_output_tokens: Some(4096),
        supports_tools: Some(true), supports_vision: Some(false), price: Some(Price { input_micro_per_mtok: 1, output_micro_per_mtok: 2,
            cached_input_micro_per_mtok: None, cache_write_micro_per_mtok: None, source: "provider list price".into(), observed_at: 0 }) };
    store::import_listed(&conn, &p.id, &[listed(100_000)], Some("provider: openrouter /models")).unwrap();
    let m = store::list_models(&conn, Some(&p.id)).unwrap().remove(0);
    assert_eq!((m.capabilities.context_tokens, m.capabilities.source.as_deref()), (Some(100_000), Some("provider: openrouter /models")));
    assert_eq!(m.price.as_ref().unwrap().source, "provider list price");

    // The user sets a tier and their own price; a later refresh changes neither.
    store::set_capabilities(&conn, &p.id, "vendor/m", &store::Capabilities { tier: Some("T2".into()), context_tokens: Some(64_000),
        ..m.capabilities.clone() }).unwrap();
    store::update_model(&conn, &p.id, "vendor/m", None, None, Some(Some(price()))).unwrap();
    store::import_listed(&conn, &p.id, &[listed(200_000)], Some("provider: openrouter /models")).unwrap();
    let m = store::list_models(&conn, Some(&p.id)).unwrap().remove(0);
    assert_eq!((m.capabilities.tier.as_deref(), m.capabilities.context_tokens, m.capabilities.source.as_deref()),
        (Some("T2"), Some(64_000), Some("user")));
    assert_eq!(m.price.unwrap().source, "user");

    // "auto" is reserved.
    let auto = store::add_provider(&conn, &NewProvider { kind: "custom", label: "Auto", endpoint: "https://a.test",
        credential_ref: None, default_billing: "paid", wire: "openai" }).unwrap();
    assert_eq!(auto.id, "auto-connection");
    assert!(store::save_combo(&conn, None, "auto", &[Target { provider_id: p.id.clone(), model_id: "vendor/m".into() }]).is_err());
}
