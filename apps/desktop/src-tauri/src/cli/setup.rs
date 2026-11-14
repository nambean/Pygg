//! `pygg setup`: CLI Tools' one-click set-up from a terminal (integrations.rs, launcher.rs), and `pygg mcp setup`,
//! the experimental task integration (on hold).
//!
//!   pygg setup                            the three tools, whether they are installed and set up, what to type next
//!   pygg setup claude|codex|opencode      route the tool through Pygg   [--dry-run | --remove]
//!   pygg mcp setup claude-code|codex|opencode   register Pygg's MCP server in the tool   [--dry-run | --remove]
//!
//! Names (docs/design/CLI.md, Codex's review of 2026-11-14): `claude`, `codex` and `opencode` are the gateway set-ups
//! people want; the old `claude-launcher`, `codex-models` and `opencode-models` still work. The MCP registration, which
//! used to answer to `setup codex`, moved to `mcp setup`. The same code as the app runs: the configuration is backed up
//! first, the key is never printed, and a tool Pygg did not set up is left alone. `--dry-run` opens nothing for writing.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::Connection;

use super::read::{Fail, Opts};
use crate::integrations::{self, Homes, Store};

pub const HELP: &str = "usage: pygg setup [claude|codex|opencode] [--dry-run | --remove] [--db PATH]

Routes a coding tool's model requests through Pygg's gateway, with the model auto.

  pygg setup              the tools, whether each is installed and set up, and what to type next
  pygg setup claude       `pygg run claude` then starts Claude Code through Pygg; plain `claude` keeps your Claude
                          sign-in. Pygg makes a key for it, kept in the system's credential store; no file changes.
  pygg setup codex        adds Pygg's provider and profile to Codex's config.toml; then `pygg run codex`
  pygg setup opencode     adds Pygg's provider to OpenCode's opencode.json; then `pygg run opencode`

  --dry-run   show what would change; nothing is written, the store is only read
  --remove    take Pygg out of the tool and revoke the key it made; the backup stays

Every change backs up the tool's configuration first. Setup sends no prompt; requests the tool makes later may cost
money, as the providers you connected charge them. The experimental task integration is `pygg mcp setup`.";

pub const MCP_HELP: &str = "usage: pygg mcp setup [claude-code|codex|opencode] [--dry-run | --remove] [--db PATH]

Experimental, on hold: registers Pygg's MCP server in the tool, so it can hand coding tasks to Pygg. This is not how a
tool's requests go through the gateway; for that, use `pygg setup`.";

/// The gateway set-up a name means: `claude` (the launcher), or the integration's id.
fn resolve(name: &str) -> Option<&'static str> {
    match name {
        "claude" | "claude-launcher" => Some("claude"),
        "codex" | "codex-models" | "codex-gateway" => Some(integrations::CODEX_GATEWAY),
        "opencode" | "opencode-models" | "opencode-gateway" => Some(integrations::OPENCODE_GATEWAY),
        _ => None,
    }
}

/// The MCP registration a name means.
fn resolve_mcp(name: &str) -> Option<&'static str> {
    integrations::TARGETS.iter().map(|t| t.0).find(|id| *id == name || (name == "claude" && *id == "claude-code"))
}

struct Db(Mutex<Connection>);

impl Store for Db {
    fn with<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
        f(&self.0.lock().unwrap())
    }
}

fn server(conn: &Connection) -> Result<integrations::Server, String> {
    let program = std::env::current_exe().map_err(|e| e.to_string())?.to_string_lossy().into_owned();
    let port: u16 = crate::gateway::store::setting(conn, crate::gateway::PORT_KEY).ok().flatten().and_then(|p| p.parse().ok())
        .unwrap_or(crate::gateway::DEFAULT_PORT);
    Ok(integrations::Server { program, args: vec!["mcp".into()],
        url: (port != crate::gateway::DEFAULT_PORT).then(|| format!("http://127.0.0.1:{port}")),
        gateway_url: format!("http://127.0.0.1:{port}"), models: integrations::gateway_models(conn) })
}

fn backups(store: &Path) -> PathBuf {
    store.parent().unwrap_or(Path::new(".")).join("integration-backups")
}

/// One coding tool's gateway set-up, as `pygg setup` and a bare `pygg` show it.
pub struct ToolState {
    pub name: &'static str,
    pub installed: bool,
    pub set_up: bool,
    /// `set up`, `not set up`, or what else integrations.rs found (`set up elsewhere`, `removed elsewhere`).
    pub state: String,
}

/// The three tools' gateway set-up, read without writing: the launcher's record and the two configurations.
pub async fn tool_states(conn: &Connection) -> Vec<ToolState> {
    let launcher = crate::launcher::state(conn);
    let mut out = vec![ToolState { name: "claude", installed: launcher.installed, set_up: launcher.set_up,
        state: if launcher.set_up { "set up".into() } else { "not set up".into() } }];
    let records: Vec<(String, Option<String>)> = integrations::records(conn).into_iter()
        .filter(|(id, _)| id == integrations::CODEX_GATEWAY || id == integrations::OPENCODE_GATEWAY).collect();
    for i in integrations::list_with(records, &Homes::from_env()).await {
        let name = if i.id == integrations::CODEX_GATEWAY { "codex" } else { "opencode" };
        out.push(ToolState { name, installed: i.installed, set_up: i.state == "set_up", state: i.state.replace('_', " ") });
    }
    out.sort_by_key(|t| ["claude", "codex", "opencode"].iter().position(|n| *n == t.name));
    out
}

/// Runs `pygg setup …`.
pub async fn run(args: Vec<String>) -> i32 {
    if super::wants_help(&args) {
        println!("{HELP}");
        return 0;
    }
    super::read::finish(setup(&args).await)
}

async fn setup(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("setup", args, &["dry-run", "remove"], &["db"])?;
    if opts.has("dry-run") && opts.has("remove") {
        return Err(Fail::usage("--dry-run and --remove: choose one."));
    }
    let store = super::read::store_path(&opts).ok_or_else(|| Fail::op("no store found: pass --db PATH"))?;
    let tool = match opts.words.as_slice() {
        [] => return list(&opts).await,
        [one] => one.clone(),
        _ => return Err(Fail::usage("name one tool.\nUse: pygg setup claude")),
    };
    if matches!(tool.as_str(), "claude-code") {
        return Err(Fail::usage("`claude-code` registered Pygg's MCP server (experimental); that is now `pygg mcp setup claude-code`.\n\
            To route Claude Code through Pygg: pygg setup claude"));
    }
    let Some(id) = resolve(&tool) else {
        return Err(Fail::usage(format!("unknown tool {tool}: one of claude, codex, opencode.")));
    };
    if id == "claude" {
        return launcher(&store, &opts);
    }
    apply(id, &tool, &store, &opts).await
}

/// `pygg setup` with no tool: the table, and what to type next. Nothing is written.
async fn list(opts: &Opts) -> Result<i32, Fail> {
    let conn = super::read::open_read(opts)?;
    let tools = tool_states(&conn).await;
    println!("Route a coding tool through Pygg\n");
    println!("{:<10}{:<11}{:<20}NEXT", "TOOL", "INSTALLED", "PYGG SETUP");
    for t in &tools {
        let next = if t.set_up { format!("pygg run {}", t.name) } else if t.installed { format!("pygg setup {}", t.name) }
            else { "install the tool first".into() };
        println!("{:<10}{:<11}{:<20}{next}", t.name, if t.installed { "yes" } else { "no" }, t.state);
    }
    println!("\nPreview a change: pygg setup claude --dry-run. Setup sends no prompt; requests the tool makes later may cost money.");
    Ok(0)
}

async fn apply(id: &'static str, tool: &str, store: &Path, opts: &Opts) -> Result<i32, Fail> {
    let homes = Homes::from_env();
    if opts.has("dry-run") {
        // Only read: a preview never creates or migrates the store.
        let conn = super::read::open_read(opts)?;
        let server = server(&conn).map_err(Fail::op)?;
        let p = integrations::plan(id, &server, &homes, &backups(store)).map_err(Fail::op)?;
        println!("Setting up {} would change {}{}", p.name, p.config_path, if p.config_exists { "" } else { " (a new file)" });
        println!("Backup first, to {}", p.backup_dir);
        println!("A new Pygg API key \"{}\" is made and written there, never printed.\n", p.client_name);
        println!("{}", p.change);
        println!("\nNothing was written. Run `pygg setup {tool}` to do it.");
        return Ok(0);
    }
    let conn = crate::db::open(store).map_err(|e| Fail::op(format!("{}: {e}", store.display())))?;
    let server = server(&conn).map_err(Fail::op)?;
    let db = Db(Mutex::new(conn));
    let now = crate::db::now_ms();
    let result = if opts.has("remove") {
        integrations::remove(id, &homes, &backups(store), &db, now).await
    } else {
        integrations::apply(id, &server, &homes, &backups(store), &db, now).await
    };
    let i = result.map_err(Fail::op)?;
    if opts.has("remove") {
        println!("Removed from {}. Its key is revoked; the backup stays in {}", i.config_path, backups(store).display());
    } else {
        println!("Set up in {}{}", i.config_path, i.backup.as_ref().map(|b| format!(" (backup: {b})")).unwrap_or_default());
        println!("Next: pygg run {}", if id == integrations::CODEX_GATEWAY { "codex" } else { "opencode" });
        if id == integrations::OPENCODE_GATEWAY {
            println!("      then choose a Pygg model with /models (pygg/auto lets Pygg choose)");
        }
    }
    // The running gateway looks keys up in the store on each request: nothing to restart.
    if !crate::gateway::store_in_use(store) {
        println!("Pygg's gateway is not running with this store. Before using the tool: {}.", super::START_GATEWAY);
    }
    Ok(0)
}

/// `pygg setup claude [--dry-run | --remove]`: the Claude Code launcher.
fn launcher(store: &Path, opts: &Opts) -> Result<i32, Fail> {
    if opts.has("dry-run") {
        println!("Setting up Claude Code makes the Pygg API key \"{}\" and keeps it in {};", crate::launcher::CLIENT_NAME,
            crate::vault::STORE_NAME);
        println!("no file changes. Then `pygg run claude` starts Claude Code through Pygg with the model auto, and plain");
        println!("`claude` keeps your Claude sign-in.\n\nNothing was written. Run `pygg setup claude` to do it.");
        return Ok(0);
    }
    let conn = crate::db::open(store).map_err(|e| Fail::op(format!("{}: {e}", store.display())))?;
    let result = if opts.has("remove") { crate::launcher::remove(&conn) } else { crate::launcher::set_up(&conn, crate::db::now_ms()) };
    match result.map_err(Fail::op)? {
        s if s.set_up => println!("Set up: the key is in {}, no file changed.\nNext: pygg run claude", crate::vault::STORE_NAME),
        _ => println!("Removed: the launcher's key is revoked and deleted from {}.", crate::vault::STORE_NAME),
    }
    Ok(0)
}

/// Runs `pygg mcp setup …`: the experimental task integration.
pub async fn run_mcp(args: Vec<String>) -> i32 {
    if super::wants_help(&args) {
        println!("{MCP_HELP}");
        return 0;
    }
    super::read::finish(mcp(&args).await)
}

async fn mcp(args: &[String]) -> Result<i32, Fail> {
    let opts = Opts::parse("mcp setup", args, &["dry-run", "remove"], &["db"])?;
    if opts.has("dry-run") && opts.has("remove") {
        return Err(Fail::usage("--dry-run and --remove: choose one."));
    }
    let store = super::read::store_path(&opts).ok_or_else(|| Fail::op("no store found: pass --db PATH"))?;
    match opts.words.as_slice() {
        [] => {
            let conn = super::read::open_read(&opts)?;
            let records: Vec<(String, Option<String>)> = integrations::records(&conn).into_iter()
                .filter(|(id, _)| resolve_mcp(id).is_some()).collect();
            println!("{:<14}{:<20}CONFIGURATION", "TOOL", "PYGG MCP SERVER");
            for i in integrations::list_with(records, &Homes::from_env()).await {
                println!("{:<14}{:<20}{}", i.id, i.state.replace('_', " "), i.config_path);
            }
            println!("\nExperimental, on hold. To route a tool's requests through Pygg, use `pygg setup`.");
            Ok(0)
        }
        [name] => {
            let id = resolve_mcp(name).ok_or_else(|| Fail::usage(format!("unknown tool {name}: one of claude-code, codex, opencode.")))?;
            apply(id, name, &store, &opts).await
        }
        _ => Err(Fail::usage("name one tool.\nUse: pygg mcp setup claude-code")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_resolve_to_the_gateway_setups_and_the_mcp_ones_apart() {
        assert_eq!(resolve("claude"), Some("claude"));
        assert_eq!(resolve("claude-launcher"), Some("claude"));
        assert_eq!(resolve("codex"), Some(integrations::CODEX_GATEWAY));
        assert_eq!(resolve("codex-models"), Some(integrations::CODEX_GATEWAY));
        assert_eq!(resolve("opencode"), Some(integrations::OPENCODE_GATEWAY));
        assert_eq!(resolve("claude-code"), None, "the MCP registration moved to `mcp setup`");
        assert_eq!(resolve("cursor"), None);
        assert_eq!(resolve_mcp("codex"), Some("codex"));
        assert_eq!(resolve_mcp("claude-code"), Some("claude-code"));
        assert_eq!(resolve_mcp("opencode-models"), None);
    }

    #[test]
    fn a_dry_run_and_bad_options_write_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("store.sqlite3");
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).chain(["--db".to_string(), store.to_string_lossy().into_owned()])
            .collect::<Vec<_>>();
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        // A preview of Codex's setup on a store that does not exist: refused, and the store is not created.
        assert_eq!(rt.block_on(setup(&args(&["codex", "--dry-run"]))).unwrap_err().code, 1);
        assert!(!store.exists());
        assert_eq!(rt.block_on(setup(&args(&["codex", "--dry-run", "--remove"]))).unwrap_err().code, 2);
        assert_eq!(rt.block_on(setup(&args(&["codex", "--force"]))).unwrap_err().code, 2);
        assert!(rt.block_on(setup(&args(&["claude-code"]))).unwrap_err().message.contains("pygg mcp setup claude-code"));
        assert!(!store.exists());
    }
}
