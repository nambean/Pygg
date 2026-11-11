//! Headless gateway: the same store, routing and ledger as the app, without the window.
//!
//!   gateway [--db PATH] serve [--port N]
//!   gateway [--db PATH] add-connection --preset gemini [--label NAME] [--key-env VAR] [--billing TYPE] [--endpoint URL]
//!   gateway [--db PATH] remove-connection --id ID
//!   gateway [--db PATH] refresh-models --id ID
//!   gateway [--db PATH] create-client --name NAME [--monthly-usd N]
//!   gateway [--db PATH] combo --name NAME --member CONNECTION/MODEL [--member …]
//!   gateway [--db PATH] list
//!   gateway [--db PATH] usage
//!
//! The key is read from an environment variable so it never appears in a command line or shell history.
//! Without --db the app's own store is used (%APPDATA%\app.smarttaskrouter.desktop on Windows).

use smarttaskrouter_desktop_lib::gateway::{self, catalog, presets, store, OsSecrets, Shared};
use smarttaskrouter_desktop_lib::{db, vault};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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
    let shared = Shared::for_store(Arc::new(Mutex::new(conn)), Arc::new(OsSecrets), &db_path);

    match command.as_str() {
        "serve" => {
            let port = one("port").map(|p| p.parse().unwrap_or_else(|_| fail("bad port"))).unwrap_or(gateway::DEFAULT_PORT);
            let server = gateway::start(shared, port).await.unwrap_or_else(|e| match e.kind() {
                std::io::ErrorKind::ResourceBusy => fail(e.to_string()),
                _ => fail(format!("127.0.0.1:{port}: {e}")),
            });
            println!("Gateway listening on http://{} (store: {})", server.addr, db_path.display());
            let _ = tokio::signal::ctrl_c().await;
            server.stop().await;
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
            let added = shared.with_db(|db| store::import_models(db, &id, &ids)).unwrap_or_else(|e| fail(e));
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
