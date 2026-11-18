//! Gateway records in the local store: connections, their models, client tokens, combos and the usage
//! ledger. Used by the gateway's request handlers and by the interface's commands alike.

use super::pricing::{Cost, Price};
use crate::db::now_ms;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
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
    /// `no_training`, `may_train` or `unknown`: what the provider's terms say about training on what is sent. The
    /// user's record, with where it was checked; Pygg does not verify it.
    pub data_use_terms: String,
    pub terms_source: Option<String>,
    pub terms_checked_at: Option<i64>,
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
    /// The pool it belongs to (ADR-034): requests for `<pool>/<model>` try the pool's connections in order.
    pub pool: Option<String>,
    pub pool_rank: Option<i64>,
    /// A web session's own profile folder name under `websessions/` (ADR-035). Never shown or exported.
    #[serde(skip)]
    pub web_profile: Option<String>,
    /// Which text of the web-session warning the user acknowledged, and when (ADR-035).
    pub web_ack_version: Option<i64>,
    pub web_ack_at: Option<i64>,
    /// `signed_in`, `signed_out`, `needs_you` or `page_changed`, with Pygg's own reason and when it was seen.
    pub web_state: Option<String>,
    pub web_state_reason: Option<String>,
    pub web_state_at: Option<i64>,
    /// Requests a web session may send to its site per UTC day; `None` is Pygg's default (migration 0026).
    pub web_daily_limit: Option<u32>,
}

const PROVIDER_COLUMNS: &str =
    "id, kind, account_label, COALESCE(endpoint, ''), enabled, credential_ref, default_billing, data_use_terms, created_at, \
     wire, COALESCE(quota_group, kind), quota_group IS NOT NULL, quota_confirmed_at, terms_source, terms_checked_at, pool, pool_rank, \
     web_profile, web_ack_version, web_ack_at, web_state, web_state_reason, web_state_at, web_daily_limit";

fn provider_row(row: &rusqlite::Row) -> rusqlite::Result<Provider> {
    let kind: String = row.get(1)?;
    let separate = row.get::<_, i64>(11)? == 1;
    // Without the user's own group, connections of one provider family share limits (DeepSeek's OpenAI and
    // Anthropic endpoints take the same key).
    let quota_group = if separate { row.get(10)? } else { super::presets::quota_family(&kind).to_string() };
    Ok(Provider {
        id: row.get(0)?,
        kind,
        label: row.get(2)?,
        endpoint: row.get(3)?,
        enabled: row.get::<_, i64>(4)? == 1,
        credential_ref: row.get(5)?,
        default_billing: row.get(6)?,
        data_use_terms: row.get(7)?,
        terms_source: row.get(13)?,
        terms_checked_at: row.get(14)?,
        created_at: row.get(8)?,
        wire: row.get(9)?,
        quota_group,
        separate_quota: separate,
        quota_confirmed_at: row.get(12)?,
        pool: row.get(15)?,
        pool_rank: row.get(16)?,
        web_profile: row.get(17)?,
        web_ack_version: row.get(18)?,
        web_ack_at: row.get(19)?,
        web_state: row.get(20)?,
        web_state_reason: row.get(21)?,
        web_state_at: row.get(22)?,
        web_daily_limit: row.get::<_, Option<i64>>(23)?.map(|n| n.clamp(1, 1000) as u32),
    })
}

pub fn list_providers(conn: &Connection) -> Result<Vec<Provider>> {
    let mut stmt =
        conn.prepare(&format!("SELECT {PROVIDER_COLUMNS} FROM providers ORDER BY created_at")).map_err(err)?;
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
    if out.is_empty() {
        "connection".into()
    } else {
        out
    }
}

pub struct NewProvider<'a> {
    pub kind: &'a str,
    pub label: &'a str,
    pub endpoint: &'a str,
    pub credential_ref: Option<&'a str>,
    pub default_billing: &'a str,
    pub wire: &'a str,
}

/// Whether an endpoint may be used as written: any `https://` address, and `http://` only to this computer or the
/// local network (a loopback, private or link-local address, a name without a dot such as a container's, or a
/// `.local`, `.lan`, `.internal`, `.home.arpa`, `.localhost` or `.test` name). Over plain http the key and the
/// prompts cross the network unencrypted.
pub fn plain_http_allowed(endpoint: &str) -> bool {
    use std::net::IpAddr;
    let Ok(url) = reqwest::Url::parse(endpoint) else { return false };
    if url.scheme() != "http" {
        return true;
    }
    let Some(host) = url.host_str() else { return false };
    match host.trim_start_matches('[').trim_end_matches(']').parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        Ok(IpAddr::V6(ip)) => {
            ip.is_loopback()
                || (ip.segments()[0] & 0xfe00) == 0xfc00
                || (ip.segments()[0] & 0xffc0) == 0xfe80
                || ip.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback() || v4.is_private())
        }
        Err(_) => {
            let name = host.trim_end_matches('.').to_ascii_lowercase();
            name == "localhost"
                || !name.contains('.')
                || [".localhost", ".local", ".lan", ".internal", ".home.arpa", ".test"]
                    .iter()
                    .any(|s| name.ends_with(s))
        }
    }
}

pub fn add_provider(conn: &Connection, new: &NewProvider) -> Result<Provider> {
    if !BILLING.contains(&new.default_billing) {
        return Err(format!("unknown billing type {}", new.default_billing));
    }
    if !matches!(new.wire, "openai" | "anthropic") {
        return Err(format!("unknown wire format {}", new.wire));
    }
    if super::presets::is_web(new.kind) {
        return Err("A web session is added with its warning acknowledged (add_web_provider)".into());
    }
    let endpoint = new.endpoint.trim().trim_end_matches('/');
    if !(endpoint.starts_with("https://") || endpoint.starts_with("http://")) {
        return Err("The endpoint must start with https:// or http://".into());
    }
    if !plain_http_allowed(endpoint) {
        return Err("Use https:// for a provider on the internet: over http:// its key and every prompt would travel                     unencrypted. http:// is for this computer and the local network."
            .into());
    }
    let base = slug(new.label);
    // `auto/…` is the auto-routing alias, so no connection may take that name.
    let base = if base == AUTO { format!("{AUTO}-connection") } else { base };
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

/// Adds a web-session connection (ADR-035) after the user acknowledged the warning `ack_version`. It starts **off**,
/// billed as the user's plan with the site (cost unknown, never $0), with a profile folder name of its own and the
/// recipe's models, which take no tools or images. The profile folder itself is created by the window.
pub fn add_web_provider(conn: &Connection, kind: &str, label: &str, ack_version: i64) -> Result<Provider> {
    let preset = super::presets::find(kind).filter(|p| p.web).ok_or("Not a web-session provider")?;
    let recipe = crate::websession::recipe(kind).ok_or("No recipe for this site")?;
    if ack_version != crate::websession::WARNING_VERSION {
        return Err("Read and acknowledge the account-risk warning first".into());
    }
    // `Muse (muse.ai)` → `Muse`, so its models are `muse/muse`.
    let label =
        if label.trim().is_empty() { preset.name.split(" (").next().unwrap_or(preset.name) } else { label.trim() };
    let base = slug(label);
    let base = if base == AUTO { format!("{AUTO}-connection") } else { base };
    let mut id = base.clone();
    let mut n = 2;
    while get_provider(conn, &id)?.is_some() {
        id = format!("{base}-{n}");
        n += 1;
    }
    let now = now_ms();
    // Random, so a new connection never finds a removed one's folder (and its sign-in) under the same name.
    let profile = format!("p-{}", uuid::Uuid::new_v4().simple());
    conn.execute(
        "INSERT INTO providers (id, kind, account_label, endpoint, credential_ref, default_billing, wire, created_at, enabled, \
         web_profile, web_ack_version, web_ack_at, web_state, web_state_reason, web_state_at) \
         VALUES (?1, ?2, ?3, ?4, NULL, 'free', ?5, ?6, 0, ?7, ?8, ?6, 'signed_out', ?9, ?6)",
        params![id, kind, label, preset.endpoint.unwrap_or_default(), preset.wire, now, profile, ack_version,
            crate::websession::NOT_SIGNED_IN],
    ).map_err(err)?;
    let listed: Vec<Listed> = recipe
        .models
        .iter()
        .map(|m| Listed {
            id: m.to_string(),
            supports_tools: Some(false),
            supports_vision: Some(false),
            ..Listed::default()
        })
        .collect();
    import_listed(conn, &id, &listed, Some(crate::websession::CAPS_SOURCE))?;
    get_provider(conn, &id)?.ok_or_else(|| "connection was not saved".into())
}

/// Records what a web session's window last showed (ADR-035). `reason` is Pygg's own wording, never the page's text.
/// Signing in clears the holds a signed-out session left, so the connection is tried again.
pub fn set_web_state(conn: &Connection, provider_id: &str, state: &str, reason: &str) -> Result<()> {
    if !crate::websession::STATES.contains(&state) {
        return Err(format!("unknown web-session state {state}"));
    }
    let before: Option<String> = conn
        .query_row("SELECT web_state FROM providers WHERE id = ?1", [provider_id], |r| r.get(0))
        .optional()
        .map_err(err)?
        .flatten();
    conn.execute(
        "UPDATE providers SET web_state = ?2, web_state_reason = ?3, web_state_at = ?4 WHERE id = ?1",
        params![provider_id, state, reason, now_ms()],
    )
    .map_err(err)?;
    if state == "signed_in" && before.as_deref() != Some("signed_in") {
        clear_health(conn, provider_id)?;
    }
    // A site that suspended or restricted the account gets nothing more from Pygg until the user turns it back on.
    if reason == crate::websession::RESTRICTED {
        conn.execute("UPDATE providers SET enabled = 0 WHERE id = ?1", [provider_id]).map_err(err)?;
    }
    Ok(())
}

/// A web session's daily request limit; `None` returns it to Pygg's default.
pub fn set_web_daily_limit(conn: &Connection, provider_id: &str, limit: Option<u32>) -> Result<()> {
    let kind: String = conn
        .query_row("SELECT kind FROM providers WHERE id = ?1", [provider_id], |r| r.get(0))
        .optional()
        .map_err(err)?
        .ok_or("No such connection")?;
    let max = crate::websession::guard::limits(&kind).max_daily_limit;
    if limit.is_some_and(|n| !(1..=max).contains(&n)) {
        return Err(format!("A daily limit for this site is 1 to {max} requests"));
    }
    conn.execute("UPDATE providers SET web_daily_limit = ?2 WHERE id = ?1", params![provider_id, limit])
        .map(|_| ())
        .map_err(err)
}

/// Requests sent to a web session's site since 00:00 UTC: the web guard's reservations (`web_guard_reserve`), each
/// written before the site is touched, so a request still running, or one cut off by a crash, counts too. Whatever Pygg
/// refused itself never got one (Codex's fourth review, A4).
pub fn web_requests_today(conn: &Connection, provider_id: &str, now: i64) -> Result<u64> {
    let day_start = now - now.rem_euclid(86_400_000);
    conn.query_row(
        "SELECT COUNT(*) FROM web_pace WHERE provider_id = ?1 AND at >= ?2",
        params![provider_id, day_start],
        |r| r.get::<_, i64>(0),
    )
    .map(|n| n.max(0) as u64)
    .map_err(err)
}

/// The web guard's record of an account (ADR-035, migration 0028): the requests of the last day with their weight, and
/// its rest and back-off.
pub fn web_guard(conn: &Connection, provider_id: &str, now: i64) -> Result<crate::websession::guard::Account> {
    let mut stmt =
        conn.prepare("SELECT at, weight FROM web_pace WHERE provider_id = ?1 AND at > ?2 ORDER BY at").map_err(err)?;
    let sent = stmt
        .query_map(params![provider_id, now - crate::websession::guard::DAY_MS], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?.clamp(1, 1_000) as u32))
        })
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err)?;
    let (next_at, backoff_until, backoff_reason) = conn
        .query_row(
            "SELECT web_next_at, web_backoff_until, web_backoff_reason FROM providers WHERE id = ?1",
            [provider_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(err)?
        .unwrap_or((None, None, None));
    Ok(crate::websession::guard::Account { sent, next_at, backoff_until, backoff_reason })
}

/// Before a request touches a web session's site: keeps its weight (outcome `sent` until it ends) and a first rest until
/// `min_next_at`, so a crash mid-request still leaves the account its weight and a rest (Codex's fourth review, A1).
/// Returns the reservation's row, which `web_guard_after` finishes.
pub fn web_guard_reserve(conn: &Connection, provider_id: &str, at: i64, weight: u32, min_next_at: i64) -> Result<i64> {
    conn.execute(
        "INSERT INTO web_pace (provider_id, at, weight, outcome) VALUES (?1, ?2, ?3, 'sent')",
        params![provider_id, at, weight.max(1)],
    )
    .map_err(err)?;
    let row = conn.last_insert_rowid();
    conn.execute(
        "UPDATE providers SET web_next_at = MAX(COALESCE(web_next_at, 0), ?2) WHERE id = ?1",
        params![provider_id, min_next_at],
    )
    .map_err(err)?;
    conn.execute("DELETE FROM web_pace WHERE at < ?1", [at - 2 * crate::websession::guard::DAY_MS]).map_err(err)?;
    Ok(row)
}

/// After a request to a web session: finishes its reservation, sets the rest until `next_at`, and backs the account off
/// after a sign of trouble (doubling with each one in a row, from `backoff_base_ms`) or clears the back-off after a
/// clean reply.
#[allow(clippy::too_many_arguments)]
pub fn web_guard_after(
    conn: &Connection, provider_id: &str, row: i64, outcome: &str, next_at: i64,
    trouble: Option<&crate::websession::guard::Trouble>, backoff_base_ms: i64, now: i64,
) -> Result<()> {
    conn.execute("UPDATE web_pace SET outcome = ?2 WHERE rowid = ?1", params![row, outcome]).map_err(err)?;
    match trouble {
        Some(trouble) => web_guard_trouble(conn, provider_id, trouble, next_at, backoff_base_ms, now),
        None => conn
            .execute(
                "UPDATE providers SET web_next_at = ?2, web_strikes = 0, web_backoff_until = NULL, \
                web_backoff_reason = NULL WHERE id = ?1",
                params![provider_id, next_at],
            )
            .map(|_| ())
            .map_err(err),
    }
}

/// A sign of trouble on a web session's site (after a request, or seen by a sign-in check): the account rests at least
/// until `next_at`, and backs off for longer with each sign in a row, never less than the kind of trouble asks.
pub fn web_guard_trouble(
    conn: &Connection, provider_id: &str, trouble: &crate::websession::guard::Trouble, next_at: i64,
    backoff_base_ms: i64, now: i64,
) -> Result<()> {
    let strikes: i64 = conn
        .query_row("SELECT web_strikes FROM providers WHERE id = ?1", [provider_id], |r| r.get(0))
        .optional()
        .map_err(err)?
        .unwrap_or(0)
        + 1;
    let until = now + trouble.backoff_ms(backoff_base_ms, strikes as u32);
    conn.execute(
        "UPDATE providers SET web_next_at = ?2, web_strikes = ?3, web_backoff_until = ?4, web_backoff_reason = ?5 \
        WHERE id = ?1",
        params![provider_id, next_at.max(until), strikes, until, trouble.reason],
    )
    .map(|_| ())
    .map_err(err)
}

/// A sign-in check loaded the site's page: the account rests at least until `next_at` before the next request or check.
pub fn web_guard_checked(conn: &Connection, provider_id: &str, next_at: i64) -> Result<()> {
    conn.execute(
        "UPDATE providers SET web_next_at = MAX(COALESCE(web_next_at, 0), ?2) WHERE id = ?1",
        params![provider_id, next_at],
    )
    .map(|_| ())
    .map_err(err)
}

/// The user lifts a web session's rest and back-off (Providers → Resume Now). The hour's and the day's shares stay.
pub fn web_guard_resume(conn: &Connection, provider_id: &str) -> Result<()> {
    conn.execute("UPDATE providers SET web_next_at = NULL, web_strikes = 0, web_backoff_until = NULL, web_backoff_reason = NULL \
        WHERE id = ?1", [provider_id]).map(|_| ()).map_err(err)
}

/// Whether the web guard would refuse even a short question to this account now: resting, backed off, or out of its
/// hour's or day's share. A web pool sends to such an account last.
pub fn web_resting(conn: &Connection, provider: &Provider, now: i64) -> bool {
    use crate::websession::guard;
    let limits = guard::limits(&provider.kind);
    let limit = limits.daily_limit(provider.web_daily_limit.unwrap_or(crate::websession::DEFAULT_DAILY_LIMIT));
    web_guard(conn, &provider.id, now)
        .is_ok_and(|account| matches!(guard::decide(&account, 1, now, limit, &limits), guard::Verdict::Refuse { .. }))
}

/// Says what the window last refused to open, keeping the session's state: a sign-in that stalls on an address
/// outside the site's hosts shows why, by host only (owner, 2026-11-13: ChatGPT's Google sign-in kept spinning).
pub fn set_web_blocked(conn: &Connection, provider_id: &str, host: &str) -> Result<()> {
    let reason = format!(
        "Pygg blocked {host} in this window: it is not one of this site's sign-in addresses. If sign-in \
        stalls, this is why; the site's recipe needs it added."
    );
    conn.execute(
        "UPDATE providers SET web_state_reason = ?2, web_state_at = ?3 WHERE id = ?1",
        params![provider_id, reason, now_ms()],
    )
    .map(|_| ())
    .map_err(err)
}

/// The profile folder names of the web sessions that exist; any other folder under `websessions/` is a leftover.
pub fn web_profiles(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT web_profile FROM providers WHERE web_profile IS NOT NULL").map_err(err)?;
    let rows = stmt.query_map([], |r| r.get(0)).map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

/// Removes the connection with its models and combo entries; returns its credential reference so the
/// caller can remove the secret too.
pub fn remove_provider(conn: &Connection, id: &str) -> Result<Vec<String>> {
    let mut references: Vec<String> = get_provider(conn, id)?.and_then(|p| p.credential_ref).into_iter().collect();
    // Its key ring's keys go with it (ADR-038): their rows by the foreign key, their secrets by the caller.
    for reference in super::keys::refs(conn, id)? {
        if !references.contains(&reference) {
            references.push(reference);
        }
    }
    conn.execute("DELETE FROM providers WHERE id = ?1", [id]).map_err(err)?;
    Ok(references)
}

pub fn update_provider(
    conn: &Connection, id: &str, enabled: Option<bool>, billing: Option<&str>, separate_quota: Option<bool>,
) -> Result<()> {
    if let Some(separate) = separate_quota {
        conn.execute(
            "UPDATE providers SET quota_group = CASE WHEN ?2 THEN id END, \
             quota_confirmed_at = CASE WHEN ?2 THEN ?3 END WHERE id = ?1",
            params![id, separate, now_ms()],
        )
        .map_err(err)?;
    }
    if let Some(enabled) = enabled {
        conn.execute("UPDATE providers SET enabled = ?2 WHERE id = ?1", params![id, enabled as i64]).map_err(err)?;
    }
    if let Some(billing) = billing {
        if !BILLING.contains(&billing) {
            return Err(format!("unknown billing type {billing}"));
        }
        let kind: Option<String> =
            conn.query_row("SELECT kind FROM providers WHERE id = ?1", [id], |r| r.get(0)).optional().map_err(err)?;
        // A web session is the user's plan with the site: free ($0 a request, the default since migration 0027) or a
        // subscription (quota, cost unknown); never paid per token, and not a local model.
        if kind.as_deref().is_some_and(super::presets::is_web) && !matches!(billing, "free" | "subscription") {
            return Err("A web session is your plan with the site: billed as free ($0 a request) or as a subscription (quota, cost unknown), never as paid or local".into());
        }
        conn.execute("UPDATE providers SET default_billing = ?2 WHERE id = ?1", params![id, billing]).map_err(err)?;
        apply_free_connection_rule(conn)?;
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
    /// What auto routing knows about it (ROADMAP P0-5).
    pub capabilities: Capabilities,
    /// Who last switched it: `None` (Pygg's default), `user`, or `free-connection` (Pygg turned it off because it
    /// has a price on a connection set as free; see `apply_free_connection_rule`).
    pub enabled_source: Option<String>,
}

pub const TIERS: [&str; 5] = ["T0", "T1", "T2", "T3", "T4"];
/// A tier from Pygg's table rather than the user (gateway/tiers.rs).
pub const CURATED: &str = super::tiers::SOURCE;

/// The auto-routing alias (`auto`, `auto/cheap`); reserved as a connection and combo name.
pub const AUTO: &str = "auto";

/// What auto routing needs to know about a model. `None` is unknown, and unknown counts as missing.
#[derive(Clone, Debug, Default, PartialEq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    /// T0 (local) to T4 (frontier): the user's, or a starting tier from Pygg's table (`tier_source`).
    pub tier: Option<String>,
    /// `user`, or `curated:v2` for Pygg's tier table (gateway/tiers.rs).
    #[serde(default)]
    pub tier_source: Option<String>,
    /// Why the table gave this tier, when it did.
    #[serde(default)]
    pub tier_note: Option<String>,
    /// Prompt and answer together.
    pub context_tokens: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub supports_tools: Option<bool>,
    pub supports_vision: Option<bool>,
    /// The thinking types it accepts (`adaptive`, `enabled`), on the Anthropic API.
    #[serde(default)]
    pub thinking_types: Option<Vec<String>>,
    /// The effort levels it accepts (`low` to `max`), on the Anthropic API.
    #[serde(default)]
    pub effort_levels: Option<Vec<String>>,
    /// `user`, or `provider: <kind> /models` for facts the provider's model list reported.
    pub source: Option<String>,
    pub observed_at: Option<i64>,
}

/// What a provider's model list said about one model.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Listed {
    pub id: String,
    pub context_tokens: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub supports_tools: Option<bool>,
    pub supports_vision: Option<bool>,
    pub thinking_types: Option<Vec<String>>,
    pub effort_levels: Option<Vec<String>>,
    /// A list price, in micro-USD per million tokens (OpenRouter reports one).
    pub price: Option<Price>,
}

impl Listed {
    fn has_facts(&self) -> bool {
        self.context_tokens.is_some()
            || self.max_output_tokens.is_some()
            || self.supports_tools.is_some()
            || self.supports_vision.is_some()
            || self.thinking_types.is_some()
            || self.effort_levels.is_some()
    }
}

/// A connection (`p`) set as free whose free part is a set of $0 models (presets.rs `free_models`). The kinds are
/// checked against the presets by a test.
macro_rules! free_models_connection {
    () => {
        "(p.default_billing = 'free' AND p.kind IN ('openrouter', 'zai', 'zai-anthropic'))"
    };
}

/// A model (`m`) listed at a price above zero.
macro_rules! priced {
    () => {
        "(COALESCE(json_extract(m.price_json, '$.inputMicroPerMtok'), 0) > 0 \
              OR COALESCE(json_extract(m.price_json, '$.outputMicroPerMtok'), 0) > 0)"
    };
}

/// A model (`m`) listed at $0 for input and output.
macro_rules! listed_free {
    () => {
        "(m.price_json IS NOT NULL AND COALESCE(json_extract(m.price_json, '$.inputMicroPerMtok'), 0) = 0 \
              AND COALESCE(json_extract(m.price_json, '$.outputMicroPerMtok'), 0) = 0)"
    };
}

/// A model's billing is its own, or its connection's when it has none. On a connection set as free whose free part
/// is a set of $0 models (OpenRouter lists its paid models next to its `:free` ones), only a model listed at $0 is
/// free: one at a price is paid, and one without a price is unknown (OpenRouter's own routers, `openrouter/auto`, list
/// none and may pick a paid model), for ranking, holds and the ledger. Found on 2026-11-12: `auto` sent an owner's
/// request to a paid OpenRouter model as free, and only the account's empty balance (HTTP 402) stopped it being billed
/// as $0. Where the account itself is free (a trial key, a free tier), a list price does not make a call paid.
macro_rules! billing {
    () => {
        concat!(
            "CASE WHEN m.billing <> 'unknown' THEN m.billing WHEN ",
            free_models_connection!(),
            " AND ",
            priced!(),
            " THEN 'paid' WHEN ",
            free_models_connection!(),
            " AND NOT ",
            listed_free!(),
            " THEN 'unknown' ELSE p.default_billing END"
        )
    };
}

const MODEL_COLUMNS: &str = concat!("m.provider_id, m.model_id, ", billing!(), ", \
     m.enabled, m.price_json, m.observed_at, CASE WHEN m.tier IS NULL THEN NULL ELSE 'T' || m.tier END, m.context_tokens, m.max_output_tokens, m.supports_tools, \
     m.supports_vision, m.caps_source, m.caps_observed_at, m.thinking_types, m.effort_levels, m.tier_source, p.kind, m.enabled_source");

/// The SQL condition for such a connection, for the test that checks its kinds against the presets.
pub const FREE_MODELS_CONNECTION: &str = free_models_connection!();

/// Why Pygg turned a model off: it is not listed at $0, on a connection set as free whose free part is its $0 models.
pub const OFF_ON_FREE_CONNECTION: &str = "free-connection";

/// On a connection set as free whose free part is a set of $0 models, only the models listed at $0 start on: the
/// user connected it for those, and turns a paid one (or one without a price) on when they mean to use it (ADR-032).
/// A model the user switched either way is theirs. A model Pygg turned off is turned back on when that stops being
/// true (the connection is no longer free, or the model's price or billing changed). Returns (turned off, turned
/// back on).
pub fn apply_free_connection_rule(conn: &Connection) -> Result<(usize, usize)> {
    const PAID_HERE: &str = concat!(
        "SELECT m.id FROM models m JOIN providers p ON p.id = m.provider_id WHERE ",
        free_models_connection!(),
        " AND (m.billing = 'paid' OR (m.billing = 'unknown' AND NOT ",
        listed_free!(),
        "))"
    );
    let off = conn
        .execute(
            &format!(
                "UPDATE models SET enabled = 0, enabled_source = '{OFF_ON_FREE_CONNECTION}' \
         WHERE enabled = 1 AND enabled_source IS NULL AND id IN ({PAID_HERE})"
            ),
            [],
        )
        .map_err(err)?;
    let on = conn
        .execute(
            &format!(
                "UPDATE models SET enabled = 1, enabled_source = NULL \
         WHERE enabled_source = '{OFF_ON_FREE_CONNECTION}' AND id NOT IN ({PAID_HERE})"
            ),
            [],
        )
        .map_err(err)?;
    Ok((off, on))
}

fn model_row(row: &rusqlite::Row) -> rusqlite::Result<Model> {
    let price: Option<String> = row.get(4)?;
    let u = |i: usize| row.get::<_, Option<i64>>(i).map(|v| v.map(|v| v.max(0) as u64));
    let b = |i: usize| row.get::<_, Option<i64>>(i).map(|v| v.map(|v| v == 1));
    // A list that cannot be read is unknown, which counts as missing.
    let list = |i: usize| {
        row.get::<_, Option<String>>(i).map(|v| v.and_then(|j| serde_json::from_str::<Vec<String>>(&j).ok()))
    };
    Ok(Model {
        provider_id: row.get(0)?,
        model_id: row.get(1)?,
        billing: row.get(2)?,
        enabled: row.get::<_, i64>(3)? == 1,
        price: price.and_then(|p| serde_json::from_str(&p).ok()),
        observed_at: row.get(5)?,
        capabilities: Capabilities {
            tier: row.get(6)?,
            context_tokens: u(7)?,
            max_output_tokens: u(8)?,
            supports_tools: b(9)?,
            supports_vision: b(10)?,
            source: row.get(11)?,
            observed_at: row.get(12)?,
            thinking_types: list(13)?,
            effort_levels: list(14)?,
            tier_note: match row.get::<_, Option<String>>(15)?.as_deref() {
                Some(super::tiers::SOURCE) => {
                    super::tiers::suggest(&row.get::<_, String>(16)?, &row.get::<_, String>(1)?)
                        .map(|(_, why)| why.to_string())
                }
                Some(super::reference::TIER_SOURCE) => Some(super::reference::TIER_NOTE.to_string()),
                _ => None,
            },
            tier_source: row.get(15)?,
        },
        enabled_source: row.get(17)?,
    })
}

pub fn list_models(conn: &Connection, provider_id: Option<&str>) -> Result<Vec<Model>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {MODEL_COLUMNS} FROM models m JOIN providers p ON p.id = m.provider_id \
             WHERE ?1 IS NULL OR m.provider_id = ?1 ORDER BY m.provider_id, m.model_id"
        ))
        .map_err(err)?;
    let rows = stmt.query_map([provider_id], model_row).map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

/// Gives models whose tier nobody set a starting tier from Pygg's table (gateway/tiers.rs), source `curated:v2`. A
/// tier the user set or cleared is never touched. Returns how many were filled.
pub fn fill_curated_tiers(conn: &Connection) -> Result<usize> {
    // Tiers an earlier version of the table filled follow the current one; a tier the user set or cleared is theirs.
    let mut older = conn
        .prepare(
            "SELECT m.provider_id, m.model_id, p.kind, m.tier FROM models m JOIN providers p ON p.id = m.provider_id \
         WHERE m.tier_source LIKE 'curated:v%' AND m.tier_source <> ?1",
        )
        .map_err(err)?;
    let stale: Vec<(String, String, String, Option<i64>)> = older
        .query_map([super::tiers::SOURCE], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;
    for (provider, model, kind, _) in stale {
        let tier = super::tiers::suggest(&kind, &model)
            .and_then(|(t, _)| TIERS.iter().position(|x| *x == t))
            .map(|n| n as i64);
        conn.execute(
            "UPDATE models SET tier = ?3, tier_source = CASE WHEN ?3 IS NULL THEN NULL ELSE ?4 END \
                      WHERE provider_id = ?1 AND model_id = ?2",
            params![provider, model, tier, super::tiers::SOURCE],
        )
        .map_err(err)?;
    }
    let mut statement = conn
        .prepare(
            "SELECT m.provider_id, m.model_id, p.kind FROM models m JOIN providers p ON p.id = m.provider_id \
         WHERE m.tier IS NULL AND m.tier_source IS NULL",
        )
        .map_err(err)?;
    let rows: Vec<(String, String, String)> = statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;
    let mut filled = 0;
    for (provider, model, kind) in rows {
        let Some((tier, _)) = super::tiers::suggest(&kind, &model) else { continue };
        let Some(n) = TIERS.iter().position(|t| *t == tier) else { continue };
        filled += conn.execute("UPDATE models SET tier = ?3, tier_source = ?4 WHERE provider_id = ?1 AND model_id = ?2 AND tier IS NULL AND tier_source IS NULL",
            params![provider, model, n as i64, super::tiers::SOURCE]).map_err(err)?;
    }
    fill_curated_tools(conn)?;
    Ok(filled)
}

/// Where Pygg's table says a model takes tools, with why (`caps_source`).
pub const CURATED_TOOLS: &str = "curated:v1 (Anthropic documents tool use for every Claude model on its API)";

/// Fills tool support that a provider's own model list does not state but its documentation does: every Claude model
/// on Anthropic's API takes tools, and Anthropic's `/v1/models` does not say so. Only an unknown is filled, and never
/// where the user set the model's facts; the other facts keep their values, and the source names both.
pub fn fill_curated_tools(conn: &Connection) -> Result<usize> {
    conn.execute(
        "UPDATE models SET supports_tools = 1, caps_source = CASE WHEN caps_source IS NULL THEN ?1 ELSE caps_source || '; tools: ' || ?1 END \
         WHERE supports_tools IS NULL AND (caps_source IS NULL OR caps_source <> 'user') \
         AND provider_id IN (SELECT id FROM providers WHERE kind = 'anthropic')",
        [CURATED_TOOLS]).map_err(err)
}

/// Adds models the provider listed; existing rows keep their settings. Returns how many were new.
pub fn import_models(conn: &Connection, provider_id: &str, model_ids: &[String]) -> Result<usize> {
    let listed: Vec<Listed> = model_ids.iter().map(|id| Listed { id: id.clone(), ..Listed::default() }).collect();
    import_listed(conn, provider_id, &listed, None)
}

/// Adds models the provider listed, with what the list said about them (`source`, e.g. `provider: openrouter
/// /models`). Facts and a list price are written only where the user has not set their own: what the user
/// entered is never overwritten. Returns how many models were new.
pub fn import_listed(conn: &Connection, provider_id: &str, listed: &[Listed], source: Option<&str>) -> Result<usize> {
    let now = now_ms();
    let mut added = 0;
    for model in listed {
        added += conn
            .execute(
                "INSERT OR IGNORE INTO models (id, provider_id, model_id, observed_at) VALUES (?1, ?2, ?3, ?4)",
                params![format!("{provider_id}/{}", model.id), provider_id, model.id, now],
            )
            .map_err(err)?;
        conn.execute(
            "UPDATE models SET observed_at = ?3 WHERE provider_id = ?1 AND model_id = ?2",
            params![provider_id, model.id, now],
        )
        .map_err(err)?;
        if let (Some(source), true) = (source, model.has_facts()) {
            conn.execute(
                "UPDATE models SET context_tokens = ?3, max_output_tokens = ?4, supports_tools = ?5, supports_vision = ?6, \
                 caps_source = ?7, caps_observed_at = ?8, thinking_types = ?9, effort_levels = ?10 \
                 WHERE provider_id = ?1 AND model_id = ?2 AND (caps_source IS NULL OR caps_source <> 'user')",
                params![provider_id, model.id, model.context_tokens.map(|v| v as i64), model.max_output_tokens.map(|v| v as i64),
                    model.supports_tools, model.supports_vision, source, now, json_list(&model.thinking_types), json_list(&model.effort_levels)],
            ).map_err(err)?;
        }
        if let Some(price) = &model.price {
            let json = serde_json::to_string(price).unwrap_or_default();
            conn.execute(
                "UPDATE models SET price_json = ?3 WHERE provider_id = ?1 AND model_id = ?2 \
                 AND (price_json IS NULL OR json_extract(price_json, '$.source') <> 'user')",
                params![provider_id, model.id, json],
            )
            .map_err(err)?;
        }
    }
    fill_curated_tiers(conn)?;
    apply_free_connection_rule(conn)?;
    Ok(added)
}

fn json_list(list: &Option<Vec<String>>) -> Option<String> {
    list.as_ref().map(|l| serde_json::to_string(l).unwrap_or_else(|_| "[]".into()))
}

/// Sets what auto routing knows about a model, as the user's own statement (source `user`).
pub fn set_capabilities(conn: &Connection, provider_id: &str, model_id: &str, caps: &Capabilities) -> Result<()> {
    if let Some(tier) = &caps.tier {
        if !TIERS.contains(&tier.as_str()) {
            return Err(format!("unknown tier {tier}"));
        }
    }
    if caps.context_tokens == Some(0) || caps.max_output_tokens == Some(0) {
        return Err("Token limits must be above zero".into());
    }
    let tier = caps.tier.as_deref().and_then(|t| TIERS.iter().position(|x| *x == t)).map(|n| n as i64);
    let changed = conn.execute(
        // Always `user`, a cleared tier included: the tier table then never fills it again.
        "UPDATE models SET tier = ?3, tier_source = 'user', context_tokens = ?4, \
         max_output_tokens = ?5, supports_tools = ?6, supports_vision = ?7, caps_source = 'user', caps_observed_at = ?8, \
         thinking_types = ?9, effort_levels = ?10 WHERE provider_id = ?1 AND model_id = ?2",
        params![provider_id, model_id, tier, caps.context_tokens.map(|v| v as i64), caps.max_output_tokens.map(|v| v as i64),
            caps.supports_tools, caps.supports_vision, now_ms(), json_list(&caps.thinking_types), json_list(&caps.effort_levels)],
    ).map_err(err)?;
    if changed == 0 {
        return Err("Model not found".into());
    }
    Ok(())
}

/// Adds a model by hand (for providers that do not list theirs).
pub fn add_model(conn: &Connection, provider_id: &str, model_id: &str) -> Result<()> {
    let model_id = model_id.trim();
    if model_id.is_empty() {
        return Err("Enter a model name".into());
    }
    import_models(conn, provider_id, &[model_id.to_string()]).map(|_| ())
}

pub fn update_model(
    conn: &Connection, provider_id: &str, model_id: &str, enabled: Option<bool>, billing: Option<&str>,
    price: Option<Option<Price>>,
) -> Result<()> {
    if let Some(enabled) = enabled {
        conn.execute(
            "UPDATE models SET enabled = ?3, enabled_source = 'user' WHERE provider_id = ?1 AND model_id = ?2",
            params![provider_id, model_id, enabled as i64],
        )
        .map_err(err)?;
    }
    if let Some(billing) = billing {
        if !BILLING.contains(&billing) {
            return Err(format!("unknown billing type {billing}"));
        }
        conn.execute(
            "UPDATE models SET billing = ?3 WHERE provider_id = ?1 AND model_id = ?2",
            params![provider_id, model_id, billing],
        )
        .map_err(err)?;
    }
    if let Some(price) = price {
        let json = price.map(|p| serde_json::to_string(&p).unwrap_or_default());
        conn.execute(
            "UPDATE models SET price_json = ?3 WHERE provider_id = ?1 AND model_id = ?2",
            params![provider_id, model_id, json],
        )
        .map_err(err)?;
    }
    apply_free_connection_rule(conn)?;
    Ok(())
}

pub fn remove_model(conn: &Connection, provider_id: &str, model_id: &str) -> Result<()> {
    conn.execute("DELETE FROM models WHERE provider_id = ?1 AND model_id = ?2", params![provider_id, model_id])
        .map(|_| ())
        .map_err(err)
}

/// Billing and price for a routed model, by the same rule as the model list (`billing!`); a model without a row uses
/// its connection's billing type.
pub fn pricing_for(conn: &Connection, provider: &Provider, model_id: &str) -> Result<(String, Option<Price>)> {
    let row: Option<(String, Option<String>)> = conn
        .query_row(
            concat!(
                "SELECT ",
                billing!(),
                ", m.price_json FROM models m JOIN providers p ON p.id = m.provider_id \
             WHERE m.provider_id = ?1 AND m.model_id = ?2"
            ),
            params![provider.id, model_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(err)?;
    Ok(match row {
        Some((billing, price)) => (billing, price.and_then(|p| serde_json::from_str(&p).ok())),
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
    /// The connections this client may send requests to; `None` for every connection (review FR-01).
    pub allowed_connections: Option<Vec<String>>,
    /// Further targets that may be tried after a failure the provider may have billed (review FR-02).
    pub paid_retries: u32,
    /// The lowest tier `auto` may choose for this client (ROADMAP P0-5).
    pub auto_min_tier: String,
    /// Whether it may start smart tasks through the gateway's task API (off until the user allows it).
    pub may_run_tasks: bool,
    /// Only connections whose recorded terms say the provider does not train on what is sent (review GW-10).
    pub require_no_training: bool,
    /// A strict monthly limit (ROADMAP B4): each request is held at the most it could cost, and a model whose price
    /// or maximum output is unknown is not used.
    pub strict_budget: bool,
    /// Whether a copy of the token is in the OS credential store, so it can be shown again (migration 0023).
    pub token_viewable: bool,
}

impl Client {
    /// What the monthly limit is checked against: known spend plus spend at risk.
    pub fn counted_micro_usd(&self) -> u64 {
        self.month_spend_micro_usd + self.month_at_risk_micro_usd
    }

    /// Whether this client may send a request to the connection. Fails closed: a restricted client may use only
    /// the connections it names.
    /// Whether this client may send a request to the connection, and why not: its connection list, then the data-use
    /// terms it requires (review GW-10). Fails closed: unknown terms count as may-train.
    pub fn allows(&self, provider: &Provider) -> std::result::Result<(), String> {
        if !self.may_use(&provider.id) {
            return Err(format!("this client may not use {}", provider.label));
        }
        if self.require_no_training && provider.data_use_terms != "no_training" {
            return Err(format!(
                "this client sends only to providers that do not train on its data, and {}'s terms are {}",
                provider.label,
                if provider.data_use_terms == "may_train" { "that it may" } else { "not recorded" }
            ));
        }
        Ok(())
    }

    pub fn may_use(&self, provider_id: &str) -> bool {
        self.allowed_connections.as_ref().is_none_or(|list| list.iter().any(|id| id == provider_id))
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
        "SELECT COALESCE(SUM(reserved_micro), 0) FROM budget_reservations WHERE client_id = ?1",
        [&id],
        |r| r.get(0),
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
        max_concurrency: row.get::<_, i64>(7)?.max(1) as u32,
        unknown_cost_policy: row.get(8)?,
        reserved_micro_usd: reserved.max(0) as u64,
        month_at_risk_micro_usd: at_risk.max(0) as u64,
        // A list that cannot be read allows nothing, rather than everything.
        allowed_connections: row
            .get::<_, Option<String>>(9)?
            .map(|json| serde_json::from_str::<Vec<String>>(&json).unwrap_or_default()),
        paid_retries: row.get::<_, i64>(10)?.clamp(0, 5) as u32,
        auto_min_tier: row.get(11)?,
        may_run_tasks: row.get(12)?,
        require_no_training: row.get(13)?,
        strict_budget: row.get(14)?,
        token_viewable: row.get(15)?,
    })
}

const CLIENT_COLUMNS: &str = "id, name, token_prefix, monthly_budget_micro_usd, created_at, last_used_at, revoked_at, \
    max_concurrency, unknown_cost_policy, allowed_connections, paid_retries, auto_min_tier, may_run_tasks, require_no_training, strict_budget, \
    token_ref IS NOT NULL";

pub fn list_clients(conn: &Connection) -> Result<Vec<Client>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {CLIENT_COLUMNS} FROM gateway_clients WHERE deleted_at IS NULL \
            ORDER BY revoked_at IS NOT NULL, created_at"
        ))
        .map_err(err)?;
    let rows = stmt.query_map([], |row| client_row(conn, row)).map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

/// Where the defaults for new API keys are kept (Policies).
pub const CLIENT_DEFAULTS: &str = "client.defaults";

/// Defaults applied when an API key is created, whoever creates it: the app, the CLI or a tool's set-up (Policies,
/// SCREENS §2.2). `None` keeps the schema's own default. Existing keys keep what they have.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDefaults {
    pub auto_min_tier: Option<String>,
    pub monthly_budget_micro_usd: Option<u64>,
    pub max_concurrency: Option<u32>,
    pub unknown_cost_policy: Option<String>,
    pub paid_retries: Option<u32>,
    pub require_no_training: Option<bool>,
    /// Strict monthly limit: each request held at the most it could cost (ROADMAP B4). Only means something with a limit.
    #[serde(default)]
    pub strict_budget: Option<bool>,
}

pub fn client_defaults(conn: &Connection) -> Result<ClientDefaults> {
    Ok(setting(conn, CLIENT_DEFAULTS)?.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default())
}

pub fn set_client_defaults(conn: &Connection, d: &ClientDefaults) -> Result<()> {
    if let Some(tier) = &d.auto_min_tier {
        if !["T0", "T1", "T2", "T3", "T4"].contains(&tier.as_str()) {
            return Err(format!("unknown tier {tier}"));
        }
    }
    if d.max_concurrency.is_some_and(|n| !(1..=256).contains(&n)) {
        return Err("Requests at once must be 1 to 256".into());
    }
    if d.paid_retries.is_some_and(|n| n > 5) {
        return Err("Paid retries must be 0 to 5".into());
    }
    if d.unknown_cost_policy.as_deref().is_some_and(|p| p != "block" && p != "allow") {
        return Err("Unknown prices: block or allow".into());
    }
    if d.monthly_budget_micro_usd == Some(0) {
        return Err("Enter a monthly limit above $0, or none".into());
    }
    set_setting(conn, CLIENT_DEFAULTS, &serde_json::to_string(d).map_err(|e| e.to_string())?)
}

/// Creates a client and returns it with its token. The store keeps only the token's hash; the app may keep a copy in
/// the credential store (commands.rs). The defaults for new keys apply here, so every way of creating one gets them.
pub fn create_client(conn: &Connection, name: &str, monthly_budget_micro_usd: Option<u64>) -> Result<(Client, String)> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Enter a name for the client".into());
    }
    let token = format!("str-{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
    let id = uuid::Uuid::new_v4().to_string();
    let defaults = client_defaults(conn)?;
    let budget = monthly_budget_micro_usd.or(defaults.monthly_budget_micro_usd);
    conn.execute(
        "INSERT INTO gateway_clients (id, name, token_hash, token_prefix, monthly_budget_micro_usd, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, name, hash_token(&token), &token[..12], budget.map(|v| v as i64), now_ms()],
    )
    .map_err(err)?;
    if let Some(tier) = &defaults.auto_min_tier {
        set_client_auto_min_tier(conn, &id, tier)?;
    }
    if defaults.max_concurrency.is_some() || defaults.unknown_cost_policy.is_some() {
        set_client_policy(conn, &id, defaults.max_concurrency, defaults.unknown_cost_policy.as_deref())?;
    }
    if let Some(n) = defaults.paid_retries {
        set_client_paid_retries(conn, &id, n)?;
    }
    if let Some(required) = defaults.require_no_training {
        set_client_require_no_training(conn, &id, required)?;
    }
    if let Some(strict) = defaults.strict_budget {
        set_client_strict_budget(conn, &id, strict)?;
    }
    let client = conn
        .query_row(&format!("SELECT {CLIENT_COLUMNS} FROM gateway_clients WHERE id = ?1"), [&id], |r| {
            client_row(conn, r)
        })
        .map_err(err)?;
    Ok((client, token))
}

/// The credential-store entry holding a copy of the client's token, if one was kept.
pub fn client_token_ref(conn: &Connection, id: &str) -> Result<Option<String>> {
    conn.query_row("SELECT token_ref FROM gateway_clients WHERE id = ?1 AND revoked_at IS NULL", [id], |r| r.get(0))
        .optional()
        .map_err(err)
        .map(Option::flatten)
}

pub fn set_client_token_ref(conn: &Connection, id: &str, token_ref: Option<&str>) -> Result<()> {
    conn.execute("UPDATE gateway_clients SET token_ref = ?2 WHERE id = ?1", params![id, token_ref])
        .map(|_| ())
        .map_err(err)
}

/// Takes a revoked client off the list; returns its token copy's reference, for the caller to delete. A key still in
/// use is refused: revoke it first.
pub fn delete_client(conn: &Connection, id: &str) -> Result<Option<String>> {
    let row: Option<(Option<i64>, Option<String>)> = conn
        .query_row(
            "SELECT revoked_at, token_ref FROM gateway_clients WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(err)?;
    let Some((revoked, token_ref)) = row else { return Err("No such API key".into()) };
    if revoked.is_none() {
        return Err("Revoke the key before deleting it.".into());
    }
    conn.execute("UPDATE gateway_clients SET deleted_at = ?2, token_ref = NULL WHERE id = ?1", params![id, now_ms()])
        .map_err(err)?;
    Ok(token_ref)
}

/// The revoked clients still listed.
pub fn revoked_client_ids(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT id FROM gateway_clients WHERE revoked_at IS NOT NULL AND deleted_at IS NULL")
        .map_err(err)?;
    let ids = stmt.query_map([], |r| r.get(0)).map_err(err)?.collect::<rusqlite::Result<Vec<String>>>().map_err(err)?;
    Ok(ids)
}

pub fn revoke_client(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE gateway_clients SET revoked_at = ?2 WHERE id = ?1 AND revoked_at IS NULL",
        params![id, now_ms()],
    )
    .map(|_| ())
    .map_err(err)
}

pub fn set_client_budget(conn: &Connection, id: &str, monthly_budget_micro_usd: Option<u64>) -> Result<()> {
    conn.execute(
        "UPDATE gateway_clients SET monthly_budget_micro_usd = ?2 WHERE id = ?1",
        params![id, monthly_budget_micro_usd.map(|v| v as i64)],
    )
    .map(|_| ())
    .map_err(err)
}

pub fn set_client_policy(
    conn: &Connection, id: &str, max_concurrency: Option<u32>, unknown_cost_policy: Option<&str>,
) -> Result<()> {
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
        conn.execute("UPDATE gateway_clients SET unknown_cost_policy = ?2 WHERE id = ?1", params![id, policy])
            .map_err(err)?;
    }
    Ok(())
}

/// Restricts a client to some connections (`Some`, possibly empty) or lets it use all of them (`None`).
pub fn set_client_connections(conn: &Connection, id: &str, allowed: Option<&[String]>) -> Result<()> {
    let json = allowed.map(|list| serde_json::to_string(list).unwrap_or_else(|_| "[]".into()));
    conn.execute("UPDATE gateway_clients SET allowed_connections = ?2 WHERE id = ?1", params![id, json])
        .map(|_| ())
        .map_err(err)
}

pub fn set_client_auto_min_tier(conn: &Connection, id: &str, tier: &str) -> Result<()> {
    if !TIERS.contains(&tier) {
        return Err(format!("unknown tier {tier}"));
    }
    conn.execute("UPDATE gateway_clients SET auto_min_tier = ?2 WHERE id = ?1", params![id, tier])
        .map(|_| ())
        .map_err(err)
}

pub fn set_client_paid_retries(conn: &Connection, id: &str, retries: u32) -> Result<()> {
    if retries > 5 {
        return Err("Allow from 0 to 5 retries after a server error".into());
    }
    conn.execute("UPDATE gateway_clients SET paid_retries = ?2 WHERE id = ?1", params![id, retries])
        .map(|_| ())
        .map_err(err)
}

pub fn set_client_strict_budget(conn: &Connection, id: &str, strict: bool) -> Result<()> {
    conn.execute("UPDATE gateway_clients SET strict_budget = ?2 WHERE id = ?1", params![id, strict])
        .map(|_| ())
        .map_err(err)
}

/// The model's maximum output, when it is known.
pub fn max_output_tokens(conn: &Connection, provider_id: &str, model_id: &str) -> Result<Option<u64>> {
    conn.query_row(
        "SELECT max_output_tokens FROM models WHERE provider_id = ?1 AND model_id = ?2",
        params![provider_id, model_id],
        |r| r.get::<_, Option<i64>>(0),
    )
    .optional()
    .map(|v| v.flatten().filter(|m| *m > 0).map(|m| m as u64))
    .map_err(err)
}

/// The thinking types and effort levels the model takes, when they are known (`None` is unknown).
pub fn thinking_support(
    conn: &Connection, provider_id: &str, model_id: &str,
) -> Result<(Option<Vec<String>>, Option<Vec<String>>)> {
    let parse = |v: Option<String>| v.and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok());
    conn.query_row(
        "SELECT thinking_types, effort_levels FROM models WHERE provider_id = ?1 AND model_id = ?2",
        params![provider_id, model_id],
        |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?)),
    )
    .optional()
    .map(|row| row.map(|(t, e)| (parse(t), parse(e))).unwrap_or((None, None)))
    .map_err(err)
}

pub fn set_client_require_no_training(conn: &Connection, id: &str, required: bool) -> Result<()> {
    conn.execute("UPDATE gateway_clients SET require_no_training = ?2 WHERE id = ?1", params![id, required])
        .map(|_| ())
        .map_err(err)
}

/// Records what a provider's terms say about training on what is sent, and where the user read it. `no_training`
/// needs a source: it is what lets a client limited to such providers send there.
pub fn set_provider_terms(conn: &Connection, id: &str, terms: &str, source: Option<&str>) -> Result<()> {
    if !matches!(terms, "no_training" | "may_train" | "unknown") {
        return Err(format!("unknown terms {terms}"));
    }
    let source = source.map(str::trim).filter(|s| !s.is_empty());
    if terms == "no_training" && source.is_none() {
        return Err(
            "Say where the terms say so (a link to the provider's policy, or the page and date you read)".into()
        );
    }
    let changed = conn.execute(
        "UPDATE providers SET data_use_terms = ?2, terms_source = ?3, terms_checked_at = CASE WHEN ?2 = 'unknown' THEN NULL ELSE ?4 END WHERE id = ?1",
        params![id, terms, source, now_ms()]).map_err(err)?;
    if changed == 0 {
        return Err("That connection no longer exists".into());
    }
    Ok(())
}

pub fn set_client_may_run_tasks(conn: &Connection, id: &str, allowed: bool) -> Result<()> {
    conn.execute("UPDATE gateway_clients SET may_run_tasks = ?2 WHERE id = ?1", params![id, allowed])
        .map(|_| ())
        .map_err(err)
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
    /// The ownership generation of the gateway sending it (review LC-01).
    pub owner_generation: &'a str,
}

/// Journals an attempt and holds its spend. Atomic across threads and processes: the limit check and the
/// insert happen under one `BEGIN IMMEDIATE`, so two requests cannot both take the last dollar. Returns
/// the attempt id, or why the hold does not fit. A hold of 0 is never refused here.
pub fn open_attempt(conn: &Connection, a: &AttemptStart) -> Result<std::result::Result<String, OverBudget>> {
    immediate(conn, || {
        if a.hold_micro > 0 {
            let budget: Option<i64> = conn.query_row(
                "SELECT monthly_budget_micro_usd FROM gateway_clients WHERE id = ?1",
                [a.client_id],
                |r| r.get(0),
            )?;
            if let Some(budget) = budget.map(|b| b.max(0) as u64) {
                let spent: i64 = conn.query_row(
                    &format!("SELECT {COUNTED_SPEND} FROM usage_events WHERE client_id = ?1 AND created_at >= ?2"),
                    params![a.client_id, month_start(now_ms())],
                    |r| r.get(0),
                )?;
                let reserved: i64 = conn.query_row(
                    "SELECT COALESCE(SUM(reserved_micro), 0) FROM budget_reservations WHERE client_id = ?1",
                    [a.client_id],
                    |r| r.get(0),
                )?;
                let (spent, reserved) = (spent.max(0) as u64, reserved.max(0) as u64);
                if spent + reserved + a.hold_micro > budget || spent >= budget {
                    return Ok((Err(OverBudget { spent, reserved, budget, wanted: a.hold_micro }), false));
                }
            }
        }
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO budget_reservations (id, client_id, reserved_micro, created_at, provider_id, model_id, \
             requested_model, session_key, route_mode, owner_generation) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![id, a.client_id, a.hold_micro as i64, now_ms(), a.provider_id, a.model_id, a.requested_model,
                a.session_key, a.route_mode, a.owner_generation],
        )?;
        Ok((Ok(id), true))
    })
}

/// Settles an attempt in one transaction: the ledger entry is written and the journal entry removed, or
/// neither (counter-review CR-06). An unknown cost is counted against the limit at the amount that was held.
/// If the journal entry is already gone (settled twice), nothing is written.
pub fn settle(conn: &Connection, attempt_id: &str, record: &UsageRecord) -> Result<()> {
    immediate(conn, || {
        let held: Option<i64> = conn
            .query_row("SELECT reserved_micro FROM budget_reservations WHERE id = ?1", [attempt_id], |r| r.get(0))
            .optional()?;
        let Some(held) = held else { return Ok(((), false)) };
        let at_risk = (record.cost.micro_usd().is_none() && held > 0).then_some(held as u64);
        insert_usage(conn, record, at_risk)?;
        conn.execute("DELETE FROM budget_reservations WHERE id = ?1", [attempt_id])?;
        Ok(((), true))
    })
}

const INTERRUPTED_SOURCE: &str = "the gateway stopped before this attempt was settled";

/// Which journal entries `settle_interrupted` turns into ledger entries.
#[derive(Clone, Copy, Debug)]
pub enum Unsettled<'a> {
    /// One attempt, dropped without being settled.
    Attempt(&'a str),
    /// Every entry not journaled by this ownership generation (review LC-01): called by a gateway that has just
    /// taken the store, so every other generation has finished or died.
    NotOf(&'a str),
}

/// Turns journal entries into ledger entries with an unknown cost: the attempt was sent, or about to be,
/// and never settled. Its held amount stays counted as spend at risk. Returns how many were turned.
pub fn settle_interrupted(conn: &Connection, which: Unsettled) -> Result<usize> {
    let (only, generation) = match which {
        Unsettled::Attempt(id) => (id, ""),
        Unsettled::NotOf(generation) => ("", generation),
    };
    immediate(conn, || {
        let condition = "(?1 <> '' AND id = ?1) OR (?1 = '' AND owner_generation IS NOT ?2)";
        let turned = conn.execute(&format!(
            "INSERT INTO usage_events (id, client_id, provider, model, cost_micro_usd, cost_status, source, created_at, \
             outcome, requested_model, error, session_key, route_mode, at_risk_micro_usd) \
             SELECT lower(hex(randomblob(16))), client_id, COALESCE(provider_id, 'unknown'), COALESCE(model_id, 'unknown'), \
             NULL, 'unknown', '{INTERRUPTED_SOURCE}', created_at, 'interrupted', requested_model, \
             'Interrupted before the outcome was recorded', session_key, route_mode, NULLIF(reserved_micro, 0) \
             FROM budget_reservations WHERE {condition}"), params![only, generation])?;
        conn.execute(&format!("DELETE FROM budget_reservations WHERE {condition}"), params![only, generation])?;
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

/// States whose cooldown grows when they repeat (a circuit breaker, Account Rotation §5): a connection that keeps
/// failing the same way waits twice as long each time, up to `BACKOFF_CAP_MS`. A success clears the count.
pub const BACKOFF_STATES: [&str; 2] = ["unreachable", "server_error"];
pub const BACKOFF_CAP_MS: i64 = 15 * 60 * 1000;

pub fn mark_health(
    conn: &Connection, provider_id: &str, model_id: &str, state: &str, reason: &str, http_status: Option<u16>,
    until_ms: Option<i64>,
) -> Result<()> {
    let now = now_ms();
    let previous: Option<(String, i64, Option<i64>)> = conn
        .query_row(
            "SELECT state, strikes, until_ms FROM connection_health WHERE provider_id = ?1 AND model_id = ?2",
            params![provider_id, model_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(err)?;
    let strikes = match &previous {
        Some((s, n, _)) if s == state => n + 1,
        _ => 1,
    };
    let (until_ms, reason) = match until_ms {
        Some(until) if BACKOFF_STATES.contains(&state) && strikes > 1 => {
            let base = (until - now).max(1_000);
            let wait = base.saturating_mul(1i64 << (strikes - 1).min(10)).min(BACKOFF_CAP_MS.max(base));
            (Some(now + wait), format!("{reason} (failure {strikes} in a row, waiting {} s)", wait / 1000))
        }
        other => (other, reason.to_string()),
    };
    conn.execute(
        "INSERT INTO connection_health (provider_id, model_id, state, reason, http_status, until_ms, updated_at, strikes) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) ON CONFLICT(provider_id, model_id) DO UPDATE SET state = ?3, reason = ?4, \
         http_status = ?5, until_ms = ?6, updated_at = ?7, strikes = ?8",
        params![provider_id, model_id, state, reason.chars().take(300).collect::<String>(), http_status, until_ms, now, strikes],
    )
    .map(|_| ())
    .map_err(err)
}

/// Connection-wide `unreachable` entries still in force and not checked for `gap_ms`: what the probe looks at.
pub fn unreachable_to_probe(conn: &Connection, gap_ms: i64) -> Result<Vec<String>> {
    let now = now_ms();
    let mut statement = conn.prepare(
        "SELECT h.provider_id FROM connection_health h JOIN providers p ON p.id = h.provider_id WHERE h.model_id = '' \
         AND h.state = 'unreachable' AND p.enabled = 1 AND h.until_ms > ?1 AND h.updated_at <= ?2").map_err(err)?;
    let rows = statement.query_map(params![now, now - gap_ms], |r| r.get(0)).map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

/// A check of a connection's state: its model list read, or a web session's sign-in (migration 0029).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub at: i64,
    pub ok: bool,
    /// `models` (an API connection's model list) or `sign_in` (a web session).
    pub kind: String,
    /// Pygg's own wording: "Reachable · 14 models listed", or the error.
    pub detail: String,
}

/// Keeps a check's result; checks older than seven days go.
pub fn record_check(conn: &Connection, provider_id: &str, ok: bool, kind: &str, detail: &str) -> Result<()> {
    let now = now_ms();
    conn.execute(
        "INSERT INTO connection_checks (provider_id, at, ok, kind, detail) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![provider_id, now, ok as i64, kind, detail],
    )
    .map_err(err)?;
    conn.execute("DELETE FROM connection_checks WHERE at < ?1", [now - 7 * 86_400_000]).map(|_| ()).map_err(err)
}

/// A connection's last check, if any.
pub fn last_check(conn: &Connection, provider_id: &str) -> Result<Option<Check>> {
    conn.query_row(
        "SELECT at, ok, kind, detail FROM connection_checks WHERE provider_id = ?1 ORDER BY at DESC LIMIT 1",
        [provider_id],
        |r| Ok(Check { at: r.get(0)?, ok: r.get::<_, i64>(1)? == 1, kind: r.get(2)?, detail: r.get(3)? }),
    )
    .optional()
    .map_err(err)
}

/// Enabled connections whose last check is older than `every_ms` (or that were never checked), oldest first.
pub fn checks_due(conn: &Connection, every_ms: i64, now: i64) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare(
            "SELECT p.id, (SELECT MAX(at) FROM connection_checks c WHERE c.provider_id = p.id) AS last \
        FROM providers p WHERE p.enabled = 1 AND (last IS NULL OR last < ?1) ORDER BY COALESCE(last, 0), p.id",
        )
        .map_err(err)?;
    let rows = stmt.query_map([now - every_ms], |r| r.get(0)).map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

/// One hour of a connection's health strip: answers and checks that went well, and those that did not.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthBucket {
    pub start: i64,
    pub ok: u32,
    pub failed: u32,
}

/// A connection's last 24 hours, an hour a bucket, oldest first, and a score from 0 to 100: the share that went well,
/// recent hours counting more (the last hour four times the oldest). An hour's requests are its evidence; an hour with
/// none is judged by its checks. `None` when nothing happened.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthStrip {
    pub buckets: Vec<HealthBucket>,
    pub score: Option<u8>,
}

pub const STRIP_HOURS: i64 = 24;

/// A request counts against the connection when the provider failed it: an error, a fallback to another model or a
/// stream cut off, except a request Pygg itself refused before sending ("Not sent", `not sent`) and one the provider
/// refused as malformed (400, 413, 422), which says nothing about the provider. A client that left, or a restart, is
/// not counted either way.
const PROVIDER_FAILED: &str =
    "(u.outcome IN ('error', 'fallback', 'stream_error') AND COALESCE(u.http_status, 0) NOT IN (400, 413, 422) \
    AND COALESCE(u.error, '') NOT LIKE '%Not sent%' AND COALESCE(u.source, '') <> 'not sent')";

pub fn health_strip(conn: &Connection, provider_id: &str, now: i64) -> Result<HealthStrip> {
    const HOUR: i64 = 3_600_000;
    let since = now - STRIP_HOURS * HOUR;
    let mut counts = vec![(0u32, 0u32); STRIP_HOURS as usize];
    let mut checks = vec![(0u32, 0u32); STRIP_HOURS as usize];
    let add = |sql: &str, counts: &mut Vec<(u32, u32)>| -> Result<()> {
        let mut stmt = conn.prepare(sql).map_err(err)?;
        let rows = stmt
            .query_map(params![provider_id, since, now, HOUR], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
            })
            .map_err(err)?;
        for row in rows {
            let (ago, ok, failed) = row.map_err(err)?;
            if (0..STRIP_HOURS).contains(&ago) {
                let slot = &mut counts[(STRIP_HOURS - 1 - ago) as usize];
                slot.0 += ok.max(0) as u32;
                slot.1 += failed.max(0) as u32;
            }
        }
        Ok(())
    };
    // Nothing from the future counts (a clock set back), nor older than the strip (Codex's fourth review, A15).
    add(&format!("SELECT (?3 - u.created_at) / ?4 AS ago, SUM(u.outcome = 'ok'), SUM({PROVIDER_FAILED}) FROM usage_events u \
        WHERE u.provider = ?1 AND u.created_at > ?2 AND u.created_at <= ?3 GROUP BY ago"), &mut counts)?;
    add("SELECT (?3 - at) / ?4 AS ago, SUM(ok = 1), SUM(ok = 0) FROM connection_checks WHERE provider_id = ?1 AND at > ?2 \
        AND at <= ?3 GROUP BY ago", &mut checks)?;
    // A model list read says less than an answer (Codex's fourth review, A10): an hour with requests is judged by them
    // alone; an hour without is judged by its checks.
    for (slot, check) in counts.iter_mut().zip(checks) {
        if slot.0 + slot.1 == 0 {
            *slot = check;
        }
    }
    let buckets: Vec<HealthBucket> = counts
        .iter()
        .enumerate()
        .map(|(i, (ok, failed))| HealthBucket {
            start: now - (STRIP_HOURS - i as i64) * HOUR,
            ok: *ok,
            failed: *failed,
        })
        .collect();
    // Each hour with anything in it gives its own share that went well, so a busy hour cannot outweigh a quiet hour's
    // outage, and pressing Check Now cannot raise the score. The newest hour weighs 1, the oldest 0.25, in a straight line.
    let (mut good, mut all) = (0.0f64, 0.0f64);
    for (i, b) in buckets.iter().enumerate().filter(|(_, b)| b.ok + b.failed > 0) {
        let weight = 0.25 + 0.75 * i as f64 / (STRIP_HOURS - 1) as f64;
        good += weight * f64::from(b.ok) / f64::from(b.ok + b.failed);
        all += weight;
    }
    let score = (all > 0.0).then(|| (100.0 * good / all).round().clamp(0.0, 100.0) as u8);
    Ok(HealthStrip { buckets, score })
}

/// The probe reached the provider: its connection-wide outage is over.
pub fn probe_succeeded(conn: &Connection, provider_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM connection_health WHERE provider_id = ?1 AND model_id = '' AND state = 'unreachable'",
        [provider_id],
    )
    .map(|_| ())
    .map_err(err)
}

/// The probe did not reach it: remember when it was checked, so it is not checked again for a while.
pub fn probe_failed(conn: &Connection, provider_id: &str) -> Result<()> {
    conn.execute("UPDATE connection_health SET updated_at = ?2 WHERE provider_id = ?1 AND model_id = '' AND state = 'unreachable'",
        params![provider_id, now_ms()]).map(|_| ()).map_err(err)
}

/// Clears the model's entry, and the connection-wide one unless it needs the user (a rejected key). An answer from a
/// model that is free without credit says nothing about the account's balance, so it leaves `payment_required`.
pub fn mark_healthy(conn: &Connection, provider_id: &str, model_id: &str) -> Result<()> {
    let free = free_without_credit(conn, provider_id, model_id)?;
    conn.execute(
        "DELETE FROM connection_health WHERE provider_id = ?1 AND (model_id = ?2 OR (model_id = '' AND until_ms IS NOT NULL          AND NOT (?3 AND state = 'payment_required')))",
        params![provider_id, model_id, free],
    )
    .map(|_| ())
    .map_err(err)
}

/// The rate-limit headers a provider last sent for a model (PV-06, ADR-030), as sent.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimits {
    pub model_id: String,
    pub headers: std::collections::BTreeMap<String, String>,
    pub http_status: Option<u16>,
    pub observed_at: i64,
    /// The headers read as limits, what is left and when they reset (quota.rs).
    #[serde(default)]
    pub windows: Vec<super::quota::Window>,
}

/// Whether a header is one a provider uses to say how much is left, or when to come back.
pub fn is_limit_header(name: &str) -> bool {
    name.starts_with("anthropic-ratelimit-") || name.starts_with("x-ratelimit-") || name == "retry-after"
}

/// Keeps the provider's rate-limit headers from one answer; does nothing when it sent none.
pub fn record_limits(
    conn: &Connection, provider_id: &str, model_id: &str, status: u16, headers: &[(String, String)],
) -> Result<()> {
    let kept: std::collections::BTreeMap<&str, &str> =
        headers.iter().filter(|(n, _)| is_limit_header(n)).map(|(n, v)| (n.as_str(), v.as_str())).collect();
    if kept.is_empty() {
        return Ok(());
    }
    let json = serde_json::to_string(&kept).map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO rate_limits (provider_id, model_id, headers_json, http_status, observed_at) \
                  VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT (provider_id, model_id) DO UPDATE SET \
                  headers_json = excluded.headers_json, http_status = excluded.http_status, observed_at = excluded.observed_at",
        params![provider_id, model_id, json, status as i64, now_ms()]).map(|_| ()).map_err(err)
}

pub fn list_limits(conn: &Connection, provider_id: &str) -> Result<Vec<RateLimits>> {
    let mut stmt = conn
        .prepare(
            "SELECT model_id, headers_json, http_status, observed_at FROM rate_limits \
        WHERE provider_id = ?1 ORDER BY observed_at DESC",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map([provider_id], |r| {
            Ok(RateLimits {
                model_id: r.get(0)?,
                headers: serde_json::from_str(&r.get::<_, String>(1)?).unwrap_or_default(),
                http_status: r.get::<_, Option<i64>>(2)?.map(|v| v as u16),
                observed_at: r.get(3)?,
                windows: Vec::new(),
            })
        })
        .map_err(err)?;
    let mut out: Vec<RateLimits> = rows.collect::<rusqlite::Result<_>>().map_err(err)?;
    for limits in &mut out {
        limits.windows = super::quota::from_headers(&limits.headers, limits.observed_at);
    }
    Ok(out)
}

/// Clears the connection's health entries, and the holds `auto` put on its models after 429 answers.
pub fn clear_health(conn: &Connection, provider_id: &str) -> Result<()> {
    conn.execute("DELETE FROM connection_health WHERE provider_id = ?1", [provider_id]).map(|_| ()).map_err(err)?;
    super::keys::clear_health(conn, provider_id)?;
    super::auto::clear_holds(conn, provider_id)
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

/// Whether a model is free on its connection without any credit: listed at $0, and billed as free, on a connection
/// set as free whose free part is its $0 models (ADR-032). OpenRouter serves its `:free` models on an account with
/// no balance, so the account being out of credit (HTTP 402) says nothing about them.
pub fn free_without_credit(conn: &Connection, provider_id: &str, model_id: &str) -> Result<bool> {
    const SQL: &str = concat!("SELECT EXISTS (SELECT 1 FROM models m JOIN providers p ON p.id = m.provider_id          WHERE m.provider_id = ?1 AND m.model_id = ?2 AND ", free_models_connection!(), " AND ", listed_free!(),
        " AND (", billing!(), ") = 'free')");
    conn.query_row(SQL, params![provider_id, model_id], |r| r.get(0)).map_err(err)
}

/// Whether a health entry covers this model: its own entry, or a connection-wide one. A connection-wide
/// `payment_required` does not cover a model that is free without credit (`free_without_credit`).
fn covers(h: &Health, model_id: &str, free_without_credit: bool) -> bool {
    h.model_id == model_id || (h.model_id.is_empty() && !(free_without_credit && h.state == "payment_required"))
}

/// Looks up `free_without_credit` only when an entry in `entries` could depend on it.
fn free_if_it_matters(conn: &Connection, entries: &[Health], provider_id: &str, model_id: &str) -> Result<bool> {
    if entries.iter().any(|h| h.model_id.is_empty() && h.state == "payment_required") {
        free_without_credit(conn, provider_id, model_id)
    } else {
        Ok(false)
    }
}

/// The entry that currently keeps this connection or model from being tried, if any.
pub fn blocking_health(conn: &Connection, provider_id: &str, model_id: &str) -> Result<Option<Health>> {
    let entries = list_health(conn, Some(provider_id))?;
    let free = free_if_it_matters(conn, &entries, provider_id, model_id)?;
    Ok(entries.into_iter().find(|h| covers(h, model_id, free)))
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
        let entries = list_health(conn, Some(&other.id))?;
        // The account being out of credit does not hold a model that is free here without credit.
        let free = free_if_it_matters(conn, &entries, &provider.id, model_id)?;
        let hold = entries.into_iter().find(|h| QUOTA_STATES.contains(&h.state.as_str()) && covers(h, model_id, free));
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

/// How long a provider's prompt cache is taken to stay warm after the request that last used it. The common
/// default is five minutes (Anthropic's ephemeral cache, OpenAI's automatic one); some last longer, none shorter.
pub const CACHE_WARM_MS: i64 = 5 * 60 * 1000;

/// Where a session is pinned, and its home: the target the session was meant for before any fallback.
#[derive(Clone, Debug, PartialEq)]
pub struct Pin {
    pub target: Target,
    pub last_used_at: i64,
    /// `None` for a pin made before migration 0033, until its next request.
    pub home: Option<Target>,
    /// When the home last answered this session; `None` if it never has.
    pub home_used_at: Option<i64>,
}

impl Pin {
    /// A session away from home goes back once the home's cache is still warm, or once the fallback's has gone cold
    /// too (both cold: nothing is lost by going back). While the fallback's cache is warm and the home's is not, it
    /// stays. Checked only when a request starts, never between the attempts of one.
    pub fn goes_home(&self, now: i64) -> bool {
        let Some(home) = &self.home else { return false };
        if *home == self.target {
            return false;
        }
        let home_warm = self.home_used_at.is_some_and(|at| now - at < CACHE_WARM_MS);
        let away_cold = now - self.last_used_at >= CACHE_WARM_MS;
        home_warm || away_cold
    }
}

pub fn session_pin(conn: &Connection, client_id: &str, session_key: &str, requested: &str) -> Result<Option<Pin>> {
    conn.query_row(
        "SELECT provider_id, model_id, last_used_at, home_provider_id, home_model_id, home_used_at          FROM session_affinity WHERE client_id = ?1 AND session_key = ?2 AND requested_model = ?3 AND last_used_at >= ?4",
        params![client_id, session_key, requested, now_ms() - AFFINITY_TTL_MS],
        |r| {
            let home = match (r.get::<_, Option<String>>(3)?, r.get::<_, Option<String>>(4)?) {
                (Some(provider_id), Some(model_id)) => Some(Target { provider_id, model_id }),
                _ => None,
            };
            Ok(Pin {
                target: Target { provider_id: r.get(0)?, model_id: r.get(1)? },
                last_used_at: r.get(2)?,
                home,
                home_used_at: r.get(5)?,
            })
        },
    )
    .optional()
    .map_err(err)
}

/// Deletes expired pins, of one client or of all. Clients may name as many sessions as they like; only the
/// last two hours of them are kept.
pub fn purge_sessions(conn: &Connection, client_id: Option<&str>) -> Result<usize> {
    conn.execute(
        "DELETE FROM session_affinity WHERE (?1 IS NULL OR client_id = ?1) AND last_used_at < ?2",
        params![client_id, now_ms() - AFFINITY_TTL_MS],
    )
    .map_err(err)
}

/// Pins a session to the target that answered it. `home` is the target the session was meant for (the one it is
/// pinned to when `None`); the home's last answer is kept while the home stays the same.
pub fn pin(
    conn: &Connection, client_id: &str, session_key: &str, requested: &str, target: &Target, home: Option<&Target>,
) -> Result<()> {
    let now = now_ms();
    let home = home.unwrap_or(target);
    purge_sessions(conn, Some(client_id))?;
    conn.execute(
        "INSERT INTO session_affinity (client_id, session_key, requested_model, provider_id, model_id, created_at,          last_used_at, home_provider_id, home_model_id, home_used_at)          VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7, ?8, CASE WHEN ?4 = ?7 AND ?5 = ?8 THEN ?6 END)          ON CONFLICT(client_id, session_key, requested_model) DO UPDATE SET          provider_id = ?4, model_id = ?5, last_used_at = ?6,          created_at = CASE WHEN provider_id = ?4 AND model_id = ?5 THEN created_at ELSE ?6 END,          home_used_at = CASE WHEN ?4 = ?7 AND ?5 = ?8 THEN ?6            WHEN home_provider_id IS ?7 AND home_model_id IS ?8 THEN home_used_at END,          home_provider_id = ?7, home_model_id = ?8",
        params![client_id, session_key, requested, target.provider_id, target.model_id, now, home.provider_id, home.model_id],
    )
    .map(|_| ())
    .map_err(err)
}

pub fn active_sessions(conn: &Connection, client_id: &str) -> Result<usize> {
    conn.query_row(
        "SELECT COUNT(*) FROM session_affinity WHERE client_id = ?1 AND last_used_at >= ?2",
        params![client_id, now_ms() - AFFINITY_TTL_MS],
        |r| r.get::<_, i64>(0),
    )
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
    /// `order` (as listed) or `cheapest` (by each request's estimated cost; migration 0025).
    pub strategy: String,
}

pub const COMBO_STRATEGIES: [&str; 2] = ["order", "cheapest"];

pub fn set_combo_strategy(conn: &Connection, id: &str, strategy: &str) -> Result<()> {
    if !COMBO_STRATEGIES.contains(&strategy) {
        return Err(format!("unknown combo strategy {strategy}"));
    }
    conn.execute("UPDATE combos SET strategy = ?2 WHERE id = ?1", params![id, strategy]).map(|_| ()).map_err(err)
}

/// The strategy of the combo with this name, if it is one.
pub fn combo_strategy(conn: &Connection, name: &str) -> Result<Option<String>> {
    conn.query_row("SELECT strategy FROM combos WHERE name = ?1", [name], |r| r.get(0)).optional().map_err(err)
}

pub fn list_combos(conn: &Connection) -> Result<Vec<Combo>> {
    let mut stmt = conn.prepare("SELECT id, name, strategy FROM combos ORDER BY name").map_err(err)?;
    let heads: Vec<(String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;
    let mut combos = Vec::new();
    for (id, name, strategy) in heads {
        let mut members = conn
            .prepare("SELECT provider_id, model_id FROM combo_members WHERE combo_id = ?1 ORDER BY position")
            .map_err(err)?;
        let members = members
            .query_map([&id], |r| Ok(Target { provider_id: r.get(0)?, model_id: r.get(1)? }))
            .map_err(err)?
            .collect::<rusqlite::Result<_>>()
            .map_err(err)?;
        combos.push(Combo { id, name, members, strategy });
    }
    Ok(combos)
}

/// Creates or replaces a combo. Names cannot contain `/`, so they never collide with `<connection>/<model>`.
pub fn save_combo(conn: &Connection, id: Option<&str>, name: &str, members: &[Target]) -> Result<Combo> {
    let name = name.trim();
    if name.is_empty() || name.contains('/') || name.contains(char::is_whitespace) {
        return Err("A combo name needs at least one character and no spaces or slashes".into());
    }
    if name == AUTO {
        return Err("\"auto\" is the name of automatic routing; choose another name for the combo".into());
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
    if e.to_string().contains("UNIQUE") {
        format!("A combo named {name} already exists")
    } else {
        e.to_string()
    }
}

pub fn delete_combo(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM combos WHERE id = ?1", [id]).map(|_| ()).map_err(err)
}

/// The connections of a pool, in the order they are tried.
pub fn pool_members(conn: &Connection, pool: &str) -> Result<Vec<Provider>> {
    let mut members: Vec<Provider> =
        list_providers(conn)?.into_iter().filter(|p| p.pool.as_deref() == Some(pool)).collect();
    members.sort_by_key(|p| (p.pool_rank.unwrap_or(i64::MAX), p.created_at));
    Ok(members)
}

/// The pools there are, by name.
pub fn pool_names(conn: &Connection) -> Result<Vec<String>> {
    let mut names: Vec<String> = list_providers(conn)?.into_iter().filter_map(|p| p.pool).collect();
    names.sort();
    names.dedup();
    Ok(names)
}

/// Puts a connection in a pool (ADR-034), at its end, or takes it out (`None`). A pool holds connections of one
/// provider kind, each with a quota the user confirmed as its own (Connections → Independent quota confirmed): a pool
/// is for separate accounts the user holds, never several keys of one account to get around its limits. Its name is
/// a routing prefix, so it may not be a connection's id, a combo's name or `auto`.
pub fn set_pool(conn: &Connection, provider_id: &str, pool: Option<&str>) -> Result<()> {
    let provider = get_provider(conn, provider_id)?.ok_or("Connection not found")?;
    let Some(name) = pool.map(str::trim).filter(|n| !n.is_empty()) else {
        conn.execute("UPDATE providers SET pool = NULL, pool_rank = NULL WHERE id = ?1", [provider_id]).map_err(err)?;
        return Ok(());
    };
    if !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') || name.len() > 40 {
        return Err("A pool's name is lowercase letters, digits and hyphens, at most 40 characters".into());
    }
    if name == AUTO || get_provider(conn, name)?.is_some() || list_combos(conn)?.iter().any(|c| c.name == name) {
        return Err(format!("{name} is already a connection, a combo or auto; pick another name for the pool"));
    }
    // Web sessions may pool with others of the same site (owner, 2026-11-13; ADR-035): each account keeps its own daily
    // limit and pace, and the pool sends to the one used least today. Confirming its own quota says it is another account.
    if !provider.separate_quota {
        return Err(
            "Confirm first that this connection has its own provider quota (Independent quota confirmed): a pool is \
            for separate accounts, not several keys of one account"
                .into(),
        );
    }
    let members = pool_members(conn, name)?;
    if let Some(other) = members.iter().find(|m| m.kind != provider.kind && m.id != provider.id) {
        return Err(format!("The pool {name} holds {} connections; this one is {}", other.kind, provider.kind));
    }
    if provider.pool.as_deref() == Some(name) {
        return Ok(());
    }
    let rank = members.iter().filter_map(|m| m.pool_rank).max().unwrap_or(0) + 1;
    conn.execute("UPDATE providers SET pool = ?2, pool_rank = ?3 WHERE id = ?1", params![provider_id, name, rank])
        .map_err(err)?;
    Ok(())
}

/// Moves a connection one place earlier (`up`) or later in its pool's order.
pub fn move_in_pool(conn: &Connection, provider_id: &str, up: bool) -> Result<()> {
    let provider = get_provider(conn, provider_id)?.ok_or("Connection not found")?;
    let Some(pool) = provider.pool.clone() else { return Err("This connection is in no pool".into()) };
    let mut members = pool_members(conn, &pool)?;
    let Some(at) = members.iter().position(|m| m.id == provider_id) else { return Ok(()) };
    let to = if up { at.checked_sub(1) } else { (at + 1 < members.len()).then_some(at + 1) };
    if let Some(to) = to {
        members.swap(at, to);
    }
    for (rank, member) in members.iter().enumerate() {
        conn.execute("UPDATE providers SET pool_rank = ?2 WHERE id = ?1", params![member.id, rank as i64 + 1])
            .map_err(err)?;
    }
    Ok(())
}

/// What a requested model name routes to: a combo's members in order, a pool's connections in order for
/// `<pool>/<model>` (ADR-034), or one `<connection>/<model>`. Disabled connections are skipped.
pub fn resolve(conn: &Connection, requested: &str) -> Result<Vec<(Provider, String)>> {
    let combo = list_combos(conn)?.into_iter().find(|c| c.name == requested);
    let targets = match combo {
        Some(combo) => combo.members,
        None => match requested.split_once('/') {
            Some((prefix, model_id)) if !model_id.is_empty() && get_provider(conn, prefix)?.is_none() => {
                let mut members = pool_members(conn, prefix)?;
                // A pool of web sessions spreads its requests: an account the web guard is resting goes last, then the
                // account that sent fewest today, then the pool's order. Each still stops at its own limits, and the
                // next one answers.
                if members.iter().all(|p| super::presets::is_web(&p.kind)) {
                    let now = now_ms();
                    let mut used: Vec<(bool, u64, Provider)> = members
                        .into_iter()
                        .map(|p| (web_resting(conn, &p, now), web_requests_today(conn, &p.id, now).unwrap_or(0), p))
                        .collect();
                    used.sort_by_key(|(resting, n, p)| (*resting, *n, p.pool_rank.unwrap_or(i64::MAX)));
                    members = used.into_iter().map(|(_, _, p)| p).collect();
                }
                members.into_iter().map(|p| Target { provider_id: p.id, model_id: model_id.into() }).collect()
            }
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
    /// Why auto routing chose this target (ROADMAP P0-5).
    pub route_reason: Option<&'a str>,
    /// Tool output this request repeated from earlier in itself, in about-tokens (P1-4, shadow: measured, never removed).
    pub repeated_tokens: Option<u64>,
    /// What the attempt was when it was sent (migration 0030); `None` where nothing sent it (a recovered journal entry,
    /// a task's own record).
    pub facts: Option<&'a AttemptFacts>,
}

/// An attempt's facts as they were when it was sent (migration 0030): history does not follow later settings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AttemptFacts {
    /// The client's request, shared by all its attempts.
    pub request_id: String,
    /// This attempt's place among the request's attempts that reached the journal, from 1.
    pub attempt: u32,
    /// The billing type it was sent under: free, local, subscription, paid or unknown.
    pub billing: String,
    /// How the route was chosen: `auto`, `auto/cheap`, `combo`, `combo:cheapest`, `pool` or `named`, with `+pinned` when
    /// a session pin put this target first.
    pub route_kind: String,
    /// The client's prompt-cache hints: `preserved`, `translated`, `dropped`, or `none` when it sent none.
    pub cache_hint: String,
    /// The key of the connection's key ring that sent it (ADR-038); `None` for a connection with one key.
    pub key_id: Option<String>,
    /// The request's tool definitions and system prompt, measured (migration 0036); `None` when it carried neither.
    pub prefix: Option<super::shadow::Prefix>,
    /// What clearing old tool results would take out of it (migration 0037, shadow); `None` without tool output or for
    /// an Anthropic-format request.
    pub clearing: Option<super::shadow::Clearing>,
    /// Claude Code's hint headers (migration 0039); `None` when the request carried none.
    pub hint: Option<super::hints::RequestHint>,
}

impl<'a> UsageRecord<'a> {
    /// An attempt that produced no usage: a rejection, a refused connection, a key that could not be read.
    pub fn failed(
        client_id: &'a str, provider_id: &'a str, model_id: &'a str, requested_model: &'a str, cost: &'a Cost,
        outcome: &'a str, http_status: Option<u16>, latency_ms: u64, error: &'a str,
    ) -> UsageRecord<'a> {
        UsageRecord {
            client_id: Some(client_id),
            provider_id,
            model_id,
            requested_model,
            input_tokens: None,
            output_tokens: None,
            cache_tokens: None,
            cost,
            outcome,
            http_status,
            latency_ms,
            error: Some(error),
            cache_write_tokens: None,
            session_key: None,
            route_mode: None,
            route_reason: None,
            facts: None,
            repeated_tokens: None,
        }
    }
}

/// A `UsageRecord` that owns its fields, kept in memory while its settlement waits for the store (review FR-03).
#[derive(Clone, Debug)]
pub struct PendingUsage {
    client_id: Option<String>,
    provider_id: String,
    model_id: String,
    requested_model: String,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_tokens: Option<u64>,
    cost: Cost,
    outcome: String,
    http_status: Option<u16>,
    latency_ms: u64,
    error: Option<String>,
    cache_write_tokens: Option<u64>,
    session_key: Option<String>,
    route_mode: Option<String>,
    route_reason: Option<String>,
}

impl PendingUsage {
    pub fn of(r: &UsageRecord) -> PendingUsage {
        PendingUsage {
            client_id: r.client_id.map(String::from),
            provider_id: r.provider_id.into(),
            model_id: r.model_id.into(),
            requested_model: r.requested_model.into(),
            input_tokens: r.input_tokens,
            output_tokens: r.output_tokens,
            cache_tokens: r.cache_tokens,
            cost: r.cost.clone(),
            outcome: r.outcome.into(),
            http_status: r.http_status,
            latency_ms: r.latency_ms,
            error: r.error.map(String::from),
            cache_write_tokens: r.cache_write_tokens,
            session_key: r.session_key.map(String::from),
            route_mode: r.route_mode.map(String::from),
            route_reason: r.route_reason.map(String::from),
        }
    }

    pub fn record(&self) -> UsageRecord<'_> {
        UsageRecord {
            client_id: self.client_id.as_deref(),
            provider_id: &self.provider_id,
            model_id: &self.model_id,
            requested_model: &self.requested_model,
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            cache_tokens: self.cache_tokens,
            cost: &self.cost,
            outcome: &self.outcome,
            http_status: self.http_status,
            latency_ms: self.latency_ms,
            error: self.error.as_deref(),
            cache_write_tokens: self.cache_write_tokens,
            session_key: self.session_key.as_deref(),
            route_mode: self.route_mode.as_deref(),
            route_reason: self.route_reason.as_deref(),
            facts: None,
            repeated_tokens: None,
        }
    }
}

/// A ledger entry outside the attempt journal (tests, imports). The gateway settles attempts with `settle`.
pub fn record_usage(conn: &Connection, r: &UsageRecord) -> Result<()> {
    insert_usage(conn, r, None).map_err(err)
}

/// Fewer prompt tokens than this are never cached: 1,024 at OpenAI and for most Anthropic models (some need more).
pub const CACHE_MIN_TOKENS: u64 = 1024;

/// How far back a model's cache reads are looked for before saying it does not cache.
const CACHED_HERE_WINDOW_MS: i64 = 7 * 24 * 60 * 60 * 1000;

/// Why an answer read nothing from the provider's cache, worked out from the ledger before the answer is written
/// (migration 0034). `None` for a hit, a failure, an answer without token counts, or a row nothing sent. The reason
/// is the ledger's best reading, never the provider's word.
pub fn cache_miss(conn: &Connection, r: &UsageRecord, now: i64) -> rusqlite::Result<Option<&'static str>> {
    let (Some(facts), Some(input)) = (r.facts, r.input_tokens) else { return Ok(None) };
    if !matches!(r.outcome, "ok" | "client_disconnected" | "stream_error") {
        return Ok(None);
    }
    let Some(read) = r.cache_tokens else { return Ok(Some("not_reported")) };
    if read > 0 {
        return Ok(None);
    }
    if input < CACHE_MIN_TOKENS {
        return Ok(Some("below_minimum"));
    }
    if facts.cache_hint == "dropped" {
        return Ok(Some("marker_dropped"));
    }
    let cached_here = || -> rusqlite::Result<bool> {
        conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM usage_events WHERE created_at >= ?3 AND provider = ?1 AND model = ?2 \
             AND cache_tokens > 0)",
            params![r.provider_id, r.model_id, now - CACHED_HERE_WINDOW_MS],
            |row| row.get(0),
        )
    };
    let (Some(client), Some(session)) = (r.client_id, r.session_key) else {
        return Ok(Some(if cached_here()? { "no_session" } else { "not_cached_here" }));
    };
    // The session's last answer for this requested model, and its last one from this very model.
    let last = |same_target: bool| -> rusqlite::Result<Option<(String, String, i64)>> {
        conn.query_row(
            "SELECT provider, model, created_at FROM usage_events WHERE client_id = ?1 AND session_key = ?2 \
             AND requested_model = ?3 AND input_tokens IS NOT NULL AND (?4 = 0 OR (provider = ?5 AND model = ?6)) \
             ORDER BY created_at DESC LIMIT 1",
            params![client, session, r.requested_model, same_target as i64, r.provider_id, r.model_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
    };
    let Some((provider, model, _)) = last(false)? else { return Ok(Some("first_request")) };
    let here = last(true)?;
    let warm_here = here.as_ref().is_some_and(|(_, _, at)| now - at < CACHE_WARM_MS);
    if (provider != r.provider_id || model != r.model_id) && !warm_here {
        return Ok(Some("model_switched"));
    }
    if !cached_here()? {
        return Ok(Some("not_cached_here"));
    }
    Ok(Some(match here {
        None => "first_request",
        Some(_) if !warm_here => "cache_expired",
        Some(_) => "prefix_changed",
    }))
}

fn insert_usage(conn: &Connection, r: &UsageRecord, at_risk_micro: Option<u64>) -> rusqlite::Result<()> {
    let now = now_ms();
    let miss = cache_miss(conn, r, now)?;
    let prefix = r.facts.and_then(|f| f.prefix);
    let clearing = r.facts.and_then(|f| f.clearing);
    let hint = r.facts.and_then(|f| f.hint.as_ref());
    conn.execute(
        "INSERT INTO usage_events (id, client_id, provider, model, input_tokens, output_tokens, cache_tokens, \
         cost_micro_usd, cost_status, source, created_at, outcome, http_status, latency_ms, requested_model, error, \
         cache_write_tokens, session_key, route_mode, at_risk_micro_usd, route_reason, repeated_tokens, request_id, attempt, \
         billing, route_kind, cache_hint, key_id, cache_miss, tools_tokens, system_tokens, prefix_hash, clear_count, \
         clear_tokens, clear_tail_tokens, request_class, agent_type, agent_id, prompt_id) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, \
         ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39)",
        params![
            uuid::Uuid::new_v4().to_string(), r.client_id, r.provider_id, r.model_id,
            r.input_tokens.map(|v| v as i64), r.output_tokens.map(|v| v as i64), r.cache_tokens.map(|v| v as i64),
            r.cost.micro_usd().map(|v| v as i64), r.cost.status(), r.cost.source(), now, r.outcome,
            r.http_status, r.latency_ms as i64, r.requested_model, r.error,
            r.cache_write_tokens.map(|v| v as i64), r.session_key, r.route_mode, at_risk_micro.map(|v| v as i64), r.route_reason,
            r.repeated_tokens.map(|v| v as i64), r.facts.map(|f| f.request_id.as_str()), r.facts.map(|f| f.attempt as i64),
            r.facts.map(|f| f.billing.as_str()), r.facts.map(|f| f.route_kind.as_str()), r.facts.map(|f| f.cache_hint.as_str()),
            r.facts.and_then(|f| f.key_id.as_deref()), miss,
            prefix.map(|p| p.tools_tokens as i64), prefix.map(|p| p.system_tokens as i64), prefix.map(|p| p.hash),
            clearing.map(|c| c.count as i64), clearing.map(|c| c.tokens as i64), clearing.map(|c| c.tail_tokens as i64),
            hint.and_then(|h| h.class.as_deref()), hint.and_then(|h| h.agent_type.as_deref()),
            hint.and_then(|h| h.agent_id.as_deref()), hint.and_then(|h| h.prompt_id.as_deref()),
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
    /// Why auto routing chose this target.
    pub route_reason: Option<String>,
    /// The request this attempt belongs to, and its place among its attempts (migration 0030; null before).
    pub request_id: Option<String>,
    pub attempt: Option<u32>,
    /// The billing type it was sent under, the route's kind and what happened to cache hints (migration 0030).
    pub billing: Option<String>,
    pub route_kind: Option<String>,
    pub cache_hint: Option<String>,
    /// The key of the connection's key ring that sent it, and its name (ADR-038); never the secret.
    pub key_id: Option<String>,
    pub key_label: Option<String>,
    /// Why the answer read nothing from the provider's cache (migration 0034; `store::cache_miss`).
    pub cache_miss: Option<String>,
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

/// Requests and money in one period of a usage chart.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageBucket {
    /// When the period starts, in milliseconds.
    pub start: i64,
    pub requests: u64,
    pub failed: u64,
    /// Actual and estimated costs; unknown ones are not in it.
    pub known_micro_usd: u64,
    pub actual_micro_usd: u64,
    pub unknown_requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Usage since `since` in periods of `bucket_ms`, aligned to the user's clock (`offset_ms` east of UTC), oldest first.
/// Periods with no request are left out; the chart shows them as empty.
pub fn usage_over_time(conn: &Connection, since: i64, bucket_ms: i64, offset_ms: i64) -> Result<Vec<UsageBucket>> {
    let bucket_ms = bucket_ms.max(60_000);
    let mut stmt = conn
        .prepare(
            "SELECT ((created_at + ?3) / ?2) * ?2 - ?3 AS start, COUNT(*), SUM(outcome NOT IN ('ok', 'fallback')), \
         COALESCE(SUM(cost_micro_usd), 0), COALESCE(SUM(CASE WHEN cost_status = 'actual' THEN cost_micro_usd END), 0), \
         SUM(cost_status = 'unknown'), COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0) \
         FROM usage_events WHERE created_at >= ?1 GROUP BY start ORDER BY start",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map(params![since, bucket_ms, offset_ms], |r| {
            let n = |i: usize| r.get::<_, Option<i64>>(i).map(|v| v.unwrap_or(0).max(0) as u64);
            Ok(UsageBucket {
                start: r.get(0)?,
                requests: n(1)?,
                failed: n(2)?,
                known_micro_usd: n(3)?,
                actual_micro_usd: n(4)?,
                unknown_requests: n(5)?,
                input_tokens: n(6)?,
                output_tokens: n(7)?,
            })
        })
        .map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

/// A UTC time in milliseconds as `YYYY-MM-DDTHH:MM:SS.mmmZ` (ISO 8601), the form spreadsheets read.
pub fn iso_utc(ms: i64) -> String {
    let (days, rest) = (ms.div_euclid(86_400_000), ms.rem_euclid(86_400_000));
    // Days since 1970-01-01 to a civil date (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rest / 3_600_000,
        rest / 60_000 % 60,
        rest / 1000 % 60,
        rest % 1000
    )
}

/// One CSV field: quoted when it holds a comma, quote or line break, and a leading `=`, `+`, `@`, tab or carriage return
/// prefixed with `'`, so a spreadsheet never runs text from a provider's error as a formula.
fn csv_field(value: &str) -> String {
    let value = if value.starts_with(['=', '+', '@', '\t', '\r']) { format!("'{value}") } else { value.to_string() };
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value
    }
}

/// Every request since `since`, oldest first, as CSV for a spreadsheet (Usage → Export CSV): the ledger's own figures,
/// costs in USD with their status and source, unknown costs left empty rather than 0.
pub fn usage_csv(conn: &Connection, since: i64) -> Result<String> {
    let mut stmt = conn.prepare(
        "SELECT u.created_at, COALESCE(c.name, ''), COALESCE(u.requested_model, ''), u.provider, u.model, COALESCE(u.route_mode, ''), \
         u.outcome, u.http_status, u.input_tokens, u.output_tokens, u.cache_tokens, u.cache_write_tokens, u.cost_micro_usd, \
         u.cost_status, COALESCE(u.source, ''), u.at_risk_micro_usd, u.latency_ms, COALESCE(u.session_key, ''), \
         COALESCE(u.route_reason, ''), COALESCE(u.error, '') \
         FROM usage_events u LEFT JOIN gateway_clients c ON c.id = u.client_id WHERE u.created_at >= ?1 ORDER BY u.created_at").map_err(err)?;
    let usd = |micro: Option<i64>| micro.map(|m| format!("{:.6}", m as f64 / 1e6)).unwrap_or_default();
    let num = |n: Option<i64>| n.map(|n| n.to_string()).unwrap_or_default();
    let mut out = String::from("time_utc,client,requested_model,provider,model,route,outcome,http_status,input_tokens,output_tokens,\
        cache_read_tokens,cache_write_tokens,cost_usd,cost_status,cost_source,at_risk_usd,latency_ms,session,route_reason,error\r\n");
    let rows = stmt
        .query_map([since], |r| {
            let text = |i: usize| r.get::<_, String>(i).map(|s| csv_field(&s));
            let status: String = r.get(13)?;
            // An unknown cost is empty, never 0 (ADR-011).
            let cost = if status == "unknown" { String::new() } else { usd(r.get(12)?) };
            Ok([
                iso_utc(r.get(0)?),
                text(1)?,
                text(2)?,
                text(3)?,
                text(4)?,
                text(5)?,
                text(6)?,
                num(r.get(7)?),
                num(r.get(8)?),
                num(r.get(9)?),
                num(r.get(10)?),
                num(r.get(11)?),
                cost,
                csv_field(&status),
                text(14)?,
                usd(r.get(15)?),
                num(r.get(16)?),
                text(17)?,
                text(18)?,
                text(19)?,
            ]
            .join(","))
        })
        .map_err(err)?;
    for row in rows {
        out.push_str(&row.map_err(err)?);
        out.push_str("\r\n");
    }
    Ok(out)
}

/// What deduplicating repeated tool output could have saved in a period (P1-4, shadow): requests that carried tool
/// output, the about-tokens they repeated, and their input and cache-read tokens to weigh it against.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShadowSummary {
    pub requests: u64,
    pub requests_with_repeats: u64,
    pub repeated_tokens: u64,
    pub input_tokens: u64,
    pub cache_read_tokens: u64,
    /// Clearing old tool results, in shadow (migration 0037).
    pub clearing: ClearingShadow,
    /// A longer cache lifetime, in shadow.
    pub lifetime: CacheLifetime,
}

/// Anthropic's longer cache lifetime: one hour instead of five minutes.
pub const CACHE_HOUR_MS: i64 = 60 * 60 * 1000;
/// How often a keep-alive would re-send a session's last request to hold its five-minute cache: just under five minutes.
pub const KEEP_ALIVE_MS: i64 = 270 * 1000;

/// What a longer cache lifetime would have changed in a period (ROADMAP item 4, measured only): for each session on a
/// model that bills cache writes (Anthropic's way), the answers that missed only because more than five minutes had
/// passed, and what two ways of keeping the cache longer would have cost and saved.
/// - **One-hour lifetime:** every cache write at twice the input price instead of the five-minute write price; a miss
///   after a pause of up to an hour reads the session's previous prompt instead of writing it again.
/// - **Keep-alive:** the session's last request re-sent every 4.5 minutes, read from the cache, for up to an hour
///   after it; the same misses become reads, and every re-send is a cache read of the whole last prompt, paid also
///   when the user never came back.
///
/// Estimates from the ledger's tokens and each model's current price. Nothing is sent.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheLifetime {
    pub sessions: u64,
    pub requests: u64,
    /// Pauses of five minutes to an hour between two answers of one session and model.
    pub idle_gaps: u64,
    /// Answers after such a pause that read nothing because the cache had expired, and the tokens of the previous
    /// prompt they wrote again.
    pub expired_misses: u64,
    pub recovered_tokens: u64,
    /// What reading those tokens instead of writing them again would save (cache-write price less cached price).
    pub saved_micro_usd: u64,
    /// One hour: the extra on every other cache write, and the difference.
    pub hour_extra_micro_usd: u64,
    pub hour_net_micro_usd: i64,
    /// Keep-alive: the re-sends, their cache reads, and the difference.
    pub keep_alive_requests: u64,
    pub keep_alive_extra_micro_usd: u64,
    pub keep_alive_net_micro_usd: i64,
    /// Answers on a model without a cached price: counted, not in the money.
    pub unpriced: u64,
}

pub fn cache_lifetime(conn: &Connection, since: i64, now: i64) -> Result<CacheLifetime> {
    use std::collections::HashMap;
    let prices: HashMap<(String, String), Price> = list_models(conn, None)?
        .into_iter()
        .filter_map(|m| m.price.map(|p| ((m.provider_id, m.model_id), p)))
        .collect();
    // Only models that wrote to the cache in the period: a provider that never reports writes (OpenAI, DeepSeek, Gemini)
    // caches on its own terms, without a five-minute lifetime to buy longer.
    let mut stmt = conn
        .prepare(
            "SELECT u.provider, u.model, u.client_id, u.session_key, u.created_at, u.input_tokens, \
             COALESCE(u.cache_write_tokens, 0), COALESCE(u.cache_miss, '') FROM usage_events u \
             WHERE u.created_at >= ?1 AND u.outcome = 'ok' AND u.session_key IS NOT NULL AND u.input_tokens IS NOT NULL \
             AND EXISTS (SELECT 1 FROM usage_events w WHERE w.provider = u.provider AND w.model = u.model \
               AND w.created_at >= ?1 AND w.cache_write_tokens > 0) \
             ORDER BY u.client_id, u.session_key, u.provider, u.model, u.created_at, u.rowid",
        )
        .map_err(err)?;
    type Row = (String, String, Option<String>, String, i64, u64, u64, String);
    let rows: Vec<Row> = stmt
        .query_map([since], |r| {
            let n = |i: usize| r.get::<_, i64>(i).map(|v| v.max(0) as u64);
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, n(5)?, n(6)?, r.get(7)?))
        })
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;

    let mut out = CacheLifetime::default();
    let (mut saved, mut hour_extra, mut keep_extra) = (0u128, 0u128, 0u128);
    // Re-sends held after an answer, until the next one or for at most an hour.
    let pings = |gap: i64| -> u64 { (gap.min(CACHE_HOUR_MS) / KEEP_ALIVE_MS).max(0) as u64 };
    let mut i = 0;
    while i < rows.len() {
        let key = |r: &Row| (r.2.clone(), r.3.clone(), r.0.clone(), r.1.clone());
        let mut end = i;
        while end < rows.len() && key(&rows[end]) == key(&rows[i]) {
            end += 1;
        }
        let group = &rows[i..end];
        out.sessions += 1;
        out.requests += group.len() as u64;
        let price = prices.get(&(group[0].0.clone(), group[0].1.clone()));
        let (mut recovered, mut writes, mut reads) = (0u64, 0u64, 0u64);
        for (j, row) in group.iter().enumerate() {
            let (at, prompt, write, miss) = (row.4, row.5, row.6, row.7.as_str());
            writes += write;
            let next_at = group.get(j + 1).map(|n| n.4).unwrap_or(now);
            let held = pings(next_at - at);
            out.keep_alive_requests += held;
            reads += held * prompt;
            let Some(before) = j.checked_sub(1).map(|b| &group[b]) else { continue };
            let gap = at - before.4;
            if !(CACHE_WARM_MS..=CACHE_HOUR_MS).contains(&gap) {
                continue;
            }
            out.idle_gaps += 1;
            if miss == "cache_expired" {
                out.expired_misses += 1;
                recovered += write.min(before.5);
            }
        }
        out.recovered_tokens += recovered;
        match price.and_then(|p| p.cached_input_micro_per_mtok.map(|cached| (p, cached))) {
            None => out.unpriced += group.len() as u64,
            Some((p, cached)) => {
                let write5 = p.cache_write_micro_per_mtok.unwrap_or(p.input_micro_per_mtok);
                let write60 = p.input_micro_per_mtok * 2;
                saved += u128::from(recovered) * u128::from(write5.saturating_sub(cached));
                hour_extra += u128::from(writes - recovered) * u128::from(write60.saturating_sub(write5));
                keep_extra += u128::from(reads) * u128::from(cached);
            }
        }
        i = end;
    }
    out.saved_micro_usd = (saved / 1_000_000) as u64;
    out.hour_extra_micro_usd = (hour_extra / 1_000_000) as u64;
    out.keep_alive_extra_micro_usd = (keep_extra / 1_000_000) as u64;
    out.hour_net_micro_usd = out.saved_micro_usd as i64 - out.hour_extra_micro_usd as i64;
    out.keep_alive_net_micro_usd = out.saved_micro_usd as i64 - out.keep_alive_extra_micro_usd as i64;
    Ok(out)
}

/// What clearing old tool results (Anthropic's context editing, at its defaults) would have saved in a period, after the
/// cache writes it causes (ROADMAP item 4). OpenAI-format clients only; answered requests with tool output. Estimates:
/// the tokens are the gateway's, the money is each model's current price. Turning it on is worth asking only if `net` is
/// above zero.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearingShadow {
    /// Requests measured, and those past the trigger, where something would be cleared.
    pub requests: u64,
    pub over_trigger: u64,
    /// Tool-result tokens that would not have been sent.
    pub cleared_tokens: u64,
    /// Tokens written to the cache again because a request cleared more than the one before it in its session.
    pub rewritten_tokens: u64,
    /// The input those tokens cost (at the cached price where the request read from the cache, else the input price),
    /// the extra the rewrites would cost (cache-write price less cached price), and the difference, in micro-USD.
    pub saved_micro_usd: u64,
    pub extra_micro_usd: u64,
    pub net_micro_usd: i64,
    /// Requests past the trigger whose model has no price now: not in the money.
    pub unpriced: u64,
}

pub fn clearing_shadow(conn: &Connection, since: i64) -> Result<ClearingShadow> {
    use std::collections::HashMap;
    let prices: HashMap<(String, String), Price> = list_models(conn, None)?
        .into_iter()
        .filter_map(|m| m.price.map(|p| ((m.provider_id, m.model_id), p)))
        .collect();
    let mut stmt = conn
        .prepare(
            "SELECT provider, model, client_id, session_key, clear_count, clear_tokens, clear_tail_tokens, \
             COALESCE(cache_tokens, 0) FROM usage_events \
             WHERE created_at >= ?1 AND outcome = 'ok' AND clear_count IS NOT NULL \
             ORDER BY client_id, session_key, created_at, rowid",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map([since], |r| {
            let n = |i: usize| r.get::<_, Option<i64>>(i).map(|v| v.unwrap_or(0).max(0) as u64);
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                n(4)?,
                n(5)?,
                n(6)?,
                n(7)?,
            ))
        })
        .map_err(err)?;
    let mut out = ClearingShadow::default();
    // The cleared count of the session's request before; a request without a session is taken as clearing anew.
    let mut before: Option<(Option<String>, String, u64)> = None;
    let (mut saved, mut extra) = (0u128, 0u128);
    for row in rows {
        let (provider, model, client, session, count, cleared, tail, read) = row.map_err(err)?;
        out.requests += 1;
        let previous = match (&session, &before) {
            (Some(s), Some((c, bs, n))) if *c == client && bs == s => *n,
            _ => 0,
        };
        if let Some(s) = &session {
            before = Some((client.clone(), s.clone(), count));
        }
        if count == 0 {
            continue;
        }
        out.over_trigger += 1;
        out.cleared_tokens += cleared;
        let rewritten = if count > previous { tail } else { 0 };
        out.rewritten_tokens += rewritten;
        let Some(price) = prices.get(&(provider, model)) else {
            out.unpriced += 1;
            continue;
        };
        let cached = price.cached_input_micro_per_mtok.unwrap_or(price.input_micro_per_mtok);
        let write = price.cache_write_micro_per_mtok.unwrap_or(price.input_micro_per_mtok);
        let per_token = if read > 0 { cached } else { price.input_micro_per_mtok };
        saved += u128::from(cleared) * u128::from(per_token);
        // A rewrite costs extra only where the request read from the cache; otherwise it was paid in full already.
        if read > 0 {
            extra += u128::from(rewritten) * u128::from(write.saturating_sub(cached));
        }
    }
    out.saved_micro_usd = (saved / 1_000_000) as u64;
    out.extra_micro_usd = (extra / 1_000_000) as u64;
    out.net_micro_usd = out.saved_micro_usd as i64 - out.extra_micro_usd as i64;
    Ok(out)
}

pub fn shadow_summary(conn: &Connection, since: i64) -> Result<ShadowSummary> {
    conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(repeated_tokens > 0), 0), COALESCE(SUM(repeated_tokens), 0), \
        COALESCE(SUM(input_tokens), 0), COALESCE(SUM(cache_tokens), 0) FROM usage_events \
        WHERE created_at >= ?1 AND repeated_tokens IS NOT NULL AND outcome = 'ok'",
        [since],
        |r| {
            let n = |i: usize| r.get::<_, i64>(i).map(|v| v.max(0) as u64);
            Ok(ShadowSummary {
                requests: n(0)?,
                requests_with_repeats: n(1)?,
                repeated_tokens: n(2)?,
                input_tokens: n(3)?,
                cache_read_tokens: n(4)?,
                clearing: ClearingShadow::default(),
                lifetime: CacheLifetime::default(),
            })
        },
    )
    .map_err(err)
    .and_then(|summary| {
        Ok(ShadowSummary {
            clearing: clearing_shadow(conn, since)?,
            lifetime: cache_lifetime(conn, since, now_ms())?,
            ..summary
        })
    })
}

/// How a model has answered, from the ledger (SCREENS §5.4: "reliability appears only once measured, with the sample
/// size"): requests that reached it, those that succeeded, and the median time to answer them.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reliability {
    pub model_id: String,
    pub requests: u64,
    pub ok: u64,
    pub median_latency_ms: Option<u64>,
}

/// Reliability of a connection's models since `since`. A request the client left before it finished counts neither
/// way: it says nothing about the model.
pub fn reliability(conn: &Connection, provider_id: &str, since: i64) -> Result<Vec<Reliability>> {
    let mut stmt = conn
        .prepare(
            "SELECT model, outcome, latency_ms FROM usage_events WHERE provider = ?1 AND created_at >= ?2 \
        AND outcome <> 'client_disconnected' ORDER BY model",
        )
        .map_err(err)?;
    let rows: Vec<(String, String, Option<i64>)> = stmt
        .query_map(params![provider_id, since], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;
    let mut out: Vec<Reliability> = Vec::new();
    let mut latencies: Vec<Vec<u64>> = Vec::new();
    for (model, outcome, latency) in rows {
        if out.last().map(|r| r.model_id != model).unwrap_or(true) {
            out.push(Reliability { model_id: model.clone(), requests: 0, ok: 0, median_latency_ms: None });
            latencies.push(Vec::new());
        }
        let (r, l) = (out.last_mut().unwrap(), latencies.last_mut().unwrap());
        r.requests += 1;
        if outcome == "ok" {
            r.ok += 1;
            if let Some(ms) = latency {
                l.push(ms.max(0) as u64);
            }
        }
    }
    for (r, mut l) in out.iter_mut().zip(latencies) {
        l.sort_unstable();
        r.median_latency_ms = (!l.is_empty()).then(|| l[l.len() / 2]);
    }
    Ok(out)
}

/// Something in the ledger worth a look (SCREENS §5.6, anomalies). Each is a plain reading of recorded figures, never
/// a forecast: a client near or over its monthly limit, a day that cost far more than the week before it, requests whose
/// cost is unknown, and a burst of failures.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageAlert {
    /// `near_limit`, `over_limit`, `spend_spike`, `unknown_costs` or `failures`.
    pub kind: String,
    pub client: Option<String>,
    /// The figure that raised it: micro-USD for money, a count for requests.
    pub value: u64,
    /// What it was measured against: the limit, the week's daily average, or the requests counted.
    pub against: u64,
}

/// A day's known spend at least this many times the daily average of the seven days before it raises `spend_spike`,
/// if it is also at least `SPIKE_FLOOR_MICRO_USD` (cents are not an anomaly).
pub const SPIKE_FACTOR: u64 = 3;
pub const SPIKE_FLOOR_MICRO_USD: u64 = 500_000;

/// The newest request each API key sent, for CLI Tools' "Working" (SCREENS §2.2): which model answered, and how.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestRequest {
    pub client_id: String,
    pub created_at: i64,
    pub provider: String,
    pub model: String,
    pub requested_model: Option<String>,
    pub outcome: String,
}

pub fn latest_requests(conn: &Connection) -> Result<Vec<LatestRequest>> {
    let mut stmt = conn.prepare("SELECT u.client_id, u.created_at, u.provider, u.model, u.requested_model, u.outcome \
        FROM usage_events u JOIN (SELECT client_id, MAX(created_at) AS at FROM usage_events WHERE client_id IS NOT NULL \
        GROUP BY client_id) m ON m.client_id = u.client_id AND m.at = u.created_at ORDER BY u.created_at DESC").map_err(err)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(LatestRequest {
                client_id: r.get(0)?,
                created_at: r.get(1)?,
                provider: r.get(2)?,
                model: r.get(3)?,
                requested_model: r.get(4)?,
                outcome: r.get(5)?,
            })
        })
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err)?;
    // Two entries at the same millisecond: the first is enough.
    let mut seen = std::collections::HashSet::new();
    Ok(rows.into_iter().filter(|r| seen.insert(r.client_id.clone())).collect())
}

pub fn usage_alerts(conn: &Connection, now: i64) -> Result<Vec<UsageAlert>> {
    let mut out = Vec::new();
    for c in list_clients(conn)?.into_iter().filter(|c| c.revoked_at.is_none()) {
        let Some(limit) = c.monthly_budget_micro_usd.filter(|l| *l > 0) else { continue };
        let used = c.month_spend_micro_usd + c.reserved_micro_usd;
        let kind = if used >= limit {
            "over_limit"
        } else if used * 5 >= limit * 4 {
            "near_limit"
        } else {
            continue;
        };
        out.push(UsageAlert { kind: kind.into(), client: Some(c.name), value: used, against: limit });
    }
    let day = 86_400_000;
    let one = |sql: &str, params: &[&dyn rusqlite::ToSql]| -> Result<i64> {
        conn.query_row(sql, params, |r| r.get::<_, Option<i64>>(0)).map(|v| v.unwrap_or(0)).map_err(err)
    };
    let known = "SELECT SUM(cost_micro_usd) FROM usage_events WHERE cost_status <> 'unknown' AND created_at >= ?1 AND created_at < ?2";
    let today = one(known, &[&(now - day), &now])?.max(0) as u64;
    let week = one(known, &[&(now - 8 * day), &(now - day)])?.max(0) as u64;
    let average = week / 7;
    if week > 0 && today >= SPIKE_FLOOR_MICRO_USD && today >= average.max(1) * SPIKE_FACTOR {
        out.push(UsageAlert { kind: "spend_spike".into(), client: None, value: today, against: average });
    }
    let unknown =
        one("SELECT COUNT(*) FROM usage_events WHERE cost_status = 'unknown' AND created_at >= ?1", &[&(now - day)])?
            .max(0) as u64;
    if unknown >= 5 {
        out.push(UsageAlert { kind: "unknown_costs".into(), client: None, value: unknown, against: 0 });
    }
    let (total, failed): (i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(outcome NOT IN ('ok', 'fallback', 'client_disconnected')), 0) FROM usage_events WHERE created_at >= ?1",
        [now - 3_600_000], |r| Ok((r.get(0)?, r.get(1)?))).map_err(err)?;
    if total >= 10 && failed * 2 >= total {
        out.push(UsageAlert { kind: "failures".into(), client: None, value: failed as u64, against: total as u64 });
    }
    Ok(out)
}

/// One change of model inside a routing session: the answered attempt before it and the one after.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteChange {
    pub at: i64,
    pub client: Option<String>,
    /// `connection/model` that answered before, and the one that answered now.
    pub from: String,
    pub to: String,
    /// Why: the failed attempts of this request before the answer, as `HTTP 429` or the outcome; empty when the request
    /// went to the new model first (a pin that expired, the route changed).
    pub reason: String,
    /// Prompt tokens the new model wrote to its cache on that answer, as its provider reported them.
    pub cache_writes_after: u64,
    pub known_micro_usd: Option<u64>,
}

/// How routing held together in a period (Codex's fifth discussion, T2), from attempts recorded with their request
/// (migration 0030): requests, those answered only after a fallback, the attempts that carried a routing session, and
/// each change of answering model inside a session, with the cache writes that followed. What a change cost extra is
/// not known (it would need what staying would have cost), so it is never called a cost.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingSummary {
    /// Client requests recorded with an id, and attempts among them.
    pub requests: u64,
    pub attempts: u64,
    /// Requests answered, and those answered after one or more fallback attempts.
    pub answered: u64,
    pub answered_after_fallback: u64,
    /// Attempts that carried a routing session.
    pub with_session: u64,
    pub sessions: u64,
    pub changes: Vec<RouteChange>,
}

pub fn routing_summary(conn: &Connection, since: i64) -> Result<RoutingSummary> {
    let mut out = RoutingSummary::default();
    let mut stmt = conn
        .prepare(
            "SELECT u.request_id, u.attempt, u.outcome, u.http_status, u.provider, u.model, u.session_key, \
        u.created_at, c.name, u.cache_write_tokens, u.cost_status, u.cost_micro_usd \
        FROM usage_events u LEFT JOIN gateway_clients c ON c.id = u.client_id \
        WHERE u.created_at >= ?1 AND u.request_id IS NOT NULL ORDER BY u.created_at, u.attempt",
        )
        .map_err(err)?;
    type Row = (
        String,
        Option<i64>,
        String,
        Option<i64>,
        String,
        String,
        Option<String>,
        i64,
        Option<String>,
        Option<i64>,
        String,
        Option<i64>,
    );
    let rows: Vec<Row> = stmt
        .query_map([since], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get(8)?,
                r.get(9)?,
                r.get(10)?,
                r.get(11)?,
            ))
        })
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;
    // The failed attempts of each request so far, for the reason of a change.
    let mut failures: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    let mut requests = std::collections::HashSet::new();
    let mut sessions: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for (request, _attempt, outcome, status, provider, model, session, at, client, written, cost_status, cost) in rows {
        out.attempts += 1;
        requests.insert(request.clone());
        if session.is_some() {
            out.with_session += 1;
        }
        if outcome != "ok" {
            failures.entry(request).or_default().push(match status {
                Some(s) if s != 200 => format!("HTTP {s}"),
                _ => outcome.replace('_', " "),
            });
            continue;
        }
        out.answered += 1;
        let failed = failures.remove(&request).unwrap_or_default();
        if !failed.is_empty() {
            out.answered_after_fallback += 1;
        }
        let Some(session) = session else { continue };
        let target = format!("{provider}/{model}");
        if let Some(before) = sessions.insert(session, target.clone()) {
            if before != target {
                out.changes.push(RouteChange {
                    at,
                    client,
                    from: before,
                    to: target,
                    reason: failed.join(", "),
                    cache_writes_after: written.unwrap_or(0).max(0) as u64,
                    known_micro_usd: (cost_status != "unknown").then(|| cost.unwrap_or(0).max(0) as u64),
                });
            }
        }
    }
    out.requests = requests.len() as u64;
    out.sessions = sessions.len() as u64;
    out.changes.reverse();
    Ok(out)
}

/// Tokens of one kind, and the part of the recorded cost that went to them.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenParts {
    /// Prompt tokens neither read from nor written to the provider's cache.
    pub new_input: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub output: u64,
}

/// The token mix of one client or model.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenMixGroup {
    pub key: String,
    pub tokens: TokenParts,
    /// Requests with token counts.
    pub requests: u64,
    /// The recorded known cost of those requests (actual and estimated).
    pub known_micro_usd: u64,
}

/// Where a period's tokens went (Codex's fifth discussion, T1): new input, cache reads, cache writes and output, and
/// each request's recorded known cost split among them by its model's current prices. Read from the ledger; nothing
/// is changed. A provider that reports no cache figures shows its whole prompt as new input: the ledger cannot tell
/// "none" from "not reported".
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenMix {
    pub tokens: TokenParts,
    /// The recorded known cost split by kind, in micro-USD; only requests whose model has a price now.
    pub cost: TokenParts,
    /// Requests in the period, and those with token counts.
    pub requests: u64,
    pub with_tokens: u64,
    /// Known cost that could not be split, its model having no price now (a provider's own charge, a price removed).
    pub unsplit_micro_usd: u64,
    /// Requests with token counts whose cost is unknown: their tokens count, their money does not.
    pub unknown_cost_requests: u64,
    pub by_client: Vec<TokenMixGroup>,
    pub by_model: Vec<TokenMixGroup>,
    /// Answers that read nothing from the cache, by why (migration 0034), most prompt tokens first.
    pub misses: Vec<CacheMissGroup>,
    /// What sessions carry before their conversation, and how often it was sent again uncached (migration 0036).
    pub prefix: PrefixMeter,
    /// Claude Code's requests by kind and its subagents' fan-out, from its hint headers (migration 0039).
    pub classes: super::hints::RequestClasses,
}

/// The tool definitions and system prompts of the period's sessions (ROADMAP item 4), from their answered requests.
/// Sizes are the gateway's estimate from the request; a request without a session is not in it.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefixMeter {
    pub sessions: u64,
    pub requests: u64,
    /// Summed over sessions, each counted at its first request.
    pub first_tools_tokens: u64,
    pub first_system_tokens: u64,
    /// Requests whose tools or system prompt differed from the session's request before.
    pub changes: u64,
    /// Requests after a session's first that read nothing from the cache though the provider reports it: the tools and
    /// system prompt went again at full price or as a cache write. Their size, summed.
    pub resent: u64,
    pub resent_tokens: u64,
    /// The sessions that sent them again the most, at most five.
    pub top: Vec<PrefixSession>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefixSession {
    pub client: String,
    pub session: String,
    pub tools_tokens: u64,
    pub system_tokens: u64,
    pub requests: u64,
    pub changes: u64,
    pub resent: u64,
    pub resent_tokens: u64,
}

/// Misses that say nothing about the prefix: the provider did not report its cache, or the prompt was too short.
const NOT_A_RESEND: [&str; 2] = ["not_reported", "below_minimum"];

pub fn prefix_meter(conn: &Connection, since: i64) -> Result<PrefixMeter> {
    let mut stmt = conn
        .prepare(
            "SELECT COALESCE(c.name, ''), u.client_id, u.session_key, u.tools_tokens, u.system_tokens, u.prefix_hash, \
             u.cache_miss FROM usage_events u LEFT JOIN gateway_clients c ON c.id = u.client_id \
             WHERE u.created_at >= ?1 AND u.outcome = 'ok' AND u.prefix_hash IS NOT NULL AND u.session_key IS NOT NULL \
             ORDER BY u.client_id, u.session_key, u.created_at, u.rowid",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map([since], |r| {
            let n = |i: usize| r.get::<_, Option<i64>>(i).map(|v| v.unwrap_or(0).max(0) as u64);
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                r.get::<_, String>(2)?,
                n(3)?,
                n(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, Option<String>>(6)?,
            ))
        })
        .map_err(err)?;
    let mut meter = PrefixMeter::default();
    let mut sessions: Vec<PrefixSession> = Vec::new();
    let mut current: Option<(String, String, i64)> = None;
    for row in rows {
        let (name, client, session, tools, system, hash, miss) = row.map_err(err)?;
        let same = current.as_ref().is_some_and(|(c, s, _)| *c == client && *s == session);
        if !same {
            sessions.push(PrefixSession {
                client: name,
                session: session.clone(),
                tools_tokens: tools,
                system_tokens: system,
                ..PrefixSession::default()
            });
        }
        let entry = sessions.last_mut().expect("a session was pushed");
        entry.requests += 1;
        if same {
            if current.as_ref().is_some_and(|(_, _, previous)| *previous != hash) {
                entry.changes += 1;
            }
            if miss.as_deref().is_some_and(|m| !NOT_A_RESEND.contains(&m)) {
                entry.resent += 1;
                entry.resent_tokens += tools + system;
            }
        }
        current = Some((client, session, hash));
    }
    for s in &sessions {
        meter.sessions += 1;
        meter.requests += s.requests;
        meter.first_tools_tokens += s.tools_tokens;
        meter.first_system_tokens += s.system_tokens;
        meter.changes += s.changes;
        meter.resent += s.resent;
        meter.resent_tokens += s.resent_tokens;
    }
    sessions.sort_by(|a, b| {
        b.resent_tokens
            .cmp(&a.resent_tokens)
            .then_with(|| (b.tools_tokens + b.system_tokens).cmp(&(a.tools_tokens + a.system_tokens)))
            .then_with(|| a.session.cmp(&b.session))
    });
    sessions.truncate(5);
    meter.top = sessions;
    Ok(meter)
}

/// Answers that missed the cache for one reason, and their prompt tokens, all read in full or written to the cache.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheMissGroup {
    pub cause: String,
    pub requests: u64,
    pub input_tokens: u64,
}

pub fn token_mix(conn: &Connection, since: i64) -> Result<TokenMix> {
    use std::collections::HashMap;
    let prices: HashMap<(String, String), Price> = list_models(conn, None)?
        .into_iter()
        .filter_map(|m| m.price.map(|p| ((m.provider_id, m.model_id), p)))
        .collect();
    let mut mix = TokenMix {
        requests: conn
            .query_row("SELECT COUNT(*) FROM usage_events WHERE created_at >= ?1", [since], |r| r.get::<_, i64>(0))
            .map_err(err)?
            .max(0) as u64,
        ..TokenMix::default()
    };
    let mut stmt = conn
        .prepare(
            "SELECT u.provider, u.model, COALESCE(c.name, ''), u.input_tokens, COALESCE(u.cache_tokens, 0), \
        COALESCE(u.cache_write_tokens, 0), COALESCE(u.output_tokens, 0), u.cost_status, u.cost_micro_usd \
        FROM usage_events u LEFT JOIN gateway_clients c ON c.id = u.client_id \
        WHERE u.created_at >= ?1 AND (u.input_tokens IS NOT NULL OR u.output_tokens IS NOT NULL)",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map([since], |r| {
            let n = |i: usize| r.get::<_, Option<i64>>(i).map(|v| v.unwrap_or(0).max(0) as u64);
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                n(3)?,
                n(4)?,
                n(5)?,
                n(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, Option<i64>>(8)?,
            ))
        })
        .map_err(err)?;
    let mut by_client: HashMap<String, TokenMixGroup> = HashMap::new();
    let mut by_model: HashMap<String, TokenMixGroup> = HashMap::new();
    for row in rows {
        let (provider, model, client, input, read, write, output, status, cost) = row.map_err(err)?;
        // An Anthropic prompt counts its cache reads and writes in the whole (pricing.rs); an OpenAI one its reads.
        let parts = TokenParts {
            new_input: input.saturating_sub(read + write),
            cache_read: read.min(input),
            cache_write: write,
            output,
        };
        let known = (status != "unknown").then(|| cost.unwrap_or(0).max(0) as u64);
        mix.with_tokens += 1;
        add_parts(&mut mix.tokens, &parts);
        match known {
            None => mix.unknown_cost_requests += 1,
            Some(0) => {}
            Some(micro) => match prices.get(&(provider.clone(), model.clone())) {
                Some(price) => add_parts(&mut mix.cost, &split_cost(micro, &parts, price)),
                None => mix.unsplit_micro_usd += micro,
            },
        }
        for (map, key) in [(&mut by_client, client), (&mut by_model, format!("{provider}/{model}"))] {
            let group = map.entry(key.clone()).or_insert_with(|| TokenMixGroup { key, ..TokenMixGroup::default() });
            add_parts(&mut group.tokens, &parts);
            group.requests += 1;
            group.known_micro_usd += known.unwrap_or(0);
        }
    }
    let sorted = |map: HashMap<String, TokenMixGroup>| -> Vec<TokenMixGroup> {
        let mut groups: Vec<TokenMixGroup> = map.into_values().collect();
        let total =
            |g: &TokenMixGroup| g.tokens.new_input + g.tokens.cache_read + g.tokens.cache_write + g.tokens.output;
        groups.sort_by(|a, b| total(b).cmp(&total(a)).then_with(|| a.key.cmp(&b.key)));
        groups
    };
    mix.by_client = sorted(by_client);
    mix.by_model = sorted(by_model);
    let mut stmt = conn
        .prepare(
            "SELECT cache_miss, COUNT(*), COALESCE(SUM(input_tokens), 0) FROM usage_events \
             WHERE created_at >= ?1 AND cache_miss IS NOT NULL GROUP BY cache_miss ORDER BY 3 DESC, 2 DESC, 1",
        )
        .map_err(err)?;
    mix.misses = stmt
        .query_map([since], |r| {
            Ok(CacheMissGroup {
                cause: r.get(0)?,
                requests: r.get::<_, i64>(1)?.max(0) as u64,
                input_tokens: r.get::<_, i64>(2)?.max(0) as u64,
            })
        })
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;
    mix.prefix = prefix_meter(conn, since)?;
    mix.classes = super::hints::request_classes(conn, since, None)?;
    Ok(mix)
}

fn add_parts(into: &mut TokenParts, parts: &TokenParts) {
    into.new_input += parts.new_input;
    into.cache_read += parts.cache_read;
    into.cache_write += parts.cache_write;
    into.output += parts.output;
}

/// A request's recorded cost split among its token kinds in proportion to what each costs at `price` (cached input
/// and cache writes at the input price when the model has none of their own). The parts add up to `micro`.
fn split_cost(micro: u64, parts: &TokenParts, price: &Price) -> TokenParts {
    let weights = [
        parts.new_input as u128 * u128::from(price.input_micro_per_mtok),
        parts.cache_read as u128 * u128::from(price.cached_input_micro_per_mtok.unwrap_or(price.input_micro_per_mtok)),
        parts.cache_write as u128 * u128::from(price.cache_write_micro_per_mtok.unwrap_or(price.input_micro_per_mtok)),
        parts.output as u128 * u128::from(price.output_micro_per_mtok),
    ];
    let all: u128 = weights.iter().sum();
    if all == 0 {
        // Priced at $0 per token, yet charged: nothing to split by, so it stays with new input.
        return TokenParts { new_input: micro, ..TokenParts::default() };
    }
    let mut shares = weights.map(|w| (u128::from(micro) * w / all) as u64);
    // What rounding left goes to the largest part, so the parts add up.
    let left = micro - shares.iter().sum::<u64>();
    let largest = (0..4).max_by_key(|i| weights[*i]).unwrap_or(0);
    shares[largest] += left;
    TokenParts { new_input: shares[0], cache_read: shares[1], cache_write: shares[2], output: shares[3] }
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
    let recent = usage_requests(conn, since, 0, recent_limit)?;
    Ok(UsageSummary { since, total, by_model, by_client, recent })
}

/// A page of the requests since `since`, newest first: `limit` of them from `offset` (Usage → Requests, 200 at a time).
pub fn usage_requests(conn: &Connection, since: i64, offset: usize, limit: usize) -> Result<Vec<UsageEvent>> {
    usage_requests_matching(conn, since, &RequestFilter::default(), offset, limit).map(|page| page.rows)
}

/// What Usage → Requests filters by, applied in the query so a filter searches the whole period, not the pages loaded
/// (Codex review 7): a client's name (empty for requests without one), `connection/model`, and `problems` (not
/// answered) or `unknown` (unknown cost).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RequestFilter {
    pub client: Option<String>,
    pub model: Option<String>,
    pub show: Option<String>,
}

/// A page of the requests that match, and how many match in all.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestPage {
    pub rows: Vec<UsageEvent>,
    pub total: u64,
}

const REQUEST_MATCH: &str = "u.created_at >= ?1 AND (?4 IS NULL OR COALESCE(c.name, '') = ?4) \
    AND (?5 IS NULL OR u.provider || '/' || u.model = ?5) \
    AND (?6 IS NULL OR (?6 = 'problems' AND u.outcome <> 'ok') OR (?6 = 'unknown' AND u.cost_status = 'unknown'))";

pub fn usage_requests_matching(
    conn: &Connection, since: i64, filter: &RequestFilter, offset: usize, limit: usize,
) -> Result<RequestPage> {
    let show = filter.show.as_deref().filter(|s| *s == "problems" || *s == "unknown");
    let values = params![since, limit as i64, offset as i64, filter.client, filter.model, show];
    let total: i64 = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM usage_events u LEFT JOIN gateway_clients c ON c.id = u.client_id \
        WHERE {REQUEST_MATCH}"
            ),
            values,
            |r| r.get(0),
        )
        .map_err(err)?;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT u.id, u.created_at, c.name, u.provider, u.model, u.requested_model, u.input_tokens, u.output_tokens, \
             u.cost_micro_usd, u.cost_status, u.source, u.outcome, u.http_status, u.latency_ms, u.error, \
             u.cache_tokens, u.cache_write_tokens, u.session_key, u.route_mode, u.at_risk_micro_usd, u.route_reason, \
             u.request_id, u.attempt, u.billing, u.route_kind, u.cache_hint, u.key_id, k.label, u.cache_miss \
             FROM usage_events u LEFT JOIN gateway_clients c ON c.id = u.client_id LEFT JOIN provider_keys k ON k.id = u.key_id \
             WHERE {REQUEST_MATCH} ORDER BY u.created_at DESC, u.id DESC LIMIT ?2 OFFSET ?3",
        ))
        .map_err(err)?;
    let rows = stmt
        .query_map(values, |r| {
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
                route_reason: r.get(20)?,
                request_id: r.get(21)?,
                attempt: r.get::<_, Option<i64>>(22)?.map(|v| v.max(0) as u32),
                billing: r.get(23)?,
                route_kind: r.get(24)?,
                cache_hint: r.get(25)?,
                key_id: r.get(26)?,
                key_label: r.get(27)?,
                cache_miss: r.get(28)?,
            })
        })
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;
    Ok(RequestPage { rows, total: total.max(0) as u64 })
}

// ---------- Subscriptions and local runtime (Usage tabs, SCREENS.md §5.6) ----------

/// How a ledger row was billed, for the Usage tabs. The row says so itself where its cost source does (`pricing.rs`:
/// "subscription: …" and "local model"); a row that cost money is not either; otherwise the model's billing now, by
/// the same rule as the model list (`billing!`). Rows from an agent's own sign-in (`route_mode = 'agent'`) are on no
/// connection and are left out by the callers.
macro_rules! row_billing {
    () => {
        concat!(
            "CASE WHEN u.source LIKE 'subscription%' THEN 'subscription' WHEN u.source = 'local model' THEN 'local' \
        WHEN COALESCE(u.cost_micro_usd, 0) > 0 THEN 'paid' ELSE ",
            billing!(),
            " END"
        )
    };
}

/// One subscription connection: what went through it in the period, and the limits its provider last stated.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionUsage {
    pub provider_id: String,
    /// The connection's label; the id when the connection was removed since.
    pub label: String,
    pub kind: Option<String>,
    pub requests: u64,
    /// Requests that ended in an error or fell back to another model.
    pub not_ok: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub last_used_at: Option<i64>,
    /// The rate-limit windows from the provider's last answer, per model (`list_limits`). Empty: none stated.
    pub limits: Vec<RateLimits>,
    /// The account's balance or allowance as last read, where the provider has an endpoint for it.
    pub account: Option<super::quota::Account>,
}

/// One model on a local connection: requests, tokens and how long they took.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelUsage {
    pub model: String,
    pub requests: u64,
    pub not_ok: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// The median time taken by its successful requests; `None` when none succeeded.
    pub median_latency_ms: Option<u64>,
    pub last_used_at: Option<i64>,
}

/// One local connection (Ollama, LM Studio, …) and its models.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalUsage {
    pub provider_id: String,
    pub label: String,
    pub kind: Option<String>,
    /// Busiest first; empty when nothing ran on it in the period.
    pub models: Vec<LocalModelUsage>,
}

/// One (connection, model) total of the rows of one billing since `since`, with the latency of each successful row.
struct Tally {
    provider_id: String,
    model: String,
    requests: u64,
    not_ok: u64,
    input_tokens: u64,
    output_tokens: u64,
    last_used_at: i64,
    ok_latencies: Vec<u64>,
}

fn tally(conn: &Connection, billing: &str, since: i64) -> Result<Vec<Tally>> {
    let sql = concat!(
        "SELECT u.provider, u.model, u.outcome, u.input_tokens, u.output_tokens, u.latency_ms, u.created_at \
         FROM usage_events u LEFT JOIN providers p ON p.id = u.provider \
         LEFT JOIN models m ON m.provider_id = u.provider AND m.model_id = u.model \
         WHERE u.created_at >= ?1 AND COALESCE(u.route_mode, '') <> 'agent' AND (",
        row_billing!(),
        ") = ?2 \
         ORDER BY u.provider, u.model"
    );
    let mut stmt = conn.prepare(sql).map_err(err)?;
    let mut rows = stmt.query(params![since, billing]).map_err(err)?;
    let mut out: Vec<Tally> = Vec::new();
    while let Some(r) = rows.next().map_err(err)? {
        let (provider_id, model): (String, String) = (r.get(0).map_err(err)?, r.get(1).map_err(err)?);
        let outcome: String = r.get(2).map_err(err)?;
        let n = |i: usize| r.get::<_, Option<i64>>(i).map(|v| v.unwrap_or(0).max(0) as u64).map_err(err);
        let (input, output) = (n(3)?, n(4)?);
        let latency: Option<i64> = r.get(5).map_err(err)?;
        let at: i64 = r.get(6).map_err(err)?;
        if !out.last().is_some_and(|t| t.provider_id == provider_id && t.model == model) {
            out.push(Tally {
                provider_id,
                model,
                requests: 0,
                not_ok: 0,
                input_tokens: 0,
                output_tokens: 0,
                last_used_at: at,
                ok_latencies: Vec::new(),
            });
        }
        let t = out.last_mut().expect("pushed above");
        t.requests += 1;
        t.input_tokens += input;
        t.output_tokens += output;
        t.last_used_at = t.last_used_at.max(at);
        if outcome == "ok" {
            if let Some(ms) = latency {
                t.ok_latencies.push(ms.max(0) as u64);
            }
        } else {
            t.not_ok += 1;
        }
    }
    Ok(out)
}

/// The middle value; the mean of the two middle ones for an even count.
fn median(mut values: Vec<u64>) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    let mid = values.len() / 2;
    Some(if values.len() % 2 == 1 { values[mid] } else { (values[mid - 1] + values[mid]).div_ceil(2) })
}

/// The connections billed `billing` now (as a connection or for one of its models), with their label and kind, plus
/// those named only by ledger rows (removed since): (id, label, kind).
fn connections_billed(
    conn: &Connection, billing: &str, seen: &[String],
) -> Result<Vec<(String, String, Option<String>)>> {
    let mut stmt = conn
        .prepare(concat!(
            "SELECT p.id, p.account_label, p.kind FROM providers p WHERE p.default_billing = ?1 \
         OR EXISTS (SELECT 1 FROM models m WHERE m.provider_id = p.id AND (",
            billing!(),
            ") = ?1) ORDER BY p.account_label"
        ))
        .map_err(err)?;
    let mut out: Vec<(String, String, Option<String>)> = stmt
        .query_map([billing], |r| Ok((r.get(0)?, r.get(1)?, Some(r.get(2)?))))
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;
    for id in seen {
        if !out.iter().any(|(known, ..)| known == id) {
            let found = get_provider(conn, id)?;
            out.push((
                id.clone(),
                found.as_ref().map_or_else(|| id.clone(), |p| p.label.clone()),
                found.map(|p| p.kind),
            ));
        }
    }
    Ok(out)
}

/// Usage → Subscriptions: each subscription connection's requests and tokens since `since`, and the limits its provider
/// last stated. A subscription is paid for by the month, so no request has a price here.
pub fn subscription_usage(conn: &Connection, since: i64) -> Result<Vec<SubscriptionUsage>> {
    let tallies = tally(conn, "subscription", since)?;
    let seen: Vec<String> = tallies.iter().map(|t| t.provider_id.clone()).collect();
    let mut out = Vec::new();
    for (provider_id, label, kind) in connections_billed(conn, "subscription", &seen)? {
        let mine: Vec<&Tally> = tallies.iter().filter(|t| t.provider_id == provider_id).collect();
        out.push(SubscriptionUsage {
            requests: mine.iter().map(|t| t.requests).sum(),
            not_ok: mine.iter().map(|t| t.not_ok).sum(),
            input_tokens: mine.iter().map(|t| t.input_tokens).sum(),
            output_tokens: mine.iter().map(|t| t.output_tokens).sum(),
            last_used_at: mine.iter().map(|t| t.last_used_at).max(),
            limits: list_limits(conn, &provider_id)?,
            account: super::quota::saved_account(conn, &provider_id),
            provider_id,
            label,
            kind,
        });
    }
    out.sort_by(|a, b| b.requests.cmp(&a.requests).then_with(|| a.label.cmp(&b.label)));
    Ok(out)
}

/// Usage → Local runtime: each local connection's models with their requests, tokens and median time since `since`.
/// The API cost is $0; electricity and the computer's time are not measured.
pub fn local_usage(conn: &Connection, since: i64) -> Result<Vec<LocalUsage>> {
    let tallies = tally(conn, "local", since)?;
    let seen: Vec<String> = tallies.iter().map(|t| t.provider_id.clone()).collect();
    let mut out = Vec::new();
    for (provider_id, label, kind) in connections_billed(conn, "local", &seen)? {
        let mut models: Vec<LocalModelUsage> = tallies
            .iter()
            .filter(|t| t.provider_id == provider_id)
            .map(|t| LocalModelUsage {
                model: t.model.clone(),
                requests: t.requests,
                not_ok: t.not_ok,
                input_tokens: t.input_tokens,
                output_tokens: t.output_tokens,
                median_latency_ms: median(t.ok_latencies.clone()),
                last_used_at: Some(t.last_used_at),
            })
            .collect();
        models.sort_by(|a, b| b.requests.cmp(&a.requests).then_with(|| a.model.cmp(&b.model)));
        out.push(LocalUsage { provider_id, label, kind, models });
    }
    let total = |l: &LocalUsage| l.models.iter().map(|m| m.requests).sum::<u64>();
    out.sort_by(|a, b| total(b).cmp(&total(a)).then_with(|| a.label.cmp(&b.label)));
    Ok(out)
}

// ---------- Settings ----------

pub fn setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0)).optional().map_err(err)
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = ?2",
        params![key, value],
    )
    .map(|_| ())
    .map_err(err)
}

pub fn delete_setting(conn: &Connection, key: &str) -> Result<()> {
    conn.execute("DELETE FROM settings WHERE key = ?1", [key]).map(|_| ()).map_err(err)
}

/// Deletes every setting whose key matches an SQL `LIKE` pattern.
pub fn delete_settings_like(conn: &Connection, pattern: &str) -> Result<()> {
    conn.execute("DELETE FROM settings WHERE key LIKE ?1", [pattern]).map(|_| ()).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn gemini(conn: &Connection) -> Provider {
        add_provider(
            conn,
            &NewProvider {
                kind: "gemini",
                label: "Google Gemini",
                endpoint: "https://example.test/v1/",
                credential_ref: Some("provider-x"),
                default_billing: "unknown",
                wire: "openai",
            },
        )
        .unwrap()
    }

    #[test]
    fn plain_http_only_on_this_computer_and_the_local_network() {
        for ok in [
            "https://api.example.com/v1",
            "http://127.0.0.1:11434/v1",
            "http://localhost:1234/v1",
            "http://[::1]:8000/v1",
            "http://192.168.1.20:8000/v1",
            "http://10.0.0.5/v1",
            "http://172.20.0.3/v1",
            "http://169.254.10.1/v1",
            "http://[fd12::1]/v1",
            "http://ollama:11434/v1",
            "http://gpu-box.local:8000/v1",
            "http://vllm.internal/v1",
            "http://o.test",
        ] {
            assert!(plain_http_allowed(ok), "{ok}");
        }
        for refused in
            ["http://api.example.com/v1", "http://8.8.8.8/v1", "http://[2001:db8::1]/v1", "http://172.32.0.1/v1"]
        {
            assert!(!plain_http_allowed(refused), "{refused}");
        }
        let conn = db::open_in_memory().unwrap();
        let new = NewProvider {
            kind: "custom-openai",
            label: "Remote",
            endpoint: "http://api.example.com/v1",
            credential_ref: Some("provider-x"),
            default_billing: "unknown",
            wire: "openai",
        };
        let error = add_provider(&conn, &new).unwrap_err();
        assert!(error.contains("https://"), "{error}");
    }

    #[test]
    fn connections_get_unique_slugs() {
        let conn = db::open_in_memory().unwrap();
        let a = gemini(&conn);
        let b = gemini(&conn);
        assert_eq!((a.id.as_str(), b.id.as_str()), ("google-gemini", "google-gemini-2"));
        assert_eq!(a.endpoint, "https://example.test/v1");
        assert_eq!(slug("  OpenRouter / Work "), "openrouter-work");
        assert!(add_provider(
            &conn,
            &NewProvider {
                kind: "custom",
                label: "x",
                endpoint: "ftp://x",
                credential_ref: None,
                default_billing: "paid",
                wire: "openai"
            }
        )
        .is_err());
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
        let b = add_provider(
            &conn,
            &NewProvider {
                kind: "openrouter",
                label: "OpenRouter",
                endpoint: "https://o.test",
                credential_ref: None,
                default_billing: "paid",
                wire: "openai",
            },
        )
        .unwrap();
        let members = vec![
            Target { provider_id: a.id.clone(), model_id: "flash".into() },
            Target { provider_id: b.id.clone(), model_id: "vendor/big".into() },
        ];
        save_combo(&conn, None, "coding", &members).unwrap();
        assert!(save_combo(&conn, None, "coding", &members).is_err(), "names are unique");
        assert!(save_combo(&conn, None, "a/b", &members).is_err());

        let routed: Vec<String> =
            resolve(&conn, "coding").unwrap().into_iter().map(|(p, m)| format!("{}/{m}", p.id)).collect();
        assert_eq!(routed, ["google-gemini/flash", "openrouter/vendor/big"]);
        update_provider(&conn, &a.id, Some(false), None, None).unwrap();
        assert_eq!(resolve(&conn, "coding").unwrap().len(), 1);
        assert_eq!(resolve(&conn, "openrouter/vendor/big").unwrap()[0].1, "vendor/big");
        assert!(resolve(&conn, "nothing").unwrap().is_empty());
    }

    #[test]
    fn a_pool_tries_separate_accounts_in_order_and_holds_only_those() {
        let conn = db::open_in_memory().unwrap();
        let add = |label: &str| {
            add_provider(
                &conn,
                &NewProvider {
                    kind: "openrouter",
                    label,
                    endpoint: "http://o.test",
                    credential_ref: None,
                    default_billing: "paid",
                    wire: "openai",
                },
            )
            .unwrap()
        };
        let (a, b) = (add("Work"), add("Personal"));
        let gem = add_provider(
            &conn,
            &NewProvider {
                kind: "gemini",
                label: "G",
                endpoint: "http://g.test",
                credential_ref: None,
                default_billing: "paid",
                wire: "openai",
            },
        )
        .unwrap();
        // Two keys of one account (no separate quota confirmed) cannot be pooled.
        assert!(set_pool(&conn, &a.id, Some("router")).unwrap_err().contains("own provider quota"));
        for p in [&a, &b, &gem] {
            update_provider(&conn, &p.id, None, None, Some(true)).unwrap();
        }
        set_pool(&conn, &a.id, Some("router")).unwrap();
        set_pool(&conn, &b.id, Some("router")).unwrap();
        assert!(
            set_pool(&conn, &gem.id, Some("router")).unwrap_err().contains("openrouter"),
            "one provider kind per pool"
        );
        assert!(set_pool(&conn, &gem.id, Some(&a.id)).unwrap_err().contains("already a connection"));
        assert!(set_pool(&conn, &gem.id, Some("Bad Name")).is_err());

        let ids = |requested: &str| {
            resolve(&conn, requested).unwrap().into_iter().map(|(p, m)| format!("{}:{m}", p.id)).collect::<Vec<_>>()
        };
        assert_eq!(ids("router/x/y"), [format!("{}:x/y", a.id), format!("{}:x/y", b.id)]);
        move_in_pool(&conn, &b.id, true).unwrap();
        assert_eq!(ids("router/x/y"), [format!("{}:x/y", b.id), format!("{}:x/y", a.id)]);
        // A connection's own name still means that connection alone.
        assert_eq!(ids(&format!("{}/m", a.id)), [format!("{}:m", a.id)]);
        update_provider(&conn, &b.id, Some(false), None, None).unwrap();
        assert_eq!(ids("router/m"), [format!("{}:m", a.id)], "a disabled member is skipped");
        set_pool(&conn, &a.id, None).unwrap();
        assert_eq!(pool_names(&conn).unwrap(), ["router".to_string()]);
    }

    #[test]
    fn tiers_from_an_older_table_follow_the_current_one_and_the_users_stay() {
        let conn = db::open_in_memory().unwrap();
        let p = add_provider(
            &conn,
            &NewProvider {
                kind: "anthropic",
                label: "Claude",
                endpoint: "http://a.test",
                credential_ref: None,
                default_billing: "paid",
                wire: "anthropic",
            },
        )
        .unwrap();
        for m in ["claude-haiku-5-5", "claude-haiku-4-5"] {
            add_model(&conn, &p.id, m).unwrap();
        }
        // As the first table left them: both Haiku at T1 (index 1).
        conn.execute("UPDATE models SET tier = 1, tier_source = 'curated:v1'", []).unwrap();
        fill_curated_tiers(&conn).unwrap();
        let tier =
            |m: &str| list_models(&conn, None).unwrap().into_iter().find(|x| x.model_id == m).unwrap().capabilities;
        assert_eq!(
            (tier("claude-haiku-5-5").tier.as_deref(), tier("claude-haiku-5-5").tier_source.as_deref()),
            (Some("T2"), Some(CURATED))
        );
        assert_eq!(tier("claude-haiku-4-5").tier.as_deref(), Some("T1"));
        // A tier the user set stays theirs.
        set_capabilities(
            &conn,
            &p.id,
            "claude-haiku-5-5",
            &Capabilities { tier: Some("T1".into()), ..Capabilities::default() },
        )
        .unwrap();
        fill_curated_tiers(&conn).unwrap();
        assert_eq!(tier("claude-haiku-5-5").tier.as_deref(), Some("T1"));
    }

    #[test]
    fn a_revoked_key_can_be_deleted_and_is_then_not_listed() {
        let conn = db::open_in_memory().unwrap();
        let (c, token) = create_client(&conn, "Old", None).unwrap();
        set_client_token_ref(&conn, &c.id, Some("client-x")).unwrap();
        assert!(list_clients(&conn).unwrap()[0].token_viewable);
        assert_eq!(client_token_ref(&conn, &c.id).unwrap().as_deref(), Some("client-x"));
        assert!(delete_client(&conn, &c.id).unwrap_err().contains("Revoke"));
        revoke_client(&conn, &c.id).unwrap();
        assert_eq!(client_token_ref(&conn, &c.id).unwrap(), None, "a revoked key is not shown");
        assert_eq!(revoked_client_ids(&conn).unwrap(), vec![c.id.clone()]);
        assert_eq!(delete_client(&conn, &c.id).unwrap().as_deref(), Some("client-x"));
        assert!(list_clients(&conn).unwrap().is_empty());
        assert!(revoked_client_ids(&conn).unwrap().is_empty());
        assert!(authenticate(&conn, &token).unwrap().is_none());
    }

    #[test]
    fn new_keys_get_the_defaults_and_old_keys_keep_theirs() {
        let conn = db::open_in_memory().unwrap();
        let (old, _) = create_client(&conn, "Old", None).unwrap();
        assert_eq!(old.auto_min_tier, "T2");
        assert!(set_client_defaults(&conn, &ClientDefaults { paid_retries: Some(9), ..Default::default() }).is_err());
        set_client_defaults(
            &conn,
            &ClientDefaults {
                auto_min_tier: Some("T1".into()),
                monthly_budget_micro_usd: Some(5_000_000),
                max_concurrency: Some(2),
                unknown_cost_policy: Some("allow".into()),
                paid_retries: Some(1),
                require_no_training: Some(true),
                strict_budget: Some(true),
            },
        )
        .unwrap();
        let (new, _) = create_client(&conn, "New", None).unwrap();
        assert_eq!(
            (
                new.auto_min_tier.as_str(),
                new.monthly_budget_micro_usd,
                new.max_concurrency,
                new.unknown_cost_policy.as_str(),
                new.paid_retries,
                new.require_no_training
            ),
            ("T1", Some(5_000_000), 2, "allow", 1, true)
        );
        assert!(new.strict_budget);
        assert!(new.strict_budget);
        assert!(new.strict_budget);
        let (own, _) = create_client(&conn, "Own limit", Some(1_000_000)).unwrap();
        assert_eq!(own.monthly_budget_micro_usd, Some(1_000_000), "a limit given at creation wins");
        let old = list_clients(&conn).unwrap().into_iter().find(|c| c.id == old.id).unwrap();
        assert_eq!((old.auto_min_tier.as_str(), old.max_concurrency), ("T2", 8), "existing keys keep theirs");
    }

    #[test]
    fn latest_requests_are_one_per_client() {
        let conn = db::open_in_memory().unwrap();
        let p = gemini(&conn);
        let cost = Cost::Estimated { micro_usd: 1, source: "x".into() };
        let (a, _) = create_client(&conn, "A", None).unwrap();
        let (b, _) = create_client(&conn, "B", None).unwrap();
        for (client, model, outcome) in [(&a, "m1", "ok"), (&a, "m2", "error"), (&b, "m3", "ok")] {
            record_usage(
                &conn,
                &UsageRecord {
                    client_id: Some(&client.id),
                    provider_id: &p.id,
                    model_id: model,
                    requested_model: "auto",
                    input_tokens: None,
                    output_tokens: None,
                    cache_tokens: None,
                    cost: &cost,
                    outcome,
                    http_status: Some(200),
                    latency_ms: 1,
                    error: None,
                    cache_write_tokens: None,
                    session_key: None,
                    route_mode: None,
                    route_reason: None,
                    facts: None,
                    repeated_tokens: None,
                },
            )
            .unwrap();
            std::thread::sleep(std::time::Duration::from_millis(3));
        }
        let latest = latest_requests(&conn).unwrap();
        assert_eq!(latest.len(), 2);
        let of = |id: &str| latest.iter().find(|r| r.client_id == id).unwrap();
        assert_eq!((of(&a.id).model.as_str(), of(&a.id).outcome.as_str()), ("m2", "error"));
        assert_eq!(of(&b.id).model, "m3");
    }

    #[test]
    fn reliability_counts_what_reached_each_model() {
        let conn = db::open_in_memory().unwrap();
        let p = gemini(&conn);
        let cost = Cost::Estimated { micro_usd: 1, source: "x".into() };
        for (model, outcome, ms) in
            [("a", "ok", 100), ("a", "ok", 300), ("a", "error", 50), ("a", "client_disconnected", 9), ("b", "ok", 700)]
        {
            record_usage(
                &conn,
                &UsageRecord {
                    client_id: None,
                    provider_id: &p.id,
                    model_id: model,
                    requested_model: model,
                    input_tokens: None,
                    output_tokens: None,
                    cache_tokens: None,
                    cost: &cost,
                    outcome,
                    http_status: Some(200),
                    latency_ms: ms,
                    error: None,
                    cache_write_tokens: None,
                    session_key: None,
                    route_mode: None,
                    route_reason: None,
                    facts: None,
                    repeated_tokens: None,
                },
            )
            .unwrap();
        }
        let r = reliability(&conn, &p.id, 0).unwrap();
        assert_eq!((r[0].model_id.as_str(), r[0].requests, r[0].ok, r[0].median_latency_ms), ("a", 3, 2, Some(300)));
        assert_eq!((r[1].requests, r[1].ok, r[1].median_latency_ms), (1, 1, Some(700)));
    }

    #[test]
    fn alerts_say_what_the_ledger_shows_and_nothing_else() {
        let conn = db::open_in_memory().unwrap();
        let p = gemini(&conn);
        let (client, _) = create_client(&conn, "Claude Code", Some(1_000_000)).unwrap();
        let now = now_ms();
        let add = |micro: Option<u64>, outcome: &str, ago_ms: i64| {
            let cost = match micro {
                Some(m) => Cost::Estimated { micro_usd: m, source: "price: user".into() },
                None => Cost::Unknown { source: "no price".into() },
            };
            record_usage(
                &conn,
                &UsageRecord {
                    client_id: Some(&client.id),
                    provider_id: &p.id,
                    model_id: "flash",
                    requested_model: "auto",
                    input_tokens: Some(10),
                    output_tokens: Some(5),
                    cache_tokens: None,
                    cost: &cost,
                    outcome,
                    http_status: Some(200),
                    latency_ms: 12,
                    error: None,
                    cache_write_tokens: None,
                    session_key: None,
                    route_mode: None,
                    route_reason: None,
                    facts: None,
                    repeated_tokens: None,
                },
            )
            .unwrap();
            conn.execute("UPDATE usage_events SET created_at = ?1 WHERE id = (SELECT id FROM usage_events ORDER BY rowid DESC LIMIT 1)",
                [now - ago_ms]).unwrap();
        };
        assert!(usage_alerts(&conn, now).unwrap().is_empty(), "nothing recorded, nothing to say");

        // The week before: $0.10 a day. Today: $0.85, more than three times the average and above the floor.
        for d in 2..=8 {
            add(Some(100_000), "ok", d * 86_400_000 - 3_600_000);
        }
        add(Some(850_000), "ok", 60_000);
        let kinds =
            |conn: &Connection| usage_alerts(conn, now).unwrap().into_iter().map(|a| a.kind).collect::<Vec<_>>();
        let alerts = usage_alerts(&conn, now).unwrap();
        let spike = alerts.iter().find(|a| a.kind == "spend_spike").unwrap();
        assert_eq!((spike.value, spike.against), (850_000, 100_000));
        // $1.55 this month against a $1 limit (the week's rows may fall in last month; today's alone is $0.85, 85%).
        assert!(kinds(&conn).iter().any(|k| k == "near_limit" || k == "over_limit"));

        for _ in 0..5 {
            add(None, "ok", 120_000);
        }
        assert!(kinds(&conn).contains(&"unknown_costs".to_string()));
        for _ in 0..6 {
            add(Some(0), "error", 30_000);
        }
        let failures = usage_alerts(&conn, now).unwrap().into_iter().find(|a| a.kind == "failures").unwrap();
        assert_eq!((failures.value, failures.against), (6, 12), "6 of the last hour's 12 requests failed");
    }

    #[test]
    fn routing_shows_fallbacks_sessions_and_changes_of_model() {
        let conn = db::open_in_memory().unwrap();
        let p = gemini(&conn);
        let record = |request: &str,
                      attempt: u32,
                      model: &str,
                      outcome: &str,
                      status: u16,
                      session: Option<&str>,
                      written: u64| {
            let facts = AttemptFacts {
                request_id: request.into(),
                attempt,
                billing: "paid".into(),
                route_kind: "auto".into(),
                cache_hint: "none".into(),
                key_id: None,
                prefix: None,
                clearing: None,
                hint: None,
            };
            record_usage(
                &conn,
                &UsageRecord {
                    client_id: None,
                    provider_id: &p.id,
                    model_id: model,
                    requested_model: "auto",
                    input_tokens: Some(100),
                    output_tokens: Some(10),
                    cache_tokens: Some(0),
                    cost: &Cost::Estimated { micro_usd: 7, source: "x".into() },
                    outcome,
                    http_status: Some(status),
                    latency_ms: 1,
                    error: None,
                    cache_write_tokens: Some(written),
                    session_key: session,
                    route_mode: None,
                    route_reason: None,
                    repeated_tokens: None,
                    facts: Some(&facts),
                },
            )
            .unwrap();
        };
        record("a", 1, "flash", "ok", 200, Some("launch-1"), 0);
        record("b", 1, "flash", "fallback", 429, Some("launch-1"), 0);
        record("b", 2, "pro", "ok", 200, Some("launch-1"), 500);
        record("c", 1, "flash", "ok", 200, None, 0);
        let r = routing_summary(&conn, 0).unwrap();
        assert_eq!((r.requests, r.attempts, r.answered, r.answered_after_fallback), (3, 4, 3, 1));
        assert_eq!((r.with_session, r.sessions), (3, 1));
        assert_eq!(r.changes.len(), 1);
        let c = &r.changes[0];
        assert_eq!(
            (c.from.clone(), c.to.clone(), c.reason.as_str(), c.cache_writes_after),
            (format!("{}/flash", p.id), format!("{}/pro", p.id), "HTTP 429", 500)
        );
        assert_eq!(c.known_micro_usd, Some(7));
    }

    /// ROADMAP item 4: each session's tools and system prompt at its first answer, the changes, and the later answers that
    /// read nothing from a cache the provider reports; failures and requests without a session are not counted.
    #[test]
    fn the_prefix_meter_counts_each_sessions_tools_and_system_prompt_and_its_uncached_resends() {
        use super::super::shadow::Prefix;
        let conn = db::open_in_memory().unwrap();
        let p = gemini(&conn);
        let (client, _) = create_client(&conn, "Codex", None).unwrap();
        let a = Prefix { tools_tokens: 900, system_tokens: 100, hash: 1 };
        let b = Prefix { tools_tokens: 1200, system_tokens: 100, hash: 2 };
        let record = |session: Option<&str>, prefix: Prefix, read: Option<u64>, outcome: &str| {
            let facts = AttemptFacts { cache_hint: "none".into(), prefix: Some(prefix), ..AttemptFacts::default() };
            record_usage(
                &conn,
                &UsageRecord {
                    client_id: Some(&client.id),
                    provider_id: &p.id,
                    model_id: "m",
                    requested_model: "auto",
                    input_tokens: Some(4000),
                    output_tokens: Some(10),
                    cache_tokens: read,
                    cost: &Cost::Estimated { micro_usd: 7, source: "x".into() },
                    outcome,
                    http_status: Some(200),
                    latency_ms: 1,
                    error: None,
                    cache_write_tokens: None,
                    session_key: session,
                    route_mode: None,
                    route_reason: None,
                    repeated_tokens: None,
                    facts: Some(&facts),
                },
            )
            .unwrap();
        };
        record(Some("s1"), a, Some(0), "ok"); // the session's first request: not a resend
        record(Some("s1"), a, Some(3000), "ok"); // read from the cache
        record(Some("s1"), b, Some(0), "ok"); // the tools changed, so the whole prefix went again
        record(Some("s1"), b, None, "ok"); // the provider did not say: not counted as a resend
        record(Some("s1"), a, None, "error"); // a failure: not counted at all
        record(Some("s2"), a, Some(0), "ok");
        record(None, b, Some(0), "ok"); // no session

        let meter = prefix_meter(&conn, 0).unwrap();
        assert_eq!((meter.sessions, meter.requests), (2, 5));
        assert_eq!((meter.first_tools_tokens, meter.first_system_tokens), (1800, 200));
        assert_eq!((meter.changes, meter.resent, meter.resent_tokens), (1, 1, 1300));
        let first = &meter.top[0];
        assert_eq!((first.client.as_str(), first.session.as_str()), ("Codex", "s1"));
        assert_eq!((first.requests, first.changes, first.resent, first.resent_tokens), (4, 1, 1, 1300));
        assert_eq!(token_mix(&conn, 0).unwrap().prefix, meter);
    }

    /// ROADMAP item 4, shadow: what clearing old tool results would have saved, less the cache writes it causes when a
    /// request clears more than the one before it in its session.
    #[test]
    fn the_clearing_shadow_weighs_what_it_saves_against_the_cache_writes_it_causes() {
        use super::super::shadow::Clearing;
        let conn = db::open_in_memory().unwrap();
        let p = gemini(&conn);
        let (client, _) = create_client(&conn, "Codex", None).unwrap();
        add_model(&conn, &p.id, "m").unwrap();
        let price = Price {
            input_micro_per_mtok: 1_000_000,
            output_micro_per_mtok: 5_000_000,
            cached_input_micro_per_mtok: Some(100_000),
            cache_write_micro_per_mtok: Some(1_250_000),
            source: "user".into(),
            observed_at: 0,
        };
        update_model(&conn, &p.id, "m", None, None, Some(Some(price))).unwrap();
        let record = |model: &str, session: Option<&str>, clearing: Clearing, read: u64| {
            let facts = AttemptFacts { cache_hint: "none".into(), clearing: Some(clearing), ..AttemptFacts::default() };
            record_usage(
                &conn,
                &UsageRecord {
                    client_id: Some(&client.id),
                    provider_id: &p.id,
                    model_id: model,
                    requested_model: "auto",
                    input_tokens: Some(150_000),
                    output_tokens: Some(10),
                    cache_tokens: Some(read),
                    cost: &Cost::Estimated { micro_usd: 7, source: "x".into() },
                    outcome: "ok",
                    http_status: Some(200),
                    latency_ms: 1,
                    error: None,
                    cache_write_tokens: None,
                    session_key: session,
                    route_mode: None,
                    route_reason: None,
                    repeated_tokens: None,
                    facts: Some(&facts),
                },
            )
            .unwrap();
        };
        let clear = |count, tokens, tail_tokens| Clearing { count, tokens, tail_tokens };
        record("m", Some("s1"), Clearing::default(), 0); // under the trigger
        record("m", Some("s1"), clear(9, 90_000, 40_000), 50_000); // clears anew: 9,000 saved, 46,000 more in writes
        record("m", Some("s1"), clear(9, 90_000, 40_000), 120_000); // the same nine: 9,000 saved, nothing rewritten
        record("m", Some("s1"), clear(10, 100_000, 30_000), 100_000); // one more: 10,000 saved, 34,500 more
        record("m", None, clear(9, 90_000, 40_000), 0); // no cache read: 90,000 saved at the input price, no extra
        record("free-x", Some("s2"), clear(1, 10_000, 5_000), 0); // no price: counted in tokens only

        let shadow = clearing_shadow(&conn, 0).unwrap();
        assert_eq!((shadow.requests, shadow.over_trigger, shadow.unpriced), (6, 5, 1));
        assert_eq!((shadow.cleared_tokens, shadow.rewritten_tokens), (380_000, 115_000));
        assert_eq!((shadow.saved_micro_usd, shadow.extra_micro_usd, shadow.net_micro_usd), (118_000, 80_500, 37_500));
    }

    #[test]
    fn a_longer_cache_lifetime_is_weighed_from_the_pauses_in_each_session() {
        let conn = db::open_in_memory().unwrap();
        let p = gemini(&conn);
        let (client, _) = create_client(&conn, "Codex", None).unwrap();
        for m in ["haiku", "gpt"] {
            add_model(&conn, &p.id, m).unwrap();
        }
        let price = Price {
            input_micro_per_mtok: 1_000_000,
            output_micro_per_mtok: 5_000_000,
            cached_input_micro_per_mtok: Some(100_000),
            cache_write_micro_per_mtok: Some(1_250_000),
            source: "user".into(),
            observed_at: 0,
        };
        update_model(&conn, &p.id, "haiku", None, None, Some(Some(price))).unwrap();
        let min = 60_000;
        let record = |model: &str, session: &str, at: i64, prompt: u64, read: u64, write: u64, miss: &str| {
            let facts = AttemptFacts { cache_hint: "added".into(), ..AttemptFacts::default() };
            record_usage(
                &conn,
                &UsageRecord {
                    client_id: Some(&client.id),
                    provider_id: &p.id,
                    model_id: model,
                    requested_model: "auto",
                    input_tokens: Some(prompt),
                    output_tokens: Some(10),
                    cache_tokens: Some(read),
                    cost: &Cost::Estimated { micro_usd: 7, source: "x".into() },
                    outcome: "ok",
                    http_status: Some(200),
                    latency_ms: 1,
                    error: None,
                    cache_write_tokens: Some(write),
                    session_key: Some(session),
                    route_mode: None,
                    route_reason: None,
                    repeated_tokens: None,
                    facts: Some(&facts),
                },
            )
            .unwrap();
            conn.execute(
                "UPDATE usage_events SET created_at = ?1, cache_miss = NULLIF(?2, '') WHERE rowid = last_insert_rowid()",
                params![at, miss],
            )
            .unwrap();
        };
        let t0 = 1_000_000_000;
        record("haiku", "s1", t0, 10_000, 0, 10_000, "first_request");
        record("haiku", "s1", t0 + 2 * min, 11_000, 10_000, 1_000, ""); // a hit after two minutes
        record("haiku", "s1", t0 + 22 * min, 12_000, 0, 12_000, "cache_expired"); // twenty minutes: 11,000 recoverable
        record("haiku", "s1", t0 + 142 * min, 13_000, 0, 13_000, "cache_expired"); // two hours: not recoverable
        record("gpt", "s2", t0, 50_000, 0, 0, "first_request"); // never writes to a cache: not measured
        record("gpt", "s2", t0 + 30 * min, 50_000, 0, 0, "cache_expired");

        let shadow = cache_lifetime(&conn, 0, t0 + 145 * min).unwrap();
        assert_eq!((shadow.sessions, shadow.requests, shadow.unpriced), (1, 4, 0));
        assert_eq!((shadow.idle_gaps, shadow.expired_misses, shadow.recovered_tokens), (1, 1, 11_000));
        // Reading 11,000 tokens instead of writing them: 11,000 × ($1.25 − $0.10) per million.
        assert_eq!(shadow.saved_micro_usd, 12_650);
        // One hour: the other 25,000 written tokens at $2.00 instead of $1.25.
        assert_eq!((shadow.hour_extra_micro_usd, shadow.hour_net_micro_usd), (18_750, -6_100));
        // Keep-alive: 4 re-sends of 11,000 in the twenty minutes, 13 of 12,000 in the hour after; none yet after the last.
        assert_eq!(shadow.keep_alive_requests, 17);
        assert_eq!((shadow.keep_alive_extra_micro_usd, shadow.keep_alive_net_micro_usd), (20_000, -7_350));
    }

    #[test]
    fn the_token_mix_splits_known_cost_and_keeps_the_rest_apart() {
        let conn = db::open_in_memory().unwrap();
        let p = gemini(&conn);
        add_model(&conn, &p.id, "flash").unwrap();
        add_model(&conn, &p.id, "pro").unwrap();
        let price = Price {
            input_micro_per_mtok: 1_000_000,
            output_micro_per_mtok: 5_000_000,
            cached_input_micro_per_mtok: Some(100_000),
            cache_write_micro_per_mtok: Some(1_250_000),
            source: "user".into(),
            observed_at: 0,
        };
        update_model(&conn, &p.id, "flash", None, None, Some(Some(price))).unwrap();
        let record = |model: &str, input: Option<u64>, read: u64, write: u64, output: u64, cost: Cost| {
            record_usage(
                &conn,
                &UsageRecord {
                    client_id: None,
                    provider_id: &p.id,
                    model_id: model,
                    requested_model: "auto",
                    input_tokens: input,
                    output_tokens: input.map(|_| output),
                    cache_tokens: Some(read),
                    cost: &cost,
                    outcome: "ok",
                    http_status: Some(200),
                    latency_ms: 1,
                    error: None,
                    cache_write_tokens: Some(write),
                    session_key: None,
                    route_mode: None,
                    route_reason: None,
                    facts: None,
                    repeated_tokens: None,
                },
            )
            .unwrap();
        };
        let est = |micro| Cost::Estimated { micro_usd: micro, source: "price: user".into() };
        // A cached prompt: 100 new, 800 read, 100 written, 100 out, $0.001 recorded.
        record("flash", Some(1000), 800, 100, 100, est(1000));
        // A model with no price now: its cost cannot be split.
        record("pro", Some(50), 0, 0, 10, est(300));
        // Unknown cost: its tokens count, its money does not. A request without token counts is not in the mix.
        record("flash", Some(20), 0, 0, 5, Cost::Unknown { source: "no price".into() });
        record("flash", None, 0, 0, 0, Cost::Unknown { source: "failed".into() });

        let mix = token_mix(&conn, 0).unwrap();
        assert_eq!((mix.requests, mix.with_tokens, mix.unknown_cost_requests), (4, 3, 1));
        assert_eq!(mix.tokens, TokenParts { new_input: 170, cache_read: 800, cache_write: 100, output: 115 });
        assert_eq!(mix.unsplit_micro_usd, 300);
        // Split by the prices' weights (output 5×, cache writes 1.25×, reads 0.1×); the parts add up to what was recorded.
        assert_eq!(mix.cost, TokenParts { new_input: 124, cache_read: 99, cache_write: 155, output: 622 });
        assert_eq!(mix.by_model[0].key, format!("{}/flash", p.id));
        assert_eq!((mix.by_model[0].requests, mix.by_model[0].known_micro_usd), (2, 1000));
        assert_eq!(mix.by_client.len(), 1, "no client is one group");
    }

    #[test]
    fn usage_keeps_unknown_apart_from_known() {
        let conn = db::open_in_memory().unwrap();
        let p = gemini(&conn);
        let (client, _) = create_client(&conn, "Codex", None).unwrap();
        let known = Cost::Estimated { micro_usd: 1500, source: "price: user".into() };
        let unknown = Cost::Unknown { source: "no price".into() };
        for (cost, outcome) in [(&known, "ok"), (&unknown, "ok"), (&known, "fallback")] {
            record_usage(
                &conn,
                &UsageRecord {
                    client_id: Some(&client.id),
                    provider_id: &p.id,
                    model_id: "flash",
                    requested_model: "coding",
                    input_tokens: Some(10),
                    output_tokens: Some(5),
                    cache_tokens: None,
                    cost,
                    outcome,
                    http_status: Some(200),
                    latency_ms: 12,
                    error: None,
                    cache_write_tokens: None,
                    session_key: None,
                    route_mode: None,
                    route_reason: None,
                    facts: None,
                    repeated_tokens: None,
                },
            )
            .unwrap();
        }
        let csv = usage_csv(&conn, 0).unwrap();
        let lines: Vec<&str> = csv.trim_end().split("\r\n").collect();
        assert!(lines[0].starts_with("time_utc,client,"));
        assert_eq!(lines.len() as u64, 1 + usage_summary(&conn, 0, 10).unwrap().total.requests);
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso_utc(1_794_469_641_058), "2026-11-12T07:47:21.058Z");
        assert_eq!(csv_field("=HYPERLINK(\"x\")"), "\"'=HYPERLINK(\"\"x\"\")\"");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        let buckets = usage_over_time(&conn, 0, 86_400_000, 7 * 3_600_000).unwrap();
        assert_eq!(
            buckets.iter().map(|b| b.requests).sum::<u64>(),
            usage_summary(&conn, 0, 10).unwrap().total.requests
        );
        assert!(buckets.iter().all(|b| (b.start + 7 * 3_600_000) % 86_400_000 == 0), "periods start at local midnight");
        let summary = usage_summary(&conn, 0, 10).unwrap();
        assert_eq!((summary.total.requests, summary.total.failed, summary.total.fallbacks), (3, 0, 1));
        assert_eq!((summary.total.known_micro_usd, summary.total.unknown_requests), (3000, 1));
        assert_eq!(summary.by_client[0].key, "Codex");
        assert_eq!(summary.recent.len(), 3);
        // The chart's periods carry tokens; the request list pages newest first without a gap or a repeat.
        assert_eq!(
            buckets.iter().map(|b| (b.input_tokens, b.output_tokens)).fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1)),
            (30, 15)
        );
        let page = |offset, limit| {
            usage_requests(&conn, 0, offset, limit).unwrap().into_iter().map(|e| e.id).collect::<Vec<_>>()
        };
        let all = page(0, 10);
        assert_eq!(all.len(), 3);
        assert_eq!([page(0, 2), page(2, 2)].concat(), all);
        assert!(page(3, 2).is_empty());
        // Filters search the whole period in the query, and say how many match.
        let matching = |client: Option<&str>, model: Option<&str>, show: Option<&str>, limit| {
            usage_requests_matching(
                &conn,
                0,
                &RequestFilter {
                    client: client.map(String::from),
                    model: model.map(String::from),
                    show: show.map(String::from),
                },
                0,
                limit,
            )
            .unwrap()
        };
        assert_eq!(matching(None, None, Some("problems"), 10).total, 1);
        assert_eq!(matching(None, None, Some("unknown"), 10).total, 1);
        let codex = matching(Some("Codex"), None, None, 1);
        assert_eq!((codex.total, codex.rows.len()), (3, 1), "the count is of every match, the page of one");
        assert_eq!(matching(Some(""), None, None, 10).total, 0, "no request without a client");
        assert_eq!(matching(None, Some(&format!("{}/flash", p.id)), None, 10).total, 3);
        let client = list_clients(&conn).unwrap().remove(0);
        assert_eq!((client.month_spend_micro_usd, client.month_unknown_requests), (3000, 1));
        // The health strip: the fallback counts against the connection, the unknown-cost answer is an answer.
        let strip = health_strip(&conn, &p.id, now_ms() + 1).unwrap();
        assert_eq!(strip.buckets.len(), STRIP_HOURS as usize);
        assert_eq!(strip.buckets.last().map(|b| (b.ok, b.failed)), Some((2, 1)));
        assert_eq!(strip.score, Some(67));
        record_check(&conn, &p.id, true, "models", "Reachable · 1 models listed").unwrap();
        record_usage(
            &conn,
            &UsageRecord {
                client_id: Some(&client.id),
                provider_id: &p.id,
                model_id: "flash",
                requested_model: "coding",
                input_tokens: None,
                output_tokens: None,
                cache_tokens: None,
                cost: &Cost::Estimated { micro_usd: 0, source: "not sent".into() },
                outcome: "error",
                http_status: Some(429),
                latency_ms: 0,
                error: Some("Not sent: the web guard"),
                cache_write_tokens: None,
                session_key: None,
                route_mode: None,
                route_reason: None,
                facts: None,
                repeated_tokens: None,
            },
        )
        .unwrap();
        // A check in an hour with requests adds nothing to them, and a request Pygg refused itself does not count.
        let at = now_ms() + 1;
        let strip = health_strip(&conn, &p.id, at).unwrap();
        assert_eq!(strip.buckets.last().map(|b| (b.ok, b.failed)), Some((2, 1)));
        assert_eq!(strip.score, Some(67));
        assert!(last_check(&conn, &p.id).unwrap().is_some_and(|c| c.ok));
        // An hour without requests is judged by its checks, as one hour however many there were: a failed check two
        // hours back gives that hour 0, weighed a little under the newest hour. A check from the future does not count.
        for (offset, ok) in [(-9_000_000i64, 0), (-8_000_000, 0), (1_800_000, 0)] {
            conn.execute("INSERT INTO connection_checks (provider_id, at, ok, kind, detail) VALUES (?1, ?2, ?3, 'models', 'down')",
                params![p.id, at + offset, ok]).unwrap();
        }
        let strip = health_strip(&conn, &p.id, at).unwrap();
        assert_eq!(strip.buckets[STRIP_HOURS as usize - 3].failed, 2);
        assert_eq!(strip.buckets.last().map(|b| (b.ok, b.failed)), Some((2, 1)));
        assert_eq!(strip.score, Some(34));
        assert!(checks_due(&conn, 60_000, now_ms()).unwrap().iter().all(|id| id != &p.id), "just checked");
        assert!(health_strip(&conn, "nobody", now_ms()).unwrap().score.is_none());
    }

    #[test]
    fn subscription_and_local_requests_are_counted_apart_from_money() {
        use super::super::pricing::{self, Usage};
        let conn = db::open_in_memory().unwrap();
        let new = |kind, label, billing| {
            add_provider(
                &conn,
                &NewProvider {
                    kind,
                    label,
                    endpoint: "https://example.test/v1",
                    credential_ref: None,
                    default_billing: billing,
                    wire: "openai",
                },
            )
            .unwrap()
        };
        let plan = new("qwen", "Qwen token plan", "subscription");
        let ollama = new("ollama", "Ollama", "local");
        let paid = new("openrouter", "OpenRouter", "paid");
        let _idle_plan = new("zai", "GLM plan", "subscription");
        let usage = Usage { prompt: 100, completion: 20, ..Default::default() };
        let record =
            |provider: &str, model: &str, cost: &Cost, outcome: &str, latency_ms: u64, route_mode: Option<&str>| {
                record_usage(
                    &conn,
                    &UsageRecord {
                        client_id: None,
                        provider_id: provider,
                        model_id: model,
                        requested_model: model,
                        input_tokens: Some(100),
                        output_tokens: Some(20),
                        cache_tokens: None,
                        cost,
                        outcome,
                        http_status: Some(200),
                        latency_ms,
                        error: None,
                        cache_write_tokens: None,
                        session_key: None,
                        route_mode,
                        route_reason: None,
                        facts: None,
                        repeated_tokens: None,
                    },
                )
                .unwrap();
            };
        let quota = pricing::resolve("subscription", None, Some(&usage));
        let local = pricing::resolve("local", None, Some(&usage));
        record(&plan.id, "qwen3-coder-plus", &quota, "ok", 900, None);
        record(&plan.id, "qwen3-coder-plus", &quota, "ok", 1100, None);
        // A failure before the provider answered says nothing about billing; the connection's billing decides.
        record(
            &plan.id,
            "qwen3-coder-plus",
            &Cost::Estimated { micro_usd: 0, source: "could not connect".into() },
            "error",
            5,
            None,
        );
        for ms in [300, 100, 200, 10_000] {
            record(&ollama.id, "qwen2.5-coder:7b", &local, if ms == 10_000 { "error" } else { "ok" }, ms, None);
        }
        record(&ollama.id, "llama3.2:3b", &local, "ok", 40, None);
        record(
            &paid.id,
            "qwen/qwen3-coder",
            &Cost::Actual { micro_usd: 900, source: "provider response".into() },
            "ok",
            700,
            None,
        );
        // An agent's own sign-in is on no connection.
        record(
            "agent:codex",
            "gpt-5",
            &Cost::Unknown { source: "subscription: agent".into() },
            "ok",
            60_000,
            Some("agent"),
        );
        // Left out: before the period.
        conn.execute("UPDATE usage_events SET created_at = 5 WHERE model = 'llama3.2:3b'", []).unwrap();
        record_limits(
            &conn,
            &plan.id,
            "qwen3-coder-plus",
            200,
            &[
                ("x-ratelimit-limit-requests".into(), "2000".into()),
                ("x-ratelimit-remaining-requests".into(), "1997".into()),
            ],
        )
        .unwrap();

        let subs = subscription_usage(&conn, 1_000).unwrap();
        let ids: Vec<&str> = subs.iter().map(|s| s.provider_id.as_str()).collect();
        assert_eq!(ids, [plan.id.as_str(), "glm-plan"], "busiest first; a plan with no requests still shows");
        let s = &subs[0];
        assert_eq!(
            (s.label.as_str(), s.requests, s.not_ok, s.input_tokens, s.output_tokens),
            ("Qwen token plan", 3, 1, 300, 60)
        );
        assert_eq!(s.limits[0].windows[0].limit, Some(2000));
        assert!(subs[1].limits.is_empty() && subs[1].requests == 0 && subs[1].last_used_at.is_none());

        let locals = local_usage(&conn, 1_000).unwrap();
        assert_eq!(locals.len(), 1);
        assert_eq!(locals[0].label, "Ollama");
        assert_eq!(
            locals[0].models,
            [LocalModelUsage {
                model: "qwen2.5-coder:7b".into(),
                requests: 4,
                not_ok: 1,
                input_tokens: 400,
                output_tokens: 80,
                median_latency_ms: Some(200),
                last_used_at: locals[0].models[0].last_used_at
            }],
            "the median is of successful requests only, and an old request is outside the period"
        );
        assert_eq!(local_usage(&conn, 0).unwrap()[0].models.len(), 2);
        assert_eq!(median(vec![100, 300]), Some(200));
        assert_eq!(median(Vec::new()), None);
    }

    #[test]
    fn month_start_is_the_first_at_midnight_utc() {
        // 2026-11-11T15:30:00Z → 2026-11-01T00:00:00Z; 2024-03-01 stays itself.
        assert_eq!(month_start(1_794_411_000_000), 1_793_491_200_000);
        assert_eq!(month_start(1_709_251_200_000), 1_709_251_200_000);
    }
}
