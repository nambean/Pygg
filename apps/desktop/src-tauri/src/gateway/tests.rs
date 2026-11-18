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
    /// The last request body the fake received, for the translation tests.
    last_body: Mutex<Option<Value>>,
    /// The keys model `ring` was asked with, in order (key rings, ADR-038).
    keys: Mutex<Vec<String>>,
    /// The headers of the last Chat Completions request, for what is passed on to a provider.
    last_headers: Mutex<Option<HeaderMap>>,
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
    *fake.last_body.lock().unwrap() = Some(body.clone());
    *fake.last_headers.lock().unwrap() = Some(headers.clone());
    if model == "badkey" {
        return (StatusCode::UNAUTHORIZED, Json(json!({ "error": { "message": "invalid api key" } }))).into_response();
    }
    // Model `ring` answers by key, as separate accounts would: one rejected, one rate-limited, the others well.
    if model == "ring" {
        let key = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .unwrap_or_default()
            .to_string();
        fake.keys.lock().unwrap().push(key.clone());
        match key.as_str() {
            "key-bad" => return (StatusCode::UNAUTHORIZED, Json(json!({ "error": { "message": "invalid api key" } }))).into_response(),
            "key-limited" => return ([(header::RETRY_AFTER, "120")], (StatusCode::TOO_MANY_REQUESTS,
                Json(json!({ "error": { "message": "rate limit" } })))).into_response(),
            _ => return ([("x-ratelimit-limit-requests", "100"), ("x-ratelimit-remaining-requests", "99")],
                Json(json!({ "id": "c", "object": "chat.completion", "model": model, "choices": [{ "index": 0,
                "message": { "role": "assistant", "content": format!("answered with {key}") }, "finish_reason": "stop" }],
                "usage": { "prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15 } }))).into_response(),
        }
    }
    if headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) != Some("Bearer key-1") {
        return (StatusCode::UNAUTHORIZED, Json(json!({ "error": { "message": "bad key" } }))).into_response();
    }
    match model.as_str() {
        "limited" => {
            return (
                [(header::RETRY_AFTER, "120")],
                (StatusCode::TOO_MANY_REQUESTS, Json(json!({ "error": { "message": "quota" } }))),
            )
                .into_response()
        }
        "forbidden" => {
            return (StatusCode::FORBIDDEN, Json(json!({ "error": { "message": "not allowed" } }))).into_response()
        }
        "broken" => {
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": { "message": "bad field" } }))).into_response()
        }
        "down" => return (StatusCode::SERVICE_UNAVAILABLE, "overloaded").into_response(),
        // Overloaded once, and ready again at once (Retry-After: 0, so no cooldown is left behind).
        "flaky" if !fake.flaky_failed.swap(true, Ordering::SeqCst) => {
            return ([(header::RETRY_AFTER, "0")], (StatusCode::SERVICE_UNAVAILABLE, "try again")).into_response();
        }
        "timeout408" => return (StatusCode::REQUEST_TIMEOUT, "generation took too long").into_response(),
        "gateway504" => return (StatusCode::GATEWAY_TIMEOUT, "upstream timed out").into_response(),
        "broke" => return (StatusCode::INTERNAL_SERVER_ERROR, "boom").into_response(),
        // OpenRouter's answer on an account with no credit (its `code` is a number).
        "nocredit" | "nocredit:free" => {
            return (
                StatusCode::PAYMENT_REQUIRED,
                Json(json!({ "error": {
            "message": "Insufficient credits", "code": 402 } })),
            )
                .into_response()
        }
        "noquota" => {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({ "error": { "message": "You exceeded your current quota",
            "type": "insufficient_quota", "code": "insufficient_quota" } })),
            )
                .into_response()
        }
        "daylong" => {
            return (
                [(header::RETRY_AFTER, "86400")],
                (StatusCode::TOO_MANY_REQUESTS, Json(json!({ "error": { "message": "daily quota" } }))),
            )
                .into_response()
        }
        // Gemini's answer for a model the key's tier does not serve: 429 "quota", no reset (CCHT benchmark, 2026-11-13).
        "quota429" => {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!([{ "error": { "code": 429, "status": "RESOURCE_EXHAUSTED",
            "message": "You exceeded your current quota, please check your plan and billing details." } }])),
            )
                .into_response()
        }
        "slow" => tokio::time::sleep(Duration::from_millis(1500)).await,
        "slow-ok" => tokio::time::sleep(Duration::from_millis(300)).await,
        _ => {}
    }
    // A stream that takes 1.5 s, for the stop-while-streaming tests (review LC-02).
    // A 200 with nothing in its stream: Gemini sometimes answers so (P0 benchmark, 2026-11-12).
    if model == "empty" && body["stream"] == true {
        return ([(header::CONTENT_TYPE, "text/event-stream")], "data: [DONE]\n\n").into_response();
    }
    if model == "drip" {
        let chunks = futures_util::stream::unfold(0u32, |i| async move {
            if i >= 15 {
                return None;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
            let data = if i == 14 {
                format!(
                    "data: {}\n\ndata: [DONE]\n\n",
                    json!({ "choices": [], "usage": { "prompt_tokens": 10, "completion_tokens": 15 } })
                )
            } else {
                format!("data: {}\n\n", json!({ "choices": [{ "delta": { "content": "x" } }] }))
            };
            Some((Ok::<_, std::io::Error>(Bytes::from(data)), i + 1))
        });
        return ([(header::CONTENT_TYPE, "text/event-stream")], Body::from_stream(chunks)).into_response();
    }
    // A model that reports more than any estimate held for it: 1M prompt tokens.
    let usage = if model == "heavy" {
        json!({ "prompt_tokens": 1_000_000, "completion_tokens": 500 })
    } else {
        json!({ "prompt_tokens": 1000, "completion_tokens": 500 })
    };
    // Says whether prompt-cache markers survived the trip, for the translation tests.
    let text = if body.to_string().contains("cache_control") { "pong cache:yes" } else { "pong" };
    if model == "keyorder" {
        // Echoes the key order of the first tool's schema properties, for the passthrough test.
        let order: Vec<String> = body["tools"][0]["function"]["parameters"]["properties"]
            .as_object()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        return (
            [("x-ratelimit-remaining-requests", "41"), ("x-request-id", "req_7")],
            Json(json!({ "choices": [{ "index": 0, "finish_reason": "stop", "message": { "role": "assistant",
                "content": order.join(",") } }], "usage": usage })),
        )
            .into_response();
    }
    if body["stream"] == true {
        assert_eq!(body["stream_options"]["include_usage"], true, "the gateway asks for usage");
        let mut sse = String::new();
        if model == "patcher" {
            // A call to Codex's free-form `apply_patch`, which reached the provider as a function taking `input`.
            sse.push_str(&format!(
                "data: {}\n\n",
                json!({ "choices": [{ "delta": { "tool_calls": [{ "index": 0, "id": "call_p",
                "function": { "name": "apply_patch", "arguments": "{\"input\":\"*** Begin Patch\\n" } }] } }] })
            ));
            sse.push_str(&format!(
                "data: {}\n\n",
                json!({ "choices": [{ "delta": { "tool_calls": [{ "index": 0,
                "function": { "arguments": "*** End Patch\"}" } }] }, "finish_reason": "tool_calls" }] })
            ));
        } else if model == "tooler" {
            sse.push_str(&format!(
                "data: {}\n\n",
                json!({ "choices": [{ "delta": { "tool_calls": [{ "index": 0, "id": "call_1",
                "function": { "name": "read", "arguments": "{\"path\":" } }] } }] })
            ));
            sse.push_str(&format!(
                "data: {}\n\n",
                json!({ "choices": [{ "delta": { "tool_calls": [{ "index": 0,
                "function": { "arguments": "\"a\"}" } }] }, "finish_reason": "tool_calls" }] })
            ));
        } else {
            sse.push_str(&format!("data: {}\n\n", json!({ "choices": [{ "delta": { "content": "po" } }] })));
            sse.push_str(&format!(
                "data: {}\n\n",
                json!({ "choices": [{ "delta": { "content": "ng" }, "finish_reason": "stop" }] })
            ));
        }
        sse.push_str(&format!("data: {}\n\ndata: [DONE]\n\n", json!({ "choices": [], "usage": usage })));
        return ([(header::CONTENT_TYPE, "text/event-stream")], sse).into_response();
    }
    Json(json!({ "id": "x", "object": "chat.completion", "model": model,
        "choices": [{ "index": 0, "finish_reason": "stop", "message": { "role": "assistant", "content": text } }],
        "usage": usage }))
    .into_response()
}

/// The fake Anthropic provider: answers with what it received, so passthrough can be checked field by field.
async fn fake_anthropic(
    State(fake): State<Arc<Fake>>, headers: HeaderMap, JsonBody(body): JsonBody<Value>,
) -> Response {
    let model = body["model"].as_str().unwrap_or("").to_string();
    *fake.calls.lock().unwrap().entry(model.clone()).or_default() += 1;
    *fake.last_body.lock().unwrap() = Some(body.clone());
    if headers.get("x-api-key").and_then(|v| v.to_str().ok()) != Some("key-1")
        || headers.get("anthropic-version").is_none()
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "type": "error", "error": { "type": "authentication_error",
            "message": "missing key or version" } })),
        )
            .into_response();
    }
    if model == "limited" {
        return (
            [(header::RETRY_AFTER, "30")],
            (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({ "type": "error",
            "error": { "type": "rate_limit_error", "message": "slow down" } })),
            ),
        )
            .into_response();
    }
    // Claude Haiku 5.5 to a conversation that ends with an assistant turn (CCHT benchmark, 2026-11-13).
    if model == "noprefill"
        && body["messages"].as_array().and_then(|m| m.last()).is_some_and(|m| m["role"] == "assistant")
    {
        return (StatusCode::BAD_REQUEST, Json(json!({ "type": "error", "error": { "type": "invalid_request_error",
            "message": "This model does not support assistant message prefill. The conversation must end with a user message." } })))
            .into_response();
    }
    // Claude Code's hint headers are Pygg's to read, never the provider's (gateway/hints.rs).
    if headers.keys().any(|k| k.as_str().starts_with("x-claude-code-")) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "type": "error", "error": { "type": "invalid_request_error",
            "message": "a Claude Code hint header reached the provider" } })),
        )
            .into_response();
    }
    let seen = format!(
        "cache_control={} thinking={} beta={}",
        body.to_string().contains("cache_control"),
        !body["thinking"].is_null(),
        headers.get("anthropic-beta").and_then(|v| v.to_str().ok()).unwrap_or("-")
    );
    let usage = json!({ "input_tokens": 10, "cache_read_input_tokens": 900, "cache_creation_input_tokens": 90, "output_tokens": 5 });
    let limits = [("anthropic-ratelimit-requests-remaining", "99"), ("request-id", "req_a1")];
    if model == "thinker" {
        // Thinks aloud about the thinking it was asked for, then a redacted block, then the answer (ROADMAP B5).
        // Not asked to think: an empty block, as Claude's default `display: "omitted"` sends.
        let thought = if body["thinking"].is_null() {
            String::new()
        } else {
            format!(
                "thinking={} effort={}",
                body["thinking"]["type"].as_str().unwrap_or("-"),
                body["output_config"]["effort"].as_str().unwrap_or("-")
            )
        };
        if body["stream"] == true {
            let events = [
                json!({ "type": "message_start", "message": { "id": "msg_t", "usage": usage } }),
                json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "thinking", "thinking": "", "signature": "" } }),
                json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "thinking_delta", "thinking": thought } }),
                json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "signature_delta", "signature": "EqQBsig" } }),
                json!({ "type": "content_block_stop", "index": 0 }),
                json!({ "type": "content_block_start", "index": 1, "content_block": { "type": "redacted_thinking", "data": "secret" } }),
                json!({ "type": "content_block_stop", "index": 1 }),
                json!({ "type": "content_block_start", "index": 2, "content_block": { "type": "text", "text": "" } }),
                json!({ "type": "content_block_delta", "index": 2, "delta": { "type": "text_delta", "text": seen } }),
                json!({ "type": "content_block_stop", "index": 2 }),
                json!({ "type": "message_delta", "delta": { "stop_reason": "end_turn" }, "usage": { "output_tokens": 7 } }),
                json!({ "type": "message_stop" }),
            ];
            let sse: String =
                events.iter().map(|e| format!("event: {}\ndata: {e}\n\n", e["type"].as_str().unwrap())).collect();
            return ([(header::CONTENT_TYPE, "text/event-stream")], limits, sse).into_response();
        }
        return (
            limits,
            Json(json!({ "id": "msg_t", "type": "message", "role": "assistant", "model": model,
            "content": [{ "type": "thinking", "thinking": thought, "signature": "EqQBsig" },
                { "type": "redacted_thinking", "data": "secret" }, { "type": "text", "text": seen }],
            "stop_reason": "end_turn", "usage": usage })),
        )
            .into_response();
    }
    if body["stream"] == true {
        let sse = format!(
            "event: message_start\ndata: {}\n\nevent: content_block_delta\ndata: {}\n\nevent: message_delta\ndata: {}\n\nevent: message_stop\ndata: {{\"type\":\"message_stop\"}}\n\n",
            json!({ "type": "message_start", "message": { "id": "msg_1", "usage": usage } }),
            json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": seen } }),
            json!({ "type": "message_delta", "delta": { "stop_reason": "end_turn" }, "usage": { "output_tokens": 7 } }));
        return ([(header::CONTENT_TYPE, "text/event-stream")], limits, sse).into_response();
    }
    (
        limits,
        Json(json!({ "id": "msg_1", "type": "message", "role": "assistant", "model": model,
        "content": [{ "type": "text", "text": seen }], "stop_reason": "end_turn", "usage": usage })),
    )
        .into_response()
}

/// The fake provider's own Responses API, for the native passthrough (ADR-033).
async fn fake_responses(
    State(fake): State<Arc<Fake>>, headers: HeaderMap, JsonBody(body): JsonBody<Value>,
) -> Response {
    let model = body["model"].as_str().unwrap_or("").to_string();
    *fake.calls.lock().unwrap().entry(format!("responses:{model}")).or_default() += 1;
    *fake.last_body.lock().unwrap() = Some(body.clone());
    if headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) != Some("Bearer key-1") {
        return (StatusCode::UNAUTHORIZED, Json(json!({ "error": { "message": "bad key" } }))).into_response();
    }
    if model == "broken" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": { "message": "Unsupported parameter: 'x'",
            "type": "invalid_request_error", "param": "x", "code": "unsupported_parameter" } })),
        )
            .into_response();
    }
    let usage = json!({ "input_tokens": 1000, "input_tokens_details": { "cached_tokens": 800 }, "output_tokens": 500,
        "output_tokens_details": { "reasoning_tokens": 64 }, "total_tokens": 1500 });
    let response = json!({ "id": "resp_up", "object": "response", "status": "completed", "model": model,
        "output": [{ "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": "native" }] }],
        "usage": usage });
    if body["stream"] == true {
        let sse = format!(
            "event: response.created\ndata: {}\n\nevent: response.output_text.delta\ndata: {}\n\nevent: response.completed\ndata: {}\n\n",
            json!({ "type": "response.created", "response": { "id": "resp_up", "status": "in_progress" } }),
            json!({ "type": "response.output_text.delta", "delta": "native" }),
            json!({ "type": "response.completed", "response": response }));
        return ([(header::CONTENT_TYPE, "text/event-stream")], sse).into_response();
    }
    Json(response).into_response()
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
    Price {
        input_micro_per_mtok: 100_000,
        output_micro_per_mtok: 400_000,
        cached_input_micro_per_mtok: None,
        cache_write_micro_per_mtok: None,
        source: "user".into(),
        observed_at: 0,
    }
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
        .route("/v1/responses", post(fake_responses))
        .route("/v1/models", axum::routing::get(|| async { Json(json!({ "data": [{ "id": "fast" }] })) }))
        .route("/anthropic/v1/messages", post(fake_anthropic))
        .with_state(fake.clone());
    tokio::spawn(async move { axum::serve(upstream, app).await.unwrap() });

    let conn = match store_path {
        Some(path) => crate::db::open(path).unwrap(),
        None => crate::db::open_in_memory().unwrap(),
    };
    let endpoint = format!("http://{upstream_addr}/v1");
    let add = |kind: &str, label: &str, endpoint: &str, billing: &str, wire: &str| {
        store::add_provider(
            &conn,
            &NewProvider {
                kind,
                label,
                endpoint,
                credential_ref: Some("provider-main"),
                default_billing: billing,
                wire,
            },
        )
        .unwrap()
    };
    let main = add("custom", "Main", &endpoint, "paid", "openai");
    // Same provider kind as Main, so the same quota group: a sibling key of the same organization.
    let sibling = add("custom", "Sibling", &endpoint, "paid", "openai");
    // Marked as its own organization.
    let backup = add("custom", "Backup", &endpoint, "free", "openai");
    store::update_provider(&conn, &backup.id, None, None, Some(true)).unwrap();
    let router = add("openrouter", "Router", &endpoint, "free", "openai");
    let claude =
        add("custom-anthropic", "Claude", &format!("http://{upstream_addr}/anthropic/v1"), "paid", "anthropic");
    for model in ["fast", "slow-ok"] {
        store::add_model(&conn, &main.id, model).unwrap();
        store::update_model(&conn, &main.id, model, None, None, Some(Some(price()))).unwrap();
    }
    store::add_model(&conn, &claude.id, "sonnet").unwrap();
    store::update_model(&conn, &claude.id, "sonnet", None, None, Some(Some(price()))).unwrap();
    let combo = |name: &str, members: &[(&str, &str)]| {
        let members: Vec<Target> =
            members.iter().map(|(p, m)| Target { provider_id: p.to_string(), model_id: m.to_string() }).collect();
        store::save_combo(&conn, None, name, &members).unwrap();
    };
    combo("coding", &[(&main.id, "limited"), (&main.id, "down"), (&backup.id, "steady")]);
    combo("siblings", &[(&main.id, "limited"), (&sibling.id, "steady")]);
    combo("slowcombo", &[(&main.id, "slow"), (&backup.id, "steady")]);
    combo("pinme", &[(&main.id, "flaky"), (&backup.id, "steady")]);
    combo("prefill", &[(&claude.id, "noprefill"), (&main.id, "fast")]);
    let _ = router;
    let (client, token) = store::create_client(&conn, "Tests", None).unwrap();

    let secrets = MapSecrets(HashMap::from([
        ("provider-main".to_string(), "key-1".to_string()),
        ("provider-k2".to_string(), "key-2".to_string()),
        ("provider-limited".to_string(), "key-limited".to_string()),
        ("provider-bad".to_string(), "key-bad".to_string()),
    ]));
    let shared = Shared::with_read_timeout(Arc::new(Mutex::new(conn)), Arc::new(secrets), read_timeout);
    let shared = match store_path {
        Some(path) => Shared::for_store(shared.db.clone(), shared.secrets.clone(), path),
        None => shared,
    };
    let server = start(shared.clone(), 0).await.unwrap();
    Rig {
        base: format!("http://{}", server.addr),
        token,
        client_id: client.id,
        shared,
        fake,
        http: reqwest::Client::new(),
        server: tokio::sync::Mutex::new(Some(server)),
    }
}

async fn rig() -> Rig {
    rig_with(Duration::from_secs(30)).await
}

impl Rig {
    async fn post(&self, path: &str, body: Value) -> reqwest::Response {
        self.http.post(format!("{}{path}", self.base)).bearer_auth(&self.token).json(&body).send().await.unwrap()
    }

    async fn post_session(&self, path: &str, session: &str, body: Value) -> reqwest::Response {
        self.http
            .post(format!("{}{path}", self.base))
            .bearer_auth(&self.token)
            .header(SESSION_HEADER, session)
            .json(&body)
            .send()
            .await
            .unwrap()
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
        let members: Vec<Target> =
            members.iter().map(|(p, m)| Target { provider_id: p.to_string(), model_id: m.to_string() }).collect();
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
        let anonymous = rig
            .http
            .post(format!("{}/v1/chat/completions", rig.base))
            .json(&json!({ "model": "x" }))
            .send()
            .await
            .unwrap();
        assert_eq!(anonymous.status(), 401);
        let rebound =
            rig.http.get(format!("{}/health", rig.base)).header(header::HOST, "evil.example:80").send().await.unwrap();
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
        assert_eq!(response.headers()["x-pygg-route"], "main/fast");
        assert_eq!(response.headers()["x-pygg-mode"], "native");
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
        let response =
            rig.post("/v1/chat/completions", json!({ "model": "main/fast", "stream": true, "messages": [] })).await;
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
        assert_eq!(response.headers()["x-pygg-route"], "backup/steady");
        let outcomes: Vec<(String, Option<u16>)> =
            rig.ledger().into_iter().map(|e| (e.outcome, e.http_status)).collect();
        assert_eq!(
            outcomes,
            [("fallback".into(), Some(429)), ("fallback".into(), Some(503)), ("ok".into(), Some(200))]
        );
        // One request, three attempts in order, each with the billing and route it was sent under (migration 0030), and
        // the request's id in the answer's headers.
        let id = response.headers()[REQUEST_ID_HEADER].to_str().unwrap().to_string();
        let ledger = rig.ledger();
        assert!(ledger.iter().all(|e| e.request_id.as_deref() == Some(id.as_str())), "{ledger:?}");
        assert_eq!(ledger.iter().map(|e| e.attempt).collect::<Vec<_>>(), [Some(1), Some(2), Some(3)]);
        assert!(ledger
            .iter()
            .all(|e| e.route_kind.as_deref() == Some("combo") && e.cache_hint.as_deref() == Some("none")));
        assert!(ledger.iter().all(|e| e.billing.is_some()));

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
        assert_eq!(response.headers()["x-pygg-route"], "sibling/steady");
    });
}

#[test]
fn a_rejected_key_blocks_the_connection_until_it_is_fixed() {
    rt().block_on(async {
        let rig = rig().await;
        assert_eq!(rig.post("/v1/chat/completions", chat("main/badkey")).await.status(), 401);
        let health = rig.shared.with_db(|db| store::list_health(db, Some("main"))).unwrap();
        assert_eq!(
            (health[0].state.as_str(), health[0].model_id.as_str(), health[0].until_ms),
            ("invalid_credentials", "", None)
        );

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
        assert_eq!(
            (event.model.as_str(), event.outcome.as_str(), event.cost_status.as_str()),
            ("slow", "error", "unknown")
        );
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
        let (a, b) =
            tokio::join!(rig.post("/v1/chat/completions", body.clone()), rig.post("/v1/chat/completions", body));
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
        let (a, b) = tokio::join!(rig.post("/v1/chat/completions", chat("main/slow-ok")), async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            rig.post("/v1/chat/completions", chat("main/slow-ok")).await
        });
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
        let result = impatient
            .post(format!("{}/v1/chat/completions", rig.base))
            .bearer_auth(&rig.token)
            .json(&chat("main/slow-ok"))
            .send()
            .await;
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
        assert_eq!(first.headers()["x-pygg-route"], "backup/steady");
        assert_eq!(first.headers()["x-pygg-session"], "new");

        // A request without a session goes back to the first model, which works again.
        let other = rig.post("/v1/chat/completions", chat("pinme")).await;
        assert_eq!(other.headers()["x-pygg-route"], "main/flaky");

        // The session stays where it landed: same model, same cache.
        let again = rig.post_session("/v1/chat/completions", "s-1", chat("pinme")).await;
        assert_eq!(again.headers()["x-pygg-route"], "backup/steady");
        assert_eq!(again.headers()["x-pygg-session"], "pinned");
        assert_eq!(again.headers()["x-pygg-attempts"], "1");
        assert_eq!(rig.ledger().last().unwrap().session_key.as_deref(), Some("s-1"));
    });
}

/// ROADMAP item 4: one passing refusal no longer keeps a session away from its home for the pin's two hours.
#[test]
fn a_session_goes_back_to_its_first_model_once_no_warm_cache_is_lost() {
    rt().block_on(async {
        let rig = rig().await;
        rig.allow_paid_retries(1);
        let first = rig.post_session("/v1/chat/completions", "s-home", chat("pinme")).await;
        assert_eq!(first.headers()["x-pygg-route"], "backup/steady");

        // The backup's cache is warm and the first model never answered this session: it stays.
        let warm = rig.post_session("/v1/chat/completions", "s-home", chat("pinme")).await;
        assert_eq!(warm.headers()["x-pygg-route"], "backup/steady");
        assert_eq!(warm.headers()["x-pygg-session"], "pinned");

        // Six quiet minutes: both caches are cold, so nothing is lost by going home.
        let six_minutes = 6 * 60 * 1000;
        rig.shared
            .with_db(|db| {
                db.execute("UPDATE session_affinity SET last_used_at = last_used_at - ?1", [six_minutes])
                    .map_err(|e| e.to_string())
            })
            .unwrap();
        let home = rig.post_session("/v1/chat/completions", "s-home", chat("pinme")).await;
        assert_eq!(home.headers()["x-pygg-route"], "main/flaky");
        assert_eq!(home.headers()["x-pygg-session"], "home");
        assert_eq!(home.headers()["x-pygg-attempts"], "1");

        // And it stays home.
        let stays = rig.post_session("/v1/chat/completions", "s-home", chat("pinme")).await;
        assert_eq!(stays.headers()["x-pygg-route"], "main/flaky");
        assert_eq!(stays.headers()["x-pygg-session"], "pinned");
    });
}

#[test]
fn a_pin_remembers_its_home_and_when_the_home_last_answered() {
    let conn = crate::db::open_in_memory().unwrap();
    let (client, _) = store::create_client(&conn, "Tests", None).unwrap();
    let mut ids = Vec::new();
    for label in ["Main", "Backup"] {
        let provider = store::add_provider(
            &conn,
            &NewProvider {
                kind: "custom",
                label,
                endpoint: "http://x.test",
                credential_ref: None,
                default_billing: "paid",
                wire: "openai",
            },
        )
        .unwrap();
        ids.push(provider.id);
    }
    let main = Target { provider_id: ids[0].clone(), model_id: "m".into() };
    let backup = Target { provider_id: ids[1].clone(), model_id: "m".into() };
    let pin = |target: &Target| store::pin(&conn, &client.id, "s", "combo", target, Some(&main)).unwrap();
    let read = || store::session_pin(&conn, &client.id, "s", "combo").unwrap().unwrap();

    pin(&main);
    let at_home = read();
    assert_eq!((at_home.home.as_ref(), at_home.home_used_at.is_some()), (Some(&main), true));
    assert!(!at_home.goes_home(at_home.last_used_at), "already home");

    pin(&backup);
    let away = read();
    assert_eq!(away.target, backup);
    assert_eq!(away.home_used_at, at_home.home_used_at, "the home's last answer is kept while away");
    let now = away.last_used_at;
    assert!(away.goes_home(now), "the home answered moments ago: its cache is warm");

    let both_old = store::Pin { home_used_at: Some(now - 20 * 60 * 1000), ..away.clone() };
    assert!(!both_old.goes_home(now), "the backup's cache is warm, the home's is cold: stay");
    assert!(both_old.goes_home(now + store::CACHE_WARM_MS), "both cold: go home");
    let never_home = store::Pin { home_used_at: None, ..away.clone() };
    assert!(!never_home.goes_home(now + 1000));
    let no_home = store::Pin { home: None, ..away };
    assert!(!no_home.goes_home(now + store::CACHE_WARM_MS * 10), "a pin from before migration 0033 stays put");

    // A different home starts with no answer of its own.
    store::pin(&conn, &client.id, "s", "combo", &main, Some(&backup)).unwrap();
    assert_eq!(read().home_used_at, None);
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
        let response = rig
            .http
            .post(format!("{}/v1/messages", rig.base))
            .header("x-api-key", &rig.token)
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "prompt-caching-test")
            .json(&claude_request(false))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["x-pygg-mode"], "native");
        assert!(response.headers().get("x-pygg-dropped").is_none());
        let body: Value = response.json().await.unwrap();
        // Cache markers, thinking and the beta header all reached the provider.
        assert_eq!(body["content"][0]["text"], "cache_control=true thinking=true beta=prompt-caching-test");
        assert_eq!(body["usage"]["cache_read_input_tokens"], 900, "the provider's answer came back as it was");
        let event = rig.ledger().pop().unwrap();
        assert_eq!(
            (event.cache_tokens, event.cache_write_tokens, event.input_tokens),
            (Some(900), Some(90), Some(1000))
        );
        assert_eq!(event.route_mode.as_deref(), Some("native"));

        let streamed = rig.post("/v1/messages", claude_request(true)).await.text().await.unwrap();
        assert!(streamed.contains("cache_control=true thinking=true") && streamed.contains("message_stop"));
        tokio::time::sleep(Duration::from_millis(100)).await;
        let event = rig.ledger().pop().unwrap();
        assert_eq!((event.cache_tokens, event.output_tokens, event.outcome.as_str()), (Some(900), Some(7), "ok"));
    });
}

/// Cost plan A2: Claude Code's hint headers are kept with the attempt and not forwarded.
#[test]
fn claude_codes_hint_headers_are_recorded_and_not_forwarded() {
    rt().block_on(async {
        let rig = rig().await;
        let response = rig
            .http
            .post(format!("{}/v1/messages", rig.base))
            .header("x-api-key", &rig.token)
            .header("anthropic-version", "2023-06-01")
            .header("x-claude-code-request-class", "subagent")
            .header("x-claude-code-agent-type", "Explore")
            .header("x-claude-code-agent-id", "agent-7")
            .header("x-claude-code-prompt-id", "prompt-1")
            .header("x-claude-code-session-id", "cc-session")
            .json(&claude_request(false))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200, "no hint header reached the provider");
        let row: (Option<String>, Option<String>, Option<String>, Option<String>) = rig
            .shared
            .with_db(|db| {
                db.query_row(
                    "SELECT request_class, agent_type, agent_id, prompt_id FROM usage_events ORDER BY created_at DESC LIMIT 1",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .map_err(|e| e.to_string())
            })
            .unwrap();
        assert_eq!(
            row,
            (Some("subagent".into()), Some("Explore".into()), Some("agent-7".into()), Some("prompt-1".into()))
        );
    });
}

/// ROADMAP B5 (N8): before it, an OpenAI-format request could not reach an Anthropic connection at all.
#[test]
fn an_openai_request_reaches_an_anthropic_connection_through_translation() {
    rt().block_on(async {
        let rig = rig().await;
        let mut request = chat("claude/sonnet");
        request["seed"] = json!(3);
        let response = rig.post("/v1/chat/completions", request).await;
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["x-pygg-mode"], "translated:openai->anthropic");
        assert_eq!(response.headers()["x-pygg-dropped"], "seed");
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["object"], "chat.completion");
        assert_eq!(body["choices"][0]["message"]["content"], "cache_control=false thinking=false beta=-");
        assert_eq!(body["choices"][0]["finish_reason"], "stop");
        assert_eq!(
            (body["usage"]["prompt_tokens"].as_u64(), body["usage"]["prompt_tokens_details"]["cached_tokens"].as_u64()),
            (Some(1000), Some(900))
        );
        assert_eq!(rig.fake.count("sonnet"), 1);
        let event = rig.ledger().pop().unwrap();
        assert_eq!(event.route_mode.as_deref(), Some("translated:openai->anthropic"));
        assert_eq!((event.input_tokens, event.cache_tokens, event.output_tokens), (Some(1000), Some(900), Some(5)));

        // Streamed, with the usage chunk the client asked for.
        let mut streamed = chat("claude/sonnet");
        streamed["stream"] = json!(true);
        streamed["stream_options"] = json!({ "include_usage": true });
        let text = rig.post("/v1/chat/completions", streamed).await.text().await.unwrap();
        assert!(text.contains("\"content\":\"cache_control=false"), "{text}");
        assert!(text.contains("\"completion_tokens\":7") && text.trim_end().ends_with("data: [DONE]"), "{text}");
        tokio::time::sleep(Duration::from_millis(100)).await;
        let event = rig.ledger().pop().unwrap();
        assert_eq!((event.output_tokens, event.outcome.as_str()), (Some(7), "ok"));
    });
}

/// ADR-036: an agent's OpenAI-format turn, translated for Anthropic's own API, gets Anthropic's automatic prompt caching;
/// a question without tools, and another provider's Anthropic-format endpoint, are left as the client sent them.
#[test]
fn an_agents_openai_turn_to_anthropics_own_api_turns_on_prompt_caching() {
    rt().block_on(async {
        let rig = rig().await;
        let with_tools = || {
            let mut request = chat("claude/sonnet");
            request["tools"] = json!([{ "type": "function", "function": { "name": "read", "parameters": { "type": "object", "properties": {} } } }]);
            request
        };
        let sent = || rig.fake.last_body.lock().unwrap().clone().unwrap();
        // A custom Anthropic-format endpoint: not Anthropic's own API, so nothing is added.
        assert_eq!(rig.post("/v1/chat/completions", with_tools()).await.status(), 200);
        assert!(sent()["cache_control"].is_null());
        assert_eq!(rig.ledger().pop().unwrap().cache_hint.as_deref(), Some("none"));

        rig.shared.with_db(|db| db.execute("UPDATE providers SET kind = 'anthropic' WHERE id = 'claude'", [])).unwrap();
        assert_eq!(rig.post("/v1/chat/completions", with_tools()).await.status(), 200);
        assert_eq!(sent()["cache_control"], json!({ "type": "ephemeral" }));
        assert_eq!(rig.ledger().pop().unwrap().cache_hint.as_deref(), Some("added"));

        // A one-off question without tools pays no cache write.
        assert_eq!(rig.post("/v1/chat/completions", chat("claude/sonnet")).await.status(), 200);
        assert!(sent()["cache_control"].is_null());
    });
}

/// ADR-036: OpenRouter takes the same top-level marker for Anthropic's models, so an agent's OpenAI-format turn to an
/// `anthropic/…` model there gets it; another model there, a turn without tools and a client's own marker are left alone,
/// and an Anthropic-format client's top-level marker crosses the translation.
#[test]
fn an_agents_turn_to_claude_through_openrouter_turns_on_prompt_caching() {
    rt().block_on(async {
        let rig = rig().await;
        let with_tools = |model: &str| {
            let mut request = chat(model);
            request["tools"] = json!([{ "type": "function", "function": { "name": "read", "parameters": { "type": "object", "properties": {} } } }]);
            request
        };
        let sent = || rig.fake.last_body.lock().unwrap().clone().unwrap();

        assert_eq!(rig.post("/v1/chat/completions", with_tools("router/anthropic/claude-haiku-5.5")).await.status(), 200);
        assert_eq!(sent()["cache_control"], json!({ "type": "ephemeral" }));
        assert_eq!(rig.ledger().pop().unwrap().cache_hint.as_deref(), Some("added"));

        // Codex's Responses turn, translated to Chat Completions for OpenRouter.
        let codex = responses_request("router/~anthropic/claude-sonnet-latest", false);
        assert_eq!(rig.post("/v1/responses", codex).await.status(), 200);
        assert_eq!(sent()["cache_control"], json!({ "type": "ephemeral" }));

        // Another vendor's model on OpenRouter, a question without tools: nothing added.
        assert_eq!(rig.post("/v1/chat/completions", with_tools("router/qwen/qwen3-coder")).await.status(), 200);
        assert!(sent()["cache_control"].is_null());
        assert_eq!(rig.ledger().pop().unwrap().cache_hint.as_deref(), Some("none"));
        assert_eq!(rig.post("/v1/chat/completions", chat("router/anthropic/claude-haiku-5.5")).await.status(), 200);
        assert!(sent()["cache_control"].is_null());

        // The client's own marker on a part is kept as it is, and nothing is added beside it.
        let mut marked = with_tools("router/anthropic/claude-haiku-5.5");
        marked["messages"][0]["content"] =
            json!([{ "type": "text", "text": "ping", "cache_control": { "type": "ephemeral" } }]);
        assert_eq!(rig.post("/v1/chat/completions", marked).await.status(), 200);
        assert!(sent()["cache_control"].is_null());
        assert_eq!(sent()["messages"][0]["content"][0]["cache_control"], json!({ "type": "ephemeral" }));

        // An Anthropic-format client's top-level marker crosses to OpenRouter; to a provider that takes none, it is named.
        let top = |model: &str| json!({ "model": model, "max_tokens": 64, "cache_control": { "type": "ephemeral" },
            "messages": [{ "role": "user", "content": "ping" }] });
        let kept = rig.post("/v1/messages", top("router/anthropic/claude-haiku-5.5")).await;
        assert!(kept.headers().get("x-pygg-dropped").is_none());
        assert_eq!(sent()["cache_control"], json!({ "type": "ephemeral" }));
        assert_eq!(rig.ledger().pop().unwrap().cache_hint.as_deref(), Some("translated"));
        let dropped = rig.post("/v1/messages", top("main/fast")).await;
        assert_eq!(dropped.headers()["x-pygg-dropped"], "cache_control");
        assert!(sent()["cache_control"].is_null());
    });
}

/// ADR-036: a request's session is passed on to providers that route by one, as an opaque hash: OpenRouter's
/// `x-session-id`, OpenAI's `prompt_cache_key`. A key the client named stays; other providers get nothing; no session,
/// nothing.
#[test]
fn the_session_is_passed_on_as_a_hash_to_providers_that_route_by_one() {
    rt().block_on(async {
        let rig = rig().await;
        let sent = || rig.fake.last_body.lock().unwrap().clone().unwrap();
        let header = || {
            let headers = rig.fake.last_headers.lock().unwrap().clone().unwrap();
            headers.get("x-session-id").map(|v| v.to_str().unwrap().to_string())
        };

        assert_eq!(rig.post_session("/v1/chat/completions", "launch-1", chat("router/m")).await.status(), 200);
        let tag = header().expect("OpenRouter gets the session");
        assert!(tag.starts_with("pygg-") && tag.len() == 37 && !tag.contains("launch-1"), "{tag}");
        assert!(sent()["session_id"].is_null() && sent()["prompt_cache_key"].is_null());
        // The same session gives the same tag; another session another.
        rig.post_session("/v1/chat/completions", "launch-1", chat("router/m")).await;
        assert_eq!(header().as_deref(), Some(tag.as_str()));
        rig.post_session("/v1/chat/completions", "launch-2", chat("router/m")).await;
        assert_ne!(header().as_deref(), Some(tag.as_str()));
        // A key the client named wins on OpenRouter, so Pygg adds none; no session, nothing.
        let mut named = chat("router/m");
        named["session_id"] = json!("theirs");
        rig.post_session("/v1/chat/completions", "launch-1", named).await;
        assert_eq!(header(), None);
        assert_eq!(sent()["session_id"], "theirs");
        rig.post("/v1/chat/completions", chat("router/m")).await;
        assert_eq!(header(), None);

        // Another provider gets nothing.
        rig.post_session("/v1/chat/completions", "launch-1", chat("main/fast")).await;
        assert_eq!(header(), None);
        assert!(sent()["prompt_cache_key"].is_null());

        // OpenAI: the tag as `prompt_cache_key`, unless the client sent its own (Codex does).
        rig.shared.with_db(|db| db.execute("UPDATE providers SET kind = 'openai' WHERE id = 'main'", [])).unwrap();
        rig.post_session("/v1/chat/completions", "launch-1", chat("main/fast")).await;
        assert_eq!(sent()["prompt_cache_key"], json!(tag));
        assert_eq!(header(), None);
        let mut theirs = chat("main/fast");
        theirs["prompt_cache_key"] = json!("conversation-7");
        rig.post_session("/v1/chat/completions", "launch-1", theirs).await;
        assert_eq!(sent()["prompt_cache_key"], "conversation-7");

        // The session's tools and system prompt are measured into the ledger, as numbers.
        let mut with_system = chat("main/fast");
        with_system["messages"] =
            json!([{ "role": "system", "content": "You are terse." }, { "role": "user", "content": "ping" }]);
        rig.post_session("/v1/chat/completions", "launch-9", with_system).await;
        let meter = rig.shared.with_db(|db| store::prefix_meter(db, 0)).unwrap();
        assert!(meter.top.iter().any(|s| s.session == "launch-9" && (s.tools_tokens, s.system_tokens) == (0, 5)));
    });
}

/// A Claude model that takes adaptive thinking and three effort levels, as Anthropic's model list reports one.
fn add_thinker(rig: &Rig, efforts: &[&str]) {
    rig.priced("claude", "thinker");
    rig.shared
        .with_db(|db| {
            store::set_capabilities(
                db,
                "claude",
                "thinker",
                &store::Capabilities {
                    thinking_types: Some(vec!["adaptive".into()]),
                    effort_levels: Some(efforts.iter().map(|e| e.to_string()).collect()),
                    ..store::Capabilities::default()
                },
            )
        })
        .unwrap();
}

/// ROADMAP B5, the other direction: an OpenAI client's `reasoning_effort` reaches Claude as thinking, and Claude's
/// thinking comes back as `reasoning_content`, streamed or not; `redacted_thinking` does not.
#[test]
fn reasoning_crosses_an_openai_to_anthropic_translation_both_ways() {
    rt().block_on(async {
        let rig = rig().await;
        add_thinker(&rig, &["low", "medium", "high"]);
        let request = |stream: bool| json!({ "model": "claude/thinker", "reasoning_effort": "xhigh", "temperature": 0.3, "stream": stream,
            "messages": [{ "role": "user", "content": "ping" },
                { "role": "assistant", "content": "pong", "reasoning_content": "earlier thought" },
                { "role": "user", "content": "again" }] });
        let response = rig.post("/v1/chat/completions", request(false)).await;
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["x-pygg-dropped"], "reasoning_content,temperature (with thinking)");
        let sent = rig.fake.last_body.lock().unwrap().clone().unwrap();
        assert_eq!(sent["thinking"], json!({ "type": "adaptive", "display": "summarized" }));
        assert_eq!(sent["output_config"], json!({ "effort": "high" }), "xhigh is not listed: the nearest lower level");
        assert!(sent.get("temperature").is_none());
        assert_eq!(sent["messages"][1], json!({ "role": "assistant", "content": [{ "type": "text", "text": "pong" }] }),
            "earlier reasoning has no signature, so it stays behind");
        let body: Value = response.json().await.unwrap();
        let message = &body["choices"][0]["message"];
        assert_eq!(message["reasoning_content"], "thinking=adaptive effort=high");
        assert_eq!(message["content"], "cache_control=false thinking=true beta=-");
        assert!(!body.to_string().contains("secret"), "redacted thinking is dropped: {body}");

        let text = rig.post("/v1/chat/completions", request(true)).await.text().await.unwrap();
        let deltas: Vec<Value> = text.lines().filter_map(|l| l.strip_prefix("data: ")).filter(|d| *d != "[DONE]")
            .map(|d| serde_json::from_str::<Value>(d).unwrap()["choices"][0]["delta"].clone()).collect();
        let reasoning = deltas.iter().position(|d| d["reasoning_content"] == "thinking=adaptive effort=high");
        let answer = deltas.iter().position(|d| d["content"] == "cache_control=false thinking=true beta=-");
        assert!(matches!((reasoning, answer), (Some(r), Some(a)) if r < a), "reasoning before the answer: {text}");
        assert!(!text.contains("secret") && !text.contains("EqQBsig"), "{text}");
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(rig.ledger().pop().unwrap().outcome, "ok");

        // DeepSeek's switch on its own: thinking at the model's default level.
        let deepseek = json!({ "model": "claude/thinker", "thinking": { "type": "enabled" }, "messages": [{ "role": "user", "content": "ping" }] });
        let response = rig.post("/v1/chat/completions", deepseek).await;
        assert!(response.headers().get("x-pygg-dropped").is_none());
        let sent = rig.fake.last_body.lock().unwrap().clone().unwrap();
        assert_eq!((sent["thinking"]["type"].as_str(), sent.get("output_config")), (Some("adaptive"), None));
    });
}

#[test]
fn reasoning_effort_is_dropped_and_named_where_claude_cannot_take_it() {
    rt().block_on(async {
        let rig = rig().await;
        add_thinker(&rig, &["low", "medium", "high"]);
        // Nothing is known of `sonnet`'s thinking.
        let mut unknown = chat("claude/sonnet");
        unknown["reasoning_effort"] = json!("low");
        let response = rig.post("/v1/chat/completions", unknown).await;
        assert_eq!(response.headers()["x-pygg-dropped"], "reasoning_effort (unknown whether the model thinks)");
        assert!(rig.fake.last_body.lock().unwrap().clone().unwrap()["thinking"].is_null());
        // Anthropic refuses thinking with a forced tool choice.
        let forced = json!({ "model": "claude/thinker", "reasoning_effort": "high", "tool_choice": "required",
            "tools": [{ "type": "function", "function": { "name": "read", "parameters": { "type": "object" } } }],
            "messages": [{ "role": "user", "content": "ping" }] });
        let response = rig.post("/v1/chat/completions", forced).await;
        assert_eq!(response.headers()["x-pygg-dropped"], "reasoning_effort (with a forced tool choice)");
        assert!(rig.fake.last_body.lock().unwrap().clone().unwrap()["thinking"].is_null());
        // `none` sends nothing: Claude's own default applies.
        let mut none = chat("claude/thinker");
        none["reasoning_effort"] = json!("none");
        let response = rig.post("/v1/chat/completions", none).await;
        assert_eq!(response.headers()["x-pygg-dropped"], "reasoning_effort (none)");
        let body: Value = response.json().await.unwrap();
        assert!(
            body["choices"][0]["message"].get("reasoning_content").is_none(),
            "an empty thought is not sent: {body}"
        );
    });
}

/// The Responses API reaches Claude through the same translation: `reasoning.effort` is carried, and Claude's
/// thinking comes back as a reasoning summary item, streamed or not.
#[test]
fn a_responses_client_gets_claudes_thinking_as_a_reasoning_summary() {
    rt().block_on(async {
        let rig = rig().await;
        add_thinker(&rig, &["low", "medium", "high"]);
        let request = |stream: bool, effort: &str| {
            let mut r = responses_request("claude/thinker", stream);
            r["reasoning"] = json!({ "effort": effort, "summary": "auto" });
            r
        };
        let response = rig.post("/v1/responses", request(false, "low")).await;
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["x-pygg-dropped"], "web_search tool,prompt_cache_key");
        let sent = rig.fake.last_body.lock().unwrap().clone().unwrap();
        assert_eq!(sent["output_config"]["effort"], "low");
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["output"][0]["type"], "reasoning");
        assert_eq!(
            body["output"][0]["summary"][0],
            json!({ "type": "summary_text", "text": "thinking=adaptive effort=low" })
        );
        assert_eq!(body["output"][1]["content"][0]["text"], "cache_control=false thinking=true beta=-");

        let text = rig.post("/v1/responses", request(true, "low")).await.text().await.unwrap();
        let events = responses_events(&text);
        assert!(
            events.iter().any(|e| e["type"] == "response.reasoning_summary_text.delta"
                && e["delta"] == "thinking=adaptive effort=low"),
            "{text}"
        );
        let done = &events.last().unwrap()["response"];
        assert_eq!(done["output"][0]["summary"][0]["text"], "thinking=adaptive effort=low");
        assert_eq!(done["output"][1]["content"][0]["text"], "cache_control=false thinking=true beta=-");

        // A level the model cannot take is named by the Responses API's own name; thinking is still asked for.
        add_thinker(&rig, &["high"]);
        let response = rig.post("/v1/responses", request(false, "minimal")).await;
        assert_eq!(response.headers()["x-pygg-dropped"], "web_search tool,prompt_cache_key,reasoning.effort (level)");
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["output"][0]["summary"][0]["text"], "thinking=adaptive effort=-");
    });
}

#[test]
fn a_translation_keeps_cache_markers_only_where_the_provider_accepts_them() {
    rt().block_on(async {
        let rig = rig().await;
        let request = |model: &str| {
            json!({ "model": model, "max_tokens": 64,
            "system": [{ "type": "text", "text": "Long", "cache_control": { "type": "ephemeral" } }],
            "messages": [{ "role": "user", "content": "ping" }] })
        };

        let kept = rig.post("/v1/messages", request("router/m")).await;
        assert_eq!(kept.headers()["x-pygg-mode"], "translated:anthropic->openai");
        assert!(kept.headers().get("x-pygg-dropped").is_none(), "nothing was lost");
        let body: Value = kept.json().await.unwrap();
        assert_eq!(body["content"][0]["text"], "pong cache:yes");

        let dropped = rig.post("/v1/messages", request("main/fast")).await;
        assert_eq!(dropped.headers()["x-pygg-dropped"], "cache_control");
        let body: Value = dropped.json().await.unwrap();
        assert_eq!(body["content"][0]["text"], "pong");
    });
}

#[test]
fn anthropic_messages_round_trip_through_translation() {
    rt().block_on(async {
        let rig = rig().await;
        let response = rig
            .http
            .post(format!("{}/v1/messages", rig.base))
            .header("x-api-key", &rig.token)
            .json(
                &json!({ "model": "main/fast", "max_tokens": 64, "messages": [{ "role": "user", "content": "ping" }] }),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["type"], "message");
        assert_eq!(body["content"][0]["text"], "pong");
        assert_eq!(body["usage"]["output_tokens"], 500);

        let streamed = rig
            .post(
                "/v1/messages",
                json!({ "model": "main/tooler", "stream": true, "max_tokens": 64,
            "messages": [{ "role": "user", "content": "read a" }] }),
            )
            .await
            .text()
            .await
            .unwrap();
        assert!(streamed.starts_with("event: message_start"));
        assert!(streamed.contains("\"type\":\"tool_use\"") && streamed.contains("\"partial_json\":\"\\\"a\\\"}\""));
        assert!(
            streamed.contains("\"stop_reason\":\"tool_use\"")
                && streamed.trim_end().ends_with("{\"type\":\"message_stop\"}")
        );

        // An empty stream is the provider failing, not an empty answer: an error event, and the ledger says so.
        let empty = rig
            .post(
                "/v1/messages",
                json!({ "model": "main/empty", "stream": true, "max_tokens": 64,
            "messages": [{ "role": "user", "content": "hi" }] }),
            )
            .await
            .text()
            .await
            .unwrap();
        assert!(empty.contains("event: error") && empty.contains(crate::gateway::anthropic::EMPTY_ANSWER), "{empty}");
        assert!(!empty.contains("message_stop"));
        tokio::time::sleep(Duration::from_millis(200)).await;
        let last = rig.shared.with_db(|db| store::usage_summary(db, 0, 1)).unwrap().recent.remove(0);
        assert_eq!((last.outcome.as_str(), last.cost_status.as_str()), ("stream_error", "unknown"));

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

#[test]
fn no_credit_on_openrouter_does_not_hold_its_free_models() {
    rt().block_on(async {
        let rig = rig().await;
        let zero = Price { input_micro_per_mtok: 0, output_micro_per_mtok: 0, ..price() };
        let listed = |provider: &str, model: &str, price: &Price| {
            rig.shared.with_db(|db| {
                store::add_model(db, provider, model).unwrap();
                store::update_model(db, provider, model, None, None, Some(Some(price.clone()))).unwrap();
            })
        };
        // A second OpenRouter key: same quota group as Router.
        let endpoint = rig.shared.with_db(|db| store::get_provider(db, "router")).unwrap().unwrap().endpoint;
        let other = rig
            .shared
            .with_db(|db| {
                store::add_provider(
                    db,
                    &NewProvider {
                        kind: "openrouter",
                        label: "Router 2",
                        endpoint: &endpoint,
                        credential_ref: Some("provider-main"),
                        default_billing: "free",
                        wire: "openai",
                    },
                )
            })
            .unwrap();
        for provider in ["router", other.id.as_str(), "main"] {
            listed(provider, "free-a:free", &zero);
        }
        listed("router", "nocredit:free", &zero);
        listed("router", "paid-b", &price());
        rig.shared.with_db(|db| store::update_model(db, "router", "paid-b", Some(true), None, None)).unwrap();

        // A paid model: the account has no credit, so the whole connection is marked.
        assert_ne!(rig.post("/v1/chat/completions", chat("router/nocredit")).await.status(), 200);
        let health = rig.shared.with_db(|db| store::list_health(db, Some("router"))).unwrap();
        assert_eq!((health[0].model_id.as_str(), health[0].state.as_str()), ("", "payment_required"));
        // Its paid models wait, here and on the sibling key; its $0 models do not, on either key.
        assert_ne!(rig.post("/v1/chat/completions", chat("router/paid-b")).await.status(), 200);
        assert_eq!(rig.fake.count("paid-b"), 0);
        assert_eq!(rig.post("/v1/chat/completions", chat("router/free-a:free")).await.status(), 200);
        assert_eq!(rig.post("/v1/chat/completions", chat(&format!("{}/free-a:free", other.id))).await.status(), 200);
        assert_eq!(rig.fake.count("free-a:free"), 2);
        // A $0 model answering says nothing about the balance: the paid models still wait.
        assert_ne!(rig.post("/v1/chat/completions", chat("router/paid-b")).await.status(), 200);
        assert_eq!(rig.fake.count("paid-b"), 0);
        // `auto` judges them the same way.
        let client = rig.client();
        let plan = rig
            .shared
            .with_db(|db| auto::plan(db, &client, "auto/cheap", auto::Strategy::Cheap, &auto::Needs::default()))
            .unwrap();
        let reason = |provider: &str, model: &str| {
            plan.excluded
                .iter()
                .find(|e| e.provider_id == provider && e.model_id == model)
                .map(|e| e.reason.clone())
                .unwrap_or_default()
        };
        assert!(reason("router", "paid-b").starts_with("payment required"), "{plan:?}");
        assert!(!reason("router", "free-a:free").contains("payment"), "{plan:?}");
        assert!(!reason(&other.id, "free-a:free").contains("payment"), "{plan:?}");

        // A 402 on a $0 model is kept on that model, which then waits; the other $0 model is still used.
        assert_ne!(rig.post("/v1/chat/completions", chat("router/nocredit:free")).await.status(), 200);
        assert_ne!(rig.post("/v1/chat/completions", chat("router/nocredit:free")).await.status(), 200);
        assert_eq!(rig.fake.count("nocredit:free"), 1);
        assert_eq!(rig.post("/v1/chat/completions", chat("router/free-a:free")).await.status(), 200);

        // Only on a connection set as free whose free part is its $0 models: a $0 price elsewhere is not an exception.
        assert_ne!(rig.post("/v1/chat/completions", chat("main/nocredit")).await.status(), 200);
        assert_ne!(rig.post("/v1/chat/completions", chat("main/free-a:free")).await.status(), 200);
        assert_eq!(rig.fake.count("free-a:free"), 3);
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
        rig.post("/v1/chat/completions", json!({ "model": "main/fast", "stream": true, "messages": [] }))
            .await
            .text()
            .await
            .unwrap();
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
    let start = store::AttemptStart {
        client_id: &client.id,
        provider_id: "main",
        model_id: "fast",
        requested_model: "coding",
        session_key: Some("s-9"),
        route_mode: "native",
        hold_micro: 4_000,
        owner_generation: "g1",
    };
    let id = store::open_attempt(&conn, &start).unwrap().unwrap();
    // The generation that journaled it is still live: its entries are left alone.
    assert_eq!(store::settle_interrupted(&conn, store::Unsettled::NotOf("g1")).unwrap(), 0);
    // A gateway of a new generation took the store: g1 has finished or died.
    assert_eq!(store::settle_interrupted(&conn, store::Unsettled::NotOf("g2")).unwrap(), 1);
    assert_eq!(store::settle_interrupted(&conn, store::Unsettled::NotOf("g3")).unwrap(), 0, "turned exactly once");
    let event = store::usage_summary(&conn, 0, 10).unwrap().recent.remove(0);
    assert_eq!(
        (event.outcome.as_str(), event.cost_status.as_str(), event.at_risk_micro_usd),
        ("interrupted", "unknown", Some(4_000))
    );
    assert_eq!((event.provider.as_str(), event.session_key.as_deref()), ("main", Some("s-9")));
    // Settling it afterwards writes nothing twice.
    let cost = pricing::Cost::Estimated { micro_usd: 1, source: "x".into() };
    store::settle(&conn, &id, &UsageRecord::failed(&client.id, "main", "fast", "coding", &cost, "ok", None, 0, ""))
        .unwrap();
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
        assert_eq!(first.headers()["x-pygg-route"], "backup/steady");
        // Another client naming the same session is not sent to the first client's pin.
        let (_, other) = rig.shared.with_db(|db| store::create_client(db, "Other", None)).unwrap();
        let theirs = rig
            .http
            .post(format!("{}/v1/chat/completions", rig.base))
            .bearer_auth(&other)
            .header(SESSION_HEADER, "shared-id")
            .json(&chat("pinme"))
            .send()
            .await
            .unwrap();
        assert_eq!(theirs.headers()["x-pygg-route"], "main/flaky");
        assert_eq!(theirs.headers()["x-pygg-session"], "new");
        // An id that is too long is ignored, not truncated into someone else's.
        let long = rig.post_session("/v1/chat/completions", &"x".repeat(200), chat("main/fast")).await;
        assert!(long.headers().get("x-pygg-session").is_none());
    });
    let mut headers = HeaderMap::new();
    headers.insert(SESSION_HEADER, HeaderValue::from_bytes("phiên-1".as_bytes()).unwrap());
    assert_eq!(session_key(&headers), None, "non-ASCII ids are ignored");
    headers.insert(SESSION_HEADER, HeaderValue::from_static("a b"));
    assert_eq!(session_key(&headers), None);
    let mut legacy = HeaderMap::new();
    legacy.insert(LEGACY_SESSION_HEADER, HeaderValue::from_static("before-the-rename"));
    assert_eq!(session_key(&legacy).as_deref(), Some("before-the-rename"), "the old header name is still read");
}

#[test]
fn expired_session_pins_are_deleted() {
    let conn = crate::db::open_in_memory().unwrap();
    let (client, _) = store::create_client(&conn, "Tests", None).unwrap();
    let provider = store::add_provider(
        &conn,
        &NewProvider {
            kind: "custom",
            label: "Main",
            endpoint: "http://x.test",
            credential_ref: None,
            default_billing: "paid",
            wire: "openai",
        },
    )
    .unwrap();
    let target = Target { provider_id: provider.id.clone(), model_id: "m".into() };
    store::pin(&conn, &client.id, "old", "m", &target, None).unwrap();
    conn.execute("UPDATE session_affinity SET last_used_at = 0", []).unwrap();
    store::pin(&conn, &client.id, "new", "m", &target, None).unwrap();
    let left: i64 = conn.query_row("SELECT COUNT(*) FROM session_affinity", [], |r| r.get(0)).unwrap();
    assert_eq!(left, 1);
}

// ---------- CR-08: what a native passthrough keeps ----------

#[test]
fn a_native_request_keeps_its_key_order_and_the_providers_limit_headers() {
    rt().block_on(async {
        let rig = rig().await;
        let response = rig
            .post(
                "/v1/chat/completions",
                json!({ "model": "main/keyorder", "messages": [],
            "tools": [{ "type": "function", "function": { "name": "f", "parameters": { "type": "object",
                "properties": { "zeta": {}, "alpha": {}, "mid": {} } } } }] }),
            )
            .await;
        assert_eq!(response.headers()["x-ratelimit-remaining-requests"], "41");
        assert_eq!(response.headers()["x-request-id"], "req_7");
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["choices"][0]["message"]["content"], "zeta,alpha,mid");

        let claude = rig.post("/v1/messages", claude_request(false)).await;
        assert_eq!(claude.headers()["anthropic-ratelimit-requests-remaining"], "99");
        assert_eq!(claude.headers()["request-id"], "req_a1");

        // PV-06: the limit headers are also kept, per model, as the provider sent them; request ids are not.
        let limits = rig.shared.with_db(|db| store::list_limits(db, "main")).unwrap();
        let keyorder = limits.iter().find(|l| l.model_id == "keyorder").expect("kept for the model that answered");
        assert_eq!(keyorder.headers.get("x-ratelimit-remaining-requests").map(String::as_str), Some("41"));
        assert!(!keyorder.headers.contains_key("x-request-id"));
        assert_eq!(keyorder.http_status, Some(200));
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
        rig.shared
            .with_db(|db| store::set_client_connections(db, &rig.client_id, Some(&["main".to_string()])))
            .unwrap();
        let (b, b_token) = rig.shared.with_db(|db| store::create_client(db, "Vetted", None)).unwrap();
        rig.shared
            .with_db(|db| store::set_client_connections(db, &b.id, Some(&["main".to_string(), "backup".to_string()])))
            .unwrap();

        let a = rig
            .post(
                "/v1/chat/completions",
                json!({ "model": "private",
            "messages": [{ "role": "user", "content": "secret-prompt-text" }] }),
            )
            .await;
        assert_eq!(a.status(), 429, "Main's refusal is returned; Backup is never tried");
        assert!(!a.text().await.unwrap().contains("secret-prompt-text"), "no prompt in the diagnostics");
        assert_eq!(rig.fake.count("steady"), 0);

        // A session pin cannot route A there either.
        rig.shared
            .with_db(|db| {
                store::pin(
                    db,
                    &rig.client_id,
                    "s-a",
                    "private",
                    &Target { provider_id: "backup".into(), model_id: "steady".into() },
                    None,
                )
            })
            .unwrap();
        rig.post_session("/v1/chat/completions", "s-a", chat("private")).await;
        assert_eq!(rig.fake.count("steady"), 0);
        // Nor a direct request, and the refusal says why.
        let direct = rig.post("/v1/chat/completions", chat("backup/steady")).await;
        assert_eq!(direct.status(), 502);
        assert!(direct.text().await.unwrap().contains("may not use Backup"));
        assert_eq!(rig.fake.count("steady"), 0);

        // B falls back as the combo says (Main is cooling down, so B goes straight to Backup).
        let vetted = rig
            .http
            .post(format!("{}/v1/chat/completions", rig.base))
            .bearer_auth(&b_token)
            .json(&chat("private"))
            .send()
            .await
            .unwrap();
        assert_eq!(vetted.headers()["x-pygg-route"], "backup/steady");

        // The model list shows A only what it may use.
        let models: Value = rig
            .http
            .get(format!("{}/v1/models", rig.base))
            .bearer_auth(&rig.token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let ids: Vec<&str> = models["data"].as_array().unwrap().iter().filter_map(|m| m["id"].as_str()).collect();
        assert!(ids.iter().any(|id| id.starts_with("main/")) && !ids.iter().any(|id| id.starts_with("claude/")));

        // An empty list allows nothing.
        rig.shared.with_db(|db| store::set_client_connections(db, &rig.client_id, Some(&[]))).unwrap();
        assert_eq!(rig.post("/v1/chat/completions", chat("main/fast")).await.status(), 502);
        assert_eq!(rig.fake.count("fast"), 0);
    });
}

// ---------- Review GW-10: data-use terms ----------

#[test]
fn a_client_that_requires_no_training_reaches_only_connections_recorded_so() {
    rt().block_on(async {
        let rig = rig().await;
        rig.combo("careful", &[("main", "fast"), ("backup", "steady")]);
        assert!(
            rig.shared.with_db(|db| store::set_provider_terms(db, "backup", "no_training", None)).is_err(),
            "no_training needs a source"
        );
        rig.shared
            .with_db(|db| {
                store::set_provider_terms(db, "backup", "no_training", Some("https://example.test/terms, 2026-11-11"))
            })
            .unwrap();
        rig.shared.with_db(|db| store::set_provider_terms(db, "router", "may_train", Some("its policy page"))).unwrap();
        rig.shared.with_db(|db| store::set_client_require_no_training(db, &rig.client_id, true)).unwrap();

        // Main's terms are not recorded, so it counts as may-train: the combo goes straight to Backup.
        let answer = rig.post("/v1/chat/completions", chat("careful")).await;
        assert_eq!(answer.headers()["x-pygg-route"], "backup/steady");
        assert_eq!(rig.fake.count("fast"), 0);
        // A direct request to Main is refused, and says why.
        let direct = rig.post("/v1/chat/completions", chat("main/fast")).await;
        assert_eq!(direct.status(), 502);
        assert!(direct.text().await.unwrap().contains("do not train on its data, and Main's terms are not recorded"));
        // auto leaves them out with the same reason.
        let client =
            rig.shared.with_db(store::list_clients).unwrap().into_iter().find(|c| c.id == rig.client_id).unwrap();
        let plan = rig
            .shared
            .with_db(|db| {
                auto::plan(
                    db,
                    &client,
                    "auto/cheap",
                    auto::Strategy::Cheap,
                    &auto::Needs { input_tokens: 100, ..auto::Needs::default() },
                )
            })
            .unwrap();
        assert!(plan
            .excluded
            .iter()
            .filter(|e| e.provider_id == "router")
            .all(|e| e.reason.contains("terms are that it may")));
        // The model list shows only Backup.
        let models: Value = rig
            .http
            .get(format!("{}/v1/models", rig.base))
            .bearer_auth(&rig.token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let ids: Vec<&str> = models["data"].as_array().unwrap().iter().filter_map(|m| m["id"].as_str()).collect();
        assert!(!ids.iter().any(|id| id.starts_with("main/")), "{ids:?}");
        let provider =
            rig.shared.with_db(store::list_providers).unwrap().into_iter().find(|p| p.id == "backup").unwrap();
        assert_eq!((provider.data_use_terms.as_str(), provider.terms_checked_at.is_some()), ("no_training", true));
    });
}

// ---------- ROADMAP B4: a strict limit ----------

#[test]
fn a_strict_limit_needs_a_stated_maximum_output_and_holds_it() {
    rt().block_on(async {
        let rig = rig().await;
        rig.shared.with_db(|db| store::set_client_budget(db, &rig.client_id, Some(1_000_000))).unwrap();
        rig.shared.with_db(|db| store::set_client_strict_budget(db, &rig.client_id, true)).unwrap();
        // No output limit in the request, none known for the model: not sent.
        let refused = rig.post("/v1/chat/completions", chat("main/fast")).await;
        assert_eq!(refused.status(), 502);
        assert!(refused
            .text()
            .await
            .unwrap()
            .contains("limit is strict, and neither the request nor the model states a maximum output"));
        assert_eq!(rig.fake.count("fast"), 0);
        // The request's own limit is enough.
        let mut body = chat("main/fast");
        body["max_tokens"] = json!(256);
        assert_eq!(rig.post("/v1/chat/completions", body).await.status(), 200);
        // So is the model's stated maximum.
        rig.shared
            .with_db(|db| {
                store::set_capabilities(
                    db,
                    "main",
                    "fast",
                    &store::Capabilities { max_output_tokens: Some(8_000), ..store::Capabilities::default() },
                )
            })
            .unwrap();
        assert_eq!(rig.post("/v1/chat/completions", chat("main/fast")).await.status(), 200);
        // A model without a price is never used, whatever the unknown-price policy says.
        rig.shared.with_db(|db| store::set_client_policy(db, &rig.client_id, None, Some("allow"))).unwrap();
        let unpriced = rig.post("/v1/chat/completions", chat("main/steady")).await;
        // Refused as a spending-limit matter (429), not sent.
        assert_eq!(unpriced.status(), 429);
        assert_eq!(rig.fake.count("steady"), 0);
        assert!(unpriced.text().await.unwrap().contains("price is unknown"));
    });
}

/// Found by the benchmark (ROADMAP C3): auto ranked a subscription model at $0, but admission skipped it for any
/// client with a spending limit because it had no price, so the request went on to a paid model.
#[test]
fn a_subscription_model_is_used_under_a_spending_limit_and_holds_nothing() {
    rt().block_on(async {
        let rig = rig().await;
        rig.shared.with_db(|db| store::set_client_budget(db, &rig.client_id, Some(1_000))).unwrap();
        rig.shared
            .with_db(|db| {
                let _ = store::add_model(db, "main", "steady");
                store::update_model(db, "main", "steady", None, Some("subscription"), None)
            })
            .unwrap();
        for strict in [false, true] {
            rig.shared.with_db(|db| store::set_client_strict_budget(db, &rig.client_id, strict)).unwrap();
            let before = rig.fake.count("steady");
            let response = rig.post("/v1/chat/completions", chat("main/steady")).await;
            let status = response.status();
            assert_eq!(status, 200, "strict: {strict}: {}", response.text().await.unwrap());
            assert_eq!(rig.fake.count("steady"), before + 1);
        }
        // Its ledger entries cost no money, so the tiny limit is untouched.
        let spent = rig.shared.with_db(|db| store::usage_summary(db, 0, 10)).unwrap().total.known_micro_usd;
        assert_eq!(spent, 0);
    });
}

// ---------- ROADMAP item 4: why the cache was missed ----------

#[test]
fn each_miss_of_the_cache_is_recorded_with_its_likely_cause() {
    use crate::gateway::pricing::Cost;
    let conn = crate::db::open_in_memory().unwrap();
    let (client, _) = store::create_client(&conn, "Tests", None).unwrap();
    let mut ids = Vec::new();
    for label in ["Main", "Backup"] {
        let provider = store::add_provider(
            &conn,
            &NewProvider {
                kind: "custom",
                label,
                endpoint: "http://x.test",
                credential_ref: None,
                default_billing: "paid",
                wire: "openai",
            },
        )
        .unwrap();
        ids.push(provider.id);
    }
    let cost = Cost::Estimated { micro_usd: 1, source: "test".into() };
    let record = |provider: &str, input: u64, cached: Option<u64>, session: Option<&str>, hint: &str, outcome: &str| {
        let facts = store::AttemptFacts { cache_hint: hint.into(), ..store::AttemptFacts::default() };
        store::record_usage(
            &conn,
            &store::UsageRecord {
                client_id: Some(&client.id),
                provider_id: provider,
                model_id: "m",
                requested_model: "combo",
                input_tokens: Some(input),
                output_tokens: Some(10),
                cache_tokens: cached,
                cost: &cost,
                outcome,
                http_status: Some(200),
                latency_ms: 5,
                error: None,
                cache_write_tokens: Some(0),
                session_key: session,
                route_mode: Some("native"),
                route_reason: None,
                repeated_tokens: None,
                facts: Some(&facts),
            },
        )
        .unwrap();
        conn.query_row("SELECT cache_miss FROM usage_events ORDER BY created_at DESC, rowid DESC LIMIT 1", [], |r| {
            r.get::<_, Option<String>>(0)
        })
        .unwrap()
    };
    let (main, backup) = (ids[0].as_str(), ids[1].as_str());
    let s = Some("s-1");
    let miss = |m: Option<String>| m.unwrap_or_else(|| "hit".into());

    assert_eq!(miss(record(main, 5000, None, s, "none", "ok")), "not_reported");
    assert_eq!(miss(record(main, 500, Some(0), None, "none", "ok")), "below_minimum");
    assert_eq!(miss(record(main, 5000, Some(0), None, "dropped", "ok")), "marker_dropped");
    assert_eq!(miss(record(main, 5000, Some(0), Some("s-2"), "none", "ok")), "first_request");
    // Main has never read anything from its cache yet.
    assert_eq!(miss(record(main, 5000, Some(0), s, "none", "ok")), "not_cached_here");
    assert_eq!(miss(record(main, 5000, Some(4000), s, "none", "ok")), "hit");
    // Moments after a hit on the same model: the prompt's beginning must have changed.
    assert_eq!(miss(record(main, 5000, Some(0), s, "none", "ok")), "prefix_changed");
    assert_eq!(miss(record(main, 5000, Some(0), None, "none", "ok")), "no_session");
    assert_eq!(record(main, 5000, Some(0), s, "none", "fallback"), None, "a failed attempt has no cause");

    // Ten quiet minutes later.
    conn.execute("UPDATE usage_events SET created_at = created_at - 600000", []).unwrap();
    assert_eq!(miss(record(main, 5000, Some(0), s, "none", "ok")), "cache_expired");
    assert_eq!(miss(record(backup, 5000, Some(0), s, "none", "ok")), "model_switched");

    let mix = store::token_mix(&conn, 0).unwrap();
    let prefix = mix.misses.iter().find(|g| g.cause == "prefix_changed").unwrap();
    assert_eq!((prefix.requests, prefix.input_tokens), (1, 5000));
    assert_eq!(mix.misses.iter().map(|g| g.requests).sum::<u64>(), 9, "every miss but the hit and the failure");
}

/// Also found by the benchmark: a subscription request that failed or was cut short was recorded as money of
/// unknown amount, so a money limit treated it as possible spend.
#[test]
fn a_failed_subscription_request_is_quota_not_money() {
    use crate::gateway::pricing::Cost;
    let failed = super::quota_not_money(
        false,
        "subscription",
        Cost::Unknown { source: "HTTP 503: the provider may have run the request".into() },
    );
    assert!(failed.source().starts_with("subscription: counts against a quota, not money"), "{}", failed.source());
    let paid = super::quota_not_money(false, "paid", Cost::Unknown { source: "HTTP 503".into() });
    assert_eq!(paid.source(), "HTTP 503");
    let zero =
        super::quota_not_money(false, "subscription", Cost::Estimated { micro_usd: 0, source: "rejected".into() });
    assert_eq!(zero.micro_usd(), Some(0));
}

// ---------- ROADMAP B4: circuit breaker and probe ----------

#[test]
fn repeated_failures_wait_longer_and_a_probe_lifts_an_outage_that_is_over() {
    rt().block_on(async {
        let rig = rig().await;
        let now = crate::db::now_ms();
        let mark = || {
            rig.shared
                .with_db(|db| {
                    store::mark_health(
                        db,
                        "main",
                        "",
                        "unreachable",
                        "connection refused",
                        None,
                        Some(crate::db::now_ms() + 15_000),
                    )
                })
                .unwrap()
        };
        let until = || {
            rig.shared.with_db(|db| store::list_health(db, Some("main"))).unwrap()[0].until_ms.unwrap()
                - crate::db::now_ms()
        };
        mark();
        assert!(until() <= 15_000);
        mark();
        assert!(until() > 25_000 && until() <= 30_000, "the second failure in a row waits twice as long");
        mark();
        assert!(until() > 55_000, "the third four times as long");
        assert!(rig.shared.with_db(|db| store::list_health(db, Some("main"))).unwrap()[0]
            .reason
            .contains("failure 3 in a row"));

        // Not checked again within a minute of the last failure.
        assert!(probe::probe_once(&rig.shared).await.is_empty());
        // A minute later, the provider's model list answers: the outage is lifted.
        rig.shared.with_db(|db| db.execute("UPDATE connection_health SET updated_at = ?1", [now - 120_000])).unwrap();
        assert_eq!(probe::probe_once(&rig.shared).await, ["main"]);
        assert!(rig.shared.with_db(|db| store::list_health(db, Some("main"))).unwrap().is_empty());

        // A connection whose list does not answer stays out, and is not checked again for a minute.
        rig.shared
            .with_db(|db| {
                db.execute("UPDATE providers SET endpoint = 'http://127.0.0.1:9/v1' WHERE id = 'sibling'", [])
            })
            .unwrap();
        rig.shared
            .with_db(|db| store::mark_health(db, "sibling", "", "unreachable", "refused", None, Some(now + 600_000)))
            .unwrap();
        rig.shared
            .with_db(|db| {
                db.execute(
                    "UPDATE connection_health SET updated_at = ?1 WHERE provider_id = 'sibling'",
                    [now - 120_000],
                )
            })
            .unwrap();
        assert!(probe::probe_once(&rig.shared).await.is_empty());
        assert_eq!(rig.shared.with_db(|db| store::list_health(db, Some("sibling"))).unwrap().len(), 1);
        assert!(probe::probe_once(&rig.shared).await.is_empty());

        // A rejected key is never lifted by a probe.
        rig.shared
            .with_db(|db| store::mark_health(db, "backup", "", "invalid_credentials", "401", Some(401), None))
            .unwrap();
        rig.shared
            .with_db(|db| {
                db.execute("UPDATE connection_health SET updated_at = ?1 WHERE provider_id = 'backup'", [now - 120_000])
            })
            .unwrap();
        probe::probe_once(&rig.shared).await;
        assert_eq!(
            rig.shared.with_db(|db| store::list_health(db, Some("backup"))).unwrap()[0].state,
            "invalid_credentials"
        );
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
        assert_eq!(retried.headers()["x-pygg-route"], "backup/steady");
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
        let start_row = store::AttemptStart {
            client_id: &client.id,
            provider_id: "main",
            model_id: "fast",
            requested_model: "main/fast",
            session_key: None,
            route_mode: "native",
            hold_micro: 0,
            owner_generation: "dead",
        };
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
        let request = {
            let (http, base, token) = (rig.http.clone(), rig.base.clone(), rig.token.clone());
            tokio::spawn(async move {
                http.post(format!("{base}/v1/chat/completions"))
                    .bearer_auth(token)
                    .json(&chat("main/slow"))
                    .send()
                    .await
                    .map(|r| r.status().as_u16())
            })
        };
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
        let stream = {
            let (http, base, token) = (rig.http.clone(), rig.base.clone(), rig.token.clone());
            tokio::spawn(async move {
                let response = http
                    .post(format!("{base}/v1/chat/completions"))
                    .bearer_auth(token)
                    .json(&json!({ "model": "main/drip", "stream": true, "messages": [] }))
                    .send()
                    .await;
                match response {
                    Ok(r) => r.text().await.map(|t| t.len()).unwrap_or(0),
                    Err(_) => 0,
                }
            })
        };
        tokio::time::sleep(Duration::from_millis(400)).await;
        rig.stop_server(Duration::from_millis(100)).await;
        assert!(rig.shared.owns_store(), "the stream pump still holds the store");
        assert_eq!(
            start(other_gateway(&path), 0).await.err().map(|e| e.kind()),
            Some(std::io::ErrorKind::ResourceBusy)
        );

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
        let orphan = store::AttemptStart {
            client_id: &rig.client_id,
            provider_id: "main",
            model_id: "orphan",
            requested_model: "main/orphan",
            session_key: None,
            route_mode: "native",
            hold_micro: 700,
            owner_generation: "earlier",
        };
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
        let request = {
            let (http, token) = (rig.http.clone(), rig.token.clone());
            tokio::spawn(async move {
                http.post(format!("{base}/v1/chat/completions"))
                    .bearer_auth(token)
                    .json(&chat("main/slow"))
                    .send()
                    .await
                    .map(|r| r.status().as_u16())
            })
        };
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
        let orphan = store::AttemptStart {
            client_id: &client.id,
            provider_id: "main",
            model_id: "orphan",
            requested_model: "main/orphan",
            session_key: None,
            route_mode: "native",
            hold_micro: 0,
            owner_generation: "dead",
        };
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
        assert_eq!(
            start(other_gateway(&path), 0).await.err().map(|e| e.kind()),
            Some(std::io::ErrorKind::ResourceBusy)
        );

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
    rig.shared
        .with_db(|db| {
            let _ = store::add_model(db, provider, model);
            store::set_capabilities(
                db,
                provider,
                model,
                &store::Capabilities {
                    tier: Some(tier.into()),
                    context_tokens: Some(128_000),
                    max_output_tokens: Some(8192),
                    supports_tools: tools,
                    supports_vision: Some(false),
                    ..store::Capabilities::default()
                },
            )
        })
        .unwrap();
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
        assert_eq!(response.headers()["x-pygg-route"], "backup/steady");
        let why = response.headers()["x-pygg-auto"].to_str().unwrap().to_string();
        assert!(
            why.starts_with(
                "auto; backup/steady ranked 1 of 2 (lowest eligible tier first): tier T2 (minimum T2); free"
            ),
            "{why}"
        );
        assert!(rig.ledger().pop().unwrap().route_reason.unwrap().contains("tier T2"));

        // A request with tools needs a model known to take them: Backup's support is unknown.
        let tools = rig
            .post(
                "/v1/chat/completions",
                json!({ "model": "auto", "messages": [],
            "tools": [{ "type": "function", "function": { "name": "f", "parameters": { "type": "object" } } }] }),
            )
            .await;
        assert_eq!(tools.headers()["x-pygg-route"], "main/fast");

        // auto/cheap: the free one, by estimated cost.
        let cheap = rig.post("/v1/chat/completions", chat("auto/cheap")).await;
        assert_eq!(cheap.headers()["x-pygg-route"], "backup/steady");

        // The aliases are listed with the models.
        let models: Value = rig
            .http
            .get(format!("{}/v1/models", rig.base))
            .bearer_auth(&rig.token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let ids: Vec<&str> = models["data"].as_array().unwrap().iter().filter_map(|m| m["id"].as_str()).collect();
        assert!(ids.contains(&"auto") && ids.contains(&"auto/cheap"));
    });
}

#[test]
fn a_model_answering_429_without_a_reset_is_held_out_of_auto_but_not_of_a_named_route() {
    rt().block_on(async {
        let rig = rig().await;
        // Backup is free, so its model ranks first; it answers 429 "quota" with no reset, as Gemini's Gemma did.
        set_caps(&rig, "backup", "quota429", "T2", Some(true));
        set_caps(&rig, "main", "fast", "T2", Some(true));
        let first = rig.post("/v1/chat/completions", chat("auto")).await;
        assert_eq!(first.status(), 200);
        assert_eq!(first.headers()["x-pygg-route"], "main/fast");
        assert_eq!(rig.fake.count("quota429"), 1);
        let outcomes: Vec<(String, Option<u16>)> =
            rig.ledger().into_iter().map(|e| (e.outcome, e.http_status)).collect();
        assert_eq!(outcomes, [("fallback".into(), Some(429)), ("ok".into(), Some(200))]);

        // Held out of auto from the first strike, and the header says so.
        let held = rig.shared.with_db(|db| auto::hold(db, "backup", "quota429", crate::db::now_ms())).unwrap();
        assert_eq!(held.strikes, 1);
        assert!(
            held.until_ms.unwrap() - crate::db::now_ms() > 9 * 60_000,
            "ten minutes, not the 30 s rate-limit cooldown"
        );
        let again = rig.post("/v1/chat/completions", chat("auto/cheap")).await;
        assert_eq!(again.headers()["x-pygg-route"], "main/fast");
        let why = again.headers()["x-pygg-auto"].to_str().unwrap().to_string();
        assert!(why.contains("excluded, 1 held after 429: backup/quota429"), "{why}");
        assert_eq!(rig.fake.count("quota429"), 1, "not sent to");

        // A client naming the model, or a combo, still reaches it once the provider's own cooldown is over; that
        // second 429 in a row doubles the hold.
        rig.shared.with_db(|db| store::mark_healthy(db, "backup", "quota429")).unwrap();
        assert_eq!(rig.post("/v1/chat/completions", chat("backup/quota429")).await.status(), 429);
        assert_eq!(rig.fake.count("quota429"), 2);
        let held = rig.shared.with_db(|db| auto::hold(db, "backup", "quota429", crate::db::now_ms())).unwrap();
        assert_eq!(held.strikes, 2);
        assert!(held.until_ms.unwrap() - crate::db::now_ms() > 19 * 60_000);
        rig.shared.with_db(|db| store::mark_healthy(db, "backup", "quota429")).unwrap();
        assert_eq!(rig.post("/v1/chat/completions", chat("auto")).await.headers()["x-pygg-route"], "main/fast");
        assert_eq!(rig.fake.count("quota429"), 2, "auto still leaves it out");

        // Clearing the connection's health in Connections lifts the hold.
        rig.shared.with_db(|db| store::clear_health(db, "backup")).unwrap();
        assert_eq!(rig.post("/v1/chat/completions", chat("auto")).await.headers()["x-pygg-route"], "main/fast");
        assert_eq!(rig.fake.count("quota429"), 3);
    });
}

#[test]
fn a_prefill_refusal_falls_back_to_the_next_target_before_anything_reaches_the_client() {
    rt().block_on(async {
        let rig = rig().await;
        let assistant_last = json!({ "model": "prefill", "messages": [{ "role": "user", "content": "ping" },
            { "role": "assistant", "content": "The answer is" }] });
        let response = rig.post("/v1/chat/completions", assistant_last.clone()).await;
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["x-pygg-route"], "main/fast");
        let outcomes: Vec<(String, Option<u16>)> =
            rig.ledger().into_iter().map(|e| (e.outcome, e.http_status)).collect();
        assert_eq!(outcomes, [("fallback".into(), Some(400)), ("ok".into(), Some(200))]);
        assert!(
            rig.shared.with_db(|db| store::list_health(db, Some("claude"))).unwrap().is_empty(),
            "the model is well"
        );
        // Named alone, the refusal reaches the client as the 400 it is.
        let alone = rig
            .post(
                "/v1/chat/completions",
                json!({ "model": "claude/noprefill", "messages": assistant_last["messages"] }),
            )
            .await;
        assert_eq!(alone.status(), 400);
        assert!(alone.text().await.unwrap().contains("assistant message prefill"));
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
        assert_eq!(first.headers()["x-pygg-route"], "main/fast");
        // A free T2 model becomes eligible.
        set_caps(&rig, "backup", "steady", "T2", None);
        let kept = rig.post_session("/v1/chat/completions", "s-auto", chat("auto")).await;
        assert_eq!(kept.headers()["x-pygg-route"], "main/fast", "no switch inside a session");
        assert!(kept.headers()["x-pygg-auto"].to_str().unwrap().ends_with("kept for this session"));
        // A new session or no session gets the better choice.
        let fresh = rig.post("/v1/chat/completions", chat("auto")).await;
        assert_eq!(fresh.headers()["x-pygg-route"], "backup/steady");
    });
}

#[test]
fn auto_respects_the_client_minimum_tier_and_its_connection_list() {
    rt().block_on(async {
        let rig = rig().await;
        set_caps(&rig, "main", "fast", "T3", Some(true));
        set_caps(&rig, "backup", "steady", "T1", None);
        assert_eq!(rig.post("/v1/chat/completions", chat("auto")).await.headers()["x-pygg-route"], "main/fast");
        rig.shared.with_db(|db| store::set_client_auto_min_tier(db, &rig.client_id, "T1")).unwrap();
        assert_eq!(rig.post("/v1/chat/completions", chat("auto")).await.headers()["x-pygg-route"], "backup/steady");
        rig.shared
            .with_db(|db| store::set_client_connections(db, &rig.client_id, Some(&["main".to_string()])))
            .unwrap();
        assert_eq!(rig.post("/v1/chat/completions", chat("auto")).await.headers()["x-pygg-route"], "main/fast");
    });
}

#[test]
fn a_listed_model_keeps_what_the_user_set() {
    let conn = crate::db::open_in_memory().unwrap();
    let p = store::add_provider(
        &conn,
        &NewProvider {
            kind: "openrouter",
            label: "Router",
            endpoint: "https://o.test",
            credential_ref: None,
            default_billing: "paid",
            wire: "openai",
        },
    )
    .unwrap();
    let listed = |context: u64| store::Listed {
        id: "vendor/m".into(),
        context_tokens: Some(context),
        max_output_tokens: Some(4096),
        supports_tools: Some(true),
        supports_vision: Some(false),
        thinking_types: None,
        effort_levels: None,
        price: Some(Price {
            input_micro_per_mtok: 1,
            output_micro_per_mtok: 2,
            cached_input_micro_per_mtok: None,
            cache_write_micro_per_mtok: None,
            source: "provider list price".into(),
            observed_at: 0,
        }),
    };
    store::import_listed(&conn, &p.id, &[listed(100_000)], Some("provider: openrouter /models")).unwrap();
    let m = store::list_models(&conn, Some(&p.id)).unwrap().remove(0);
    assert_eq!(
        (m.capabilities.context_tokens, m.capabilities.source.as_deref()),
        (Some(100_000), Some("provider: openrouter /models"))
    );
    assert_eq!(m.price.as_ref().unwrap().source, "provider list price");

    // The user sets a tier and their own price; a later refresh changes neither.
    store::set_capabilities(
        &conn,
        &p.id,
        "vendor/m",
        &store::Capabilities { tier: Some("T2".into()), context_tokens: Some(64_000), ..m.capabilities.clone() },
    )
    .unwrap();
    store::update_model(&conn, &p.id, "vendor/m", None, None, Some(Some(price()))).unwrap();
    store::import_listed(&conn, &p.id, &[listed(200_000)], Some("provider: openrouter /models")).unwrap();
    let m = store::list_models(&conn, Some(&p.id)).unwrap().remove(0);
    assert_eq!(
        (m.capabilities.tier.as_deref(), m.capabilities.context_tokens, m.capabilities.source.as_deref()),
        (Some("T2"), Some(64_000), Some("user"))
    );
    assert_eq!(m.price.unwrap().source, "user");

    // "auto" is reserved.
    let auto = store::add_provider(
        &conn,
        &NewProvider {
            kind: "custom",
            label: "Auto",
            endpoint: "https://a.test",
            credential_ref: None,
            default_billing: "paid",
            wire: "openai",
        },
    )
    .unwrap();
    assert_eq!(auto.id, "auto-connection");
    assert!(store::save_combo(
        &conn,
        None,
        "auto",
        &[Target { provider_id: p.id.clone(), model_id: "vendor/m".into() }]
    )
    .is_err());
}

#[test]
fn one_providers_two_api_formats_share_a_quota_group() {
    let conn = crate::db::open_in_memory().unwrap();
    let add = |kind: &str, wire: &str| {
        store::add_provider(
            &conn,
            &NewProvider {
                kind,
                label: kind,
                endpoint: "https://x.test",
                credential_ref: None,
                default_billing: "paid",
                wire,
            },
        )
        .unwrap()
    };
    let openai = add("deepseek", "openai");
    let anthropic = add("deepseek-anthropic", "anthropic");
    let other = add("mistral", "openai");
    assert_eq!(
        (openai.quota_group.as_str(), anthropic.quota_group.as_str(), other.quota_group.as_str()),
        ("deepseek", "deepseek", "mistral")
    );
    store::update_provider(&conn, &anthropic.id, None, None, Some(true)).unwrap();
    assert_eq!(store::get_provider(&conn, &anthropic.id).unwrap().unwrap().quota_group, anthropic.id);
}

// ---------- ADR-033: the Responses API surface ----------

/// A Responses request as Codex sends it, trimmed: instructions, a developer message, a function and a custom tool,
/// a hosted tool no other provider runs, and the fields Codex always sets.
fn responses_request(model: &str, stream: bool) -> Value {
    json!({ "model": model, "stream": stream, "store": false, "include": ["reasoning.encrypted_content"],
        "reasoning": { "summary": "auto" }, "tool_choice": "auto", "parallel_tool_calls": true, "prompt_cache_key": "t-1",
        "instructions": "Be brief.",
        "tools": [
            { "type": "function", "name": "read", "description": "Read a file", "strict": false,
                "parameters": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] } },
            { "type": "custom", "name": "apply_patch", "description": "Edit files",
                "format": { "type": "grammar", "syntax": "lark", "definition": "start: patch" } },
            { "type": "web_search", "external_web_access": false }],
        "input": [{ "type": "message", "role": "developer", "content": [{ "type": "input_text", "text": "Use tools." }] },
            { "type": "message", "role": "user", "content": [{ "type": "input_text", "text": "ping" }] }] })
}

/// The `data` of each server-sent event.
fn responses_events(text: &str) -> Vec<Value> {
    text.split("\n\n")
        .filter_map(|block| block.lines().find_map(|l| l.strip_prefix("data: ")))
        .map(|d| serde_json::from_str(d).unwrap())
        .collect()
}

#[test]
fn a_responses_request_reaches_an_openai_format_connection_through_chat_completions() {
    rt().block_on(async {
        let rig = rig().await;
        let response = rig.post("/v1/responses", responses_request("main/fast", false)).await;
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["x-pygg-mode"], "translated:responses->openai");
        assert_eq!(response.headers()["x-pygg-route"], "main/fast");
        assert_eq!(response.headers()["x-pygg-dropped"], "web_search tool,prompt_cache_key");
        let body: Value = response.json().await.unwrap();
        assert_eq!(
            (body["object"].as_str(), body["status"].as_str(), body["model"].as_str()),
            (Some("response"), Some("completed"), Some("main/fast"))
        );
        assert_eq!(
            body["output"][0]["content"][0],
            json!({ "type": "output_text", "text": "pong", "annotations": [] })
        );
        assert_eq!(
            (body["usage"]["input_tokens"].as_u64(), body["usage"]["output_tokens"].as_u64()),
            (Some(1000), Some(500))
        );

        // What the provider received: one system message, the user turn, and both tools as functions.
        let sent = rig.fake.last_body.lock().unwrap().clone().unwrap();
        assert_eq!(sent["messages"][0], json!({ "role": "system", "content": "Be brief.\n\nUse tools." }));
        assert_eq!(sent["messages"][1], json!({ "role": "user", "content": "ping" }));
        let tools: Vec<&str> =
            sent["tools"].as_array().unwrap().iter().map(|t| t["function"]["name"].as_str().unwrap()).collect();
        assert_eq!(tools, ["read", "apply_patch"]);
        assert_eq!(sent["tools"][1]["function"]["parameters"]["properties"]["input"]["type"], "string");
        assert!(sent.get("input").is_none() && sent.get("instructions").is_none());

        let event = rig.ledger().pop().unwrap();
        assert_eq!(event.route_mode.as_deref(), Some("translated:responses->openai"));
        assert_eq!(
            (event.input_tokens, event.output_tokens, event.cost_status.as_str()),
            (Some(1000), Some(500), "estimated")
        );
        assert_eq!(event.cost_micro_usd, Some(300));
    });
}

#[test]
fn a_streamed_custom_tool_call_comes_back_as_codex_reads_it() {
    rt().block_on(async {
        let rig = rig().await;
        rig.priced("main", "patcher");
        let response = rig.post_session("/v1/responses", "codex-1", responses_request("main/patcher", true)).await;
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "text/event-stream");
        let events = responses_events(&response.text().await.unwrap());
        let kinds: Vec<&str> = events.iter().map(|e| e["type"].as_str().unwrap()).collect();
        assert_eq!(
            kinds,
            [
                "response.created",
                "response.in_progress",
                "response.output_item.added",
                "response.custom_tool_call_input.delta",
                "response.custom_tool_call_input.done",
                "response.output_item.done",
                "response.completed"
            ]
        );
        assert_eq!(
            events[5]["item"],
            json!({ "id": events[5]["item"]["id"], "type": "custom_tool_call", "status": "completed",
            "call_id": "call_p", "name": "apply_patch", "input": "*** Begin Patch\n*** End Patch" })
        );
        let done = &events[6]["response"];
        assert_eq!((done["status"].as_str(), done["usage"]["output_tokens"].as_u64()), (Some("completed"), Some(500)));

        // The next turn sends the call and its output back; they reach the provider as a tool call and its result.
        let mut next = responses_request("main/fast", false);
        let input = next["input"].as_array_mut().unwrap();
        input.push(events[5]["item"].clone());
        input
            .push(json!({ "type": "custom_tool_call_output", "call_id": "call_p", "output": "Success. Updated a.py" }));
        let response = rig.post_session("/v1/responses", "codex-1", next).await;
        assert_eq!(response.status(), 200);
        let sent = rig.fake.last_body.lock().unwrap().clone().unwrap();
        assert_eq!(
            sent["messages"][2]["tool_calls"][0],
            json!({ "id": "call_p", "type": "function", "function": {
            "name": "apply_patch", "arguments": "{\"input\":\"*** Begin Patch\\n*** End Patch\"}" } })
        );
        assert_eq!(
            sent["messages"][3],
            json!({ "role": "tool", "tool_call_id": "call_p", "content": "Success. Updated a.py" })
        );

        tokio::time::sleep(Duration::from_millis(100)).await;
        let ledger = rig.ledger();
        let streamed = &ledger[ledger.len() - 2];
        assert_eq!(
            (streamed.outcome.as_str(), streamed.output_tokens, streamed.session_key.as_deref()),
            ("ok", Some(500), Some("codex-1"))
        );
    });
}

#[test]
fn a_responses_request_reaches_an_anthropic_connection_through_two_translations() {
    rt().block_on(async {
        let rig = rig().await;
        let response = rig.post("/v1/responses", responses_request("claude/sonnet", false)).await;
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["x-pygg-mode"], "translated:responses->anthropic");
        assert_eq!(response.headers()["x-pygg-dropped"], "web_search tool,prompt_cache_key");
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["output"][0]["content"][0]["text"], "cache_control=false thinking=false beta=-");
        assert_eq!(
            (body["usage"]["input_tokens"].as_u64(), body["usage"]["input_tokens_details"]["cached_tokens"].as_u64()),
            (Some(1000), Some(900))
        );
        let sent = rig.fake.last_body.lock().unwrap().clone().unwrap();
        assert_eq!(sent["system"], "Be brief.\n\nUse tools.");
        assert_eq!(sent["tools"][1]["name"], "apply_patch");
        assert_eq!(sent["tools"][1]["input_schema"]["required"], json!(["input"]));

        let text = rig.post("/v1/responses", responses_request("claude/sonnet", true)).await.text().await.unwrap();
        let events = responses_events(&text);
        assert_eq!(events.first().unwrap()["type"], "response.created");
        let done = events.last().unwrap();
        assert_eq!(done["type"], "response.completed");
        assert_eq!(done["response"]["output"][0]["content"][0]["text"], "cache_control=false thinking=false beta=-");
        tokio::time::sleep(Duration::from_millis(100)).await;
        let event = rig.ledger().pop().unwrap();
        assert_eq!(
            (event.route_mode.as_deref(), event.output_tokens, event.cache_write_tokens),
            (Some("translated:responses->anthropic"), Some(7), Some(90))
        );
    });
}

#[test]
fn a_provider_that_serves_responses_gets_the_request_unchanged() {
    rt().block_on(async {
        let rig = rig().await;
        let endpoint = rig.shared.with_db(|db| store::get_provider(db, "main")).unwrap().unwrap().endpoint;
        let openai = rig
            .shared
            .with_db(|db| {
                store::add_provider(
                    db,
                    &NewProvider {
                        kind: "openai",
                        label: "OpenAI",
                        endpoint: &endpoint,
                        credential_ref: Some("provider-main"),
                        default_billing: "paid",
                        wire: "openai",
                    },
                )
            })
            .unwrap();
        rig.priced(&openai.id, "gpt");
        let request = responses_request(&format!("{}/gpt", openai.id), false);
        let response = rig.post("/v1/responses", request.clone()).await;
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["x-pygg-mode"], "native");
        assert!(response.headers().get("x-pygg-dropped").is_none());
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["id"], "resp_up", "the provider's own answer");
        let sent = rig.fake.last_body.lock().unwrap().clone().unwrap();
        let mut expected = request.clone();
        expected["model"] = json!("gpt");
        assert_eq!(sent, expected, "only the model changed");
        assert_eq!(rig.fake.count("responses:gpt"), 1);
        let event = rig.ledger().pop().unwrap();
        assert_eq!((event.input_tokens, event.cache_tokens, event.output_tokens), (Some(1000), Some(800), Some(500)));

        let text = rig
            .post("/v1/responses", responses_request(&format!("{}/gpt", openai.id), true))
            .await
            .text()
            .await
            .unwrap();
        assert!(text.starts_with("event: response.created") && text.contains("\"resp_up\""), "{text}");
        tokio::time::sleep(Duration::from_millis(100)).await;
        let event = rig.ledger().pop().unwrap();
        assert_eq!((event.outcome.as_str(), event.output_tokens), ("ok", Some(500)));

        // The provider's own error body goes back to a client that speaks its format.
        rig.priced(&openai.id, "broken");
        let refused = rig.post("/v1/responses", responses_request(&format!("{}/broken", openai.id), false)).await;
        assert_eq!(refused.status(), 400);
        let body: Value = refused.json().await.unwrap();
        assert_eq!(body["error"]["code"], "unsupported_parameter");
    });
}

#[test]
fn responses_errors_are_in_the_openai_format() {
    rt().block_on(async {
        let rig = rig().await;
        let missing = rig.post("/v1/responses", responses_request("nowhere", false)).await;
        assert_eq!(missing.status(), 404);
        let body: Value = missing.json().await.unwrap();
        assert_eq!(body["error"]["type"], "model_not_found");

        let mut chained = responses_request("main/fast", false);
        chained["previous_response_id"] = json!("resp_1");
        let refused = rig.post("/v1/responses", chained).await;
        assert_eq!(refused.status(), 400);
        let body: Value = refused.json().await.unwrap();
        assert!(body["error"]["message"].as_str().unwrap().contains("does not store responses"));
        assert_eq!(rig.fake.count("fast"), 0, "nothing was sent");

        // A translated provider error is described, not passed through in a format the client does not speak.
        rig.priced("main", "broken");
        let broken = rig.post("/v1/responses", responses_request("main/broken", false)).await;
        assert_eq!(broken.status(), 400);
        let body: Value = broken.json().await.unwrap();
        assert!(body["error"]["message"].as_str().unwrap().contains("bad field"), "{body}");

        let anonymous = rig
            .http
            .post(format!("{}/v1/responses", rig.base))
            .json(&responses_request("main/fast", false))
            .send()
            .await
            .unwrap();
        assert_eq!(anonymous.status(), 401);
    });
}

#[test]
fn a_cheapest_combo_orders_its_members_by_this_requests_cost() {
    let conn = crate::db::open_in_memory().unwrap();
    let paid = store::add_provider(
        &conn,
        &store::NewProvider {
            kind: "openai",
            label: "Paid",
            endpoint: "https://example.test/v1/",
            credential_ref: Some("provider-x"),
            default_billing: "paid",
            wire: "openai",
        },
    )
    .unwrap();
    let local = store::add_provider(
        &conn,
        &store::NewProvider {
            kind: "ollama",
            label: "Local",
            endpoint: "http://127.0.0.1:11434/v1/",
            credential_ref: None,
            default_billing: "local",
            wire: "openai",
        },
    )
    .unwrap();
    for (p, m) in [(&paid.id, "dear"), (&paid.id, "cheap"), (&paid.id, "unpriced"), (&local.id, "qwen")] {
        store::add_model(&conn, p, m).unwrap();
    }
    let price = |input: u64, output: u64| {
        Some(Some(crate::gateway::pricing::Price {
            input_micro_per_mtok: input,
            output_micro_per_mtok: output,
            cached_input_micro_per_mtok: None,
            cache_write_micro_per_mtok: None,
            source: "user".into(),
            observed_at: 0,
        }))
    };
    store::update_model(&conn, &paid.id, "dear", None, None, price(15_000_000, 75_000_000)).unwrap();
    store::update_model(&conn, &paid.id, "cheap", None, None, price(100_000, 500_000)).unwrap();
    let members = [("dear", &paid.id), ("unpriced", &paid.id), ("cheap", &paid.id), ("qwen", &local.id)]
        .map(|(m, p)| store::Target { provider_id: p.clone(), model_id: m.into() });
    let combo = store::save_combo(&conn, None, "thrifty", &members).unwrap();
    store::set_combo_strategy(&conn, &combo.id, "cheapest").unwrap();
    assert!(store::set_combo_strategy(&conn, &combo.id, "random").is_err());

    let shared = Shared::new(Arc::new(Mutex::new(conn)), Arc::new(crate::gateway::OsSecrets));
    let mut targets = shared.with_db(|db| store::resolve(db, "thrifty")).unwrap();
    let mut reasons = HashMap::new();
    super::cheapest_first(
        &shared,
        &mut targets,
        &json!({"messages": [{"role": "user", "content": "hi"}], "max_tokens": 100}),
        "thrifty",
        &mut reasons,
    );
    let order: Vec<&str> = targets.iter().map(|(_, m)| m.as_str()).collect();
    assert_eq!(order, ["qwen", "cheap", "dear", "unpriced"], "local at $0, then by cost, unknown price last");
    assert!(reasons[&(paid.id.clone(), "unpriced".to_string())].contains("price unknown"));
}

// ---------- Overhead (production readiness: "no measured overhead") ----------

/// What the gateway adds to a request: the same requests sent to the fake provider directly and through a gateway on a
/// store file (WAL, every attempt journaled and settled as in the app). Ignored by default; run in release:
/// `cargo test --release --lib gateway::tests::overhead -- --ignored --nocapture`, with `PYGG_OVERHEAD_OUT=file.json`
/// to save the figures (benchmark/reports/2026-11-14-gateway-overhead.md).
#[test]
#[ignore]
fn overhead() {
    fn stats(mut ms: Vec<f64>) -> Value {
        ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let at = |q: f64| ms[((ms.len() as f64 - 1.0) * q).round() as usize];
        json!({ "n": ms.len(), "p50": at(0.5), "p95": at(0.95), "p99": at(0.99), "mean": ms.iter().sum::<f64>() / ms.len() as f64 })
    }
    rt().block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let rig = rig_on_store(&dir.path().join("store.sqlite3")).await;
        rig.shared.with_db(|db| store::set_client_policy(db, &rig.client_id, Some(64), None)).unwrap();
        if let Ok(mode) = std::env::var("PYGG_OVERHEAD_SYNC") { rig.shared.with_db(|db| db.execute_batch(&format!("PRAGMA synchronous = {mode}"))).unwrap(); }
        let endpoint = rig.shared.with_db(store::list_providers).unwrap().into_iter().find(|p| p.id == "main").unwrap().endpoint;
        let http = reqwest::Client::builder().pool_max_idle_per_host(64).build().unwrap();
        let direct = |body: Value| http.post(format!("{endpoint}/chat/completions")).bearer_auth("key-1").json(&body).send();
        let through = |body: Value| http.post(format!("{}/v1/chat/completions", rig.base)).bearer_auth(&rig.token).json(&body).send();
        let body = |model: &str, stream: bool, chars: usize| {
            let mut b = json!({ "model": model, "messages": [{ "role": "user", "content": "x".repeat(chars) }] });
            if stream { b["stream"] = json!(true); b["stream_options"] = json!({ "include_usage": true }); }
            b
        };
        let mut scenarios = Vec::new();
        // A short prompt and an agent-sized one (400 000 characters, about 100 000 tokens), whole and streamed.
        for (name, chars, stream, n) in [("short prompt", 20, false, 400), ("short prompt, streamed", 20, true, 400),
            ("400 kB prompt", 400_000, false, 150), ("400 kB prompt, streamed", 400_000, true, 150)] {
            let mut sides = Vec::new();
            for via in ["direct", "gateway"] {
                let model = if via == "direct" { "fast" } else { "main/fast" };
                let mut ms = Vec::new();
                for i in 0..n + 20 {
                    let started = Instant::now();
                    let response = if via == "direct" { direct(body(model, stream, chars)).await } else { through(body(model, stream, chars)).await }.unwrap();
                    assert_eq!(response.status(), 200);
                    response.bytes().await.unwrap();
                    if i >= 20 { ms.push(started.elapsed().as_secs_f64() * 1000.0); }
                }
                sides.push(stats(ms));
            }
            let added = sides[1]["p50"].as_f64().unwrap() - sides[0]["p50"].as_f64().unwrap();
            println!("{name:<26} direct p50 {:>7.2} ms p95 {:>7.2} | gateway p50 {:>7.2} ms p95 {:>7.2} | added p50 {added:>6.2} ms",
                sides[0]["p50"].as_f64().unwrap(), sides[0]["p95"].as_f64().unwrap(), sides[1]["p50"].as_f64().unwrap(), sides[1]["p95"].as_f64().unwrap());
            scenarios.push(json!({ "name": name, "promptChars": chars, "stream": stream, "direct": sides[0], "gateway": sides[1], "addedP50Ms": added }));
        }
        // Sixteen clients at once, short prompts: throughput and latency under load.
        let mut load = Vec::new();
        for via in ["direct", "gateway"] {
            let per = 125;
            let started = Instant::now();
            let workers: Vec<_> = (0..16).map(|_| {
                let (http, endpoint, base, token) = (http.clone(), endpoint.clone(), rig.base.clone(), rig.token.clone());
                tokio::spawn(async move {
                    let mut ms = Vec::new();
                    for _ in 0..per {
                        let t = Instant::now();
                        let r = if via == "direct" {
                            http.post(format!("{endpoint}/chat/completions")).bearer_auth("key-1").json(&json!({ "model": "fast", "messages": [] })).send().await
                        } else {
                            http.post(format!("{base}/v1/chat/completions")).bearer_auth(&token).json(&json!({ "model": "main/fast", "messages": [] })).send().await
                        }.unwrap();
                        assert_eq!(r.status(), 200);
                        r.bytes().await.unwrap();
                        ms.push(t.elapsed().as_secs_f64() * 1000.0);
                    }
                    ms
                })
            }).collect();
            let mut ms = Vec::new();
            for w in workers { ms.extend(w.await.unwrap()); }
            let seconds = started.elapsed().as_secs_f64();
            let s = stats(ms);
            println!("16 at once, {via:<7} {:>7.0} requests/s, p50 {:>6.2} ms, p95 {:>6.2} ms", 2000.0 / seconds, s["p50"].as_f64().unwrap(), s["p95"].as_f64().unwrap());
            load.push(json!({ "via": via, "requestsPerSecond": 2000.0 / seconds, "latency": s }));
        }
        let ledger: i64 = rig.shared.with_db(|db| db.query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))).unwrap();
        println!("ledger entries written: {ledger}");
        if let Ok(out) = std::env::var("PYGG_OVERHEAD_OUT") {
            std::fs::write(out, serde_json::to_string_pretty(&json!({ "scenarios": scenarios, "concurrent16": load,
                "ledgerEntries": ledger })).unwrap()).unwrap();
        }
    });
}

// ---------- Key rings (ADR-038) ----------

fn ring_model(rig: &Rig) {
    rig.shared.with_db(|db| {
        store::add_model(db, "main", "ring").unwrap();
        store::update_model(db, "main", "ring", None, None, Some(Some(price()))).unwrap();
    });
}

#[test]
fn a_key_ring_moves_past_a_rate_limited_or_rejected_key_and_the_ledger_names_each_key() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let rig = rig().await;
        ring_model(&rig);
        let limited = rig.shared.with_db(|db| {
            let limited = keys::add(db, "main", "Limited", "provider-limited", true, crate::db::now_ms()).unwrap();
            keys::add(db, "main", "Second", "provider-k2", true, crate::db::now_ms()).unwrap();
            keys::move_key(db, &limited.id, true).unwrap();
            limited
        });
        // Order: Limited, First key, Second. Limited answers 429; the first key answers.
        let response = rig.post("/v1/chat/completions", chat("main/ring")).await;
        assert_eq!(response.status(), 200);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["choices"][0]["message"]["content"], "answered with key-1");
        assert_eq!(*rig.fake.keys.lock().unwrap(), ["key-limited", "key-1"]);
        let ledger = rig.ledger();
        let labels: Vec<(Option<&str>, &str)> =
            ledger.iter().map(|e| (e.key_label.as_deref(), e.outcome.as_str())).collect();
        assert_eq!(labels, [(Some("Limited"), "fallback"), (Some("First key"), "ok")]);
        assert!(
            rig.shared.with_db(|db| store::list_health(db, Some("main"))).unwrap().is_empty(),
            "the connection is not marked"
        );
        let marks = rig.shared.with_db(|db| keys::health(db, "main", crate::db::now_ms())).unwrap();
        assert_eq!(
            (marks[0].key_id.as_str(), marks[0].model_id.as_str(), marks[0].state.as_str()),
            (limited.id.as_str(), "ring", "rate_limited")
        );
        assert!(marks[0].until_ms.unwrap() > crate::db::now_ms() + 100_000, "its Retry-After of 120 s");

        // The next request skips the cooling key without asking it.
        rig.fake.keys.lock().unwrap().clear();
        assert_eq!(rig.post("/v1/chat/completions", chat("main/ring")).await.status(), 200);
        assert_eq!(*rig.fake.keys.lock().unwrap(), ["key-1"]);

        // A rejected key is blocked until the user fixes it, and the ring goes on.
        let bad = rig.shared.with_db(|db| {
            let bad = keys::add(db, "main", "Bad", "provider-bad", true, crate::db::now_ms()).unwrap();
            for _ in 0..3 {
                keys::move_key(db, &bad.id, true).unwrap();
            }
            bad
        });
        rig.fake.keys.lock().unwrap().clear();
        assert_eq!(rig.post("/v1/chat/completions", chat("main/ring")).await.status(), 200);
        assert_eq!(*rig.fake.keys.lock().unwrap(), ["key-bad", "key-1"]);
        let blocked =
            rig.shared.with_db(|db| keys::blocking(db, &bad, "anything", crate::db::now_ms())).unwrap().unwrap();
        assert_eq!((blocked.state.as_str(), blocked.until_ms), ("invalid_credentials", None));
        assert!(rig.shared.with_db(|db| store::list_health(db, Some("main"))).unwrap().is_empty());
    });
}

#[test]
fn round_robin_spreads_new_requests_and_a_session_keeps_its_key() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let rig = rig().await;
        ring_model(&rig);
        rig.shared.with_db(|db| {
            keys::add(db, "main", "Second", "provider-k2", true, crate::db::now_ms()).unwrap();
            keys::set_strategy(db, "main", "round_robin").unwrap();
        });
        for _ in 0..4 {
            assert_eq!(rig.post("/v1/chat/completions", chat("main/ring")).await.status(), 200);
        }
        assert_eq!(*rig.fake.keys.lock().unwrap(), ["key-1", "key-2", "key-1", "key-2"]);
        rig.fake.keys.lock().unwrap().clear();
        for _ in 0..3 {
            assert_eq!(rig.post_session("/v1/chat/completions", "s-1", chat("main/ring")).await.status(), 200);
        }
        let used = rig.fake.keys.lock().unwrap().clone();
        assert!(used.iter().all(|k| *k == used[0]), "one session, one key (its account's prompt cache): {used:?}");
        // The ring's stated limits are kept per key, for the quota-aware order.
        let ring = rig.shared.with_db(|db| keys::list(db, "main")).unwrap();
        let windows = rig.shared.with_db(|db| keys::windows(db, &ring[0].id, "ring")).unwrap();
        assert_eq!(windows.iter().find(|w| w.kind == "requests").and_then(|w| w.remaining), Some(99));
    });
}
