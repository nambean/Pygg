//! Tauri commands: what the interface may ask of the core. Secrets go in (a key being saved) but never come
//! back out; the interface only learns whether a connection has a key.

use crate::gateway::pricing::Price;
use crate::gateway::store::{self, Client, Combo, Model, NewProvider, Provider, Target, UsageSummary};
use crate::gateway::{self, catalog, presets, Server, Shared};
use crate::vault;
use serde::{Deserialize, Serialize};
use tauri::State;

pub struct AppState {
    pub shared: Shared,
    pub server: tokio::sync::Mutex<Option<Server>>,
    pub last_error: std::sync::Mutex<Option<String>>,
    /// Where agent runs get their worktrees (the app data folder).
    pub data_dir: std::path::PathBuf,
    /// Agent runs whose worktree is still there, by id. The interface refers to a run by id only, never by path.
    pub runs: std::sync::Mutex<std::collections::HashMap<String, crate::agents::TaskRun>>,
    /// Smart tasks running in the background (tasks.rs).
    pub tasks: crate::tasks::Runner,
    /// The web sessions' windows and endpoint (ADR-035); `None` where no window can be opened.
    pub web: Option<std::sync::Arc<crate::websession::Service>>,
}

type Res<T> = Result<T, String>;

pub(crate) use crate::gateway::PORT_KEY;
const AUTOSTART_KEY: &str = "gateway.autostart";

impl AppState {
    pub fn port(&self) -> u16 {
        self.shared
            .with_db(|db| store::setting(db, PORT_KEY))
            .ok()
            .flatten()
            .and_then(|p| p.parse().ok())
            .unwrap_or(gateway::DEFAULT_PORT)
    }

    pub fn autostart(&self) -> bool {
        self.shared.with_db(|db| store::setting(db, AUTOSTART_KEY)).ok().flatten().as_deref() != Some("false")
    }

    pub async fn start(&self) -> Res<()> {
        let mut server = self.server.lock().await;
        if server.is_some() {
            return Ok(());
        }
        let port = self.port();
        match gateway::start(self.shared.clone(), port).await {
            Ok(started) => {
                *server = Some(started);
                *self.last_error.lock().unwrap_or_else(|p| p.into_inner()) = None;
                Ok(())
            }
            Err(e) => {
                let message = if e.kind() == std::io::ErrorKind::AddrInUse {
                    format!("Port {port} is already in use by another program")
                } else if e.kind() == std::io::ErrorKind::ResourceBusy {
                    // Another gateway (`gateway serve`) owns the store; this one waits until it stops.
                    e.to_string()
                } else {
                    format!("Could not listen on 127.0.0.1:{port}: {e}")
                };
                *self.last_error.lock().unwrap_or_else(|p| p.into_inner()) = Some(message.clone());
                Err(message)
            }
        }
    }

    pub async fn stop(&self) {
        if let Some(server) = self.server.lock().await.take() {
            server.stop().await;
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayStatus {
    /// `running`, `stopped` or `failed`.
    pub state: &'static str,
    pub port: u16,
    /// `http://127.0.0.1:<port>` while running.
    pub base_url: Option<String>,
    pub error: Option<String>,
    pub autostart: bool,
    /// Why the last ledger entry could not be written, while that is still the latest outcome.
    pub accounting_error: Option<String>,
    /// Stopped, but requests that were in flight are still finishing and keep the store (review LC-02).
    pub finishing: bool,
}

async fn status_of(state: &AppState) -> GatewayStatus {
    let server = state.server.lock().await;
    let error = state.last_error.lock().unwrap_or_else(|p| p.into_inner()).clone();
    GatewayStatus {
        state: if server.is_some() {
            "running"
        } else if error.is_some() {
            "failed"
        } else {
            "stopped"
        },
        port: server.as_ref().map(|s| s.addr.port()).unwrap_or_else(|| state.port()),
        base_url: server.as_ref().map(|s| format!("http://{}", s.addr)),
        error,
        autostart: state.autostart(),
        accounting_error: state.shared.accounting_error(),
        finishing: server.is_none() && state.shared.owns_store(),
    }
}

#[tauri::command]
pub async fn gateway_status(state: State<'_, AppState>) -> Res<GatewayStatus> {
    Ok(status_of(&state).await)
}

#[tauri::command]
pub async fn gateway_start(state: State<'_, AppState>) -> Res<GatewayStatus> {
    let _ = state.start().await;
    Ok(status_of(&state).await)
}

#[tauri::command]
pub async fn gateway_stop(state: State<'_, AppState>) -> Res<GatewayStatus> {
    state.stop().await;
    *state.last_error.lock().unwrap_or_else(|p| p.into_inner()) = None;
    Ok(status_of(&state).await)
}

#[tauri::command]
pub async fn gateway_configure(state: State<'_, AppState>, port: u16, autostart: bool) -> Res<GatewayStatus> {
    if port < 1024 {
        return Err("Choose a port from 1024 to 65535".into());
    }
    let changed = port != state.port();
    state.shared.with_db(|db| {
        store::set_setting(db, PORT_KEY, &port.to_string())?;
        store::set_setting(db, AUTOSTART_KEY, if autostart { "true" } else { "false" })
    })?;
    if changed && state.server.lock().await.is_some() {
        state.stop().await;
        let _ = state.start().await;
    }
    Ok(status_of(&state).await)
}

// ---------- Connections ----------

#[tauri::command]
pub fn presets_list() -> Vec<presets::Preset> {
    presets::PRESETS.to_vec()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionView {
    #[serde(flatten)]
    pub provider: Provider,
    pub preset_name: String,
    /// Its free part is a set of $0 models, so paid ones start off while it is set as free (ADR-032).
    pub free_models: bool,
    pub has_key: bool,
    pub needs_key: bool,
    pub models: usize,
    /// Current cooldowns and blocks on the connection or its models (ROADMAP P0-2).
    pub health: Vec<store::Health>,
    /// Other connections that share this one's quota group.
    pub shares_quota_with: Vec<String>,
    /// The rate-limit headers its provider last sent, per model (PV-06): as the provider said, never estimated.
    pub limits: Vec<store::RateLimits>,
    /// Its account's balance or free allowance, as its account endpoint last said (quota.rs).
    pub account: Option<gateway::quota::Account>,
    /// It has an account endpoint Pygg can read.
    pub reads_account: bool,
    /// A web session (ADR-035), and whether its warning was acknowledged in its current text.
    pub web: bool,
    pub web_ack_current: bool,
    /// A web session's daily limit (its own, or Pygg's default) and the requests sent today (migration 0026).
    pub web_daily_limit: Option<u32>,
    pub web_requests_today: Option<u64>,
    /// The web guard's view of a web session (ADR-035, migration 0028).
    pub web_guard: Option<WebGuardView>,
    /// The last check of its state (its model list, or a web session's sign-in), and its last 24 hours.
    pub last_check: Option<store::Check>,
    pub health_strip: store::HealthStrip,
    /// The site's assistant can act for the user (Muse).
    pub web_acts: bool,
    /// Its key ring (ADR-038): every key with its state, use today and stated room, without secrets; empty until it has
    /// a second key. How it orders them, and why it cannot have several keys, if it cannot.
    pub keys: Vec<gateway::keys::KeyView>,
    pub key_strategy: String,
    pub keys_refused: Option<String>,
}

/// What the web guard allows a web session now: its hour's and day's shares by weight, its rest and its back-off.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebGuardView {
    pub hour_units: u32,
    pub hour_requests: u32,
    pub hour_units_cap: u32,
    pub hour_requests_cap: u32,
    pub today_units: u32,
    pub today_units_cap: u32,
    pub max_prompt_chars: usize,
    /// Requests in the last `burst_window_min` minutes, and how many fit there.
    pub burst_requests: u32,
    pub burst_requests_cap: u32,
    pub burst_window_min: u32,
    /// The highest daily request limit this site may have, and the least rest after a reply, in seconds.
    pub daily_limit_max: u32,
    pub rest_base_secs: u32,
    /// Why this site's limits are lower than other sites', if they are.
    pub site_note: Option<String>,
    /// When the rest after the last reply ends, if it has not.
    pub resting_until: Option<i64>,
    /// Backed off after a sign of trouble: until when, and Pygg's reason.
    pub backoff_until: Option<i64>,
    pub backoff_reason: Option<String>,
}

fn web_guard_view(state: &AppState, provider: &Provider) -> Option<WebGuardView> {
    use crate::websession::guard;
    let now = crate::db::now_ms();
    let account = state.shared.with_db(|db| store::web_guard(db, &provider.id, now)).ok()?;
    let (hour_units, hour_requests) = account.hour(now);
    let limits = guard::limits(&provider.kind);
    let limit = limits.daily_limit(provider.web_daily_limit.unwrap_or(crate::websession::DEFAULT_DAILY_LIMIT));
    Some(WebGuardView {
        hour_units,
        hour_requests,
        hour_units_cap: limits.hour_units,
        hour_requests_cap: limits.hour_requests,
        today_units: account.today(now),
        today_units_cap: limit * guard::DAY_UNITS_PER_REQUEST,
        max_prompt_chars: limits.max_prompt_chars,
        burst_requests: account.burst(now, limits.burst_window_ms),
        burst_requests_cap: limits.burst_requests,
        burst_window_min: (limits.burst_window_ms / 60_000) as u32,
        daily_limit_max: limits.max_daily_limit,
        rest_base_secs: (limits.rest_base_ms / 1000) as u32,
        site_note: limits.note.map(str::to_string),
        resting_until: account.next_at.filter(|at| *at > now),
        backoff_until: account.backoff_until.filter(|at| *at > now),
        backoff_reason: account.backoff_until.filter(|at| *at > now).and(account.backoff_reason),
    })
}

fn view(state: &AppState, provider: Provider) -> ConnectionView {
    let preset = presets::find(&provider.kind);
    let has_key = provider.credential_ref.as_deref().map(|r| matches!(vault::read(r), Ok(Some(_)))).unwrap_or(false);
    let models = state.shared.with_db(|db| store::list_models(db, Some(&provider.id))).map(|m| m.len()).unwrap_or(0);
    let health = state.shared.with_db(|db| store::list_health(db, Some(&provider.id))).unwrap_or_default();
    let shares_quota_with = state
        .shared
        .with_db(store::list_providers)
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.id != provider.id && p.quota_group == provider.quota_group)
        .map(|p| p.label)
        .collect();
    let limits = state.shared.with_db(|db| store::list_limits(db, &provider.id)).unwrap_or_default();
    let account = state.shared.with_db(|db| gateway::quota::saved_account(db, &provider.id));
    let web = presets::is_web(&provider.kind);
    ConnectionView {
        web,
        web_ack_current: web && provider.web_ack_version == Some(crate::websession::WARNING_VERSION),
        web_daily_limit: web.then(|| {
            crate::websession::guard::limits(&provider.kind)
                .daily_limit(provider.web_daily_limit.unwrap_or(crate::websession::DEFAULT_DAILY_LIMIT))
        }),
        web_requests_today: if web {
            state.shared.with_db(|db| store::web_requests_today(db, &provider.id, crate::db::now_ms())).ok()
        } else {
            None
        },
        web_guard: if web { web_guard_view(state, &provider) } else { None },
        last_check: state.shared.with_db(|db| store::last_check(db, &provider.id)).ok().flatten(),
        health_strip: state
            .shared
            .with_db(|db| store::health_strip(db, &provider.id, crate::db::now_ms()))
            .unwrap_or_default(),
        web_acts: preset.is_some_and(|p| p.web_acts),
        keys: state.shared.with_db(|db| gateway::keys::views(db, &provider, crate::db::now_ms())).unwrap_or_default(),
        key_strategy: state
            .shared
            .with_db(|db| gateway::keys::strategy(db, &provider.id))
            .unwrap_or_else(|_| "in_order".into()),
        keys_refused: gateway::keys::can_have_keys(&provider).err(),
        limits,
        account,
        reads_account: gateway::quota::account_endpoint(&provider.kind).is_some(),
        preset_name: preset.map(|p| p.name.to_string()).unwrap_or_else(|| provider.kind.clone()),
        needs_key: preset.map(|p| p.needs_key).unwrap_or(false),
        free_models: preset.map(|p| p.free_models).unwrap_or(false),
        has_key,
        models,
        health,
        shares_quota_with,
        provider,
    }
}

#[tauri::command]
pub fn connections_list(state: State<'_, AppState>) -> Res<Vec<ConnectionView>> {
    let providers = state.shared.with_db(store::list_providers)?;
    Ok(providers.into_iter().map(|p| view(&state, p)).collect())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewConnection {
    pub preset: String,
    pub label: String,
    pub endpoint: Option<String>,
    pub api_key: Option<String>,
    pub billing: String,
    /// For a web session (ADR-035): the text of the warning the user acknowledged (`websession::WARNING_VERSION`).
    #[serde(default)]
    pub web_ack: Option<i64>,
}

#[tauri::command]
pub fn connection_add(state: State<'_, AppState>, connection: NewConnection) -> Res<ConnectionView> {
    add_connection(&state, connection)
}

/// Connects a provider by signing in on its own page in the browser, where it offers that (ADR-034, `oauth.rs`):
/// the provider issues an API key, kept like a typed one. Never a website session or cookie.
#[tauri::command]
pub async fn connection_sign_in(state: State<'_, AppState>, mut connection: NewConnection) -> Res<ConnectionView> {
    if !gateway::oauth::supports(&connection.preset) {
        return Err("This provider cannot be connected with the browser; enter its API key".into());
    }
    let key = gateway::oauth::openrouter_key(&state.shared.http).await?;
    connection.api_key = Some(key);
    add_connection(&state, connection)
}

fn add_connection(state: &AppState, connection: NewConnection) -> Res<ConnectionView> {
    let preset = presets::find(&connection.preset).ok_or("Unknown provider")?;
    // A web session (ADR-035): no key and no password, only the user's acknowledgement of the account risk. It starts
    // off; the user signs in in its window and turns it on.
    if preset.web {
        let ack = connection.web_ack.ok_or("Read and acknowledge the account-risk warning first")?;
        let provider = state.shared.with_db(|db| store::add_web_provider(db, preset.id, &connection.label, ack))?;
        return Ok(view(state, provider));
    }
    let endpoint = connection
        .endpoint
        .as_deref()
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .or(preset.endpoint)
        .ok_or("Enter the endpoint URL")?
        .to_string();
    let label = if connection.label.trim().is_empty() { preset.name } else { connection.label.trim() };
    let key = connection.api_key.as_deref().map(str::trim).filter(|k| !k.is_empty());
    if preset.needs_key && key.is_none() {
        return Err(format!("{} needs an API key", preset.name));
    }
    let reference = match key {
        Some(key) => {
            let reference = vault::new_reference();
            vault::store(&reference, key).map_err(|e| e.to_string())?;
            Some(reference)
        }
        None => None,
    };
    let added = state.shared.with_db(|db| {
        store::add_provider(
            db,
            &NewProvider {
                kind: preset.id,
                label,
                endpoint: &endpoint,
                credential_ref: reference.as_deref(),
                default_billing: &connection.billing,
                wire: preset.wire,
            },
        )
    });
    match added {
        Ok(provider) => Ok(view(state, provider)),
        Err(e) => {
            if let Some(reference) = &reference {
                let _ = vault::remove(reference);
            }
            Err(e)
        }
    }
}

#[tauri::command]
pub fn connection_update(
    state: State<'_, AppState>, id: String, enabled: Option<bool>, billing: Option<String>,
    separate_quota: Option<bool>,
) -> Res<ConnectionView> {
    state.shared.with_db(|db| store::update_provider(db, &id, enabled, billing.as_deref(), separate_quota))?;
    sync_opencode(&state);
    let provider = state.shared.with_db(|db| store::get_provider(db, &id))?.ok_or("Connection not found")?;
    Ok(view(&state, provider))
}

#[tauri::command]
pub fn connection_set_key(state: State<'_, AppState>, id: String, api_key: String) -> Res<ConnectionView> {
    let key = api_key.trim();
    if key.is_empty() {
        return Err("Enter the API key".into());
    }
    let provider = state.shared.with_db(|db| store::get_provider(db, &id))?.ok_or("Connection not found")?;
    match &provider.credential_ref {
        Some(reference) => vault::store(reference, key).map_err(|e| e.to_string())?,
        None => {
            let reference = vault::new_reference();
            vault::store(&reference, key).map_err(|e| e.to_string())?;
            state.shared.with_db(|db| {
                db.execute("UPDATE providers SET credential_ref = ?2 WHERE id = ?1", rusqlite::params![id, reference])
                    .map_err(|e| e.to_string())
            })?;
        }
    }
    // A new key deserves a fresh start: earlier rejections were about the old one.
    state.shared.with_db(|db| store::clear_health(db, &id))?;
    let provider = state.shared.with_db(|db| store::get_provider(db, &id))?.ok_or("Connection not found")?;
    Ok(view(&state, provider))
}

/// Clears cooldowns and blocks so the connection is tried again on the next request.
#[tauri::command]
pub fn connection_clear_health(state: State<'_, AppState>, id: String) -> Res<ConnectionView> {
    state.shared.with_db(|db| store::clear_health(db, &id))?;
    let provider = state.shared.with_db(|db| store::get_provider(db, &id))?.ok_or("Connection not found")?;
    Ok(view(&state, provider))
}

/// Records what the provider's terms say about training on what is sent (`no_training` needs a source).
#[tauri::command]
pub fn connection_set_terms(
    state: State<'_, AppState>, id: String, terms: String, source: Option<String>,
) -> Res<ConnectionView> {
    state.shared.with_db(|db| store::set_provider_terms(db, &id, &terms, source.as_deref()))?;
    let provider = state
        .shared
        .with_db(store::list_providers)?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or("That connection no longer exists")?;
    Ok(view(&state, provider))
}

#[tauri::command]
pub fn connection_remove(state: State<'_, AppState>, id: String) -> Res<()> {
    let profile = state.shared.with_db(|db| store::get_provider(db, &id))?.and_then(|p| p.web_profile);
    let references = state.shared.with_db(|db| store::remove_provider(db, &id))?;
    // A web session's window closes and its profile folder, with the sign-in, is deleted (ADR-035).
    if let Some(profile) = profile {
        match &state.web {
            Some(web) => web.forget(&id, &profile),
            None => {
                crate::websession::remove_profile(&state.data_dir.join("websessions"), &profile);
            }
        }
    }
    sync_opencode(&state);
    for reference in references {
        vault::remove(&reference).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub listed: usize,
    pub added: usize,
}

/// Tests the connection by asking for its models, and adds the ones not known yet.
#[tauri::command]
pub async fn connection_refresh_models(state: State<'_, AppState>, id: String) -> Res<ImportResult> {
    let provider = state.shared.with_db(|db| store::get_provider(db, &id))?.ok_or("Connection not found")?;
    // A web session's models are its recipe's; the site has no list to ask (ADR-035).
    if presets::is_web(&provider.kind) {
        let listed = crate::websession::recipe(&provider.kind).map(|r| r.models.len()).unwrap_or(0);
        return Ok(ImportResult { listed, added: 0 });
    }
    // Reading the list is the connection's check too (migration 0029), in its turn with the other checks.
    let ids = gateway::checker::list_and_check(&state.shared, &provider).await?;
    let source = catalog::list_source(&provider);
    let added = state.shared.with_db(|db| store::import_listed(db, &id, &ids, Some(&source)))?;
    // What the provider's list left out is looked up (ADR-031): the user only enters a key.
    let cache = gateway::reference::cache_path(&state.data_dir.join("store.sqlite3"));
    if let Some(catalog) = gateway::reference::current(&state.shared.http, &cache).await {
        state.shared.with_db(|db| {
            let _ = gateway::reference::apply(db, &catalog);
            gateway::reference::mark_applied(db);
        });
    }
    sync_opencode(&state);
    if let Ok(Some(account)) = gateway::quota::fetch_account(&state.shared, &provider).await {
        let _ = state.shared.with_db(|db| gateway::quota::save_account(db, &id, &account));
    }
    Ok(ImportResult { listed: ids.len(), added })
}

/// What routing saved over the last `days`, against the model the user chose (gateway/savings.rs).
#[tauri::command]
pub fn usage_savings(state: State<'_, AppState>, days: u32, offset_minutes: i32) -> Res<gateway::savings::Savings> {
    let since = crate::db::now_ms() - i64::from(days.clamp(1, 366)) * 86_400_000;
    // The trend's periods as the usage chart's: hours for one day, days beyond, on the user's clock.
    let bucket = if days <= 1 { 3_600_000 } else { 86_400_000 };
    let offset = offset_minutes.clamp(-14 * 60, 14 * 60) as i64 * 60_000;
    state.shared.with_db(|db| gateway::savings::savings(db, since, bucket, offset))
}

/// The model to compare with, as `connection/model`, or none.
#[tauri::command]
pub fn usage_set_savings_baseline(state: State<'_, AppState>, baseline: Option<String>) -> Res<()> {
    state.shared.with_db(|db| gateway::savings::set_baseline(db, baseline.as_deref()))
}

/// The defaults for new API keys (Policies).
#[tauri::command]
pub fn client_defaults(state: State<'_, AppState>) -> Res<store::ClientDefaults> {
    state.shared.with_db(store::client_defaults)
}

#[tauri::command]
pub fn client_set_defaults(state: State<'_, AppState>, defaults: store::ClientDefaults) -> Res<()> {
    state.shared.with_db(|db| store::set_client_defaults(db, &defaults))
}

/// The newest request of each API key (CLI Tools' "Working").
#[tauri::command]
pub fn client_latest_requests(state: State<'_, AppState>) -> Res<Vec<store::LatestRequest>> {
    state.shared.with_db(store::latest_requests)
}

/// The Claude Code launcher (launcher.rs) and the command that starts it.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherInfo {
    #[serde(flatten)]
    pub state: crate::launcher::State,
    /// The console program shipped next to the app, when it is there; `pygg` from npm works the same.
    pub cli_path: Option<String>,
}

fn launcher_info(state: crate::launcher::State) -> LauncherInfo {
    let cli_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("pygg-cli.exe")))
        .filter(|p| p.is_file())
        .map(|p| p.to_string_lossy().into_owned());
    LauncherInfo { state, cli_path }
}

#[tauri::command]
pub fn launcher_state(state: State<'_, AppState>) -> Res<LauncherInfo> {
    Ok(launcher_info(state.shared.with_db(crate::launcher::state)))
}

#[tauri::command]
pub fn launcher_set_up(state: State<'_, AppState>) -> Res<LauncherInfo> {
    state.shared.with_db(|db| crate::launcher::set_up(db, crate::db::now_ms())).map(launcher_info)
}

#[tauri::command]
pub fn launcher_remove(state: State<'_, AppState>) -> Res<LauncherInfo> {
    state.shared.with_db(crate::launcher::remove).map(launcher_info)
}

/// Shows a web session's window (ADR-035), where the user signs in on the site's own page, or deals with a check.
/// Async on purpose: a synchronous command runs on the main thread, and building a WebView2 window there waits for
/// that same thread, so the window never opened and the button kept spinning (found by the owner, 2026-11-12).
#[tauri::command]
pub async fn web_session_open(state: State<'_, AppState>, id: String) -> Res<()> {
    state.web.as_ref().ok_or(crate::websession::APP_ONLY)?.show(&id)
}

/// Opens a new chat in a web session's window to see whether it is signed in, unless a request is using it, and returns
/// the connection with what was seen (ADR-035).
#[tauri::command]
pub async fn web_session_check(state: State<'_, AppState>, id: String) -> Res<ConnectionView> {
    if let Some(why) = check_web(&state, &id).await? {
        return Err(why);
    }
    let provider = state.shared.with_db(|db| store::get_provider(db, &id))?.ok_or("Connection not found")?;
    Ok(view(&state, provider))
}

/// A web session's sign-in check, kept as the connection's check: it loads the site's page and sends no prompt. A check
/// the web guard or a running request held back is not one, and returns why (Codex's fourth review, A6).
async fn check_web(state: &AppState, id: &str) -> Res<Option<String>> {
    if let crate::websession::Checked::Skipped(why) =
        state.web.as_ref().ok_or(crate::websession::APP_ONLY)?.check(id).await?
    {
        return Ok(Some(why));
    }
    let provider = state.shared.with_db(|db| store::get_provider(db, id))?.ok_or("Connection not found")?;
    let ok = provider.web_state.as_deref() == Some("signed_in");
    let detail = provider.web_state_reason.unwrap_or_else(|| crate::websession::SIGNED_IN.into());
    state.shared.with_db(|db| store::record_check(db, id, ok, "sign_in", &detail))?;
    Ok(None)
}

/// Checks one connection now: an API connection's model list (free, no prompt, nothing imported), or a web session's
/// sign-in (its page loads; no prompt).
#[tauri::command]
pub async fn connection_check(state: State<'_, AppState>, id: String) -> Res<ConnectionView> {
    let provider = state.shared.with_db(|db| store::get_provider(db, &id))?.ok_or("Connection not found")?;
    if presets::is_web(&provider.kind) {
        if let Some(why) = check_web(&state, &id).await? {
            return Err(why);
        }
    } else {
        gateway::checker::check_api(&state.shared, &provider).await;
    }
    let provider = state.shared.with_db(|db| store::get_provider(db, &id))?.ok_or("Connection not found")?;
    Ok(view(&state, provider))
}

/// What Check All did.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckAllResult {
    pub checked: usize,
    pub failed: usize,
    /// Web sessions, left alone: each is checked on its own (Check Sign-In), so no site sees a program loading pages
    /// (Codex's fourth review, A7).
    pub web_skipped: usize,
}

/// Checks every enabled API connection now by its model list, a few at a time (`checker::check_many`). Web sessions are
/// left out; each has its own Check Sign-In.
#[tauri::command]
pub async fn connections_check_all(state: State<'_, AppState>) -> Res<CheckAllResult> {
    let providers: Vec<store::Provider> =
        state.shared.with_db(store::list_providers)?.into_iter().filter(|p| p.enabled).collect();
    let web_skipped = providers.iter().filter(|p| presets::is_web(&p.kind)).count();
    let api: Vec<store::Provider> = providers.into_iter().filter(|p| !presets::is_web(&p.kind)).collect();
    let checks = gateway::checker::check_many(&state.shared, api).await;
    Ok(CheckAllResult { checked: checks.len(), failed: checks.iter().filter(|c| !c.ok).count(), web_skipped })
}

/// Lifts a web session's rest and back-off now (Providers → Resume Now); the hour's and the day's shares stay.
#[tauri::command]
pub fn web_session_resume(state: State<'_, AppState>, id: String) -> Res<()> {
    state.shared.with_db(|db| store::web_guard_resume(db, &id))
}

/// A web session's daily request limit; `None` returns it to Pygg's default.
#[tauri::command]
pub fn web_session_set_daily_limit(state: State<'_, AppState>, id: String, limit: Option<u32>) -> Res<()> {
    state.shared.with_db(|db| store::set_web_daily_limit(db, &id, limit))
}

/// How a connection's models have answered over the last 30 days (SCREENS §5.4), from the ledger.
#[tauri::command]
pub fn connection_reliability(state: State<'_, AppState>, id: String) -> Res<Vec<store::Reliability>> {
    state.shared.with_db(|db| store::reliability(db, &id, crate::db::now_ms() - 30 * 86_400_000))
}

/// Sends one short request to a model through the gateway's own routing and ledger (Test Model, SCREENS §6.2).
#[tauri::command]
pub async fn model_test(
    state: State<'_, AppState>, provider_id: String, model_id: String,
) -> Res<gateway::model_test::TestResult> {
    gateway::model_test::run(&state.shared, &provider_id, &model_id).await
}

/// Sends one short request to a combo by its name, through its real route (Combos → Test). The member that answered,
/// and how many calls it took, come back; the request is recorded in Usage like any other.
#[tauri::command]
pub async fn combo_test(state: State<'_, AppState>, name: String) -> Res<gateway::model_test::TestResult> {
    gateway::model_test::run_requested(&state.shared, &name).await
}

// ---------- Key rings (ADR-038) ----------

/// Adds a key from another account to the connection's key ring. The secret goes to the system's credential store;
/// the store keeps only its reference. A key already on the connection is refused (compared in memory, never logged).
#[tauri::command]
pub fn connection_key_add(
    state: State<'_, AppState>, id: String, label: String, api_key: String, separate_account: bool,
) -> Res<ConnectionView> {
    let key = api_key.trim();
    if key.is_empty() {
        return Err("Enter the API key".into());
    }
    let provider = state.shared.with_db(|db| store::get_provider(db, &id))?.ok_or("Connection not found")?;
    gateway::keys::can_have_keys(&provider)?;
    if !separate_account {
        return Err(gateway::keys::SEPARATE_ACCOUNT.into());
    }
    let mut existing = state.shared.with_db(|db| gateway::keys::refs(db, &id))?;
    existing.extend(provider.credential_ref.clone());
    if existing.iter().any(|reference| vault::read(reference).ok().flatten().as_deref() == Some(key)) {
        return Err("This key is already on this connection.".into());
    }
    let reference = vault::new_reference();
    vault::store(&reference, key).map_err(|e| e.to_string())?;
    if let Err(e) =
        state.shared.with_db(|db| gateway::keys::add(db, &id, &label, &reference, true, crate::db::now_ms()))
    {
        let _ = vault::remove(&reference);
        return Err(e);
    }
    let provider = state.shared.with_db(|db| store::get_provider(db, &id))?.ok_or("Connection not found")?;
    Ok(view(&state, provider))
}

/// Removes a key from its connection's ring and deletes its secret.
#[tauri::command]
pub fn connection_key_remove(state: State<'_, AppState>, key_id: String) -> Res<()> {
    let reference = state.shared.with_db(|db| gateway::keys::remove(db, &key_id))?;
    vault::remove(&reference).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn connection_key_move(state: State<'_, AppState>, key_id: String, up: bool) -> Res<()> {
    state.shared.with_db(|db| gateway::keys::move_key(db, &key_id, up))
}

#[tauri::command]
pub fn connection_key_enable(state: State<'_, AppState>, key_id: String, enabled: bool) -> Res<()> {
    state.shared.with_db(|db| gateway::keys::set_enabled(db, &key_id, enabled))
}

/// How the connection orders its keys for a new request: in_order, round_robin, least_used or quota_aware.
#[tauri::command]
pub fn connection_key_strategy(state: State<'_, AppState>, id: String, strategy: String) -> Res<ConnectionView> {
    state.shared.with_db(|db| gateway::keys::set_strategy(db, &id, &strategy))?;
    let provider = state.shared.with_db(|db| store::get_provider(db, &id))?.ok_or("Connection not found")?;
    Ok(view(&state, provider))
}

/// Puts a connection in a pool, or takes it out with `None` (ADR-034).
#[tauri::command]
pub fn connection_set_pool(state: State<'_, AppState>, id: String, pool: Option<String>) -> Res<ConnectionView> {
    state.shared.with_db(|db| store::set_pool(db, &id, pool.as_deref()))?;
    let provider = state.shared.with_db(|db| store::get_provider(db, &id))?.ok_or("Connection not found")?;
    Ok(view(&state, provider))
}

/// Moves a connection earlier or later in its pool's order.
#[tauri::command]
pub fn connection_move_in_pool(state: State<'_, AppState>, id: String, up: bool) -> Res<()> {
    state.shared.with_db(|db| store::move_in_pool(db, &id, up))
}

/// Reads the connection's account endpoint now (balance, free allowance) and keeps what it says.
#[tauri::command]
pub async fn connection_check_account(state: State<'_, AppState>, id: String) -> Res<ConnectionView> {
    let provider = state.shared.with_db(|db| store::get_provider(db, &id))?.ok_or("Connection not found")?;
    if let Some(account) = gateway::quota::fetch_account(&state.shared, &provider).await? {
        state.shared.with_db(|db| gateway::quota::save_account(db, &id, &account))?;
    }
    Ok(view(&state, provider))
}

// ---------- Models ----------

#[tauri::command]
pub fn models_list(state: State<'_, AppState>, provider_id: Option<String>) -> Res<Vec<Model>> {
    state.shared.with_db(|db| store::list_models(db, provider_id.as_deref()))
}

/// Sets what auto routing knows about a model, as the user's statement.
#[tauri::command]
pub fn model_set_capabilities(
    state: State<'_, AppState>, provider_id: String, model_id: String, capabilities: store::Capabilities,
) -> Res<()> {
    state.shared.with_db(|db| store::set_capabilities(db, &provider_id, &model_id, &capabilities))?;
    sync_opencode(&state);
    Ok(())
}

/// What `auto` or `auto/cheap` would choose for a request with these needs, from a client's point of view,
/// with every excluded model and why. Sends nothing (ROADMAP P0-5).
#[tauri::command]
pub fn auto_simulate(
    state: State<'_, AppState>, client_id: Option<String>, alias: String, needs: gateway::auto::Needs,
) -> Res<gateway::auto::Plan> {
    let strategy = gateway::auto::strategy(&alias).ok_or_else(|| format!("{alias} is not an automatic route"))??;
    state.shared.with_db(|db| {
        let clients = store::list_clients(db)?;
        let client = match &client_id {
            Some(id) => clients.into_iter().find(|c| &c.id == id).ok_or("Client not found")?,
            // No client chosen: the defaults a new client gets.
            None => Client {
                id: String::new(),
                name: String::new(),
                token_prefix: String::new(),
                monthly_budget_micro_usd: None,
                created_at: 0,
                last_used_at: None,
                revoked_at: None,
                month_spend_micro_usd: 0,
                month_unknown_requests: 0,
                max_concurrency: 8,
                unknown_cost_policy: "block".into(),
                reserved_micro_usd: 0,
                month_at_risk_micro_usd: 0,
                allowed_connections: None,
                paid_retries: 0,
                auto_min_tier: "T2".into(),
                may_run_tasks: false,
                require_no_training: false,
                strict_budget: false,
                token_viewable: false,
                session_budget_micro_usd: None,
                day_budget_micro_usd: None,
                day_counted_micro_usd: 0,
            },
        };
        gateway::auto::plan(db, &client, &alias, strategy, &needs)
    })
}

#[tauri::command]
pub fn model_add(state: State<'_, AppState>, provider_id: String, model_id: String) -> Res<()> {
    state.shared.with_db(|db| store::add_model(db, &provider_id, &model_id))?;
    sync_opencode(&state);
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceInput {
    pub input_usd_per_mtok: f64,
    pub output_usd_per_mtok: f64,
    pub cached_input_usd_per_mtok: Option<f64>,
    pub cache_write_usd_per_mtok: Option<f64>,
}

fn micro(usd: f64) -> Res<u64> {
    if usd.is_finite() && (0.0..10_000.0).contains(&usd) {
        Ok((usd * 1_000_000.0).round() as u64)
    } else {
        Err("Enter a price from 0 to 10,000 USD per million tokens".into())
    }
}

#[tauri::command]
pub fn model_update(
    state: State<'_, AppState>, provider_id: String, model_id: String, enabled: Option<bool>, billing: Option<String>,
    price: Option<PriceInput>, clear_price: Option<bool>,
) -> Res<()> {
    let price = match (price, clear_price) {
        (_, Some(true)) => Some(None),
        (Some(p), _) => Some(Some(Price {
            input_micro_per_mtok: micro(p.input_usd_per_mtok)?,
            output_micro_per_mtok: micro(p.output_usd_per_mtok)?,
            cached_input_micro_per_mtok: p.cached_input_usd_per_mtok.map(micro).transpose()?,
            cache_write_micro_per_mtok: p.cache_write_usd_per_mtok.map(micro).transpose()?,
            source: "user".into(),
            observed_at: crate::db::now_ms(),
        })),
        (None, _) => None,
    };
    state.shared.with_db(|db| store::update_model(db, &provider_id, &model_id, enabled, billing.as_deref(), price))?;
    sync_opencode(&state);
    Ok(())
}

#[tauri::command]
pub fn model_remove(state: State<'_, AppState>, provider_id: String, model_id: String) -> Res<()> {
    state.shared.with_db(|db| store::remove_model(db, &provider_id, &model_id))?;
    sync_opencode(&state);
    Ok(())
}

// ---------- Combos ----------

#[tauri::command]
pub fn combos_list(state: State<'_, AppState>) -> Res<Vec<Combo>> {
    state.shared.with_db(store::list_combos)
}

#[tauri::command]
pub fn combo_save(state: State<'_, AppState>, id: Option<String>, name: String, members: Vec<Target>) -> Res<Combo> {
    state.shared.with_db(|db| store::save_combo(db, id.as_deref(), &name, &members))
}

/// `order` or `cheapest` (migration 0025).
#[tauri::command]
pub fn combo_set_strategy(state: State<'_, AppState>, id: String, strategy: String) -> Res<()> {
    state.shared.with_db(|db| store::set_combo_strategy(db, &id, &strategy))
}

#[tauri::command]
pub fn combo_delete(state: State<'_, AppState>, id: String) -> Res<()> {
    state.shared.with_db(|db| store::delete_combo(db, &id))
}

// ---------- Clients ----------

#[tauri::command]
pub fn clients_list(state: State<'_, AppState>) -> Res<Vec<Client>> {
    state.shared.with_db(store::list_clients)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedClient {
    pub client: Client,
    /// Shown once; only its hash is kept.
    pub token: String,
}

fn budget(usd: Option<f64>) -> Res<Option<u64>> {
    match usd {
        None => Ok(None),
        Some(v) if v.is_finite() && v > 0.0 && v <= 100_000.0 => Ok(Some((v * 1_000_000.0).round() as u64)),
        Some(_) => Err("Enter a limit above $0".into()),
    }
}

#[tauri::command]
pub fn client_create(state: State<'_, AppState>, name: String, monthly_budget_usd: Option<f64>) -> Res<CreatedClient> {
    let limit = budget(monthly_budget_usd)?;
    let (mut client, token) = state.shared.with_db(|db| store::create_client(db, &name, limit))?;
    // A copy in the credential store, so the key can be shown again; without one it is shown this once only.
    let reference = crate::vault::new_client_reference();
    if crate::vault::store(&reference, &token).is_ok() {
        state.shared.with_db(|db| store::set_client_token_ref(db, &client.id, Some(&reference)))?;
        client.token_viewable = true;
    }
    Ok(CreatedClient { client, token })
}

/// The token of a key, from its copy in the credential store (Endpoint → API Keys → Show).
#[tauri::command]
pub fn client_token(state: State<'_, AppState>, id: String) -> Res<String> {
    let reference = state.shared.with_db(|db| store::client_token_ref(db, &id))?.ok_or(
        "This key was made before Pygg kept a copy, or it was revoked, so it can't be shown. Create a new one.",
    )?;
    crate::vault::read(&reference)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "The key's copy is missing from the credential store. Create a new key.".into())
}

#[tauri::command]
pub fn client_revoke(state: State<'_, AppState>, id: String) -> Res<()> {
    // A revoked key's copy is of no use to anyone: it goes with it.
    let reference = state.shared.with_db(|db| store::client_token_ref(db, &id))?;
    state.shared.with_db(|db| store::revoke_client(db, &id))?;
    if let Some(reference) = reference {
        let _ = crate::vault::remove(&reference);
        state.shared.with_db(|db| store::set_client_token_ref(db, &id, None))?;
    }
    Ok(())
}

/// Takes a revoked key off the list (its past requests keep its name in Usage).
#[tauri::command]
pub fn client_delete(state: State<'_, AppState>, id: String) -> Res<()> {
    if let Some(reference) = state.shared.with_db(|db| store::delete_client(db, &id))? {
        let _ = crate::vault::remove(&reference);
    }
    Ok(())
}

/// Deletes every revoked key; returns how many.
#[tauri::command]
pub fn client_delete_revoked(state: State<'_, AppState>) -> Res<u32> {
    let ids = state.shared.with_db(store::revoked_client_ids)?;
    for id in &ids {
        if let Some(reference) = state.shared.with_db(|db| store::delete_client(db, id))? {
            let _ = crate::vault::remove(&reference);
        }
    }
    Ok(ids.len() as u32)
}

#[tauri::command]
pub fn client_set_budget(state: State<'_, AppState>, id: String, monthly_budget_usd: Option<f64>) -> Res<()> {
    let limit = budget(monthly_budget_usd)?;
    state.shared.with_db(|db| store::set_client_budget(db, &id, limit))
}

/// The limits per routing session and per UTC day (cost plan A3); `None` turns one off.
#[tauri::command]
pub fn client_set_limits(
    state: State<'_, AppState>, id: String, session_budget_usd: Option<f64>, day_budget_usd: Option<f64>,
) -> Res<()> {
    let (session, day) = (budget(session_budget_usd)?, budget(day_budget_usd)?);
    state.shared.with_db(|db| store::set_client_limits(db, &id, session, day))
}

/// Requests allowed in flight at once, and whether a target with an unknown price may be used while the
/// client has a limit (`block` or `allow`).
#[tauri::command]
pub fn client_set_policy(
    state: State<'_, AppState>, id: String, max_concurrency: Option<u32>, unknown_cost_policy: Option<String>,
    restrict_connections: Option<bool>, allowed_connections: Option<Vec<String>>, paid_retries: Option<u32>,
    auto_min_tier: Option<String>, may_run_tasks: Option<bool>, require_no_training: Option<bool>,
    strict_budget: Option<bool>,
) -> Res<()> {
    state.shared.with_db(|db| -> store::Result<()> {
        store::set_client_policy(db, &id, max_concurrency, unknown_cost_policy.as_deref())?;
        if let Some(tier) = &auto_min_tier {
            store::set_client_auto_min_tier(db, &id, tier)?;
        }
        if let Some(restrict) = restrict_connections {
            let list = allowed_connections.unwrap_or_default();
            store::set_client_connections(db, &id, restrict.then_some(list.as_slice()))?;
        }
        if let Some(retries) = paid_retries {
            store::set_client_paid_retries(db, &id, retries)?;
        }
        if let Some(allowed) = may_run_tasks {
            store::set_client_may_run_tasks(db, &id, allowed)?;
        }
        if let Some(required) = require_no_training {
            store::set_client_require_no_training(db, &id, required)?;
        }
        if let Some(strict) = strict_budget {
            store::set_client_strict_budget(db, &id, strict)?;
        }
        Ok(())
    })?;
    Ok(())
}

// ---------- Usage ----------

#[tauri::command]
pub fn usage_summary(state: State<'_, AppState>, days: u32) -> Res<UsageSummary> {
    let days = days.clamp(1, 366) as i64;
    let since = crate::db::now_ms() - days * 86_400_000;
    state.shared.with_db(|db| store::usage_summary(db, since, 200))
}

/// Usage → Requests, a page at a time: `limit` requests of the last `days` from `offset`, newest first.
#[tauri::command]
pub fn usage_requests_matching(
    state: State<'_, AppState>, days: u32, filter: store::RequestFilter, offset: u32, limit: u32,
) -> Res<store::RequestPage> {
    let since = crate::db::now_ms() - days.clamp(1, 366) as i64 * 86_400_000;
    state
        .shared
        .with_db(|db| store::usage_requests_matching(db, since, &filter, offset as usize, limit.clamp(1, 500) as usize))
}

/// A page of the period's requests, newest first.
#[tauri::command]
pub fn usage_requests(state: State<'_, AppState>, days: u32, offset: u32, limit: u32) -> Res<Vec<store::UsageEvent>> {
    let days = days.clamp(1, 366) as i64;
    let since = crate::db::now_ms() - days * 86_400_000;
    state.shared.with_db(|db| store::usage_requests(db, since, offset as usize, limit.clamp(1, 500) as usize))
}

/// Usage → Routing: requests answered after a fallback, routing sessions, and changes of model inside a session.
#[tauri::command]
pub fn usage_routing(state: State<'_, AppState>, days: u32) -> Res<store::RoutingSummary> {
    let days = days.clamp(1, 366) as i64;
    let since = crate::db::now_ms() - days * 86_400_000;
    state.shared.with_db(|db| store::routing_summary(db, since))
}

/// Usage → Token Mix: where the period's tokens went, and the known cost split among them (Codex's fifth discussion, T1).
#[tauri::command]
pub fn usage_token_mix(state: State<'_, AppState>, days: u32) -> Res<store::TokenMix> {
    let days = days.clamp(1, 366) as i64;
    let since = crate::db::now_ms() - days * 86_400_000;
    state.shared.with_db(|db| store::token_mix(db, since))
}

/// Usage → Subscriptions: each subscription connection's requests and tokens over the last `days`, and its stated limits.
#[tauri::command]
pub fn usage_subscriptions(state: State<'_, AppState>, days: u32) -> Res<Vec<store::SubscriptionUsage>> {
    let since = crate::db::now_ms() - days.clamp(1, 366) as i64 * 86_400_000;
    state.shared.with_db(|db| store::subscription_usage(db, since))
}

/// Usage → Local runtime: each local connection's models over the last `days`, with requests, tokens and median time.
#[tauri::command]
pub fn usage_local(state: State<'_, AppState>, days: u32) -> Res<Vec<store::LocalUsage>> {
    let since = crate::db::now_ms() - days.clamp(1, 366) as i64 * 86_400_000;
    state.shared.with_db(|db| store::local_usage(db, since))
}

/// How long usage history is kept (Settings → Data), and what the last clean-up deleted.
#[tauri::command]
pub fn retention_get(state: State<'_, AppState>) -> Res<crate::retention::Retention> {
    state.shared.with_db(crate::retention::view)
}

/// Keeps usage history for 30, 90 or 365 days, or forever (`None`); what is older goes now, then once a day.
#[tauri::command]
pub fn retention_set(state: State<'_, AppState>, days: Option<u32>) -> Res<crate::retention::Retention> {
    state.shared.with_db(|db| {
        crate::retention::set_days(db, days)?;
        crate::retention::prune(db, crate::db::now_ms())?;
        crate::retention::view(db)
    })
}

/// Skills & Prompts (ADR-039): the library, with where each skill is installed.
#[tauri::command]
pub fn skills_list(state: State<'_, AppState>) -> Res<Vec<crate::skills::Skill>> {
    let homes = crate::skills::Homes::from_env();
    state.shared.with_db(|db| crate::skills::library(db, &homes))
}

/// Saves one of the user's skills, new or edited.
#[tauri::command]
pub fn skills_save(state: State<'_, AppState>, skill: crate::skills::SkillEdit) -> Res<crate::skills::Skill> {
    let homes = crate::skills::Homes::from_env();
    state.shared.with_db(|db| crate::skills::save(db, &homes, &skill))
}

/// Deletes one of the user's skills once no installed copy is left.
#[tauri::command]
pub fn skills_delete(state: State<'_, AppState>, id: String) -> Res<()> {
    let homes = crate::skills::Homes::from_env();
    state.shared.with_db(|db| crate::skills::delete(db, &homes, &id))
}

/// Writes a skill into an agent's skills folder (`claude-code` or `agents`), globally or in a project the user added.
#[tauri::command]
pub fn skills_install(
    state: State<'_, AppState>, id: String, target: String, project: Option<String>,
) -> Res<crate::skills::Skill> {
    let homes = crate::skills::Homes::from_env();
    let target = crate::skills::Target::parse(&target)?;
    state.shared.with_db(|db| crate::skills::install(db, &homes, &id, target, project.as_deref()))
}

/// Removes Pygg's unchanged copy of a skill from an agent's skills folder, globally or in a project.
#[tauri::command]
pub fn skills_uninstall(
    state: State<'_, AppState>, id: String, target: String, project: Option<String>,
) -> Res<crate::skills::Skill> {
    let homes = crate::skills::Homes::from_env();
    let target = crate::skills::Target::parse(&target)?;
    state.shared.with_db(|db| crate::skills::uninstall(db, &homes, &id, target, project.as_deref()))
}

/// The project folders added for skills, with what each agent adds to every request there.
#[tauri::command]
pub fn skills_projects(state: State<'_, AppState>) -> Res<Vec<crate::skills::Project>> {
    let homes = crate::skills::Homes::from_env();
    state.shared.with_db(|db| crate::skills::projects(db, &homes))
}

/// Adds a project folder for skills.
#[tauri::command]
pub fn skills_add_project(state: State<'_, AppState>, path: String) -> Res<Vec<crate::skills::Project>> {
    let homes = crate::skills::Homes::from_env();
    state.shared.with_db(|db| crate::skills::add_project(db, &homes, &path))
}

/// Forgets a project folder once Pygg has no copy of a skill left in it.
#[tauri::command]
pub fn skills_remove_project(state: State<'_, AppState>, path: String) -> Res<Vec<crate::skills::Project>> {
    let homes = crate::skills::Homes::from_env();
    state.shared.with_db(|db| crate::skills::remove_project(db, &homes, &path))
}

/// What each agent sends with every request from its global instruction files, skills and commands.
#[tauri::command]
pub fn skills_context(state: State<'_, AppState>) -> Res<Vec<crate::skills::AgentContext>> {
    let homes = crate::skills::Homes::from_env();
    state.shared.with_db(|db| crate::skills::context(db, &homes))
}

/// Copies a skill found in an agent's folder into the library.
#[tauri::command]
pub fn skills_import(state: State<'_, AppState>, path: String) -> Res<crate::skills::Skill> {
    let homes = crate::skills::Homes::from_env();
    state.shared.with_db(|db| crate::skills::import(db, &homes, &path))
}

/// How many ledger rows keeping `days` days would delete now, for the confirmation; nothing is deleted.
#[tauri::command]
pub fn retention_preview(state: State<'_, AppState>, days: u32) -> Res<u64> {
    state.shared.with_db(|db| crate::retention::would_delete(db, days, crate::db::now_ms()))
}

/// Repeated tool output in the period's requests (P1-4, shadow): measured, never removed.
#[tauri::command]
pub fn usage_shadow(state: State<'_, AppState>, days: u32) -> Res<store::ShadowSummary> {
    let since = crate::db::now_ms() - days.clamp(1, 366) as i64 * 86_400_000;
    state.shared.with_db(|db| store::shadow_summary(db, since))
}

/// What in the ledger is worth a look now: limits near or passed, a spend spike, unknown costs, a burst of failures.
#[tauri::command]
pub fn usage_alerts(state: State<'_, AppState>) -> Res<Vec<store::UsageAlert>> {
    state.shared.with_db(|db| store::usage_alerts(db, crate::db::now_ms()))
}

/// Every request of the period as CSV, saved where the user picks; `None` when they cancel.
#[tauri::command]
pub async fn usage_export(app: tauri::AppHandle, state: State<'_, AppState>, days: u32) -> Res<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let days = days.clamp(1, 366) as i64;
    let now = crate::db::now_ms();
    // A byte-order mark, so Excel reads the file as UTF-8.
    let csv = format!("\u{feff}{}", state.shared.with_db(|db| store::usage_csv(db, now - days * 86_400_000))?);
    let name = format!("pygg-usage-{}.csv", &store::iso_utc(now)[..10]);
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().set_title("Export Usage").set_file_name(&name).add_filter("CSV", &["csv"]).save_file(
        move |file| {
            let _ = tx.send(file);
        },
    );
    let Some(file) = rx.await.map_err(|e| e.to_string())? else { return Ok(None) };
    let path = file.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, csv).map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// The diagnostics bundle (diagnostics.rs), saved where the user picks; `None` when they cancel.
#[tauri::command]
pub async fn diagnostics_export(app: tauri::AppHandle, state: State<'_, AppState>) -> Res<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let now = crate::db::now_ms();
    let gateway = status_of(&state).await;
    let context = crate::diagnostics::Context {
        made_by: "app",
        gateway_state: gateway.state.to_string(),
        gateway_port: gateway.port,
        gateway_error: gateway.error.or(gateway.accounting_error),
        log_lines: crate::applog::tail(200),
    };
    let zip = state.shared.with_db(|db| crate::diagnostics::bundle(db, &context, now))?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Save Diagnostics")
        .set_file_name(crate::diagnostics::file_name(now))
        .add_filter("Zip", &["zip"])
        .save_file(move |file| {
            let _ = tx.send(file);
        });
    let Some(file) = rx.await.map_err(|e| e.to_string())? else { return Ok(None) };
    let path = file.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, zip).map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// Usage per hour for one day, per day for longer, on the user's clock (`offset_minutes` east of UTC).
#[tauri::command]
pub fn usage_over_time(state: State<'_, AppState>, days: u32, offset_minutes: i32) -> Res<Vec<store::UsageBucket>> {
    let days = days.clamp(1, 366) as i64;
    let bucket = if days <= 1 { 3_600_000 } else { 86_400_000 };
    let offset = offset_minutes.clamp(-14 * 60, 14 * 60) as i64 * 60_000;
    // Whole periods: the oldest one starts on the boundary at or before the period's start.
    let since = ((crate::db::now_ms() - days * 86_400_000 + offset) / bucket) * bucket - offset;
    state.shared.with_db(|db| store::usage_over_time(db, since, bucket, offset))
}

// ---------- Agents (official agent CLIs with their own sign-in) ----------

#[tauri::command]
pub async fn agents_detect() -> Res<Vec<crate::agents::Detected>> {
    Ok(crate::agents::detect().await)
}

/// Runs a task through an agent CLI in a worktree of the workspace, records it in the ledger, and keeps the run
/// until it is kept as a branch or discarded.
#[tauri::command]
pub async fn agent_run(state: State<'_, AppState>, request: crate::agents::TaskRequest) -> Res<crate::agents::TaskRun> {
    let worktrees = state.data_dir.join("worktrees");
    let run = crate::agents::run(&request, &worktrees, std::future::pending()).await?;
    state.shared.with_db(|db| crate::agents::record(db, &run))?;
    state.runs.lock().unwrap_or_else(|p| p.into_inner()).insert(run.id.clone(), run.clone());
    Ok(run)
}

fn run_by_id(state: &AppState, id: &str) -> Res<crate::agents::TaskRun> {
    state
        .runs
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(id)
        .cloned()
        .ok_or_else(|| "That run is no longer open".into())
}

#[tauri::command]
pub fn agent_diff(state: State<'_, AppState>, id: String) -> Res<String> {
    crate::agents::diff(&run_by_id(&state, &id)?)
}

/// Keeps the run's changes as a new branch of the workspace (`keep` is its title), or discards them. Either way
/// the worktree is removed. Returns the branch name when kept.
#[tauri::command]
pub fn agent_finish(state: State<'_, AppState>, id: String, keep: Option<String>) -> Res<Option<String>> {
    let run = run_by_id(&state, &id)?;
    let branch = crate::agents::finish(&run, keep.as_deref().map(str::trim).filter(|k| !k.is_empty()))?;
    state.runs.lock().unwrap_or_else(|p| p.into_inner()).remove(&id);
    Ok(branch)
}

// ---------- Workspaces and smart tasks (tasks.rs) ----------

use crate::tasks::{self, NewTask, TaskDetail, TaskSummary, WorkspaceView};

#[tauri::command]
pub fn workspaces_list(state: State<'_, AppState>) -> Res<Vec<WorkspaceView>> {
    state.shared.with_db(tasks::list_workspaces)
}

#[tauri::command]
pub async fn workspace_add(state: State<'_, AppState>, path: String, name: Option<String>) -> Res<WorkspaceView> {
    state.shared.with_db(|db| tasks::add_workspace(db, &path, name.as_deref()))
}

#[tauri::command]
pub fn workspace_remove(state: State<'_, AppState>, id: String) -> Res<()> {
    state.shared.with_db(|db| tasks::remove_workspace(db, &id))
}

#[tauri::command]
pub fn workspace_detect_checks(state: State<'_, AppState>, id: String) -> Res<WorkspaceView> {
    state.shared.with_db(|db| tasks::detect_checks(db, &id))
}

/// Approves a check (or withdraws the approval), adds one the user typed, or removes one.
#[tauri::command]
pub fn workspace_check(state: State<'_, AppState>, id: String, command: String, action: String) -> Res<WorkspaceView> {
    state.shared.with_db(|db| {
        match action.as_str() {
            "approve" => tasks::set_check(db, &id, &command, true),
            "unapprove" => tasks::set_check(db, &id, &command, false),
            "add" => tasks::add_check(db, &id, &command),
            "remove" => tasks::remove_check(db, &id, &command),
            other => Err(format!("unknown action {other}")),
        }?;
        tasks::workspace(db, &id)
    })
}

#[tauri::command]
pub fn tasks_list(state: State<'_, AppState>) -> Res<Vec<TaskSummary>> {
    state.shared.with_db(tasks::list_tasks)
}

#[tauri::command]
pub fn task_get(state: State<'_, AppState>, id: String) -> Res<TaskDetail> {
    state.shared.with_db(|db| tasks::task_detail(db, &id))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpSetup {
    /// The program an IDE starts: the app itself, with `args`.
    pub path: String,
    pub args: Vec<String>,
    pub exists: bool,
    pub url: String,
}

/// How an IDE starts the MCP server: this app's own program with `mcp` (src/mcp.rs), so the installer ships
/// nothing extra.
#[tauri::command]
pub fn mcp_setup(state: State<'_, AppState>) -> Res<McpSetup> {
    let path = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(McpSetup {
        exists: path.is_file(),
        path: path.to_string_lossy().into_owned(),
        args: vec!["mcp".into()],
        url: format!("http://127.0.0.1:{}", state.port()),
    })
}

fn mcp_server(state: &AppState) -> Res<crate::integrations::Server> {
    let path = std::env::current_exe().map_err(|e| e.to_string())?;
    let port = state.port();
    Ok(crate::integrations::Server {
        program: path.to_string_lossy().into_owned(),
        args: vec!["mcp".into()],
        url: (port != gateway::DEFAULT_PORT).then(|| format!("http://127.0.0.1:{port}")),
        gateway_url: format!("http://127.0.0.1:{port}"),
        models: state.shared.with_db(crate::integrations::gateway_models),
    })
}

fn integration_backups(state: &AppState) -> std::path::PathBuf {
    state.data_dir.join("integration-backups")
}

/// After a change to connections or models: OpenCode's list of Pygg's models and Codex's catalog follow, when Pygg set them up.
fn sync_opencode(state: &AppState) {
    if let Err(e) = crate::integrations::sync_opencode_models(&state.shared, &crate::integrations::Homes::from_env()) {
        eprintln!("OpenCode's model list: {e}");
    }
    // And the catalog Codex's pygg profile reads (codex_catalog.rs).
    if let Err(e) = crate::integrations::sync_codex_catalog(&state.shared, &crate::integrations::Homes::from_env()) {
        eprintln!("Codex's model catalog: {e}");
    }
}

/// The agents Pygg can register its MCP server with, and whether it is (integrations.rs, ROADMAP D2).
#[tauri::command]
pub async fn integrations_list(state: State<'_, AppState>) -> Res<Vec<crate::integrations::Integration>> {
    let records = state.shared.with_db(crate::integrations::records);
    Ok(crate::integrations::list_with(records, &crate::integrations::Homes::from_env()).await)
}

/// What setting one up would change, shown before the user agrees.
#[tauri::command]
pub fn integration_plan(state: State<'_, AppState>, id: String) -> Res<crate::integrations::Plan> {
    crate::integrations::plan(
        &id,
        &mcp_server(&state)?,
        &crate::integrations::Homes::from_env(),
        &integration_backups(&state),
    )
}

#[tauri::command]
pub async fn integration_apply(state: State<'_, AppState>, id: String) -> Res<crate::integrations::Integration> {
    let server = mcp_server(&state)?;
    crate::integrations::apply(
        &id,
        &server,
        &crate::integrations::Homes::from_env(),
        &integration_backups(&state),
        &state.shared,
        crate::db::now_ms(),
    )
    .await
}

#[tauri::command]
pub async fn integration_remove(state: State<'_, AppState>, id: String) -> Res<crate::integrations::Integration> {
    crate::integrations::remove(
        &id,
        &crate::integrations::Homes::from_env(),
        &integration_backups(&state),
        &state.shared,
        crate::db::now_ms(),
    )
    .await
}

fn budget_micro(usd: Option<f64>) -> Option<u64> {
    usd.filter(|u| u.is_finite() && *u > 0.0).map(|u| (u * 1e6).round() as u64)
}

/// `standard` or `sensitive` (every step asks before it runs).
#[tauri::command]
pub fn workspace_set_privacy(state: State<'_, AppState>, id: String, class: String) -> Res<WorkspaceView> {
    state.shared.with_db(|db| {
        tasks::set_privacy(db, &id, &class)?;
        tasks::workspace(db, &id)
    })
}

/// The system's folder picker; `None` when the user cancels. Shown from the core, so the interface needs no
/// file-system permission of its own.
#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle) -> Res<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().set_title("Choose a Git repository").pick_folder(move |folder| {
        let _ = tx.send(folder);
    });
    let folder = rx.await.map_err(|e| e.to_string())?;
    Ok(folder.and_then(|f| f.into_path().ok()).map(|p| p.to_string_lossy().into_owned()))
}

/// Saves the task's result as a patch file where the user chooses (the system's save dialog). Returns the path, or
/// `None` when canceled. The result stays open.
#[tauri::command]
pub async fn task_export_patch(app: tauri::AppHandle, state: State<'_, AppState>, id: String) -> Res<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let (name, patch) = state.shared.with_db(|db| tasks::result_patch(db, &id))?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Export Patch")
        .set_file_name(&name)
        .add_filter("Patch", &["patch", "diff"])
        .save_file(move |file| {
            let _ = tx.send(file);
        });
    let Some(file) = rx.await.map_err(|e| e.to_string())? else { return Ok(None) };
    let path = file.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, patch).map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    let shown = path.to_string_lossy().into_owned();
    state.shared.with_db(|db| tasks::record_export(db, &id, &shown))?;
    Ok(Some(shown))
}

/// Today's paid steps against the daily budget, for the toolbar.
#[tauri::command]
pub fn task_budget(state: State<'_, AppState>) -> Res<tasks::DayBudget> {
    state.shared.with_db(tasks::day_budget)
}

/// How tasks turned out over the last `days` (ROADMAP C4): results kept, checks passed, money per kept result.
#[tauri::command]
pub fn tasks_quality(state: State<'_, AppState>, days: u32) -> Res<tasks::Quality> {
    let since = crate::db::now_ms() - days.clamp(1, 366) as i64 * 86_400_000;
    state.shared.with_db(|db| tasks::quality(db, since))
}

/// Sets the daily budget for paid steps (`None`: no limit).
#[tauri::command]
pub fn task_set_budget(state: State<'_, AppState>, daily_usd: Option<f64>) -> Res<tasks::DayBudget> {
    state.shared.with_db(|db| {
        tasks::set_daily_budget(db, budget_micro(daily_usd))?;
        tasks::day_budget(db)
    })
}

#[tauri::command]
pub fn workspace_set_budget(state: State<'_, AppState>, id: String, daily_usd: Option<f64>) -> Res<WorkspaceView> {
    state.shared.with_db(|db| {
        tasks::set_workspace_budget(db, &id, budget_micro(daily_usd))?;
        tasks::workspace(db, &id)
    })
}

/// Creates a task and starts it in the background. Returns its id.
#[tauri::command]
pub async fn task_create(state: State<'_, AppState>, task: NewTask) -> Res<String> {
    let id = state.shared.with_db(|db| tasks::create_task(db, &task))?;
    state.tasks.start(&id);
    Ok(id)
}

#[tauri::command]
pub async fn task_cancel(state: State<'_, AppState>, id: String) -> Res<()> {
    state.tasks.cancel(&id)
}

#[tauri::command]
pub async fn task_decide(state: State<'_, AppState>, id: String, approve: bool, cap_usd: Option<f64>) -> Res<()> {
    let cap = cap_usd.filter(|c| c.is_finite() && *c > 0.0).map(|c| (c * 1e6).round() as u64);
    state.tasks.decide(&id, approve, cap).map(|_| ())
}

#[tauri::command]
pub async fn task_retry(state: State<'_, AppState>, id: String) -> Res<()> {
    state.tasks.retry(&id).map(|_| ())
}

#[tauri::command]
pub async fn task_diff(state: State<'_, AppState>, id: String) -> Res<String> {
    state.shared.with_db(|db| tasks::result_diff(db, &id))
}

/// Keeps the task's result as a branch (`title` names it) or discards it. Returns the branch when kept.
#[tauri::command]
pub async fn task_finish(
    state: State<'_, AppState>, id: String, keep: bool, title: Option<String>,
) -> Res<Option<String>> {
    state.shared.with_db(|db| {
        if keep {
            tasks::keep(db, &id, title.as_deref()).map(Some)
        } else {
            tasks::discard(db, &id).map(|()| None)
        }
    })
}
