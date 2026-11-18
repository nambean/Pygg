//! The read commands of `pygg` (docs/design/CLI.md, Codex's review of 2026-11-14): `providers`, `models`, `logs`,
//! `usage`, and what a bare `pygg` shows. They open the store **read-only**: never created, migrated or filled, so they
//! are safe while the app holds it. Each takes `--json`, whose objects are this module's own types (`schemaVersion` 1),
//! never the app's structs: no credential reference, endpoint secret, prompt, session id or raw upstream error.
//!
//! Also the pieces every command shares: a strict option parser, the exit codes and the gateway's port.

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::gateway::store;

/// The version of every `--json` document's shape.
pub const JSON_SCHEMA: u32 = 1;

/// A command that did not do what was asked: its exit code (1 operational, 2 usage, 3 a check found a problem) and
/// what to say, ending with the next thing to try.
#[derive(Debug, PartialEq)]
pub struct Fail {
    pub code: i32,
    pub message: String,
}

impl Fail {
    pub fn usage(message: impl Into<String>) -> Fail {
        Fail { code: 2, message: message.into() }
    }
    pub fn op(message: impl Into<String>) -> Fail {
        Fail { code: 1, message: message.into() }
    }
}

/// Prints a result's message and returns its exit code.
pub fn finish(result: Result<i32, Fail>) -> i32 {
    match result {
        Ok(code) => code,
        Err(f) => {
            eprintln!("pygg: {}", f.message);
            f.code
        }
    }
}

/// Parsed options: words, and `--name` / `--name value`.
#[derive(Debug, Default)]
pub struct Opts {
    pub words: Vec<String>,
    flags: HashMap<String, Option<String>>,
}

impl Opts {
    /// Parses `args` for `command`, allowing only `switches` (no value) and `valued` (one value). An unknown option,
    /// a missing value or a repeated option is a usage error that says what to type instead.
    pub fn parse(command: &str, args: &[String], switches: &[&str], valued: &[&str]) -> Result<Opts, Fail> {
        let mut opts = Opts::default();
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            let Some(name) = arg.strip_prefix("--") else {
                opts.words.push(arg.clone());
                continue;
            };
            let (name, inline) = match name.split_once('=') {
                Some((n, v)) => (n.to_string(), Some(v.to_string())),
                None => (name.to_string(), None),
            };
            let value = if valued.contains(&name.as_str()) {
                Some(match inline {
                    Some(v) => v,
                    None => iter
                        .next()
                        .cloned()
                        .ok_or_else(|| Fail::usage(format!("--{name} needs a value.\nUse: pygg {command} --help")))?,
                })
            } else if switches.contains(&name.as_str()) && inline.is_none() {
                None
            } else {
                return Err(Fail::usage(format!("unknown option --{name}.\nUse: pygg {command} --help")));
            };
            if opts.flags.insert(name.clone(), value).is_some() {
                return Err(Fail::usage(format!("--{name} is given twice.\nUse: pygg {command} --help")));
            }
        }
        Ok(opts)
    }

    pub fn has(&self, name: &str) -> bool {
        self.flags.contains_key(name)
    }

    pub fn value(&self, name: &str) -> Option<&str> {
        self.flags.get(name).and_then(|v| v.as_deref())
    }

    /// A number option within `range`, or `default`.
    pub fn number(
        &self, command: &str, name: &str, range: std::ops::RangeInclusive<i64>, default: i64,
    ) -> Result<i64, Fail> {
        match self.value(name) {
            None => Ok(default),
            Some(v) => v.parse::<i64>().ok().filter(|n| range.contains(n)).ok_or_else(|| {
                Fail::usage(format!(
                    "--{name} takes a number from {} to {}.\nUse: pygg {command} --help",
                    range.start(),
                    range.end()
                ))
            }),
        }
    }
}

/// The store a command reads: `--db`, or the app's own.
pub fn store_path(opts: &Opts) -> Option<PathBuf> {
    opts.value("db").map(PathBuf::from).or_else(super::default_store)
}

/// Opens the store read-only for a read command. A missing store, an older or newer schema and a store that cannot be
/// opened each say what to do; nothing is created or changed.
pub fn open_read(opts: &Opts) -> Result<Connection, Fail> {
    let path = store_path(opts).ok_or_else(|| Fail::op("no store found: pass --db PATH"))?;
    if !path.is_file() {
        return Err(Fail::op(format!(
            "Pygg's store was not found at {}.\nNext: {}, then {}.",
            path.display(),
            super::MAKE_STORE,
            super::CONNECT_PROVIDER
        )));
    }
    let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|e| Fail::op(format!("could not open {}: {e}", path.display())))?;
    let _ = conn.busy_timeout(Duration::from_secs(2));
    let schema: u32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| Fail::op(format!("could not read {}: {e}", path.display())))?;
    let ours = crate::db::SCHEMA_VERSION;
    if schema < ours {
        return Err(Fail::op(format!(
            "the store is at schema v{schema}, older than this program (v{ours}).\nTo update it, {}. \
            The store was not changed.",
            super::MAKE_STORE
        )));
    }
    if schema > ours {
        return Err(Fail::op(format!(
            "the store is at schema v{schema}, newer than this program (v{ours}).\nUpdate @pygg/cli \
            (npm install -g @pygg/cli@dev). The store was not changed."
        )));
    }
    Ok(conn)
}

/// The gateway's port: `--port`, then the store's setting, then 8741.
pub fn gateway_port(opts: &Opts, conn: Option<&Connection>) -> Result<u16, Fail> {
    if let Some(p) = opts.value("port") {
        return p
            .parse()
            .ok()
            .filter(|p| *p > 0)
            .ok_or_else(|| Fail::usage(format!("--port {p} is not a port number.")));
    }
    Ok(conn
        .and_then(|c| store::setting(c, crate::gateway::PORT_KEY).ok().flatten())
        .and_then(|p| p.parse().ok())
        .unwrap_or(crate::gateway::DEFAULT_PORT))
}

fn print_json(value: Value) -> i32 {
    println!("{}", serde_json::to_string_pretty(&value).unwrap_or_default());
    0
}

fn when(ms: i64) -> String {
    store::iso_utc(ms)[..19].replace('T', " ")
}

fn ago(ms: i64) -> String {
    let secs = (crate::db::now_ms() - ms).max(0) / 1000;
    match secs {
        0..=59 => "just now".into(),
        60..=3_599 => format!("{} min ago", secs / 60),
        3_600..=86_399 => format!("{} h ago", secs / 3_600),
        _ => format!("{} d ago", secs / 86_400),
    }
}

/// Strips control characters from text that came from outside (a provider's message, a label), so it cannot move
/// the terminal's cursor or change its colours.
fn clean(text: &str) -> String {
    text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect()
}

fn table(headers: &[&str], rows: &[Vec<String>]) {
    let widths: Vec<usize> = (0..headers.len())
        .map(|i| rows.iter().map(|r| r[i].chars().count()).chain([headers[i].len()]).max().unwrap_or(0))
        .collect();
    let line = |cells: Vec<String>| {
        let last = cells.len() - 1;
        cells
            .iter()
            .enumerate()
            .map(|(i, c)| if i == last { c.clone() } else { format!("{c:<w$}", w = widths[i]) })
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_string()
    };
    println!("{}", line(headers.iter().map(|h| h.to_string()).collect()));
    for row in rows {
        println!("{}", line(row.clone()));
    }
}

pub(crate) fn money(micro: u64) -> String {
    let usd = micro as f64 / 1e6;
    if micro > 0 && usd < 0.0001 {
        "<$0.0001".into()
    } else if usd >= 0.01 || micro == 0 {
        format!("${usd:.2}")
    } else {
        format!("${usd:.4}")
    }
}

fn compact(n: u64) -> String {
    match n {
        0..=9_999 => n.to_string(),
        10_000..=999_999 => format!("{:.1}K", n as f64 / 1e3),
        _ => format!("{:.1}M", n as f64 / 1e6),
    }
}

// ---------- providers ----------

pub const PROVIDERS_HELP: &str = "usage: pygg providers [list] [--json] [--db PATH]

The connections saved in Pygg, read from its store: whether each is on, its last check, its health over the last 24
hours and its models (enabled/total). Nothing is sent to any provider.

  --json   the same as a JSON document (schemaVersion 1)

Connect, check or change providers in the Pygg desktop app (Providers).";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderOut {
    id: String,
    label: String,
    kind: String,
    enabled: bool,
    billing: String,
    /// `openai` or `anthropic`.
    api_format: String,
    models_enabled: usize,
    models_total: usize,
    last_check: Option<CheckOut>,
    /// 0–100 over the last 24 hours; null when nothing happened.
    health_score: Option<u8>,
    /// What keeps the whole connection from answering now, if anything.
    blocked: Option<BlockOut>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckOut {
    at: String,
    ok: bool,
    detail: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BlockOut {
    state: String,
    reason: String,
}

fn providers_of(conn: &Connection) -> Result<Vec<ProviderOut>, Fail> {
    let read = |e: String| Fail::op(format!("could not read the providers: {e}"));
    let models = store::list_models(conn, None).map_err(read)?;
    let now = crate::db::now_ms();
    store::list_providers(conn)
        .map_err(read)?
        .into_iter()
        .map(|p| {
            let mine: Vec<_> = models.iter().filter(|m| m.provider_id == p.id).collect();
            let check = store::last_check(conn, &p.id).map_err(read)?;
            let strip = store::health_strip(conn, &p.id, now).map_err(read)?;
            let blocked = store::list_health(conn, Some(&p.id))
                .map_err(read)?
                .into_iter()
                .find(|h| h.model_id.is_empty() && h.until_ms.is_none_or(|u| u > now))
                .map(|h| BlockOut { state: h.state, reason: clean(&h.reason) });
            Ok(ProviderOut {
                id: p.id.clone(),
                label: clean(&p.label),
                kind: p.kind.clone(),
                enabled: p.enabled,
                billing: p.default_billing.clone(),
                api_format: p.wire.clone(),
                models_enabled: mine.iter().filter(|m| m.enabled).count(),
                models_total: mine.len(),
                last_check: check.map(|c| CheckOut { at: store::iso_utc(c.at), ok: c.ok, detail: clean(&c.detail) }),
                health_score: strip.score,
                blocked,
            })
        })
        .collect()
}

/// The enabled providers that need attention, one line each: the whole connection blocked now, or its last check
/// failed. For `doctor`.
pub fn attention(conn: &Connection) -> Vec<String> {
    providers_of(conn)
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.enabled)
        .filter_map(|p| {
            if let Some(b) = &p.blocked {
                return Some(format!("{}: {} ({})", p.id, b.state.replace('_', " "), b.reason));
            }
            p.last_check.as_ref().filter(|c| !c.ok).map(|c| format!("{}: last check failed: {}", p.id, c.detail))
        })
        .collect()
}

pub fn providers(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("providers", args, &["json"], &["db"])?;
    match opts.words.first().map(String::as_str) {
        None | Some("list") if opts.words.len() <= 1 => {}
        Some(other) => {
            return Err(Fail::usage(format!(
                "pygg providers {other}: not a providers command.\nUse: pygg providers list \
            (to add one, {})",
                super::CONNECT_PROVIDER
            )))
        }
        None => {}
    }
    let conn = open_read(&opts)?;
    let list = providers_of(&conn)?;
    if opts.has("json") {
        return Ok(print_json(json!({ "schemaVersion": JSON_SCHEMA, "providers": list })));
    }
    if list.is_empty() {
        println!("No providers yet. Next: {}.", super::CONNECT_PROVIDER);
        return Ok(0);
    }
    println!("Saved connections - nothing was sent to any provider\n");
    let rows: Vec<Vec<String>> = list
        .iter()
        .map(|p| {
            vec![
                p.id.clone(),
                if p.enabled { "yes".into() } else { "no".into() },
                p.billing.clone(),
                match &p.last_check {
                    None => "never".into(),
                    Some(c) => format!("{}, {}", if c.ok { "OK" } else { "FAILED" }, ago(chrono_ms(&c.at))),
                },
                p.health_score.map(|s| s.to_string()).unwrap_or_else(|| "-".into()),
                format!("{}/{}", p.models_enabled, p.models_total),
                p.blocked.as_ref().map(|b| b.state.replace('_', " ")).unwrap_or_default(),
            ]
        })
        .collect();
    table(&["ID", "ON", "BILLING", "LAST CHECK", "HEALTH", "MODELS", "NOW"], &rows);
    println!("\nMODELS is enabled/total. HEALTH is 0-100 over the last 24 hours, not a live test.");
    println!("Check or change providers in the Pygg desktop app (Providers); `pygg models` lists their models.");
    Ok(0)
}

/// Milliseconds from the ISO time this module printed (`iso_utc`), for "how long ago".
fn chrono_ms(iso: &str) -> i64 {
    // iso_utc's own inverse is not needed elsewhere; parse its fixed shape.
    let p = |a: usize, b: usize| iso.get(a..b).and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    let (y, m, d) = (p(0, 4), p(5, 7), p(8, 10));
    let (y2, m2) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * m2 + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    days * 86_400_000 + p(11, 13) * 3_600_000 + p(14, 16) * 60_000 + p(17, 19) * 1000 + p(20, 23)
}

// ---------- models ----------

pub const MODELS_HELP: &str = "usage: pygg models [list] [CONNECTION] [--search TEXT] [--all] [--limit N] [--json] [--db PATH]

The models Pygg can route to, read from its store: the name to ask for (CONNECTION/MODEL), billing and price. Only
enabled models of enabled connections, unless --all. Nothing is sent: a listed model has not been tested by this command.

  CONNECTION      only this connection's models
  --search TEXT   only models whose name contains TEXT
  --all           disabled models and connections too
  --limit N       at most N rows (50 by default, 0 for all); --json lists all unless --limit is given
  --json          the same as a JSON document (schemaVersion 1)

Coding tools can also ask for auto or auto/cheap, and Pygg picks an eligible model for each request.";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelOut {
    /// What a client asks for: `connection/model`.
    route: String,
    connection: String,
    model: String,
    enabled: bool,
    billing: String,
    /// USD per million tokens; null when no price is known (cost then stays unknown, never $0).
    price: Option<PriceOut>,
    tier: Option<String>,
    context_tokens: Option<u64>,
    supports_tools: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PriceOut {
    input_usd_per_mtok: f64,
    output_usd_per_mtok: f64,
    cached_input_usd_per_mtok: Option<f64>,
    source: String,
}

pub fn models(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("models", args, &["json", "all"], &["db", "search", "limit"])?;
    let limit = opts.number("models", "limit", 0..=100_000, if opts.has("json") { 0 } else { 50 })? as usize;
    let mut words = opts.words.clone();
    if words.first().map(String::as_str) == Some("list") {
        words.remove(0);
    }
    if words.len() > 1 {
        return Err(Fail::usage("name at most one connection.\nUse: pygg models [CONNECTION] [--search TEXT]"));
    }
    let conn = open_read(&opts)?;
    let read = |e: String| Fail::op(format!("could not read the models: {e}"));
    let providers = store::list_providers(&conn).map_err(read)?;
    if let Some(c) = words.first() {
        if !providers.iter().any(|p| &p.id == c) {
            return Err(Fail::op(format!("no connection named {c}.\nUse: pygg providers (to see their names)")));
        }
    }
    let search = opts.value("search").map(str::to_lowercase);
    let all = opts.has("all");
    let usd = |micro: u64| micro as f64 / 1e6;
    let list: Vec<ModelOut> = store::list_models(&conn, words.first().map(String::as_str))
        .map_err(read)?
        .into_iter()
        .filter(|m| {
            let on = m.enabled && providers.iter().any(|p| p.id == m.provider_id && p.enabled);
            (all || on)
                && search
                    .as_ref()
                    .is_none_or(|s| m.model_id.to_lowercase().contains(s) || m.provider_id.to_lowercase().contains(s))
        })
        .map(|m| ModelOut {
            route: format!("{}/{}", m.provider_id, m.model_id),
            enabled: m.enabled && providers.iter().any(|p| p.id == m.provider_id && p.enabled),
            billing: m.billing.clone(),
            price: m.price.as_ref().map(|p| PriceOut {
                input_usd_per_mtok: usd(p.input_micro_per_mtok),
                output_usd_per_mtok: usd(p.output_micro_per_mtok),
                cached_input_usd_per_mtok: p.cached_input_micro_per_mtok.map(usd),
                source: p.source.clone(),
            }),
            tier: m.capabilities.tier.clone(),
            context_tokens: m.capabilities.context_tokens,
            supports_tools: m.capabilities.supports_tools,
            connection: m.provider_id,
            model: m.model_id,
        })
        .collect();
    if opts.has("json") {
        let total = list.len();
        let list: Vec<ModelOut> = if limit == 0 { list } else { list.into_iter().take(limit).collect() };
        return Ok(print_json(
            json!({ "schemaVersion": JSON_SCHEMA, "aliases": crate::gateway::auto::ALIASES, "total": total,
            "models": list }),
        ));
    }
    let total = list.len();
    let list: Vec<ModelOut> = if limit == 0 { list } else { list.into_iter().take(limit).collect() };
    if list.is_empty() {
        println!(
            "{}",
            if providers.is_empty() {
                format!("No providers yet. Next: {}.", super::CONNECT_PROVIDER)
            } else if search.is_some() || !words.is_empty() {
                "No model matches.".to_string()
            } else {
                "No enabled models. Enable models in the Pygg desktop app (Providers) or with `pygg gateway model`, or use --all.".to_string()
            }
        );
        return Ok(0);
    }
    println!("{} models{}\n", if all { "All" } else { "Enabled" }, if all { "" } else { " of enabled connections" });
    let price = |m: &ModelOut| match &m.price {
        Some(p) => format!("${:.2} in / ${:.2} out per 1M", p.input_usd_per_mtok, p.output_usd_per_mtok),
        None if m.billing == "free" || m.billing == "local" => "-".into(),
        None if m.billing == "subscription" => "plan quota".into(),
        None => "no price (cost unknown)".into(),
    };
    let rows: Vec<Vec<String>> = list
        .iter()
        .map(|m| {
            let mut row =
                vec![m.route.clone(), m.billing.clone(), price(m), m.tier.clone().unwrap_or_else(|| "-".into())];
            if all {
                row.insert(1, if m.enabled { "yes".into() } else { "no".into() });
            }
            row
        })
        .collect();
    let mut headers = vec!["ROUTE", "BILLING", "PRICE", "TIER"];
    if all {
        headers.insert(1, "ON");
    }
    table(&headers, &rows);
    if limit > 0 && total > limit {
        println!("\n{limit} of {total} shown: narrow with --search TEXT or a CONNECTION, or show all with --limit 0.");
    }
    println!("\nAsk for a ROUTE by name, or for auto or auto/cheap to let Pygg choose. Listed models were not tested by this command.");
    Ok(0)
}

// ---------- logs ----------

pub const LOGS_HELP: &str = "usage: pygg logs [list] [--limit N] [--client NAME] [--failed] [--json] [--db PATH]

The gateway's recent request attempts, newest first, from Pygg's ledger (times in UTC). A fallback shows as more than
one attempt. No prompt, key or session id is shown.

  --limit N        how many (1 to 500; 20 by default)
  --client NAME    only this Pygg API key's requests (its name, as Pygg shows it)
  --failed         only attempts that did not answer
  --json           the same as a JSON document (schemaVersion 1)";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AttemptOut {
    at: String,
    client: Option<String>,
    /// What the client asked for: a model, a combo, auto.
    requested: Option<String>,
    /// What answered, or was tried: `connection/model`.
    route: String,
    outcome: String,
    http_status: Option<u16>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    /// `actual`, `estimated` or `unknown`; `costMicroUsd` is null when unknown, never 0.
    cost_status: String,
    cost_micro_usd: Option<u64>,
    latency_ms: Option<u64>,
}

fn attempts(conn: &Connection, client: Option<&str>, failed: bool, limit: i64) -> Result<Vec<AttemptOut>, Fail> {
    let read = |e: rusqlite::Error| Fail::op(format!("could not read the ledger: {e}"));
    // Filters apply before the limit, so the last N failures are the last N failures.
    let mut stmt = conn.prepare("SELECT u.created_at, c.name, u.requested_model, u.provider, u.model, u.outcome, u.http_status, \
        u.input_tokens, u.output_tokens, u.cost_status, u.cost_micro_usd, u.latency_ms \
        FROM usage_events u LEFT JOIN gateway_clients c ON c.id = u.client_id \
        WHERE (?1 IS NULL OR c.name = ?1) AND (?2 = 0 OR u.outcome <> 'ok') ORDER BY u.created_at DESC, u.rowid DESC LIMIT ?3")
        .map_err(read)?;
    let rows = stmt
        .query_map(rusqlite::params![client, failed as i64, limit], |r| {
            let n = |i: usize| r.get::<_, Option<i64>>(i).map(|v| v.map(|v| v.max(0) as u64));
            let status: String = r.get(9)?;
            Ok(AttemptOut {
                at: store::iso_utc(r.get(0)?),
                client: r.get::<_, Option<String>>(1)?.map(|s| clean(&s)),
                requested: r.get::<_, Option<String>>(2)?.map(|s| clean(&s)),
                route: clean(&format!("{}/{}", r.get::<_, String>(3)?, r.get::<_, String>(4)?)),
                outcome: r.get(5)?,
                http_status: r.get::<_, Option<i64>>(6)?.and_then(|s| u16::try_from(s).ok()),
                input_tokens: n(7)?,
                output_tokens: n(8)?,
                cost_micro_usd: if status == "unknown" { None } else { n(10)? },
                cost_status: status,
                latency_ms: n(11)?,
            })
        })
        .map_err(read)?;
    rows.collect::<rusqlite::Result<_>>().map_err(read)
}

pub fn logs(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("logs", args, &["json", "failed"], &["db", "limit", "client"])?;
    match opts.words.as_slice() {
        [] => {}
        [w] if w == "list" => {}
        _ => return Err(Fail::usage("pygg logs takes options only.\nUse: pygg logs --limit 20")),
    }
    let limit = opts.number("logs", "limit", 1..=500, 20)?;
    let conn = open_read(&opts)?;
    let list = attempts(&conn, opts.value("client"), opts.has("failed"), limit)?;
    if opts.has("json") {
        return Ok(print_json(json!({ "schemaVersion": JSON_SCHEMA, "attempts": list })));
    }
    if list.is_empty() {
        println!(
            "{}",
            if opts.has("failed") || opts.value("client").is_some() {
                "No attempt matches."
            } else {
                "No requests yet. Set up a coding tool (pygg setup) and send one."
            }
        );
        return Ok(0);
    }
    println!("Recent gateway attempts - newest first, times UTC\n");
    let rows: Vec<Vec<String>> = list
        .iter()
        .map(|a| {
            vec![
                when(chrono_ms(&a.at)),
                a.client.clone().unwrap_or_else(|| "-".into()),
                a.route.clone(),
                match (a.outcome.as_str(), a.http_status) {
                    ("ok", _) => "OK".into(),
                    ("fallback", Some(s)) => format!("fell back ({s})"),
                    ("fallback", None) => "fell back".into(),
                    (o, Some(s)) if s != 200 => format!("{} ({s})", o.replace('_', " ")),
                    (o, _) => o.replace('_', " "),
                },
                match (a.input_tokens, a.output_tokens) {
                    (None, None) => "-".into(),
                    (i, o) => format!("{}/{}", compact(i.unwrap_or(0)), compact(o.unwrap_or(0))),
                },
                match a.cost_micro_usd {
                    Some(m) => money(m),
                    None => "unknown".into(),
                },
                a.latency_ms.map(|ms| format!("{:.1} s", ms as f64 / 1000.0)).unwrap_or_else(|| "-".into()),
            ]
        })
        .collect();
    table(&["TIME", "CLIENT", "ROUTE", "RESULT", "TOKENS IN/OUT", "COST", "TOOK"], &rows);
    println!(
        "\n{} shown. A fallback is more than one attempt. Usage in the Pygg app has each request's details.",
        list.len()
    );
    Ok(0)
}

// ---------- usage ----------

pub const USAGE_HELP: &str = "usage: pygg usage [--days 1|7|30] [--json] [--db PATH]

What went through the gateway: requests, known cost (actual and estimated) and requests of unknown cost, kept apart;
where the tokens went (new input, cache reads, cache writes, output); and the clients and models that used the most.

  --days N   the period: 1, 7 (default) or 30 days
  --json     the same as a JSON document (schemaVersion 1)";

pub fn usage(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("usage", args, &["json"], &["db", "days"])?;
    if !opts.words.is_empty() {
        return Err(Fail::usage("pygg usage takes options only.\nUse: pygg usage --days 7"));
    }
    let days = opts.number("usage", "days", 1..=366, 7)?;
    let conn = open_read(&opts)?;
    let since = crate::db::now_ms() - days * 86_400_000;
    let read = |e: String| Fail::op(format!("could not read the ledger: {e}"));
    let summary = store::usage_summary(&conn, since, 0).map_err(read)?;
    let mix = store::token_mix(&conn, since).map_err(read)?;
    let t = &summary.total;
    if opts.has("json") {
        let group = |g: &store::UsageGroup| {
            json!({ "key": clean(&g.key), "requests": g.requests, "failed": g.failed,
            "fallbacks": g.fallbacks, "knownMicroUsd": g.known_micro_usd, "unknownCostRequests": g.unknown_requests })
        };
        return Ok(print_json(json!({
            "schemaVersion": JSON_SCHEMA, "days": days,
            "requests": t.requests, "failed": t.failed, "fallbacks": t.fallbacks,
            "knownMicroUsd": t.known_micro_usd, "actualMicroUsd": t.actual_micro_usd, "unknownCostRequests": t.unknown_requests,
            "tokens": mix.tokens, "tokenCostMicroUsd": mix.cost, "tokenCostNote": "known cost split by each model's current prices",
            "byClient": summary.by_client.iter().map(group).collect::<Vec<_>>(),
            "byModel": summary.by_model.iter().map(group).collect::<Vec<_>>(),
        })));
    }
    if t.requests == 0 {
        println!("No gateway requests in the last {days} days. Set up a coding tool (pygg setup) and send one.");
        return Ok(0);
    }
    println!("Last {days} days through Pygg's gateway\n");
    println!("Requests     {} ({} failed, {} fell back)", t.requests, t.failed, t.fallbacks);
    println!(
        "Known cost   {} ({} actual, {} estimated)",
        money(t.known_micro_usd),
        money(t.actual_micro_usd),
        money(t.known_micro_usd - t.actual_micro_usd)
    );
    if t.unknown_requests > 0 {
        println!(
            "Unknown      {} requests of unknown cost, not in the total (give their models a price in the app)",
            t.unknown_requests
        );
    }
    let input = mix.tokens.new_input + mix.tokens.cache_read + mix.tokens.cache_write;
    println!(
        "Tokens       {} new input, {} cache reads, {} cache writes, {} output{}",
        compact(mix.tokens.new_input),
        compact(mix.tokens.cache_read),
        compact(mix.tokens.cache_write),
        compact(mix.tokens.output),
        (mix.tokens.cache_read * 100)
            .checked_div(input)
            .map(|p| format!(" ({p}% of input from cache)"))
            .unwrap_or_default()
    );
    for (title, groups) in [("By client", &summary.by_client), ("By model", &summary.by_model)] {
        println!("\n{title}");
        let rows: Vec<Vec<String>> = groups
            .iter()
            .take(8)
            .map(|g| {
                vec![
                    if g.key.is_empty() { "-".into() } else { clean(&g.key) },
                    g.requests.to_string(),
                    money(g.known_micro_usd),
                    if g.unknown_requests > 0 { format!("+{} unknown", g.unknown_requests) } else { String::new() },
                ]
            })
            .collect();
        table(&["NAME", "REQUESTS", "KNOWN COST", ""], &rows);
    }
    Ok(0)
}

// ---------- a bare `pygg` ----------

/// What a bare `pygg` shows: the gateway, the store, providers and tools, and the next thing to type. Quick: one
/// loopback probe and a few reads, no agent detection or provider requests.
pub async fn home() -> i32 {
    println!("Pygg - one local gateway for your AI coding tools\n");
    let opts = Opts::default();
    let conn = open_read(&opts);
    let port = gateway_port(&opts, conn.as_ref().ok()).unwrap_or(crate::gateway::DEFAULT_PORT);
    let gateway = super::health_quick(port).await;
    match &gateway {
        Ok(()) => println!("Gateway    running at http://127.0.0.1:{port}"),
        Err(e) => println!("Gateway    not running at http://127.0.0.1:{port} ({e})"),
    }
    let conn = match conn {
        Ok(c) => c,
        Err(f) => {
            println!("Store      {}", f.message.lines().next().unwrap_or_default());
            println!("\n{}", super::GET_STARTED);
            println!("Then: pygg setup");
            println!("\n`pygg --help` lists every command.");
            return 0;
        }
    };
    let providers = providers_of(&conn).unwrap_or_default();
    let on = providers.iter().filter(|p| p.enabled).count();
    let models: usize = providers.iter().filter(|p| p.enabled).map(|p| p.models_enabled).sum();
    let failing = providers
        .iter()
        .filter(|p| p.enabled && (p.blocked.is_some() || p.last_check.as_ref().is_some_and(|c| !c.ok)))
        .count();
    println!(
        "Providers  {on} on, {models} models enabled{}",
        if failing > 0 {
            format!(", {failing} {} attention (pygg doctor)", if failing == 1 { "needs" } else { "need" })
        } else {
            String::new()
        }
    );
    let tools = super::setup::tool_states(&conn).await;
    let set_up: Vec<&str> = tools.iter().filter(|t| t.set_up).map(|t| t.name).collect();
    println!(
        "Tools      {}",
        if set_up.is_empty() { "none set up".to_string() } else { format!("set up: {}", set_up.join(", ")) }
    );
    println!();
    if gateway.is_err() {
        println!("Next: {}.", super::START_GATEWAY);
    } else if on == 0 {
        println!("Next: {}.", super::CONNECT_PROVIDER);
    } else if set_up.is_empty() {
        println!("Next:\n  pygg setup claude     (or codex, opencode)\n  pygg run claude");
    } else {
        println!("Next:\n  pygg run {}\n  pygg logs              what went through, newest first", set_up[0]);
    }
    println!("\n`pygg --help` lists every command.");
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn options_are_strict_and_say_what_to_type() {
        let o = Opts::parse(
            "logs",
            &strings(&["list", "--limit", "5", "--json", "--client=Codex"]),
            &["json"],
            &["limit", "client"],
        )
        .unwrap();
        assert_eq!(o.words, vec!["list"]);
        assert_eq!((o.value("limit"), o.value("client"), o.has("json")), (Some("5"), Some("Codex"), true));
        let e = Opts::parse("logs", &strings(&["--limt", "5"]), &["json"], &["limit"]).unwrap_err();
        assert_eq!(e.code, 2);
        assert!(
            e.message.starts_with("unknown option --limt.") && e.message.contains("pygg logs --help"),
            "{}",
            e.message
        );
        assert!(Opts::parse("logs", &strings(&["--limit"]), &[], &["limit"])
            .unwrap_err()
            .message
            .contains("needs a value"));
        assert!(Opts::parse("logs", &strings(&["--json", "--json"]), &["json"], &[])
            .unwrap_err()
            .message
            .contains("twice"));
        assert!(Opts::parse("logs", &strings(&["--json=yes"]), &["json"], &[]).is_err(), "a switch takes no value");
        let o = Opts::parse("logs", &strings(&["--limit", "900"]), &[], &["limit"]).unwrap();
        assert_eq!(o.number("logs", "limit", 1..=500, 20).unwrap_err().code, 2);
    }

    #[test]
    fn a_missing_or_newer_store_is_refused_without_being_touched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.sqlite3");
        let opts = Opts::parse("logs", &strings(&["--db", path.to_str().unwrap()]), &[], &["db"]).unwrap();
        let e = open_read(&opts).unwrap_err();
        assert!(e.message.contains("not found") && !path.exists(), "nothing created: {}", e.message);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(&format!("PRAGMA user_version = {}", crate::db::SCHEMA_VERSION + 1)).unwrap();
        }
        assert!(open_read(&opts).unwrap_err().message.contains("Update @pygg/cli"));
    }

    #[test]
    fn the_read_commands_list_what_the_store_holds_without_secrets() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.sqlite3");
        {
            let conn = crate::db::open(&path).unwrap();
            let p = store::add_provider(
                &conn,
                &store::NewProvider {
                    kind: "gemini",
                    label: "Google Gemini",
                    endpoint: "https://example.test/v1",
                    credential_ref: Some("provider-secret-ref"),
                    default_billing: "free",
                    wire: "openai",
                },
            )
            .unwrap();
            store::add_model(&conn, &p.id, "flash").unwrap();
            let (client, _) = store::create_client(&conn, "Codex", None).unwrap();
            for (outcome, status) in [("fallback", 429), ("ok", 200)] {
                store::record_usage(
                    &conn,
                    &store::UsageRecord {
                        client_id: Some(&client.id),
                        provider_id: &p.id,
                        model_id: "flash",
                        requested_model: "auto",
                        input_tokens: (outcome == "ok").then_some(1200),
                        output_tokens: (outcome == "ok").then_some(80),
                        cache_tokens: None,
                        cost: &crate::gateway::pricing::Cost::Estimated { micro_usd: 0, source: "free".into() },
                        outcome,
                        http_status: Some(status),
                        latency_ms: 900,
                        error: Some("upstream \u{1b}[31msecret"),
                        cache_write_tokens: None,
                        session_key: Some("launch-abc"),
                        route_mode: None,
                        route_reason: None,
                        facts: None,
                        repeated_tokens: None,
                    },
                )
                .unwrap();
            }
        }
        let conn =
            open_read(&Opts::parse("x", &strings(&["--db", path.to_str().unwrap()]), &[], &["db"]).unwrap()).unwrap();
        let providers = providers_of(&conn).unwrap();
        assert_eq!((providers[0].models_enabled, providers[0].models_total), (1, 1));
        let text = serde_json::to_string(&providers).unwrap();
        assert!(!text.contains("provider-secret-ref") && !text.contains("example.test"), "{text}");
        let list = attempts(&conn, Some("Codex"), true, 20).unwrap();
        assert_eq!(list.len(), 1, "filters apply before the limit");
        assert_eq!((list[0].outcome.as_str(), list[0].http_status), ("fallback", Some(429)));
        let all = serde_json::to_string(&attempts(&conn, None, false, 20).unwrap()).unwrap();
        assert!(!all.contains("launch-abc") && !all.contains("secret"), "no session or upstream error: {all}");
        assert!(attempts(&conn, Some("nobody"), false, 20).unwrap().is_empty());
        assert_eq!(chrono_ms(&store::iso_utc(1_794_469_641_058)), 1_794_469_641_058);
        assert_eq!(clean("a\u{1b}[31mb"), "a [31mb");
    }
}
