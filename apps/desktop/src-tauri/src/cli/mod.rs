//! `pygg-cli`, one console program for everything Pygg does without its window (PV-04, ADR-030). The app's own
//! program is `Pygg.exe`, and Windows does not tell `pygg.exe` from it, so the console program has its own name; an
//! npm launcher can expose it as `pygg` later. It runs the same Rust core and store as the app:
//!
//!   pygg-cli status                  is Pygg serving, on which store, and is its gateway answering
//!   pygg-cli doctor                  status, plus the store's schema, the agents found and their Pygg set-up
//!   pygg-cli mcp                     the MCP server for IDE agents (as `Pygg.exe mcp`)
//!   pygg-cli gateway [--db P] …      the headless gateway: serve, connections, clients, combos, usage
//!   pygg-cli agent [--db P] …        agent detection, one-off runs, smart tasks, quality
//!
//! `status` and `doctor` only read: they open the store read-only, never migrate it, and never start a second
//! gateway on a store the app holds (gateway/mod.rs, one store per gateway).

pub mod agent;
pub mod gateway;

use std::path::{Path, PathBuf};
use std::time::Duration;

const USAGE: &str = "usage: pygg-cli status|doctor|mcp|gateway …|agent … [--db PATH] [--port N]";

/// The app's own store, as the app and `gateway` find it.
pub fn default_store() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(base.join("app.smarttaskrouter.desktop").join("store.sqlite3"))
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

/// The store's schema version, read without migrating it.
fn schema_of(store: &Path) -> Result<u32, String> {
    use rusqlite::OpenFlags;
    let conn = rusqlite::Connection::open_with_flags(store, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|e| e.to_string())?;
    conn.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(|e| e.to_string())
}

/// What `/health` says on the port, if anything answers there.
async fn health(port: u16) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(3)).build().map_err(|e| e.to_string())?;
    let response = client.get(format!("http://127.0.0.1:{port}/health")).send().await.map_err(|e| {
        if e.is_connect() { "nothing is listening".to_string() } else { e.to_string() }
    })?;
    response.json().await.map_err(|e| format!("an answer that is not Pygg's: {e}"))
}

/// Prints the state, and says whether everything checked is as it should be.
async fn status(args: &[String]) -> bool {
    let store = flag(args, "--db").map(PathBuf::from).or_else(default_store);
    let port: u16 = flag(args, "--port").and_then(|p| p.parse().ok()).unwrap_or(crate::gateway::DEFAULT_PORT);
    let mut ok = true;
    match &store {
        Some(path) if path.is_file() => {
            let held = crate::gateway::store_in_use(path);
            println!("Store       {} ({})", path.display(), if held { "served by a running Pygg" } else { "not served" });
        }
        Some(path) => { println!("Store       {} (not created yet: Pygg has not run with it)", path.display()); ok = false; }
        None => { println!("Store       not found: pass --db"); ok = false; }
    }
    match health(port).await {
        Ok(h) => {
            let app = h["app"].as_str().unwrap_or("unknown");
            let accounting = h["accounting"].as_str().unwrap_or("unknown");
            println!("Gateway     http://127.0.0.1:{port} answers: {app}, accounting {accounting}");
            if accounting != "ok" {
                ok = false;
                if let Some(e) = h["accountingError"].as_str() {
                    println!("            {e}");
                }
            }
        }
        Err(e) => { println!("Gateway     http://127.0.0.1:{port}: {e}"); ok = false; }
    }
    ok
}

async fn doctor(args: &[String]) -> bool {
    let mut ok = status(args).await;
    if let Some(path) = flag(args, "--db").map(PathBuf::from).or_else(default_store).filter(|p| p.is_file()) {
        match schema_of(&path) {
            Ok(v) if v == crate::db::SCHEMA_VERSION => println!("Schema      v{v}, as this program expects"),
            Ok(v) if v < crate::db::SCHEMA_VERSION =>
                println!("Schema      v{v}; this program knows v{}. Pygg updates it the next time it opens the store.", crate::db::SCHEMA_VERSION),
            Ok(v) => { println!("Schema      v{v} is newer than this program (v{}): update Pygg", crate::db::SCHEMA_VERSION); ok = false; }
            Err(e) => { println!("Schema      could not be read: {e}"); ok = false; }
        }
    }
    println!("Agents");
    for a in crate::agents::detect().await {
        println!("  {:<12} {:<10} {} {}", a.id, if a.installed { "installed" } else { "not found" },
            a.version.unwrap_or_default(), a.login.unwrap_or_default());
    }
    // Pygg's own set-up records are read from the store without migrating it.
    let records = flag(args, "--db").map(PathBuf::from).or_else(default_store).filter(|p| p.is_file())
        .and_then(|p| rusqlite::Connection::open_with_flags(&p, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).ok())
        .map(|c| crate::integrations::records(&c))
        .unwrap_or_else(|| crate::integrations::TARGETS.iter().map(|t| (t.0.to_string(), None)).collect());
    println!("Set up in your agents (Agents → From Your IDE)");
    for i in crate::integrations::list_with(records, &crate::integrations::Homes::from_env()).await {
        println!("  {:<12} {:<18} {}", i.id, i.state, i.config_path);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_schema_without_migrating_and_says_when_nothing_answers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.sqlite3");
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute_batch("PRAGMA user_version = 3").unwrap();
        }
        assert_eq!(schema_of(&path).unwrap(), 3);
        assert_eq!(schema_of(&path).unwrap(), 3, "reading it twice changes nothing");
        assert_eq!(flag(&["--db".into(), "x".into()], "--db").as_deref(), Some("x"));
        assert_eq!(flag(&["--db".into()], "--db"), None);
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        // A port nothing listens on (bound, then released).
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        assert_eq!(rt.block_on(health(port)).unwrap_err(), "nothing is listening");
    }
}

/// Runs `pygg-cli` with its arguments (without the program's name); the exit code says whether all was well.
pub async fn run(mut args: Vec<String>) -> i32 {
    if args.is_empty() {
        eprintln!("{USAGE}");
        return 2;
    }
    let command = args.remove(0);
    match command.as_str() {
        "status" => if status(&args).await { 0 } else { 1 },
        "doctor" => if doctor(&args).await { 0 } else { 1 },
        // Started by the program before its runtime (bin/pygg-cli.rs), since the MCP server runs its own.
        "mcp" => { eprintln!("start the MCP server as `pygg-cli mcp`"); 2 }
        "gateway" => { gateway::run(args).await; 0 }
        "agent" => { agent::run(args).await; 0 }
        "-h" | "--help" | "help" => { println!("{USAGE}"); 0 }
        other => { eprintln!("unknown command {other}\n{USAGE}"); 2 }
    }
}
