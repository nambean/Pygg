//! The local gateway (ADR-004, ADR-016, ADR-025): one loopback base URL for every client the user already has.
//!
//! - Listens on 127.0.0.1 only, and refuses requests whose `Host` is not a loopback name, so a web page
//!   cannot reach it through DNS rebinding.
//! - Every request needs a client token (`Authorization: Bearer …` or `x-api-key`); tokens are stored hashed.
//! - A model name is either `<connection>/<model>` or the name of a combo, whose models are tried in order.
//! - **Same format is passed through** (ROADMAP P0-1): an Anthropic request to an Anthropic connection keeps
//!   its cache markers, thinking and every other field (a semantic passthrough: the JSON is rewritten with
//!   only `model` changed, in the same key order). Other pairs are translated only where tested.
//! - **Fallback is error-scoped** (P0-2, `policy.rs`): only before any byte reaches the client, never after
//!   an ambiguous failure, and never to a connection that shares the failed one's quota group after a
//!   credential, payment, permission or rate-limit error, for as long as that refusal lasts. Failing
//!   connections cool down for as long as the provider says.
//! - **Every attempt is journaled before it is sent** (P0-3, counter-review CR-06) and settled in one
//!   transaction with its ledger entry. A client with a limit holds an *estimate* of the attempt's cost, so
//!   concurrent requests cannot pass the limit together; the estimate is not a bound, and the provider's bill
//!   can be higher (CR-03). Spend that may have been charged but cannot be stated counts against the limit at
//!   the amount held. Clients have a cap on requests in flight and by default cannot use a target whose price
//!   is unknown. Handlers run detached, so a client that disconnects early still leaves a ledger entry.
//! - **Explicit sessions are pinned** (P0-4): a request with `x-smarttaskrouter-session` goes back to the
//!   model that last answered that session. Sessions are never guessed from prompt content.
//! - Every upstream attempt is written to the usage ledger with its cost and that cost's provenance.

pub mod anthropic;
pub mod auto;
pub mod catalog;
pub mod openai_to_anthropic;
pub mod policy;
pub mod presets;
pub mod model_test;
pub mod savings;
pub mod oauth;
pub mod probe;
pub mod quota;
pub mod pricing;
pub mod reference;
pub mod responses;
pub mod signatures;
pub mod shadow;
pub mod sse;
pub mod store;
pub mod task_api;
pub mod tiers;

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
use std::sync::{Arc, Mutex, Weak};
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
    /// Ledger health (counter-review CR-06, review FR-03).
    accounting: Arc<Mutex<Accounting>>,
    /// The store file, when there is one. A gateway serving it must own it alone (review PR-01).
    store_path: Option<std::path::PathBuf>,
    /// The current ownership of the store, if this process holds it (review LC-02). Weak: the server, every
    /// attempt in flight and every pending settlement hold it strongly, and the store is released only when
    /// the last of them is gone.
    owner: Arc<Mutex<Weak<Owner>>>,
    /// The app's task runner, for the task API (task_api.rs). `None` in `gateway serve`: tasks run in the app.
    pub tasks: Option<crate::tasks::Runner>,
    /// The app's web-session endpoint (ADR-035, `websession`). `None` in `gateway serve`: no window can be driven there.
    pub web: Option<crate::websession::Link>,
}

/// Ownership of the store by this process (reviews PR-01, LC-01, LC-02). Held by the running server, by every
/// journaled attempt until it is settled, and by every settlement waiting for the store. While any of them is
/// alive the store's lock stays taken, so a stopped gateway still finishing a long answer keeps another
/// gateway from taking the store and turning that live attempt into an interrupted one.
pub struct Owner {
    /// Written on every journal entry of this ownership; a gateway that takes the store later turns every
    /// entry of another generation into an interrupted entry, since that generation has finished or died.
    generation: String,
    /// The exclusive OS lock on `<store>.gateway-lock`; `None` without a store file (tests). Dropped, and so
    /// released, with the last holder of this ownership.
    _lock: Option<std::fs::File>,
}

/// The lock file next to a store, held exclusively by the one gateway that serves it.
pub fn owner_lock_path(store: &std::path::Path) -> std::path::PathBuf {
    let mut name = store.as_os_str().to_owned();
    name.push(".gateway-lock");
    std::path::PathBuf::from(name)
}

/// Takes the store's owner lock, or says who has it. The operating system releases it when the process
/// ends, however it ends, so a crash leaves no stale lock.
fn claim_store(store: &std::path::Path) -> std::io::Result<std::fs::File> {
    let path = owner_lock_path(store);
    let file = std::fs::OpenOptions::new().create(true).truncate(false).read(true).write(true).open(&path)?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(std::fs::TryLockError::WouldBlock) => Err(std::io::Error::new(std::io::ErrorKind::ResourceBusy,
            "Another gateway is already serving this store: the Pygg app or `gateway serve`, or one \
             that was stopped and is still finishing requests. Stop that one first, or wait for it to finish; only \
             one gateway may write the usage ledger at a time.")),
        Err(std::fs::TryLockError::Error(e)) => Err(e),
    }
}

/// Whether another process holds the store (a gateway serving it, which may also be running tasks). Takes and
/// releases the lock to find out.
pub fn store_in_use(store: &std::path::Path) -> bool {
    matches!(claim_store(store), Err(e) if e.kind() == std::io::ErrorKind::ResourceBusy)
}

/// What the ledger is waiting for. While any settlement is pending, no paid request is admitted.
#[derive(Default)]
struct Accounting {
    /// Why the last journal entry could not be written; cleared by the next one that is.
    journal_error: Option<String>,
    /// Attempts whose ledger entry could not be written, by attempt id, with the entry and the error. Each one
    /// still has its journal entry in the store, so a crash turns it into an interrupted entry; otherwise it is
    /// settled exactly as it would have been once the store accepts it. Each holds the ownership, so the store
    /// is not handed to another gateway while it waits.
    pending: HashMap<String, (store::PendingUsage, String, Arc<Owner>)>,
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
            .user_agent(concat!("Pygg/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("HTTP client");
        Shared { db, secrets, http, signatures: Arc::default(), inflight: Arc::default(), accounting: Arc::default(),
            store_path: None, owner: Arc::new(Mutex::new(Weak::new())), tasks: None, web: None }
    }

    /// For a store on disk: `start` then takes the store's owner lock, so two processes never serve one store.
    pub fn for_store(db: Arc<Mutex<Connection>>, secrets: Arc<dyn Secrets>, path: &std::path::Path) -> Shared {
        Shared { store_path: Some(path.to_path_buf()), ..Shared::new(db, secrets) }
    }

    /// Lets gateway clients start smart tasks on this runner (the app's).
    pub fn with_tasks(self, runner: crate::tasks::Runner) -> Shared {
        Shared { tasks: Some(runner), ..self }
    }

    /// Lets web-session connections be reached through the app's endpoint (ADR-035).
    pub fn with_web(self, link: crate::websession::Link) -> Shared {
        Shared { web: Some(link), ..self }
    }

    /// The current ownership, if this process still holds the store.
    fn current_owner(&self) -> Option<Arc<Owner>> {
        self.owner.lock().unwrap_or_else(|p| p.into_inner()).upgrade()
    }

    /// Whether this process still holds the store: a server is running, or a stopped one's work is still being
    /// settled.
    pub fn owns_store(&self) -> bool {
        self.current_owner().is_some()
    }

    fn accounting(&self) -> std::sync::MutexGuard<'_, Accounting> {
        self.accounting.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Why the ledger is degraded, if it is: settlements waiting for the store, or a journal entry that could
    /// not be written. Attempts are journaled before they are sent, so this never means spend went unrecorded.
    pub fn accounting_error(&self) -> Option<String> {
        let a = self.accounting();
        if let Some((_, error, _)) = a.pending.values().next() {
            return Some(format!("{} usage {} waiting to be written: {error}", a.pending.len(),
                if a.pending.len() == 1 { "entry is" } else { "entries are" }));
        }
        a.journal_error.clone()
    }

    fn set_journal_error(&self, error: Option<String>) {
        self.accounting().journal_error = error;
    }

    /// Settlements still waiting for the store.
    pub fn pending_settlements(&self) -> usize {
        self.accounting().pending.len()
    }

    /// Tries the waiting settlements again, each exactly as it would have been written. A journal entry already
    /// turned into an interrupted entry is not written twice. A settled one gives up its hold on the store.
    pub fn recover_accounting(&self) {
        let waiting: Vec<(String, store::PendingUsage)> =
            self.accounting().pending.iter().map(|(id, (usage, _, _))| (id.clone(), usage.clone())).collect();
        for (id, usage) in waiting {
            let settled = self.with_db(|db| store::settle(db, &id, &usage.record()));
            let mut a = self.accounting();
            match settled {
                Ok(()) => { a.pending.remove(&id); }
                Err(e) => { if let Some(entry) = a.pending.get_mut(&id) { entry.1 = e; } }
            }
        }
    }

    /// Runs `f` with the database. Never held across an await.
    pub fn with_db<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
        let conn = self.db.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&conn)
    }

    /// Remembers Gemini tool-call signatures in a response or stream chunk, in memory and in the store, so a
    /// conversation that goes on after a restart still has them (review GW-8).
    pub fn capture_signatures(&self, value: &Value) {
        let found = self.signatures.lock().unwrap_or_else(|p| p.into_inner()).capture(value);
        if !found.is_empty() {
            let _ = self.with_db(|db| signatures::store(db, &found).map_err(|e| e.to_string()));
        }
    }

    /// Brings back stored signatures that a request to Gemini needs and memory has lost (after a restart).
    fn restore_signatures(&self, body: &Value) {
        let missing = self.signatures.lock().unwrap_or_else(|p| p.into_inner()).missing(body);
        if missing.is_empty() {
            return;
        }
        if let Ok(found) = self.with_db(|db| signatures::load(db, &missing).map_err(|e| e.to_string())) {
            let mut cache = self.signatures.lock().unwrap_or_else(|p| p.into_inner());
            for (id, signature) in found {
                cache.insert(&id, &signature);
            }
        }
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

/// One upstream attempt, journaled before it was sent, with the spend it holds (counter-review CR-06).
/// `settle` writes its ledger entry and removes the journal entry in one transaction. An attempt dropped
/// without being settled (a panic, an aborted task) becomes an entry with an unknown cost; one that cannot
/// be written at all stays in the journal and is turned into an entry by the next owner of the store. It
/// holds the store's ownership until then (review LC-02).
pub struct Attempt {
    db: Arc<Mutex<Connection>>,
    id: String,
    settled: bool,
    owner: Option<Arc<Owner>>,
}

impl Attempt {
    fn settle(mut self, shared: &Shared, record: &UsageRecord) {
        let mut error = None;
        // Each try already waits up to the store's busy timeout for another writer.
        for _ in 0..3 {
            match shared.with_db(|db| store::settle(db, &self.id, record)) {
                Ok(()) => {
                    self.settled = true;
                    return;
                }
                Err(e) => error = Some(e),
            }
        }
        let error = error.unwrap_or_default();
        eprintln!("gateway: could not write the ledger entry for attempt {}: {error}", self.id);
        // Kept with its exact figures and retried before every request; paid requests wait until it is written,
        // and the store stays owned by this process meanwhile.
        if let Some(owner) = self.owner.take() {
            shared.accounting().pending.insert(self.id.clone(), (store::PendingUsage::of(record), error, owner));
        }
        self.settled = true;
    }
}

impl Drop for Attempt {
    fn drop(&mut self) {
        if !self.settled {
            let conn = self.db.lock().unwrap_or_else(|p| p.into_inner());
            let _ = store::settle_interrupted(&conn, store::Unsettled::Attempt(&self.id));
        }
        // `owner` is dropped after this, so the store is released only once the attempt is accounted for.
    }
}

pub struct Server {
    pub addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
    /// The health probe (probe.rs), stopped with the server.
    probe: (tokio::sync::watch::Sender<bool>, tokio::task::JoinHandle<()>),
    /// This server's hold on the store's ownership. Attempts in flight hold it too (review LC-02).
    _owner: Arc<Owner>,
}

impl Server {
    /// Stops accepting requests and gives open responses a few seconds to finish before ending the listener.
    /// Attempts still in flight run to completion and are settled; the store stays owned by this process until
    /// the last of them is, so no other gateway can take it over a live attempt (review LC-02).
    pub async fn stop(self) {
        self.stop_within(Duration::from_secs(5)).await
    }

    pub async fn stop_within(mut self, grace: Duration) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        let _ = self.probe.0.send(true);
        self.probe.1.abort();
        if tokio::time::timeout(grace, &mut self.task).await.is_err() {
            self.task.abort();
        }
    }
}

/// Starts serving (reviews PR-01, LC-01, LC-02, LC-03).
///
/// - If this process still owns the store (a stopped server's attempts are still settling), the new server
///   joins that ownership: same generation, nothing of it is turned into interrupted entries.
/// - Otherwise it takes the store's exclusive OS lock (a second gateway fails with `ErrorKind::ResourceBusy`
///   and sends nothing) and starts a new generation. Every journal entry of another generation is then known
///   to be finished or orphaned, and becomes an interrupted entry. If that cannot be written, the start
///   fails and the lock is released: the gateway does not report itself running over unaccounted attempts.
pub async fn start(shared: Shared, port: u16) -> std::io::Result<Server> {
    let owner = match shared.current_owner() {
        Some(owner) => owner,
        None => {
            let lock = match &shared.store_path {
                Some(path) => Some(claim_store(path)?),
                None => None,
            };
            let owner = Arc::new(Owner { generation: uuid::Uuid::new_v4().to_string(), _lock: lock });
            shared.with_db(|db| store::settle_interrupted(db, store::Unsettled::NotOf(&owner.generation)))
                .map_err(|e| std::io::Error::other(format!(
                    "The usage ledger could not record the attempts a previous gateway left unsettled ({e}), so \
                     the gateway did not start. Check that the store is writable and start it again.")))?;
            *shared.owner.lock().unwrap_or_else(|p| p.into_inner()) = Arc::downgrade(&owner);
            owner
        }
    };
    shared.recover_accounting();
    // Expired session pins only cost space; a failure here does not touch accounting.
    if let Err(e) = shared.with_db(|db| store::purge_sessions(db, None)) {
        eprintln!("gateway: could not delete expired session pins: {e}");
    }
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
    let addr = listener.local_addr()?;
    let (probe_stop, probe_rx) = tokio::sync::watch::channel(false);
    let probe = (probe_stop, probe::spawn(shared.clone(), probe_rx));
    let app = router(shared, addr.port());
    let (tx, rx) = oneshot::channel::<()>();
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, app).with_graceful_shutdown(async { let _ = rx.await; }).await;
    });
    Ok(Server { addr, shutdown: Some(tx), task, probe, _owner: owner })
}

pub fn router(shared: Shared, port: u16) -> Router {
    Router::new()
        .route("/health", get(|State(shared): State<Shared>| async move {
            let accounting = shared.accounting_error();
            Json(json!({ "ok": true, "app": "Pygg",
                "accounting": if accounting.is_some() { "degraded" } else { "ok" }, "accountingError": accounting }))
        }))
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(|s: State<Shared>, h: HeaderMap, b: Bytes| detached(chat_completions(s, h, b))))
        .route("/v1/messages", post(|s: State<Shared>, h: HeaderMap, b: Bytes| detached(anthropic::messages(s, h, b))))
        .route("/v1/messages/count_tokens", post(anthropic::count_tokens))
        .route("/v1/responses", post(|s: State<Shared>, h: HeaderMap, b: Bytes| detached(responses::responses(s, h, b))))
        .merge(task_api::routes())
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
    // A request log for checking what a client sends (which paths, whether each carries a session id).
    // Off unless SMARTTASKROUTER_LOG_REQUESTS is set; it never prints tokens or bodies.
    if std::env::var_os("SMARTTASKROUTER_LOG_REQUESTS").is_some() {
        let h = |name: &str| request.headers().get(name).and_then(|v| v.to_str().ok()).unwrap_or("-").to_string();
        eprintln!("{} {} {} session={} ua={}", crate::db::now_ms(), request.method(), request.uri().path(),
            h(SESSION_HEADER), h("user-agent"));
    }
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
            "Missing client token. Create one in Pygg → Clients."));
    };
    let client = shared.with_db(|db| store::authenticate(db, token)).map_err(|e| {
        Refusal::new(StatusCode::INTERNAL_SERVER_ERROR, "api_error", e)
    })?;
    let Some(client) = client else {
        return Err(Refusal::new(StatusCode::UNAUTHORIZED, "authentication_error", "Unknown or revoked client token"));
    };
    if let Some(budget) = client.monthly_budget_micro_usd {
        if client.counted_micro_usd() >= budget {
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
    let client = match authenticate(&shared, &headers) {
        Ok(client) => client,
        Err(refusal) => return refusal.openai(),
    };
    let listed = shared.with_db(|db| -> store::Result<Vec<Value>> {
        let mut data: Vec<Value> = auto::ALIASES.iter()
            .map(|alias| json!({ "id": alias, "object": "model", "created": 0, "owned_by": "smarttaskrouter auto" }))
            .collect();
        data.extend(store::list_combos(db)?
            .into_iter()
            .map(|c| json!({ "id": c.name, "object": "model", "created": 0, "owned_by": "smarttaskrouter combo" })));
        let providers = store::list_providers(db)?;
        // A pool's models, as `<pool>/<model>` (ADR-034): those an enabled member the client may use offers.
        let mut pooled = std::collections::BTreeSet::new();
        for model in store::list_models(db, None)?.into_iter().filter(|m| m.enabled) {
            if let Some(pool) = providers.iter().find(|p| p.id == model.provider_id && p.enabled && client.allows(p).is_ok())
                .and_then(|p| p.pool.clone()) {
                pooled.insert(format!("{pool}/{}", model.model_id));
            }
        }
        data.extend(pooled.into_iter().map(|id| json!({ "id": id, "object": "model", "created": 0, "owned_by": "smarttaskrouter pool" })));
        for model in store::list_models(db, None)? {
            let allowed = providers.iter().any(|p| p.id == model.provider_id && p.enabled && client.allows(p).is_ok());
            if model.enabled && allowed {
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
    /// OpenAI's Responses API (`/v1/responses`, ADR-033), which Codex CLI speaks.
    Responses,
}

/// How a request reached its target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    /// Same format: the body went through unchanged apart from the model name.
    Native,
    /// An Anthropic request translated to Chat Completions.
    AnthropicToOpenAi,
    /// A Chat Completions request translated to Anthropic Messages (ROADMAP B5).
    OpenAiToAnthropic,
    /// A Responses request translated to Chat Completions (ADR-033).
    ResponsesToOpenAi,
    /// A Responses request translated to Chat Completions, then to Anthropic Messages (ADR-033).
    ResponsesToAnthropic,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Native => "native",
            Mode::AnthropicToOpenAi => "translated:anthropic->openai",
            Mode::OpenAiToAnthropic => "translated:openai->anthropic",
            Mode::ResponsesToOpenAi => "translated:responses->openai",
            Mode::ResponsesToAnthropic => "translated:responses->anthropic",
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
    /// How many upstream calls this request made, this one included. Skipped targets are not counted.
    pub attempt: usize,
    pub mode: Mode,
    /// What the translation could not carry (empty for native).
    pub dropped: Vec<String>,
    pub session: Option<String>,
    /// The session was already pinned to this target.
    pub pinned: bool,
    pub client_id: String,
    pub requested: String,
    /// Why auto routing chose this target, and the header that says so (ROADMAP P0-5).
    pub route_reason: Option<String>,
    pub auto_header: Option<String>,
    /// Tool output the request repeated from earlier in itself (P1-4, shadow; `shadow.rs`).
    pub repeated_tokens: Option<u64>,
    /// The journaled attempt, settled by `record_success`.
    journal: Option<Attempt>,
    _inflight: InflightGuard,
}

const MAX_ERROR_BODY: usize = 64 * 1024;

/// Prompt tokens assumed for a request, for the hold only. An estimate, not a bound (counter-review CR-03):
/// a tokenizer can split text more finely. It leans high: the whole serialized body counts, ASCII at three
/// bytes a token where English averages about four, and every other character as a token of its own, which
/// covers Vietnamese and CJK text better than counting bytes. Base64 images count as text, far above what
/// providers charge for them.
fn estimate_input_tokens(body: &Value) -> u64 {
    estimate_text_tokens(&body.to_string())
}

/// The same estimate over a piece of text (a web session's prompt).
pub(crate) fn estimate_text_tokens(text: &str) -> u64 {
    let ascii = text.bytes().filter(u8::is_ascii).count() as u64;
    let other = text.chars().filter(|c| !c.is_ascii()).count() as u64;
    ascii.div_ceil(3) + other
}

/// The spend held for an attempt: the estimated prompt at the higher of the input and cache-write prices
/// (a prompt may be written to the cache), plus the output limit the request sets, or 4096 tokens when it
/// sets none. Settled to the real figure when usage arrives.
fn estimate_micro(body: &Value, price: &Price) -> u64 {
    estimate_micro_with(body, price, requested_output(body).unwrap_or(DEFAULT_OUTPUT_ESTIMATE))
}

/// The output limit a request sets, if any.
fn requested_output(body: &Value) -> Option<u64> {
    ["max_tokens", "max_completion_tokens", "max_output_tokens"].iter().find_map(|k| body[*k].as_u64())
}

fn estimate_micro_with(body: &Value, price: &Price, output_tokens: u64) -> u64 {
    let input_price = price.input_micro_per_mtok.max(price.cache_write_micro_per_mtok.unwrap_or(0));
    let per = |tokens: u64, micro: u64| ((tokens as u128 * micro as u128).div_ceil(1_000_000)) as u64;
    per(estimate_input_tokens(body), input_price) + per(output_tokens, price.output_micro_per_mtok)
}

/// A `cheapest` combo (migration 0025): its members by this request's estimated cost. Free, local and subscription
/// members count as $0 (a plan is quota, not money); a member whose price is unknown goes last, in its listed order.
fn cheapest_first(shared: &Shared, targets: &mut Vec<(Provider, String)>, body: &Value, combo: &str,
    reasons: &mut HashMap<(String, String), String>) {
    let mut keyed: Vec<(Option<u64>, usize, (Provider, String))> = std::mem::take(targets).into_iter().enumerate().map(|(i, (p, m))| {
        let (billing, price) = shared.with_db(|db| store::pricing_for(db, &p, &m)).unwrap_or(("unknown".into(), None));
        let cost = if is_free(&billing) || billing == "subscription" { Some(0) } else { price.as_ref().map(|price| estimate_micro(body, price)) };
        (cost, i, (p, m))
    }).collect();
    keyed.sort_by_key(|(cost, i, _)| (cost.is_none(), cost.unwrap_or(0), *i));
    for (cost, _, (p, m)) in keyed {
        let why = match cost {
            Some(micro) => format!("combo {combo}, cheapest first: about ${:.4} for this request", micro as f64 / 1e6),
            None => format!("combo {combo}, cheapest first: price unknown, tried last"),
        };
        reasons.insert((p.id.clone(), m.clone()), why);
        targets.push((p, m));
    }
}

fn is_free(billing: &str) -> bool {
    matches!(billing, "free" | "local")
}

/// A failed request on a subscription uses the plan's quota, not money: an unknown cost says so, as a successful
/// one does, so a money limit does not count it as spend at risk (found by the benchmark, ROADMAP C3). On a free web
/// session (ADR-035, 2026-11-13) a request costs $0 whatever happened to it: the site is used on the user's plan.
pub(crate) fn quota_not_money(web: bool, billing: &str, cost: Cost) -> Cost {
    match cost {
        Cost::Unknown { source } if web && billing == "free" => Cost::Estimated { micro_usd: 0,
            source: format!("web session: your plan with the site, no price per request ({source})") },
        Cost::Unknown { source } if billing == "subscription" => Cost::Unknown {
            source: format!("subscription: counts against a quota, not money ({source})") },
        other => other,
    }
}

/// Sends the request to each target of `requested` in turn, until one starts a successful response. Each
/// failed attempt is recorded in the ledger as it happens; each skipped target says why.
pub async fn forward(shared: &Shared, client: &Client, incoming: &Incoming<'_>) -> Result<Forwarded, Refusal> {
    let Some(inflight) = shared.enter(client) else {
        return Err(Refusal { retry_after: Some(1), ..Refusal::new(StatusCode::TOO_MANY_REQUESTS, "rate_limit_error",
            format!("Client \"{}\" already has {} requests in flight, its limit.", client.name, client.max_concurrency)) });
    };
    if shared.pending_settlements() > 0 {
        shared.recover_accounting();
    }
    let requested = incoming.requested;
    // `auto` and `auto/cheap` build their route from the connected models, with a reason for each (P0-5).
    let mut reasons: HashMap<(String, String), String> = HashMap::new();
    let mut auto_plan: Option<auto::Plan> = None;
    let mut targets = match auto::strategy(requested) {
        None => {
            let mut targets = shared.with_db(|db| store::resolve(db, requested))
                .map_err(|e| Refusal::new(StatusCode::INTERNAL_SERVER_ERROR, "api_error", e))?;
            if shared.with_db(|db| store::combo_strategy(db, requested)).ok().flatten().as_deref() == Some("cheapest") {
                cheapest_first(shared, &mut targets, incoming.body, requested, &mut reasons);
            }
            targets
        }
        Some(Err(message)) => return Err(Refusal::new(StatusCode::NOT_FOUND, "model_not_found", message)),
        Some(Ok(strategy)) => {
            let needs = auto::Needs::of(incoming.surface, incoming.body);
            let plan = shared.with_db(|db| auto::plan(db, client, requested, strategy, &needs))
                .map_err(|e| Refusal::new(StatusCode::INTERNAL_SERVER_ERROR, "api_error", e))?;
            if plan.ranked.is_empty() {
                let why = auto::summarize(&plan.excluded, 8);
                return Err(Refusal::new(StatusCode::UNPROCESSABLE_ENTITY, "no_eligible_model", format!(
                    "No connected model is eligible for \"{requested}\". Set tiers, context sizes and prices in \
                     Pygg → Providers, or ask for a model by name.{}{}", if why.is_empty() { "" } else { " " }, why)));
            }
            let mut route = Vec::new();
            for ranked in plan.ranked.iter().take(auto::ROUTE_LENGTH) {
                if let Ok(Some(provider)) = shared.with_db(|db| store::get_provider(db, &ranked.provider_id)) {
                    reasons.insert((ranked.provider_id.clone(), ranked.model_id.clone()), ranked.reason.clone());
                    route.push((provider, ranked.model_id.clone()));
                }
            }
            auto_plan = Some(plan);
            route
        }
    };
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
    let mut sent = 0;
    let mut last: Option<Refusal> = None;
    let mut skipped: Vec<String> = Vec::new();
    // Quota group → the connection that refused. Its own other models may still answer (quotas are often per
    // model); other connections of the group may not, which would be using a sibling key to get around a limit.
    let mut blocked_groups: HashMap<String, String> = HashMap::new();
    let mut retry_at: Option<i64> = None;
    let has_limit = client.monthly_budget_micro_usd.is_some();
    // Further targets this client allows after a failure the provider may have processed (reviews FR-02, PR-02).
    let mut paid_retries_left = client.paid_retries;
    let mut accounting_held = false;
    // Every target was a web session and the request uses tools: the request is the problem, not the targets.
    let mut tools_refused = false;

    for (index, (provider, model_id)) in targets.into_iter().enumerate() {
        let outcome_if_failed = if index + 1 < count { "fallback" } else { "error" };
        let name = format!("{}/{model_id}", provider.id);
        let route_reason = reasons.get(&(provider.id.clone(), model_id.clone())).cloned();

        // A client restricted to some connections, or to providers that do not train on its data, never reaches
        // another one, whatever the combo, the session pin or a fallback says (reviews FR-01, GW-10).
        if let Err(why) = client.allows(&provider) {
            skipped.push(format!("{name}: {why}"));
            continue;
        }

        // Which mode would carry this request to this target, if any.
        let mode = match (incoming.surface, provider.wire.as_str()) {
            (Surface::OpenAi, "openai") | (Surface::Anthropic, "anthropic") => Mode::Native,
            (Surface::Anthropic, "openai") => Mode::AnthropicToOpenAi,
            (Surface::OpenAi, "anthropic") => Mode::OpenAiToAnthropic,
            (Surface::Responses, "openai") if presets::find(&provider.kind).is_some_and(|p| p.serves_responses) => Mode::Native,
            (Surface::Responses, "openai") => Mode::ResponsesToOpenAi,
            (Surface::Responses, "anthropic") => Mode::ResponsesToAnthropic,
            _ => {
                skipped.push(format!("{name}: speaks the {} API, which this request's format cannot reach yet", provider.wire));
                continue;
            }
        };
        // A web session (ADR-035) takes a request only with its warning acknowledged, without tools, and while the app
        // that drives its window runs; it is reached through that app's loopback endpoint.
        let web = presets::is_web(&provider.kind);
        if web {
            if let Err((why, tools)) = crate::websession::routable(&provider, incoming.body, shared.web.is_some()) {
                tools_refused |= tools;
                skipped.push(format!("{name}: {why}"));
                continue;
            }
        }
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
        // A refusal on another connection of the same quota group keeps this one off the same model too, for as
        // long as that refusal lasts, not only within the request that hit it (counter-review CR-01).
        if let Ok(Some((holder, hold))) = shared.with_db(|db| store::group_hold(db, &provider, &model_id)) {
            if let Some(until) = hold.until_ms {
                retry_at = Some(retry_at.map_or(until, |r: i64| r.min(until)));
            }
            skipped.push(format!("{name}: shares limits with {}, which is {} ({})", holder.label,
                hold.state.replace('_', " "), hold.reason));
            continue;
        }

        let (billing, price) = shared.with_db(|db| store::pricing_for(db, &provider, &model_id)).unwrap_or_default();
        // The user's plan with the site: free ($0 a request, the default) or a subscription (quota, cost unknown),
        // whatever a model's own settings say; never paid per token.
        let billing = if web && billing != "free" { "subscription".to_string() } else { billing };
        // A subscription (a token plan) uses a quota, not money per request: it holds nothing against a money
        // limit, as auto ranks it and as its ledger entry says. Its price, if one was entered, is not what it costs.
        let hold = if is_free(&billing) || billing == "subscription" || !has_limit {
            0
        } else if let Some(price) = price.as_ref() {
            if client.strict_budget {
                // The most this request could cost: its own output limit, or all the model can write.
                let most = requested_output(incoming.body)
                    .or_else(|| shared.with_db(|db| store::max_output_tokens(db, &provider.id, &model_id)).ok().flatten());
                let Some(most) = most else {
                    skipped.push(format!("{name}: this client's limit is strict, and neither the request nor the model states a maximum output"));
                    continue;
                };
                estimate_micro_with(incoming.body, price, most)
            } else {
                estimate_micro(incoming.body, price)
            }
        } else if client.unknown_cost_policy == "block" || client.strict_budget {
            skipped.push(format!("{name}: its price is unknown, and this client has a spending limit"));
            continue;
        } else {
            0
        };
        // While a ledger entry waits to be written, nothing that costs money is sent (review FR-03).
        if !is_free(&billing) && shared.pending_settlements() > 0 {
            accounting_held = true;
            skipped.push(format!("{name}: paid requests wait until the usage ledger can be written ({})",
                shared.accounting_error().unwrap_or_default()));
            continue;
        }
        // Every attempt holds the store's ownership until it is settled (review LC-02). Without one (the
        // gateway was stopped and its last work has finished), nothing new is sent.
        let Some(owner) = shared.current_owner() else {
            return Err(Refusal::new(StatusCode::SERVICE_UNAVAILABLE, "api_error",
                "The gateway has stopped, so nothing was sent. Start it again in Pygg."));
        };
        // Journaled before anything is sent (counter-review CR-06); nothing is sent that the ledger could not
        // account for.
        let start = store::AttemptStart { client_id: &client.id, provider_id: &provider.id, model_id: &model_id,
            requested_model: requested, session_key: incoming.session, route_mode: mode.label(), hold_micro: hold,
            owner_generation: &owner.generation };
        let journal = match shared.with_db(|db| store::open_attempt(db, &start)) {
            Ok(Ok(id)) => {
                shared.set_journal_error(None);
                Attempt { db: shared.db.clone(), id, settled: false, owner: Some(owner) }
            }
            Ok(Err(over)) => {
                skipped.push(format!("{name}: up to ${:.4} (estimated) would pass the monthly limit (${:.2} counted, ${:.2} held, ${:.2} limit)",
                    over.wanted as f64 / 1e6, over.spent as f64 / 1e6, over.reserved as f64 / 1e6, over.budget as f64 / 1e6));
                continue;
            }
            Err(e) => {
                shared.set_journal_error(Some(e.clone()));
                return Err(Refusal::new(StatusCode::SERVICE_UNAVAILABLE, "api_error",
                    format!("The usage ledger cannot be written ({e}), so nothing was sent.")));
            }
        };

        let started = Instant::now();
        let settle_failure = |journal: Attempt, status: Option<u16>, cost: Cost, outcome: &str, error: &str| {
            journal.settle(shared, &UsageRecord {
                session_key: incoming.session, route_mode: Some(mode.label()), route_reason: route_reason.as_deref(), repeated_tokens: None,
                ..UsageRecord::failed(&client.id, &provider.id, &model_id, requested, &cost, outcome, status,
                    started.elapsed().as_millis() as u64, error)
            });
        };
        let mut apply = |failure: &Failure, message: &str, provider: &Provider| -> policy::Decision {
            let decision = policy::decide(failure);
            if let Some(mark) = &decision.mark {
                let until = mark.cooldown.map(|d| db_now() + d.as_millis() as i64);
                // A 402 on a model that is free without credit (an OpenRouter `:free` one) is kept on that model: a
                // connection-wide entry does not hold such a model (`store::blocking_health`), so it would be sent
                // again at once.
                let model_scope = mark.model_scope || (mark.state == "payment_required"
                    && shared.with_db(|db| store::free_without_credit(db, &provider.id, &model_id)).unwrap_or(false));
                let model = if model_scope { model_id.as_str() } else { "" };
                let status = match failure { Failure::Http(e) => Some(e.status), _ => None };
                let _ = shared.with_db(|db| store::mark_health(db, &provider.id, model, mark.state, message, status, until));
                // A 429 on one model is a strike against it in `auto`: without a usable reset, or repeated, the model
                // is held out of the automatic routes for a growing while (`auto::strike`). Named routes still try it.
                if mark.state == "rate_limited" && model_scope {
                    let reset = match failure { Failure::Http(e) => e.retry_after, _ => None };
                    let _ = shared.with_db(|db| auto::strike(db, &provider.id, &model_id, reset, message, db_now()));
                }
            }
            if decision.next == Next::OtherGroups {
                blocked_groups.entry(provider.quota_group.clone()).or_insert_with(|| provider.id.clone());
            }
            decision
        };

        let key = match &provider.credential_ref {
            _ if web => shared.web.as_ref().map(|link| link.token.clone()),
            Some(reference) => match shared.secrets.read(reference) {
                Ok(Some(key)) => Some(key),
                Ok(None) => {
                    let message = format!("No key is stored for {}", provider.label);
                    settle_failure(journal, None, Cost::Estimated { micro_usd: 0, source: "not sent".into() }, outcome_if_failed, &message);
                    let _ = shared.with_db(|db| store::mark_health(db, &provider.id, "", "invalid_credentials", &message, None, None));
                    last = Some(Refusal::new(StatusCode::BAD_GATEWAY, "api_error", message));
                    continue;
                }
                Err(e) => {
                    let message = format!("Could not read the key for {}: {e}", provider.label);
                    settle_failure(journal, None, Cost::Estimated { micro_usd: 0, source: "not sent".into() }, outcome_if_failed, &message);
                    last = Some(Refusal::new(StatusCode::BAD_GATEWAY, "api_error", message));
                    continue;
                }
            },
            None => None,
        };

        // What the model takes for thinking, for an OpenAI client's `reasoning_effort` (ROADMAP B5).
        let thinking = if matches!(mode, Mode::OpenAiToAnthropic | Mode::ResponsesToAnthropic) {
            let (types, efforts) = shared.with_db(|db| store::thinking_support(db, &provider.id, &model_id)).unwrap_or_default();
            openai_to_anthropic::ThinkingSupport { types, efforts }
        } else {
            openai_to_anthropic::ThinkingSupport::default()
        };
        let (url, mut body, dropped) = match shared.web.as_ref().filter(|_| web) {
            Some(link) => upstream_request(incoming, &Provider { endpoint: link.endpoint(&provider.id), ..provider.clone() },
                &model_id, mode, &thinking),
            None => upstream_request(incoming, &provider, &model_id, mode, &thinking),
        };
        if provider.wire == "openai" {
            // Gemini needs the signatures of earlier tool calls back; others must not see the field.
            if provider.kind == "gemini" {
                shared.restore_signatures(&body);
            }
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

        sent += 1;
        let attempt = sent;
        match request.send().await {
            Err(e) => {
                let secrets: Vec<&str> = key.as_deref().into_iter().collect();
                let message = crate::redact::redact(&format!("{}: {e}", provider.label), &secrets);
                // A refused or failed connection never reached the provider. Anything else (a timeout while
                // waiting, a dropped connection) may have run the request: stop, and say the cost is unknown.
                let failure = if e.is_connect() { Failure::NotSent } else { Failure::Ambiguous };
                let decision = apply(&failure, &message, &provider);
                let cost = quota_not_money(web, &billing, if decision.maybe_billed {
                    pricing::resolve_failure(None)
                } else {
                    Cost::Estimated { micro_usd: 0, source: "could not connect".into() }
                });
                let next = decision.next;
                settle_failure(journal, None, cost, if next == Next::Stop { "error" } else { outcome_if_failed }, &message);
                if next == Next::Stop {
                    return Err(Refusal::new(StatusCode::GATEWAY_TIMEOUT, "api_error", format!(
                        "{message}. The provider may still have run the request, so no other model was tried.")));
                }
                last = Some(Refusal::new(StatusCode::BAD_GATEWAY, "api_error", message));
            }
            Ok(response) if response.status().is_success() => {
                note_limits(shared, &provider.id, &model_id, response.status().as_u16(), response.headers());
                let _ = shared.with_db(|db| store::mark_healthy(db, &provider.id, &model_id));
                let _ = shared.with_db(|db| auto::clear_hold(db, &provider.id, &model_id));
                let pinned = pinned_target.as_ref().is_some_and(|p| p.provider_id == provider.id && p.model_id == model_id);
                let auto_header = auto_plan.as_ref().and_then(|plan| plan.ranked.iter()
                    .find(|r| r.provider_id == provider.id && r.model_id == model_id).map(|r| {
                        let mut header = plan.header(r);
                        if pinned {
                            header.push_str("; kept for this session");
                        }
                        header
                    }));
                return Ok(Forwarded {
                    response, provider, model_id, billing, price, started, attempt, mode, dropped,
                    session: incoming.session.map(String::from), pinned, client_id: client.id.clone(),
                    requested: requested.to_string(), auto_header, route_reason, journal: Some(journal), _inflight: inflight,
                    repeated_tokens: shadow::repeated_tokens(incoming.surface, incoming.body),
                });
            }
            Ok(response) => {
                let status = response.status();
                let headers = response.headers().clone();
                note_limits(shared, &provider.id, &model_id, status.as_u16(), &headers);
                let content_type = headers.get(header::CONTENT_TYPE).cloned();
                let bytes = read_limited(response, MAX_ERROR_BODY).await;
                let error = policy::ProviderError::from_response(status.as_u16(), &headers, &bytes);
                let retry_after = error.retry_after;
                let secrets: Vec<&str> = key.as_deref().into_iter().collect();
                let detail = crate::redact::redact(&upstream_message(&bytes), &secrets);
                let message = format!("{} answered HTTP {}: {detail}", provider.label, status.as_u16());
                let decision = apply(&Failure::Http(error), &message, &provider);
                let mut next = decision.next;
                let mut message = message;
                // A failure after which the provider may have processed the request (a 5xx) moves on to another
                // target only if this client opted into retries after a server error. By default it stops, on
                // every billing type: a free or local label says nothing was charged, not that running the
                // request twice is safe (reviews FR-02, PR-02).
                if decision.maybe_billed && next != Next::Stop && index + 1 < count {
                    if paid_retries_left == 0 {
                        next = Next::Stop;
                        message = format!("{message}. The provider may have processed this request, so no other model \
                            was tried; this client allows no retry after a server error.");
                    } else {
                        paid_retries_left -= 1;
                    }
                }
                let cost = quota_not_money(web, &billing, if decision.maybe_billed {
                    Cost::Unknown { source: format!("HTTP {}: the provider may have run the request", status.as_u16()) }
                } else {
                    pricing::resolve_failure(Some(status.as_u16()))
                });
                settle_failure(journal, Some(status.as_u16()), cost,
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
    let tools_only = tools_refused && skipped.iter().all(|s| s.ends_with(crate::websession::NO_TOOLS));
    let (status, kind) = if accounting_held {
        (StatusCode::SERVICE_UNAVAILABLE, "api_error")
    } else if tools_only {
        (StatusCode::BAD_REQUEST, "invalid_request_error")
    } else if over_budget {
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

/// The URL and body for one target, and what a translation had to drop.
fn upstream_request(incoming: &Incoming, provider: &Provider, model_id: &str, mode: Mode,
    thinking: &openai_to_anthropic::ThinkingSupport) -> (String, Value, Vec<String>) {
    let accepts_cache = presets::find(&provider.kind).is_some_and(|p| p.accepts_cache_control);
    let takes_reasoning = presets::find(&provider.kind).is_some_and(|p| p.takes_reasoning);
    let (mut body, dropped) = match mode {
        // Anthropic checks the signature of every thinking block: one Pygg made from another provider's reasoning
        // would be refused, so it is left out (ROADMAP B5).
        Mode::Native if provider.kind == "anthropic" && incoming.surface == Surface::Anthropic => {
            match anthropic::without_pygg_reasoning(incoming.body) {
                Some(body) => (body, vec!["thinking (from another provider)".to_string()]),
                None => (incoming.body.clone(), Vec::new()),
            }
        }
        Mode::Native => (incoming.body.clone(), Vec::new()),
        Mode::AnthropicToOpenAi => {
            let mut dropped = anthropic::untranslated(incoming.body, takes_reasoning);
            if !accepts_cache && anthropic::has_cache_control(incoming.body) {
                dropped.insert(0, "cache_control".to_string());
            }
            (anthropic::to_openai(incoming.body, accepts_cache, takes_reasoning), dropped)
        }
        Mode::OpenAiToAnthropic => openai_to_anthropic::to_anthropic(incoming.body, thinking),
        Mode::ResponsesToOpenAi => responses::to_chat(incoming.body, takes_reasoning),
        // Through the tested Chat Completions → Messages translator; what either step drops is named. The Responses
        // API's `reasoning.effort` crosses as `reasoning_effort`, and is named by its own name if it cannot.
        Mode::ResponsesToAnthropic => {
            let (mut chat, mut dropped) = responses::to_chat(incoming.body, false);
            if let Some(effort) = incoming.body["reasoning"]["effort"].as_str() {
                chat["reasoning_effort"] = json!(effort);
                dropped.retain(|d| d != "reasoning.effort");
            }
            let (body, more) = openai_to_anthropic::to_anthropic(&chat, thinking);
            let more = more.into_iter().map(|m| match m.strip_prefix("reasoning_effort") {
                Some(rest) => format!("reasoning.effort{rest}"),
                None => m,
            });
            dropped.extend(more.filter(|m| !dropped.contains(m)).collect::<Vec<_>>());
            (body, dropped)
        }
    };
    body["model"] = json!(model_id);
    if incoming.surface == Surface::Responses && mode == Mode::Native {
        // The provider serves the Responses API itself: the body goes as it came.
        (format!("{}/responses", provider.endpoint), body, dropped)
    } else if provider.wire == "openai" {
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

/// Settles a response that succeeded upstream: its ledger entry is written and its journal entry removed in
/// one transaction. Also pins an explicit session to this target.
pub fn record_success(shared: &Shared, mut forwarded: Forwarded, usage: Option<&Usage>, outcome: &str, error: Option<&str>) {
    let cost = if outcome == "ok" {
        match pricing::resolve(&forwarded.billing, forwarded.price.as_ref(), usage) {
            // A web session on the user's plan with the site (ADR-035, 2026-11-13): $0 a request, and the ledger says so.
            Cost::Estimated { micro_usd: 0, .. } if presets::is_web(&forwarded.provider.kind) =>
                Cost::Estimated { micro_usd: 0, source: "web session: your plan with the site, no price per request".into() },
            cost => cost,
        }
    } else {
        // Cut short: whatever usage arrived is partial, so the amount cannot be stated. Free, local and
        // subscription billing cost no money either way, so they keep what they say (a subscription's entry
        // still says quota, not money: the benchmark, ROADMAP C3, read it as possible spend).
        match pricing::resolve(&forwarded.billing, forwarded.price.as_ref(), usage) {
            Cost::Estimated { micro_usd: 0, .. } if presets::is_web(&forwarded.provider.kind) =>
                Cost::Estimated { micro_usd: 0, source: "web session: your plan with the site, no price per request (cut short)".into() },
            cost @ Cost::Estimated { micro_usd: 0, .. } => cost,
            cost if forwarded.billing == "subscription" => cost,
            _ => Cost::Unknown { source: "the response was cut short before usage was reported".into() },
        }
    };
    let journal = forwarded.journal.take();
    let record = UsageRecord {
        client_id: Some(&forwarded.client_id), provider_id: &forwarded.provider.id, model_id: &forwarded.model_id,
        requested_model: &forwarded.requested, input_tokens: usage.map(|u| u.prompt),
        output_tokens: usage.map(|u| u.completion), cache_tokens: usage.map(|u| u.cached), cost: &cost, outcome,
        http_status: Some(forwarded.response.status().as_u16()),
        latency_ms: forwarded.started.elapsed().as_millis() as u64, error,
        cache_write_tokens: usage.map(|u| u.cache_write), session_key: forwarded.session.as_deref(),
        route_mode: Some(forwarded.mode.label()), route_reason: forwarded.route_reason.as_deref(),
        repeated_tokens: forwarded.repeated_tokens,
    };
    if let Some(journal) = journal {
        journal.settle(shared, &record);
    }
    if let Some(session) = &forwarded.session {
        let target = Target { provider_id: forwarded.provider.id.clone(), model_id: forwarded.model_id.clone() };
        let _ = shared.with_db(|db| store::pin(db, &forwarded.client_id, session, &forwarded.requested, &target));
    }
}

/// Keeps the rate-limit headers a provider sent with an answer, to show as "as the provider last said" (PV-06,
/// ADR-030). Recording is best effort: a failed write never fails the request.
fn note_limits(shared: &Shared, provider_id: &str, model_id: &str, status: u16, headers: &HeaderMap) {
    let pairs: Vec<(String, String)> = headers.iter()
        .filter(|(n, _)| store::is_limit_header(n.as_str()))
        .filter_map(|(n, v)| v.to_str().ok().map(|v| (n.as_str().to_string(), v.to_string())))
        .collect();
    if !pairs.is_empty() {
        let _ = shared.with_db(|db| store::record_limits(db, provider_id, model_id, status, &pairs));
    }
}

/// The provider's rate-limit and request-id headers, passed on with a native answer (counter-review CR-08)
/// so a client that paces itself by them, or reports a request id, still can.
pub fn provider_headers(from: &HeaderMap, to: &mut HeaderMap) {
    for (name, value) in from {
        let n = name.as_str();
        if n.starts_with("anthropic-ratelimit-") || n.starts_with("x-ratelimit-") || n == "request-id" || n == "x-request-id" {
            to.insert(name.clone(), value.clone());
        }
    }
}

pub fn route_headers(response: &mut Response, forwarded: &Forwarded) {
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(&format!("{}/{}", forwarded.provider.id, forwarded.model_id)) {
        headers.insert("x-smarttaskrouter-route", value);
    }
    headers.insert("x-smarttaskrouter-attempts", HeaderValue::from(forwarded.attempt));
    if let Some(value) = forwarded.auto_header.as_deref().and_then(|h| HeaderValue::from_str(h).ok()) {
        headers.insert("x-smarttaskrouter-auto", value);
    }
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
    let upstream_headers = forwarded.response.headers().clone();
    if forwarded.mode == Mode::OpenAiToAnthropic {
        let include_usage = value["stream_options"]["include_usage"] == true;
        return from_anthropic_response(shared, forwarded, stream, include_usage, requested).await;
    }

    if !stream {
        let mut forwarded = forwarded;
        let response = std::mem::replace(&mut forwarded.response, status_only(status));
        return match response.bytes().await {
            Ok(bytes) => {
                let parsed = serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null);
                shared.capture_signatures(&parsed);
                let usage = Usage::from_json(&parsed["usage"]);
                let mut out = passthrough(status, content_type, bytes);
                provider_headers(&upstream_headers, out.headers_mut());
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
    provider_headers(&upstream_headers, out.headers_mut());
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

/// An Anthropic answer to a Chat Completions request, translated back (ROADMAP B5), streamed or not.
async fn from_anthropic_response(shared: Shared, mut forwarded: Forwarded, stream: bool, include_usage: bool,
    requested: String) -> Response {
    let upstream = std::mem::replace(&mut forwarded.response, status_only(StatusCode::OK));
    if !stream {
        return match upstream.bytes().await {
            Ok(bytes) => {
                let parsed = serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null);
                let (body, usage) = openai_to_anthropic::from_anthropic(&parsed, &requested);
                let mut out = Json(body).into_response();
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
    let (tx, rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(64);
    let mut out = stream_response(rx, StatusCode::OK, Some(HeaderValue::from_static("text/event-stream")));
    route_headers(&mut out, &forwarded);
    tokio::spawn(async move {
        let mut translator = openai_to_anthropic::ChunkTranslator::new(&requested, include_usage);
        let mut bytes = upstream.bytes_stream();
        let mut line = Vec::<u8>::new();
        let (mut outcome, mut error) = ("ok", None);
        'read: while let Some(chunk) = bytes.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(e) => { outcome = "stream_error"; error = Some(crate::redact::redact(&e.to_string(), &[])); break; }
            };
            for &b in chunk.iter() {
                if b != b'\n' {
                    if line.len() < 4 * 1024 * 1024 { line.push(b); }
                    continue;
                }
                let text = String::from_utf8_lossy(&line).trim_end_matches('\r').to_string();
                line.clear();
                let Some(data) = text.strip_prefix("data:").map(str::trim) else { continue };
                let Ok(event) = serde_json::from_str::<Value>(data) else { continue };
                if event["type"] == "error" {
                    outcome = "stream_error";
                    error = event["error"]["message"].as_str().map(String::from);
                    break 'read;
                }
                for piece in translator.push(&event) {
                    if tx.send(Ok(Bytes::from(piece))).await.is_err() {
                        outcome = "client_disconnected";
                        break 'read;
                    }
                }
            }
        }
        if outcome == "ok" {
            for piece in translator.finish() {
                let _ = tx.send(Ok(Bytes::from(piece))).await;
            }
        } else if outcome == "stream_error" {
            let _ = tx.send(Err(std::io::Error::other("upstream stream failed"))).await;
        }
        let usage = translator.usage.clone();
        record_success(&shared, forwarded, usage.as_ref(), outcome, error.as_deref());
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
