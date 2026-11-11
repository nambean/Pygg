//! Headless gateway: the same store, routing and ledger as the app, without the window.
//!
//!   gateway [--db PATH] serve [--port N] [--tasks yes]
//!       --tasks yes: also run smart tasks from clients allowed to start them (pygg-mcp), with worktrees next to
//!       the store. For a computer where the app is not open on this store: the app runs tasks itself.
//!   gateway [--db PATH] add-connection --preset gemini [--label NAME] [--key-env VAR] [--billing TYPE] [--endpoint URL]
//!   gateway [--db PATH] remove-connection --id ID
//!   gateway [--db PATH] refresh-models --id ID
//!   gateway [--db PATH] create-client --name NAME [--monthly-usd N]
//!   gateway [--db PATH] combo --name NAME --member CONNECTION/MODEL [--member …]
//!   gateway [--db PATH] model --model CONNECTION/MODEL [--tier T0-T4] [--context N] [--max-output N]
//!                             [--tools yes|no] [--vision yes|no] [--price-in USD] [--price-out USD] [--billing TYPE]
//!                             [--price-cached USD] [--price-cache-write USD] [--price-source WHERE]   (USD per million tokens)
//!   gateway [--db PATH] client --name NAME [--auto-min-tier T0-T4] [--paid-retries N] [--tasks yes|no] [--no-training yes|no]
//!                             [--strict yes|no]
//!   gateway [--db PATH] terms --id CONNECTION --terms no_training|may_train|unknown [--source WHERE]
//!   gateway [--db PATH] auto --alias auto|auto/cheap [--client NAME] [--anthropic yes] [--tools yes] [--prompt N] [--max-tokens N]
//!                             [--thinking adaptive|enabled] [--effort low|medium|high|xhigh|max]
//!   gateway [--db PATH] list
//!   gateway [--db PATH] usage
//!
//! The key is read from an environment variable so it never appears in a command line or shell history.
//! Without --db the app's own store is used (%APPDATA%\app.smarttaskrouter.desktop on Windows).

use smarttaskrouter_desktop_lib::gateway::{self, catalog, presets, store, OsSecrets, Shared};
use smarttaskrouter_desktop_lib::{db, tasks, vault};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
}

fn default_db() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| fail("cannot find the app data folder; pass --db"));
    base.join("app.smarttaskrouter.desktop").join("store.sqlite3")
}

#[tokio::main]
async fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut db_path = None;
    if args.first().map(String::as_str) == Some("--db") {
        args.remove(0);
        db_path = Some(PathBuf::from(if args.is_empty() { fail("--db needs a path") } else { args.remove(0) }));
    }
    let db_path = db_path.unwrap_or_else(default_db);
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|e| fail(e));
    }
    let command = if args.is_empty() { fail("usage: gateway [--db PATH] serve|add-connection|remove-connection|refresh-models|create-client|combo|list|usage") } else { args.remove(0) };
    let mut flags: HashMap<String, Vec<String>> = HashMap::new();
    let mut iter = args.into_iter();
    while let Some(flag) = iter.next() {
        let name = flag.strip_prefix("--").unwrap_or_else(|| fail(format!("unexpected {flag}"))).to_string();
        let value = iter.next().unwrap_or_else(|| fail(format!("--{name} needs a value")));
        flags.entry(name).or_default().push(value);
    }
    let one = |name: &str| flags.get(name).and_then(|v| v.first()).cloned();

    let conn = db::open(&db_path).unwrap_or_else(|e| fail(format!("{}: {e}", db_path.display())));
    // Starting tiers from Pygg's table where nobody set one (ROADMAP B3).
    let _ = store::fill_curated_tiers(&conn);
    let db = Arc::new(Mutex::new(conn));
    let mut shared = Shared::for_store(db.clone(), Arc::new(OsSecrets), &db_path);
    if command == "serve" && one("tasks").as_deref() == Some("yes") {
        if gateway::store_in_use(&db_path) {
            fail("Another gateway is already serving this store");
        }
        let worktrees = db_path.with_file_name("worktrees");
        let interrupted = tasks::recover(&db.lock().unwrap(), &worktrees).unwrap_or_else(|e| fail(e));
        if interrupted > 0 {
            eprintln!("{interrupted} task(s) were cut off when this store was last served; they are interrupted");
        }
        shared = shared.with_tasks(tasks::Runner::new(db, worktrees, tasks::agent_executor()));
    }

    match command.as_str() {
        "serve" => {
            let port = one("port").map(|p| p.parse().unwrap_or_else(|_| fail("bad port"))).unwrap_or(gateway::DEFAULT_PORT);
            let owner = shared.clone();
            let server = gateway::start(shared, port).await.unwrap_or_else(|e| match e.kind() {
                std::io::ErrorKind::ResourceBusy => fail(e.to_string()),
                _ => fail(format!("127.0.0.1:{port}: {e}")),
            });
            println!("Gateway listening on http://{} (store: {})", server.addr, db_path.display());
            let _ = tokio::signal::ctrl_c().await;
            server.stop().await;
            // Requests still running are let finish, so they are settled with their real cost rather than
            // recorded as interrupted (review LC-02). A second Ctrl+C stops at once.
            if owner.owns_store() {
                eprintln!("Finishing the requests in flight; press Ctrl+C again to stop now (they are then recorded \
                    as interrupted, with an unknown cost).");
                let drained = async { while owner.owns_store() { tokio::time::sleep(Duration::from_millis(100)).await; } };
                tokio::select! { _ = drained => {}, _ = tokio::signal::ctrl_c() => {} }
            }
        }
        "add-connection" => {
            let preset = presets::find(&one("preset").unwrap_or_else(|| fail("--preset is required")))
                .unwrap_or_else(|| fail("unknown preset"));
            let endpoint = one("endpoint").or(preset.endpoint.map(String::from)).unwrap_or_else(|| fail("--endpoint is required"));
            let label = one("label").unwrap_or_else(|| preset.name.to_string());
            let billing = one("billing").unwrap_or_else(|| preset.default_billing.to_string());
            let reference = one("key-env").map(|var| {
                let key = std::env::var(&var).unwrap_or_else(|_| fail(format!("{var} is not set")));
                let reference = vault::new_reference();
                vault::store(&reference, key.trim()).unwrap_or_else(|e| fail(e));
                reference
            });
            let provider = shared.with_db(|db| store::add_provider(db, &store::NewProvider {
                kind: preset.id, label: &label, endpoint: &endpoint, credential_ref: reference.as_deref(), default_billing: &billing, wire: preset.wire,
            })).unwrap_or_else(|e| fail(e));
            println!("Added connection {} ({})", provider.id, provider.endpoint);
        }
        "remove-connection" => {
            let id = one("id").unwrap_or_else(|| fail("--id is required"));
            if let Some(reference) = shared.with_db(|db| store::remove_provider(db, &id)).unwrap_or_else(|e| fail(e)) {
                vault::remove(&reference).unwrap_or_else(|e| fail(e));
            }
            println!("Removed {id} and its key");
        }
        "refresh-models" => {
            let id = one("id").unwrap_or_else(|| fail("--id is required"));
            let provider = shared.with_db(|db| store::get_provider(db, &id)).unwrap_or_else(|e| fail(e))
                .unwrap_or_else(|| fail("no such connection"));
            let ids = catalog::fetch_models(&shared, &provider).await.unwrap_or_else(|e| fail(e));
            let source = catalog::list_source(&provider);
            let added = shared.with_db(|db| store::import_listed(db, &id, &ids, Some(&source))).unwrap_or_else(|e| fail(e));
            println!("{} listed {} models, {added} new", provider.label, ids.len());
        }
        "create-client" => {
            let name = one("name").unwrap_or_else(|| fail("--name is required"));
            let limit = one("monthly-usd").map(|v| (v.parse::<f64>().unwrap_or_else(|_| fail("bad amount")) * 1e6).round() as u64);
            let (client, token) = shared.with_db(|db| store::create_client(db, &name, limit)).unwrap_or_else(|e| fail(e));
            println!("Client {} created. Token (shown once):\n{token}", client.name);
        }
        "combo" => {
            let name = one("name").unwrap_or_else(|| fail("--name is required"));
            let members: Vec<store::Target> = flags.get("member").cloned().unwrap_or_default().iter().map(|m| {
                let (provider_id, model_id) = m.split_once('/').unwrap_or_else(|| fail(format!("{m}: use CONNECTION/MODEL")));
                store::Target { provider_id: provider_id.into(), model_id: model_id.into() }
            }).collect();
            let existing = shared.with_db(store::list_combos).unwrap_or_else(|e| fail(e)).into_iter().find(|c| c.name == name);
            let combo = shared.with_db(|db| store::save_combo(db, existing.as_ref().map(|c| c.id.as_str()), &name, &members))
                .unwrap_or_else(|e| fail(e));
            println!("Combo {} → {}", combo.name,
                combo.members.iter().map(|m| format!("{}/{}", m.provider_id, m.model_id)).collect::<Vec<_>>().join(" → "));
        }
        "model" => {
            // The user's statement about a model, for auto routing and pricing (ROADMAP P0-5).
            let target = one("model").unwrap_or_else(|| fail("--model is required"));
            let (provider, model) = target.split_once('/').unwrap_or_else(|| fail("use --model CONNECTION/MODEL"));
            let yes = |name: &str| one(name).map(|v| match v.as_str() {
                "yes" => true, "no" => false, _ => fail(format!("--{name} takes yes or no")) });
            let count = |name: &str| one(name).map(|v| v.parse::<u64>().unwrap_or_else(|_| fail(format!("bad --{name}"))));
            let usd = |name: &str| one(name).map(|v| (v.parse::<f64>().unwrap_or_else(|_| fail(format!("bad --{name}"))) * 1e6).round() as u64);
            shared.with_db(|db| -> store::Result<()> {
                let _ = store::add_model(db, provider, model);
                let current = store::list_models(db, Some(provider))?.into_iter().find(|m| m.model_id == model)
                    .ok_or("model not found")?;
                let caps = current.capabilities;
                store::set_capabilities(db, provider, model, &store::Capabilities {
                    tier: one("tier").or(caps.tier), context_tokens: count("context").or(caps.context_tokens),
                    max_output_tokens: count("max-output").or(caps.max_output_tokens),
                    supports_tools: yes("tools").or(caps.supports_tools), supports_vision: yes("vision").or(caps.supports_vision),
                    ..store::Capabilities::default() })?;
                if let (Some(input), Some(output)) = (usd("price-in"), usd("price-out")) {
                    store::update_model(db, provider, model, None, None, Some(Some(gateway::pricing::Price {
                        input_micro_per_mtok: input, output_micro_per_mtok: output, cached_input_micro_per_mtok: usd("price-cached"),
                        cache_write_micro_per_mtok: usd("price-cache-write"), source: one("price-source").unwrap_or_else(|| "user".into()),
                        observed_at: db::now_ms() })))?;
                }
                if let Some(billing) = one("billing") {
                    store::update_model(db, provider, model, None, Some(&billing), None)?;
                }
                Ok(())
            }).unwrap_or_else(|e| fail(e));
            println!("Updated {target}");
        }
        "terms" => {
            let id = one("id").unwrap_or_else(|| fail("--id is required"));
            let terms = one("terms").unwrap_or_else(|| fail("--terms is required"));
            shared.with_db(|db| store::set_provider_terms(db, &id, &terms, one("source").as_deref())).unwrap_or_else(|e| fail(e));
            println!("Recorded {id}: {terms}");
        }
        "client" => {
            let name = one("name").unwrap_or_else(|| fail("--name is required"));
            let client = shared.with_db(store::list_clients).unwrap_or_else(|e| fail(e)).into_iter()
                .find(|c| c.name == name && c.revoked_at.is_none()).unwrap_or_else(|| fail("no such client"));
            shared.with_db(|db| -> store::Result<()> {
                if let Some(tier) = one("auto-min-tier") { store::set_client_auto_min_tier(db, &client.id, &tier)?; }
                if let Some(n) = one("paid-retries") {
                    store::set_client_paid_retries(db, &client.id, n.parse().map_err(|_| "bad --paid-retries".to_string())?)?;
                }
                if let Some(tasks) = one("tasks") { store::set_client_may_run_tasks(db, &client.id, tasks == "yes")?; }
                if let Some(only) = one("no-training") { store::set_client_require_no_training(db, &client.id, only == "yes")?; }
                if let Some(strict) = one("strict") { store::set_client_strict_budget(db, &client.id, strict == "yes")?; }
                Ok(())
            }).unwrap_or_else(|e| fail(e));
            println!("Updated client {name}");
        }
        "auto" => {
            // What an auto alias would choose; sends nothing.
            let alias = one("alias").unwrap_or_else(|| "auto".into());
            let strategy = gateway::auto::strategy(&alias).unwrap_or_else(|| fail("not an auto alias")).unwrap_or_else(|e| fail(e));
            let clients = shared.with_db(store::list_clients).unwrap_or_else(|e| fail(e));
            let client = match one("client") {
                Some(name) => clients.into_iter().find(|c| c.name == name).unwrap_or_else(|| fail("no such client")),
                None => clients.into_iter().next().unwrap_or_else(|| fail("create a client first")),
            };
            let needs = gateway::auto::Needs { surface_anthropic: one("anthropic").as_deref() == Some("yes"),
                tools: one("tools").as_deref() == Some("yes"), vision: one("vision").as_deref() == Some("yes"),
                input_tokens: one("prompt").and_then(|v| v.parse().ok()).unwrap_or(1000),
                max_tokens: one("max-tokens").and_then(|v| v.parse().ok()), thinking: one("thinking"), effort: one("effort") };
            let plan = shared.with_db(|db| gateway::auto::plan(db, &client, &alias, strategy, &needs)).unwrap_or_else(|e| fail(e));
            for (i, r) in plan.ranked.iter().enumerate() {
                println!("{:>2}. {}/{}  {}", i + 1, r.provider_id, r.model_id, r.reason);
            }
            println!("{} excluded", plan.excluded.len());
            for e in plan.excluded.iter().take(one("show").and_then(|v| v.parse().ok()).unwrap_or(10)) {
                println!("    {}/{}: {}", e.provider_id, e.model_id, e.reason);
            }
        }
        "list" => {
            for p in shared.with_db(store::list_providers).unwrap_or_else(|e| fail(e)) {
                let models = shared.with_db(|db| store::list_models(db, Some(&p.id))).unwrap_or_default().len();
                println!("{:<20} {:<12} {:<9} {models:>4} models  {}", p.id, p.kind, p.default_billing, p.endpoint);
            }
            for c in shared.with_db(store::list_combos).unwrap_or_else(|e| fail(e)) {
                println!("combo {:<14} {}", c.name,
                    c.members.iter().map(|m| format!("{}/{}", m.provider_id, m.model_id)).collect::<Vec<_>>().join(" → "));
            }
            for c in shared.with_db(store::list_clients).unwrap_or_else(|e| fail(e)) {
                println!("client {:<13} {}…{}", c.name, c.token_prefix, if c.revoked_at.is_some() { " (revoked)" } else { "" });
            }
        }
        "usage" => {
            let summary = shared.with_db(|db| store::usage_summary(db, 0, 50)).unwrap_or_else(|e| fail(e));
            let money = |m: u64| format!("${:.6}", m as f64 / 1e6);
            println!("{} requests, {} failed, {} fell back, known cost {}, {} with unknown cost",
                summary.total.requests, summary.total.failed, summary.total.fallbacks, money(summary.total.known_micro_usd), summary.total.unknown_requests);
            for e in summary.recent {
                println!("{:<22} {:<10} {:>6}/{:<6} {:<10} {:<10} {} [{} session={}]", format!("{}/{}", e.provider, e.model), e.outcome,
                    e.input_tokens.map(|v| v.to_string()).unwrap_or("-".into()),
                    e.output_tokens.map(|v| v.to_string()).unwrap_or("-".into()),
                    e.cost_micro_usd.map(money).unwrap_or("unknown".into()), e.cost_status, e.cost_source,
                    e.route_mode.as_deref().unwrap_or("-"), e.session_key.as_deref().unwrap_or("-"));
            }
        }
        other => fail(format!("unknown command {other}")),
    }
}
