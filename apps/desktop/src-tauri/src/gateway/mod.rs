//! The local gateway (ADR-004, ADR-016): one loopback base URL for every client the user already has.
//!
//! - Listens on 127.0.0.1 only, and refuses requests whose `Host` is not a loopback name, so a web page
//!   cannot reach it through DNS rebinding.
//! - Every request needs a client token (`Authorization: Bearer …` or `x-api-key`); tokens are stored hashed.
//! - A model name is either `<connection>/<model>` or the name of a combo, whose models are tried in order.
//!   The next one is tried only on a hard error (auth, quota, unavailable model, server error) and only
//!   before any byte has been sent to the client, so a response is never stitched from two models (ADR-003).
//! - Every upstream attempt is written to the usage ledger with its cost and that cost's provenance.

pub mod anthropic;
pub mod catalog;
pub mod presets;
pub mod pricing;
pub mod signatures;
pub mod sse;
pub mod store;

use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::StreamExt;
use pricing::{Cost, Price, Usage};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use store::{Client, Provider, UsageRecord};
use tokio::sync::{mpsc, oneshot};

pub const DEFAULT_PORT: u16 = 8741;

/// Where connection keys come from; the OS credential store in the app, a map in tests.
pub trait Secrets: Send + Sync + 'static {
    fn read(&self, reference: &str) -> Result<Option<String>, String>;
}

pub struct OsSecrets;

impl Secrets for OsSecrets {
    fn read(&self, reference: &str) -> Result<Option<String>, String> {
        crate::vault::read(reference).map_err(|e| e.to_string())
    }
}

#[derive(Clone)]
pub struct Shared {
    pub db: Arc<Mutex<Connection>>,
    pub secrets: Arc<dyn Secrets>,
    pub http: reqwest::Client,
    pub signatures: Arc<Mutex<signatures::SignatureCache>>,
}

impl Shared {
    pub fn new(db: Arc<Mutex<Connection>>, secrets: Arc<dyn Secrets>) -> Shared {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            // Between bytes, not overall: a long answer may stream for many minutes.
            .read_timeout(Duration::from_secs(300))
            .user_agent(concat!("SmartTaskRouter/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("HTTP client");
        Shared { db, secrets, http, signatures: Arc::default() }
    }

    /// Runs `f` with the database. Never held across an await.
    pub fn with_db<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
        let conn = self.db.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&conn)
    }

    /// Remembers Gemini tool-call signatures in a response or stream chunk.
    pub fn capture_signatures(&self, value: &Value) {
        self.signatures.lock().unwrap_or_else(|p| p.into_inner()).capture(value);
    }
}

pub struct Server {
    pub addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}

impl Server {
    /// Stops accepting requests, gives open responses a few seconds to finish, then ends them.
    pub async fn stop(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if tokio::time::timeout(Duration::from_secs(5), &mut self.task).await.is_err() {
            self.task.abort();
        }
    }
}

pub async fn start(shared: Shared, port: u16) -> std::io::Result<Server> {
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
    let addr = listener.local_addr()?;
    let app = router(shared, addr.port());
    let (tx, rx) = oneshot::channel::<()>();
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, app).with_graceful_shutdown(async { let _ = rx.await; }).await;
    });
    Ok(Server { addr, shutdown: Some(tx), task })
}

pub fn router(shared: Shared, port: u16) -> Router {
    Router::new()
        .route("/health", get(|| async { Json(json!({ "ok": true, "app": "SmartTaskRouter" })) }))
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/messages", post(anthropic::messages))
        .route("/v1/messages/count_tokens", post(anthropic::count_tokens))
        .layer(DefaultBodyLimit::max(64 * 1024 * 1024))
        .layer(middleware::from_fn(move |request: Request, next: Next| loopback_host_only(request, next, port)))
        .with_state(shared)
}

async fn loopback_host_only(request: Request, next: Next, port: u16) -> Response {
    let host = request.headers().get(header::HOST).and_then(|h| h.to_str().ok()).unwrap_or("");
    let allowed = ["127.0.0.1", "localhost", "[::1]"]
        .iter()
        .any(|name| host == *name || host == format!("{name}:{port}"));
    if allowed {
        next.run(request).await
    } else {
        openai_error(StatusCode::FORBIDDEN, "permission_error", "This gateway only answers requests addressed to 127.0.0.1")
    }
}

pub fn openai_error(status: StatusCode, kind: &str, message: &str) -> Response {
    (status, Json(json!({ "error": { "message": message, "type": kind, "code": kind } }))).into_response()
}

/// A failure before anything was forwarded, in a neutral form each API surface renders its own way.
pub struct Refusal {
    pub status: StatusCode,
    pub kind: &'static str,
    pub message: String,
    /// The provider's own response, passed through when the last target answered with an error.
    pub upstream: Option<(StatusCode, Option<HeaderValue>, Bytes)>,
}

impl Refusal {
    fn new(status: StatusCode, kind: &'static str, message: impl Into<String>) -> Refusal {
        Refusal { status, kind, message: message.into(), upstream: None }
    }

    pub fn openai(self) -> Response {
        match self.upstream {
            Some((status, content_type, body)) => passthrough(status, content_type, body),
            None => openai_error(self.status, self.kind, &self.message),
        }
    }
}

fn passthrough(status: StatusCode, content_type: Option<HeaderValue>, body: Bytes) -> Response {
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    if let Some(content_type) = content_type {
        response.headers_mut().insert(header::CONTENT_TYPE, content_type);
    }
    response
}

pub fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer ")))
        .or_else(|| headers.get("x-api-key").and_then(|v| v.to_str().ok()))
        .map(str::trim)
}

pub fn authenticate(shared: &Shared, headers: &HeaderMap) -> Result<Client, Refusal> {
    let Some(token) = bearer(headers) else {
        return Err(Refusal::new(StatusCode::UNAUTHORIZED, "authentication_error",
            "Missing client token. Create one in SmartTaskRouter → Endpoints."));
    };
    let client = shared.with_db(|db| store::authenticate(db, token)).map_err(|e| {
        Refusal::new(StatusCode::INTERNAL_SERVER_ERROR, "api_error", e)
    })?;
    let Some(client) = client else {
        return Err(Refusal::new(StatusCode::UNAUTHORIZED, "authentication_error", "Unknown or revoked client token"));
    };
    if let Some(budget) = client.monthly_budget_micro_usd {
        if client.month_spend_micro_usd >= budget {
            return Err(Refusal::new(StatusCode::TOO_MANY_REQUESTS, "budget_exceeded", format!(
                "Client \"{}\" has reached its monthly limit (${:.2}).", client.name, budget as f64 / 1e6)));
        }
    }
    Ok(client)
}

async fn list_models(State(shared): State<Shared>, headers: HeaderMap) -> Response {
    if let Err(refusal) = authenticate(&shared, &headers) {
        return refusal.openai();
    }
    let listed = shared.with_db(|db| -> store::Result<Vec<Value>> {
        let mut data: Vec<Value> = store::list_combos(db)?
            .into_iter()
            .map(|c| json!({ "id": c.name, "object": "model", "created": 0, "owned_by": "smarttaskrouter combo" }))
            .collect();
        let providers = store::list_providers(db)?;
        for model in store::list_models(db, None)? {
            let enabled = providers.iter().any(|p| p.id == model.provider_id && p.enabled);
            if model.enabled && enabled {
                data.push(json!({ "id": format!("{}/{}", model.provider_id, model.model_id), "object": "model",
                    "created": 0, "owned_by": model.provider_id }));
            }
        }
        Ok(data)
    });
    match listed {
        Ok(data) => Json(json!({ "object": "list", "data": data })).into_response(),
        Err(e) => openai_error(StatusCode::INTERNAL_SERVER_ERROR, "api_error", &e),
    }
}

/// An upstream response that started successfully, with what is needed to account for it.
pub struct Forwarded {
    pub response: reqwest::Response,
    pub provider: Provider,
    pub model_id: String,
    pub billing: String,
    pub price: Option<Price>,
    pub started: Instant,
    /// How many targets were tried, this one included.
    pub attempt: usize,
}

/// HTTP statuses after which the next model of a combo is tried: the request may well succeed elsewhere.
fn worth_falling_back(status: StatusCode) -> bool {
    matches!(status.as_u16(), 401 | 402 | 403 | 404 | 408 | 409 | 425 | 429) || status.is_server_error()
}

const MAX_ERROR_BODY: usize = 64 * 1024;

/// Sends an OpenAI chat completions body to each target of `requested` in turn, until one starts a
/// successful response. Failed attempts are recorded in the ledger as they happen.
pub async fn forward(shared: &Shared, client: &Client, requested: &str, mut body: Value) -> Result<Forwarded, Refusal> {
    let targets = shared.with_db(|db| store::resolve(db, requested))
        .map_err(|e| Refusal::new(StatusCode::INTERNAL_SERVER_ERROR, "api_error", e))?;
    if targets.is_empty() {
        return Err(Refusal::new(StatusCode::NOT_FOUND, "model_not_found", format!(
            "No model or combo named \"{requested}\". Use <connection>/<model> or a combo name; GET /v1/models lists them.")));
    }
    if body.get("stream").and_then(Value::as_bool) == Some(true) {
        // Ask for the usage chunk at the end of the stream, keeping any options the client set.
        if !body["stream_options"].is_object() {
            body["stream_options"] = json!({});
        }
        body["stream_options"]["include_usage"] = json!(true);
    }

    let count = targets.len();
    let mut last: Option<Refusal> = None;
    for (index, (provider, model_id)) in targets.into_iter().enumerate() {
        let attempt = index + 1;
        let outcome_if_failed = if attempt < count { "fallback" } else { "error" };
        let (billing, price) = shared.with_db(|db| store::pricing_for(db, &provider, &model_id)).unwrap_or_default();
        let started = Instant::now();
        let record_failure = |status: Option<u16>, cost: Cost, error: &str| {
            let _ = shared.with_db(|db| store::record_usage(db, &UsageRecord {
                client_id: Some(&client.id), provider_id: &provider.id, model_id: &model_id, requested_model: requested,
                input_tokens: None, output_tokens: None, cache_tokens: None, cost: &cost, outcome: outcome_if_failed,
                http_status: status, latency_ms: started.elapsed().as_millis() as u64, error: Some(error),
            }));
        };

        let key = match &provider.credential_ref {
            Some(reference) => match shared.secrets.read(reference) {
                Ok(Some(key)) => Some(key),
                Ok(None) => {
                    let message = format!("No key is stored for {}", provider.label);
                    record_failure(None, Cost::Estimated { micro_usd: 0, source: "not sent".into() }, &message);
                    last = Some(Refusal::new(StatusCode::BAD_GATEWAY, "api_error", message));
                    continue;
                }
                Err(e) => {
                    let message = format!("Could not read the key for {}: {e}", provider.label);
                    record_failure(None, Cost::Estimated { micro_usd: 0, source: "not sent".into() }, &message);
                    last = Some(Refusal::new(StatusCode::BAD_GATEWAY, "api_error", message));
                    continue;
                }
            },
            None => None,
        };

        let mut body = body.clone();
        body["model"] = json!(model_id);
        shared.signatures.lock().unwrap_or_else(|p| p.into_inner()).apply(&mut body, provider.kind == "gemini");
        let mut request = shared.http.post(format!("{}/chat/completions", provider.endpoint)).json(&body);
        if let Some(key) = &key {
            request = request.bearer_auth(key);
        }
        match request.send().await {
            Err(e) => {
                let message = crate::redact::redact(&format!("{}: {e}", provider.label), &[]);
                let cost = if e.is_connect() {
                    Cost::Estimated { micro_usd: 0, source: "could not connect".into() }
                } else {
                    pricing::resolve_failure(None)
                };
                record_failure(None, cost, &message);
                last = Some(Refusal::new(StatusCode::BAD_GATEWAY, "api_error", message));
            }
            Ok(response) if response.status().is_success() => {
                return Ok(Forwarded { response, provider, model_id, billing, price, started, attempt });
            }
            Ok(response) => {
                let status = response.status();
                let content_type = response.headers().get(header::CONTENT_TYPE).cloned();
                let bytes = read_limited(response, MAX_ERROR_BODY).await;
                let secrets: Vec<&str> = key.as_deref().into_iter().collect();
                let detail = crate::redact::redact(&upstream_message(&bytes), &secrets);
                let message = format!("{} answered HTTP {}: {detail}", provider.label, status.as_u16());
                let fallback = worth_falling_back(status);
                let _ = shared.with_db(|db| store::record_usage(db, &UsageRecord {
                    client_id: Some(&client.id), provider_id: &provider.id, model_id: &model_id, requested_model: requested,
                    input_tokens: None, output_tokens: None, cache_tokens: None,
                    cost: &pricing::resolve_failure(Some(status.as_u16())),
                    outcome: if fallback { outcome_if_failed } else { "error" },
                    http_status: Some(status.as_u16()), latency_ms: started.elapsed().as_millis() as u64,
                    error: Some(&message),
                }));
                let refusal = Refusal { status, kind: "upstream_error", message, upstream: Some((status, content_type, bytes)) };
                if !fallback {
                    // A malformed request fails the same way everywhere; hand the provider's answer back.
                    return Err(refusal);
                }
                last = Some(refusal);
            }
        }
    }
    Err(last.unwrap_or_else(|| Refusal::new(StatusCode::BAD_GATEWAY, "api_error", "No target answered")))
}

async fn read_limited(response: reqwest::Response, limit: usize) -> Bytes {
    let mut stream = response.bytes_stream();
    let mut out = Vec::new();
    while let Some(Ok(chunk)) = stream.next().await {
        let room = limit.saturating_sub(out.len());
        out.extend_from_slice(&chunk[..chunk.len().min(room)]);
        if out.len() >= limit {
            break;
        }
    }
    Bytes::from(out)
}

/// The human part of a provider's error body.
pub(crate) fn upstream_message(body: &[u8]) -> String {
    let text = String::from_utf8_lossy(body);
    let parsed: Option<Value> = serde_json::from_str(&text).ok();
    let message = parsed.as_ref().and_then(|v| {
        let v = if v.is_array() { &v[0] } else { v };
        v["error"]["message"].as_str().or_else(|| v["message"].as_str()).or_else(|| v["error"].as_str()).map(String::from)
    });
    let message = message.unwrap_or_else(|| text.trim().to_string());
    message.chars().take(300).collect()
}

/// Ledger entry for a response that succeeded upstream.
pub fn record_success(shared: &Shared, client: &Client, requested: &str, forwarded: &Forwarded, usage: Option<&Usage>,
    outcome: &str, error: Option<&str>) {
    let cost = if outcome == "ok" {
        pricing::resolve(&forwarded.billing, forwarded.price.as_ref(), usage)
    } else {
        // Cut short: whatever usage arrived is partial, so the amount cannot be stated.
        match pricing::resolve(&forwarded.billing, forwarded.price.as_ref(), usage) {
            cost @ Cost::Estimated { micro_usd: 0, .. } => cost,
            _ => Cost::Unknown { source: "the response was cut short before usage was reported".into() },
        }
    };
    let _ = shared.with_db(|db| store::record_usage(db, &UsageRecord {
        client_id: Some(&client.id), provider_id: &forwarded.provider.id, model_id: &forwarded.model_id,
        requested_model: requested, input_tokens: usage.map(|u| u.prompt), output_tokens: usage.map(|u| u.completion),
        cache_tokens: usage.map(|u| u.cached), cost: &cost, outcome, http_status: Some(forwarded.response.status().as_u16()),
        latency_ms: forwarded.started.elapsed().as_millis() as u64, error,
    }));
}

pub fn route_headers(response: &mut Response, forwarded: &Forwarded) {
    let route = format!("{}/{}", forwarded.provider.id, forwarded.model_id);
    if let Ok(value) = HeaderValue::from_str(&route) {
        response.headers_mut().insert("x-smarttaskrouter-route", value);
    }
    response.headers_mut().insert("x-smarttaskrouter-attempts", HeaderValue::from(forwarded.attempt));
}

pub fn parse_body(body: &Bytes) -> Result<(Value, String), Refusal> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| Refusal::new(StatusCode::BAD_REQUEST, "invalid_request_error", format!("The body is not JSON: {e}")))?;
    let model = value.get("model").and_then(Value::as_str).map(str::to_string).ok_or_else(|| {
        Refusal::new(StatusCode::BAD_REQUEST, "invalid_request_error", "The request has no \"model\"")
    })?;
    Ok((value, model))
}

async fn chat_completions(State(shared): State<Shared>, headers: HeaderMap, body: Bytes) -> Response {
    let client = match authenticate(&shared, &headers) {
        Ok(client) => client,
        Err(refusal) => return refusal.openai(),
    };
    let (value, requested) = match parse_body(&body) {
        Ok(parsed) => parsed,
        Err(refusal) => return refusal.openai(),
    };
    let stream = value.get("stream").and_then(Value::as_bool) == Some(true);
    let forwarded = match forward(&shared, &client, &requested, value).await {
        Ok(forwarded) => forwarded,
        Err(refusal) => return refusal.openai(),
    };
    let status = forwarded.response.status();
    let content_type = forwarded.response.headers().get(header::CONTENT_TYPE).cloned();

    if !stream {
        let mut forwarded = forwarded;
        let response = std::mem::replace(&mut forwarded.response, placeholder_response());
        return match response.bytes().await {
            Ok(bytes) => {
                let parsed = serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null);
                shared.capture_signatures(&parsed);
                let usage = Usage::from_json(&parsed["usage"]);
                forwarded.response = status_only(status);
                record_success(&shared, &client, &requested, &forwarded, usage.as_ref(), "ok", None);
                let mut out = passthrough(status, content_type, bytes);
                route_headers(&mut out, &forwarded);
                out
            }
            Err(e) => {
                forwarded.response = status_only(status);
                let message = crate::redact::redact(&e.to_string(), &[]);
                record_success(&shared, &client, &requested, &forwarded, None, "stream_error", Some(&message));
                openai_error(StatusCode::BAD_GATEWAY, "api_error", &format!("The provider's response broke off: {message}"))
            }
        };
    }

    // Streaming: forward each chunk as it arrives, watching for the usage chunk on the way.
    let (tx, rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(64);
    let mut forwarded = forwarded;
    let upstream = std::mem::replace(&mut forwarded.response, status_only(status));
    let mut out = Response::new(Body::from_stream(futures_util::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    })));
    *out.status_mut() = status;
    out.headers_mut().insert(header::CONTENT_TYPE,
        content_type.unwrap_or_else(|| HeaderValue::from_static("text/event-stream")));
    out.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    route_headers(&mut out, &forwarded);
    tokio::spawn(async move {
        let mut scanner = sse::UsageScanner::default();
        let mut bytes = upstream.bytes_stream();
        let mut outcome = "ok";
        let mut error = None;
        while let Some(chunk) = bytes.next().await {
            match chunk {
                Ok(chunk) => {
                    scanner.feed(&chunk);
                    for event in scanner.take_tool_events() {
                        shared.capture_signatures(&event);
                    }
                    if tx.send(Ok(chunk)).await.is_err() {
                        outcome = "client_disconnected";
                        break;
                    }
                }
                Err(e) => {
                    outcome = "stream_error";
                    error = Some(crate::redact::redact(&e.to_string(), &[]));
                    let _ = tx.send(Err(std::io::Error::other("upstream stream failed"))).await;
                    break;
                }
            }
        }
        record_success(&shared, &client, &requested, &forwarded, scanner.usage.as_ref(), outcome, error.as_deref());
    });
    out
}

/// `reqwest::Response` has no public constructor; this stands in once the body has been taken.
pub fn status_only(status: StatusCode) -> reqwest::Response {
    let mut response = axum::http::Response::new(Vec::<u8>::new());
    *response.status_mut() = status;
    reqwest::Response::from(response)
}

fn placeholder_response() -> reqwest::Response {
    status_only(StatusCode::OK)
}

#[cfg(test)]
mod tests;
