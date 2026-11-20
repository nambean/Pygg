//! The model facts feed (ADR-040, docs/design/BRAIN.md §1): per-role scores from public benchmarks, built and
//! signed by the project's own job (`scripts/model-facts.py`), fetched by the app, verified, kept next to the store
//! like the models.dev catalog (reference.rs), and ranked per role over the user's connected models.
//!
//! Three rules hold everywhere here: a fact names its source, date and licence or it does not exist; a derived
//! fact (a reasoning rank standing in for planning) says so; and a feed that does not verify, or is older than the
//! one kept, is refused. The feed changes what the screens suggest, never a choice the user saved.
use super::store::{self, Model, Provider, Result};
use super::Shared;
use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The feed's fixed address: the assets of the `model-facts-v1` tag of the project's repository, replaced by every
/// build. A `v2` of the format gets a new tag and a new constant.
pub const URL: &str = "https://github.com/nambean/Pygg/releases/download/model-facts-v1/facts.json";
pub const SIG_URL: &str = "https://github.com/nambean/Pygg/releases/download/model-facts-v1/facts.json.sig";
/// The format this app reads.
pub const VERSION: u32 = 1;
/// The public keys the app trusts, by the id the `.sig` file names. Rotation adds a key here in a release before
/// the job's secret changes; a compromised key is removed here and every feed it signed is refused from then on.
pub const KEYS: &[(&str, &str)] = &[("mf-2026-1", "bccb0bf92f9c32091c3a21324660d19669cc1e7f7e55d143d1aff28e42e2e3ab")];
pub const SETTING_ENABLED: &str = "model_facts.enabled";
pub const SETTING_FETCHED_AT: &str = "model_facts.fetched_at";
pub const SETTING_LAST_ERROR: &str = "model_facts.last_error";
pub const ROLES: &[&str] = &["plan", "code", "test", "review", "image"];
/// The roles a client may route by (`auto/<role>`, ADR-041); `image` waits for an images path in the gateway.
pub const ROUTED_ROLES: &[&str] = &["plan", "code", "test", "review"];
/// Written as a fact's source into anything that records where a rank came from.
pub const SOURCE: &str = "model-facts:v1";
const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
/// A fact older than this is shown, never ranked (BRAIN.md §1).
const RANKS_FOR_DAYS: i64 = 366;
/// The agents Pygg runs a card with; a row run with one of them is preferred among equal ranks.
const PYGG_AGENTS: &[&str] = &["claude-code", "codex", "opencode", "gemini-cli"];

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Feed {
    pub version: u32,
    /// When the job built it, RFC 3339 in UTC; a later feed must have a later `built`.
    pub built: String,
    pub sources: Vec<Source>,
    pub facts: Vec<Fact>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: String,
    pub name: String,
    pub url: String,
    pub licence: String,
    pub attribution: String,
    pub measures: String,
    pub roles: Vec<String>,
    /// `ok`, or `stale` when the job carried the last facts forward because the source failed.
    pub status: String,
    #[serde(default)]
    pub observed: Option<String>,
    #[serde(default)]
    pub rows: Option<u64>,
    #[serde(default)]
    pub terms_read: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Fact {
    /// Canonical ids this row is about (`claude-opus-4-5`), as `canonical` writes them.
    pub ids: Vec<String>,
    /// The name as the source writes it.
    pub name: String,
    pub role: String,
    pub score: f64,
    pub unit: String,
    #[serde(default)]
    pub ci: Option<f64>,
    /// 1 is best; rows with the same score share a place.
    pub place: u32,
    pub of: u32,
    /// `(of − place) / (of − 1)`: 1 best, 0 worst, within the source.
    pub rank: f64,
    pub derived: bool,
    pub source: String,
    #[serde(default)]
    pub url: Option<String>,
    /// The row's own date in the source (`YYYY-MM-DD`): what the twelve-month rule reads.
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    pub licence: String,
    #[serde(default)]
    pub votes: Option<u64>,
    #[serde(default)]
    pub cost_usd: Option<f64>,
}

// ---------- identity ----------

/// One spelling for a model id across providers, the same rule as the job's `canonical`: the tail after a router's
/// vendor prefix, lower-cased, `.`, `_` and spaces as `-`, a `:free`-style variant, `-latest` and a trailing date
/// dropped. `openrouter/anthropic/claude-opus-4.5` and `claude-opus-4-5-20251101` both give `claude-opus-4-5`.
pub fn canonical(model_id: &str) -> String {
    let tail = model_id.trim().rsplit('/').next().unwrap_or("").to_ascii_lowercase();
    let tail = match tail.rsplit_once(':') {
        Some((base, variant)) if ["free", "nitro", "floor", "exacto", "online"].contains(&variant) => base.to_string(),
        _ => tail,
    };
    let mut out = String::with_capacity(tail.len());
    for c in tail.chars() {
        let c = if c == '.' || c == '_' || c.is_whitespace() { '-' } else { c };
        if c == '-' && out.ends_with('-') {
            continue;
        }
        out.push(c);
    }
    let out = out.trim_matches('-').to_string();
    let out = out.strip_suffix("-latest").map(str::to_string).unwrap_or(out);
    strip_date(&out)
}

/// Drops a trailing `-20251101` or `-2025-11-01`.
fn strip_date(id: &str) -> String {
    if let Some((base, tail)) = id.rsplit_once('-') {
        if tail.len() == 8 && tail.bytes().all(|b| b.is_ascii_digit()) {
            return base.to_string();
        }
    }
    let bytes = id.as_bytes();
    if bytes.len() > 11 {
        let tail = &id[id.len() - 11..];
        let shaped = tail.as_bytes()[0] == b'-'
            && tail[1..]
                .bytes()
                .enumerate()
                .all(|(i, b)| if i == 4 || i == 7 { b == b'-' } else { b.is_ascii_digit() });
        if shaped {
            return id[..id.len() - 11].to_string();
        }
    }
    id.to_string()
}

// ---------- the file ----------

/// `<store's folder>/reference/model-facts.json`; its signature is kept beside it as `.sig`.
pub fn cache_path(store: &Path) -> PathBuf {
    store.parent().unwrap_or(Path::new(".")).join("reference").join("model-facts.json")
}

fn sig_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".sig");
    path.with_file_name(name)
}

/// Checks `sig` (`<key id>:<base64>`) over the exact `bytes` against `keys`; the key's id when it holds.
pub fn verify_bytes(bytes: &[u8], sig: &str, keys: &[(&str, &str)]) -> std::result::Result<String, String> {
    let (key_id, sig_b64) = sig.trim().split_once(':').ok_or("the signature file is not <key id>:<signature>")?;
    let (_, public_hex) = keys
        .iter()
        .find(|(id, _)| *id == key_id)
        .ok_or_else(|| format!("signed with a key this app does not trust ({key_id})"))?;
    let public = hex_bytes(public_hex).ok_or("a trusted key is not 32 bytes of hex")?;
    let verifying = VerifyingKey::from_bytes(&public).map_err(|e| format!("a trusted key is not valid: {e}"))?;
    let signature = base64::engine::general_purpose::STANDARD
        .decode(sig_b64.trim())
        .map_err(|e| format!("the signature is not base64: {e}"))?;
    let signature =
        Signature::from_slice(&signature).map_err(|e| format!("the signature is not an ed25519 signature: {e}"))?;
    verifying.verify_strict(bytes, &signature).map_err(|_| "the signature does not match the feed".to_string())?;
    Ok(key_id.to_string())
}

fn hex_bytes(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        out[i] = u8::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok()?;
    }
    Some(out)
}

/// A feed from its bytes: the format version this app reads, every fact with a source the feed lists.
pub fn parse(bytes: &[u8]) -> std::result::Result<Feed, String> {
    let feed: Feed =
        serde_json::from_slice(bytes).map_err(|e| format!("the feed is not in the expected shape: {e}"))?;
    if feed.version != VERSION {
        return Err(format!("the feed is format {}; this app reads format {VERSION}", feed.version));
    }
    if feed.facts.iter().any(|f| !feed.sources.iter().any(|s| s.id == f.source)) {
        return Err("a fact names a source the feed does not list".into());
    }
    if feed.facts.iter().any(|f| {
        !ROLES.contains(&f.role.as_str())
            || !(0.0..=1.0).contains(&f.rank)
            || f.of == 0
            || f.place == 0
            || f.place > f.of
    }) {
        return Err("a fact has a role, place or rank outside the format".into());
    }
    Ok(feed)
}

/// The kept feed, verified again on every read (a changed file on disk is refused like a changed download).
pub fn load(path: &Path, keys: &[(&str, &str)]) -> Option<Feed> {
    let bytes = std::fs::read(path).ok()?;
    let sig = std::fs::read_to_string(sig_path(path)).ok()?;
    verify_bytes(&bytes, &sig, keys).ok()?;
    parse(&bytes).ok()
}

/// The kept feed for routing, parsed once per file version: a request to `auto/<role>` must not read and verify
/// 400 KB each time. The cache is keyed by the file's path and modification time, so a new feed is picked up at
/// once and a removed one is dropped.
pub fn cached(path: &Path) -> Option<std::sync::Arc<Feed>> {
    use std::sync::{Arc, Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<Option<(PathBuf, std::time::SystemTime, Arc<Feed>)>>> = OnceLock::new();
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    let mut slot = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((p, t, feed)) = slot.as_ref() {
        if p == path && *t == modified {
            return Some(feed.clone());
        }
    }
    let feed = Arc::new(load(path, KEYS)?);
    *slot = Some((path.to_path_buf(), modified, feed.clone()));
    Some(feed)
}

/// The feed the store's routing reads: on, and kept beside the store this connection opened (none for a store in
/// memory, as in tests, which pass a feed by hand).
pub fn for_conn(conn: &Connection) -> Option<std::sync::Arc<Feed>> {
    if !enabled(conn) {
        return None;
    }
    let store = conn.path().filter(|p| !p.is_empty())?;
    cached(&cache_path(Path::new(store)))
}

/// Fetches the feed and its signature, verifies, refuses a `built` not later than the kept one, and writes both
/// files atomically beside the store.
pub async fn fetch(
    http: &reqwest::Client, path: &Path, keys: &[(&str, &str)], urls: (&str, &str),
) -> std::result::Result<Feed, String> {
    async fn get(http: &reqwest::Client, url: &str) -> std::result::Result<Vec<u8>, String> {
        let response =
            http.get(url).timeout(Duration::from_secs(60)).send().await.map_err(|e| format!("model facts: {e}"))?;
        if !response.status().is_success() {
            return Err(format!("model facts: HTTP {} for {url}", response.status().as_u16()));
        }
        response.bytes().await.map(|b| b.to_vec()).map_err(|e| format!("model facts: {e}"))
    }
    let bytes = get(http, urls.0).await?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("model facts: the feed is over 16 MB; refused".into());
    }
    let sig = String::from_utf8(get(http, urls.1).await?).map_err(|_| "model facts: the signature file is not text")?;
    verify_bytes(&bytes, &sig, keys)?;
    let feed = parse(&bytes)?;
    if let Some(kept) = load(path, keys) {
        if feed.built <= kept.built && feed != kept {
            return Err(format!(
                "model facts: the fetched feed ({}) is not newer than the kept one ({}); refused",
                feed.built, kept.built
            ));
        }
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &bytes).map_err(|e| format!("model facts: could not write the feed: {e}"))?;
    std::fs::write(sig_path(path), sig.as_bytes())
        .map_err(|e| format!("model facts: could not write the signature: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("model facts: could not keep the feed: {e}"))?;
    Ok(feed)
}

// ---------- settings ----------

pub fn enabled(conn: &Connection) -> bool {
    store::setting(conn, SETTING_ENABLED).ok().flatten().is_none_or(|v| v != "0")
}

pub fn set_enabled(conn: &Connection, on: bool) -> Result<()> {
    store::set_setting(conn, SETTING_ENABLED, if on { "1" } else { "0" })
}

fn fetched_at(conn: &Connection) -> Option<i64> {
    store::setting(conn, SETTING_FETCHED_AT).ok().flatten().and_then(|v| v.parse().ok())
}

/// Whether a daily check is due: fetching is on and the last fetch is older than a day (or never happened).
pub fn due(conn: &Connection) -> bool {
    enabled(conn) && fetched_at(conn).is_none_or(|t| crate::db::now_ms() - t > MAX_AGE.as_millis() as i64)
}

/// Fetches now and records the outcome in the settings: the time on success, the reason on failure (the kept
/// feed stays either way).
pub async fn refresh(shared: &Shared, path: &Path) -> std::result::Result<Feed, String> {
    let result = fetch(&shared.http, path, KEYS, (URL, SIG_URL)).await;
    shared.with_db(|db| {
        let _ = match &result {
            Ok(_) => store::set_setting(db, SETTING_FETCHED_AT, &crate::db::now_ms().to_string())
                .and_then(|()| store::set_setting(db, SETTING_LAST_ERROR, "")),
            Err(e) => store::set_setting(db, SETTING_LAST_ERROR, e),
        };
    });
    result
}

/// The daily check, for start-up and the headless gateway: fetches when due, else keeps what is kept.
pub async fn refresh_if_due(shared: &Shared, path: &Path) -> Option<Feed> {
    if shared.with_db(due) {
        match refresh(shared, path).await {
            Ok(feed) => return Some(feed),
            Err(e) => crate::applog::warn(&e),
        }
    }
    if shared.with_db(enabled) {
        load(path, KEYS)
    } else {
        None
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub enabled: bool,
    /// When the feed was last fetched, ms since the epoch.
    pub fetched_at: Option<i64>,
    /// The kept feed's `built`, when one is kept and verifies.
    pub built: Option<String>,
    pub facts: usize,
    pub sources: Vec<Source>,
    /// Why the last fetch failed, when it did.
    pub last_error: Option<String>,
    /// The kept feed is older than the weekly cadence allows for (ten days).
    pub stale: bool,
}

pub fn status(conn: &Connection, path: &Path) -> Status {
    let feed = load(path, KEYS);
    let built_days = feed.as_ref().and_then(|f| days(&f.built));
    let stale = built_days.is_some_and(|b| today_days() - b > 10);
    Status {
        enabled: enabled(conn),
        fetched_at: fetched_at(conn),
        built: feed.as_ref().map(|f| f.built.clone()),
        facts: feed.as_ref().map(|f| f.facts.len()).unwrap_or(0),
        sources: feed.map(|f| f.sources).unwrap_or_default(),
        last_error: store::setting(conn, SETTING_LAST_ERROR).ok().flatten().filter(|e| !e.is_empty()),
        stale,
    }
}

// ---------- ranking ----------

/// One fact as a screen shows it.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FactView {
    pub source: String,
    pub source_name: String,
    pub licence: String,
    pub url: Option<String>,
    pub name: String,
    pub place: u32,
    pub of: u32,
    pub rank: f64,
    pub score: f64,
    pub unit: String,
    pub ci: Option<f64>,
    pub date: Option<String>,
    pub derived: bool,
    pub agent: Option<String>,
    pub effort: Option<String>,
    pub votes: Option<u64>,
    /// False when the row is older than twelve months: shown, not ranked.
    pub ranks: bool,
    /// What the source measures and does not, from its source entry.
    pub measures: String,
}

/// What the user's own runs say about a model for a role (ADR-029): attempts on cards of that role whose step
/// ran this model, how many passed their checks, and how many results were kept.
#[derive(Clone, Debug, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Own {
    pub attempts: u32,
    pub checks_passed: u32,
    pub kept: u32,
    /// True from `OWN_MIN_ATTEMPTS` attempts on: then this layer wins over the feed.
    pub ranks: bool,
    /// What those attempts' priced requests cost through the gateway (actual or estimated), micro-USD.
    pub known_micro_usd: u64,
    /// Their requests with no price: the known money is then a floor.
    pub unknown_requests: u32,
}

impl Own {
    /// Known money per kept result; `None` with nothing kept.
    pub fn micro_usd_per_kept(&self) -> Option<u64> {
        (self.kept > 0).then(|| self.known_micro_usd / self.kept as u64)
    }
}

/// Own evidence ranks from this many attempts (BRAIN.md §1).
pub const OWN_MIN_ATTEMPTS: u32 = 10;

/// The user's own evidence per `connection/model` for a role, from the attempts of tasks made from cards
/// (`tasks.card_role`, migration 0042): an attempt's `model_ref` is `<agent> · <model>`, the model being
/// `pygg/<connection/model>` for an agent pointed at the gateway. A result is counted as kept once per task, on the
/// attempt that made it.
pub fn own_evidence(conn: &Connection, role: &str) -> Result<std::collections::HashMap<String, Own>> {
    let mut stmt = conn
        .prepare(
            "SELECT a.model_ref, a.state, t.result, a.number, (SELECT MAX(number) FROM attempts x JOIN task_stages y ON x.stage_id = y.id WHERE y.task_id = t.id), (SELECT COALESCE(SUM(CASE WHEN u.cost_status <> 'unknown' THEN u.cost_micro_usd END), 0) FROM usage_events u WHERE u.attempt_id = a.id), (SELECT COUNT(*) FROM usage_events u WHERE u.attempt_id = a.id AND u.cost_status = 'unknown') FROM attempts a JOIN task_stages s ON a.stage_id = s.id JOIN tasks t ON s.task_id = t.id WHERE t.card_role = ?1 AND a.state IN ('passed', 'failed', 'done')",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([role], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, Option<i64>>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, i64>(6)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut out: std::collections::HashMap<String, Own> = std::collections::HashMap::new();
    for row in rows {
        let (model_ref, state, result, number, last, known, unknown) = row.map_err(|e| e.to_string())?;
        let model = model_ref.rsplit(" · ").next().unwrap_or(&model_ref);
        let model = model.strip_prefix("pygg/").unwrap_or(model).to_string();
        let own = out.entry(model).or_default();
        own.attempts += 1;
        own.known_micro_usd += known.max(0) as u64;
        own.unknown_requests += unknown.max(0) as u32;
        if state == "passed" {
            own.checks_passed += 1;
        }
        if result.as_deref() == Some("kept") && Some(number) == last {
            own.kept += 1;
        }
    }
    for own in out.values_mut() {
        own.ranks = own.attempts >= OWN_MIN_ATTEMPTS;
    }
    Ok(out)
}
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    /// `connection/model`, the name a client sends.
    pub model: String,
    pub provider_id: String,
    pub model_id: String,
    pub billing: String,
    pub tier: Option<String>,
    pub tier_source: Option<String>,
    /// `own` (the user's own attempts on cards of this role, from ten on), `feed`, `tier`, or `none`.
    pub layer: String,
    /// The user's own evidence for this model and role, when any attempt exists (ranks from `OWN_MIN_ATTEMPTS`).
    pub own: Option<Own>,
    /// The fact the rule used, when the feed layer won.
    pub fact: Option<FactView>,
    /// Every other fact for this model and role, the ones too old to rank included.
    pub others: Vec<FactView>,
    /// The list price per million output tokens, micro-dollars.
    pub output_micro_per_mtok: Option<u64>,
    /// A typical agent turn at list price (`typical_turn_micro`); 0 on a local, free or subscription model, `None`
    /// unpriced. The cheaper of two models in one band ranks first.
    pub turn_micro_usd: Option<u64>,
}

fn view(fact: &Fact, sources: &[Source], today: i64) -> FactView {
    let source = sources.iter().find(|s| s.id == fact.source);
    let ranks = fact.date.as_deref().and_then(days).is_some_and(|d| today - d <= RANKS_FOR_DAYS);
    FactView {
        source: fact.source.clone(),
        source_name: source.map(|s| s.name.clone()).unwrap_or_else(|| fact.source.clone()),
        licence: fact.licence.clone(),
        url: fact.url.clone(),
        name: fact.name.clone(),
        place: fact.place,
        of: fact.of,
        rank: fact.rank,
        score: fact.score,
        unit: fact.unit.clone(),
        ci: fact.ci,
        date: fact.date.clone(),
        derived: fact.derived,
        agent: fact.agent.clone(),
        effort: fact.effort.clone(),
        votes: fact.votes,
        ranks,
        measures: source.map(|s| s.measures.clone()).unwrap_or_default(),
    }
}

/// The connected models ranked for `role`: enabled models of enabled connections, each with the best fact that
/// still ranks (the row run with an agent Pygg runs preferred among equal ranks), else its tier. Order: the layer
/// that won, then its measure, then the output list price, then the name (BRAIN.md §1).
pub fn rank_role(conn: &Connection, feed: Option<&Feed>, role: &str) -> Result<Vec<Line>> {
    rank_role_on(conn, feed, role, today_days())
}

fn rank_role_on(conn: &Connection, feed: Option<&Feed>, role: &str, today: i64) -> Result<Vec<Line>> {
    if !ROLES.contains(&role) {
        return Err(format!("{role} is not a role; the roles are {}", ROLES.join(", ")));
    }
    let providers: Vec<Provider> = store::list_providers(conn)?.into_iter().filter(|p| p.enabled).collect();
    let own = own_evidence(conn, role)?;
    let mut lines = Vec::new();
    for m in store::list_models(conn, None)?.into_iter().filter(|m| m.enabled) {
        if !providers.iter().any(|p| p.id == m.provider_id) {
            continue;
        }
        let mut line = line_for(&m, feed, role, today);
        line.own = own.get(&format!("{}/{}", m.provider_id, m.model_id)).cloned();
        if line.own.as_ref().is_some_and(|o| o.ranks) {
            line.layer = "own".into();
        }
        lines.push(line);
    }
    let standings: Vec<Standing> = lines
        .iter()
        .map(|l| Standing {
            own: l.own.clone().filter(|o| o.ranks),
            rank: l.fact.as_ref().map(|f| f.rank).or_else(|| l.tier.as_deref().map(tier_standing)),
            from_fact: l.fact.is_some(),
            billing: billing_order(&l.billing),
            turn_micro: l.turn_micro_usd,
            id: l.model.clone(),
        })
        .collect();
    let mut slots: Vec<Option<Line>> = lines.into_iter().map(Some).collect();
    Ok(order_for_role(&standings).into_iter().filter_map(|(i, _)| slots[i].take()).collect())
}

impl FactView {
    /// The fact in one ASCII line, for a routing reason and the ledger: the source, place, score, date, the agent
    /// and effort the row was run with, and `derived` where the source measures something near the role.
    pub fn describe(&self) -> String {
        let mut s =
            format!("{} #{} of {} ({}{}", self.source_name, self.place, self.of, self.score, unit_suffix(&self.unit));
        if let Some(ci) = self.ci {
            s.push_str(&format!(" +/-{ci}"));
        }
        s.push(')');
        if let Some(date) = &self.date {
            s.push_str(&format!(" dated {date}"));
        }
        match (&self.agent, &self.effort) {
            (Some(a), Some(e)) => s.push_str(&format!(", run with {a} at {e}")),
            (Some(a), None) => s.push_str(&format!(", run with {a}")),
            (None, Some(e)) => s.push_str(&format!(" at {e}")),
            (None, None) => {}
        }
        if self.derived {
            s.push_str(", derived");
        }
        s
    }
}

fn unit_suffix(unit: &str) -> String {
    if unit.starts_with('%') {
        "%".into()
    } else {
        format!(" {unit}")
    }
}

/// The best fact that ranks for a model and role, and every other fact behind it (BRAIN.md section 1).
pub fn best_for(feed: &Feed, model_id: &str, role: &str) -> (Option<FactView>, Vec<FactView>) {
    best_for_on(feed, model_id, role, today_days())
}

fn best_for_on(feed: &Feed, model_id: &str, role: &str, today: i64) -> (Option<FactView>, Vec<FactView>) {
    let id = canonical(model_id);
    let mut views: Vec<FactView> = feed
        .facts
        .iter()
        .filter(|fact| fact.role == role && fact.ids.contains(&id))
        .map(|fact| view(fact, &feed.sources, today))
        .collect();
    // The best ranking row first: rank, then an agent Pygg runs, then the newest.
    views.sort_by(|a, b| {
        b.ranks
            .cmp(&a.ranks)
            .then_with(|| b.rank.partial_cmp(&a.rank).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| {
                let pygg = |v: &FactView| v.agent.as_deref().is_some_and(|a| PYGG_AGENTS.contains(&a));
                pygg(b).cmp(&pygg(a))
            })
            .then_with(|| b.date.cmp(&a.date))
    });
    let fact = views.first().filter(|v| v.ranks).cloned();
    let others = if fact.is_some() { views[1..].to_vec() } else { views };
    (fact, others)
}

fn line_for(m: &Model, feed: Option<&Feed>, role: &str, today: i64) -> Line {
    let (fact, others) = feed.map(|f| best_for_on(f, &m.model_id, role, today)).unwrap_or((None, Vec::new()));
    let layer = if fact.is_some() {
        "feed"
    } else if m.capabilities.tier.is_some() {
        "tier"
    } else {
        "none"
    };
    Line {
        model: format!("{}/{}", m.provider_id, m.model_id),
        provider_id: m.provider_id.clone(),
        model_id: m.model_id.clone(),
        billing: m.billing.clone(),
        tier: m.capabilities.tier.clone(),
        tier_source: m.capabilities.tier_source.clone(),
        layer: layer.to_string(),
        own: None,
        fact,
        others,
        output_micro_per_mtok: m.price.as_ref().map(|p| p.output_micro_per_mtok),
        turn_micro_usd: turn_cost(&m.billing, m.price.as_ref()),
    }
}

// ---------- the order within a role (BRAIN.md §1) ----------

/// Facts whose ranks lie this close are one band: within a band the cheaper model ranks first. A place on a public
/// benchmark, run with another agent on other tasks, does not order two models this close for the user's own work:
/// on 2026-11-20, Terminal-Bench's #4 for code lost to its #12 on every count with OpenCode on Pygg's 36 cases
/// (benchmark/reports/2026-11-20-all-models.md), and the two were 0.11 apart.
pub const BAND: f64 = 0.15;

/// Kept rates this close are one band within the user's own evidence: ten attempts do not tell 9 of 10 from 10.
pub const KEPT_BAND: f64 = 0.1;

/// Where a tier stands among the facts' ranks, for a model the feed does not name: the middle of the tier's band, so
/// a tiered model can share a band with a weak fact and win it on price, but not one with a strong fact.
pub fn tier_standing(tier: &str) -> f64 {
    match tier {
        "T4" => 0.9,
        "T3" => 0.7,
        "T2" => 0.5,
        "T1" => 0.3,
        _ => 0.1,
    }
}

/// A typical agent turn at list price, micro-USD: 8,000 tokens in, four in five of them read from the cache where the
/// model prices cache reads, and 1,000 out. Only to order models by price; it is not an estimate of a request.
pub fn typical_turn_micro(price: &super::pricing::Price) -> u64 {
    let per = |tokens: u64, micro: u64| (tokens as u128 * micro as u128 / 1_000_000) as u64;
    let input = match price.cached_input_micro_per_mtok {
        Some(cached) => per(1_600, price.input_micro_per_mtok) + per(6_400, cached),
        None => per(8_000, price.input_micro_per_mtok),
    };
    input + per(1_000, price.output_micro_per_mtok)
}

/// A model's typical turn by its billing: nothing per request on a local, free or subscription model.
pub fn turn_cost(billing: &str, price: Option<&super::pricing::Price>) -> Option<u64> {
    match billing {
        "local" | "free" | "subscription" => Some(0),
        _ => price.map(typical_turn_micro),
    }
}

/// Local, free, subscription, then paid and unknown: who pays per request, least first.
pub fn billing_order(billing: &str) -> u8 {
    match billing {
        "local" => 0,
        "free" => 1,
        "subscription" => 2,
        _ => 3,
    }
}

/// What the role order compares about one model.
#[derive(Clone, Debug, Default)]
pub struct Standing {
    /// The user's own evidence, when it ranks (ten attempts or more).
    pub own: Option<Own>,
    /// The fact's rank, or the tier's standing for a model with no fact.
    pub rank: Option<f64>,
    pub from_fact: bool,
    pub billing: u8,
    /// A typical turn's price; `None` unpriced (last within a band).
    pub turn_micro: Option<u64>,
    /// `connection/model`, the last tie-break.
    pub id: String,
}

/// The order of a role (BRAIN.md §1), as indices into `items`. First the models with the user's own evidence: by kept
/// rate in bands of `KEPT_BAND`, within a band by known money per kept result (a model with unpriced requests after
/// those fully priced), then checks passed. Then the models with a fact or a tier: by rank in bands of `BAND`, within a
/// band cheapest first (billing, then a typical turn's price). Then the rest, cheapest first. Each index comes with
/// its band's leader when it ranks ahead of that leader, so a route can say why.
pub fn order_for_role(items: &[Standing]) -> Vec<(usize, Option<usize>)> {
    use std::cmp::Ordering;
    let cheaper = |a: &Standing, b: &Standing| {
        a.billing.cmp(&b.billing).then_with(|| a.turn_micro.unwrap_or(u64::MAX).cmp(&b.turn_micro.unwrap_or(u64::MAX)))
    };
    let kept = |s: &Standing| s.own.as_ref().map(|o| o.kept as f64 / o.attempts.max(1) as f64).unwrap_or(0.0);
    let checks =
        |s: &Standing| s.own.as_ref().map(|o| o.checks_passed as f64 / o.attempts.max(1) as f64).unwrap_or(0.0);
    let money = |s: &Standing| {
        s.own
            .as_ref()
            .map(|o| (o.unknown_requests > 0, o.micro_usd_per_kept().unwrap_or(u64::MAX)))
            .unwrap_or((true, u64::MAX))
    };
    let desc = |x: f64, y: f64| y.partial_cmp(&x).unwrap_or(Ordering::Equal);
    let mut own: Vec<usize> = (0..items.len()).filter(|&i| items[i].own.as_ref().is_some_and(|o| o.ranks)).collect();
    let mut ranked: Vec<usize> = (0..items.len()).filter(|&i| !own.contains(&i) && items[i].rank.is_some()).collect();
    let mut rest: Vec<usize> = (0..items.len()).filter(|&i| !own.contains(&i) && !ranked.contains(&i)).collect();
    own.sort_by(|&a, &b| desc(kept(&items[a]), kept(&items[b])).then_with(|| items[a].id.cmp(&items[b].id)));
    ranked.sort_by(|&a, &b| {
        desc(items[a].rank.unwrap_or(0.0), items[b].rank.unwrap_or(0.0))
            .then_with(|| items[b].from_fact.cmp(&items[a].from_fact))
            .then_with(|| items[a].id.cmp(&items[b].id))
    });
    rest.sort_by(|&a, &b| cheaper(&items[a], &items[b]).then_with(|| items[a].id.cmp(&items[b].id)));
    let mut out = Vec::with_capacity(items.len());
    let mut banded = |sorted: &[usize],
                      value: &dyn Fn(&Standing) -> f64,
                      width: f64,
                      within: &dyn Fn(&Standing, &Standing) -> Ordering| {
        let mut start = 0;
        while start < sorted.len() {
            let leader = sorted[start];
            let top = value(&items[leader]);
            let mut end = start + 1;
            while end < sorted.len() && top - value(&items[sorted[end]]) <= width + 1e-9 {
                end += 1;
            }
            let mut band = sorted[start..end].to_vec();
            band.sort_by(|&a, &b| within(&items[a], &items[b]).then_with(|| items[a].id.cmp(&items[b].id)));
            let at = band.iter().position(|&i| i == leader).unwrap_or(0);
            for (n, &i) in band.iter().enumerate() {
                out.push((i, (n < at).then_some(leader)));
            }
            start = end;
        }
    };
    banded(&own, &kept, KEPT_BAND, &|a, b| {
        money(a).cmp(&money(b)).then_with(|| desc(checks(a), checks(b))).then_with(|| cheaper(a, b))
    });
    banded(&ranked, &|s| s.rank.unwrap_or(0.0), BAND, &|a, b| {
        cheaper(a, b).then_with(|| desc(a.rank.unwrap_or(0.0), b.rank.unwrap_or(0.0)))
    });
    out.extend(rest.into_iter().map(|i| (i, None)));
    out
}

// ---------- dates ----------

/// Days since 1970-01-01 for a `YYYY-MM-DD` (a longer RFC 3339 string is read by its first ten characters).
fn days(date: &str) -> Option<i64> {
    let date = date.get(..10)?;
    let mut parts = date.split('-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let d: i64 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146097 + doe - 719468)
}

fn today_days() -> i64 {
    crate::db::now_ms() / 86_400_000
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::store::NewProvider;
    use ed25519_dalek::{Signer, SigningKey};
    use rusqlite::params;

    const SEED: [u8; 32] = [7; 32];

    fn signed(feed: &serde_json::Value) -> (Vec<u8>, String, String) {
        let key = SigningKey::from_bytes(&SEED);
        let bytes = serde_json::to_vec(feed).unwrap();
        let sig = base64::engine::general_purpose::STANDARD.encode(key.sign(&bytes).to_bytes());
        let public = key.verifying_key().to_bytes().iter().map(|b| format!("{b:02x}")).collect::<String>();
        (bytes, format!("test-1:{sig}\n"), public)
    }

    fn source(id: &str, name: &str, roles: &[&str]) -> serde_json::Value {
        serde_json::json!({"id": id, "name": name, "url": "https://example.test", "licence": "Apache-2.0", "attribution": "x",
            "measures": "what it measures", "roles": roles, "status": "ok", "observed": "2026-11-19", "rows": 2, "termsRead": "2026-11-19"})
    }

    fn fact(
        ids: &[&str], role: &str, place: u32, of: u32, date: &str, agent: Option<&str>, derived: bool,
    ) -> serde_json::Value {
        let rank = if of <= 1 { 1.0 } else { (of - place) as f64 / (of - 1) as f64 };
        serde_json::json!({"ids": ids, "name": ids[0], "role": role, "score": 50.0, "unit": "%", "ci": 2.0, "place": place, "of": of,
            "rank": rank, "derived": derived, "source": "tb", "url": null, "date": date, "agent": agent, "effort": "high", "licence": "Apache-2.0"})
    }

    fn feed(built: &str, facts: Vec<serde_json::Value>) -> serde_json::Value {
        serde_json::json!({"version": 1, "built": built, "sources": [source("tb", "Terminal-Bench 4.x", &["code"]), source("lm", "LMArena hard prompts", &["plan", "review"])], "facts": facts})
    }

    #[test]
    fn canonical_ids_agree_with_the_job() {
        assert_eq!(canonical("openrouter/anthropic/claude-opus-4.5"), "claude-opus-4-5");
        assert_eq!(canonical("claude-opus-4-5-20251101"), "claude-opus-4-5");
        assert_eq!(canonical("qwen/qwen3-coder-480b-a35b-instruct:free"), "qwen3-coder-480b-a35b-instruct");
        assert_eq!(canonical("gemini-2.5-flash-preview-05-20"), "gemini-2-5-flash-preview-05-20");
        assert_eq!(canonical("gpt-5.2-chat-latest"), "gpt-5-2-chat");
        assert_eq!(canonical("Claude Opus 5.5"), "claude-opus-5-5");
        assert_eq!(canonical("deepseek-chat-2025-11-01"), "deepseek-chat");
    }

    #[test]
    fn a_feed_verifies_only_with_a_trusted_key_over_the_exact_bytes() {
        let (bytes, sig, public) = signed(&feed("2026-11-19T00:00:00Z", vec![]));
        let keys = [("test-1", public.as_str())];
        assert_eq!(verify_bytes(&bytes, &sig, &keys).unwrap(), "test-1");
        let mut tampered = bytes.clone();
        tampered.push(b' ');
        assert!(verify_bytes(&tampered, &sig, &keys).unwrap_err().contains("does not match"));
        assert!(verify_bytes(&bytes, &sig, &[("other", public.as_str())]).unwrap_err().contains("does not trust"));
        assert!(verify_bytes(&bytes, "test-1:not base64!", &keys).is_err());
        assert!(verify_bytes(&bytes, "no colon", &keys).is_err());
        assert!(parse(&bytes).is_ok());
        let v2 =
            serde_json::to_vec(&serde_json::json!({"version": 2, "built": "x", "sources": [], "facts": []})).unwrap();
        assert!(parse(&v2).unwrap_err().contains("format 2"));
        let orphan = serde_json::to_vec(&feed("x", vec![serde_json::json!({"ids": ["a"], "name": "a", "role": "code", "score": 1.0, "unit": "%", "place": 1, "of": 1, "rank": 1.0, "derived": false, "source": "nope", "licence": "x"})])).unwrap();
        assert!(parse(&orphan).unwrap_err().contains("does not list"));
    }

    #[test]
    fn the_kept_feed_is_verified_again_when_read() {
        let dir = std::env::temp_dir().join(format!("pygg-facts-{}", uuid::Uuid::new_v4()));
        let path = dir.join("reference").join("model-facts.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let (bytes, sig, public) = signed(&feed("2026-11-19T00:00:00Z", vec![]));
        let keys = [("test-1", public.as_str())];
        std::fs::write(&path, &bytes).unwrap();
        std::fs::write(sig_path(&path), &sig).unwrap();
        assert_eq!(load(&path, &keys).unwrap().built, "2026-11-19T00:00:00Z");
        std::fs::write(&path, b"{}").unwrap();
        assert!(load(&path, &keys).is_none(), "a changed file is refused");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_role_is_ranked_by_the_feed_then_the_tier_and_old_rows_only_show() {
        let conn = crate::db::open_in_memory().unwrap();
        let p = store::add_provider(
            &conn,
            &NewProvider {
                kind: "openrouter",
                label: "openrouter",
                endpoint: "https://openrouter.ai/api/v1",
                credential_ref: None,
                default_billing: "paid",
                wire: "openai",
            },
        )
        .unwrap();
        for m in [
            "anthropic/claude-opus-4.5",
            "anthropic/claude-haiku-4.5",
            "google/gemini-3-pro",
            "z-ai/glm-4.7-flash:free",
        ] {
            store::add_model(&conn, &p.id, m).unwrap();
        }
        conn.execute(
            "UPDATE models SET tier = 3, tier_source = 'curated:v1' WHERE model_id = 'google/gemini-3-pro'",
            [],
        )
        .unwrap();
        let today = days("2026-11-19").unwrap();
        let the_feed: Feed = serde_json::from_value(feed(
            "2026-11-19T00:00:00Z",
            vec![
                fact(&["claude-opus-4-5"], "code", 1, 10, "2026-06-30", Some("claude-code"), false),
                fact(&["claude-opus-4-5"], "code", 3, 10, "2026-07-30", Some("mini-swe-agent"), false),
                fact(&["claude-haiku-4-5"], "code", 9, 10, "2025-08-01", Some("claude-code"), false),
                fact(&["glm-4-7-flash"], "code", 5, 10, "2026-10-01", None, false),
                fact(&["claude-opus-4-5"], "plan", 2, 10, "2026-11-18", None, true),
            ],
        ))
        .unwrap();
        let lines = rank_role_on(&conn, Some(&the_feed), "code", today).unwrap();
        let names: Vec<&str> = lines.iter().map(|l| l.model.as_str()).collect();
        // opus (rank 1.0) is a band of its own. gemini-3-pro has no fact; its tier T3 stands at 0.7 and shares a band
        // with glm's fact (0.56); neither is priced here, so the higher stands first. haiku's only fact is too old to
        // rank, and its T1 (0.3) comes last.
        assert_eq!(
            names,
            [
                "openrouter/anthropic/claude-opus-4.5",
                "openrouter/google/gemini-3-pro",
                "openrouter/z-ai/glm-4.7-flash:free",
                "openrouter/anthropic/claude-haiku-4.5"
            ]
        );
        let opus = &lines[0];
        assert_eq!(opus.layer, "feed");
        let best = opus.fact.as_ref().unwrap();
        assert_eq!((best.place, best.agent.as_deref(), best.ranks), (1, Some("claude-code"), true));
        assert_eq!(opus.others.len(), 1, "the other row is kept behind the best");
        let haiku = &lines[3];
        assert_eq!(
            (haiku.layer.as_str(), haiku.tier.as_deref()),
            ("tier", Some("T1")),
            "the naming table's tier ranks it"
        );
        assert!(
            haiku.fact.is_none() && haiku.others.len() == 1 && !haiku.others[0].ranks,
            "older than a year: shown, not ranked"
        );
        assert_eq!((lines[1].layer.as_str(), lines[1].tier.as_deref()), ("tier", Some("T3")));
        let plan = rank_role_on(&conn, Some(&the_feed), "plan", today).unwrap();
        assert!(plan[0].fact.as_ref().unwrap().derived);
        assert!(rank_role_on(&conn, None, "code", today).unwrap().iter().all(|l| l.layer != "feed"));
        assert!(rank_role_on(&conn, None, "coding", today).is_err());
    }

    #[test]
    fn the_users_own_runs_rank_a_model_above_the_feed_from_ten_attempts_on() {
        let conn = crate::db::open_in_memory().unwrap();
        let p = store::add_provider(
            &conn,
            &NewProvider {
                kind: "openrouter",
                label: "openrouter",
                endpoint: "https://openrouter.ai/api/v1",
                credential_ref: None,
                default_billing: "paid",
                wire: "openai",
            },
        )
        .unwrap();
        for m in ["anthropic/claude-opus-4.5", "google/gemini-3-pro", "z-ai/glm-4.7-flash:free"] {
            store::add_model(&conn, &p.id, m).unwrap();
        }
        conn.execute("INSERT INTO workspaces (id, name, path, created_at) VALUES ('w1', 'repo', 'C:/src/shop', 1)", [])
            .unwrap();
        // Tasks made from cards carry the card's role; their attempts are the evidence. Gemini: 10 attempts on code
        // cards, 7 passed their checks, 6 results kept (one task's kept result counts once, on its last attempt).
        // GLM: 3 attempts, below the threshold, so it shows but does not rank. Opus: an attempt on a review card only.
        let mut n = 0;
        let mut task = |role: &str, model: &str, states: &[&str], result: Option<&str>| {
            n += 1;
            let id = format!("t{n}");
            conn.execute(
                "INSERT INTO tasks (id, workspace_id, title, prompt, strategy, state, result, card_role, created_at, updated_at) \
                 VALUES (?1, 'w1', 'x', 'x', 'free_first', 'checks_passed', ?2, ?3, 1, 1)",
                params![id, result, role],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO task_stages (id, task_id, position, name, state) VALUES (?1, ?2, 0, 'change', 'done')",
                params![format!("s{n}"), id],
            )
            .unwrap();
            for (i, state) in states.iter().enumerate() {
                conn.execute(
                    "INSERT INTO attempts (id, stage_id, number, agent, model_ref, tier, billing, state) VALUES (?1, ?2, ?3, 'opencode', ?4, 1, 'paid', ?5)",
                    params![format!("a{n}_{i}"), format!("s{n}"), i as i64 + 1, format!("OpenCode · pygg/{model}"), state],
                )
                .unwrap();
            }
        };
        let gemini = "openrouter/google/gemini-3-pro";
        for _ in 0..6 {
            task("code", gemini, &["passed"], Some("kept"));
        }
        task("code", gemini, &["failed", "passed"], Some("discarded"));
        task("code", gemini, &["failed", "failed"], None);
        task("code", "openrouter/z-ai/glm-4.7-flash:free", &["passed", "passed", "passed"], Some("kept"));
        task("review", "openrouter/anthropic/claude-opus-4.5", &["passed"], Some("kept"));
        let today = days("2026-11-19").unwrap();
        let the_feed: Feed = serde_json::from_value(feed(
            "2026-11-19T00:00:00Z",
            vec![fact(&["claude-opus-4-5"], "code", 1, 10, "2026-10-30", Some("claude-code"), false)],
        ))
        .unwrap();
        let lines = rank_role_on(&conn, Some(&the_feed), "code", today).unwrap();
        let names: Vec<&str> = lines.iter().map(|l| l.model.as_str()).collect();
        assert_eq!(names, [gemini, "openrouter/anthropic/claude-opus-4.5", "openrouter/z-ai/glm-4.7-flash:free"]);
        let own = lines[0].own.as_ref().unwrap();
        assert_eq!(
            (lines[0].layer.as_str(), own.attempts, own.checks_passed, own.kept, own.ranks),
            ("own", 10, 7, 6, true)
        );
        assert_eq!(
            (lines[1].layer.as_str(), lines[1].own.is_none()),
            ("feed", true),
            "a review attempt is not code evidence"
        );
        let glm = lines[2].own.as_ref().unwrap();
        assert_eq!(
            (lines[2].layer.as_str(), glm.attempts, glm.kept, glm.ranks),
            ("none", 3, 1, false),
            "under ten attempts: shown, not ranked"
        );
    }

    fn standing(id: &str, rank: Option<f64>, billing: u8, turn: Option<u64>) -> Standing {
        Standing { own: None, rank, from_fact: rank.is_some(), billing, turn_micro: turn, id: id.into() }
    }

    #[test]
    fn a_band_of_close_facts_goes_to_the_cheaper_model_and_says_whose_band() {
        // The 2026-11-20 case: Terminal-Bench #4 (rank 0.957) and #12 (0.843) of 71, the second 25% cheaper a turn;
        // a third at #30 (0.586) is a band below; a model with neither fact nor tier comes last.
        let items = vec![
            standing("g/carbon", Some(0.957), 3, Some(13_840)),
            standing("g/hydrogen", Some(0.843), 3, Some(10_380)),
            standing("g/flash", Some(0.586), 3, Some(5_000)),
            standing("x/none", None, 1, Some(0)),
        ];
        let order = order_for_role(&items);
        let ids: Vec<&str> = order.iter().map(|(i, _)| items[*i].id.as_str()).collect();
        assert_eq!(ids, ["g/hydrogen", "g/carbon", "g/flash", "x/none"]);
        assert_eq!(order[0].1, Some(0), "hydrogen ranks ahead of carbon, its band's leader");
        assert_eq!(order[1].1, None);
        // A plan model with a tier only (T3 stands at 0.7) leads the next band; flash (0.586) joins it, and the plan
        // model, costing nothing a request, stays first.
        let mut items = items;
        items.push(Standing { from_fact: false, ..standing("q/max", Some(tier_standing("T3")), 2, Some(0)) });
        let ids: Vec<String> = order_for_role(&items).into_iter().map(|(i, _)| items[i].id.clone()).collect();
        assert_eq!(ids, ["g/hydrogen", "g/carbon", "q/max", "g/flash", "x/none"]);
    }

    #[test]
    fn own_evidence_ranks_by_kept_rate_in_bands_then_by_money_per_kept_result() {
        let own = |attempts: u32, kept: u32, money: u64, unknown: u32| Own {
            attempts,
            checks_passed: kept,
            kept,
            ranks: attempts >= OWN_MIN_ATTEMPTS,
            known_micro_usd: money,
            unknown_requests: unknown,
        };
        let with = |id: &str, o: Own| Standing { own: Some(o), ..standing(id, Some(0.5), 3, Some(1_000)) };
        let items = vec![
            with("a/dear", own(10, 10, 900_000, 0)), // 10/10 kept, $0.09 a kept result
            with("b/cheap", own(10, 9, 180_000, 0)), // 9/10, $0.02: one band with a/dear, and cheaper
            with("c/floor", own(10, 10, 50_000, 3)), // 10/10, cheapest known, but 3 requests unpriced: after
            with("d/weak", own(10, 6, 0, 0)),        // 6/10: a band below, though it costs nothing
            with("e/few", own(4, 4, 0, 0)),          // under ten attempts: ranks by its fact (0.5), after own
        ];
        let ids: Vec<String> = order_for_role(&items).into_iter().map(|(i, _)| items[i].id.clone()).collect();
        assert_eq!(ids, ["b/cheap", "a/dear", "c/floor", "d/weak", "e/few"]);
        assert_eq!(own(10, 9, 180_000, 0).micro_usd_per_kept(), Some(20_000));
    }

    #[test]
    fn a_typical_turn_reads_four_fifths_from_the_cache_where_it_is_priced() {
        let price = |input: u64, cached: Option<u64>, output: u64| super::super::pricing::Price {
            input_micro_per_mtok: input,
            output_micro_per_mtok: output,
            cached_input_micro_per_mtok: cached,
            cache_write_micro_per_mtok: None,
            source: "t".into(),
            observed_at: 0,
        };
        // Gemini 4 Carbon and Hydrogen at their 2026-11 list prices.
        assert_eq!(typical_turn_micro(&price(2_000_000, Some(100_000), 10_000_000)), 13_840);
        assert_eq!(typical_turn_micro(&price(1_500_000, Some(75_000), 7_500_000)), 10_380);
        assert_eq!(typical_turn_micro(&price(1_000_000, None, 1_000_000)), 9_000);
        assert_eq!(turn_cost("subscription", None), Some(0));
        assert_eq!(turn_cost("paid", None), None);
    }

    #[test]
    fn days_count_civil_dates() {
        assert_eq!(days("1970-01-01"), Some(0));
        assert_eq!(days("2000-03-01"), Some(11017));
        assert_eq!(days("2026-11-19T05:17:11Z"), days("2026-11-19"));
        assert_eq!(days("2026-13-01"), None);
        assert_eq!(days("soon"), None);
    }
}
