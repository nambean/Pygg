//! `pygg-cli`, one console program for everything Pygg does without its window (PV-04, ADR-030), published on npm as
//! the `pygg` command. The app's own program is `Pygg.exe`, and Windows does not tell `pygg.exe` from it, so the
//! console program has its own name. It runs the same Rust core and store as the app (docs/design/CLI.md):
//!
//!   pygg                             where you are: gateway, providers, tools set up, and what to type next
//!   pygg setup [claude|codex|opencode]   route a coding tool through Pygg (setup.rs)
//!   pygg run claude|codex|opencode   start it through Pygg, a routing session per launch (run.rs), and its receipt
//!   pygg statusline                  the launch's cost in one line, for Claude Code's status line (launches.rs)
//!   pygg chat [--model M]            a conversation with any connected model through the gateway (ask.rs)
//!   pygg ask [--model M] QUESTION    one question through the gateway, the answer printed (ask.rs)
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
pub mod ask;
pub mod gateway;
pub mod manage;
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
                              prints the launch's cost, cache and Git changes when the tool exits
  statusline                  this launch's cost in one line (pygg run --statusline claude sets it up)

Talk to any connected model through Pygg's router
  chat [-m M]                 a conversation in the terminal (pygg chat -m muse/muse); nothing is kept
  ask [-m M] QUESTION         one question, the answer printed (also from a pipe: git diff | pygg ask QUESTION)
                              -s system prompt, -t temperature, -f FILE, --json, -x code only (pygg ask --help)
  default [MODEL]             the model chat and ask use without -m (pygg default muse/muse)

Providers and models
  providers                   saved connections: on, last check, health, models
  providers presets | add | refresh | enable | disable | remove     connect or change one without the app
  models [CONNECTION]         the models to ask for, with billing and price   (--search TEXT, --all)
  models info | test | enable | disable CONNECTION/MODEL            one model's facts, a test answer, on or off
  route [auto|auto/cheap]     which models the router would try for a request, and why the rest are left out
  combos                      lists of models tried in turn, asked for by name
  keys                        Pygg's API keys: last used, spend this month, limit

See what Pygg has and does (read only; --json on each)
  status                      is the gateway answering, and on which store
  doctor                      status, plus your tools and the providers that need attention
  logs                        recent request attempts   (--limit N, --client NAME, --failed)
  usage                       requests, known and unknown cost, token mix   (--days 1|7|30)
  savings                     what routing saved against the model you compare with   (--days N)
  launches                    the tools started with pygg run: cost and what changed in Git

Advanced
  gateway <command> ...       the gateway without the window: serve, add-connection, model, client, combo, auto
  diagnostics --out PATH      save the redacted diagnostics bundle (no keys, tokens or prompts)
  completion powershell|bash|zsh   Tab completion for your shell
  help COMMAND                the same as pygg COMMAND --help
  --version                   this program's version

Experimental task integration (on hold)
  mcp setup [TOOL]            register Pygg's MCP server in claude-code, codex or opencode
  mcp                         the MCP server itself, which the tool starts
  agent <command> ...         agent CLIs: detect, run, task, quality

Every command takes --db PATH (another store; PYGG_DATA_DIR sets the folder for all); `pygg <command> --help` says more.
Exit codes: 0 done, 1 failed, 2 wrong usage, 3 a check found a problem; `run` returns the tool's own.";

/// Where providers are connected and the gateway runs, on this platform: the desktop app on Windows; elsewhere, where
/// no app is published yet, the headless gateway (ADR-037).
#[cfg(windows)]
pub(crate) const GET_STARTED: &str =
    "Connect providers with pygg providers add (API keys) or in the Pygg desktop app, which also runs the gateway \
and signs in to web sessions: https://github.com/nambean/Pygg";
#[cfg(not(windows))]
pub(crate) const GET_STARTED: &str =
    "The Pygg desktop app is Windows only for now. Here, connect a provider and run the gateway from the terminal:
  pygg gateway add-connection --preset openrouter --key-env OPENROUTER_API_KEY
  pygg gateway serve";
#[cfg(windows)]
pub(crate) const START_GATEWAY: &str = "open the Pygg desktop app; it runs the gateway";
#[cfg(not(windows))]
pub(crate) const START_GATEWAY: &str = "start the gateway with `pygg gateway serve` (in a terminal of its own)";
#[cfg(windows)]
pub(crate) const CONNECT_PROVIDER: &str = "connect a provider in the Pygg desktop app (Providers > Connect Provider)";
#[cfg(not(windows))]
pub(crate) const CONNECT_PROVIDER: &str = "connect a provider: pygg gateway add-connection --preset KIND --key-env VAR";
#[cfg(windows)]
pub(crate) const MAKE_STORE: &str = "open the Pygg desktop app once";
#[cfg(not(windows))]
pub(crate) const MAKE_STORE: &str = "run `pygg gateway list` once, which makes it";

/// Every command `pygg` knows, for "did you mean" (`suggest`).
const COMMANDS: &[&str] = &[
    "setup",
    "run",
    "status",
    "doctor",
    "providers",
    "models",
    "logs",
    "usage",
    "gateway",
    "diagnostics",
    "mcp",
    "agent",
    "help",
    "version",
];

/// The known command closest to a mistyped one, if it is a slip (at most two letters off) and not a guess.
fn suggest(typed: &str) -> Option<&'static str> {
    fn distance(a: &str, b: &str) -> usize {
        let b: Vec<char> = b.chars().collect();
        let mut row: Vec<usize> = (0..=b.len()).collect();
        for (i, ca) in a.chars().enumerate() {
            let mut prev = row[0];
            row[0] = i + 1;
            for (j, cb) in b.iter().enumerate() {
                let here = row[j + 1];
                row[j + 1] = (prev + usize::from(ca != *cb)).min(row[j] + 1).min(here + 1);
                prev = here;
            }
        }
        row[b.len()]
    }
    let typed = typed.to_lowercase();
    COMMANDS.iter().map(|c| (distance(&typed, c), *c)).filter(|(d, c)| *d <= 2.min(c.len() / 2)).min().map(|(_, c)| c)
}

/// Whether the arguments ask for help.
pub fn wants_help(args: &[String]) -> bool {
    args.iter().any(|a| a == "--help" || a == "-h") || args.first().map(String::as_str) == Some("help")
}

/// The app's own store, as the app and `gateway` find it.
pub fn default_store() -> Option<PathBuf> {
    data_dir(|name| std::env::var_os(name), std::env::consts::OS).map(|dir| dir.join("store.sqlite3"))
}

/// Pygg's data folder: `PYGG_DATA_DIR` when set (as the app reads it), else where the app keeps it on each platform,
/// Tauri's app data folder for `app.smarttaskrouter.desktop`: `%APPDATA%` on Windows, `~/Library/Application Support`
/// on macOS, `$XDG_DATA_HOME` or `~/.local/share` elsewhere.
pub fn data_dir(env: impl Fn(&str) -> Option<std::ffi::OsString>, os: &str) -> Option<PathBuf> {
    const ID: &str = "app.smarttaskrouter.desktop";
    if let Some(dir) = env("PYGG_DATA_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    let home = || env("HOME").filter(|h| !h.is_empty()).map(PathBuf::from);
    let base = match os {
        "windows" => env("APPDATA").map(PathBuf::from).or_else(|| home().map(|h| h.join("AppData").join("Roaming"))),
        "macos" => home().map(|h| h.join("Library").join("Application Support")),
        _ => env("XDG_DATA_HOME")
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
            .or_else(|| home().map(|h| h.join(".local").join("share"))),
    }?;
    Some(base.join(ID))
}

pub(crate) fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

/// The store's schema version, read without migrating it.
fn schema_of(store: &Path) -> Result<u32, String> {
    use rusqlite::OpenFlags;
    let conn = rusqlite::Connection::open_with_flags(
        store,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
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
        if e.is_connect() {
            "nothing is listening".to_string()
        } else if e.is_timeout() {
            "no answer in time".to_string()
        } else {
            e.to_string()
        }
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
    let conn = path.as_ref().filter(|_| exists).and_then(|p| {
        rusqlite::Connection::open_with_flags(
            p,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .ok()
    });
    let port = read::gateway_port(opts, conn.as_ref())?;
    let url = format!("http://127.0.0.1:{port}");
    let gateway = match health(port).await {
        Ok(h) => GatewayOut {
            url,
            answering: true,
            accounting: h["accounting"].as_str().map(String::from),
            error: h["accountingError"].as_str().map(String::from),
        },
        Err(e) => GatewayOut { url, answering: false, accounting: None, error: Some(e) },
    };
    let ok = exists
        && schema == Some(crate::db::SCHEMA_VERSION)
        && gateway.answering
        && gateway.accounting.as_deref() == Some("ok");
    let store = StoreOut {
        served: path.as_ref().is_some_and(|p| exists && crate::gateway::store_in_use(p)),
        path: path.map(|p| p.display().to_string()),
        exists,
        schema,
        expected_schema: crate::db::SCHEMA_VERSION,
    };
    Ok((store, gateway, ok))
}

fn print_status(store: &StoreOut, gateway: &GatewayOut) {
    match (&store.path, store.exists) {
        (Some(p), true) => {
            println!("Store       {p} ({})", if store.served { "served by a running Pygg" } else { "not served" })
        }
        (Some(p), false) => println!("Store       {p} (not created yet: {MAKE_STORE})"),
        (None, _) => println!("Store       not found: pass --db PATH"),
    }
    match store.schema {
        Some(v) if v == store.expected_schema => println!("Schema      v{v}, as this program expects"),
        Some(v) if v < store.expected_schema => {
            println!("Schema      v{v}; this program knows v{}. To update it, {MAKE_STORE}.", store.expected_schema)
        }
        Some(v) => {
            println!("Schema      v{v} is newer than this program (v{}): update @pygg/cli", store.expected_schema)
        }
        None => {}
    }
    if gateway.answering {
        println!(
            "Gateway     {} answers, accounting {}",
            gateway.url,
            gateway.accounting.as_deref().unwrap_or("unknown")
        );
        if let Some(e) = &gateway.error {
            println!("            {e}");
        }
    } else {
        println!(
            "Gateway     {}: {}. Next: {START_GATEWAY}.",
            gateway.url,
            gateway.error.as_deref().unwrap_or("not answering")
        );
    }
}

const STATUS_HELP: &str = "usage: pygg status [--json] [--port N] [--db PATH]

Whether Pygg's gateway answers, on which store, and whether the store's schema is the one this program knows.
Exit code 3 when something is not as it should be. The port is --port, else the one Pygg saved, else 8741.";

const DIAGNOSTICS_HELP: &str = "usage: pygg diagnostics --out PATH [--db PATH] [--port N]

Saves the redacted diagnostics bundle (a zip) of Pygg's store, as Settings > Diagnostics does: versions, schema, the
gateway's state, connections and recent request outcomes. No keys, tokens, prompts or agent output. Reads the store
without changing it.";

const DOCTOR_HELP: &str = "usage: pygg doctor [--json] [--port N] [--db PATH]

`status`, plus the coding tools found on this computer and their Pygg set-up, and the providers that need attention
(last check failed, or cooling down). Sends nothing to any provider. Exit code 3 when something is not as it should be.";

async fn status(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("status", args, &["json"], &["db", "port"])?;
    let (store, gateway, ok) = status_of(&opts).await?;
    if opts.has("json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "schemaVersion": read::JSON_SCHEMA, "ok": ok, "store": store,
            "gateway": gateway }))
            .unwrap_or_default()
        );
    } else {
        print_status(&store, &gateway);
    }
    Ok(if ok { 0 } else { 3 })
}

async fn doctor(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("doctor", args, &["json"], &["db", "port"])?;
    let (store, gateway, mut ok) = status_of(&opts).await?;
    let conn = read::open_read(&opts).ok();
    let tools = match &conn {
        Some(c) => setup::tool_states(c).await,
        None => Vec::new(),
    };
    let agents = crate::agents::detect().await;
    let attention = conn.as_ref().map(read::attention).unwrap_or_default();
    ok &= attention.is_empty();
    if opts.has("json") {
        let tools: Vec<_> = tools
            .iter()
            .map(|t| json!({ "name": t.name, "installed": t.installed, "setUp": t.set_up, "state": t.state }))
            .collect();
        let agents: Vec<_> =
            agents.iter().map(|a| json!({ "id": a.id, "installed": a.installed, "version": a.version })).collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "schemaVersion": read::JSON_SCHEMA, "ok": ok, "store": store,
            "gateway": gateway, "tools": tools, "agents": agents, "providersNeedingAttention": attention }))
            .unwrap_or_default()
        );
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
        println!(
            "  {:<12} {:<10} {}",
            a.id,
            if a.installed { "installed" } else { "not found" },
            a.version.clone().unwrap_or_default()
        );
    }
    if conn.is_some() {
        if attention.is_empty() {
            println!("\nProviders   none needs attention");
        } else {
            println!("\nProviders that need attention (pygg providers)");
            for a in &attention {
                println!("  {a}");
            }
        }
    }
    Ok(if ok { 0 } else { 3 })
}

const KEYS_HELP: &str = "usage: pygg keys [--all] [--json] [--db PATH]

Pygg's API keys (the clients that may call the gateway): name, the start of the key, when it was last used, its spend
this month and its monthly limit. Never the key itself. --all shows revoked keys too. Keys are made, limited and
revoked in the app (Endpoint → API Keys).";

const COMBOS_HELP: &str = "usage: pygg combos [--json] [--db PATH]

Combos: lists of models tried in turn (or the cheapest first), asked for by name like a model: pygg chat -m NAME.
They are made in the app (Combos).";

const LAUNCHES_HELP: &str = "usage: pygg launches [--limit N] [--json] [--db PATH]
       pygg launches --by-repo [--weeks N] [--json] [--db PATH]

The tools started with pygg run, newest first: the tool, the repository and branch, requests, cost, and what changed
in Git during the launch (commits, files, lines). Only the repository's folder, branch and commit ids are kept.

--by-repo: per repository and UTC week (8 weeks by default), the launches' known cost, the commits they made, cost
per commit, and the launches that changed nothing. Observations, not a measure of productivity or quality.";

const SAVINGS_HELP: &str = "usage: pygg savings [--days N] [--json] [--db PATH]

What the gateway's answers cost against the model you chose to compare with (Usage → Savings in the app), on the same
tokens: an estimate. 30 days by default.";

const COMPLETION_HELP: &str = "usage: pygg completion powershell|bash|zsh

Prints a script that completes pygg's commands as you type (Tab).
  PowerShell:  pygg completion powershell >> $PROFILE    then open a new terminal
  bash:        pygg completion bash >> ~/.bashrc
  zsh:         pygg completion zsh >> ~/.zshrc";

const STATUSLINE_HELP: &str = "usage: pygg statusline [--db PATH]

One line for Claude Code's status line: what this launch's requests through Pygg cost so far, how much of the prompt
came from the cache, the model that answered and any fallback. It finds the launch from the session `pygg run` gave
it, so start Claude Code with `pygg run --statusline claude`, which sets it up for that launch only. Claude Code's own
status line setting is not changed. It reads the store and never writes it.";

/// `pygg statusline`: never fails, since Claude Code shows whatever it prints.
fn statusline(args: &[String]) -> String {
    let headers = std::env::var("ANTHROPIC_CUSTOM_HEADERS").ok();
    let attempt = std::env::var(crate::agents::ATTEMPT_ENV).ok();
    let Some(session) = crate::launches::session_from_env(headers.as_deref(), attempt.as_deref()) else {
        return "Pygg · not started with pygg run".into();
    };
    let Some(store) = flag(args, "--db").map(PathBuf::from).or_else(default_store).filter(|p| p.is_file()) else {
        return "Pygg · store not found".into();
    };
    let cost = rusqlite::Connection::open_with_flags(
        &store,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .and_then(|conn| {
        conn.busy_timeout(Duration::from_millis(500))?;
        let agents: i64 = conn.query_row(
            "SELECT COUNT(DISTINCT agent_id) FROM usage_events WHERE session_key = ?1 AND created_at >= 0",
            [&session],
            |r| r.get(0),
        )?;
        // A store older than the session and day limits has none to show.
        let limits = crate::gateway::store::session_limits(&conn, &session, crate::db::now_ms()).ok().flatten();
        Ok((crate::launches::session_cost(&conn, &session)?, agents.max(0) as u64, limits))
    });
    match cost {
        Ok((cost, agents, limits)) => crate::launches::status_line(&cost, agents, limits.as_ref()),
        Err(_) => "Pygg · could not read the ledger".into(),
    }
}

/// Writes the diagnostics bundle of the store to `--out`, reading the store without migrating it.
async fn diagnostics(args: &[String]) -> Result<PathBuf, String> {
    let out = flag(args, "--out").map(PathBuf::from).ok_or("pass --out PATH, the zip to write")?;
    let store = flag(args, "--db")
        .map(PathBuf::from)
        .or_else(default_store)
        .filter(|p| p.is_file())
        .ok_or("no store found: pass --db")?;
    let port: u16 = flag(args, "--port").and_then(|p| p.parse().ok()).unwrap_or(crate::gateway::DEFAULT_PORT);
    let (gateway_state, gateway_error) = match health(port).await {
        Ok(h) => (
            format!(
                "answers: {}, accounting {}",
                h["app"].as_str().unwrap_or("unknown"),
                h["accounting"].as_str().unwrap_or("unknown")
            ),
            h["accountingError"].as_str().map(String::from),
        ),
        Err(e) => ("not answering".to_string(), Some(e)),
    };
    let context = crate::diagnostics::Context {
        made_by: "cli",
        gateway_state,
        gateway_port: port,
        gateway_error,
        log_lines: Vec::new(),
    };
    let conn = rusqlite::Connection::open_with_flags(
        &store,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("could not open {}: {e}", store.display()))?;
    conn.busy_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    let zip = crate::diagnostics::bundle(&conn, &context, crate::db::now_ms())?;
    std::fs::write(&out, zip).map_err(|e| format!("could not write {}: {e}", out.display()))?;
    Ok(out)
}

/// Runs `pygg-cli` with its arguments (without the program's name); the exit code says whether all was well.
pub async fn run(mut args: Vec<String>) -> i32 {
    if args.is_empty() {
        return read::home().await;
    }
    let command = args.remove(0);
    // A command's own --help, before anything is opened.
    let help = |text: &str| {
        println!("{text}");
        0
    };
    match command.as_str() {
        "-V" | "--version" | "version" => {
            // The npm package's version (build.rs), so `pygg --version` names what `npm install` gave you.
            println!("pygg-cli {}", env!("PYGG_CLI_VERSION"));
            0
        }
        "status" if wants_help(&args) => help(STATUS_HELP),
        "status" => read::finish(status(&args).await),
        "doctor" if wants_help(&args) => help(DOCTOR_HELP),
        "doctor" => read::finish(doctor(&args).await),
        "providers" | "provider" if args.first().map(String::as_str) != Some("add") && wants_help(&args) => {
            help(&format!("{}\n{}", read::PROVIDERS_HELP, manage::PROVIDERS_MORE))
        }
        "providers" | "provider" => {
            let rest = args.get(1..).unwrap_or_default();
            read::finish(match args.first().map(String::as_str) {
                Some("presets") => manage::presets(rest),
                Some("add") if wants_help(rest) => {
                    println!("{}", manage::PROVIDERS_MORE.trim_start());
                    Ok(0)
                }
                Some("add") => manage::provider_add(rest).await,
                Some("refresh") => manage::provider_refresh(rest).await,
                Some("enable") => manage::provider_switch(rest, true),
                Some("disable") => manage::provider_switch(rest, false),
                Some("remove") => manage::provider_remove(rest),
                _ => read::providers(&args),
            })
        }
        "models" | "model" if wants_help(&args) => help(&format!("{}\n{}", read::MODELS_HELP, manage::MODELS_MORE)),
        "models" | "model" => {
            let rest = args.get(1..).unwrap_or_default();
            read::finish(match args.first().map(String::as_str) {
                Some("info") | Some("show") => manage::model_info(rest),
                Some("test") => manage::model_test(rest).await,
                Some("enable") => manage::model_switch(rest, true),
                Some("disable") => manage::model_switch(rest, false),
                _ => read::models(&args),
            })
        }
        "default" if wants_help(&args) => help(manage::DEFAULT_HELP),
        "default" => read::finish(manage::default(&args)),
        "route" if wants_help(&args) => help(manage::ROUTE_HELP),
        "route" => read::finish(manage::route(&args)),
        "keys" | "key" if wants_help(&args) => help(KEYS_HELP),
        "keys" | "key" => read::finish(manage::keys(&args)),
        "combos" | "combo" if wants_help(&args) => help(COMBOS_HELP),
        "combos" | "combo" => read::finish(manage::combos(&args)),
        "launches" if wants_help(&args) => help(LAUNCHES_HELP),
        "launches" => read::finish(manage::launches(&args)),
        "savings" if wants_help(&args) => help(SAVINGS_HELP),
        "savings" => read::finish(manage::savings(&args)),
        "completion" if wants_help(&args) => help(COMPLETION_HELP),
        "completion" => read::finish(manage::completion(&args)),
        "logs" | "log" if wants_help(&args) => help(read::LOGS_HELP),
        "logs" | "log" => read::finish(read::logs(&args)),
        "usage" if wants_help(&args) => help(read::USAGE_HELP),
        "usage" => read::finish(read::usage(&args)),
        // `pygg mcp setup …` registers the server; a bare `pygg mcp` is started by the program before its runtime
        // (bin/pygg-cli.rs), since the MCP server runs its own.
        "mcp" if args.first().map(String::as_str) == Some("setup") => setup::run_mcp(args[1..].to_vec()).await,
        "mcp" => {
            eprintln!("pygg: start the MCP server as `pygg mcp`; register it with `pygg mcp setup`.");
            2
        }
        "gateway" => {
            gateway::run(args).await;
            0
        }
        "agent" => {
            agent::run(args).await;
            0
        }
        "setup" => setup::run(args).await,
        "run" => run::run(args).await,
        "ask" => ask::ask(args).await,
        "chat" => ask::chat(args).await,
        "statusline" if wants_help(&args) => help(STATUSLINE_HELP),
        "statusline" => {
            println!("{}", statusline(&args));
            0
        }
        "diagnostics" if wants_help(&args) => help(DIAGNOSTICS_HELP),
        "diagnostics" => match diagnostics(&args).await {
            Ok(path) => {
                println!("Saved {} (no keys, tokens, prompts or agent output; see README.txt inside)", path.display());
                0
            }
            Err(e) => {
                eprintln!("pygg: {e}");
                1
            }
        },
        "help" if !args.is_empty() => {
            // `pygg help COMMAND` is `pygg COMMAND --help`.
            let mut again = args.clone();
            again.push("--help".into());
            Box::pin(run(again)).await
        }
        "-h" | "--help" | "help" => {
            println!("{HELP}\n\n{GET_STARTED}");
            0
        }
        other => {
            match suggest(other) {
                Some(close) => eprintln!("pygg: unknown command {other}. Did you mean `pygg {close}`?"),
                None => eprintln!("pygg: unknown command {other}."),
            }
            eprintln!("Use: pygg --help (or `pygg` alone for where to start)");
            2
        }
    }
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
        let args = |extra: &[&str]| {
            ["--db", store.to_str().unwrap()].iter().chain(extra).map(|s| s.to_string()).collect::<Vec<_>>()
        };
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        assert_eq!(rt.block_on(status(&args(&["--port", &port]))), Ok(3), "no store, no gateway: a problem found");
        assert_eq!(rt.block_on(status(&args(&["--port", "nonsense"]))).unwrap_err().code, 2);
        assert_eq!(rt.block_on(status(&args(&["--verbose"]))).unwrap_err().code, 2);
        assert!(!store.exists(), "status creates nothing");
    }

    #[test]
    fn the_store_is_where_the_app_keeps_it_on_each_platform() {
        use std::ffi::OsString;
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| pairs.iter().find(|(k, _)| *k == name).map(|(_, v)| OsString::from(v))
        };
        let id = "app.smarttaskrouter.desktop";
        assert_eq!(
            data_dir(env(&[("APPDATA", "C:/Users/a/AppData/Roaming")]), "windows").unwrap(),
            PathBuf::from("C:/Users/a/AppData/Roaming").join(id)
        );
        assert_eq!(
            data_dir(env(&[("HOME", "/Users/a")]), "macos").unwrap(),
            PathBuf::from("/Users/a/Library/Application Support").join(id)
        );
        assert_eq!(
            data_dir(env(&[("HOME", "/home/a")]), "linux").unwrap(),
            PathBuf::from("/home/a/.local/share").join(id)
        );
        assert_eq!(
            data_dir(env(&[("HOME", "/home/a"), ("XDG_DATA_HOME", "/data")]), "linux").unwrap(),
            PathBuf::from("/data").join(id)
        );
        // PYGG_DATA_DIR is the folder itself, as the app reads it.
        assert_eq!(
            data_dir(env(&[("HOME", "/home/a"), ("PYGG_DATA_DIR", "/tmp/try")]), "linux").unwrap(),
            PathBuf::from("/tmp/try")
        );
        assert_eq!(data_dir(env(&[]), "linux"), None);
    }

    #[test]
    fn a_slip_of_the_keyboard_gets_a_suggestion_and_a_guess_does_not() {
        assert_eq!(suggest("stauts"), Some("status"));
        assert_eq!(suggest("provider"), Some("providers"));
        assert_eq!(suggest("Setpu"), Some("setup"));
        assert_eq!(suggest("lgos"), Some("logs"));
        assert_eq!(suggest("deploy"), None);
        assert_eq!(suggest("x"), None);
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        assert_eq!(rt.block_on(run(vec!["stauts".into()])), 2, "an unknown command is wrong usage");
        assert_eq!(rt.block_on(run(vec!["diagnostics".into(), "--help".into()])), 0);
    }

    #[test]
    fn diagnostics_writes_a_zip_without_migrating_the_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("store.sqlite3");
        drop(crate::db::open(&store).unwrap());
        let out = dir.path().join("bundle.zip");
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port().to_string();
        let args: Vec<String> = ["--db", store.to_str().unwrap(), "--out", out.to_str().unwrap(), "--port", &port]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        assert_eq!(rt.block_on(diagnostics(&args)).unwrap(), out);
        let bytes = std::fs::read(&out).unwrap();
        assert_eq!(&bytes[..4], b"PK");
        assert!(String::from_utf8_lossy(&bytes).contains("not answering"));
        assert!(rt.block_on(diagnostics(&args[..2])).unwrap_err().contains("--out"));
    }
}
