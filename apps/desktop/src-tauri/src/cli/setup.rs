//! `pygg setup`: the Agents screen's one-click set-up (integrations.rs, ROADMAP D2) from a terminal.
//!
//!   pygg setup                          what can be set up, and its state
//!   pygg setup <tool> --dry-run         what setting it up would change; nothing is written
//!   pygg setup <tool>                   set it up: back up the tool's configuration, make its token, write, check
//!   pygg setup <tool> --remove          take it out again and revoke the token Pygg made
//!
//! The same code as the app runs: the configuration is backed up first, the token is never printed, and a tool Pygg did
//! not set up is left alone.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::Connection;

use crate::integrations::{self, Homes, Store};

pub const HELP: &str = "usage: pygg setup [<tool>] [--dry-run | --remove] [--db PATH]

  pygg setup                   list the tools and whether Pygg is set up in them
  pygg setup codex-models      Codex uses your connections through Pygg (run it with `codex --profile pygg`)
  pygg setup opencode-models   OpenCode lists Pygg's models, `auto` among them
  pygg setup claude-launcher   `pygg run claude` starts Claude Code through Pygg, one launch at a time
  pygg setup claude-code       Claude Code can hand tasks to Pygg (its MCP server); also codex, opencode

  --dry-run   show what would change, write nothing
  --remove    take Pygg out of the tool and revoke the token it made; the backup stays

Every change backs up the tool's configuration first. The token Pygg makes is written into the tool's own
configuration and never printed; the launcher's is kept in the Windows Credential Manager.";

/// Friendly names for the integrations' ids.
fn resolve(name: &str) -> Option<&'static str> {
    let id = match name {
        "codex-models" | "codex-gateway" => integrations::CODEX_GATEWAY,
        "opencode-models" | "opencode-gateway" => integrations::OPENCODE_GATEWAY,
        "claude" | "claude-code" => "claude-code",
        other => other,
    };
    integrations::ids().into_iter().find(|i| *i == id)
}

struct Db(Mutex<Connection>);

impl Store for Db {
    fn with<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
        f(&self.0.lock().unwrap())
    }
}

fn server(conn: &Connection) -> Result<integrations::Server, String> {
    let program = std::env::current_exe().map_err(|e| e.to_string())?.to_string_lossy().into_owned();
    let port: u16 = crate::gateway::store::setting(conn, crate::commands::PORT_KEY).ok().flatten().and_then(|p| p.parse().ok())
        .unwrap_or(crate::gateway::DEFAULT_PORT);
    Ok(integrations::Server { program, args: vec!["mcp".into()],
        url: (port != crate::gateway::DEFAULT_PORT).then(|| format!("http://127.0.0.1:{port}")),
        gateway_url: format!("http://127.0.0.1:{port}"), models: integrations::gateway_models(conn) })
}

fn backups(store: &Path) -> PathBuf {
    store.parent().unwrap_or(Path::new(".")).join("integration-backups")
}

/// The next thing to type once a tool is set up.
fn next_step(id: &str) -> Option<&'static str> {
    match id {
        integrations::CODEX_GATEWAY => Some("Run Codex with: codex --profile pygg"),
        integrations::OPENCODE_GATEWAY => Some("In OpenCode, choose a Pygg model with /models (pygg/auto picks one for you)."),
        "claude-code" | "codex" | "opencode" => Some("Your agent now has Pygg's tools; tasks it starts appear in Pygg's Tasks."),
        _ => None,
    }
}

fn print_one(i: &integrations::Integration) {
    println!("  {:<18} {:<8} {:<18} {}", i.id, i.kind, i.state, i.config_path);
}

/// Runs `pygg setup …`; the exit code says whether it did what was asked.
pub async fn run(args: Vec<String>) -> i32 {
    if super::wants_help(&args) {
        println!("{HELP}");
        return 0;
    }
    let store = super::flag(&args, "--db").map(PathBuf::from).or_else(super::default_store);
    let Some(store) = store else { eprintln!("no store found: pass --db PATH"); return 2 };
    let tool = args.iter().enumerate()
        .find(|(n, a)| !a.starts_with("--") && (*n == 0 || args[n - 1] != "--db")).map(|(_, a)| a.clone());
    let homes = Homes::from_env();

    let Some(tool) = tool else {
        // The list only reads: the store is not created or migrated for it.
        let records = Some(&store).filter(|p| p.is_file())
            .and_then(|p| Connection::open_with_flags(p, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).ok())
            .map(|c| integrations::records(&c))
            .unwrap_or_else(|| integrations::ids().into_iter().map(|id| (id.to_string(), None)).collect());
        println!("  {:<18} {:<8} {:<18} {}", "TOOL", "KIND", "STATE", "CONFIGURATION");
        for i in integrations::list_with(records, &homes).await {
            print_one(&i);
        }
        println!("\nkind gateway: the tool's model requests go through Pygg. kind mcp: the tool can hand tasks to Pygg.");
        println!("`pygg setup <tool> --dry-run` shows what setting one up would change.");
        return 0;
    };
    if tool == "claude-launcher" {
        return launcher(&store, &args).await;
    }
    let Some(id) = resolve(&tool) else {
        eprintln!("unknown tool {tool}: one of codex-models, opencode-models, {}", integrations::TARGETS.iter()
            .map(|t| t.0).collect::<Vec<_>>().join(", "));
        return 2;
    };
    let conn = match crate::db::open(&store) {
        Ok(c) => c,
        Err(e) => { eprintln!("{}: {e}", store.display()); return 1; }
    };
    let server = match server(&conn) {
        Ok(s) => s,
        Err(e) => { eprintln!("{e}"); return 1; }
    };
    let db = Db(Mutex::new(conn));
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let remove = args.iter().any(|a| a == "--remove");
    if dry_run && remove {
        eprintln!("--dry-run and --remove: choose one");
        return 2;
    }
    if dry_run {
        return match integrations::plan(id, &server, &homes, &backups(&store)) {
            Ok(p) => {
                println!("Setting up {} would change {}{}", p.name, p.config_path,
                    if p.config_exists { "" } else { " (a new file)" });
                println!("Backup first, to {}", p.backup_dir);
                println!("A new client token \"{}\" is made and written there, never printed.\n", p.client_name);
                println!("{}", p.change);
                println!("\nNothing was written. Run `pygg setup {tool}` to do it.");
                0
            }
            Err(e) => { eprintln!("{e}"); 1 }
        };
    }
    let now = crate::db::now_ms();
    let result = if remove {
        integrations::remove(id, &homes, &backups(&store), &db, now).await
    } else {
        integrations::apply(id, &server, &homes, &backups(&store), &db, now).await
    };
    match result {
        Ok(i) => {
            print_one(&i);
            if remove {
                println!("Removed. The backup of the configuration stays in {}", backups(&store).display());
            } else if let Some(step) = next_step(id) {
                println!("{step}");
            }
            // The running gateway looks tokens up in the store on each request: nothing to restart.
            if !crate::gateway::store_in_use(&store) {
                println!("Pygg's gateway is not running with this store: start the app (or `pygg gateway serve`) before using the tool.");
            }
            0
        }
        Err(e) => { eprintln!("{e}"); 1 }
    }
}

/// `pygg setup claude-launcher [--dry-run | --remove]`.
async fn launcher(store: &Path, args: &[String]) -> i32 {
    if args.iter().any(|a| a == "--dry-run") {
        println!("Setting up the Claude Code launcher makes the Pygg API key \"{}\", keeps it in the Windows Credential Manager", crate::launcher::CLIENT_NAME);
        println!("and changes no file. Then `pygg run claude` starts Claude Code through Pygg with the model auto.

Nothing was written.");
        return 0;
    }
    let conn = match crate::db::open(store) {
        Ok(c) => c,
        Err(e) => { eprintln!("{}: {e}", store.display()); return 1; }
    };
    let result = if args.iter().any(|a| a == "--remove") { crate::launcher::remove(&conn) }
        else { crate::launcher::set_up(&conn, crate::db::now_ms()) };
    match result {
        Ok(s) if s.set_up => { println!("Set up. Start Claude Code through Pygg with: pygg run claude"); 0 }
        Ok(_) => { println!("Removed: the launcher's key is revoked and deleted from the Credential Manager."); 0 }
        Err(e) => { eprintln!("{e}"); 1 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_resolve_to_integrations() {
        assert_eq!(resolve("codex-models"), Some(integrations::CODEX_GATEWAY));
        assert_eq!(resolve("opencode-gateway"), Some(integrations::OPENCODE_GATEWAY));
        assert_eq!(resolve("claude"), Some("claude-code"));
        assert_eq!(resolve("codex"), Some("codex"));
        assert_eq!(resolve("cursor"), None);
    }
}
