//! The local gateway (ADR-004, ADR-016, ADR-025): one loopback base URL for every client the user already has.
//!
//! - Listens on 127.0.0.1 only, and refuses requests whose `Host` is not a loopback name, so a web page
//!   cannot reach it through DNS rebinding.
//! - Every request needs a client token (`Authorization: Bearer …` or `x-api-key`); tokens are stored hashed.
//! - A model name is either `<connection>/<model>` or the name of a combo, whose models are tried in order.
//! - **Same format is passed through** (ROADMAP P0-1): an Anthropic request to an Anthropic connection keeps
//!   its cache markers, thinking and every other field. Other pairs are translated only where tested.
//! - **Fallback is error-scoped** (P0-2, `policy.rs`): only before any byte reaches the client, never after
//!   an ambiguous failure, and never to a connection that shares the failed one's quota group after a
//!   credential, payment, permission or rate-limit error. Failing connections cool down.
//! - **Spend is reserved before sending** (P0-3): a client with a limit holds an estimate atomically, has a
//!   cap on requests in flight, and by default cannot use a target whose price is unknown. Handlers run
//!   detached, so a client that disconnects early still leaves a ledger entry.
//! - **Explicit sessions are pinned** (P0-4): a request with `x-smarttaskrouter-session` goes back to the
//!   model that last answered that session. Sessions are never guessed from prompt content.
//! - Every upstream attempt is written to the usage ledger with its cost and that cost's provenance.

pub mod anthropic;
pub mod catalog;
pub mod policy;
pub mod presets;
pub mod pricing;
pub mod signatures;
pub mod sse;
pub mod store;

use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::{self, Next as Middleware};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::StreamExt;
use policy::{Failure, Next};
use pricing::{Cost, Price, Usage};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::future::Future;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use store::{Client, Provider, Target, UsageRecord};
use tokio::sync::{mpsc, oneshot};

pub const DEFAULT_PORT: u16 = 8741;
/// The header a client sets to keep one session on one model (`ANTHROPIC_CUSTOM_HEADERS` in Claude Code).
pub const SESSION_HEADER: &str = "x-smarttaskrouter-session";
/// Output tokens assumed when a request sets no limit, for the reservation only.
const DEFAULT_OUTPUT_ESTIMATE: u64 = 4096;
const ANTHROPIC_VERSION: &str = "2023-06-01";

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
    /// Requests in flight per client id.
    inflight: Arc<Mutex<HashMap<String, u32>>>,
}

impl Shared {
    pub fn new(db: Arc<Mutex<Connection>>, secrets: Arc<dyn Secrets>) -> Shared {
        // Between bytes, not overall: a long answer may stream for many minutes, and a non-streamed reasoning
        // answer can take several before its first byte.
        Shared::with_read_timeout(db, secrets, Duration::from_secs(600))
    }

    pub fn with_read_timeout(db: Arc<Mutex<Connection>>, secrets: Arc<dyn Secrets>, read_timeout: Duration) -> Shared {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(read_timeout)
            .user_agent(concat!("SmartTaskRouter/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("HTTP client");
        Shared { db, secrets, http, signatures: Arc::default(), inflight: Arc::default() }
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

    fn enter(&self, client: &Client) -> Option<InflightGuard> {
        let mut map = self.inflight.lock().unwrap_or_else(|p| p.into_inner());
        let count = map.entry(client.id.clone()).or_insert(0);
        if *count >= client.max_concurrency {
            return None;
        }
        *count += 1;
        Some(InflightGuard { inflight: self.inflight.clone(), client_id: client.id.clone() })
    }
}

/// One request in flight for a client; leaving decrements the count.
pub struct InflightGuard {
    inflight: Arc<Mutex<HashMap<String, u32>>>,
    client_id: String,
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        let mut map = self.inflight.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(count) = map.get_mut(&self.client_id) {
            *count = count.saturating_sub(1);
        }
    }
}

/// Spend held for one attempt; released when the attempt is settled in the ledger, whatever happens.
pub struct Reservation {
    db: Arc<Mutex<Connection>>,
    id: String,
}

impl Drop for Reservation {
    fn drop(&mut self) {
        let conn = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let _ = store::release(&conn, &self.id);
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
    // Reservations a crash left behind would otherwise hold a client's limit forever.
    let _ = shared.with_db(|db| store::release_stale_reservations(db, 60 * 60 * 1000));
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
        .route("/v1/chat/completions", post(|s: State<Shared>, h: HeaderMap, b: Bytes| detached(chat_completions(s, h, b))))
        .route("/v1/messages", post(|s: State<Shared>, h: HeaderMap, b: Bytes| detached(anthropic::messages(s, h, b))))
        .route("/v1/messages/count_tokens", post(anthropic::count_tokens))
        .layer(DefaultBodyLimit::max(64 * 1024 * 1024))
        .layer(middleware::from_fn(move |request: Request, next: Middleware| loopback_host_only(request, next, port)))
        .with_state(shared)
}

/// Runs a handler in its own task. If the client disconnects, axum drops the handler's future; the task
/// keeps going, so an upstream call already under way finishes and is written to the ledger.
async fn detached<F>(handler: F) -> Response
where
    F: Future<Output = Response> + Send + 'static,
{
    match tokio::spawn(handler).await {
        Ok(response) => response,
        Err(_) => openai_error(StatusCode::INTERNAL_SERVER_ERROR, "api_error", "The gateway failed while handling the request"),
    }
}

async fn loopback_host_only(request: Request, next: Middleware, port: u16) -> Response {
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
    /// The provider's own response, passed through when the last target answered with an error and the
    /// client speaks the same format.
    pub upstream: Option<(StatusCode, Option<HeaderValue>, Bytes)>,
    /// Seconds until a cooled-down target may be tried again.
    pub retry_after: Option<u64>,
}

impl Refusal {
    fn new(status: StatusCode, kind: &'static str, message: impl Into<String>) -> Refusal {
        Refusal { status, kind, message: message.into(), upstream: None, retry_after: None }
    }

    pub fn openai(self) -> Response {
        let retry_after = self.retry_after;
        let mut response = match self.upstream {
            Some((status, content_type, body)) => passthrough(status, content_type, body),
            None => openai_error(self.status, self.kind, &self.message),
        };
        if let Some(seconds) = retry_after {
            response.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from(seconds));
        }
        response
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
            "Missing client token. Create one in SmartTaskRouter → Clients."));
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

/// The explicit session a request belongs to, if the client named one. Never derived from the prompt.
pub fn session_key(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(SESSION_HEADER)?.to_str().ok()?.trim();
    let ok = !value.is_empty() && value.len() <= 128 && value.chars().all(|c| c.is_ascii_graphic());
    ok.then(|| value.to_string())
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

/// The API a request arrived on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Surface {
    OpenAi,
    Anthropic,
}

/// How a request reached its target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    /// Same format: the body went through unchanged apart from the model name.
    Native,
    /// An Anthropic request translated to Chat Completions.
    AnthropicToOpenAi,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Native => "native",
            Mode::AnthropicToOpenAi => "translated:anthropic->openai",
        }
    }
}

pub struct Incoming<'a> {
    pub surface: Surface,
    pub body: &'a Value,
    pub headers: &'a HeaderMap,
    pub requested: &'a str,
    pub session: Option<&'a str>,
    pub stream: bool,
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
    pub mode: Mode,
    /// Cache hints the translation could not carry (empty for native).
    pub dropped: Vec<&'static str>,
    pub session: Option<String>,
    /// The session was already pinned to this target.
    pub pinned: bool,
    pub client_id: String,
    pub requested: String,
    /// Held until the attempt is settled in the ledger; dropping it releases the reserved spend.
    _reservation: Option<Reservation>,
    _inflight: InflightGuard,
}

const MAX_ERROR_BODY: usize = 64 * 1024;

fn estimate_micro(body: &Value, price: &Price) -> u64 {
    // Pessimistic on purpose: the serialized body overstates its tokens, and the output limit is the most
    // the provider may bill. Settled to the real figure as soon as usage arrives.
    let input_tokens = (body.to_string().len() / 4) as u64;
    let output_tokens = ["max_tokens", "max_completion_tokens", "max_output_tokens"]
        .iter()
        .find_map(|k| body[*k].as_u64())
        .unwrap_or(DEFAULT_OUTPUT_ESTIMATE);
    let per = |tokens: u64, micro: u64| ((tokens as u128 * micro as u128).div_ceil(1_000_000)) as u64;
    per(input_tokens, price.input_micro_per_mtok) + per(output_tokens, price.output_micro_per_mtok)
}

fn is_free(billing: &str) -> bool {
    matches!(billing, "free" | "local")
}

/// Sends the request to each target of `requested` in turn, until one starts a successful response. Each
/// failed attempt is recorded in the ledger as it happens; each skipped target says why.
pub async fn forward(shared: &Shared, client: &Client, incoming: &Incoming<'_>) -> Result<Forwarded, Refusal> {
    let Some(inflight) = shared.enter(client) else {
        return Err(Refusal { retry_after: Some(1), ..Refusal::new(StatusCode::TOO_MANY_REQUESTS, "rate_limit_error",
            format!("Client \"{}\" already has {} requests in flight, its limit.", client.name, client.max_concurrency)) });
    };
    let requested = incoming.requested;
    let mut targets = shared.with_db(|db| store::resolve(db, requested))
        .map_err(|e| Refusal::new(StatusCode::INTERNAL_SERVER_ERROR, "api_error", e))?;
    if targets.is_empty() {
        return Err(Refusal::new(StatusCode::NOT_FOUND, "model_not_found", format!(
            "No model or combo named \"{requested}\". Use <connection>/<model> or a combo name; GET /v1/models lists them.")));
    }
    // A pinned session goes back to the target that last answered it, while that target is still part of
    // the route; the others stay behind it as fallbacks.
    let mut pinned_target = None;
    if let Some(session) = incoming.session {
        if let Ok(Some(pin)) = shared.with_db(|db| store::pinned_target(db, &client.id, session, requested)) {
            if let Some(i) = targets.iter().position(|(p, m)| p.id == pin.provider_id && *m == pin.model_id) {
                let target = targets.remove(i);
                targets.insert(0, target);
                pinned_target = Some(pin);
            }
        }
    }

    let count = targets.len();
    let mut last: Option<Refusal> = None;
    let mut skipped: Vec<String> = Vec::new();
    // Quota group → the connection that refused. Its own other models may still answer (quotas are often per
    // model); other connections of the group may not, which would be using a sibling key to get around a limit.
    let mut blocked_groups: HashMap<String, String> = HashMap::new();
    let mut retry_at: Option<i64> = None;
    let has_limit = client.monthly_budget_micro_usd.is_some();

    for (index, (provider, model_id)) in targets.into_iter().enumerate() {
        let attempt = index + 1;
        let outcome_if_failed = if attempt < count { "fallback" } else { "error" };
        let name = format!("{}/{model_id}", provider.id);

        // Which mode would carry this request to this target, if any.
        let mode = match (incoming.surface, provider.wire.as_str()) {
            (Surface::OpenAi, "openai") | (Surface::Anthropic, "anthropic") => Mode::Native,
            (Surface::Anthropic, "openai") => Mode::AnthropicToOpenAi,
            _ => {
                skipped.push(format!("{name}: speaks the {} API, which this request's format cannot reach yet", provider.wire));
                continue;
            }
        };
        if blocked_groups.get(&provider.quota_group).is_some_and(|refused_by| *refused_by != provider.id) {
            skipped.push(format!("{name}: shares limits with a connection that just refused this request"));
            continue;
        }
        if let Ok(Some(health)) = shared.with_db(|db| store::blocking_health(db, &provider.id, &model_id)) {
            if let Some(until) = health.until_ms {
                retry_at = Some(retry_at.map_or(until, |r: i64| r.min(until)));
            }
            skipped.push(format!("{name}: {} ({})", health.state.replace('_', " "), health.reason));
            continue;
        }

        let (billing, price) = shared.with_db(|db| store::pricing_for(db, &provider, &model_id)).unwrap_or_default();
        let reservation = if is_free(&billing) || !has_limit {
            None
        } else if let Some(price) = price.as_ref() {
            let micro = estimate_micro(incoming.body, price);
            match shared.with_db(|db| store::reserve(db, &client.id, micro)) {
                Ok(Ok(id)) => Some(Reservation { db: shared.db.clone(), id }),
                Ok(Err(over)) => {
                    skipped.push(format!("{name}: up to ${:.4} would pass the monthly limit (${:.2} spent, ${:.2} held, ${:.2} limit)",
                        over.wanted as f64 / 1e6, over.spent as f64 / 1e6, over.reserved as f64 / 1e6, over.budget as f64 / 1e6));
                    continue;
                }
                Err(e) => return Err(Refusal::new(StatusCode::INTERNAL_SERVER_ERROR, "api_error", e)),
            }
        } else if client.unknown_cost_policy == "block" {
            skipped.push(format!("{name}: its price is unknown, and this client has a spending limit"));
            continue;
        } else {
            None
        };

        let started = Instant::now();
        let record_failure = |status: Option<u16>, cost: Cost, outcome: &str, error: &str| {
            let _ = shared.with_db(|db| store::record_usage(db, &UsageRecord {
                session_key: incoming.session, route_mode: Some(mode.label()),
                ..UsageRecord::failed(&client.id, &provider.id, &model_id, requested, &cost, outcome, status,
                    started.elapsed().as_millis() as u64, error)
            }));
        };
        let mut apply = |failure: &Failure, message: &str, provider: &Provider| -> Next {
            let decision = policy::decide(failure);
            if let Some(mark) = &decision.mark {
                let until = mark.cooldown.map(|d| db_now() + d.as_millis() as i64);
                let model = if mark.model_scope { model_id.as_str() } else { "" };
                let status = match failure { Failure::Http { status, .. } => Some(*status), _ => None };
                let _ = shared.with_db(|db| store::mark_health(db, &provider.id, model, mark.state, message, status, until));
            }
            if decision.next == Next::OtherGroups {
                blocked_groups.entry(provider.quota_group.clone()).or_insert_with(|| provider.id.clone());
            }
            decision.next
        };

        let key = match &provider.credential_ref {
            Some(reference) => match shared.secrets.read(reference) {
                Ok(Some(key)) => Some(key),
                Ok(None) => {
                    let message = format!("No key is stored for {}", provider.label);
                    record_failure(None, Cost::Estimated { micro_usd: 0, source: "not sent".into() }, outcome_if_failed, &message);
                    let _ = shared.with_db(|db| store::mark_health(db, &provider.id, "", "invalid_credentials", &message, None, None));
                    last = Some(Refusal::new(StatusCode::BAD_GATEWAY, "api_error", message));
                    continue;
                }
                Err(e) => {
                    let message = format!("Could not read the key for {}: {e}", provider.label);
                    record_failure(None, Cost::Estimated { micro_usd: 0, source: "not sent".into() }, outcome_if_failed, &message);
                    last = Some(Refusal::new(StatusCode::BAD_GATEWAY, "api_error", message));
                    continue;
                }
            },
            None => None,
        };

        let (url, mut body, dropped) = upstream_request(incoming, &provider, &model_id, mode);
        if provider.wire == "openai" {
            // Gemini needs the signatures of earlier tool calls back; others must not see the field.
            shared.signatures.lock().unwrap_or_else(|p| p.into_inner()).apply(&mut body, provider.kind == "gemini");
        }
        let mut request = shared.http.post(url).json(&body);
        if provider.wire == "anthropic" {
            if let Some(key) = &key {
                request = request.header("x-api-key", key);
            }
            let version = incoming.headers.get("anthropic-version").cloned()
                .unwrap_or_else(|| HeaderValue::from_static(ANTHROPIC_VERSION));
            request = request.header("anthropic-version", version);
            if let Some(beta) = incoming.headers.get("anthropic-beta") {
                request = request.header("anthropic-beta", beta.clone());
            }
        } else {
            if let Some(key) = &key {
                request = request.bearer_auth(key);
            }
        }

        match request.send().await {
            Err(e) => {
                let secrets: Vec<&str> = key.as_deref().into_iter().collect();
                let message = crate::redact::redact(&format!("{}: {e}", provider.label), &secrets);
                // A refused or failed connection never reached the provider. Anything else (a timeout while
                // waiting, a dropped connection) may have run the request: stop, and say the cost is unknown.
                let failure = if e.is_connect() { Failure::NotSent } else { Failure::Ambiguous };
                let cost = if failure == Failure::NotSent {
                    Cost::Estimated { micro_usd: 0, source: "could not connect".into() }
                } else {
                    pricing::resolve_failure(None)
                };
                let next = apply(&failure, &message, &provider);
                record_failure(None, cost, if next == Next::Stop { "error" } else { outcome_if_failed }, &message);
                if next == Next::Stop {
                    return Err(Refusal::new(StatusCode::GATEWAY_TIMEOUT, "api_error", format!(
                        "{message}. The provider may still have run the request, so no other model was tried.")));
                }
                last = Some(Refusal::new(StatusCode::BAD_GATEWAY, "api_error", message));
            }
            Ok(response) if response.status().is_success() => {
                let _ = shared.with_db(|db| store::mark_healthy(db, &provider.id, &model_id));
                let pinned = pinned_target.as_ref().is_some_and(|p| p.provider_id == provider.id && p.model_id == model_id);
                return Ok(Forwarded {
                    response, provider, model_id, billing, price, started, attempt, mode, dropped,
                    session: incoming.session.map(String::from), pinned, client_id: client.id.clone(),
                    requested: requested.to_string(), _reservation: reservation, _inflight: inflight,
                });
            }
            Ok(response) => {
                let status = response.status();
                let retry_after = policy::retry_after(response.headers());
                let content_type = response.headers().get(header::CONTENT_TYPE).cloned();
                let bytes = read_limited(response, MAX_ERROR_BODY).await;
                let secrets: Vec<&str> = key.as_deref().into_iter().collect();
                let detail = crate::redact::redact(&upstream_message(&bytes), &secrets);
                let message = format!("{} answered HTTP {}: {detail}", provider.label, status.as_u16());
                let next = apply(&Failure::Http { status: status.as_u16(), retry_after }, &message, &provider);
                record_failure(Some(status.as_u16()), pricing::resolve_failure(Some(status.as_u16())),
                    if next == Next::Stop { "error" } else { outcome_if_failed }, &message);
                // The provider's own body is only handed back to a client that speaks its format.
                let same_format = mode == Mode::Native;
                let refusal = Refusal { status, kind: "upstream_error", message,
                    upstream: same_format.then_some((status, content_type, bytes)),
                    retry_after: retry_after.map(|d| d.as_secs().max(1)) };
                if next == Next::Stop {
                    return Err(refusal);
                }
                last = Some(refusal);
            }
        }
    }

    // Nothing answered. Prefer the last provider error; otherwise explain why every target was skipped.
    if let Some(mut refusal) = last {
        if !skipped.is_empty() {
            refusal.message = format!("{} Also skipped: {}.", refusal.message, skipped.join("; "));
        }
        return Err(refusal);
    }
    let wait = retry_at.map(|at| ((at - db_now()).max(1000) as u64).div_ceil(1000));
    let over_budget = skipped.iter().any(|s| s.contains("monthly limit") || s.contains("spending limit"));
    let (status, kind) = if over_budget {
        (StatusCode::TOO_MANY_REQUESTS, "budget_exceeded")
    } else if wait.is_some() {
        (StatusCode::TOO_MANY_REQUESTS, "rate_limit_error")
    } else {
        (StatusCode::BAD_GATEWAY, "api_error")
    };
    Err(Refusal { retry_after: wait, ..Refusal::new(status, kind, format!("No target could take \"{requested}\": {}.",
        skipped.join("; "))) })
}

fn db_now() -> i64 {
    crate::db::now_ms()
}

/// The URL and body for one target, and which cache hints a translation had to drop.
fn upstream_request(incoming: &Incoming, provider: &Provider, model_id: &str, mode: Mode) -> (String, Value, Vec<&'static str>) {
    let accepts_cache = presets::find(&provider.kind).is_some_and(|p| p.accepts_cache_control);
    let (mut body, dropped) = match mode {
        Mode::Native => (incoming.body.clone(), Vec::new()),
        Mode::AnthropicToOpenAi => {
            let mut dropped = Vec::new();
            if anthropic::has_thinking(incoming.body) {
                dropped.push("thinking");
            }
            if !accepts_cache && anthropic::has_cache_control(incoming.body) {
                dropped.push("cache_control");
            }
            (anthropic::to_openai(incoming.body, accepts_cache), dropped)
        }
    };
    body["model"] = json!(model_id);
    if provider.wire == "openai" {
        if incoming.stream {
            // Ask for the usage chunk at the end of the stream, keeping any options the client set.
            if !body["stream_options"].is_object() {
                body["stream_options"] = json!({});
            }
            body["stream_options"]["include_usage"] = json!(true);
        }
        (format!("{}/chat/completions", provider.endpoint), body, dropped)
    } else {
        (format!("{}/messages", provider.endpoint), body, dropped)
    }
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

/// Ledger entry for a response that succeeded upstream; also pins an explicit session to this target and
/// releases the attempt's reservation (by consuming `forwarded`).
pub fn record_success(shared: &Shared, forwarded: Forwarded, usage: Option<&Usage>, outcome: &str, error: Option<&str>) {
    let cost = if outcome == "ok" {
        pricing::resolve(&forwarded.billing, forwarded.price.as_ref(), usage)
    } else {
        // Cut short: whatever usage arrived is partial, so the amount cannot be stated.
        match pricing::resolve(&forwarded.billing, forwarded.price.as_ref(), usage) {
            cost @ Cost::Estimated { micro_usd: 0, .. } => cost,
            _ => Cost::Unknown { source: "the response was cut short before usage was reported".into() },
        }
    };
    let _ = shared.with_db(|db| -> store::Result<()> {
        store::record_usage(db, &UsageRecord {
            client_id: Some(&forwarded.client_id), provider_id: &forwarded.provider.id, model_id: &forwarded.model_id,
            requested_model: &forwarded.requested, input_tokens: usage.map(|u| u.prompt),
            output_tokens: usage.map(|u| u.completion), cache_tokens: usage.map(|u| u.cached), cost: &cost, outcome,
            http_status: Some(forwarded.response.status().as_u16()),
            latency_ms: forwarded.started.elapsed().as_millis() as u64, error,
            cache_write_tokens: usage.map(|u| u.cache_write), session_key: forwarded.session.as_deref(),
            route_mode: Some(forwarded.mode.label()),
        })?;
        if let Some(session) = &forwarded.session {
            store::pin(db, &forwarded.client_id, session, &forwarded.requested,
                &Target { provider_id: forwarded.provider.id.clone(), model_id: forwarded.model_id.clone() })?;
        }
        Ok(())
    });
    drop(forwarded);
}

pub fn route_headers(response: &mut Response, forwarded: &Forwarded) {
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(&format!("{}/{}", forwarded.provider.id, forwarded.model_id)) {
        headers.insert("x-smarttaskrouter-route", value);
    }
    headers.insert("x-smarttaskrouter-attempts", HeaderValue::from(forwarded.attempt));
    if let Ok(value) = HeaderValue::from_str(forwarded.mode.label()) {
        headers.insert("x-smarttaskrouter-mode", value);
    }
    if !forwarded.dropped.is_empty() {
        if let Ok(value) = HeaderValue::from_str(&forwarded.dropped.join(",")) {
            headers.insert("x-smarttaskrouter-dropped", value);
        }
    }
    if forwarded.session.is_some() {
        headers.insert("x-smarttaskrouter-session",
            HeaderValue::from_static(if forwarded.pinned { "pinned" } else { "new" }));
    }
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
    let session = session_key(&headers);
    let incoming = Incoming { surface: Surface::OpenAi, body: &value, headers: &headers, requested: &requested,
        session: session.as_deref(), stream };
    let forwarded = match forward(&shared, &client, &incoming).await {
        Ok(forwarded) => forwarded,
        Err(refusal) => return refusal.openai(),
    };
    let status = forwarded.response.status();
    let content_type = forwarded.response.headers().get(header::CONTENT_TYPE).cloned();

    if !stream {
        let mut forwarded = forwarded;
        let response = std::mem::replace(&mut forwarded.response, status_only(status));
        return match response.bytes().await {
            Ok(bytes) => {
                let parsed = serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null);
                shared.capture_signatures(&parsed);
                let usage = Usage::from_json(&parsed["usage"]);
                let mut out = passthrough(status, content_type, bytes);
                route_headers(&mut out, &forwarded);
                record_success(&shared, forwarded, usage.as_ref(), "ok", None);
                out
            }
            Err(e) => {
                let message = crate::redact::redact(&e.to_string(), &[]);
                record_success(&shared, forwarded, None, "stream_error", Some(&message));
                openai_error(StatusCode::BAD_GATEWAY, "api_error", &format!("The provider's response broke off: {message}"))
            }
        };
    }

    // Streaming: forward each chunk as it arrives, watching for the usage chunk on the way.
    let (tx, rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(64);
    let mut forwarded = forwarded;
    let upstream = std::mem::replace(&mut forwarded.response, status_only(status));
    let mut out = stream_response(rx, status, content_type);
    route_headers(&mut out, &forwarded);
    tokio::spawn(async move {
        let mut scanner = sse::UsageScanner::default();
        let (outcome, error) = pump(upstream, &tx, |chunk| {
            scanner.feed(chunk);
            for event in scanner.take_tool_events() {
                shared.capture_signatures(&event);
            }
        }).await;
        record_success(&shared, forwarded, scanner.usage.as_ref(), outcome, error.as_deref());
    });
    out
}

/// A streamed response whose body is fed from `rx`.
pub fn stream_response(rx: mpsc::Receiver<Result<Bytes, std::io::Error>>, status: StatusCode,
    content_type: Option<HeaderValue>) -> Response {
    let mut out = Response::new(Body::from_stream(futures_util::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    })));
    *out.status_mut() = status;
    out.headers_mut().insert(header::CONTENT_TYPE,
        content_type.unwrap_or_else(|| HeaderValue::from_static("text/event-stream")));
    out.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    out
}

/// Copies an upstream body to the client unchanged, showing each chunk to `watch` first. Returns the
/// outcome for the ledger.
pub async fn pump(upstream: reqwest::Response, tx: &mpsc::Sender<Result<Bytes, std::io::Error>>,
    mut watch: impl FnMut(&[u8])) -> (&'static str, Option<String>) {
    let mut bytes = upstream.bytes_stream();
    while let Some(chunk) = bytes.next().await {
        match chunk {
            Ok(chunk) => {
                watch(&chunk);
                if tx.send(Ok(chunk)).await.is_err() {
                    return ("client_disconnected", None);
                }
            }
            Err(e) => {
                let _ = tx.send(Err(std::io::Error::other("upstream stream failed"))).await;
                return ("stream_error", Some(crate::redact::redact(&e.to_string(), &[])));
            }
        }
    }
    ("ok", None)
}

/// `reqwest::Response` has no public constructor; this stands in once the body has been taken.
pub fn status_only(status: StatusCode) -> reqwest::Response {
    let mut response = axum::http::Response::new(Vec::<u8>::new());
    *response.status_mut() = status;
    reqwest::Response::from(response)
}

#[cfg(test)]
mod tests;
