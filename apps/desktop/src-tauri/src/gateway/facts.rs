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

/// One connected model's line for a role: which layer won and the facts behind it.
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
    /// `feed`, `tier`, or `none` (the own-evidence layer fills once attempts carry a role, BRAIN.md §1).
    pub layer: String,
    /// The fact the rule used, when the feed layer won.
    pub fact: Option<FactView>,
    /// Every other fact for this model and role, the ones too old to rank included.
    pub others: Vec<FactView>,
    /// The list price per million output tokens, micro-dollars, for the order within a layer.
    pub output_micro_per_mtok: Option<u64>,
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
    let mut lines = Vec::new();
    for m in store::list_models(conn, None)?.into_iter().filter(|m| m.enabled) {
        if !providers.iter().any(|p| p.id == m.provider_id) {
            continue;
        }
        lines.push(line_for(&m, feed, role, today));
    }
    lines.sort_by(|a, b| {
        let layer = |l: &Line| match l.layer.as_str() {
            "feed" => 0,
            "tier" => 1,
            _ => 2,
        };
        layer(a)
            .cmp(&layer(b))
            .then_with(|| {
                let rank = |l: &Line| l.fact.as_ref().map(|f| f.rank).unwrap_or(0.0);
                rank(b).partial_cmp(&rank(a)).unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| b.tier.cmp(&a.tier))
            .then_with(|| a.output_micro_per_mtok.unwrap_or(u64::MAX).cmp(&b.output_micro_per_mtok.unwrap_or(u64::MAX)))
            .then_with(|| a.model.cmp(&b.model))
    });
    Ok(lines)
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
        fact,
        others,
        output_micro_per_mtok: m.price.as_ref().map(|p| p.output_micro_per_mtok),
    }
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
        assert_eq!(
            names,
            [
                "openrouter/anthropic/claude-opus-4.5",
                "openrouter/z-ai/glm-4.7-flash:free",
                "openrouter/google/gemini-3-pro",
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
        assert_eq!((lines[2].layer.as_str(), lines[2].tier.as_deref()), ("tier", Some("T3")));
        let plan = rank_role_on(&conn, Some(&the_feed), "plan", today).unwrap();
        assert!(plan[0].fact.as_ref().unwrap().derived);
        assert!(rank_role_on(&conn, None, "code", today).unwrap().iter().all(|l| l.layer != "feed"));
        assert!(rank_role_on(&conn, None, "coding", today).is_err());
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
