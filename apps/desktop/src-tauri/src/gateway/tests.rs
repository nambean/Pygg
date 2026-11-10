//! End-to-end tests: a real gateway on a loopback port in front of a fake OpenAI-compatible provider.

use super::pricing::Price;
use super::store::{self, NewProvider, Target};
use super::*;
use axum::extract::Json as JsonBody;
use std::collections::HashMap;

struct MapSecrets(HashMap<String, String>);

impl Secrets for MapSecrets {
    fn read(&self, reference: &str) -> Result<Option<String>, String> {
        Ok(self.0.get(reference).cloned())
    }
}

/// The fake provider. The model name picks the behaviour: `limited` answers 429, `broken` 400, `down` 503;
/// anything else answers "pong", streamed when asked, with usage.
async fn fake_provider(headers: HeaderMap, JsonBody(body): JsonBody<Value>) -> Response {
    if headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) != Some("Bearer key-1") {
        return (StatusCode::UNAUTHORIZED, Json(json!({ "error": { "message": "bad key" } }))).into_response();
    }
    let model = body["model"].as_str().unwrap_or("");
    match model {
        "limited" => return (StatusCode::TOO_MANY_REQUESTS, Json(json!({ "error": { "message": "quota" } }))).into_response(),
        "broken" => return (StatusCode::BAD_REQUEST, Json(json!({ "error": { "message": "bad field" } }))).into_response(),
        "down" => return (StatusCode::SERVICE_UNAVAILABLE, "overloaded").into_response(),
        _ => {}
    }
    let usage = json!({ "prompt_tokens": 1000, "completion_tokens": 500 });
    if body["stream"] == true {
        assert_eq!(body["stream_options"]["include_usage"], true, "the gateway asks for usage");
        let wants_tool = model == "tooler";
        let mut sse = String::new();
        if wants_tool {
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
        "choices": [{ "index": 0, "finish_reason": "stop", "message": { "role": "assistant", "content": "pong" } }],
        "usage": usage })).into_response()
}

struct Rig {
    base: String,
    token: String,
    shared: Shared,
    http: reqwest::Client,
    _server: Server,
}

async fn rig() -> Rig {
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(upstream, Router::new().route("/v1/chat/completions", post(fake_provider))).await.unwrap();
    });

    let conn = crate::db::open_in_memory().unwrap();
    let endpoint = format!("http://{upstream_addr}/v1");
    let main = store::add_provider(&conn, &NewProvider { kind: "custom", label: "Main", endpoint: &endpoint,
        credential_ref: Some("provider-main"), default_billing: "paid" }).unwrap();
    let backup = store::add_provider(&conn, &NewProvider { kind: "custom", label: "Backup", endpoint: &endpoint,
        credential_ref: Some("provider-main"), default_billing: "free" }).unwrap();
    store::add_model(&conn, &main.id, "fast").unwrap();
    // $0.10 in, $0.40 out per million tokens: 1000 in + 500 out = $0.0001 + $0.0002.
    store::update_model(&conn, &main.id, "fast", None, None, Some(Some(Price { input_micro_per_mtok: 100_000,
        output_micro_per_mtok: 400_000, cached_input_micro_per_mtok: None, source: "user".into(), observed_at: 0 }))).unwrap();
    store::save_combo(&conn, None, "coding", &[
        Target { provider_id: main.id.clone(), model_id: "limited".into() },
        Target { provider_id: main.id.clone(), model_id: "down".into() },
        Target { provider_id: backup.id.clone(), model_id: "steady".into() },
    ]).unwrap();
    let (_, token) = store::create_client(&conn, "Tests", None).unwrap();

    let secrets = MapSecrets(HashMap::from([("provider-main".to_string(), "key-1".to_string())]));
    let shared = Shared::new(Arc::new(Mutex::new(conn)), Arc::new(secrets));
    let server = start(shared.clone(), 0).await.unwrap();
    Rig { base: format!("http://{}", server.addr), token, shared, http: reqwest::Client::new(), _server: server }
}

impl Rig {
    async fn post(&self, path: &str, body: Value) -> reqwest::Response {
        self.http.post(format!("{}{path}", self.base)).bearer_auth(&self.token).json(&body).send().await.unwrap()
    }

    fn ledger(&self) -> Vec<store::UsageEvent> {
        let mut events = self.shared.with_db(|db| store::usage_summary(db, 0, 100)).unwrap().recent;
        events.reverse();
        events
    }
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap()
}

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
        let response = rig.post("/v1/chat/completions", json!({ "model": "main/fast",
            "messages": [{ "role": "user", "content": "ping" }] })).await;
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["x-smarttaskrouter-route"], "main/fast");
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["choices"][0]["message"]["content"], "pong");
        let event = &rig.ledger()[0];
        assert_eq!((event.cost_status.as_str(), event.cost_micro_usd), ("estimated", Some(300)));
        assert_eq!((event.input_tokens, event.output_tokens), (Some(1000), Some(500)));

        // An unpriced model of a paid connection is unknown, not free.
        rig.post("/v1/chat/completions", json!({ "model": "main/other", "messages": [] })).await;
        let event = rig.ledger().pop().unwrap();
        assert_eq!((event.cost_status.as_str(), event.cost_micro_usd), ("unknown", None));
    });
}

#[test]
fn a_combo_falls_back_on_hard_errors_and_records_each_attempt() {
    rt().block_on(async {
        let rig = rig().await;
        let response = rig.post("/v1/chat/completions", json!({ "model": "coding", "messages": [] })).await;
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["x-smarttaskrouter-attempts"], "3");
        assert_eq!(response.headers()["x-smarttaskrouter-route"], "backup/steady");
        let outcomes: Vec<(String, Option<u16>, String)> =
            rig.ledger().into_iter().map(|e| (e.outcome, e.http_status, e.cost_status)).collect();
        assert_eq!(outcomes, [
            ("fallback".into(), Some(429), "estimated".into()),
            ("fallback".into(), Some(503), "unknown".into()),
            ("ok".into(), Some(200), "estimated".into()),
        ]);
    });
}

#[test]
fn a_bad_request_is_returned_not_retried() {
    rt().block_on(async {
        let rig = rig().await;
        let response = rig.post("/v1/chat/completions", json!({ "model": "main/broken", "messages": [] })).await;
        assert_eq!(response.status(), 400);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["error"]["message"], "bad field");
        let missing = rig.post("/v1/chat/completions", json!({ "model": "nope", "messages": [] })).await;
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

#[test]
fn a_client_over_its_monthly_limit_is_refused() {
    rt().block_on(async {
        let rig = rig().await;
        rig.post("/v1/chat/completions", json!({ "model": "main/fast", "messages": [] })).await;
        let id = rig.shared.with_db(|db| store::list_clients(db)).unwrap()[0].id.clone();
        rig.shared.with_db(|db| store::set_client_budget(db, &id, Some(300))).unwrap();
        let refused = rig.post("/v1/chat/completions", json!({ "model": "main/fast", "messages": [] })).await;
        assert_eq!(refused.status(), 429);
    });
}

#[test]
fn anthropic_messages_round_trip() {
    rt().block_on(async {
        let rig = rig().await;
        let response = rig.http.post(format!("{}/v1/messages", rig.base)).header("x-api-key", &rig.token)
            .json(&json!({ "model": "coding", "max_tokens": 64, "messages": [{ "role": "user", "content": "ping" }] }))
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
