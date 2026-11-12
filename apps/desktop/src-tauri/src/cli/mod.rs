//! `pygg-cli`, one console program for everything Pygg does without its window (PV-04, ADR-030). The app's own
//! program is `Pygg.exe`, and Windows does not tell `pygg.exe` from it, so the console program has its own name; an
//! npm launcher can expose it as `pygg` later. It runs the same Rust core and store as the app:
//!
//!   pygg-cli status                  is Pygg serving, on which store, and is its gateway answering
//!   pygg-cli doctor                  status, plus the store's schema, the agents found and their Pygg set-up
//!   pygg-cli mcp                     the MCP server for IDE agents (as `Pygg.exe mcp`)
//!   pygg-cli gateway [--db P] …      the headless gateway: serve, connections, clients, combos, usage
//!   pygg-cli agent [--db P] …        agent detection, one-off runs, smart tasks, quality
//!   pygg-cli run claude [args…]      Claude Code through Pygg for one launch (run.rs, launcher.rs)
//!   pygg-cli setup [<tool>]          set Pygg up in an agent, as the Agents screen does (setup.rs)
//!   pygg-cli diagnostics --out PATH  the redacted diagnostics bundle (diagnostics.rs), as Settings → Diagnostics saves
//!
//! `status`, `doctor` and `diagnostics` only read: they open the store read-only, never migrate it, and never start a second
//! gateway on a store the app holds (gateway/mod.rs, one store per gateway).

pub mod agent;
pub mod gateway;
pub mod run;
pub mod setup;

use std::path::{Path, PathBuf};
use std::time::Duration;

const USAGE: &str = "usage: pygg-cli status|doctor|mcp|gateway …|agent …|setup …|diagnostics --out PATH [--db PATH] [--port N]";

/// `pygg --help`: every command in a line, and where to read more.
const HELP: &str = "Pygg: one local gateway for your AI coding tools, and the agents behind them.

usage: pygg <command> [options]

  status                    is Pygg serving, on which store, and does its gateway answer
  doctor                    status, plus the store's schema, the agents found and their Pygg set-up
  mcp                       the MCP server for IDE agents (what Claude Code, Codex and OpenCode start)
  gateway <command> …       the gateway without the window: serve, connections, models, clients, combos, usage
  agent <command> …         agent CLIs: detect, run, task, quality
  run claude [args…]        start Claude Code through Pygg for this launch only (set up: `pygg setup claude-launcher`)
  setup [<tool>]            set Pygg up in Codex, OpenCode or Claude Code, as the Agents screen does (--dry-run, --remove)
  diagnostics --out PATH    save the redacted diagnostics bundle (no keys, tokens or prompts)
  --version                 the version of this program

Options: --db PATH (another store), --port N (status and doctor). `pygg gateway --help` and `pygg agent --help` list
their commands. The desktop app does everything here with a window: https://github.com/nambean/smarttaskrouter";

/// Whether the arguments ask for help.
pub fn wants_help(args: &[String]) -> bool {
    args.iter().any(|a| a == "--help" || a == "-h") || args.first().map(String::as_str) == Some("help")
}

/// The app's own store, as the app and `gateway` find it.
pub fn default_store() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(base.join("app.smarttaskrouter.desktop").join("store.sqlite3"))
}

pub(crate) fn flag(args: &[String], name: &str) -> Option<String> {
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
pub(crate) async fn health(port: u16) -> Result<serde_json::Value, String> {
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
    println!("Set up in your agents (Agents screen, or `pygg setup`)");
    for i in crate::integrations::list_with(records, &crate::integrations::Homes::from_env()).await {
        println!("  {:<12} {:<18} {}", i.id, i.state, i.config_path);
    }
    ok
}

/// Writes the diagnostics bundle of the store to `--out`, reading the store without migrating it.
async fn diagnostics(args: &[String]) -> Result<PathBuf, String> {
    let out = flag(args, "--out").map(PathBuf::from).ok_or("pass --out PATH, the zip to write")?;
    let store = flag(args, "--db").map(PathBuf::from).or_else(default_store).filter(|p| p.is_file())
        .ok_or("no store found: pass --db")?;
    let port: u16 = flag(args, "--port").and_then(|p| p.parse().ok()).unwrap_or(crate::gateway::DEFAULT_PORT);
    let (gateway_state, gateway_error) = match health(port).await {
        Ok(h) => (format!("answers: {}, accounting {}", h["app"].as_str().unwrap_or("unknown"),
            h["accounting"].as_str().unwrap_or("unknown")), h["accountingError"].as_str().map(String::from)),
        Err(e) => ("not answering".to_string(), Some(e)),
    };
    let context = crate::diagnostics::Context { made_by: "cli", gateway_state, gateway_port: port, gateway_error,
        log_lines: Vec::new() };
    let conn = rusqlite::Connection::open_with_flags(&store, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|e| format!("could not open {}: {e}", store.display()))?;
    conn.busy_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    let zip = crate::diagnostics::bundle(&conn, &context, crate::db::now_ms())?;
    std::fs::write(&out, zip).map_err(|e| format!("could not write {}: {e}", out.display()))?;
    Ok(out)
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

    #[test]
    fn diagnostics_writes_a_zip_without_migrating_the_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("store.sqlite3");
        drop(crate::db::open(&store).unwrap());
        let out = dir.path().join("bundle.zip");
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port().to_string();
        let args: Vec<String> = ["--db", store.to_str().unwrap(), "--out", out.to_str().unwrap(), "--port", &port]
            .iter().map(|s| s.to_string()).collect();
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        assert_eq!(rt.block_on(diagnostics(&args)).unwrap(), out);
        let bytes = std::fs::read(&out).unwrap();
        assert_eq!(&bytes[..4], b"PK");
        assert!(String::from_utf8_lossy(&bytes).contains("not answering"));
        assert!(rt.block_on(diagnostics(&args[..2])).unwrap_err().contains("--out"));
    }
}

/// Runs `pygg-cli` with its arguments (without the program's name); the exit code says whether all was well.
pub async fn run(mut args: Vec<String>) -> i32 {
    if args.is_empty() {
        eprintln!("{HELP}");
        return 2;
    }
    let command = args.remove(0);
    match command.as_str() {
        "-V" | "--version" | "version" => { println!("pygg-cli {}", env!("CARGO_PKG_VERSION")); 0 }
        "status" => if status(&args).await { 0 } else { 1 },
        "doctor" => if doctor(&args).await { 0 } else { 1 },
        // Started by the program before its runtime (bin/pygg-cli.rs), since the MCP server runs its own.
        "mcp" => { eprintln!("start the MCP server as `pygg-cli mcp`"); 2 }
        "gateway" => { gateway::run(args).await; 0 }
        "agent" => { agent::run(args).await; 0 }
        "setup" => setup::run(args).await,
        "run" => run::run(args).await,
        "diagnostics" => match diagnostics(&args).await {
            Ok(path) => { println!("Saved {} (no keys, tokens, prompts or agent output; see README.txt inside)", path.display()); 0 }
            Err(e) => { eprintln!("{e}"); 1 }
        },
        "-h" | "--help" | "help" => { println!("{HELP}"); 0 }
        other => { eprintln!("unknown command {other}\n{USAGE}\nRun `pygg --help` for the list."); 2 }
    }
}
