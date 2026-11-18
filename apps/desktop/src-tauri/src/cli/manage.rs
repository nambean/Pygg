//! The commands that look after Pygg from the terminal without the window (owner's request, 2026-11-18): a model's
//! facts and a test answer, turning models and connections on and off, adding and removing an API connection, the
//! default model for `chat` and `ask`, what a router alias would choose, and the API keys, combos, `pygg run` launches
//! and savings the app shows.
//!
//! Commands that change something open the store read-write without migrating it, and only at the schema this program
//! knows; a running app and its gateway see the change at their next request, as they read the store. Web sessions are
//! connected in the app, which opens the site's sign-in window. Keys are read from an environment variable, never
//! from the command line or a prompt (docs/design/CLI.md).

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};
use serde_json::json;

use super::read::{money, open_read, print_json, store_path, table, Fail, Opts, JSON_SCHEMA};
use crate::gateway::{self, presets, store};

/// The setting holding the model `chat` and `ask` use when given none.
pub const DEFAULT_MODEL: &str = "cli.default_model";

/// Opens the store to change it: never created or migrated, and only at this program's schema.
pub fn open_write(opts: &Opts) -> Result<Connection, Fail> {
    // The read checks say what to do about a missing store or another schema.
    drop(open_read(opts)?);
    let path = store_path(opts).ok_or_else(|| Fail::op("no store found: pass --db PATH"))?;
    let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|e| Fail::op(format!("could not open {}: {e}", path.display())))?;
    let _ = conn.busy_timeout(Duration::from_secs(5));
    Ok(conn)
}

fn op(e: impl std::fmt::Display) -> Fail {
    Fail::op(e.to_string())
}

/// `connection/model` split, or a usage error naming the command.
fn target(command: &str, name: &str) -> Result<(String, String), Fail> {
    name.split_once('/')
        .filter(|(c, m)| !c.is_empty() && !m.is_empty())
        .map(|(c, m)| (c.to_string(), m.to_string()))
        .ok_or_else(|| {
            Fail::usage(format!(
                "{name} is not CONNECTION/MODEL.\nUse: pygg models to see the names, then pygg {command}"
            ))
        })
}

fn find_model(conn: &Connection, name: &str, command: &str) -> Result<store::Model, Fail> {
    let (provider, model) = target(command, name)?;
    store::list_models(conn, Some(&provider)).map_err(op)?.into_iter().find(|m| m.model_id == model).ok_or_else(|| {
        Fail::op(format!(
            "{name} is not in Pygg's models.\nNext: pygg models {provider} --all lists that connection's models."
        ))
    })
}

fn per_mtok(micro: u64) -> String {
    let usd = micro as f64 / 1e6;
    if usd == 0.0 {
        "$0".into()
    } else if usd < 0.01 {
        format!("${usd:.4}")
    } else {
        format!("${usd:.2}")
    }
}

fn yes_no(v: Option<bool>) -> &'static str {
    match v {
        Some(true) => "yes",
        Some(false) => "no",
        None => "unknown",
    }
}

// ---------- models info | test | enable | disable ----------

pub const MODELS_MORE: &str = "
  pygg models info CONNECTION/MODEL      everything Pygg knows about one model, and whether auto can choose it
  pygg models test CONNECTION/MODEL      one short question through the gateway (a paid model costs a little)
  pygg models enable|disable CONNECTION/MODEL…   let the router use it, or not";

/// `pygg models info|show NAME`
pub fn model_info(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("models info", args, &["json"], &["db"])?;
    let [name] = opts.words.as_slice() else {
        return Err(Fail::usage("name one model.\nUse: pygg models info CONNECTION/MODEL"));
    };
    let conn = open_read(&opts)?;
    let m = find_model(&conn, name, "models info CONNECTION/MODEL")?;
    let provider = store::get_provider(&conn, &m.provider_id).map_err(op)?;
    let since = crate::db::now_ms() - 86_400_000;
    let (requests, failed): (i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(outcome <> 'ok'), 0) FROM usage_events WHERE provider = ?1 AND model = ?2 \
             AND created_at >= ?3",
            rusqlite::params![m.provider_id, m.model_id, since],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(op)?;
    let auto = auto_place(&conn, &m);
    let c = &m.capabilities;
    if opts.has("json") {
        return Ok(print_json(json!({
            "schemaVersion": JSON_SCHEMA, "model": format!("{}/{}", m.provider_id, m.model_id), "enabled": m.enabled,
            "connectionEnabled": provider.as_ref().map(|p| p.enabled), "billing": m.billing,
            "price": m.price.as_ref().map(|p| json!({ "inputUsdPerMtok": p.input_micro_per_mtok as f64 / 1e6,
                "outputUsdPerMtok": p.output_micro_per_mtok as f64 / 1e6,
                "cachedUsdPerMtok": p.cached_input_micro_per_mtok.map(|v| v as f64 / 1e6),
                "cacheWriteUsdPerMtok": p.cache_write_micro_per_mtok.map(|v| v as f64 / 1e6), "source": p.source })),
            "tier": c.tier, "contextTokens": c.context_tokens, "maxOutputTokens": c.max_output_tokens,
            "tools": c.supports_tools, "vision": c.supports_vision,
            "last24h": { "requests": requests, "failed": failed },
            "auto": auto.iter().map(|(alias, place)| json!({ "alias": alias, "place": place })).collect::<Vec<_>>(),
        })));
    }
    println!("{}/{}", m.provider_id, m.model_id);
    let on = match (m.enabled, provider.as_ref().map(|p| p.enabled)) {
        (true, Some(true)) => "on".to_string(),
        (false, _) => "off (pygg models enable to turn it on)".into(),
        (true, _) => format!("on, but its connection {} is off", m.provider_id),
    };
    println!("  State       {on}");
    println!("  Billing     {}", m.billing);
    match &m.price {
        Some(p) => println!(
            "  Price       {} in, {} out per million tokens{}{} ({})",
            per_mtok(p.input_micro_per_mtok),
            per_mtok(p.output_micro_per_mtok),
            p.cached_input_micro_per_mtok.map(|v| format!(", {} cached", per_mtok(v))).unwrap_or_default(),
            p.cache_write_micro_per_mtok.map(|v| format!(", {} cache write", per_mtok(v))).unwrap_or_default(),
            p.source
        ),
        None => println!("  Price       none known"),
    }
    println!(
        "  Tier        {}{}",
        c.tier.as_deref().unwrap_or("not set"),
        c.tier_source.as_deref().map(|s| format!(" ({s})")).unwrap_or_default()
    );
    println!(
        "  Limits      context {}, output {}",
        c.context_tokens.map(|n| n.to_string()).unwrap_or("unknown".into()),
        c.max_output_tokens.map(|n| n.to_string()).unwrap_or("unknown".into())
    );
    println!("  Takes       tools {}, images {}", yes_no(c.supports_tools), yes_no(c.supports_vision));
    println!("  Last 24 h   {requests} requests, {failed} failed");
    for (alias, place) in &auto {
        println!("  {alias:<11} {place}");
    }
    println!(
        "\nAsk it: pygg ask --model {}/{} \"…\"   Test it: pygg models test {}/{}",
        m.provider_id, m.model_id, m.provider_id, m.model_id
    );
    Ok(0)
}

/// Where the model stands in `auto` and `auto/cheap` for a plain chat request, by the first API key's rules.
fn auto_place(conn: &Connection, m: &store::Model) -> Vec<(String, String)> {
    let Some(client) = store::list_clients(conn).ok().and_then(|c| c.into_iter().find(|c| c.revoked_at.is_none()))
    else {
        return Vec::new();
    };
    let needs = gateway::auto::Needs { input_tokens: 1000, ..Default::default() };
    ["auto", "auto/cheap"]
        .iter()
        .filter_map(|alias| {
            let strategy = gateway::auto::strategy(alias)?.ok()?;
            let plan = gateway::auto::plan(conn, &client, alias, strategy, &needs).ok()?;
            let ranked = plan.ranked.iter().position(|r| r.provider_id == m.provider_id && r.model_id == m.model_id);
            let place = match ranked {
                Some(i) => format!("choice {} of {}", i + 1, plan.ranked.len()),
                None => match plan.excluded.iter().find(|e| e.provider_id == m.provider_id && e.model_id == m.model_id)
                {
                    Some(e) => format!("left out: {}", e.reason),
                    None => "not considered".into(),
                },
            };
            Some((alias.to_string(), place))
        })
        .collect()
}

/// `pygg models enable|disable NAME…`
pub fn model_switch(args: &[String], on: bool) -> Result<i32, Fail> {
    let verb = if on { "enable" } else { "disable" };
    let opts = Opts::parse(&format!("models {verb}"), args, &[], &["db"])?;
    if opts.words.is_empty() {
        return Err(Fail::usage(format!("name a model.\nUse: pygg models {verb} CONNECTION/MODEL")));
    }
    let conn = open_write(&opts)?;
    for name in &opts.words {
        let m = find_model(&conn, name, &format!("models {verb} CONNECTION/MODEL"))?;
        store::update_model(&conn, &m.provider_id, &m.model_id, Some(on), None, None).map_err(op)?;
        println!("{name} is {}", if on { "on: the router may use it" } else { "off: the router leaves it out" });
    }
    Ok(0)
}

/// `pygg models test NAME`: one short question through the running gateway, as `ask` sends it.
pub async fn model_test(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("models test", args, &[], &["db"])?;
    let [name] = opts.words.as_slice() else {
        return Err(Fail::usage("name one model.\nUse: pygg models test CONNECTION/MODEL"));
    };
    target("models test CONNECTION/MODEL", name)?;
    let mut ask = vec!["--no-input".to_string(), "--model".to_string(), name.clone()];
    if let Some(db) = opts.value("db") {
        ask.extend(["--db".to_string(), db.to_string()]);
    }
    ask.push("Reply with the single word OK.".into());
    Ok(super::ask::ask(ask).await)
}

// ---------- providers presets | add | remove | enable | disable | refresh ----------

pub const PROVIDERS_MORE: &str = "
  pygg providers presets                         the kinds of connection Pygg knows, and their names here
  pygg providers add PRESET [--key-env VAR] [--label NAME] [--endpoint URL] [--billing TYPE]
                                                 connect an API provider; the key is read from the variable VAR
  pygg providers refresh CONNECTION              list its models again
  pygg providers enable|disable CONNECTION       let the router use it, or not
  pygg providers remove CONNECTION --yes         remove it and its keys

Web sessions (Muse, ChatGPT, DeepSeek…) are connected in the app, which opens the site's sign-in window.";

/// `pygg providers presets`
pub fn presets(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("providers presets", args, &["json"], &["db"])?;
    if opts.has("json") {
        return Ok(print_json(
            json!({ "schemaVersion": JSON_SCHEMA, "presets": presets::PRESETS.iter().map(|p| json!({
            "preset": p.id, "name": p.name, "endpoint": p.endpoint, "needsKey": p.needs_key, "billing": p.default_billing,
            "webSession": p.web })).collect::<Vec<_>>() }),
        ));
    }
    let rows: Vec<Vec<String>> = presets::PRESETS
        .iter()
        .map(|p| {
            vec![
                p.id.to_string(),
                p.name.to_string(),
                if p.web {
                    "web session (app only)".into()
                } else if p.needs_key {
                    "API key".into()
                } else {
                    "no key".into()
                },
                p.endpoint.unwrap_or("--endpoint URL").to_string(),
            ]
        })
        .collect();
    table(&["PRESET", "NAME", "SIGN-IN", "ENDPOINT"], &rows);
    println!(
        "\nConnect one: $env:MY_KEY = \"…\"; pygg providers add PRESET --key-env MY_KEY   (bash: export MY_KEY=…)"
    );
    Ok(0)
}

/// `pygg providers add PRESET …`: the connection, its key in the credential store, and its models listed.
pub async fn provider_add(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("providers add", args, &[], &["db", "key-env", "label", "endpoint", "billing"])?;
    let [preset] = opts.words.as_slice() else {
        return Err(Fail::usage(
            "name a preset.\nUse: pygg providers presets, then pygg providers add PRESET --key-env VAR",
        ));
    };
    let preset = presets::find(preset)
        .ok_or_else(|| Fail::usage(format!("{preset} is not a preset.\nUse: pygg providers presets")))?;
    if preset.web {
        return Err(Fail::op(format!(
            "{} is a web session: connect it in the Pygg app (Providers → Connect Provider), which opens the site's \
             sign-in window. Once connected, pygg chat --model CONNECTION/MODEL uses it.",
            preset.name
        )));
    }
    let endpoint =
        opts.value("endpoint").map(String::from).or(preset.endpoint.map(String::from)).ok_or_else(|| {
            Fail::usage(format!("{} needs --endpoint URL (its address, ending in /v1).", preset.name))
        })?;
    let key = match opts.value("key-env") {
        Some(var) => {
            Some(std::env::var(var).map(|k| k.trim().to_string()).ok().filter(|k| !k.is_empty()).ok_or_else(|| {
                Fail::usage(format!("the variable {var} is not set.\nUse (PowerShell): $env:{var} = \"your key\""))
            })?)
        }
        None if preset.needs_key => {
            return Err(Fail::usage(format!(
            "{} needs an API key, read from an environment variable.\nUse (PowerShell): $env:MY_KEY = \"your key\"; \
                 pygg providers add {} --key-env MY_KEY",
            preset.name, preset.id
        )))
        }
        None => None,
    };
    let path = store_path(&opts).ok_or_else(|| Fail::op("no store found: pass --db PATH"))?;
    let conn = open_write(&opts)?;
    let reference = match &key {
        Some(k) => {
            let r = crate::vault::new_reference();
            crate::vault::store(&r, k).map_err(op)?;
            Some(r)
        }
        None => None,
    };
    let label = opts.value("label").unwrap_or(preset.name).to_string();
    let billing = opts.value("billing").unwrap_or(preset.default_billing).to_string();
    let added = store::add_provider(
        &conn,
        &store::NewProvider {
            kind: preset.id,
            label: &label,
            endpoint: &endpoint,
            credential_ref: reference.as_deref(),
            default_billing: &billing,
            wire: preset.wire,
        },
    );
    let provider = match added {
        Ok(p) => p,
        Err(e) => {
            if let Some(r) = &reference {
                let _ = crate::vault::remove(r);
            }
            return Err(op(e));
        }
    };
    println!("Connected {} as {} ({}), billing {billing}.", preset.name, provider.id, provider.endpoint);
    match refresh(conn, &path, &provider.id).await {
        Ok(text) => println!("{text}"),
        Err(e) => println!("Its models could not be listed yet ({e}). Next: pygg providers refresh {}", provider.id),
    }
    println!("Next: pygg models {}   then   pygg chat --model {}/MODEL", provider.id, provider.id);
    Ok(0)
}

/// Lists a connection's models again and fills their facts from the models.dev catalog, as the app does.
async fn refresh(conn: Connection, path: &Path, id: &str) -> Result<String, String> {
    let db = Arc::new(Mutex::new(conn));
    let shared = gateway::Shared::for_store(db, Arc::new(gateway::OsSecrets), path);
    let provider = shared.with_db(|db| store::get_provider(db, id))?.ok_or("no such connection")?;
    let ids = gateway::catalog::fetch_models(&shared, &provider).await?;
    let source = gateway::catalog::list_source(&provider);
    let added = shared.with_db(|db| store::import_listed(db, id, &ids, Some(&source)))?;
    let mut text = format!("{} listed {} models, {added} new.", provider.label, ids.len());
    if let Some(catalog) = gateway::reference::current(&shared.http, &gateway::reference::cache_path(path)).await {
        if let Ok(applied) = shared.with_db(|db| gateway::reference::apply(db, &catalog)) {
            text.push_str(&format!(" Facts and prices filled for {} from models.dev.", applied.filled));
        }
    }
    let _ = shared.with_db(store::fill_curated_tiers);
    Ok(text)
}

/// `pygg providers refresh ID`
pub async fn provider_refresh(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("providers refresh", args, &[], &["db"])?;
    let [id] = opts.words.as_slice() else {
        return Err(Fail::usage("name a connection.\nUse: pygg providers, then pygg providers refresh CONNECTION"));
    };
    let path = store_path(&opts).ok_or_else(|| Fail::op("no store found: pass --db PATH"))?;
    let conn = open_write(&opts)?;
    println!("{}", refresh(conn, &path, id).await.map_err(Fail::op)?);
    Ok(0)
}

/// `pygg providers enable|disable ID`
pub fn provider_switch(args: &[String], on: bool) -> Result<i32, Fail> {
    let verb = if on { "enable" } else { "disable" };
    let opts = Opts::parse(&format!("providers {verb}"), args, &[], &["db"])?;
    let [id] = opts.words.as_slice() else {
        return Err(Fail::usage(format!(
            "name a connection.\nUse: pygg providers, then pygg providers {verb} CONNECTION"
        )));
    };
    let conn = open_write(&opts)?;
    store::get_provider(&conn, id)
        .map_err(op)?
        .ok_or_else(|| Fail::op(format!("{id} is not a connection.\nUse: pygg providers")))?;
    store::update_provider(&conn, id, Some(on), None, None).map_err(op)?;
    println!("{id} is {}", if on { "on" } else { "off: the router leaves its models out" });
    Ok(0)
}

/// `pygg providers remove ID --yes`
pub fn provider_remove(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("providers remove", args, &["yes"], &["db"])?;
    let [id] = opts.words.as_slice() else {
        return Err(Fail::usage("name a connection.\nUse: pygg providers remove CONNECTION --yes"));
    };
    if !opts.has("yes") {
        return Err(Fail::usage(format!(
            "removing {id} deletes the connection and its keys from the credential store. To go ahead: pygg providers \
             remove {id} --yes   (to pause it instead: pygg providers disable {id})"
        )));
    }
    let conn = open_write(&opts)?;
    store::get_provider(&conn, id)
        .map_err(op)?
        .ok_or_else(|| Fail::op(format!("{id} is not a connection.\nUse: pygg providers")))?;
    for reference in store::remove_provider(&conn, id).map_err(op)? {
        let _ = crate::vault::remove(&reference);
    }
    println!("Removed {id} and its keys. Its past requests stay in the ledger.");
    Ok(0)
}

// ---------- default ----------

pub const DEFAULT_HELP: &str = "usage: pygg default [MODEL | --clear] [--db PATH]

The model pygg chat and pygg ask use when no --model is given: CONNECTION/MODEL, a combo, auto or auto/cheap.
Without MODEL it says which one is set. --clear goes back to auto.

  pygg default muse/muse";

/// The default model of `chat` and `ask`, or `auto`.
pub fn default_model(conn: &Connection) -> String {
    store::setting(conn, DEFAULT_MODEL).ok().flatten().filter(|m| !m.trim().is_empty()).unwrap_or_else(|| "auto".into())
}

/// `pygg default [MODEL|--clear]`
pub fn default(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("default", args, &["clear"], &["db"])?;
    match (opts.words.as_slice(), opts.has("clear")) {
        ([], false) => {
            let conn = open_read(&opts)?;
            println!("{}", default_model(&conn));
        }
        ([], true) => {
            let conn = open_write(&opts)?;
            store::set_setting(&conn, DEFAULT_MODEL, "").map_err(op)?;
            println!("pygg chat and pygg ask now use auto.");
        }
        ([model], false) => {
            let conn = open_write(&opts)?;
            let known = model == "auto"
                || model.starts_with("auto/")
                || store::list_combos(&conn).map_err(op)?.iter().any(|c| &c.name == model)
                || find_model(&conn, model, "default MODEL").is_ok();
            if !known {
                return Err(Fail::usage(format!(
                    "{model} is not a model, combo or auto alias Pygg has.\nUse: pygg models, pygg combos"
                )));
            }
            store::set_setting(&conn, DEFAULT_MODEL, model).map_err(op)?;
            println!("pygg chat and pygg ask now use {model} unless --model says otherwise.");
        }
        _ => return Err(Fail::usage("give one model or --clear.\nUse: pygg default --help")),
    }
    Ok(0)
}

// ---------- route ----------

pub const ROUTE_HELP: &str =
    "usage: pygg route [auto|auto/cheap] [--key NAME] [--tools] [--images] [--prompt N] [--json] [--db PATH]

Which models the router would try for a request, best first, and why the others are left out. Nothing is sent.

  --key NAME    by this API key's rules (its minimum tier, allowed connections); the first key otherwise
  --tools       a request that uses tools (coding agents do)
  --images      a request with images
  --prompt N    a prompt of about N tokens (1000 by default)";

/// `pygg route [ALIAS]`
pub fn route(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("route", args, &["tools", "images", "json"], &["db", "key", "prompt"])?;
    let alias = opts.words.first().map(String::as_str).unwrap_or("auto");
    let strategy = gateway::auto::strategy(alias)
        .ok_or_else(|| Fail::usage(format!("{alias} is not a router alias: use auto or auto/cheap.")))?
        .map_err(Fail::usage)?;
    let conn = open_read(&opts)?;
    let clients = store::list_clients(&conn).map_err(op)?;
    let client = match opts.value("key") {
        Some(name) => clients
            .into_iter()
            .find(|c| c.name == name && c.revoked_at.is_none())
            .ok_or_else(|| Fail::usage(format!("{name} is not an API key here.\nUse: pygg keys")))?,
        None => clients.into_iter().find(|c| c.revoked_at.is_none()).ok_or_else(|| {
            Fail::op("there is no API key yet: run pygg ask once, or make one in the app (Endpoint → API Keys)")
        })?,
    };
    let needs = gateway::auto::Needs {
        tools: opts.has("tools"),
        vision: opts.has("images"),
        input_tokens: opts.number("route", "prompt", 1..=2_000_000, 1000)? as u64,
        ..Default::default()
    };
    let plan = gateway::auto::plan(&conn, &client, alias, strategy, &needs).map_err(op)?;
    if opts.has("json") {
        return Ok(print_json(json!({ "schemaVersion": JSON_SCHEMA, "alias": alias, "key": client.name,
            "minTier": plan.min_tier,
            "ranked": plan.ranked.iter().map(|r| json!({ "model": format!("{}/{}", r.provider_id, r.model_id),
                "tier": r.tier, "billing": r.billing, "estimateMicroUsd": r.estimate_micro_usd, "reason": r.reason,
                "tried": plan.route.iter().any(|x| x.provider_id == r.provider_id && x.model_id == r.model_id) })).collect::<Vec<_>>(),
            "excluded": plan.excluded.iter().map(|e| json!({ "model": format!("{}/{}", e.provider_id, e.model_id),
                "reason": e.reason })).collect::<Vec<_>>() })));
    }
    println!(
        "{alias} for {} (minimum tier {}){}\n",
        client.name,
        plan.min_tier.as_deref().unwrap_or("none"),
        if needs.tools { ", with tools" } else { "" }
    );
    if plan.ranked.is_empty() {
        println!("No model is eligible. The reasons are below; pygg models info CONNECTION/MODEL says more about one.");
    }
    let rows: Vec<Vec<String>> = plan
        .ranked
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let tried = plan.route.iter().any(|x| x.provider_id == r.provider_id && x.model_id == r.model_id);
            vec![
                format!("{}{}", i + 1, if tried { " ›" } else { "" }),
                format!("{}/{}", r.provider_id, r.model_id),
                r.tier.clone().unwrap_or("-".into()),
                r.billing.clone(),
                money(r.estimate_micro_usd),
                r.reason.clone(),
            ]
        })
        .collect();
    if !rows.is_empty() {
        table(&["#", "MODEL", "TIER", "BILLING", "EST. COST", "WHY"], &rows);
        println!("› the models a request tries, in order, before it fails");
    }
    if !plan.excluded.is_empty() {
        println!("\nLeft out ({}):", plan.excluded.len());
        for e in plan.excluded.iter().take(12) {
            println!("  {}/{}: {}", e.provider_id, e.model_id, e.reason);
        }
        if plan.excluded.len() > 12 {
            println!("  … {} more (--json lists all)", plan.excluded.len() - 12);
        }
    }
    Ok(0)
}

// ---------- keys, combos, launches, savings ----------

/// `pygg keys`: the API keys (clients), never their secrets.
pub fn keys(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("keys", args, &["json", "all"], &["db"])?;
    let conn = open_read(&opts)?;
    let clients: Vec<store::Client> = store::list_clients(&conn)
        .map_err(op)?
        .into_iter()
        .filter(|c| opts.has("all") || c.revoked_at.is_none())
        .collect();
    if opts.has("json") {
        return Ok(print_json(json!({ "schemaVersion": JSON_SCHEMA, "keys": clients.iter().map(|c| json!({
            "name": c.name, "prefix": c.token_prefix, "createdAt": c.created_at, "lastUsedAt": c.last_used_at,
            "revoked": c.revoked_at.is_some(), "monthlyLimitMicroUsd": c.monthly_budget_micro_usd,
            "monthSpendMicroUsd": c.month_spend_micro_usd, "sessionLimitMicroUsd": c.session_budget_micro_usd,
            "dayLimitMicroUsd": c.day_budget_micro_usd, "dayCountedMicroUsd": c.day_counted_micro_usd,
        })).collect::<Vec<_>>() })));
    }
    let rows: Vec<Vec<String>> = clients
        .iter()
        .map(|c| {
            vec![
                c.name.clone(),
                format!("{}…", c.token_prefix),
                c.last_used_at.map(super::read::ago).unwrap_or("never".into()),
                money(c.month_spend_micro_usd),
                c.monthly_budget_micro_usd.map(money).unwrap_or("none".into()),
                c.session_budget_micro_usd.map(money).unwrap_or("none".into()),
                c.day_budget_micro_usd
                    .map(|l| format!("{} of {}", money(c.day_counted_micro_usd), money(l)))
                    .unwrap_or("none".into()),
                if c.revoked_at.is_some() { "revoked".into() } else { String::new() },
            ]
        })
        .collect();
    table(&["NAME", "KEY", "LAST USED", "THIS MONTH", "MONTHLY LIMIT", "SESSION LIMIT", "TODAY (UTC)", ""], &rows);
    println!(
        "\nKeys are made and limited in the app (Endpoint → API Keys); a key is shown only once, when it is made."
    );
    Ok(0)
}

/// `pygg combos`
pub fn combos(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("combos", args, &["json"], &["db"])?;
    let conn = open_read(&opts)?;
    let combos = store::list_combos(&conn).map_err(op)?;
    if opts.has("json") {
        return Ok(print_json(json!({ "schemaVersion": JSON_SCHEMA, "combos": combos.iter().map(|c| json!({
            "name": c.name, "strategy": c.strategy,
            "members": c.members.iter().map(|m| format!("{}/{}", m.provider_id, m.model_id)).collect::<Vec<_>>() }))
            .collect::<Vec<_>>() })));
    }
    if combos.is_empty() {
        println!("No combos yet. A combo is a list of models tried in turn; make one in the app (Combos).");
        return Ok(0);
    }
    let rows: Vec<Vec<String>> = combos
        .iter()
        .map(|c| {
            vec![
                c.name.clone(),
                c.strategy.clone(),
                c.members.iter().map(|m| format!("{}/{}", m.provider_id, m.model_id)).collect::<Vec<_>>().join(" → "),
            ]
        })
        .collect();
    table(&["NAME", "ORDER", "MODELS"], &rows);
    println!("\nUse one by name: pygg chat --model NAME");
    Ok(0)
}

/// `pygg launches`: the `pygg run` launches, with cost and what changed in Git (launches.rs).
pub fn launches(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("launches", args, &["json", "by-repo"], &["db", "limit", "weeks"])?;
    if opts.has("by-repo") {
        return launches_by_repo(&opts);
    }
    let limit = opts.number("launches", "limit", 1..=500, 20)?;
    let conn = open_read(&opts)?;
    type Row = (
        String,
        String,
        i64,
        Option<i64>,
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
    );
    let mut stmt = conn
        .prepare(
            "SELECT session_key, tool, started_at, finished_at, repo, branch, commits, files_changed, insertions, \
             deletions FROM launches ORDER BY started_at DESC LIMIT ?1",
        )
        .map_err(op)?;
    let rows: Vec<Row> = stmt
        .query_map([limit], |r| {
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
            ))
        })
        .map_err(op)?
        .collect::<rusqlite::Result<_>>()
        .map_err(op)?;
    let mut out = Vec::new();
    for (session, tool, started, finished, repo, branch, commits, files, ins, del) in rows {
        let cost = crate::launches::session_cost(&conn, &session).map_err(op)?;
        out.push((session, tool, started, finished, repo, branch, commits, files, ins, del, cost));
    }
    if opts.has("json") {
        return Ok(print_json(json!({ "schemaVersion": JSON_SCHEMA, "launches": out.iter().map(|l| json!({
            "tool": l.1, "startedAt": l.2, "finishedAt": l.3, "repo": l.4, "branch": l.5, "commits": l.6,
            "filesChanged": l.7, "insertions": l.8, "deletions": l.9, "requests": l.10.answered,
            "knownMicroUsd": l.10.known_micro_usd, "unknownCost": l.10.unknown_cost })).collect::<Vec<_>>() })));
    }
    if out.is_empty() {
        println!("No launches yet. Start a tool through Pygg with pygg run claude (or codex, opencode).");
        return Ok(0);
    }
    let rows: Vec<Vec<String>> = out
        .iter()
        .map(|l| {
            let git = match (&l.4, l.6, l.7) {
                (None, _, _) => "not in Git".to_string(),
                (Some(_), c, f) => {
                    let mut s = format!("{} commit(s)", c.map(|n| n.to_string()).unwrap_or("?".into()));
                    if f.unwrap_or(0) > 0 {
                        s.push_str(&format!(", {} files +{} −{}", f.unwrap_or(0), l.8.unwrap_or(0), l.9.unwrap_or(0)));
                    }
                    s
                }
            };
            let repo =
                l.4.as_deref().map(|r| r.rsplit(['/', '\\']).next().unwrap_or(r).to_string()).unwrap_or("-".into());
            vec![
                super::read::when(l.2),
                l.1.clone(),
                format!("{}{}", repo, l.5.as_deref().map(|b| format!(" ({b})")).unwrap_or_default()),
                l.10.answered.to_string(),
                format!("{}{}", money(l.10.known_micro_usd), if l.10.unknown_cost > 0 { " +?" } else { "" }),
                git,
                if l.3.is_none() { "running or cut off".into() } else { String::new() },
            ]
        })
        .collect();
    table(&["STARTED (UTC)", "TOOL", "REPO", "REQUESTS", "COST", "GIT", ""], &rows);
    Ok(0)
}

/// What `--by-repo` prints under its table: these are observations, not a measure of work (ADR-036).
const BY_REPO_NOTE: &str = "Observations from the ledger and Git, not a measure of productivity or quality: a commit is \
not a unit of work, and a launch that changed nothing may have answered a question or reviewed code. \
\"Changed nothing\": a settled launch in a repository that made no commit and left no change against its starting commit.";

/// `pygg launches --by-repo`: cost per repository per UTC week, per commit, and the launches that changed nothing.
fn launches_by_repo(opts: &Opts) -> Result<i32, Fail> {
    let weeks = opts.number("launches", "weeks", 1..=104, 8)?;
    let conn = open_read(opts)?;
    let since = crate::launches::week_start(crate::db::now_ms()) - (weeks - 1) * 7 * 86_400_000;
    let rows = crate::launches::by_repo(&conn, since).map_err(op)?;
    if opts.has("json") {
        return Ok(print_json(json!({ "schemaVersion": JSON_SCHEMA, "weeks": weeks, "since": since,
            "note": BY_REPO_NOTE, "repos": rows.iter().map(|w| json!({
            "repo": w.repo, "weekStart": w.week_start, "launches": w.launches, "unsettled": w.unsettled,
            "requests": w.requests, "knownMicroUsd": w.known_micro_usd, "unknownCost": w.unknown_cost,
            "commits": w.commits, "perCommitMicroUsd": w.per_commit(), "changedNothing": w.changed_nothing,
            "changedNothingMicroUsd": w.changed_nothing_micro_usd })).collect::<Vec<_>>() })));
    }
    if rows.is_empty() {
        println!("No launches in the last {weeks} weeks. Start a tool through Pygg with pygg run claude (or codex, opencode).");
        return Ok(0);
    }
    let table_rows: Vec<Vec<String>> = rows
        .iter()
        .map(|w| {
            let repo = w.repo.as_deref().unwrap_or("(not in Git)");
            let mut launches = w.launches.to_string();
            if w.unsettled > 0 {
                launches.push_str(&format!(" ({} unsettled)", w.unsettled));
            }
            vec![
                repo.to_string(),
                super::read::when(w.week_start).chars().take(10).collect(),
                launches,
                format!("{}{}", money(w.known_micro_usd), if w.unknown_cost > 0 { " +?" } else { "" }),
                w.commits.to_string(),
                w.per_commit().map(money).unwrap_or("-".into()),
                if w.repo.is_none() {
                    "-".into()
                } else {
                    format!("{} ({})", w.changed_nothing, money(w.changed_nothing_micro_usd))
                },
            ]
        })
        .collect();
    table(&["REPOSITORY", "WEEK (UTC)", "LAUNCHES", "COST", "COMMITS", "PER COMMIT", "CHANGED NOTHING"], &table_rows);
    println!("\n{BY_REPO_NOTE}");
    Ok(0)
}

/// `pygg savings`: the app's estimate, against the model chosen to compare with.
pub fn savings(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("savings", args, &["json"], &["db", "days"])?;
    let days = opts.number("savings", "days", 1..=366, 30)?;
    let conn = open_read(&opts)?;
    let since = crate::db::now_ms() - days * 86_400_000;
    let s = gateway::savings::savings(&conn, since, 86_400_000, 0).map_err(op)?;
    if opts.has("json") {
        return Ok(print_json(json!({ "schemaVersion": JSON_SCHEMA, "days": days, "baseline": s.baseline,
            "compared": s.compared, "routedMicroUsd": s.routed_micro_usd, "baselineMicroUsd": s.baseline_micro_usd,
            "fellBack": s.fell_back, "leftOutUnknownCost": s.left_out_unknown_cost, "suggested": s.suggested })));
    }
    let Some(baseline) = s.baseline.as_deref().filter(|_| s.baseline_priced) else {
        println!(
            "No model to compare with is chosen{}. Choose one in the app (Usage → Savings → Change comparison).",
            s.suggested
                .as_deref()
                .map(|m| format!(" (Pygg suggests {m}, the paid model asked for most)"))
                .unwrap_or_default()
        );
        return Ok(0);
    };
    let saved = s.baseline_micro_usd.saturating_sub(s.routed_micro_usd);
    println!("Last {days} days, {} answers compared with {baseline} on the same tokens (an estimate):", s.compared);
    println!("  Through Pygg   {}", money(s.routed_micro_usd));
    println!("  At {baseline}   {}", money(s.baseline_micro_usd));
    println!("  Difference     {}", money(saved));
    if s.left_out_unknown_cost > 0 {
        println!("  {} answers of unknown cost are left out.", s.left_out_unknown_cost);
    }
    if s.fell_back > 0 {
        println!("  {} requests were kept alive by a fallback.", s.fell_back);
    }
    Ok(0)
}

// ---------- completion ----------

/// The top-level commands, for `pygg completion` and `pygg help`.
pub const COMMANDS: &[&str] = &[
    "chat",
    "ask",
    "models",
    "providers",
    "default",
    "route",
    "combos",
    "keys",
    "usage",
    "savings",
    "logs",
    "launches",
    "setup",
    "run",
    "statusline",
    "status",
    "doctor",
    "diagnostics",
    "gateway",
    "completion",
    "help",
];

/// `pygg completion powershell|bash|zsh`
pub fn completion(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("completion", args, &[], &[])?;
    let words = COMMANDS.join(" ");
    let script = match opts.words.first().map(String::as_str) {
        Some("powershell") | Some("pwsh") => format!(
            "# pygg completion for PowerShell: add to $PROFILE with  pygg completion powershell >> $PROFILE\n\
             Register-ArgumentCompleter -Native -CommandName pygg -ScriptBlock {{\n\
             \x20 param($word, $ast, $cursor)\n\
             \x20 $sub = @{{ models = 'info test enable disable'; providers = 'presets add refresh enable disable remove'; \
             setup = 'claude codex opencode'; run = 'claude codex opencode'; route = 'auto auto/cheap'; completion = 'powershell bash zsh' }}\n\
             \x20 $parts = $ast.CommandElements | ForEach-Object {{ $_.ToString() }}\n\
             \x20 $list = if ($parts.Count -le 2 -and -not ($parts.Count -eq 2 -and $word -eq '')) {{ '{words}' }} elseif ($sub.ContainsKey($parts[1])) {{ $sub[$parts[1]] }} else {{ '' }}\n\
             \x20 $list -split ' ' | Where-Object {{ $_ -and $_ -like \"$word*\" }} | ForEach-Object {{ [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_) }}\n\
             }}\n"
        ),
        Some("bash") => format!(
            "# pygg completion for bash: add to ~/.bashrc with  pygg completion bash >> ~/.bashrc\n\
             _pygg() {{\n\
             \x20 local cur=${{COMP_WORDS[COMP_CWORD]}}\n\
             \x20 if [ \"$COMP_CWORD\" -eq 1 ]; then COMPREPLY=( $(compgen -W \"{words}\" -- \"$cur\") ); return; fi\n\
             \x20 case \"${{COMP_WORDS[1]}}\" in\n\
             \x20   models) COMPREPLY=( $(compgen -W \"info test enable disable\" -- \"$cur\") );;\n\
             \x20   providers) COMPREPLY=( $(compgen -W \"presets add refresh enable disable remove\" -- \"$cur\") );;\n\
             \x20   setup|run) COMPREPLY=( $(compgen -W \"claude codex opencode\" -- \"$cur\") );;\n\
             \x20   completion) COMPREPLY=( $(compgen -W \"powershell bash zsh\" -- \"$cur\") );;\n\
             \x20 esac\n\
             }}\n\
             complete -F _pygg pygg\n"
        ),
        Some("zsh") => format!(
            "# pygg completion for zsh: add to ~/.zshrc with  pygg completion zsh >> ~/.zshrc\n\
             _pygg() {{ if (( CURRENT == 2 )); then compadd {words}; fi }}\n\
             compdef _pygg pygg\n"
        ),
        _ => return Err(Fail::usage("name a shell.\nUse: pygg completion powershell | bash | zsh")),
    };
    print!("{script}");
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_model_name_is_connection_and_model() {
        assert_eq!(target("x", "muse/muse").unwrap(), ("muse".into(), "muse".into()));
        assert_eq!(target("x", "openrouter/anthropic/claude").unwrap().1, "anthropic/claude");
        assert!(target("x", "muse").is_err());
        assert!(target("x", "/m").is_err());
    }

    #[test]
    fn prices_per_million_tokens() {
        assert_eq!(per_mtok(0), "$0");
        assert_eq!(per_mtok(3_000_000), "$3.00");
        assert_eq!(per_mtok(1_500), "$0.0015");
    }

    #[test]
    fn the_default_model_falls_back_to_auto() {
        let conn = crate::db::open_in_memory().unwrap();
        assert_eq!(default_model(&conn), "auto");
        store::set_setting(&conn, DEFAULT_MODEL, "muse/muse").unwrap();
        assert_eq!(default_model(&conn), "muse/muse");
        store::set_setting(&conn, DEFAULT_MODEL, "").unwrap();
        assert_eq!(default_model(&conn), "auto");
    }

    #[test]
    fn completion_scripts_name_every_command() {
        for shell in ["powershell", "bash", "zsh"] {
            assert_eq!(completion(&[shell.to_string()]).unwrap(), 0);
        }
        assert_eq!(completion(&["fish".to_string()]).unwrap_err().code, 2);
    }
}
