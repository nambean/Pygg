//! Key rings (ADR-038): several API keys on one connection, each from a separate account with its own quota, used in
//! turn. The key a connection was made with is the first of its ring; each key added later is confirmed by the user as
//! another account's, as a pool member is (ADR-034), since several keys of one account share its limits and rotating
//! them would only get around them. Sign-ins are never rotated: web sessions and connections without keys have no ring
//! (ADR-026, ADR-035).
//!
//! A ring is active once it holds two keys. For each request the gateway tries the connection's enabled keys in the
//! order of its strategy (`order`): `in_order` (the user's order), `round_robin` (the next key first, request by
//! request), `least_used` (fewest requests today) or `quota_aware` (most room left by the limits each key's account
//! last stated, a key whose stated limit is used up last). A session stays on the key that answered it, for the
//! provider's prompt cache. A 401 blocks a key, a 402 or a 429 cools it (`policy.rs` decides; `KEY_STATES`), and the
//! next key is tried; the ledger names the key that answered (`usage_events.key_id`). Secrets stay in the system's
//! credential store: a row holds only its reference, and nothing here reads, logs or returns a secret.

use super::presets;
use super::quota::{self, Window};
use super::store::{self, Provider, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

/// How a connection orders its keys for a new request.
pub const STRATEGIES: [&str; 4] = ["in_order", "round_robin", "least_used", "quota_aware"];
/// At most this many keys on one connection.
pub const MAX_KEYS: usize = 20;
/// The failures that belong to one key's account, not to the connection: a rejected key, no credit, a rate limit. They
/// mark the key (`mark`), and the connection's other keys go on.
pub const KEY_STATES: [&str; 3] = ["invalid_credentials", "payment_required", "rate_limited"];

pub const SEPARATE_ACCOUNT: &str = "Confirm that this key belongs to another account, with its own quota. Several keys of \
    one account share its limits, so Pygg does not rotate them.";
pub const NO_SIGN_INS: &str = "Only API-key connections have several keys. Sign-ins (web sessions, subscriptions) are never \
    rotated.";

fn err(e: rusqlite::Error) -> String {
    e.to_string()
}

/// One key of a ring, without its secret.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    pub id: String,
    pub provider_id: String,
    pub label: String,
    #[serde(skip)]
    pub credential_ref: String,
    pub rank: i64,
    pub enabled: bool,
    /// When the user confirmed it is another account's; `None` for the connection's first key.
    pub separate_account_at: Option<i64>,
    pub created_at: i64,
}

/// A key's cooldown or block.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyHealth {
    pub key_id: String,
    /// Empty for the whole key.
    pub model_id: String,
    pub state: String,
    pub reason: String,
    pub http_status: Option<u16>,
    /// `None`: until the user fixes it (a rejected key).
    pub until_ms: Option<i64>,
    pub updated_at: i64,
}

const KEY_COLUMNS: &str = "id, provider_id, label, credential_ref, rank, enabled, separate_account_at, created_at";

fn key_of(r: &rusqlite::Row) -> rusqlite::Result<Key> {
    Ok(Key { id: r.get(0)?, provider_id: r.get(1)?, label: r.get(2)?, credential_ref: r.get(3)?, rank: r.get(4)?,
        enabled: r.get::<_, i64>(5)? == 1, separate_account_at: r.get(6)?, created_at: r.get(7)? })
}

/// A connection's keys, in the user's order. Empty when it has never had a second one.
pub fn list(conn: &Connection, provider_id: &str) -> Result<Vec<Key>> {
    let mut stmt = conn.prepare(&format!("SELECT {KEY_COLUMNS} FROM provider_keys WHERE provider_id = ?1 ORDER BY rank, created_at"))
        .map_err(err)?;
    let rows = stmt.query_map([provider_id], key_of).map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

pub fn get(conn: &Connection, key_id: &str) -> Result<Option<Key>> {
    conn.query_row(&format!("SELECT {KEY_COLUMNS} FROM provider_keys WHERE id = ?1"), [key_id], key_of).optional().map_err(err)
}

/// Whether this connection may have several keys: an API-key connection with a key, never a web session.
pub fn can_have_keys(provider: &Provider) -> std::result::Result<(), String> {
    // Web sessions are sign-ins; a local runtime (Ollama, LM Studio) has no account to rotate.
    if presets::find(&provider.kind).is_some_and(|p| p.web || p.offer == "local") {
        return Err(NO_SIGN_INS.into());
    }
    if provider.credential_ref.is_none() {
        return Err("This connection has no key yet: set its key first, then add others.".into());
    }
    Ok(())
}

/// Every credential reference a connection holds (its own and its ring's), to delete with it.
pub fn refs(conn: &Connection, provider_id: &str) -> Result<Vec<String>> {
    Ok(list(conn, provider_id)?.into_iter().map(|k| k.credential_ref).collect())
}

/// Adds a key, kept under `credential_ref`, after the user confirmed it is another account's (`separate_account`).
/// The connection's own key becomes the ring's first key the first time.
pub fn add(conn: &Connection, provider_id: &str, label: &str, credential_ref: &str, separate_account: bool, now: i64)
    -> Result<Key> {
    let provider = store::get_provider(conn, provider_id)?.ok_or("No such connection")?;
    can_have_keys(&provider)?;
    if !separate_account {
        return Err(SEPARATE_ACCOUNT.into());
    }
    let label = label.trim();
    if label.is_empty() || label.chars().count() > 60 {
        return Err("Give the key a name of 1 to 60 characters, such as the account it belongs to.".into());
    }
    let mut keys = list(conn, provider_id)?;
    if keys.is_empty() {
        let first = Key { id: uuid::Uuid::new_v4().simple().to_string(), provider_id: provider_id.into(), label: "First key".into(),
            credential_ref: provider.credential_ref.clone().unwrap_or_default(), rank: 0, enabled: true, separate_account_at: None,
            created_at: now };
        insert(conn, &first)?;
        keys.push(first);
    }
    if keys.len() >= MAX_KEYS {
        return Err(format!("A connection holds at most {MAX_KEYS} keys."));
    }
    if keys.iter().any(|k| k.label.eq_ignore_ascii_case(label)) {
        return Err(format!("This connection already has a key named \"{label}\"."));
    }
    let key = Key { id: uuid::Uuid::new_v4().simple().to_string(), provider_id: provider_id.into(), label: label.into(),
        credential_ref: credential_ref.into(), rank: keys.iter().map(|k| k.rank).max().unwrap_or(0) + 1, enabled: true,
        separate_account_at: Some(now), created_at: now };
    insert(conn, &key)?;
    Ok(key)
}

fn insert(conn: &Connection, k: &Key) -> Result<()> {
    conn.execute("INSERT INTO provider_keys (id, provider_id, label, credential_ref, rank, enabled, separate_account_at, created_at) \
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)", params![k.id, k.provider_id, k.label, k.credential_ref, k.rank, k.enabled as i64,
        k.separate_account_at, k.created_at]).map(|_| ()).map_err(err)
}

/// Removes a key and returns its credential reference, for the caller to delete the secret. Removing the connection's
/// own key makes the next one its key; the last key is not removed (remove the connection, or replace its key).
pub fn remove(conn: &Connection, key_id: &str) -> Result<String> {
    let key = get(conn, key_id)?.ok_or("No such key")?;
    let keys = list(conn, &key.provider_id)?;
    let Some(next) = keys.iter().find(|k| k.id != key.id) else {
        return Err("This is the connection's only key. Replace it, or remove the connection.".into());
    };
    let own: Option<String> = conn.query_row("SELECT credential_ref FROM providers WHERE id = ?1", [&key.provider_id], |r| r.get(0))
        .optional().map_err(err)?.flatten();
    if own.as_deref() == Some(key.credential_ref.as_str()) {
        conn.execute("UPDATE providers SET credential_ref = ?2 WHERE id = ?1", params![key.provider_id, next.credential_ref])
            .map_err(err)?;
    }
    if keys.iter().filter(|k| k.id != key.id && k.enabled).count() == 0 {
        conn.execute("UPDATE provider_keys SET enabled = 1 WHERE id = ?1", [&next.id]).map_err(err)?;
    }
    conn.execute("DELETE FROM provider_keys WHERE id = ?1", [key_id]).map_err(err)?;
    renumber(conn, &key.provider_id)?;
    Ok(key.credential_ref)
}

fn renumber(conn: &Connection, provider_id: &str) -> Result<()> {
    for (i, k) in list(conn, provider_id)?.iter().enumerate() {
        conn.execute("UPDATE provider_keys SET rank = ?2 WHERE id = ?1", params![k.id, i as i64]).map_err(err)?;
    }
    Ok(())
}

/// Moves a key one place earlier (`up`) or later in the user's order.
pub fn move_key(conn: &Connection, key_id: &str, up: bool) -> Result<()> {
    let key = get(conn, key_id)?.ok_or("No such key")?;
    let mut keys = list(conn, &key.provider_id)?;
    let Some(i) = keys.iter().position(|k| k.id == key_id) else { return Ok(()) };
    let j = if up { i.checked_sub(1) } else { (i + 1 < keys.len()).then_some(i + 1) };
    if let Some(j) = j {
        keys.swap(i, j);
        for (rank, k) in keys.iter().enumerate() {
            conn.execute("UPDATE provider_keys SET rank = ?2 WHERE id = ?1", params![k.id, rank as i64]).map_err(err)?;
        }
    }
    Ok(())
}

/// Turns a key on or off; the last key on stays on (a connection with no key to send has nothing to do).
pub fn set_enabled(conn: &Connection, key_id: &str, enabled: bool) -> Result<()> {
    let key = get(conn, key_id)?.ok_or("No such key")?;
    if !enabled && list(conn, &key.provider_id)?.iter().filter(|k| k.enabled && k.id != key.id).count() == 0 {
        return Err("This is the only key on; turn another one on first, or turn the connection off.".into());
    }
    conn.execute("UPDATE provider_keys SET enabled = ?2 WHERE id = ?1", params![key_id, enabled as i64]).map(|_| ()).map_err(err)
}

pub fn set_strategy(conn: &Connection, provider_id: &str, strategy: &str) -> Result<()> {
    if !STRATEGIES.contains(&strategy) {
        return Err(format!("A key order is one of {}", STRATEGIES.join(", ")));
    }
    let n = conn.execute("UPDATE providers SET key_strategy = ?2 WHERE id = ?1", params![provider_id, strategy]).map_err(err)?;
    if n == 0 { Err("No such connection".into()) } else { Ok(()) }
}

pub fn strategy(conn: &Connection, provider_id: &str) -> Result<String> {
    conn.query_row("SELECT key_strategy FROM providers WHERE id = ?1", [provider_id], |r| r.get(0)).optional().map_err(err)
        .map(|s| s.unwrap_or_else(|| STRATEGIES[0].to_string()))
}

// ---------- Health ----------

/// Marks a key: a rejected key until the user fixes it (`until_ms` `None`), a cooldown until `until_ms`.
pub fn mark(conn: &Connection, key_id: &str, model_id: &str, state: &str, reason: &str, http_status: Option<u16>,
    until_ms: Option<i64>) -> Result<()> {
    conn.execute("INSERT INTO key_health (key_id, model_id, state, reason, http_status, until_ms, updated_at) \
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT(key_id, model_id) DO UPDATE SET state = ?3, reason = ?4, \
        http_status = ?5, until_ms = ?6, updated_at = ?7",
        params![key_id, model_id, state, reason.chars().take(300).collect::<String>(), http_status, until_ms, crate::db::now_ms()])
        .map(|_| ()).map_err(err)
}

/// A key answered for this model: its marks for the model and for the whole key go.
pub fn healthy(conn: &Connection, key_id: &str, model_id: &str) -> Result<()> {
    conn.execute("DELETE FROM key_health WHERE key_id = ?1 AND (model_id = ?2 OR model_id = '')", params![key_id, model_id])
        .map(|_| ()).map_err(err)
}

/// A connection's keys' marks still in force.
pub fn health(conn: &Connection, provider_id: &str, now: i64) -> Result<Vec<KeyHealth>> {
    let mut stmt = conn.prepare("SELECT h.key_id, h.model_id, h.state, h.reason, h.http_status, h.until_ms, h.updated_at \
        FROM key_health h JOIN provider_keys k ON k.id = h.key_id WHERE k.provider_id = ?1 \
        AND (h.until_ms IS NULL OR h.until_ms > ?2) ORDER BY h.updated_at DESC").map_err(err)?;
    let rows = stmt.query_map(params![provider_id, now], |r| Ok(KeyHealth { key_id: r.get(0)?, model_id: r.get(1)?,
        state: r.get(2)?, reason: r.get(3)?, http_status: r.get::<_, Option<i64>>(4)?.map(|v| v as u16), until_ms: r.get(5)?,
        updated_at: r.get(6)? })).map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

/// The mark that keeps this key off this model now, if any.
pub fn blocking(conn: &Connection, key: &Key, model_id: &str, now: i64) -> Result<Option<KeyHealth>> {
    Ok(health(conn, &key.provider_id, now)?.into_iter().find(|h| h.key_id == key.id && (h.model_id.is_empty() || h.model_id == model_id)))
}

/// Connections → Clear: every key's marks go too.
pub fn clear_health(conn: &Connection, provider_id: &str) -> Result<()> {
    conn.execute("DELETE FROM key_health WHERE key_id IN (SELECT id FROM provider_keys WHERE provider_id = ?1)", [provider_id])
        .map(|_| ()).map_err(err)
}

// ---------- Use and stated limits ----------

/// Keeps the rate-limit headers this key's account sent with an answer; nothing when there were none.
pub fn record_limits(conn: &Connection, key_id: &str, model_id: &str, status: u16, headers: &[(String, String)]) -> Result<()> {
    let kept: std::collections::BTreeMap<&str, &str> = headers.iter()
        .filter(|(n, _)| store::is_limit_header(n)).map(|(n, v)| (n.as_str(), v.as_str())).collect();
    if kept.is_empty() {
        return Ok(());
    }
    let json = serde_json::to_string(&kept).map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO key_limits (key_id, model_id, headers_json, http_status, observed_at) VALUES (?1, ?2, ?3, ?4, ?5) \
        ON CONFLICT (key_id, model_id) DO UPDATE SET headers_json = excluded.headers_json, http_status = excluded.http_status, \
        observed_at = excluded.observed_at", params![key_id, model_id, json, status as i64, crate::db::now_ms()]).map(|_| ()).map_err(err)
}

/// The limits this key's account last stated for this model, read.
pub fn windows(conn: &Connection, key_id: &str, model_id: &str) -> Result<Vec<Window>> {
    let row: Option<(String, i64)> = conn.query_row("SELECT headers_json, observed_at FROM key_limits WHERE key_id = ?1 AND model_id = ?2",
        params![key_id, model_id], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(err)?;
    Ok(row.map(|(json, at)| quota::from_headers(&serde_json::from_str(&json).unwrap_or_default(), at)).unwrap_or_default())
}

/// Requests this key sent since 00:00 UTC, and when it last sent one.
pub fn use_today(conn: &Connection, key_id: &str, now: i64) -> Result<(u64, Option<i64>)> {
    let day_start = now - now.rem_euclid(86_400_000);
    let today: i64 = conn.query_row("SELECT COUNT(*) FROM usage_events WHERE key_id = ?1 AND created_at >= ?2",
        params![key_id, day_start], |r| r.get(0)).map_err(err)?;
    let last: Option<i64> = conn.query_row("SELECT MAX(created_at) FROM usage_events WHERE key_id = ?1", [key_id], |r| r.get(0))
        .map_err(err)?;
    Ok((today.max(0) as u64, last))
}

/// The key that last answered this session on this connection and model, so the session stays on its account (and its
/// prompt cache).
pub fn session_key(conn: &Connection, client_id: &str, session: &str, provider_id: &str, model_id: &str) -> Result<Option<String>> {
    conn.query_row("SELECT key_id FROM usage_events WHERE client_id = ?1 AND session_key = ?2 AND provider = ?3 AND model = ?4 \
        AND key_id IS NOT NULL AND outcome = 'ok' ORDER BY created_at DESC LIMIT 1",
        params![client_id, session, provider_id, model_id], |r| r.get(0)).optional().map_err(err)
}

/// How much room the stated limits leave, from 0 (none) to 1 (all), the tightest window counting; `None` when the
/// account stated none. A window used up until a time still ahead is 0.
fn room(windows: &[Window], now: i64) -> Option<f64> {
    windows.iter().filter(|w| w.kind != "retry").filter_map(|w| {
        if w.exhausted_at(now).is_some() {
            return Some(0.0);
        }
        match (w.limit, w.remaining) {
            (Some(limit), Some(remaining)) if limit > 0 => Some((remaining as f64 / limit as f64).min(1.0)),
            _ => None,
        }
    }).reduce(f64::min)
}

/// The keys to try for one request on this connection and model, in its strategy's order, the session's own key first.
/// Empty when the connection has no ring (fewer than two keys): its own key is used, as before. Disabled keys are left
/// out; a key's cooldowns are the caller's to skip, with their reasons. `round_robin` moves its start on by one.
pub fn order(conn: &Connection, provider: &Provider, model_id: &str, sticky: Option<&str>, now: i64) -> Result<Vec<Key>> {
    let all = list(conn, &provider.id)?;
    if all.len() < 2 {
        return Ok(Vec::new());
    }
    let mut keys: Vec<Key> = all.into_iter().filter(|k| k.enabled).collect();
    match strategy(conn, &provider.id)?.as_str() {
        "round_robin" if !keys.is_empty() => {
            let cursor: i64 = conn.query_row("SELECT key_cursor FROM providers WHERE id = ?1", [&provider.id], |r| r.get(0)).map_err(err)?;
            let len = keys.len();
            keys.rotate_left(cursor.rem_euclid(len as i64) as usize);
            conn.execute("UPDATE providers SET key_cursor = (key_cursor + 1) % ?2 WHERE id = ?1", params![provider.id, len as i64])
                .map_err(err)?;
        }
        "least_used" => {
            let mut used = Vec::with_capacity(keys.len());
            for k in keys {
                used.push((use_today(conn, &k.id, now)?.0, k));
            }
            used.sort_by_key(|(n, k)| (*n, k.rank));
            keys = used.into_iter().map(|(_, k)| k).collect();
        }
        "quota_aware" => {
            let mut scored = Vec::with_capacity(keys.len());
            for k in keys {
                // A key whose account stated nothing yet is taken as having room: a fresh key usually has.
                let room = room(&windows(conn, &k.id, model_id)?, now).unwrap_or(1.0);
                scored.push((room, use_today(conn, &k.id, now)?.0, k));
            }
            scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.rank.cmp(&b.2.rank)));
            keys = scored.into_iter().map(|(_, _, k)| k).collect();
        }
        _ => {}
    }
    if let Some(i) = sticky.and_then(|id| keys.iter().position(|k| k.id == id)) {
        let key = keys.remove(i);
        keys.insert(0, key);
    }
    Ok(keys)
}

/// One key as Providers shows it: its state, its use today and what its account last stated.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyView {
    #[serde(flatten)]
    pub key: Key,
    /// The connection's own key (the one model lists and checks use).
    pub primary: bool,
    pub health: Vec<KeyHealth>,
    pub requests_today: u64,
    pub last_used_at: Option<i64>,
    /// The tightest room its account's stated limits leave on any model, 0 to 1, if it stated any.
    pub room: Option<f64>,
}

pub fn views(conn: &Connection, provider: &Provider, now: i64) -> Result<Vec<KeyView>> {
    let marks = health(conn, &provider.id, now)?;
    let mut out = Vec::new();
    for key in list(conn, &provider.id)? {
        let (requests_today, last_used_at) = use_today(conn, &key.id, now)?;
        let mut stmt = conn.prepare("SELECT model_id FROM key_limits WHERE key_id = ?1").map_err(err)?;
        let models: Vec<String> = stmt.query_map([&key.id], |r| r.get(0)).map_err(err)?.collect::<rusqlite::Result<_>>().map_err(err)?;
        let mut tightest: Option<f64> = None;
        for model in models {
            if let Some(r) = room(&windows(conn, &key.id, &model)?, now) {
                tightest = Some(tightest.map_or(r, |t| t.min(r)));
            }
        }
        out.push(KeyView { primary: provider.credential_ref.as_deref() == Some(key.credential_ref.as_str()),
            health: marks.iter().filter(|h| h.key_id == key.id).cloned().collect(), requests_today, last_used_at, room: tightest, key });
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
