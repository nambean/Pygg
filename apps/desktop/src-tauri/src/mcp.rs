//! The MCP server (stdio), run as `<Pygg app> mcp` or `pygg-mcp` that lets an IDE agent (Claude Code, Codex, Cursor, …) hand a coding
//! subtask to Pygg. The task runs in the Pygg app: in a Git worktree of the repository, checked by the
//! workspace's approved checks, escalating across agents on failure. The agent's own working tree is never
//! touched; a result becomes a branch only when kept.
//!
//! Configuration (environment):
//!   PYGG_TOKEN  a client token from Pygg → Endpoint → API Keys, allowed to start tasks (Limits… → May start tasks)
//!   PYGG_URL    the gateway, default http://127.0.0.1:8741
//!
//! Example: claude mcp add pygg -e PYGG_TOKEN=str-… -- "C:\path\to\Pygg.exe" mcp
//! The app's own program serves it, so the installer ships nothing extra; `pygg-mcp` is the same server for
//! development and for machines with only the headless tools.
//!
//! The server forwards to the gateway's task API (gateway/task_api.rs); it holds no state of its own.

use crate::gateway::DEFAULT_PORT;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const PROTOCOL: &str = "2025-06-18";
/// Under the 60 s many MCP clients allow a tool call by default (Codex's `tool_timeout_sec`); an agent that
/// wants to wait longer calls pygg_task_status again.
const DEFAULT_WAIT: u64 = 50;

struct Gateway {
    base: String,
    token: Option<String>,
    http: reqwest::Client,
}

impl Gateway {
    async fn call(&self, method: reqwest::Method, path: &str, body: Option<Value>) -> Result<Value, String> {
        let token = self.token.as_deref().ok_or(
            "PYGG_TOKEN is not set. Create a client token in Pygg → Endpoint → API Keys, allow it to start tasks (Limits… → May start tasks), \
             and register this server with it, e.g. `claude mcp add pygg -e PYGG_TOKEN=… -- pygg-mcp`.")?;
        let mut request = self.http.request(method, format!("{}{path}", self.base)).bearer_auth(token);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.map_err(|e| {
            if e.is_connect() {
                format!("Pygg is not reachable at {}. Open the Pygg app (its gateway must be running).", self.base)
            } else {
                e.to_string()
            }
        })?;
        let status = response.status();
        let value: Value = response.json().await.unwrap_or(Value::Null);
        if status.is_success() {
            Ok(value)
        } else {
            Err(value["error"]["message"]
                .as_str()
                .map(String::from)
                .unwrap_or_else(|| format!("Pygg answered {status}")))
        }
    }
}

fn tools() -> Value {
    let id = json!({"type": "string", "description": "The task id returned by pygg_run_task."});
    let wait = json!({"type": "integer", "minimum": 0, "maximum": 600,
        "description": "Seconds to wait for the task to finish before answering (default 50; raise it only if your client allows tool calls that long)."});
    json!([
        {"name": "pygg_run_task",
         "description": "Hand a self-contained coding subtask to other agents (OpenCode, Codex CLI, Claude Code, Gemini CLI), each with its own sign-in, cheapest first. Pygg runs it in a separate Git worktree of this repository, checks the result with the checks the user approved for it, and moves to the next agent when a check fails. Your working tree is never changed: a result becomes a branch only when you keep it. Describe the change completely: the agent does not see this conversation.",
         "inputSchema": {"type": "object", "required": ["prompt"], "properties": {
            "prompt": {"type": "string", "description": "The full task: what to change, where, and what done looks like."},
            "title": {"type": "string", "description": "A short title, also used for the branch name."},
            "workspace": {"type": "string", "description": "A folder in the repository. Default: the folder this server runs in."},
            "strategy": {"type": "string", "enum": ["free_first", "balanced", "quality"],
                "description": "Which agents go first when steps are not given. Default free_first."},
            "steps": {"type": "array", "description": "The agents to try, in order. Default: the installed agents in the strategy's order.",
                "items": {"type": "object", "required": ["agent"], "properties": {
                    "agent": {"type": "string", "enum": ["opencode", "codex", "claude-code", "gemini-cli"]},
                    "model": {"type": "string", "description": "In the agent's own naming; default: its default."}}}},
            "allow_shell": {"type": "boolean", "description": "Let the agents run shell commands in their copy (default false)."},
            "wait_seconds": wait.clone()}}},
        {"name": "pygg_task_status", "description": "The state of a task, its attempts and why each failed. Waits while it runs.",
         "inputSchema": {"type": "object", "required": ["task_id"], "properties": {"task_id": id.clone(), "wait_seconds": wait}}},
        {"name": "pygg_task_diff", "description": "The changes of a finished task's result, as a unified diff.",
         "inputSchema": {"type": "object", "required": ["task_id"], "properties": {"task_id": id.clone()}}},
        {"name": "pygg_task_brief", "description": "A task as a brief to continue it elsewhere: the goal, where it started, each attempt and why it stopped, the checks that define done, and where it stands.",
         "inputSchema": {"type": "object", "required": ["task_id"], "properties": {"task_id": id.clone()}}},
        {"name": "pygg_keep_result", "description": "Keep a finished task's result as a new branch of the repository (your checkout is not changed). Returns the branch name, which you can merge or check out. A result no check vouches for (pygg_task_status says NOT READY) is refused unless accept_unchecked is true.",
         "inputSchema": {"type": "object", "required": ["task_id"], "properties": {"task_id": id.clone(),
            "title": {"type": "string", "description": "Commit message and branch title. Default: the task's title."},
            "accept_unchecked": {"type": "boolean", "description": "Keep a NOT READY result anyway: only after you read its whole diff with pygg_task_diff and judged it yourself, since no check confirmed it. Say so to the user."}}}},
        {"name": "pygg_discard_result", "description": "Discard a finished task's result.",
         "inputSchema": {"type": "object", "required": ["task_id"], "properties": {"task_id": id.clone()}}},
        {"name": "pygg_stop_task", "description": "Stop a running or waiting task.",
         "inputSchema": {"type": "object", "required": ["task_id"], "properties": {"task_id": id}}},
        {"name": "pygg_list_tasks", "description": "Tasks you started, newest first.",
         "inputSchema": {"type": "object", "properties": {}}},
        {"name": "pygg_agents", "description": "The agents installed on this computer that tasks can use.",
         "inputSchema": {"type": "object", "properties": {}}},
        {"name": "pygg_brainstorm",
         "description": "A brainstorm pygg/brain made for you (ask for the model pygg/brain with a requirement to start one): its state, planner, open questions and cards with the model Pygg picked for each. Approval happens in the Pygg app, never here.",
         "inputSchema": {"type": "object", "properties": {
            "brainstorm_id": {"type": "string", "description": "The id in the brain's reply (b_…). Default: your newest brainstorm."}}}},
        {"name": "pygg_board",
         "description": "The Kanban board of a workspace: your brainstorms' cards by column (discovery, ready, running, checking, blocked, review, done), each with its task's state. Paid steps are approved in the Pygg app, never here.",
         "inputSchema": {"type": "object", "required": ["workspace_id"], "properties": {
            "workspace_id": {"type": "string", "description": "The workspace's id (pygg_brainstorm shows it once a brainstorm was approved)."}}}},
        {"name": "pygg_board_move",
         "description": "Move one of your cards between Discovery and Ready: to ready releases a held card, to discovery holds it. Nothing else moves by hand; the engine moves the rest.",
         "inputSchema": {"type": "object", "required": ["card_id", "to"], "properties": {
            "card_id": {"type": "string", "description": "The board card id (bc_…)."}, "to": {"type": "string", "enum": ["ready", "discovery"]}}}},
        {"name": "pygg_model_facts",
         "description": "The user's connected models ranked for a role (plan, code, test, review) from Pygg's model facts feed: each with the public benchmark it ranks by, its place and date, or its tier when no fact applies.",
         "inputSchema": {"type": "object", "properties": {
            "role": {"type": "string", "enum": ["plan", "code", "test", "review"], "description": "Default code."}}}},
    ])
}

/// What an agent needs to know about a task, and what to do next.
pub(crate) fn describe(task: &Value) -> String {
    let s = |v: &Value| v.as_str().unwrap_or("").to_string();
    let mut out = format!("Task {} \"{}\": {}", s(&task["id"]), s(&task["title"]), s(&task["state"]));
    if let Some(reason) = task["reason"].as_str() {
        out.push_str(&format!(" — {reason}"));
    }
    out.push('\n');
    for a in task["attemptDetails"].as_array().into_iter().flatten() {
        out.push_str(&format!("  {}. {} — {}", a["number"], s(&a["label"]), s(&a["outcome"])));
        if let Some(why) = a["escalationReason"].as_str() {
            out.push_str(&format!(": {why}"));
        }
        if let Some(files) = a["filesChanged"].as_u64() {
            out.push_str(&format!(" ({files} files, +{} −{})", a["insertions"], a["deletions"]));
        }
        out.push('\n');
        for c in a["checks"].as_array().into_iter().flatten() {
            let verdict = if c["passed"].as_bool() == Some(true) { "passed" } else { "FAILED" };
            out.push_str(&format!(
                "     check `{}`: {verdict} ({}, {:.1} s)\n",
                s(&c["command"]),
                s(&c["detail"]),
                c["durationMs"].as_u64().unwrap_or(0) as f64 / 1000.0
            ));
        }
        // What that attempt was told about earlier ones (ADR-030), so the calling agent sees the same as the user.
        if let Some(sent) = a["handoffSent"].as_str() {
            out.push_str("     It was given, after the task:\n");
            for line in sent.lines() {
                out.push_str(&format!("     | {line}\n"));
            }
        }
    }
    let next = match task["state"].as_str().unwrap_or("") {
        "running" => "Still running. Call pygg_task_status with wait_seconds to wait for it.".to_string(),
        "waiting_approval" => "Waiting for the user to approve a paid step in the Pygg app. Tell the user; check again later.".into(),
        "checks_passed" if task["result"].is_null() =>
            "The approved checks passed. Review the changes with pygg_task_diff, then pygg_keep_result (a new branch) or pygg_discard_result.".into(),
        "needs_review" if task["result"].is_null() && task["checked"] == "definitions_changed" =>
            "NOT READY: the checks passed, but this result changed a file that defines them, so they do not vouch for it. Read the whole diff with pygg_task_diff; keep it only if you judge it correct yourself (pygg_keep_result with accept_unchecked, and tell the user it is unchecked), or pygg_discard_result.".into(),
        "needs_review" if task["result"].is_null() =>
            "NOT READY: no check ran on this result, so nothing confirmed it. Read the whole diff with pygg_task_diff; keep it only if you judge it correct yourself (pygg_keep_result with accept_unchecked, and tell the user it is unchecked), or pygg_discard_result. The user can approve checks for this workspace in the Pygg app.".into(),
        "checks_passed" | "manually_accepted" | "needs_review" | "canceled" if !task["resultBranch"].is_null() =>
            format!("Kept as branch {}.", s(&task["resultBranch"])),
        "answered" => format!(
            "Answered; nothing was changed. The answer:
{}",
            task["attemptDetails"]
                .as_array()
                .and_then(|a| a.iter().rev().find(|a| a["outcome"] == "done"))
                .and_then(|a| a["summary"].as_str())
                .unwrap_or("(no text)")
        ),
        "interrupted" => "The Pygg app closed while it ran; the user can retry it in the app.".into(),
        "check_failed" => "Every step failed. Read the reasons above; you may do the task yourself or start a new one with more detail.".into(),
        _ => String::new(),
    };
    if !next.is_empty() {
        out.push_str(&next);
    }
    out
}

fn wait_of(args: &Value) -> u64 {
    args["wait_seconds"].as_u64().unwrap_or(DEFAULT_WAIT).min(600)
}

fn task_id(args: &Value) -> Result<String, String> {
    let id = args["task_id"].as_str().unwrap_or("").trim();
    let ok = !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric());
    if ok {
        Ok(id.to_string())
    } else {
        Err("task_id must be the id pygg_run_task returned".into())
    }
}

async fn call_tool(gateway: &Gateway, name: &str, args: &Value) -> Result<String, String> {
    use reqwest::Method;
    match name {
        "pygg_run_task" => {
            let workspace = args["workspace"].as_str().map(String::from).unwrap_or_else(|| {
                std::env::current_dir().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default()
            });
            let steps = args["steps"].as_array().map(|steps| {
                steps
                    .iter()
                    .map(|s| {
                        json!({
                            "agent": s["agent"], "model": s["model"].as_str(),
                        })
                    })
                    .collect::<Vec<_>>()
            });
            let body = json!({"workspace": workspace, "prompt": args["prompt"], "title": args["title"].as_str(),
                "strategy": args["strategy"].as_str(), "steps": steps, "allowShell": args["allow_shell"].as_bool().unwrap_or(false)});
            let created = gateway.call(Method::POST, "/pygg/v1/tasks", Some(body)).await?;
            let id = created["id"].as_str().unwrap_or_default().to_string();
            let task = gateway.call(Method::GET, &format!("/pygg/v1/tasks/{id}?wait={}", wait_of(args)), None).await?;
            Ok(describe(&task))
        }
        "pygg_task_status" => {
            let task = gateway
                .call(Method::GET, &format!("/pygg/v1/tasks/{}?wait={}", task_id(args)?, wait_of(args)), None)
                .await?;
            Ok(describe(&task))
        }
        "pygg_task_brief" => {
            let brief = gateway.call(Method::GET, &format!("/pygg/v1/tasks/{}/brief", task_id(args)?), None).await?;
            Ok(brief["brief"].as_str().unwrap_or("").to_string())
        }
        "pygg_task_diff" => {
            let diff = gateway.call(Method::GET, &format!("/pygg/v1/tasks/{}/diff", task_id(args)?), None).await?;
            Ok(diff["diff"].as_str().unwrap_or("").to_string())
        }
        "pygg_keep_result" => {
            let kept = gateway
                .call(
                    Method::POST,
                    &format!("/pygg/v1/tasks/{}/keep", task_id(args)?),
                    Some(json!({"title": args["title"].as_str(),
                        "acceptUnchecked": args["accept_unchecked"].as_bool().unwrap_or(false)})),
                )
                .await?;
            Ok(format!(
                "Kept as branch {}. Your checkout was not changed; merge or check out the branch to use it.",
                kept["branch"].as_str().unwrap_or("")
            ))
        }
        "pygg_discard_result" => {
            gateway.call(Method::POST, &format!("/pygg/v1/tasks/{}/discard", task_id(args)?), None).await?;
            Ok("Discarded.".into())
        }
        "pygg_stop_task" => {
            gateway.call(Method::POST, &format!("/pygg/v1/tasks/{}/stop", task_id(args)?), None).await?;
            Ok("Stopping.".into())
        }
        "pygg_list_tasks" => {
            let list = gateway.call(Method::GET, "/pygg/v1/tasks", None).await?;
            let lines: Vec<String> = list["tasks"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|t| {
                    format!(
                        "{} {} — {}",
                        t["id"].as_str().unwrap_or(""),
                        t["state"].as_str().unwrap_or(""),
                        t["title"].as_str().unwrap_or("")
                    )
                })
                .collect();
            Ok(if lines.is_empty() { "No tasks yet.".into() } else { lines.join("\n") })
        }
        "pygg_agents" => {
            let agents = gateway.call(Method::GET, "/pygg/v1/agents", None).await?;
            Ok(agents["agents"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|a| {
                    format!(
                        "{} ({}): {}",
                        a["id"].as_str().unwrap_or(""),
                        if a["installed"] == true { "installed" } else { "not installed" },
                        a["signIn"].as_str().unwrap_or("")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"))
        }
        "pygg_brainstorm" => {
            let b = match args["brainstorm_id"].as_str().filter(|s| !s.is_empty()) {
                Some(id) => gateway.call(Method::GET, &format!("/pygg/v1/brainstorms/{id}"), None).await?,
                None => {
                    let list = gateway.call(Method::GET, "/pygg/v1/brainstorms", None).await?;
                    list["brainstorms"]
                        .as_array()
                        .and_then(|l| l.first().cloned())
                        .ok_or("You have no brainstorm yet: ask for the model pygg/brain with a requirement.")?
                }
            };
            let s = |v: &Value| v.as_str().unwrap_or("").to_string();
            let mut out = format!(
                "Brainstorm {} · {} · revision {} · planner {}\nRequirement: {}\n",
                s(&b["id"]),
                s(&b["state"]),
                b["revision"].as_u64().unwrap_or(0),
                b["planner"].as_str().unwrap_or("none yet"),
                s(&b["requirement"])
            );
            if let Some(note) = b["note"].as_str().or_else(|| b["error"].as_str()) {
                out.push_str(&format!("Note: {note}\n"));
            }
            for q in b["proposal"]["questions"].as_array().into_iter().flatten().filter(|q| q["answered"] != true) {
                out.push_str(&format!("Question {}: {}\n", s(&q["id"]), s(&q["text"])));
            }
            for c in b["proposal"]["cards"].as_array().into_iter().flatten().filter(|c| c["dropped"] != true) {
                out.push_str(&format!(
                    "Card {} [{}] wave {}: {} -> {} · done when: {}\n",
                    s(&c["id"]),
                    s(&c["role"]),
                    c["wave"].as_u64().unwrap_or(0),
                    s(&c["title"]),
                    c["model"].as_str().unwrap_or("nothing eligible"),
                    s(&c["done"])
                ));
            }
            out.push_str("Approve or change it in the Pygg app; this tool cannot.");
            Ok(out)
        }
        "pygg_board" => {
            let ws = args["workspace_id"].as_str().unwrap_or("");
            let board = gateway.call(Method::GET, &format!("/pygg/v1/board?workspace={ws}"), None).await?;
            let s = |v: &Value| v.as_str().unwrap_or("").to_string();
            let mut out = format!(
                "Board of {} · Running {}/{}\n",
                s(&board["workspaceName"]),
                board["cards"].as_array().map(|c| c.iter().filter(|x| x["lane"] == "running").count()).unwrap_or(0),
                board["limits"]["wipRunning"].as_u64().unwrap_or(1)
            );
            for lane in ["blocked", "review", "running", "checking", "ready", "discovery", "done"] {
                for c in board["cards"].as_array().into_iter().flatten().filter(|c| c["lane"] == lane) {
                    out.push_str(&format!(
                        "[{lane}] {} {} ({}) {}{}\n",
                        s(&c["id"]),
                        s(&c["title"]),
                        s(&c["role"]),
                        c["task"]["state"]
                            .as_str()
                            .map(|st| format!("task {st}"))
                            .unwrap_or_else(|| "no task yet".into()),
                        c["blockedReason"].as_str().map(|r| format!(" · {r}")).unwrap_or_default()
                    ));
                }
            }
            Ok(out)
        }
        "pygg_board_move" => {
            let card = args["card_id"].as_str().unwrap_or("");
            let to = args["to"].as_str().unwrap_or("ready");
            let c = gateway.call(Method::POST, &format!("/pygg/v1/board/{card}/move"), Some(json!({"to": to}))).await?;
            Ok(format!(
                "Card {} is in {} now{}",
                c["id"].as_str().unwrap_or(card),
                c["lane"].as_str().unwrap_or("?"),
                if c["held"] == true { " (held)" } else { "" }
            ))
        }
        "pygg_model_facts" => {
            let role = args["role"].as_str().unwrap_or("code");
            let facts = gateway.call(Method::GET, &format!("/pygg/v1/facts?role={role}"), None).await?;
            let mut out = format!(
                "Models for {role} (feed built {}):\n",
                facts["built"].as_str().unwrap_or("none kept; by tier")
            );
            for m in facts["models"].as_array().into_iter().flatten().take(30) {
                let line = match m["fact"].as_object() {
                    Some(f) => format!(
                        "{} #{} of {} · {} · {}{}",
                        f["sourceName"].as_str().unwrap_or(""),
                        f["place"].as_u64().unwrap_or(0),
                        f["of"].as_u64().unwrap_or(0),
                        f["score"],
                        f["date"].as_str().unwrap_or("undated"),
                        if f["derived"] == true { " · derived" } else { "" }
                    ),
                    None => format!("no fact for {role}; tier {}", m["tier"].as_str().unwrap_or("none")),
                };
                out.push_str(&format!(
                    "{} ({}): {line}\n",
                    m["model"].as_str().unwrap_or(""),
                    m["billing"].as_str().unwrap_or("")
                ));
            }
            Ok(out)
        }
        other => Err(format!("Unknown tool {other}")),
    }
}

async fn handle(gateway: &Gateway, message: &Value) -> Option<Value> {
    let id = message.get("id").cloned();
    let method = message["method"].as_str().unwrap_or("");
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": message["params"]["protocolVersion"].as_str().unwrap_or(PROTOCOL),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "pygg", "version": env!("CARGO_PKG_VERSION")},
            "instructions": "Use pygg_run_task to hand a well-defined coding subtask (a bug fix, tests, a refactor) to cheaper agents. It runs in a separate worktree and is checked by the repository's approved checks; you review and keep or discard the result.",
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tools()})),
        "tools/call" => {
            let params = &message["params"];
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            Ok(match call_tool(gateway, params["name"].as_str().unwrap_or(""), &args).await {
                Ok(text) => json!({"content": [{"type": "text", "text": text}]}),
                Err(text) => json!({"content": [{"type": "text", "text": text}], "isError": true}),
            })
        }
        _ if id.is_none() => return None, // a notification, e.g. notifications/initialized
        other => Err(json!({"code": -32601, "message": format!("Method not found: {other}")})),
    };
    let id = id?;
    Some(match result {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err(error) => json!({"jsonrpc": "2.0", "id": id, "error": error}),
    })
}

/// Serves MCP on stdin and stdout until stdin closes.
pub fn run() {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime");
    runtime.block_on(serve());
}

async fn serve() {
    let gateway = Gateway {
        base: std::env::var("PYGG_URL")
            .ok()
            .map(|u| u.trim_end_matches('/').to_string())
            .unwrap_or_else(|| format!("http://127.0.0.1:{DEFAULT_PORT}")),
        token: std::env::var("PYGG_TOKEN").ok().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()),
        http: reqwest::Client::builder().timeout(Duration::from_secs(660)).build().expect("HTTP client"),
    };
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    while let Ok(Some(line)) = lines.next_line().await {
        // Some shells (Windows PowerShell) start piped input with a byte-order mark.
        let line = line.trim_start_matches('\u{feff}');
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(line) {
            Ok(message) => handle(&gateway, &message).await,
            Err(e) => Some(json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": e.to_string()}})),
        };
        if let Some(reply) = reply {
            let mut text = reply.to_string();
            text.push('\n');
            if stdout.write_all(text.as_bytes()).await.is_err() || stdout.flush().await.is_err() {
                break;
            }
        }
    }
}
