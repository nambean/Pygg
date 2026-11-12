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
}

type Res<T> = Result<T, String>;

const PORT_KEY: &str = "gateway.port";
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
        state: if server.is_some() { "running" } else if error.is_some() { "failed" } else { "stopped" },
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
}

fn view(state: &AppState, provider: Provider) -> ConnectionView {
    let preset = presets::find(&provider.kind);
    let has_key = provider.credential_ref.as_deref().map(|r| matches!(vault::read(r), Ok(Some(_)))).unwrap_or(false);
    let models = state.shared.with_db(|db| store::list_models(db, Some(&provider.id))).map(|m| m.len()).unwrap_or(0);
    let health = state.shared.with_db(|db| store::list_health(db, Some(&provider.id))).unwrap_or_default();
    let shares_quota_with = state.shared.with_db(store::list_providers).unwrap_or_default().into_iter()
        .filter(|p| p.id != provider.id && p.quota_group == provider.quota_group).map(|p| p.label).collect();
    let limits = state.shared.with_db(|db| store::list_limits(db, &provider.id)).unwrap_or_default();
    let account = state.shared.with_db(|db| gateway::quota::saved_account(db, &provider.id));
    ConnectionView {
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
}

#[tauri::command]
pub fn connection_add(state: State<'_, AppState>, connection: NewConnection) -> Res<ConnectionView> {
    let preset = presets::find(&connection.preset).ok_or("Unknown provider")?;
    let endpoint = connection.endpoint.as_deref().map(str::trim).filter(|e| !e.is_empty())
        .or(preset.endpoint).ok_or("Enter the endpoint URL")?.to_string();
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
    let added = state.shared.with_db(|db| store::add_provider(db, &NewProvider {
        kind: preset.id, label, endpoint: &endpoint, credential_ref: reference.as_deref(),
        default_billing: &connection.billing, wire: preset.wire,
    }));
    match added {
        Ok(provider) => Ok(view(&state, provider)),
        Err(e) => {
            if let Some(reference) = &reference {
                let _ = vault::remove(reference);
            }
            Err(e)
        }
    }
}

#[tauri::command]
pub fn connection_update(state: State<'_, AppState>, id: String, enabled: Option<bool>, billing: Option<String>,
    separate_quota: Option<bool>) -> Res<ConnectionView> {
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
            state.shared.with_db(|db| db.execute("UPDATE providers SET credential_ref = ?2 WHERE id = ?1",
                rusqlite::params![id, reference]).map_err(|e| e.to_string()))?;
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
pub fn connection_set_terms(state: State<'_, AppState>, id: String, terms: String, source: Option<String>) -> Res<ConnectionView> {
    state.shared.with_db(|db| store::set_provider_terms(db, &id, &terms, source.as_deref()))?;
    let provider = state.shared.with_db(store::list_providers)?.into_iter().find(|p| p.id == id).ok_or("That connection no longer exists")?;
    Ok(view(&state, provider))
}

#[tauri::command]
pub fn connection_remove(state: State<'_, AppState>, id: String) -> Res<()> {
    let reference = state.shared.with_db(|db| store::remove_provider(db, &id))?;
    sync_opencode(&state);
    if let Some(reference) = reference {
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
    let ids = catalog::fetch_models(&state.shared, &provider).await?;
    let source = catalog::list_source(&provider);
    let added = state.shared.with_db(|db| store::import_listed(db, &id, &ids, Some(&source)))?;
    // What the provider's list left out is looked up (ADR-031): the user only enters a key.
    let cache = gateway::reference::cache_path(&state.data_dir.join("store.sqlite3"));
    if let Some(catalog) = gateway::reference::current(&state.shared.http, &cache).await {
        state.shared.with_db(|db| { let _ = gateway::reference::apply(db, &catalog); gateway::reference::mark_applied(db); });
    }
    sync_opencode(&state);
    if let Ok(Some(account)) = gateway::quota::fetch_account(&state.shared, &provider).await {
        let _ = state.shared.with_db(|db| gateway::quota::save_account(db, &id, &account));
    }
    Ok(ImportResult { listed: ids.len(), added })
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
pub fn model_set_capabilities(state: State<'_, AppState>, provider_id: String, model_id: String,
    capabilities: store::Capabilities) -> Res<()> {
    state.shared.with_db(|db| store::set_capabilities(db, &provider_id, &model_id, &capabilities))?;
    sync_opencode(&state);
    Ok(())
}

/// What `auto` or `auto/cheap` would choose for a request with these needs, from a client's point of view,
/// with every excluded model and why. Sends nothing (ROADMAP P0-5).
#[tauri::command]
pub fn auto_simulate(state: State<'_, AppState>, client_id: Option<String>, alias: String, needs: gateway::auto::Needs)
    -> Res<gateway::auto::Plan> {
    let strategy = gateway::auto::strategy(&alias).ok_or_else(|| format!("{alias} is not an automatic route"))??;
    state.shared.with_db(|db| {
        let clients = store::list_clients(db)?;
        let client = match &client_id {
            Some(id) => clients.into_iter().find(|c| &c.id == id).ok_or("Client not found")?,
            // No client chosen: the defaults a new client gets.
            None => Client { id: String::new(), name: String::new(), token_prefix: String::new(), monthly_budget_micro_usd: None,
                created_at: 0, last_used_at: None, revoked_at: None, month_spend_micro_usd: 0, month_unknown_requests: 0,
                max_concurrency: 8, unknown_cost_policy: "block".into(), reserved_micro_usd: 0, month_at_risk_micro_usd: 0,
                allowed_connections: None, paid_retries: 0, auto_min_tier: "T2".into(), may_run_tasks: false, require_no_training: false, strict_budget: false },
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
    if usd.is_finite() && usd >= 0.0 && usd < 10_000.0 {
        Ok((usd * 1_000_000.0).round() as u64)
    } else {
        Err("Enter a price from 0 to 10,000 USD per million tokens".into())
    }
}

#[tauri::command]
pub fn model_update(state: State<'_, AppState>, provider_id: String, model_id: String, enabled: Option<bool>,
    billing: Option<String>, price: Option<PriceInput>, clear_price: Option<bool>) -> Res<()> {
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
        Some(_) => Err("Enter a monthly limit above $0".into()),
    }
}

#[tauri::command]
pub fn client_create(state: State<'_, AppState>, name: String, monthly_budget_usd: Option<f64>) -> Res<CreatedClient> {
    let limit = budget(monthly_budget_usd)?;
    let (client, token) = state.shared.with_db(|db| store::create_client(db, &name, limit))?;
    Ok(CreatedClient { client, token })
}

#[tauri::command]
pub fn client_revoke(state: State<'_, AppState>, id: String) -> Res<()> {
    state.shared.with_db(|db| store::revoke_client(db, &id))
}

#[tauri::command]
pub fn client_set_budget(state: State<'_, AppState>, id: String, monthly_budget_usd: Option<f64>) -> Res<()> {
    let limit = budget(monthly_budget_usd)?;
    state.shared.with_db(|db| store::set_client_budget(db, &id, limit))
}

/// Requests allowed in flight at once, and whether a target with an unknown price may be used while the
/// client has a limit (`block` or `allow`).
#[tauri::command]
pub fn client_set_policy(state: State<'_, AppState>, id: String, max_concurrency: Option<u32>,
    unknown_cost_policy: Option<String>, restrict_connections: Option<bool>, allowed_connections: Option<Vec<String>>,
    paid_retries: Option<u32>, auto_min_tier: Option<String>, may_run_tasks: Option<bool>, require_no_training: Option<bool>, strict_budget: Option<bool>) -> Res<()> {
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
    app.dialog().file().set_title("Export Usage").set_file_name(&name).add_filter("CSV", &["csv"])
        .save_file(move |file| { let _ = tx.send(file); });
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
    let context = crate::diagnostics::Context { made_by: "app", gateway_state: gateway.state.to_string(),
        gateway_port: gateway.port, gateway_error: gateway.error.or(gateway.accounting_error), log_lines: Vec::new() };
    let zip = state.shared.with_db(|db| crate::diagnostics::bundle(db, &context, now))?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().set_title("Save Diagnostics").set_file_name(crate::diagnostics::file_name(now)).add_filter("Zip", &["zip"])
        .save_file(move |file| { let _ = tx.send(file); });
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
    state.runs.lock().unwrap_or_else(|p| p.into_inner()).get(id).cloned().ok_or_else(|| "That run is no longer open".into())
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
    Ok(McpSetup { exists: path.is_file(), path: path.to_string_lossy().into_owned(), args: vec!["mcp".into()],
        url: format!("http://127.0.0.1:{}", state.port()) })
}

fn mcp_server(state: &AppState) -> Res<crate::integrations::Server> {
    let path = std::env::current_exe().map_err(|e| e.to_string())?;
    let port = state.port();
    Ok(crate::integrations::Server { program: path.to_string_lossy().into_owned(), args: vec!["mcp".into()],
        url: (port != gateway::DEFAULT_PORT).then(|| format!("http://127.0.0.1:{port}")),
        gateway_url: format!("http://127.0.0.1:{port}"), models: state.shared.with_db(crate::integrations::gateway_models) })
}

fn integration_backups(state: &AppState) -> std::path::PathBuf {
    state.data_dir.join("integration-backups")
}

/// After a change to connections or models: OpenCode's list of Pygg's models follows, when Pygg set it up.
fn sync_opencode(state: &AppState) {
    if let Err(e) = crate::integrations::sync_opencode_models(&state.shared, &crate::integrations::Homes::from_env()) {
        eprintln!("OpenCode's model list: {e}");
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
    crate::integrations::plan(&id, &mcp_server(&state)?, &crate::integrations::Homes::from_env(), &integration_backups(&state))
}

#[tauri::command]
pub async fn integration_apply(state: State<'_, AppState>, id: String) -> Res<crate::integrations::Integration> {
    let server = mcp_server(&state)?;
    crate::integrations::apply(&id, &server, &crate::integrations::Homes::from_env(), &integration_backups(&state),
        &state.shared, crate::db::now_ms()).await
}

#[tauri::command]
pub async fn integration_remove(state: State<'_, AppState>, id: String) -> Res<crate::integrations::Integration> {
    crate::integrations::remove(&id, &crate::integrations::Homes::from_env(), &integration_backups(&state), &state.shared,
        crate::db::now_ms()).await
}

fn budget_micro(usd: Option<f64>) -> Option<u64> {
    usd.filter(|u| u.is_finite() && *u > 0.0).map(|u| (u * 1e6).round() as u64)
}

/// `standard` or `sensitive` (every step asks before it runs).
#[tauri::command]
pub fn workspace_set_privacy(state: State<'_, AppState>, id: String, class: String) -> Res<WorkspaceView> {
    state.shared.with_db(|db| { tasks::set_privacy(db, &id, &class)?; tasks::workspace(db, &id) })
}

/// The system's folder picker; `None` when the user cancels. Shown from the core, so the interface needs no
/// file-system permission of its own.
#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle) -> Res<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().set_title("Choose a Git repository").pick_folder(move |folder| { let _ = tx.send(folder); });
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
    app.dialog().file().set_title("Export Patch").set_file_name(&name).add_filter("Patch", &["patch", "diff"])
        .save_file(move |file| { let _ = tx.send(file); });
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
    state.shared.with_db(|db| { tasks::set_daily_budget(db, budget_micro(daily_usd))?; tasks::day_budget(db) })
}

#[tauri::command]
pub fn workspace_set_budget(state: State<'_, AppState>, id: String, daily_usd: Option<f64>) -> Res<WorkspaceView> {
    state.shared.with_db(|db| { tasks::set_workspace_budget(db, &id, budget_micro(daily_usd))?; tasks::workspace(db, &id) })
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
pub async fn task_finish(state: State<'_, AppState>, id: String, keep: bool, title: Option<String>) -> Res<Option<String>> {
    state.shared.with_db(|db| if keep { tasks::keep(db, &id, title.as_deref()).map(Some) } else { tasks::discard(db, &id).map(|()| None) })
}

