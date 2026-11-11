//! Headless agent runner: the same detection, worktree and ledger as the app.
//!
//!   agent detect
//!   agent [--db PATH] run --agent ID --workspace DIR --prompt TEXT [--model M] [--shell yes] [--timeout SECS]
//!                         [--keep TITLE | --discard yes]
//!
//! The agent runs with its own sign-in; nothing here reads or stores it. With --db the run is recorded in that
//! store's ledger. Without --keep or --discard the worktree is left for inspection and its path printed.

use smarttaskrouter_desktop_lib::{agents, db};
use std::collections::HashMap;
use std::path::PathBuf;

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
}

#[tokio::main]
async fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut db_path = None;
    if args.first().map(String::as_str) == Some("--db") {
        args.remove(0);
        db_path = Some(PathBuf::from(if args.is_empty() { fail("--db needs a path") } else { args.remove(0) }));
    }
    let command = if args.is_empty() { fail("usage: agent [--db PATH] detect|run …") } else { args.remove(0) };
    let mut flags: HashMap<String, String> = HashMap::new();
    let mut iter = args.into_iter();
    while let Some(flag) = iter.next() {
        let name = flag.strip_prefix("--").unwrap_or_else(|| fail(format!("unexpected {flag}"))).to_string();
        flags.insert(name.clone(), iter.next().unwrap_or_else(|| fail(format!("--{name} needs a value"))));
    }
    match command.as_str() {
        "detect" => {
            for a in agents::detect().await {
                println!("{:<12} {:<10} {:<40} {}", a.id, if a.installed { "installed" } else { "missing" },
                    a.version.unwrap_or_default(), a.login.unwrap_or_default());
            }
        }
        "run" => {
            let request = agents::TaskRequest {
                agent: flags.get("agent").cloned().unwrap_or_else(|| fail("--agent is required")),
                workspace: PathBuf::from(flags.get("workspace").cloned().unwrap_or_else(|| fail("--workspace is required"))),
                prompt: flags.get("prompt").cloned().unwrap_or_else(|| fail("--prompt is required")),
                model: flags.get("model").cloned(),
                allow_shell: flags.get("shell").map(String::as_str) == Some("yes"),
                timeout_secs: flags.get("timeout").and_then(|t| t.parse().ok()),
            };
            let worktrees = std::env::temp_dir().join("pygg-agent-worktrees");
            let run = agents::run(&request, &worktrees, std::future::pending()).await.unwrap_or_else(|e| fail(e));
            println!("outcome: {} ({}) in {} ms", run.outcome, run.exit, run.duration_ms);
            println!("changes: {} files, +{} -{}", run.files_changed, run.insertions, run.deletions);
            let u = &run.usage;
            println!("model: {}  tokens in/out/cache read: {:?}/{:?}/{:?}  agent's estimate: {:?}",
                u.model.as_deref().unwrap_or("-"), u.input_tokens, u.output_tokens, u.cache_read_tokens, u.estimate_usd);
            if let Some(result) = &u.result {
                println!("result: {}", result.chars().take(400).collect::<String>());
            }
            if let Some(error) = &u.error {
                println!("error: {error}");
            }
            if run.outcome != "ok" {
                println!("log tail:\n{}", run.log_tail);
            }
            if let Some(path) = db_path {
                let conn = db::open(&path).unwrap_or_else(|e| fail(e));
                agents::record(&conn, &run).unwrap_or_else(|e| fail(e));
                println!("recorded in {}", path.display());
            }
            if let Ok(diff) = agents::diff(&run) {
                println!("--- diff ---\n{}", diff.chars().take(2000).collect::<String>());
            }
            match (flags.get("keep"), flags.get("discard")) {
                (Some(title), _) => println!("kept as {}", agents::finish(&run, Some(title)).unwrap_or_else(|e| fail(e)).unwrap_or_default()),
                (None, Some(_)) => { agents::finish(&run, None).unwrap_or_else(|e| fail(e)); println!("discarded"); }
                _ => println!("worktree left at {}", run.worktree),
            }
        }
        other => fail(format!("unknown command {other}")),
    }
}
