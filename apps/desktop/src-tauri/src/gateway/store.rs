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
}

const PROVIDER_COLUMNS: &str =
    "id, kind, account_label, COALESCE(endpoint, ''), enabled, credential_ref, default_billing, data_use_terms, created_at";

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
}

pub fn add_provider(conn: &Connection, new: &NewProvider) -> Result<Provider> {
    if !BILLING.contains(&new.default_billing) {
        return Err(format!("unknown billing type {}", new.default_billing));
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
        "INSERT INTO providers (id, kind, account_label, endpoint, credential_ref, default_billing, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, new.kind, new.label.trim(), endpoint, new.credential_ref, new.default_billing, now_ms()],
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

pub fn update_provider(conn: &Connection, id: &str, enabled: Option<bool>, billing: Option<&str>) -> Result<()> {
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
}

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
    let (spend, unknown): (i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(cost_micro_usd), 0), COALESCE(SUM(cost_status = 'unknown'), 0) FROM usage_events \
         WHERE client_id = ?1 AND created_at >= ?2",
        params![id, since],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
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
    })
}

const CLIENT_COLUMNS: &str = "id, name, token_prefix, monthly_budget_micro_usd, created_at, last_used_at, revoked_at";

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
}

pub fn record_usage(conn: &Connection, r: &UsageRecord) -> Result<()> {
    let now = now_ms();
    conn.execute(
        "INSERT INTO usage_events (id, client_id, provider, model, input_tokens, output_tokens, cache_tokens, \
         cost_micro_usd, cost_status, source, created_at, outcome, http_status, latency_ms, requested_model, error) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            uuid::Uuid::new_v4().to_string(), r.client_id, r.provider_id, r.model_id,
            r.input_tokens.map(|v| v as i64), r.output_tokens.map(|v| v as i64), r.cache_tokens.map(|v| v as i64),
            r.cost.micro_usd().map(|v| v as i64), r.cost.status(), r.cost.source(), now, r.outcome,
            r.http_status, r.latency_ms as i64, r.requested_model, r.error,
        ],
    )
    .map_err(err)?;
    if let Some(client) = r.client_id {
        conn.execute("UPDATE gateway_clients SET last_used_at = ?2 WHERE id = ?1", params![client, now]).map_err(err)?;
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
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageGroup {
    pub key: String,
    pub requests: u64,
    pub failed: u64,
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
            "SELECT {key_sql} AS k, COUNT(*), SUM(outcome NOT IN ('ok')), COALESCE(SUM(input_tokens), 0), \
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
                    input_tokens: n(3)?,
                    output_tokens: n(4)?,
                    known_micro_usd: n(5)?,
                    actual_micro_usd: n(6)?,
                    unknown_requests: n(7)?,
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
             u.cost_micro_usd, u.cost_status, u.source, u.outcome, u.http_status, u.latency_ms, u.error \
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
            credential_ref: Some("provider-x"), default_billing: "unknown" }).unwrap()
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
            default_billing: "paid" }).is_err());
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
            credential_ref: None, default_billing: "paid" }).unwrap();
        let members = vec![
            Target { provider_id: a.id.clone(), model_id: "flash".into() },
            Target { provider_id: b.id.clone(), model_id: "vendor/big".into() },
        ];
        save_combo(&conn, None, "coding", &members).unwrap();
        assert!(save_combo(&conn, None, "coding", &members).is_err(), "names are unique");
        assert!(save_combo(&conn, None, "a/b", &members).is_err());

        let routed: Vec<String> = resolve(&conn, "coding").unwrap().into_iter().map(|(p, m)| format!("{}/{m}", p.id)).collect();
        assert_eq!(routed, ["google-gemini/flash", "openrouter/vendor/big"]);
        update_provider(&conn, &a.id, Some(false), None).unwrap();
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
                outcome, http_status: Some(200), latency_ms: 12, error: None }).unwrap();
        }
        let summary = usage_summary(&conn, 0, 10).unwrap();
        assert_eq!((summary.total.requests, summary.total.failed), (3, 1));
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
