//! Headless agent runner: the same detection, worktree and ledger as the app.
//!
//!   agent detect
//!   agent [--db PATH] run --agent ID --workspace DIR --prompt TEXT [--model M] [--shell yes] [--timeout SECS]
//!                         [--keep TITLE | --discard yes]
//!   agent --db PATH task --workspace DIR --prompt TEXT --steps "agent[:model],…" [--check CMD] [--keep TITLE | --discard yes]
//!
//! The agent runs with its own sign-in; nothing here reads or stores it. With --db the run is recorded in that
//! store's ledger. Without --keep or --discard the worktree is left for inspection and its path printed.

use smarttaskrouter_desktop_lib::{agents, db, tasks};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// A smart task, as the app runs it: the workspace's approved checks plus `--check`, then the steps in order.
///
///   agent --db PATH task --workspace DIR --prompt TEXT --steps "opencode:opencode/some-free-model,codex"
///                        [--check CMD] [--shell yes] [--keep TITLE | --discard yes]
///
/// A step is `agent[:model]`; its billing is recorded as unknown, so no step waits for approval here.
async fn task(db_path: PathBuf, flags: &HashMap<String, String>) {
    let need = |name: &str| flags.get(name).cloned().unwrap_or_else(|| fail(format!("--{name} is required")));
    let conn = db::open(&db_path).unwrap_or_else(|e| fail(e));
    // No `tasks::recover` here: the store may be the app's, and its running tasks are not this process's to close.
    let worktrees = std::env::temp_dir().join("pygg-task-worktrees");
    let path = need("workspace");
    let found = tasks::list_workspaces(&conn).unwrap_or_else(|e| fail(e)).into_iter()
        .find(|w| std::fs::canonicalize(&w.path).ok() == std::fs::canonicalize(&path).ok());
    let workspace = match found {
        Some(w) => w,
        None => tasks::add_workspace(&conn, &path, None).unwrap_or_else(|e| fail(e)),
    };
    if let Some(check) = flags.get("check") {
        tasks::add_check(&conn, &workspace.id, check).unwrap_or_else(|e| fail(e));
    }
    let steps = need("steps").split(',').map(str::trim).filter(|s| !s.is_empty()).map(|s| {
        let (agent, model) = s.split_once(':').map(|(a, m)| (a, Some(m.to_string()))).unwrap_or((s, None));
        tasks::Step { agent: agent.into(), model, tier: 2, billing: "unknown".into() }
    }).collect();
    let new = tasks::NewTask { workspace_id: workspace.id.clone(), title: None, prompt: need("prompt"), strategy: "free_first".into(),
        steps, allow_shell: flags.get("shell").map(String::as_str) == Some("yes"), timeout_secs: flags.get("timeout").and_then(|t| t.parse().ok()) };
    let id = tasks::create_task(&conn, &new).unwrap_or_else(|e| fail(e));
    let db = Arc::new(Mutex::new(conn));
    let runner = tasks::Runner::new(db.clone(), worktrees, tasks::agent_executor());
    let handle = runner.start(&id).unwrap_or_else(|| fail("the task is already running"));
    let stopper = runner.clone();
    let stop_id = id.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            eprintln!("stopping…");
            let _ = stopper.cancel(&stop_id);
        }
    });
    handle.await.unwrap_or_else(|e| fail(e));

    let detail = tasks::task_detail(&db.lock().unwrap(), &id).unwrap_or_else(|e| fail(e));
    println!("task {id}: {}{}", detail.summary.state, detail.reason.as_deref().map(|r| format!(" ({r})")).unwrap_or_default());
    for a in &detail.attempt_details {
        fn n(v: Option<impl ToString>) -> String { v.map_or("?".to_string(), |v| v.to_string()) }
        println!("  #{} step {} {:<40} {:<8} {}  files {} +{} -{}  tokens {}/{}", a.number, a.step + 1, a.label, a.outcome,
            a.escalation_reason.as_deref().or(a.detail.as_deref()).unwrap_or(""), n(a.files_changed), n(a.insertions),
            n(a.deletions), n(a.input_tokens), n(a.output_tokens));
        for c in &a.checks {
            println!("      check `{}`: {} ({} ms)", c.command, if c.passed { "passed" } else { &c.detail }, c.duration_ms);
            if !c.passed {
                // What the next step is handed: the end of the check's (redacted) output.
                for line in c.output_tail.lines().rev().take(8).collect::<Vec<_>>().into_iter().rev() {
                    println!("        | {line}");
                }
            }
        }
    }
    let conn = db.lock().unwrap();
    if let Ok(diff) = tasks::result_diff(&conn, &id) {
        println!("--- diff ---\n{}", diff.chars().take(2000).collect::<String>());
        match (flags.get("keep"), flags.get("discard")) {
            (Some(title), _) => println!("kept as {}", tasks::keep(&conn, &id, Some(title)).unwrap_or_else(|e| fail(e))),
            (None, Some(_)) => { tasks::discard(&conn, &id).unwrap_or_else(|e| fail(e)); println!("discarded"); }
            _ => println!("the result waits in the app (or run again with --keep / --discard)"),
        }
    }
}

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
                base: None,
                max_budget_usd: None,
                attempt_id: None,
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
        "task" => task(db_path.unwrap_or_else(|| fail("task needs --db PATH")), &flags).await,
        other => fail(format!("unknown command {other}")),
    }
}
