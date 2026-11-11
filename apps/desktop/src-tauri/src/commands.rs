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
    pub has_key: bool,
    pub needs_key: bool,
    pub models: usize,
    /// Current cooldowns and blocks on the connection or its models (ROADMAP P0-2).
    pub health: Vec<store::Health>,
    /// Other connections that share this one's quota group.
    pub shares_quota_with: Vec<String>,
}

fn view(state: &AppState, provider: Provider) -> ConnectionView {
    let preset = presets::find(&provider.kind);
    let has_key = provider.credential_ref.as_deref().map(|r| matches!(vault::read(r), Ok(Some(_)))).unwrap_or(false);
    let models = state.shared.with_db(|db| store::list_models(db, Some(&provider.id))).map(|m| m.len()).unwrap_or(0);
    let health = state.shared.with_db(|db| store::list_health(db, Some(&provider.id))).unwrap_or_default();
    let shares_quota_with = state.shared.with_db(store::list_providers).unwrap_or_default().into_iter()
        .filter(|p| p.id != provider.id && p.quota_group == provider.quota_group).map(|p| p.label).collect();
    ConnectionView {
        preset_name: preset.map(|p| p.name.to_string()).unwrap_or_else(|| provider.kind.clone()),
        needs_key: preset.map(|p| p.needs_key).unwrap_or(false),
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

#[tauri::command]
pub fn connection_remove(state: State<'_, AppState>, id: String) -> Res<()> {
    let reference = state.shared.with_db(|db| store::remove_provider(db, &id))?;
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
    let added = state.shared.with_db(|db| store::import_models(db, &id, &ids))?;
    Ok(ImportResult { listed: ids.len(), added })
}

// ---------- Models ----------

#[tauri::command]
pub fn models_list(state: State<'_, AppState>, provider_id: Option<String>) -> Res<Vec<Model>> {
    state.shared.with_db(|db| store::list_models(db, provider_id.as_deref()))
}

#[tauri::command]
pub fn model_add(state: State<'_, AppState>, provider_id: String, model_id: String) -> Res<()> {
    state.shared.with_db(|db| store::add_model(db, &provider_id, &model_id))
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
    state.shared.with_db(|db| store::update_model(db, &provider_id, &model_id, enabled, billing.as_deref(), price))
}

#[tauri::command]
pub fn model_remove(state: State<'_, AppState>, provider_id: String, model_id: String) -> Res<()> {
    state.shared.with_db(|db| store::remove_model(db, &provider_id, &model_id))
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
    unknown_cost_policy: Option<String>) -> Res<()> {
    state.shared.with_db(|db| store::set_client_policy(db, &id, max_concurrency, unknown_cost_policy.as_deref()))
}

// ---------- Usage ----------

#[tauri::command]
pub fn usage_summary(state: State<'_, AppState>, days: u32) -> Res<UsageSummary> {
    let days = days.clamp(1, 366) as i64;
    let since = crate::db::now_ms() - days * 86_400_000;
    state.shared.with_db(|db| store::usage_summary(db, since, 200))
}
