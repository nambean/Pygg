//! `pygg run [--model M] claude [claude's own arguments…]`: Claude Code through Pygg for this one launch
//! (launcher.rs). Its own settings, sign-in and plan are untouched; a plain `claude` is as it was.

use std::path::PathBuf;
use std::time::Duration;

pub const HELP: &str = "usage: pygg run [--model M] [--db PATH] claude [arguments for claude…]

Starts Claude Code with its requests going through Pygg: the gateway's address, the launcher's own Pygg API key
(kept in the Windows Credential Manager, never in a file) and the model auto, for this launch only. Claude Code's
own settings and sign-in are not changed; run plain `claude` to use them as before.

  --model M   a model for every slot instead of auto: auto/cheap, a combo, or connection/model

Set it up once with `pygg setup claude-launcher` or in Pygg → CLI Tools.";

/// Runs `pygg run …`; the exit code is Claude Code's, or Pygg's own for a problem before it starts.
pub async fn run(args: Vec<String>) -> i32 {
    if args.is_empty() || super::wants_help(&args) {
        println!("{HELP}");
        return if args.is_empty() { 2 } else { 0 };
    }
    // Pygg's flags come before the tool's name; everything after it is the tool's.
    let Some(at) = args.iter().position(|a| !a.starts_with("--") && !is_flag_value(&args, a)) else {
        eprintln!("name the tool to run: pygg run claude");
        return 2;
    };
    let (ours, rest) = args.split_at(at);
    let tool = rest[0].as_str();
    let tool_args = &rest[1..];
    if tool != "claude" {
        eprintln!("pygg run knows claude. Codex and OpenCode are set up once with `pygg setup codex-models` or `pygg setup opencode-models`.");
        return 2;
    }
    let model = super::flag(ours, "--model").unwrap_or_else(|| "auto".into());
    let Some(store) = super::flag(ours, "--db").map(PathBuf::from).or_else(super::default_store).filter(|p| p.is_file()) else {
        eprintln!("Pygg's store was not found: open Pygg once, or pass --db PATH");
        return 1;
    };
    let conn = match rusqlite::Connection::open_with_flags(&store, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(c) => c,
        Err(e) => { eprintln!("{}: {e}", store.display()); return 1; }
    };
    let _ = conn.busy_timeout(Duration::from_secs(5));
    let token = match crate::launcher::token(&conn) {
        Ok(t) => t,
        Err(e) => { eprintln!("{e}"); return 1; }
    };
    let port: u16 = crate::gateway::store::setting(&conn, crate::commands::PORT_KEY).ok().flatten()
        .and_then(|p| p.parse().ok()).unwrap_or(crate::gateway::DEFAULT_PORT);
    drop(conn);
    if let Err(e) = super::health(port).await {
        eprintln!("Pygg's gateway on http://127.0.0.1:{port}: {e}. Start the Pygg app first.");
        return 1;
    }
    let Some(launch) = crate::agents::locate("claude") else {
        eprintln!("Claude Code was not found on this computer (PATH or ~/.local/bin).");
        return 1;
    };
    let mut command = std::process::Command::new(&launch.program);
    command.args(&launch.prefix).args(tool_args);
    for name in crate::launcher::removed_variables() {
        command.env_remove(name);
    }
    let session = uuid::Uuid::new_v4().to_string();
    for (k, v) in crate::launcher::environment(&format!("http://127.0.0.1:{port}"), &token, &model, &session) {
        command.env(k, v);
    }
    match command.status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(e) => { eprintln!("Claude Code could not be started: {e}"); 1 }
    }
}

/// Whether `a` is the value of one of Pygg's own flags (`--model auto`), not the tool's name.
fn is_flag_value(args: &[String], a: &str) -> bool {
    args.windows(2).any(|w| (w[0] == "--model" || w[0] == "--db") && w[1] == a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_before_the_tool_are_pyggs() {
        let args: Vec<String> = ["--model", "auto/cheap", "claude", "--model", "x", "-p", "hi"].iter().map(|s| s.to_string()).collect();
        let at = args.iter().position(|a| !a.starts_with("--") && !is_flag_value(&args, a)).unwrap();
        assert_eq!(at, 2);
        assert_eq!(super::super::flag(&args[..at], "--model").as_deref(), Some("auto/cheap"));
    }
}
