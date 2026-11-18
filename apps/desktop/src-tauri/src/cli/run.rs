//! `pygg run [--model M] claude [claude's own arguments…]`: Claude Code through Pygg for this one launch
//! (launcher.rs). Its own settings, sign-in and plan are untouched; a plain `claude` is as it was.
//!
//! `pygg run opencode […]` and `pygg run codex […]` (2026-11-14, Codex's fifth discussion, T3): OpenCode or Codex as
//! set up in CLI Tools, with a **launch session**: a fresh id in `PYGG_ATTEMPT_ID`, which their Pygg provider already
//! sends as `x-pygg-session`, so the gateway keeps that launch on the model that answered it (and on its
//! prompt cache). An explicit id per launch, never one made from the prompt.

use std::path::PathBuf;
use std::time::Duration;

pub const HELP: &str = "usage: pygg run [--model M] [--statusline] [--db PATH] claude [arguments for claude…]
       pygg run [--db PATH] opencode [arguments for opencode…]
       pygg run [--db PATH] codex [arguments for codex…]

claude: starts Claude Code with its requests going through Pygg: the gateway's address, the launcher's own Pygg API
key (kept in the system's credential store, never in a file) and the model auto, for this launch only. Claude Code's
own settings and sign-in are not changed; run plain `claude` to use them as before.

  --model M      a model for every slot instead of auto: auto/cheap, a combo, or connection/model
  --statusline   show this launch's cost through Pygg in Claude Code's status line (`pygg statusline`), for this
                 launch only; Claude Code's own status line setting is not changed

opencode, codex: starts the tool as set up in Pygg → CLI Tools (`pygg setup opencode`, `pygg setup
codex-models`; codex runs with --profile pygg), with a routing session of its own for this launch.

Each launch gets its own routing session: the gateway keeps the launch on the model that answered it, while that
model stays eligible, so its prompt cache is kept. An editor or a long-lived tool shares one session across its
conversations until it is started again.

When the tool exits, Pygg prints the launch's receipt: what its requests cost (unknown and subscription ones counted
apart), how much of the prompt came from the cache, which models answered, and what changed in the Git repository the
tool ran in (commits and changed lines since the start). Only the repository's folder, branch and commit ids are kept,
never file names or code.

Set Claude Code up once with `pygg setup claude` or in Pygg → CLI Tools.";

/// The tools `pygg run` starts.
pub const TOOLS: [&str; 3] = ["claude", "opencode", "codex"];

/// Runs `pygg run …`; the exit code is the tool's, or Pygg's own for a problem before it starts.
pub async fn run(args: Vec<String>) -> i32 {
    // Pygg's flags come before the tool's name; everything after it is the tool's, `--help` included
    // (`pygg run claude --help` is Claude Code's help).
    let at = args.iter().position(|a| !a.starts_with("--") && !is_flag_value(&args, a));
    if args.is_empty() || super::wants_help(&args[..at.unwrap_or(args.len())]) {
        println!("{HELP}");
        return if args.is_empty() { 2 } else { 0 };
    }
    let Some(at) = at else {
        eprintln!(
            "pygg: name the tool to run.
Use: pygg run claude, pygg run opencode or pygg run codex"
        );
        return 2;
    };
    if let Some(unknown) =
        args[..at].iter().find(|a| a.starts_with("--") && !matches!(a.as_str(), "--model" | "--db" | "--statusline"))
    {
        eprintln!(
            "pygg: unknown option {unknown} for pygg run (Pygg's options go before the tool's name: --model, --statusline, --db).
Use: pygg run --help"
        );
        return 2;
    }
    let (ours, rest) = args.split_at(at);
    let tool = rest[0].as_str();
    let tool_args = &rest[1..];
    if !TOOLS.contains(&tool) {
        eprintln!("pygg run knows claude, opencode and codex.");
        return 2;
    }
    if tool != "claude" && super::flag(ours, "--model").is_some() {
        eprintln!("--model is for claude. Choose {tool}'s model in {tool} itself (Pygg's provider lists auto first).");
        return 2;
    }
    let statusline = ours.iter().any(|a| a == "--statusline");
    if tool != "claude" && statusline {
        eprintln!("--statusline is for claude: OpenCode and Codex have no status line command.");
        return 2;
    }
    let Some(store) =
        super::flag(ours, "--db").map(PathBuf::from).or_else(super::default_store).filter(|p| p.is_file())
    else {
        eprintln!("Pygg's store was not found: open Pygg once, or pass --db PATH");
        return 1;
    };
    let conn = match rusqlite::Connection::open_with_flags(&store, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: {e}", store.display());
            return 1;
        }
    };
    let _ = conn.busy_timeout(Duration::from_secs(5));
    let token = if tool == "claude" {
        match crate::launcher::token(&conn) {
            Ok(t) => Some(t),
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        }
    } else {
        // OpenCode and Codex keep their own key in the configuration Pygg wrote.
        if let Err(e) = crate::integrations::launch_ready(tool, &crate::integrations::Homes::from_env()) {
            eprintln!("{e}");
            return 1;
        }
        None
    };
    let port: u16 = crate::gateway::store::setting(&conn, crate::gateway::PORT_KEY)
        .ok()
        .flatten()
        .and_then(|p| p.parse().ok())
        .unwrap_or(crate::gateway::DEFAULT_PORT);
    drop(conn);
    if let Err(e) = super::health(port).await {
        eprintln!("Pygg's gateway on http://127.0.0.1:{port}: {e}. Next: {}.", super::START_GATEWAY);
        return 1;
    }
    let Some(launch) = crate::agents::locate(tool) else {
        eprintln!("{} was not found on this computer (PATH or ~/.local/bin).", tool_name(tool));
        return 1;
    };
    let mut command = std::process::Command::new(&launch.program);
    command.args(&launch.prefix);
    let session = crate::launcher::launch_session();
    let model = super::flag(ours, "--model");
    match token {
        Some(token) => {
            if statusline {
                command.args(statusline_settings());
            }
            command.args(tool_args);
            for name in crate::launcher::removed_variables() {
                command.env_remove(name);
            }
            let model = model.clone().unwrap_or_else(|| "auto".into());
            for (k, v) in crate::launcher::environment(&format!("http://127.0.0.1:{port}"), &token, &model, &session) {
                command.env(k, v);
            }
        }
        None => {
            command.args(tool_launch_args(tool, tool_args)).args(tool_args);
            command.env(crate::agents::ATTEMPT_ENV, &session);
        }
    }
    let folder = std::env::current_dir().ok();
    let before = folder.as_deref().and_then(crate::launches::git_state);
    let recorder = recorder(&store);
    let saved = recorder.as_ref().is_some_and(|c| {
        crate::launches::record_start(c, &session, tool, model.as_deref(), crate::db::now_ms(), before.as_ref()).is_ok()
    });
    let code = match command.status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(e) => {
            eprintln!("{} could not be started: {e}", tool_name(tool));
            return 1;
        }
    };
    let change = folder.as_deref().zip(before.as_ref()).and_then(|(f, b)| crate::launches::git_change(f, b));
    if let (Some(c), true) = (&recorder, saved) {
        let _ = crate::launches::record_end(c, &session, crate::db::now_ms(), code, change.as_ref());
    }
    // The gateway settles an attempt's row just after its stream ends.
    tokio::time::sleep(Duration::from_millis(800)).await;
    let note = (!saved).then_some("This launch was not saved: open the updated Pygg once so its store has launches.");
    match print_receipt(&store, tool, &session, before.as_ref().zip(change.as_ref()), note) {
        Ok(text) => eprintln!("\n{text}"),
        Err(e) => eprintln!("\nPygg could not read this launch's cost: {e}"),
    }
    code
}

/// A connection that may write the launch's row: only to a store that already has the table (the app migrates it,
/// `pygg run` never does).
fn recorder(store: &std::path::Path) -> Option<rusqlite::Connection> {
    let conn = rusqlite::Connection::open_with_flags(store, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE).ok()?;
    conn.busy_timeout(Duration::from_secs(5)).ok()?;
    crate::launches::can_record(&conn).then_some(conn)
}

fn print_receipt(
    store: &std::path::Path, tool: &str, session: &str,
    git: Option<(&crate::launches::GitState, &crate::launches::GitChange)>, note: Option<&str>,
) -> Result<String, String> {
    let conn = rusqlite::Connection::open_with_flags(store, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    let _ = conn.busy_timeout(Duration::from_secs(5));
    let cost = crate::launches::session_cost(&conn, session).map_err(|e| e.to_string())?;
    let savings = crate::gateway::savings::session_savings(&conn, session).ok();
    let compared = savings
        .as_ref()
        .filter(|s| s.baseline_priced && s.compared > 0)
        .and_then(|s| s.baseline.as_deref().map(|b| (b, s.routed_micro_usd, s.baseline_micro_usd)));
    let classes = crate::gateway::hints::request_classes(&conn, 0, Some(session)).ok();
    // A store older than the session and day limits has none to show.
    let limits = crate::gateway::store::session_limits(&conn, session, crate::db::now_ms()).ok().flatten();
    Ok(crate::launches::receipt(
        tool_name(tool),
        session,
        &cost,
        compared,
        classes.as_ref(),
        git,
        note,
        limits.as_ref(),
    ))
}

/// Claude Code's `--settings` for this launch: its status line runs `pygg statusline` (this program).
fn statusline_settings() -> [String; 2] {
    let program =
        std::env::current_exe().map(|p| p.display().to_string().replace('\\', "/")).unwrap_or_else(|_| "pygg".into());
    let command = format!("\"{program}\" statusline");
    let settings = serde_json::json!({ "statusLine": { "type": "command", "command": command, "padding": 0 } });
    ["--settings".into(), settings.to_string()]
}

fn tool_name(tool: &str) -> &'static str {
    match tool {
        "claude" => "Claude Code",
        "opencode" => "OpenCode",
        _ => "Codex",
    }
}

/// What Pygg puts before the tool's own arguments: Codex is told to use Pygg's profile, unless the user named one.
fn tool_launch_args(tool: &str, tool_args: &[String]) -> Vec<String> {
    let named = tool_args.iter().any(|a| a == "--profile" || a == "-p" || a.starts_with("--profile="));
    if tool == "codex" && !named {
        vec!["--profile".into(), crate::integrations::CODEX_PROFILE.into()]
    } else {
        Vec::new()
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
        let args: Vec<String> =
            ["--model", "auto/cheap", "claude", "--model", "x", "-p", "hi"].iter().map(|s| s.to_string()).collect();
        let at = args.iter().position(|a| !a.starts_with("--") && !is_flag_value(&args, a)).unwrap();
        assert_eq!(at, 2);
        assert_eq!(super::super::flag(&args[..at], "--model").as_deref(), Some("auto/cheap"));
    }

    #[test]
    fn codex_runs_with_pyggs_profile_unless_another_is_named() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(tool_launch_args("codex", &args(&["exec", "hi"])), args(&["--profile", "pygg"]));
        assert!(tool_launch_args("codex", &args(&["--profile", "work"])).is_empty());
        assert!(tool_launch_args("opencode", &args(&[])).is_empty());
    }
}
