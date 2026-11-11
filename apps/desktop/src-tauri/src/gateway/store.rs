//! Gateway records in the local store: connections, their models, client tokens, combos and the usage
//! ledger. Used by the gateway's request handlers and by the interface's commands alike.

use super::pricing::{Cost, Price};
use crate::db::now_ms;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub type Result<T> = std::result::Result<T, String>;

fn err(e: rusqlite::Error) -> String {
    e.to_string()
}

pub const BILLING: &[&str] = &["free", "local", "subscription", "paid", "unknown"];

// ---------- Connections ----------

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    /// Also the first segment of a routed model name: `<id>/<model>`.
    pub id: String,
    pub kind: String,
    pub label: String,
    pub endpoint: String,
    pub enabled: bool,
    pub credential_ref: Option<String>,
    pub default_billing: String,
    pub data_use_terms: String,
    pub created_at: i64,
    /// `openai` or `anthropic`: the API this connection speaks.
    pub wire: String,
    /// Connections in the same group share rate limits and terms: after a 401, 402, 403 or 429 on one, the
    /// others are not tried for that request. Defaults to the provider kind.
    pub quota_group: String,
    /// The user confirmed that this connection has its own provider quota (counter-review CR-02). This is the
    /// user's statement, not something the gateway verified; the provider's own limits still apply.
    pub separate_quota: bool,
    /// When the user confirmed it.
    pub quota_confirmed_at: Option<i64>,
}

const PROVIDER_COLUMNS: &str =
    "id, kind, account_label, COALESCE(endpoint, ''), enabled, credential_ref, default_billing, data_use_terms, created_at, \
     wire, COALESCE(quota_group, kind), quota_group IS NOT NULL, quota_confirmed_at";

fn provider_row(row: &rusqlite::Row) -> rusqlite::Result<Provider> {
    Ok(Provider {
        id: row.get(0)?,
        kind: row.get(1)?,
        label: row.get(2)?,
        endpoint: row.get(3)?,
        enabled: row.get::<_, i64>(4)? == 1,
        credential_ref: row.get(5)?,
        default_billing: row.get(6)?,
        data_use_terms: row.get(7)?,
        created_at: row.get(8)?,
        wire: row.get(9)?,
        quota_group: row.get(10)?,
        separate_quota: row.get::<_, i64>(11)? == 1,
        quota_confirmed_at: row.get(12)?,
    })
}

pub fn list_providers(conn: &Connection) -> Result<Vec<Provider>> {
    let mut stmt = conn.prepare(&format!("SELECT {PROVIDER_COLUMNS} FROM providers ORDER BY created_at")).map_err(err)?;
    let rows = stmt.query_map([], provider_row).map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

pub fn get_provider(conn: &Connection, id: &str) -> Result<Option<Provider>> {
    conn.query_row(&format!("SELECT {PROVIDER_COLUMNS} FROM providers WHERE id = ?1"), [id], provider_row)
        .optional()
        .map_err(err)
}

/// `Google Gemini` → `google-gemini`; never contains `/`, so it can lead a routed model name.
pub fn slug(label: &str) -> String {
    let mut out = String::new();
    for c in label.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').chars().take(32).collect::<String>();
    if out.is_empty() { "connection".into() } else { out }
}

pub struct NewProvider<'a> {
    pub kind: &'a str,
    pub label: &'a str,
    pub endpoint: &'a str,
    pub credential_ref: Option<&'a str>,
    pub default_billing: &'a str,
    pub wire: &'a str,
}

pub fn add_provider(conn: &Connection, new: &NewProvider) -> Result<Provider> {
    if !BILLING.contains(&new.default_billing) {
        return Err(format!("unknown billing type {}", new.default_billing));
    }
    if !matches!(new.wire, "openai" | "anthropic") {
        return Err(format!("unknown wire format {}", new.wire));
    }
    let endpoint = new.endpoint.trim().trim_end_matches('/');
    if !(endpoint.starts_with("https://") || endpoint.starts_with("http://")) {
        return Err("The endpoint must start with https:// or http://".into());
    }
    let base = slug(new.label);
    let mut id = base.clone();
    let mut n = 2;
    while get_provider(conn, &id)?.is_some() {
        id = format!("{base}-{n}");
        n += 1;
    }
    conn.execute(
        "INSERT INTO providers (id, kind, account_label, endpoint, credential_ref, default_billing, wire, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, new.kind, new.label.trim(), endpoint, new.credential_ref, new.default_billing, new.wire, now_ms()],
    )
    .map_err(err)?;
    get_provider(conn, &id)?.ok_or_else(|| "connection was not saved".into())
}

/// Removes the connection with its models and combo entries; returns its credential reference so the
/// caller can remove the secret too.
pub fn remove_provider(conn: &Connection, id: &str) -> Result<Option<String>> {
    let reference = get_provider(conn, id)?.and_then(|p| p.credential_ref);
    conn.execute("DELETE FROM providers WHERE id = ?1", [id]).map_err(err)?;
    Ok(reference)
}

pub fn update_provider(conn: &Connection, id: &str, enabled: Option<bool>, billing: Option<&str>,
    separate_quota: Option<bool>) -> Result<()> {
    if let Some(separate) = separate_quota {
        conn.execute("UPDATE providers SET quota_group = CASE WHEN ?2 THEN id END, \
             quota_confirmed_at = CASE WHEN ?2 THEN ?3 END WHERE id = ?1", params![id, separate, now_ms()])
            .map_err(err)?;
    }
    if let Some(enabled) = enabled {
        conn.execute("UPDATE providers SET enabled = ?2 WHERE id = ?1", params![id, enabled as i64]).map_err(err)?;
    }
    if let Some(billing) = billing {
        if !BILLING.contains(&billing) {
            return Err(format!("unknown billing type {billing}"));
        }
        conn.execute("UPDATE providers SET default_billing = ?2 WHERE id = ?1", params![id, billing]).map_err(err)?;
    }
    Ok(())
}

// ---------- Models ----------

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub provider_id: String,
    pub model_id: String,
    /// The model's own billing type, or the connection's when it has none.
    pub billing: String,
    pub enabled: bool,
    pub price: Option<Price>,
    pub observed_at: Option<i64>,
}

pub fn list_models(conn: &Connection, provider_id: Option<&str>) -> Result<Vec<Model>> {
    let mut stmt = conn
        .prepare(
            "SELECT m.provider_id, m.model_id, CASE WHEN m.billing = 'unknown' THEN p.default_billing ELSE m.billing END, \
             m.enabled, m.price_json, m.observed_at FROM models m JOIN providers p ON p.id = m.provider_id \
             WHERE ?1 IS NULL OR m.provider_id = ?1 ORDER BY m.provider_id, m.model_id",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map([provider_id], |row| {
            let price: Option<String> = row.get(4)?;
            Ok(Model {
                provider_id: row.get(0)?,
                model_id: row.get(1)?,
                billing: row.get(2)?,
                enabled: row.get::<_, i64>(3)? == 1,
                price: price.and_then(|p| serde_json::from_str(&p).ok()),
                observed_at: row.get(5)?,
            })
        })
        .map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

/// Adds models the provider listed; existing rows keep their settings. Returns how many were new.
pub fn import_models(conn: &Connection, provider_id: &str, model_ids: &[String]) -> Result<usize> {
    let now = now_ms();
    let mut added = 0;
    for model in model_ids {
        added += conn
            .execute(
                "INSERT OR IGNORE INTO models (id, provider_id, model_id, observed_at) VALUES (?1, ?2, ?3, ?4)",
                params![format!("{provider_id}/{model}"), provider_id, model, now],
            )
            .map_err(err)?;
        conn.execute("UPDATE models SET observed_at = ?3 WHERE provider_id = ?1 AND model_id = ?2",
            params![provider_id, model, now]).map_err(err)?;
    }
    Ok(added)
}

/// Adds a model by hand (for providers that do not list theirs).
pub fn add_model(conn: &Connection, provider_id: &str, model_id: &str) -> Result<()> {
    let model_id = model_id.trim();
    if model_id.is_empty() {
        return Err("Enter a model name".into());
    }
    import_models(conn, provider_id, &[model_id.to_string()]).map(|_| ())
}

pub fn update_model(conn: &Connection, provider_id: &str, model_id: &str, enabled: Option<bool>,
    billing: Option<&str>, price: Option<Option<Price>>) -> Result<()> {
    if let Some(enabled) = enabled {
        conn.execute("UPDATE models SET enabled = ?3 WHERE provider_id = ?1 AND model_id = ?2",
            params![provider_id, model_id, enabled as i64]).map_err(err)?;
    }
    if let Some(billing) = billing {
        if !BILLING.contains(&billing) {
            return Err(format!("unknown billing type {billing}"));
        }
        conn.execute("UPDATE models SET billing = ?3 WHERE provider_id = ?1 AND model_id = ?2",
            params![provider_id, model_id, billing]).map_err(err)?;
    }
    if let Some(price) = price {
        let json = price.map(|p| serde_json::to_string(&p).unwrap_or_default());
        conn.execute("UPDATE models SET price_json = ?3 WHERE provider_id = ?1 AND model_id = ?2",
            params![provider_id, model_id, json]).map_err(err)?;
    }
    Ok(())
}

pub fn remove_model(conn: &Connection, provider_id: &str, model_id: &str) -> Result<()> {
    conn.execute("DELETE FROM models WHERE provider_id = ?1 AND model_id = ?2", params![provider_id, model_id])
        .map(|_| ())
        .map_err(err)
}

/// Billing and price for a routed model; a model without a row uses its connection's billing type.
pub fn pricing_for(conn: &Connection, provider: &Provider, model_id: &str) -> Result<(String, Option<Price>)> {
    let row: Option<(String, Option<String>)> = conn
        .query_row("SELECT billing, price_json FROM models WHERE provider_id = ?1 AND model_id = ?2",
            params![provider.id, model_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()
        .map_err(err)?;
    Ok(match row {
        Some((billing, price)) => (
            if billing == "unknown" { provider.default_billing.clone() } else { billing },
            price.and_then(|p| serde_json::from_str(&p).ok()),
        ),
        None => (provider.default_billing.clone(), None),
    })
}

// ---------- Client tokens ----------

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Client {
    pub id: String,
    pub name: String,
    pub token_prefix: String,
    pub monthly_budget_micro_usd: Option<u64>,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
    pub revoked_at: Option<i64>,
    /// Known spend this calendar month (UTC).
    pub month_spend_micro_usd: u64,
    /// Requests this month whose cost is unknown, so the limit could not count them.
    pub month_unknown_requests: u64,
    /// Requests allowed in flight at once.
    pub max_concurrency: u32,
    /// `block` or `allow`: whether a target with an unknown price may be used while this client has a limit.
    pub unknown_cost_policy: String,
    /// Spend held for requests still in flight.
    pub reserved_micro_usd: u64,
    /// Attempts this month that may have been charged but whose cost is unknown, counted at the amount held
    /// for them (counter-review CR-03). Part of what the limit is checked against.
    pub month_at_risk_micro_usd: u64,
}

impl Client {
    /// What the monthly limit is checked against: known spend plus spend at risk.
    pub fn counted_micro_usd(&self) -> u64 {
        self.month_spend_micro_usd + self.month_at_risk_micro_usd
    }
}

/// Known spend and spend at risk, as one sum: what a monthly limit is checked against.
const COUNTED_SPEND: &str = "COALESCE(SUM(COALESCE(cost_micro_usd, at_risk_micro_usd, 0)), 0)";

pub fn hash_token(token: &str) -> String {
    Sha256::digest(token.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// Unix milliseconds at the start of the current UTC month.
pub fn month_start(now: i64) -> i64 {
    let days = now.div_euclid(86_400_000);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm), then back to the 1st.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5; // 0-based day of month
    (days - day) * 86_400_000
}

fn client_row(conn: &Connection, row: &rusqlite::Row) -> rusqlite::Result<Client> {
    let id: String = row.get(0)?;
    let since = month_start(now_ms());
    let (spend, unknown, at_risk): (i64, i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(cost_micro_usd), 0), COALESCE(SUM(cost_status = 'unknown'), 0), \
         COALESCE(SUM(CASE WHEN cost_micro_usd IS NULL THEN at_risk_micro_usd END), 0) FROM usage_events \
         WHERE client_id = ?1 AND created_at >= ?2",
        params![id, since],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let reserved: i64 = conn.query_row(
        "SELECT COALESCE(SUM(reserved_micro), 0) FROM budget_reservations WHERE client_id = ?1", [&id], |r| r.get(0))?;
    Ok(Client {
        id,
        name: row.get(1)?,
        token_prefix: row.get(2)?,
        monthly_budget_micro_usd: row.get::<_, Option<i64>>(3)?.map(|v| v as u64),
        created_at: row.get(4)?,
        last_used_at: row.get(5)?,
        revoked_at: row.get(6)?,
        month_spend_micro_usd: spend.max(0) as u64,
        month_unknown_requests: unknown.max(0) as u64,
        max_concurrency: row.get::<_, i64>(7)?.max(1) as u32,
        unknown_cost_policy: row.get(8)?,
        reserved_micro_usd: reserved.max(0) as u64,
        month_at_risk_micro_usd: at_risk.max(0) as u64,
    })
}

const CLIENT_COLUMNS: &str =
    "id, name, token_prefix, monthly_budget_micro_usd, created_at, last_used_at, revoked_at, max_concurrency, unknown_cost_policy";

pub fn list_clients(conn: &Connection) -> Result<Vec<Client>> {
    let mut stmt = conn
        .prepare(&format!("SELECT {CLIENT_COLUMNS} FROM gateway_clients ORDER BY revoked_at IS NOT NULL, created_at"))
        .map_err(err)?;
    let rows = stmt.query_map([], |row| client_row(conn, row)).map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

/// Creates a client and returns it with its token, which is not stored and cannot be shown again.
pub fn create_client(conn: &Connection, name: &str, monthly_budget_micro_usd: Option<u64>) -> Result<(Client, String)> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Enter a name for the client".into());
    }
    let token = format!("str-{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO gateway_clients (id, name, token_hash, token_prefix, monthly_budget_micro_usd, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, name, hash_token(&token), &token[..12], monthly_budget_micro_usd.map(|v| v as i64), now_ms()],
    )
    .map_err(err)?;
    let client = conn
        .query_row(&format!("SELECT {CLIENT_COLUMNS} FROM gateway_clients WHERE id = ?1"), [&id], |r| client_row(conn, r))
        .map_err(err)?;
    Ok((client, token))
}

pub fn revoke_client(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("UPDATE gateway_clients SET revoked_at = ?2 WHERE id = ?1 AND revoked_at IS NULL", params![id, now_ms()])
        .map(|_| ())
        .map_err(err)
}

pub fn set_client_budget(conn: &Connection, id: &str, monthly_budget_micro_usd: Option<u64>) -> Result<()> {
    conn.execute("UPDATE gateway_clients SET monthly_budget_micro_usd = ?2 WHERE id = ?1",
        params![id, monthly_budget_micro_usd.map(|v| v as i64)]).map(|_| ()).map_err(err)
}

pub fn set_client_policy(conn: &Connection, id: &str, max_concurrency: Option<u32>, unknown_cost_policy: Option<&str>)
    -> Result<()> {
    if let Some(n) = max_concurrency {
        if !(1..=256).contains(&n) {
            return Err("Allow from 1 to 256 requests at once".into());
        }
        conn.execute("UPDATE gateway_clients SET max_concurrency = ?2 WHERE id = ?1", params![id, n]).map_err(err)?;
    }
    if let Some(policy) = unknown_cost_policy {
        if !matches!(policy, "block" | "allow") {
            return Err(format!("unknown policy {policy}"));
        }
        conn.execute("UPDATE gateway_clients SET unknown_cost_policy = ?2 WHERE id = ?1", params![id, policy]).map_err(err)?;
    }
    Ok(())
}

// ---------- Budget reservations (ROADMAP P0-3) ----------

/// Why a reservation was refused.
#[derive(Debug, PartialEq)]
pub struct OverBudget {
    pub spent: u64,
    pub reserved: u64,
    pub budget: u64,
    pub wanted: u64,
}

/// Runs `f` inside `BEGIN IMMEDIATE`: it commits when `f` returns `Ok(true)`, rolls back on `Ok(false)` or an
/// error. Immediate, so a writer in another process (the CLI next to the app) waits instead of interleaving.
fn immediate<T>(conn: &Connection, f: impl FnOnce() -> rusqlite::Result<(T, bool)>) -> Result<T> {
    conn.execute_batch("BEGIN IMMEDIATE").map_err(err)?;
    match f() {
        Ok((value, commit)) => {
            conn.execute_batch(if commit { "COMMIT" } else { "ROLLBACK" }).map_err(err)?;
            Ok(value)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(err(e))
        }
    }
}

/// An upstream attempt about to be sent (counter-review CR-06). It is journaled before the request leaves,
/// so the ledger can account for it whatever happens next.
pub struct AttemptStart<'a> {
    pub client_id: &'a str,
    pub provider_id: &'a str,
    pub model_id: &'a str,
    pub requested_model: &'a str,
    pub session_key: Option<&'a str>,
    pub route_mode: &'a str,
    /// Spend held against the client's limit; 0 for free targets and clients without a limit.
    pub hold_micro: u64,
}

/// Journals an attempt and holds its spend. Atomic across threads and processes: the limit check and the
/// insert happen under one `BEGIN IMMEDIATE`, so two requests cannot both take the last dollar. Returns
/// the attempt id, or why the hold does not fit. A hold of 0 is never refused here.
pub fn open_attempt(conn: &Connection, a: &AttemptStart) -> Result<std::result::Result<String, OverBudget>> {
    immediate(conn, || {
        if a.hold_micro > 0 {
            let budget: Option<i64> = conn.query_row(
                "SELECT monthly_budget_micro_usd FROM gateway_clients WHERE id = ?1", [a.client_id], |r| r.get(0))?;
            if let Some(budget) = budget.map(|b| b.max(0) as u64) {
                let spent: i64 = conn.query_row(
                    &format!("SELECT {COUNTED_SPEND} FROM usage_events WHERE client_id = ?1 AND created_at >= ?2"),
                    params![a.client_id, month_start(now_ms())], |r| r.get(0))?;
                let reserved: i64 = conn.query_row(
                    "SELECT COALESCE(SUM(reserved_micro), 0) FROM budget_reservations WHERE client_id = ?1",
                    [a.client_id], |r| r.get(0))?;
                let (spent, reserved) = (spent.max(0) as u64, reserved.max(0) as u64);
                if spent + reserved + a.hold_micro > budget || spent >= budget {
                    return Ok((Err(OverBudget { spent, reserved, budget, wanted: a.hold_micro }), false));
                }
            }
        }
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO budget_reservations (id, client_id, reserved_micro, created_at, provider_id, model_id, \
             requested_model, session_key, route_mode) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![id, a.client_id, a.hold_micro as i64, now_ms(), a.provider_id, a.model_id, a.requested_model,
                a.session_key, a.route_mode],
        )?;
        Ok((Ok(id), true))
    })
}

/// Settles an attempt in one transaction: the ledger entry is written and the journal entry removed, or
/// neither (counter-review CR-06). An unknown cost is counted against the limit at the amount that was held.
/// If the journal entry is already gone (settled twice), nothing is written.
pub fn settle(conn: &Connection, attempt_id: &str, record: &UsageRecord) -> Result<()> {
    immediate(conn, || {
        let held: Option<i64> = conn.query_row("SELECT reserved_micro FROM budget_reservations WHERE id = ?1",
            [attempt_id], |r| r.get(0)).optional()?;
        let Some(held) = held else { return Ok(((), false)) };
        let at_risk = (record.cost.micro_usd().is_none() && held > 0).then_some(held as u64);
        insert_usage(conn, record, at_risk)?;
        conn.execute("DELETE FROM budget_reservations WHERE id = ?1", [attempt_id])?;
        Ok(((), true))
    })
}

const INTERRUPTED_SOURCE: &str = "the gateway stopped before this attempt was settled";

/// Turns journal entries into ledger entries with an unknown cost: the attempt was sent, or about to be,
/// and never settled. `only` picks one entry (an attempt dropped without being settled); otherwise every
/// entry older than `max_age_ms` (left by a crash or a killed process). Returns how many were turned.
pub fn settle_interrupted(conn: &Connection, only: Option<&str>, max_age_ms: i64) -> Result<usize> {
    immediate(conn, || {
        let condition = "(?1 <> '' AND id = ?1) OR (?1 = '' AND created_at < ?2)";
        let cutoff = now_ms() - max_age_ms;
        let turned = conn.execute(&format!(
            "INSERT INTO usage_events (id, client_id, provider, model, cost_micro_usd, cost_status, source, created_at, \
             outcome, requested_model, error, session_key, route_mode, at_risk_micro_usd) \
             SELECT lower(hex(randomblob(16))), client_id, COALESCE(provider_id, 'unknown'), COALESCE(model_id, 'unknown'), \
             NULL, 'unknown', '{INTERRUPTED_SOURCE}', created_at, 'interrupted', requested_model, \
             'Interrupted before the outcome was recorded', session_key, route_mode, NULLIF(reserved_micro, 0) \
             FROM budget_reservations WHERE {condition}"), params![only.unwrap_or(""), cutoff])?;
        conn.execute(&format!("DELETE FROM budget_reservations WHERE {condition}"), params![only.unwrap_or(""), cutoff])?;
        Ok((turned, true))
    })
}

// ---------- Connection health (ROADMAP P0-2) ----------

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    pub provider_id: String,
    /// Empty for the whole connection.
    pub model_id: String,
    pub state: String,
    pub reason: String,
    pub http_status: Option<u16>,
    /// When the connection may be tried again; `None` until the user fixes it.
    pub until_ms: Option<i64>,
    pub updated_at: i64,
}

pub fn mark_health(conn: &Connection, provider_id: &str, model_id: &str, state: &str, reason: &str,
    http_status: Option<u16>, until_ms: Option<i64>) -> Result<()> {
    conn.execute(
        "INSERT INTO connection_health (provider_id, model_id, state, reason, http_status, until_ms, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT(provider_id, model_id) DO UPDATE SET state = ?3, reason = ?4, \
         http_status = ?5, until_ms = ?6, updated_at = ?7",
        params![provider_id, model_id, state, reason.chars().take(300).collect::<String>(), http_status, until_ms, now_ms()],
    )
    .map(|_| ())
    .map_err(err)
}

/// Clears the model's entry, and the connection-wide one unless it needs the user (a rejected key).
pub fn mark_healthy(conn: &Connection, provider_id: &str, model_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM connection_health WHERE provider_id = ?1 AND (model_id = ?2 OR (model_id = '' AND until_ms IS NOT NULL))",
        params![provider_id, model_id],
    )
    .map(|_| ())
    .map_err(err)
}

pub fn clear_health(conn: &Connection, provider_id: &str) -> Result<()> {
    conn.execute("DELETE FROM connection_health WHERE provider_id = ?1", [provider_id]).map(|_| ()).map_err(err)
}

pub fn list_health(conn: &Connection, provider_id: Option<&str>) -> Result<Vec<Health>> {
    let mut stmt = conn
        .prepare(
            "SELECT provider_id, model_id, state, reason, http_status, until_ms, updated_at FROM connection_health \
             WHERE (?1 IS NULL OR provider_id = ?1) AND (until_ms IS NULL OR until_ms > ?2) ORDER BY updated_at DESC",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map(params![provider_id, now_ms()], |r| {
            Ok(Health {
                provider_id: r.get(0)?,
                model_id: r.get(1)?,
                state: r.get(2)?,
                reason: r.get(3)?,
                http_status: r.get::<_, Option<i64>>(4)?.map(|v| v as u16),
                until_ms: r.get(5)?,
                updated_at: r.get(6)?,
            })
        })
        .map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

/// The entry that currently keeps this connection or model from being tried, if any.
pub fn blocking_health(conn: &Connection, provider_id: &str, model_id: &str) -> Result<Option<Health>> {
    Ok(list_health(conn, Some(provider_id))?.into_iter().find(|h| h.model_id.is_empty() || h.model_id == model_id))
}

/// States that speak for the provider account or organization, not just one connection's transport: while
/// one is active, other connections of the same quota group are not used for the same model (a
/// connection-wide entry: for any model). Counter-review CR-01: this holds across requests, not only within
/// the request that hit the refusal.
pub const QUOTA_STATES: [&str; 4] = ["invalid_credentials", "payment_required", "forbidden", "rate_limited"];

/// The active quota-state entry on *another* connection of `provider`'s quota group that covers `model_id`.
pub fn group_hold(conn: &Connection, provider: &Provider, model_id: &str) -> Result<Option<(Provider, Health)>> {
    let mut found = None;
    for other in list_providers(conn)? {
        if other.id == provider.id || other.quota_group != provider.quota_group {
            continue;
        }
        let hold = list_health(conn, Some(&other.id))?.into_iter().find(|h| {
            QUOTA_STATES.contains(&h.state.as_str()) && (h.model_id.is_empty() || h.model_id == model_id)
        });
        if let Some(hold) = hold {
            found = Some((other, hold));
            break;
        }
    }
    Ok(found)
}

// ---------- Session affinity (ROADMAP P0-4) ----------

/// How long a session stays pinned after its last request.
pub const AFFINITY_TTL_MS: i64 = 2 * 60 * 60 * 1000;

pub fn pinned_target(conn: &Connection, client_id: &str, session_key: &str, requested: &str) -> Result<Option<Target>> {
    conn.query_row(
        "SELECT provider_id, model_id FROM session_affinity WHERE client_id = ?1 AND session_key = ?2 \
         AND requested_model = ?3 AND last_used_at >= ?4",
        params![client_id, session_key, requested, now_ms() - AFFINITY_TTL_MS],
        |r| Ok(Target { provider_id: r.get(0)?, model_id: r.get(1)? }),
    )
    .optional()
    .map_err(err)
}

/// Deletes expired pins, of one client or of all. Clients may name as many sessions as they like; only the
/// last two hours of them are kept.
pub fn purge_sessions(conn: &Connection, client_id: Option<&str>) -> Result<usize> {
    conn.execute("DELETE FROM session_affinity WHERE (?1 IS NULL OR client_id = ?1) AND last_used_at < ?2",
        params![client_id, now_ms() - AFFINITY_TTL_MS]).map_err(err)
}

pub fn pin(conn: &Connection, client_id: &str, session_key: &str, requested: &str, target: &Target) -> Result<()> {
    let now = now_ms();
    purge_sessions(conn, Some(client_id))?;
    conn.execute(
        "INSERT INTO session_affinity (client_id, session_key, requested_model, provider_id, model_id, created_at, last_used_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6) ON CONFLICT(client_id, session_key, requested_model) DO UPDATE SET \
         provider_id = ?4, model_id = ?5, last_used_at = ?6, \
         created_at = CASE WHEN provider_id = ?4 AND model_id = ?5 THEN created_at ELSE ?6 END",
        params![client_id, session_key, requested, target.provider_id, target.model_id, now],
    )
    .map(|_| ())
    .map_err(err)
}

pub fn active_sessions(conn: &Connection, client_id: &str) -> Result<usize> {
    conn.query_row("SELECT COUNT(*) FROM session_affinity WHERE client_id = ?1 AND last_used_at >= ?2",
        params![client_id, now_ms() - AFFINITY_TTL_MS], |r| r.get::<_, i64>(0))
        .map(|n| n.max(0) as usize)
        .map_err(err)
}

/// The active client a bearer token belongs to.
pub fn authenticate(conn: &Connection, token: &str) -> Result<Option<Client>> {
    conn.query_row(
        &format!("SELECT {CLIENT_COLUMNS} FROM gateway_clients WHERE token_hash = ?1 AND revoked_at IS NULL"),
        [hash_token(token)],
        |r| client_row(conn, r),
    )
    .optional()
    .map_err(err)
}

// ---------- Combos ----------

#[derive(Clone, Debug, Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub provider_id: String,
    pub model_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Combo {
    pub id: String,
    pub name: String,
    pub members: Vec<Target>,
}

pub fn list_combos(conn: &Connection) -> Result<Vec<Combo>> {
    let mut stmt = conn.prepare("SELECT id, name FROM combos ORDER BY name").map_err(err)?;
    let heads: Vec<(String, String)> =
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).map_err(err)?.collect::<rusqlite::Result<_>>().map_err(err)?;
    let mut combos = Vec::new();
    for (id, name) in heads {
        let mut members = conn
            .prepare("SELECT provider_id, model_id FROM combo_members WHERE combo_id = ?1 ORDER BY position")
            .map_err(err)?;
        let members = members
            .query_map([&id], |r| Ok(Target { provider_id: r.get(0)?, model_id: r.get(1)? }))
            .map_err(err)?
            .collect::<rusqlite::Result<_>>()
            .map_err(err)?;
        combos.push(Combo { id, name, members });
    }
    Ok(combos)
}

/// Creates or replaces a combo. Names cannot contain `/`, so they never collide with `<connection>/<model>`.
pub fn save_combo(conn: &Connection, id: Option<&str>, name: &str, members: &[Target]) -> Result<Combo> {
    let name = name.trim();
    if name.is_empty() || name.contains('/') || name.contains(char::is_whitespace) {
        return Err("A combo name needs at least one character and no spaces or slashes".into());
    }
    if members.is_empty() {
        return Err("A combo needs at least one model".into());
    }
    let tx = conn.unchecked_transaction().map_err(err)?;
    let id = match id {
        Some(id) => {
            tx.execute("UPDATE combos SET name = ?2 WHERE id = ?1", params![id, name]).map_err(|e| unique(e, name))?;
            tx.execute("DELETE FROM combo_members WHERE combo_id = ?1", [id]).map_err(err)?;
            id.to_string()
        }
        None => {
            let id = uuid::Uuid::new_v4().to_string();
            tx.execute("INSERT INTO combos (id, name, created_at) VALUES (?1, ?2, ?3)", params![id, name, now_ms()])
                .map_err(|e| unique(e, name))?;
            id
        }
    };
    for (position, member) in members.iter().enumerate() {
        tx.execute(
            "INSERT INTO combo_members (combo_id, position, provider_id, model_id) VALUES (?1, ?2, ?3, ?4)",
            params![id, position as i64, member.provider_id, member.model_id],
        )
        .map_err(|e| format!("{}: {e}", member.provider_id))?;
    }
    tx.commit().map_err(err)?;
    list_combos(conn)?.into_iter().find(|c| c.id == id).ok_or_else(|| "combo was not saved".into())
}

fn unique(e: rusqlite::Error, name: &str) -> String {
    if e.to_string().contains("UNIQUE") { format!("A combo named {name} already exists") } else { e.to_string() }
}

pub fn delete_combo(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM combos WHERE id = ?1", [id]).map(|_| ()).map_err(err)
}

/// What a requested model name routes to: a combo's members in order, or one `<connection>/<model>`.
/// Disabled connections are skipped.
pub fn resolve(conn: &Connection, requested: &str) -> Result<Vec<(Provider, String)>> {
    let combo = list_combos(conn)?.into_iter().find(|c| c.name == requested);
    let targets = match combo {
        Some(combo) => combo.members,
        None => match requested.split_once('/') {
            Some((provider_id, model_id)) if !model_id.is_empty() => {
                vec![Target { provider_id: provider_id.into(), model_id: model_id.into() }]
            }
            _ => Vec::new(),
        },
    };
    let mut out = Vec::new();
    for target in targets {
        if let Some(provider) = get_provider(conn, &target.provider_id)? {
            if provider.enabled {
                out.push((provider, target.model_id));
            }
        }
    }
    Ok(out)
}

// ---------- Usage ----------

pub struct UsageRecord<'a> {
    pub client_id: Option<&'a str>,
    pub provider_id: &'a str,
    pub model_id: &'a str,
    pub requested_model: &'a str,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_tokens: Option<u64>,
    pub cost: &'a Cost,
    /// `ok`, `fallback` (failed, the next target was tried), `error`, `client_disconnected`, `stream_error`.
    pub outcome: &'a str,
    pub http_status: Option<u16>,
    pub latency_ms: u64,
    pub error: Option<&'a str>,
    /// Prompt tokens written to the provider's cache (Anthropic `cache_creation_input_tokens`).
    pub cache_write_tokens: Option<u64>,
    pub session_key: Option<&'a str>,
    /// `native` or `translated:anthropic->openai`.
    pub route_mode: Option<&'a str>,
}

impl<'a> UsageRecord<'a> {
    /// An attempt that produced no usage: a rejection, a refused connection, a key that could not be read.
    pub fn failed(client_id: &'a str, provider_id: &'a str, model_id: &'a str, requested_model: &'a str, cost: &'a Cost,
        outcome: &'a str, http_status: Option<u16>, latency_ms: u64, error: &'a str) -> UsageRecord<'a> {
        UsageRecord { client_id: Some(client_id), provider_id, model_id, requested_model, input_tokens: None,
            output_tokens: None, cache_tokens: None, cost, outcome, http_status, latency_ms, error: Some(error),
            cache_write_tokens: None, session_key: None, route_mode: None }
    }
}

/// A ledger entry outside the attempt journal (tests, imports). The gateway settles attempts with `settle`.
pub fn record_usage(conn: &Connection, r: &UsageRecord) -> Result<()> {
    insert_usage(conn, r, None).map_err(err)
}

fn insert_usage(conn: &Connection, r: &UsageRecord, at_risk_micro: Option<u64>) -> rusqlite::Result<()> {
    let now = now_ms();
    conn.execute(
        "INSERT INTO usage_events (id, client_id, provider, model, input_tokens, output_tokens, cache_tokens, \
         cost_micro_usd, cost_status, source, created_at, outcome, http_status, latency_ms, requested_model, error, \
         cache_write_tokens, session_key, route_mode, at_risk_micro_usd) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
        params![
            uuid::Uuid::new_v4().to_string(), r.client_id, r.provider_id, r.model_id,
            r.input_tokens.map(|v| v as i64), r.output_tokens.map(|v| v as i64), r.cache_tokens.map(|v| v as i64),
            r.cost.micro_usd().map(|v| v as i64), r.cost.status(), r.cost.source(), now, r.outcome,
            r.http_status, r.latency_ms as i64, r.requested_model, r.error,
            r.cache_write_tokens.map(|v| v as i64), r.session_key, r.route_mode, at_risk_micro.map(|v| v as i64),
        ],
    )?;
    if let Some(client) = r.client_id {
        conn.execute("UPDATE gateway_clients SET last_used_at = ?2 WHERE id = ?1", params![client, now])?;
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageEvent {
    pub id: String,
    pub created_at: i64,
    pub client: Option<String>,
    pub provider: String,
    pub model: String,
    pub requested_model: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cost_micro_usd: Option<u64>,
    pub cost_status: String,
    pub cost_source: String,
    pub outcome: String,
    pub http_status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
    /// Prompt tokens read from the provider's cache.
    pub cache_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub session_key: Option<String>,
    pub route_mode: Option<String>,
    /// Counted against the client's limit although the cost is unknown: the amount held for the attempt.
    pub at_risk_micro_usd: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageGroup {
    pub key: String,
    pub requests: u64,
    /// Requests that ended in an error for the client.
    pub failed: u64,
    /// Attempts that failed but were followed by another model of the combo.
    pub fallbacks: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// Sum of actual and estimated costs.
    pub known_micro_usd: u64,
    pub actual_micro_usd: u64,
    pub unknown_requests: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    pub since: i64,
    pub total: UsageGroup,
    pub by_model: Vec<UsageGroup>,
    pub by_client: Vec<UsageGroup>,
    pub recent: Vec<UsageEvent>,
}

pub fn usage_summary(conn: &Connection, since: i64, recent_limit: usize) -> Result<UsageSummary> {
    let group = |key_sql: &str| -> Result<Vec<UsageGroup>> {
        let sql = format!(
            "SELECT {key_sql} AS k, COUNT(*), SUM(outcome NOT IN ('ok', 'fallback')), SUM(outcome = 'fallback'), COALESCE(SUM(input_tokens), 0), \
             COALESCE(SUM(output_tokens), 0), COALESCE(SUM(cost_micro_usd), 0), \
             COALESCE(SUM(CASE WHEN cost_status = 'actual' THEN cost_micro_usd END), 0), SUM(cost_status = 'unknown') \
             FROM usage_events u LEFT JOIN gateway_clients c ON c.id = u.client_id \
             WHERE u.created_at >= ?1 GROUP BY k ORDER BY COUNT(*) DESC"
        );
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let rows = stmt
            .query_map([since], |r| {
                let n = |i: usize| r.get::<_, Option<i64>>(i).map(|v| v.unwrap_or(0).max(0) as u64);
                Ok(UsageGroup {
                    key: r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    requests: n(1)?,
                    failed: n(2)?,
                    fallbacks: n(3)?,
                    input_tokens: n(4)?,
                    output_tokens: n(5)?,
                    known_micro_usd: n(6)?,
                    actual_micro_usd: n(7)?,
                    unknown_requests: n(8)?,
                })
            })
            .map_err(err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(err)
    };
    let total = group("'all'")?.into_iter().next().unwrap_or(UsageGroup { key: "all".into(), ..Default::default() });
    let by_model = group("u.provider || '/' || u.model")?;
    let by_client = group("COALESCE(c.name, '')")?;

    let mut stmt = conn
        .prepare(
            "SELECT u.id, u.created_at, c.name, u.provider, u.model, u.requested_model, u.input_tokens, u.output_tokens, \
             u.cost_micro_usd, u.cost_status, u.source, u.outcome, u.http_status, u.latency_ms, u.error, \
             u.cache_tokens, u.cache_write_tokens, u.session_key, u.route_mode, u.at_risk_micro_usd \
             FROM usage_events u LEFT JOIN gateway_clients c ON c.id = u.client_id \
             WHERE u.created_at >= ?1 ORDER BY u.created_at DESC LIMIT ?2",
        )
        .map_err(err)?;
    let recent = stmt
        .query_map(params![since, recent_limit as i64], |r| {
            let u = |i: usize| r.get::<_, Option<i64>>(i).map(|v| v.map(|v| v.max(0) as u64));
            Ok(UsageEvent {
                id: r.get(0)?,
                created_at: r.get(1)?,
                client: r.get(2)?,
                provider: r.get(3)?,
                model: r.get(4)?,
                requested_model: r.get(5)?,
                input_tokens: u(6)?,
                output_tokens: u(7)?,
                cost_micro_usd: u(8)?,
                cost_status: r.get(9)?,
                cost_source: r.get(10)?,
                outcome: r.get(11)?,
                http_status: r.get::<_, Option<i64>>(12)?.map(|v| v as u16),
                latency_ms: u(13)?,
                error: r.get(14)?,
                cache_tokens: u(15)?,
                cache_write_tokens: u(16)?,
                session_key: r.get(17)?,
                route_mode: r.get(18)?,
                at_risk_micro_usd: u(19)?,
            })
        })
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;
    Ok(UsageSummary { since, total, by_model, by_client, recent })
}

// ---------- Settings ----------

pub fn setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0)).optional().map_err(err)
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute("INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = ?2",
        params![key, value]).map(|_| ()).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn gemini(conn: &Connection) -> Provider {
        add_provider(conn, &NewProvider { kind: "gemini", label: "Google Gemini", endpoint: "https://example.test/v1/",
            credential_ref: Some("provider-x"), default_billing: "unknown", wire: "openai" }).unwrap()
    }

    #[test]
    fn connections_get_unique_slugs() {
        let conn = db::open_in_memory().unwrap();
        let a = gemini(&conn);
        let b = gemini(&conn);
        assert_eq!((a.id.as_str(), b.id.as_str()), ("google-gemini", "google-gemini-2"));
        assert_eq!(a.endpoint, "https://example.test/v1");
        assert_eq!(slug("  OpenRouter / Work "), "openrouter-work");
        assert!(add_provider(&conn, &NewProvider { kind: "custom", label: "x", endpoint: "ftp://x", credential_ref: None,
            default_billing: "paid", wire: "openai" }).is_err());
    }

    #[test]
    fn tokens_are_stored_hashed_and_can_be_revoked() {
        let conn = db::open_in_memory().unwrap();
        let (client, token) = create_client(&conn, "Codex", Some(5_000_000)).unwrap();
        assert!(token.starts_with("str-") && token.len() == 68);
        let stored: String = conn.query_row("SELECT token_hash FROM gateway_clients", [], |r| r.get(0)).unwrap();
        assert_ne!(stored, token);
        assert_eq!(authenticate(&conn, &token).unwrap().unwrap().id, client.id);
        assert!(authenticate(&conn, "str-wrong").unwrap().is_none());
        revoke_client(&conn, &client.id).unwrap();
        assert!(authenticate(&conn, &token).unwrap().is_none());
    }

    #[test]
    fn combos_resolve_in_order_and_skip_disabled_connections() {
        let conn = db::open_in_memory().unwrap();
        let a = gemini(&conn);
        let b = add_provider(&conn, &NewProvider { kind: "openrouter", label: "OpenRouter", endpoint: "https://o.test",
            credential_ref: None, default_billing: "paid", wire: "openai" }).unwrap();
        let members = vec![
            Target { provider_id: a.id.clone(), model_id: "flash".into() },
            Target { provider_id: b.id.clone(), model_id: "vendor/big".into() },
        ];
        save_combo(&conn, None, "coding", &members).unwrap();
        assert!(save_combo(&conn, None, "coding", &members).is_err(), "names are unique");
        assert!(save_combo(&conn, None, "a/b", &members).is_err());

        let routed: Vec<String> = resolve(&conn, "coding").unwrap().into_iter().map(|(p, m)| format!("{}/{m}", p.id)).collect();
        assert_eq!(routed, ["google-gemini/flash", "openrouter/vendor/big"]);
        update_provider(&conn, &a.id, Some(false), None, None).unwrap();
        assert_eq!(resolve(&conn, "coding").unwrap().len(), 1);
        assert_eq!(resolve(&conn, "openrouter/vendor/big").unwrap()[0].1, "vendor/big");
        assert!(resolve(&conn, "nothing").unwrap().is_empty());
    }

    #[test]
    fn usage_keeps_unknown_apart_from_known() {
        let conn = db::open_in_memory().unwrap();
        let p = gemini(&conn);
        let (client, _) = create_client(&conn, "Codex", None).unwrap();
        let known = Cost::Estimated { micro_usd: 1500, source: "price: user".into() };
        let unknown = Cost::Unknown { source: "no price".into() };
        for (cost, outcome) in [(&known, "ok"), (&unknown, "ok"), (&known, "fallback")] {
            record_usage(&conn, &UsageRecord { client_id: Some(&client.id), provider_id: &p.id, model_id: "flash",
                requested_model: "coding", input_tokens: Some(10), output_tokens: Some(5), cache_tokens: None, cost,
                outcome, http_status: Some(200), latency_ms: 12, error: None, cache_write_tokens: None, session_key: None,
                route_mode: None }).unwrap();
        }
        let summary = usage_summary(&conn, 0, 10).unwrap();
        assert_eq!((summary.total.requests, summary.total.failed, summary.total.fallbacks), (3, 0, 1));
        assert_eq!((summary.total.known_micro_usd, summary.total.unknown_requests), (3000, 1));
        assert_eq!(summary.by_client[0].key, "Codex");
        assert_eq!(summary.recent.len(), 3);
        let client = list_clients(&conn).unwrap().remove(0);
        assert_eq!((client.month_spend_micro_usd, client.month_unknown_requests), (3000, 1));
    }

    #[test]
    fn month_start_is_the_first_at_midnight_utc() {
        // 2026-11-11T15:30:00Z → 2026-11-01T00:00:00Z; 2024-03-01 stays itself.
        assert_eq!(month_start(1_794_411_000_000), 1_793_491_200_000);
        assert_eq!(month_start(1_709_251_200_000), 1_709_251_200_000);
    }
}
