//! `pygg-cli`, one console program for everything Pygg does without its window (PV-04, ADR-030), published on npm as
//! the `pygg` command. The app's own program is `Pygg.exe`, and Windows does not tell `pygg.exe` from it, so the
//! console program has its own name. It runs the same Rust core and store as the app (docs/design/CLI.md):
//!
//!   pygg                             where you are: gateway, providers, tools set up, and what to type next
//!   pygg setup [claude|codex|opencode]   route a coding tool through Pygg (setup.rs)
//!   pygg run claude|codex|opencode   start it through Pygg, a routing session per launch (run.rs)
//!   pygg status | doctor             is the gateway answering; plus the schema, the tools and providers needing attention
//!   pygg providers | models | logs | usage   read the store, `--json` on each (read.rs)
//!   pygg gateway [--db P] …          the headless gateway: serve, connections, clients, combos, usage (advanced)
//!   pygg mcp [setup …]               the experimental task integration's MCP server, and registering it
//!   pygg agent [--db P] …            agent detection, one-off runs, smart tasks, quality (on hold)
//!   pygg diagnostics --out PATH      the redacted diagnostics bundle (diagnostics.rs), as Settings → Diagnostics saves
//!
//! The read commands, `status`, `doctor` and `diagnostics` open the store read-only: they never create or migrate it,
//! and never start a second gateway on a store the app holds (gateway/mod.rs, one store per gateway). Exit codes: 0
//! done, 1 failed, 2 wrong usage, 3 a check found a problem; `run` returns the tool's own.

pub mod agent;
pub mod gateway;
pub mod read;
pub mod run;
pub mod setup;

use std::path::{Path, PathBuf};
use std::time::Duration;

use read::{Fail, Opts};
use serde::Serialize;
use serde_json::json;

/// `pygg --help`: every command in a line, grouped by what a person is doing (docs/design/CLI.md).
const HELP: &str = "Pygg: one local gateway for your AI coding tools.

usage: pygg <command> [options]        `pygg` alone shows where you are and what to type next

Use coding tools through Pygg
  setup                       the tools, whether each is installed and set up, and what to type next
  setup claude|codex|opencode route the tool through Pygg   (--dry-run previews, --remove undoes)
  run claude|codex|opencode   start it through Pygg, with a routing session for this launch
                              (pygg run --model auto/cheap claude: Pygg's options go before the tool's name)

See what Pygg has and does (read only; --json on each)
  status                      is the gateway answering, and on which store
  doctor                      status, plus your tools and the providers that need attention
  providers                   saved connections: on, last check, health, models
  models [CONNECTION]         the models to ask for, with billing and price   (--search TEXT, --all)
  logs                        recent request attempts   (--limit N, --client NAME, --failed)
  usage                       requests, known and unknown cost, token mix   (--days 1|7|30)

Advanced
  gateway <command> ...       the gateway without the window: serve, add-connection, model, client, combo, auto
  diagnostics --out PATH      save the redacted diagnostics bundle (no keys, tokens or prompts)
  --version                   this program's version

Experimental task integration (on hold)
  mcp setup [TOOL]            register Pygg's MCP server in claude-code, codex or opencode
  mcp                         the MCP server itself, which the tool starts
  agent <command> ...         agent CLIs: detect, run, task, quality

Every command takes --db PATH (another store); `pygg <command> --help` says more.
Connect providers in the Pygg desktop app, which also runs the gateway: https://github.com/nambean/smarttaskrouter
Exit codes: 0 done, 1 failed, 2 wrong usage, 3 a check found a problem; `run` returns the tool's own.";

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

/// What `/health` says on the port, if Pygg answers there: an HTTP success whose `app` is Pygg (Codex's review of
/// 2026-11-14: any JSON used to pass).
pub(crate) async fn health(port: u16) -> Result<serde_json::Value, String> {
    health_within(port, Duration::from_secs(3)).await
}

/// Whether Pygg answers on the port, waiting under a second: for a bare `pygg`, which must stay quick.
pub(crate) async fn health_quick(port: u16) -> Result<(), String> {
    health_within(port, Duration::from_millis(900)).await.map(|_| ())
}

async fn health_within(port: u16, wait: Duration) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::builder().timeout(wait).build().map_err(|e| e.to_string())?;
    let response = client.get(format!("http://127.0.0.1:{port}/health")).send().await.map_err(|e| {
        if e.is_connect() { "nothing is listening".to_string() } else if e.is_timeout() { "no answer in time".to_string() } else { e.to_string() }
    })?;
    if !response.status().is_success() {
        return Err(format!("an answer that is not Pygg's (HTTP {})", response.status().as_u16()));
    }
    let body: serde_json::Value = response.json().await.map_err(|_| "an answer that is not Pygg's".to_string())?;
    if body["app"].as_str() != Some("Pygg") {
        return Err("an answer that is not Pygg's".into());
    }
    Ok(body)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StoreOut {
    path: Option<String>,
    exists: bool,
    /// A running Pygg serves this store.
    served: bool,
    schema: Option<u32>,
    /// The schema this program expects.
    expected_schema: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GatewayOut {
    url: String,
    answering: bool,
    /// `ok` when the ledger is written as it should be.
    accounting: Option<String>,
    error: Option<String>,
}

/// The state `status` and `doctor` report, and whether it is all as it should be.
async fn status_of(opts: &Opts) -> Result<(StoreOut, GatewayOut, bool), Fail> {
    let path = read::store_path(opts);
    let exists = path.as_ref().is_some_and(|p| p.is_file());
    let schema = path.as_ref().filter(|_| exists).and_then(|p| schema_of(p).ok());
    let conn = path.as_ref().filter(|_| exists).and_then(|p| rusqlite::Connection::open_with_flags(p,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX).ok());
    let port = read::gateway_port(opts, conn.as_ref())?;
    let url = format!("http://127.0.0.1:{port}");
    let gateway = match health(port).await {
        Ok(h) => GatewayOut { url, answering: true, accounting: h["accounting"].as_str().map(String::from),
            error: h["accountingError"].as_str().map(String::from) },
        Err(e) => GatewayOut { url, answering: false, accounting: None, error: Some(e) },
    };
    let ok = exists && schema == Some(crate::db::SCHEMA_VERSION) && gateway.answering && gateway.accounting.as_deref() == Some("ok");
    let store = StoreOut { served: path.as_ref().is_some_and(|p| exists && crate::gateway::store_in_use(p)),
        path: path.map(|p| p.display().to_string()), exists, schema, expected_schema: crate::db::SCHEMA_VERSION };
    Ok((store, gateway, ok))
}

fn print_status(store: &StoreOut, gateway: &GatewayOut) {
    match (&store.path, store.exists) {
        (Some(p), true) => println!("Store       {p} ({})", if store.served { "served by a running Pygg" } else { "not served" }),
        (Some(p), false) => println!("Store       {p} (not created yet: open the Pygg desktop app once)"),
        (None, _) => println!("Store       not found: pass --db PATH"),
    }
    match store.schema {
        Some(v) if v == store.expected_schema => println!("Schema      v{v}, as this program expects"),
        Some(v) if v < store.expected_schema =>
            println!("Schema      v{v}; this program knows v{}. Open the Pygg app once to update it.", store.expected_schema),
        Some(v) => println!("Schema      v{v} is newer than this program (v{}): update @pygg/cli", store.expected_schema),
        None => {}
    }
    if gateway.answering {
        println!("Gateway     {} answers, accounting {}", gateway.url, gateway.accounting.as_deref().unwrap_or("unknown"));
        if let Some(e) = &gateway.error {
            println!("            {e}");
        }
    } else {
        println!("Gateway     {}: {}. Open the Pygg desktop app; it runs the gateway.", gateway.url,
            gateway.error.as_deref().unwrap_or("not answering"));
    }
}

const STATUS_HELP: &str = "usage: pygg status [--json] [--port N] [--db PATH]

Whether Pygg's gateway answers, on which store, and whether the store's schema is the one this program knows.
Exit code 3 when something is not as it should be. The port is --port, else the one Pygg saved, else 8741.";

const DOCTOR_HELP: &str = "usage: pygg doctor [--json] [--port N] [--db PATH]

`status`, plus the coding tools found on this computer and their Pygg set-up, and the providers that need attention
(last check failed, or cooling down). Sends nothing to any provider. Exit code 3 when something is not as it should be.";

async fn status(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("status", args, &["json"], &["db", "port"])?;
    let (store, gateway, ok) = status_of(&opts).await?;
    if opts.has("json") {
        println!("{}", serde_json::to_string_pretty(&json!({ "schemaVersion": read::JSON_SCHEMA, "ok": ok, "store": store,
            "gateway": gateway })).unwrap_or_default());
    } else {
        print_status(&store, &gateway);
    }
    Ok(if ok { 0 } else { 3 })
}

async fn doctor(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("doctor", args, &["json"], &["db", "port"])?;
    let (store, gateway, mut ok) = status_of(&opts).await?;
    let conn = read::open_read(&opts).ok();
    let tools = match &conn { Some(c) => setup::tool_states(c).await, None => Vec::new() };
    let agents = crate::agents::detect().await;
    let attention = conn.as_ref().map(read::attention).unwrap_or_default();
    ok &= attention.is_empty();
    if opts.has("json") {
        let tools: Vec<_> = tools.iter().map(|t| json!({ "name": t.name, "installed": t.installed, "setUp": t.set_up, "state": t.state })).collect();
        let agents: Vec<_> = agents.iter().map(|a| json!({ "id": a.id, "installed": a.installed, "version": a.version })).collect();
        println!("{}", serde_json::to_string_pretty(&json!({ "schemaVersion": read::JSON_SCHEMA, "ok": ok, "store": store,
            "gateway": gateway, "tools": tools, "agents": agents, "providersNeedingAttention": attention })).unwrap_or_default());
        return Ok(if ok { 0 } else { 3 });
    }
    print_status(&store, &gateway);
    if !tools.is_empty() {
        println!("\nCoding tools through Pygg (pygg setup)");
        for t in &tools {
            println!("  {:<10} {:<10} {}", t.name, if t.installed { "installed" } else { "not found" }, t.state);
        }
    }
    println!("\nAgents found on this computer");
    for a in &agents {
        println!("  {:<12} {:<10} {}", a.id, if a.installed { "installed" } else { "not found" }, a.version.clone().unwrap_or_default());
    }
    if conn.is_some() {
        if attention.is_empty() {
            println!("\nProviders   none needs attention");
        } else {
            println!("\nProviders that need attention (pygg providers; fix them in the Pygg app)");
            for a in &attention {
                println!("  {a}");
            }
        }
    }
    Ok(if ok { 0 } else { 3 })
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
    fn status_says_what_is_wrong_with_exit_code_3_and_refuses_a_bad_port() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("store.sqlite3");
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port().to_string();
        let args = |extra: &[&str]| ["--db", store.to_str().unwrap()].iter().chain(extra).map(|s| s.to_string()).collect::<Vec<_>>();
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        assert_eq!(rt.block_on(status(&args(&["--port", &port]))), Ok(3), "no store, no gateway: a problem found");
        assert_eq!(rt.block_on(status(&args(&["--port", "nonsense"]))).unwrap_err().code, 2);
        assert_eq!(rt.block_on(status(&args(&["--verbose"]))).unwrap_err().code, 2);
        assert!(!store.exists(), "status creates nothing");
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
        return read::home().await;
    }
    let command = args.remove(0);
    // A command's own --help, before anything is opened.
    let help = |text: &str| { println!("{text}"); 0 };
    match command.as_str() {
        "-V" | "--version" | "version" => { println!("pygg-cli {}", env!("CARGO_PKG_VERSION")); 0 }
        "status" if wants_help(&args) => help(STATUS_HELP),
        "status" => read::finish(status(&args).await),
        "doctor" if wants_help(&args) => help(DOCTOR_HELP),
        "doctor" => read::finish(doctor(&args).await),
        "providers" | "provider" if wants_help(&args) => help(read::PROVIDERS_HELP),
        "providers" | "provider" => read::finish(read::providers(&args)),
        "models" | "model" if wants_help(&args) => help(read::MODELS_HELP),
        "models" | "model" => read::finish(read::models(&args)),
        "logs" | "log" if wants_help(&args) => help(read::LOGS_HELP),
        "logs" | "log" => read::finish(read::logs(&args)),
        "usage" if wants_help(&args) => help(read::USAGE_HELP),
        "usage" => read::finish(read::usage(&args)),
        // `pygg mcp setup …` registers the server; a bare `pygg mcp` is started by the program before its runtime
        // (bin/pygg-cli.rs), since the MCP server runs its own.
        "mcp" if args.first().map(String::as_str) == Some("setup") => setup::run_mcp(args[1..].to_vec()).await,
        "mcp" => { eprintln!("pygg: start the MCP server as `pygg mcp`; register it with `pygg mcp setup`."); 2 }
        "gateway" => { gateway::run(args).await; 0 }
        "agent" => { agent::run(args).await; 0 }
        "setup" => setup::run(args).await,
        "run" => run::run(args).await,
        "diagnostics" => match diagnostics(&args).await {
            Ok(path) => { println!("Saved {} (no keys, tokens, prompts or agent output; see README.txt inside)", path.display()); 0 }
            Err(e) => { eprintln!("pygg: {e}"); 1 }
        },
        "-h" | "--help" | "help" => help(HELP),
        other => {
            eprintln!("pygg: unknown command {other}.\nUse: pygg --help (or `pygg` alone for where to start)");
            2
        }
    }
}
