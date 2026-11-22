//! Official agent CLIs as executors (owner decision 2026-11-11, docs/AGENTS.md). Pygg hands a task to the
//! agent the user already has (Claude Code, Codex CLI, Gemini CLI, OpenCode), signed in its own way: a
//! subscription login, the vendor's free tier or a key the agent manages. Pygg never reads, stores or forwards
//! that sign-in, and never offers it to other clients; it starts the agent's own program in a Git worktree of
//! the user's repository, reads what the agent reports, and records it in the ledger.
//!
//! - Detection: the program on PATH (or the usual install folder), its version, and for Codex the result of
//!   `codex login status`, the CLI's own status command.
//! - A run: one detached worktree per task (ADR-008), the agent started headless in it with edits allowed
//!   and nothing else unless asked, a time limit and cancellation (process.rs), then the diff. The user keeps
//!   the result as a branch or discards it.
//! - Usage: tokens and the agent's own cost estimate when it reports them. With a subscription sign-in that
//!   estimate is not money charged, so the ledger records the cost as unknown, with the estimate in its source.

use crate::gateway::pricing::Cost;
use crate::gateway::store::{self, UsageRecord};
use crate::isolation::{self, Worktree};
use crate::process::{self, Exit, Spec};
use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// An agent CLI Pygg knows how to run headless.
#[derive(Clone, Copy, Debug)]
pub struct AgentSpec {
    pub id: &'static str,
    pub name: &'static str,
    /// Program names to look for, in order.
    pub programs: &'static [&'static str],
    /// How the agent signs in, in the user's words.
    pub sign_in: &'static str,
    pub install: &'static str,
}

pub const AGENTS: [AgentSpec; 4] = [
    AgentSpec { id: "claude-code", name: "Claude Code", programs: &["claude"],
        sign_in: "Your Claude subscription or Anthropic Console login, managed by Claude Code (`claude` then /login).",
        install: "https://docs.claude.com/en/docs/claude-code/setup" },
    AgentSpec { id: "codex", name: "Codex CLI", programs: &["codex"],
        sign_in: "Your ChatGPT plan or an OpenAI API key, managed by Codex (`codex login`).",
        install: "https://developers.openai.com/codex/cli" },
    AgentSpec { id: "gemini-cli", name: "Gemini CLI", programs: &["gemini"],
        // On 2026-11-11 Google answered a Google-account login with "This client is no longer supported for Gemini
        // Code Assist for individuals"; a Gemini API key still works.
        sign_in: "A Gemini API key set in Gemini CLI (run `gemini`, choose the API key). Google no longer accepts its free Google-account login from this CLI.",
        install: "https://github.com/google-gemini/gemini-cli" },
    AgentSpec { id: "opencode", name: "OpenCode", programs: &["opencode"],
        sign_in: "The providers configured in OpenCode (`opencode auth login`).",
        install: "https://opencode.ai/docs" },
];

pub fn find(id: &str) -> Option<&'static AgentSpec> {
    AGENTS.iter().find(|a| a.id == id)
}

/// How to start a program: directly, or through Node for an npm shim whose target is a script.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Launch {
    pub program: String,
    pub prefix: Vec<String>,
}

/// Looks for `name` on PATH, then in `~/.local/bin`, where the official native installers put Claude Code even
/// when that folder is not on PATH yet. On Windows, npm installs `.cmd` shims; their target (an `.exe`, or a `.js`
/// run by Node) is started directly, because passing a free-text prompt through `cmd` is not safe to quote.
pub fn locate(name: &str) -> Option<Launch> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    let fallback = home.map(|h| h.join(".local").join("bin"));
    for dir in std::env::split_paths(&path).chain(fallback) {
        if cfg!(windows) {
            let exe = dir.join(format!("{name}.exe"));
            if exe.is_file() {
                return Some(Launch { program: exe.to_string_lossy().into_owned(), prefix: Vec::new() });
            }
            let shim = dir.join(format!("{name}.cmd"));
            if shim.is_file() {
                if let Some(launch) = read_npm_shim(&shim) {
                    return Some(launch);
                }
            }
        } else {
            let file = dir.join(name);
            if file.is_file() {
                return Some(Launch { program: file.to_string_lossy().into_owned(), prefix: Vec::new() });
            }
        }
    }
    None
}

/// The target of an npm `.cmd` shim: `"%dp0%\node_modules\…\x.exe"` or `"%dp0%\node_modules\…\x.js"`.
fn read_npm_shim(shim: &Path) -> Option<Launch> {
    let text = std::fs::read_to_string(shim).ok()?;
    let dir = shim.parent()?;
    // The shim also names `%dp0%\node.exe` (the Node it prefers); the target is the other quoted path.
    let path = text
        .split('"')
        .filter(|part| part.starts_with("%dp0%\\") && !part.ends_with("\\node.exe"))
        .filter(|part| [".js", ".cjs", ".mjs", ".exe"].iter().any(|ext| part.ends_with(ext)))
        .map(|part| dir.join(part.trim_start_matches("%dp0%\\")))
        .find(|path| path.is_file())?;
    let path = path.to_string_lossy().into_owned();
    if path.ends_with(".exe") {
        return Some(Launch { program: path, prefix: Vec::new() });
    }
    let node = dir.join("node.exe");
    let node = if node.is_file() { node.to_string_lossy().into_owned() } else { "node".to_string() };
    Some(Launch { program: node, prefix: vec![path] })
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detected {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub program: Option<String>,
    pub version: Option<String>,
    /// What the CLI's own status command says, where it has one (Codex: "Logged in using ChatGPT").
    pub login: Option<String>,
    pub sign_in: String,
    pub install: String,
}

fn command(launch: &Launch, args: Vec<String>, cwd: &Path, timeout: Duration) -> Spec {
    Spec {
        program: launch.program.clone(),
        args,
        raw_tail: None,
        cwd: cwd.to_path_buf(),
        env: Vec::new(),
        remove_env: Vec::new(),
        timeout,
        output_limit: 2 * 1024 * 1024,
    }
}

async fn quick(launch: &Launch, args: &[&str]) -> Option<String> {
    let args = launch.prefix.iter().cloned().chain(args.iter().map(|a| a.to_string())).collect();
    let out =
        process::run(&command(launch, args, &std::env::temp_dir(), Duration::from_secs(20)), std::future::pending())
            .await;
    let text = format!("{}{}", out.stdout, out.stderr);
    let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    Some(crate::redact::redact(line, &[]).chars().take(120).collect())
}

/// Claude Code's own `auth status` (JSON), as one line. Only whether and how it is signed in, never an account.
async fn claude_login(launch: &Launch) -> Option<String> {
    let args = launch.prefix.iter().cloned().chain(["auth".to_string(), "status".to_string()]).collect();
    let out =
        process::run(&command(launch, args, &std::env::temp_dir(), Duration::from_secs(20)), std::future::pending())
            .await;
    let status: Value = serde_json::from_str(out.stdout.trim()).ok()?;
    Some(if status["loggedIn"] == true {
        format!("Signed in ({})", status["authMethod"].as_str().unwrap_or("unknown method"))
    } else {
        "Not signed in: run claude auth login".to_string()
    })
}

/// Every known agent, installed or not, with its version and sign-in state where the CLI reports one.
pub async fn detect() -> Vec<Detected> {
    // Every agent, and each agent's two questions, at once: each answer is a program starting up, so asking in turn
    // made the Agents screen wait for all of them one after another.
    futures_util::future::join_all(AGENTS.iter().map(|spec| async move {
        let launch = spec.programs.iter().find_map(|p| locate(p));
        let (version, login) = match &launch {
            Some(launch) => {
                futures_util::future::join(quick(launch, &["--version"]), async {
                    match spec.id {
                        "codex" => quick(launch, &["login", "status"]).await,
                        "claude-code" => claude_login(launch).await,
                        _ => None,
                    }
                })
                .await
            }
            None => (None, None),
        };
        Detected {
            id: spec.id.into(),
            name: spec.name.into(),
            installed: launch.is_some(),
            program: launch.map(|l| l.prefix.last().cloned().unwrap_or(l.program)),
            version,
            login,
            sign_in: spec.sign_in.into(),
            install: spec.install.into(),
        }
    }))
    .await
}

/// A task for an agent.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRequest {
    pub agent: String,
    /// A Git repository; the agent works in a worktree of its current commit.
    pub workspace: PathBuf,
    pub prompt: String,
    /// Passed to the agent's own `--model`, in the agent's own naming. Empty: the agent's default.
    pub model: Option<String>,
    /// Lets the agent run shell commands (Claude Code's Bash tool, Gemini's yolo mode). Codex always runs in
    /// its workspace-write sandbox.
    #[serde(default)]
    pub allow_shell: bool,
    /// Lets the agent fetch from the web. Off: Claude Code gets no web tools, Codex's sandbox keeps the network off,
    /// OpenCode's webfetch is denied; Gemini CLI cannot be held to it (`sandbox_profile`).
    #[serde(default)]
    pub allow_web: bool,
    pub timeout_secs: Option<u64>,
    /// The commit the worktree starts from. Empty: the workspace's current commit. A task fixes it when it is
    /// created, so every attempt starts from the same code.
    #[serde(default)]
    pub base: Option<String>,
    /// A spending cap the agent enforces itself, where it can (Claude Code's `--max-budget-usd`).
    #[serde(default)]
    pub max_budget_usd: Option<f64>,
    /// The task attempt this run is, given to the agent as `PYGG_ATTEMPT_ID`. An agent the user pointed at the
    /// gateway can send it as its session (`x-pygg-session`), so the gateway's ledger says which
    /// attempt each request belonged to; the benchmark costs attempts that way.
    #[serde(default)]
    pub attempt_id: Option<String>,
}

/// What the agent reported about the run.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentUsage {
    pub model: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    /// The agent's own estimate at list price (Claude Code's `total_cost_usd`, OpenCode's step costs).
    pub estimate_usd: Option<f64>,
    /// The agent's final message.
    pub result: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRun {
    pub id: String,
    pub agent: String,
    /// The model asked for, in the agent's naming (`None`: its default).
    pub model: Option<String>,
    pub workspace: String,
    pub worktree: String,
    pub base: String,
    /// `ok`, `failed`, `timed_out`, `canceled` or `not_started`.
    pub outcome: String,
    pub exit: String,
    pub duration_ms: u64,
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
    pub usage: AgentUsage,
    /// The end of the agent's output, for when something went wrong.
    pub log_tail: String,
}

/// The command line for one agent run, without the program itself.
pub fn arguments(
    agent: &str, prompt: &str, model: Option<&str>, allow_shell: bool, allow_web: bool,
) -> Result<Vec<String>, String> {
    let mut args: Vec<String> = Vec::new();
    let s = |v: &str| v.to_string();
    match agent {
        "claude-code" => {
            // Write and Edit are not pre-allowed: a bare `Write` allows every path, and a headless run wrote outside its
            // folder with it (the sandbox proof, 2026-11-23). acceptEdits accepts edits inside the working folder; one
            // outside would need a prompt, which a headless run refuses.
            let mut tools = String::from("Read,Glob,Grep");
            if allow_shell {
                tools.push_str(",Bash");
            }
            if allow_web {
                tools.push_str(",WebFetch,WebSearch");
            }
            // An allow-list only spares the prompt; what is not allowed is denied outright.
            let mut denied: Vec<&str> = Vec::new();
            if !allow_shell {
                denied.push("Bash");
            }
            if !allow_web {
                denied.extend(["WebFetch", "WebSearch"]);
            }
            args.extend([
                s("-p"),
                s(prompt),
                s("--output-format"),
                s("json"),
                s("--permission-mode"),
                s("acceptEdits"),
                s("--allowedTools"),
                tools,
            ]);
            if !denied.is_empty() {
                args.extend([s("--disallowedTools"), denied.join(",")]);
            }
        }
        "codex" => {
            // Pygg's own MCP server is off in the runs Pygg starts: with it, an agent handed its task back to Pygg.
            args.extend([
                s("-c"),
                s(NO_PYGG_MCP_CODEX),
                s("exec"),
                s("--json"),
                s("--sandbox"),
                s("workspace-write"),
                s("--ephemeral"),
            ]);
            if allow_web {
                args.extend([s("-c"), s("sandbox_workspace_write.network_access=true")]);
            }
            args.push(s(prompt));
        }
        "gemini-cli" => args.extend([
            s("-p"),
            s(prompt),
            s("--output-format"),
            s("json"),
            s("--approval-mode"),
            s(if allow_shell { "yolo" } else { "auto_edit" }),
        ]),
        "opencode" => args.extend([s("run"), s("--format"), s("json"), s(prompt)]),
        other => return Err(format!("unknown agent {other}")),
    }
    if let Some(model) = model.map(str::trim).filter(|m| !m.is_empty()) {
        // Codex takes options before the prompt; the others take them anywhere.
        let at = if agent == "codex" { args.len() - 1 } else { args.len() };
        args.splice(at..at, [s("--model"), s(model)]);
    }
    Ok(args)
}

/// The cap an agent enforces itself: only Claude Code has one (`--max-budget-usd`). Requests any agent sends through
/// Pygg are held to the cap by the gateway (the attempt's session limit); others are checked after the run against the
/// cost the agent reports (tasks.rs).
pub fn budget_arguments(agent: &str, max_usd: Option<f64>) -> Vec<String> {
    match (agent, max_usd) {
        ("claude-code", Some(usd)) if usd > 0.0 => vec!["--max-budget-usd".into(), format!("{usd:.2}")],
        _ => Vec::new(),
    }
}

/// Whether the agent itself stops at a spending cap.
pub fn enforces_budget(agent: &str) -> bool {
    agent == "claude-code"
}

/// Reads what an agent printed: one JSON document (Claude Code, Gemini CLI) or JSON lines (Codex, OpenCode).
pub fn read_usage(agent: &str, stdout: &str) -> AgentUsage {
    let mut u = AgentUsage::default();
    let n = |v: &Value| v.as_u64();
    match agent {
        "claude-code" => {
            let Some(doc) = last_json_document(stdout) else { return u };
            let usage = &doc["usage"];
            u.input_tokens = n(&usage["input_tokens"]);
            u.output_tokens = n(&usage["output_tokens"]);
            u.cache_read_tokens = n(&usage["cache_read_input_tokens"]);
            u.cache_write_tokens = n(&usage["cache_creation_input_tokens"]);
            u.estimate_usd = doc["total_cost_usd"].as_f64();
            u.model = doc["modelUsage"].as_object().and_then(|m| m.keys().next().cloned());
            u.result = doc["result"].as_str().map(String::from);
            if doc["is_error"] == true {
                u.error = u.result.clone().or(Some(crate::messages::AGENT_ERROR.into()));
            }
        }
        "gemini-cli" => {
            let Some(doc) = last_json_document(stdout) else { return u };
            u.result = doc["response"].as_str().map(String::from);
            if let Some(models) = doc["stats"]["models"].as_object() {
                u.model = models.keys().next().cloned();
                let sum = |key: &str| models.values().filter_map(|m| m["tokens"][key].as_u64()).reduce(|a, b| a + b);
                u.input_tokens = sum("prompt");
                u.output_tokens = sum("candidates");
                u.cache_read_tokens = sum("cached");
            }
            u.error = doc["error"]["message"].as_str().map(String::from);
        }
        "codex" => {
            for event in stdout.lines().filter_map(|l| serde_json::from_str::<Value>(l.trim()).ok()) {
                match event["type"].as_str() {
                    Some("item.completed") if event["item"]["type"] == "agent_message" => {
                        u.result = event["item"]["text"].as_str().map(String::from);
                    }
                    Some("turn.completed") => {
                        let usage = &event["usage"];
                        let add = |acc: Option<u64>, v: Option<u64>| match (acc, v) {
                            (Some(a), Some(b)) => Some(a + b),
                            (a, b) => a.or(b),
                        };
                        u.input_tokens = add(u.input_tokens, n(&usage["input_tokens"]));
                        u.output_tokens = add(u.output_tokens, n(&usage["output_tokens"]));
                        u.cache_read_tokens = add(u.cache_read_tokens, n(&usage["cached_input_tokens"]));
                    }
                    Some("turn.failed") | Some("error") => {
                        u.error =
                            event["error"]["message"].as_str().or_else(|| event["message"].as_str()).map(String::from);
                    }
                    _ => {}
                }
            }
        }
        "opencode" => {
            let report = crate::bench::read_agent_events(stdout);
            u.input_tokens = Some(report.tokens.input);
            u.output_tokens = Some(report.tokens.output);
            u.cache_read_tokens = Some(report.tokens.cache_read);
            u.cache_write_tokens = Some(report.tokens.cache_write);
            u.estimate_usd = report.cost_micro_usd.map(|m| m as f64 / 1e6);
            u.error = report.error;
            u.result = opencode_final_text(stdout);
        }
        _ => {}
    }
    u
}

#[cfg(test)]
mod sandbox_tests {
    use super::*;

    #[test]
    fn secrets_an_agent_does_not_need_are_withheld() {
        let vars = [
            ("AWS_SECRET_ACCESS_KEY", "x"),
            ("GITHUB_TOKEN", "x"),
            ("ANTHROPIC_API_KEY", "x"),
            ("OPENAI_API_KEY", "x"),
            ("PATH", "C:/bin"),
            ("PYGG_ATTEMPT_ID", "a1"),
            ("DB_PASSWORD", "x"),
            ("DATABASE_URL", "postgres://app:s3cret@db:5432/app"),
            ("HTTPS_PROXY", "http://me:pw@proxy:8080"),
            ("HOMEPAGE", "https://example.com/a@b"),
        ]
        .map(|(k, v)| (k.to_string(), v.to_string()));
        let claude = withheld_env("claude-code", vars.clone().into_iter());
        assert_eq!(claude, ["AWS_SECRET_ACCESS_KEY", "GITHUB_TOKEN", "OPENAI_API_KEY", "DB_PASSWORD", "DATABASE_URL"]);
        let opencode = withheld_env("opencode", vars.into_iter());
        assert_eq!(
            opencode,
            ["AWS_SECRET_ACCESS_KEY", "GITHUB_TOKEN", "DB_PASSWORD", "DATABASE_URL"],
            "OpenCode signs in to many providers"
        );
        assert!(
            has_url_credentials("https://u:p@host/x")
                && !has_url_credentials("https://host/a@b")
                && !has_url_credentials("plain")
        );
    }

    #[test]
    fn opencode_is_held_to_the_runs_permissions() {
        let c: serde_json::Value = serde_json::from_str(&opencode_inline_config(None, false, false)).unwrap();
        assert_eq!(
            (
                c["permission"]["bash"].as_str(),
                c["permission"]["webfetch"].as_str(),
                c["permission"]["external_directory"].as_str()
            ),
            (Some("deny"), Some("deny"), Some("deny"))
        );
        let c: serde_json::Value = serde_json::from_str(&opencode_inline_config(None, true, true)).unwrap();
        assert_eq!(
            (c["permission"]["bash"].as_str(), c["permission"]["webfetch"].as_str()),
            (Some("allow"), Some("allow"))
        );
    }

    #[test]
    fn web_is_off_unless_allowed() {
        let claude = arguments("claude-code", "x", None, false, false).unwrap();
        let allowed =
            |args: &[String]| args.iter().position(|a| a == "--allowedTools").map(|i| args[i + 1].clone()).unwrap();
        assert!(!allowed(&claude).contains("WebFetch"));
        assert!(allowed(&arguments("claude-code", "x", None, false, true).unwrap()).contains("WebFetch"));
        let codex = arguments("codex", "x", None, false, true).unwrap();
        assert!(codex.contains(&"sandbox_workspace_write.network_access=true".to_string()));
        assert_eq!(codex.last().map(String::as_str), Some("x"), "the prompt stays last");
        let claude = arguments("claude-code", "x", None, false, false).unwrap();
        let denied = claude.iter().position(|a| a == "--disallowedTools").map(|i| claude[i + 1].clone());
        assert_eq!(denied.as_deref(), Some("Bash,WebFetch,WebSearch"));
        assert_eq!(sandbox_profile_on("gemini-cli", false, false, false).network, "not_guaranteed");
        let open = sandbox_profile_on("opencode", false, false, false);
        assert_eq!(
            (open.shell.as_str(), open.writes_outside.as_str(), open.network.as_str()),
            ("blocked", "blocked", "blocked")
        );
        let shell = sandbox_profile_on("opencode", true, false, false);
        assert_eq!(
            (shell.writes_outside.as_str(), shell.network.as_str()),
            ("not_guaranteed", "not_guaranteed"),
            "a shell reaches past tool rules"
        );
        assert_eq!(
            sandbox_profile_on("codex", false, false, true).writes_outside,
            "not_guaranteed",
            "not relied on on Windows"
        );
        assert_eq!(sandbox_profile_on("codex", false, false, false).writes_outside, "blocked");
        assert_eq!(sandbox_profile_on("claude-code", false, false, false).reads_outside, "not_guaranteed");
        let c: serde_json::Value = serde_json::from_str(&opencode_inline_config(None, false, false)).unwrap();
        assert_eq!(c["permission"]["websearch"].as_str(), Some("deny"));
    }
}

#[cfg(test)]
mod final_text_tests {
    #[test]
    fn opencodes_final_message_is_the_text_after_its_last_tool_call() {
        let out = [
            r#"{"type":"text","part":{"text":"Let me look."}}"#,
            r#"{"type":"tool_use","part":{"tool":"read","state":{"input":{"filePath":"a"}}}}"#,
            r#"{"type":"text","part":{"text":"It starts in main.rs."}}"#,
            r#"{"type":"step_finish","part":{}}"#,
        ]
        .join("\n");
        assert_eq!(super::opencode_final_text(&out).as_deref(), Some("It starts in main.rs."));
        assert_eq!(super::opencode_final_text(r#"{"type":"tool_use","part":{}}"#), None);
    }
}

/// OpenCode's final message: the text parts it wrote after its last tool call (`--format json` events).
fn opencode_final_text(stdout: &str) -> Option<String> {
    let mut tail: Vec<String> = Vec::new();
    for event in stdout.lines().filter_map(|l| serde_json::from_str::<Value>(l.trim()).ok()) {
        match event["type"].as_str() {
            Some("tool_use") => tail.clear(),
            Some("text") => {
                if let Some(text) = event["part"]["text"].as_str() {
                    tail.push(text.to_string());
                }
            }
            _ => {}
        }
    }
    let text = tail.join("\n").trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// The last complete JSON object in `text` (agents may print log lines before it).
fn last_json_document(text: &str) -> Option<Value> {
    if let Ok(v) = serde_json::from_str::<Value>(text.trim()) {
        return Some(v);
    }
    let start = text.rfind("\n{").map(|i| i + 1).or_else(|| text.find('{'))?;
    serde_json::from_str(text[start..].trim()).ok()
}

/// The environment variable that names the task attempt an agent run belongs to.
pub const ATTEMPT_ENV: &str = "PYGG_ATTEMPT_ID";

/// `ANTHROPIC_CUSTOM_HEADERS` for a Claude Code run that goes through a gateway (`ANTHROPIC_BASE_URL` set): the
/// headers already set, then `x-pygg-session: <attempt>`. `None` for another agent, or for Claude Code
/// talking to Anthropic directly, which would not use the header.
pub fn claude_session_headers(agent: &str, attempt: &str, var: impl Fn(&str) -> Option<String>) -> Option<String> {
    if agent != "claude-code" || var("ANTHROPIC_BASE_URL").filter(|v| !v.trim().is_empty()).is_none() {
        return None;
    }
    let header = format!("{}: {attempt}", crate::gateway::SESSION_HEADER);
    let kept: Vec<String> = var("ANTHROPIC_CUSTOM_HEADERS")
        .unwrap_or_default()
        .lines()
        .filter(|l| {
            let l = l.trim().to_ascii_lowercase();
            !l.is_empty()
                && !l.starts_with(crate::gateway::SESSION_HEADER)
                && !l.starts_with(crate::gateway::LEGACY_SESSION_HEADER)
        })
        .map(String::from)
        .collect();
    Some(kept.into_iter().chain(std::iter::once(header)).collect::<Vec<_>>().join("\n"))
}

/// The MCP server `pygg setup` adds to the agents, turned off for the runs Pygg itself starts: a run that could call
/// `pygg_run_task` handed its own task back to Pygg instead of doing it (seen 2026-11-21 with OpenCode).
pub const NO_PYGG_MCP_CODEX: &str = "mcp_servers.pygg.enabled=false";

/// OpenCode's inline configuration for a run Pygg starts: `extra` (an inline configuration the user already set, if
/// any) with the `pygg` MCP server turned off and the run's permissions: shell and web as chosen, nothing outside its
/// copy. Without them OpenCode's `run` allows every tool, so Shell off was not kept (the gap study).
pub fn opencode_inline_config(extra: Option<&str>, allow_shell: bool, allow_web: bool) -> String {
    let mut config = extra
        .and_then(|e| serde_json::from_str::<serde_json::Value>(e).ok())
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}));
    config["mcp"]["pygg"]["enabled"] = serde_json::Value::Bool(false);
    let rule = |allowed: bool| serde_json::Value::String(if allowed { "allow" } else { "deny" }.into());
    config["permission"]["bash"] = rule(allow_shell);
    config["permission"]["webfetch"] = rule(allow_web);
    config["permission"]["websearch"] = rule(allow_web);
    config["permission"]["external_directory"] = rule(false);
    config.to_string()
}

/// What an agent's own sign-in may read from the environment: its provider's variables (and, for OpenCode, which
/// signs in to many providers, every model provider's). Anything else that looks like a secret is withheld.
fn own_prefixes(agent: &str) -> &'static [&'static str] {
    match agent {
        "claude-code" => &["ANTHROPIC_", "CLAUDE_"],
        "codex" => &["OPENAI_", "CODEX_", "AZURE_OPENAI_"],
        "gemini-cli" => &["GEMINI_", "GOOGLE_"],
        "opencode" => &[
            "OPENCODE_",
            "ANTHROPIC_",
            "OPENAI_",
            "OPENROUTER_",
            "GEMINI_",
            "GOOGLE_",
            "GROQ_",
            "DEEPSEEK_",
            "MISTRAL_",
            "XAI_",
            "TOGETHER_",
            "FIREWORKS_",
            "CEREBRAS_",
            "MOONSHOT_",
            "DASHSCOPE_",
            "ZHIPU_",
        ],
        _ => &[],
    }
}

/// The variables of Pygg's environment an agent does not get (best effort, by name and value): those named like a secret
/// (a key, a token, a password, a credential) that its own sign-in does not use, such as cloud, GitHub or database
/// keys, and any whose value is a URL with a user and password in it (a `DATABASE_URL`). Proxy variables stay: the
/// agent needs them to reach its own provider. Secrets in files and credential stores are not covered.
pub fn withheld_env(agent: &str, vars: impl Iterator<Item = (String, String)>) -> Vec<String> {
    const SECRET: &[&str] = &["KEY", "TOKEN", "SECRET", "PASSWORD", "PASSWD", "CREDENTIAL", "PRIVATE", "AUTH"];
    let own = own_prefixes(agent);
    vars.filter(|(name, value)| {
        let upper = name.to_ascii_uppercase();
        if upper.ends_with("_PROXY") || upper == "PYGG_TOKEN" || own.iter().any(|p| upper.starts_with(p)) {
            return false;
        }
        SECRET.iter().any(|s| upper.contains(s)) || has_url_credentials(value)
    })
    .map(|(name, _)| name)
    .collect()
}

/// `scheme://user:password@host`: a URL that carries its own credentials.
pub fn has_url_credentials(value: &str) -> bool {
    value
        .split("://")
        .nth(1)
        .and_then(|rest| rest.split(['/', '?', '#']).next())
        .is_some_and(|host| host.split_once('@').is_some_and(|(user, _)| user.contains(':')))
}

/// How far a run is held to its profile, per concern: `blocked` (the agent is refused it), `allowed` (the user allowed
/// it), `not_guaranteed` (Pygg cannot hold this agent to it here), `partial` (best effort, said in the note).
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sandbox {
    pub agent: String,
    pub writes_outside: String,
    pub reads_outside: String,
    pub shell: String,
    pub network: String,
    pub secrets: String,
    /// What the agent itself does, and the limits of each claim.
    pub note: String,
}

/// What a run of `agent` with these choices is held to, as Pygg starts it on this platform. Conservative: a shell, once
/// allowed, can reach files and the network the agent's file and web tools are refused; Codex's sandbox is not relied
/// on on Windows; Gemini CLI has no sandbox here.
pub fn sandbox_profile(agent: &str, allow_shell: bool, allow_web: bool) -> Sandbox {
    sandbox_profile_on(agent, allow_shell, allow_web, cfg!(windows))
}

fn sandbox_profile_on(agent: &str, allow_shell: bool, allow_web: bool, windows: bool) -> Sandbox {
    let state = |s: &str| s.to_string();
    let (blocked, open, unsure) = ("blocked", "allowed", "not_guaranteed");
    // A shell the user allowed reaches past every tool rule.
    let through_shell = |held: &'static str| -> &'static str {
        if allow_shell && held == blocked {
            unsure
        } else {
            held
        }
    };
    let (writes, reads, shell, network, note) = match agent {
        "claude-code" => (
            through_shell(blocked),
            unsure,
            if allow_shell { open } else { blocked },
            if allow_web { open } else { through_shell(blocked) },
            "Headless Claude Code is refused edits outside its folder and every tool Pygg denies (--disallowedTools); its Read tool may still read outside.",
        ),
        "codex" => {
            let sandbox = if windows { unsure } else { blocked };
            (
                sandbox,
                unsure,
                // Codex always has a shell, inside its sandbox.
                if windows { unsure } else { "sandboxed" },
                if allow_web { open } else { sandbox },
                if windows {
                    "Codex's workspace-write sandbox is not relied on on Windows: its writes and network are not guaranteed to stay in."
                } else {
                    "Codex's workspace-write sandbox keeps writes in the copy and the network off unless allowed; it can read outside."
                },
            )
        }
        "opencode" => (
            through_shell(blocked),
            through_shell(blocked),
            if allow_shell { open } else { blocked },
            if allow_web { open } else { through_shell(blocked) },
            "Pygg sets OpenCode's permissions for the run: bash, webfetch and websearch as allowed, nothing outside the copy for its file tools.",
        ),
        "gemini-cli" => (unsure, unsure, if allow_shell { open } else { unsure }, if allow_web { open } else { unsure },
            "Gemini CLI has no sandbox here without Docker, and its approval mode is not a denial: Pygg cannot hold it to these."),
        _ => (unsure, unsure, unsure, unsure, ""),
    };
    Sandbox {
        agent: agent.into(),
        writes_outside: state(writes),
        reads_outside: state(reads),
        shell: state(shell),
        network: state(network),
        secrets: state("partial"),
        note: note.into(),
    }
}

/// Runs a task: a worktree of the workspace's current commit under `worktrees`, the agent started in it, and
/// the result measured. The worktree is kept for the user to keep as a branch or discard.
pub async fn run(
    request: &TaskRequest, worktrees: &Path, cancel: impl std::future::Future<Output = ()>,
) -> Result<TaskRun, String> {
    use crate::messages::{self as m, fill};
    let spec = find(&request.agent).ok_or_else(|| fill(m::UNKNOWN_AGENT, &[&request.agent]))?;
    if request.prompt.trim().is_empty() {
        return Err(m::DESCRIBE.into());
    }
    if !isolation::is_repository(&request.workspace) {
        return Err(m::NOT_A_REPOSITORY_WORKSPACE.into());
    }
    let launch = spec
        .programs
        .iter()
        .find_map(|p| locate(p))
        .ok_or_else(|| fill(m::NOT_INSTALLED, &[&spec.name, &spec.install]))?;
    let id = uuid::Uuid::new_v4().simple().to_string();
    let base = match request.base.as_deref().map(str::trim).filter(|b| !b.is_empty()) {
        Some(base) => base.to_string(),
        None => isolation::head(&request.workspace).map_err(|e| e.to_string())?,
    };
    std::fs::create_dir_all(worktrees).map_err(|e| e.to_string())?;
    let dir = worktrees.join(&id[..12]);
    let worktree = isolation::create(&request.workspace, &base, &dir).map_err(|e| e.to_string())?;

    let mut args = launch.prefix.clone();
    args.extend(arguments(spec.id, &request.prompt, request.model.as_deref(), request.allow_shell, request.allow_web)?);
    args.extend(budget_arguments(spec.id, request.max_budget_usd));
    let mut command = command(&launch, args, &worktree.path, Duration::from_secs(request.timeout_secs.unwrap_or(900)));
    if spec.id == "opencode" {
        let extra = std::env::var("OPENCODE_CONFIG_CONTENT").ok();
        command.env.push((
            "OPENCODE_CONFIG_CONTENT".into(),
            opencode_inline_config(extra.as_deref(), request.allow_shell, request.allow_web),
        ));
    }
    // Secrets the agent does not need for its own sign-in stay out of its environment (the sandbox profile).
    command.remove_env = withheld_env(spec.id, std::env::vars());
    if spec.id == "gemini-cli" {
        // The worktree is a copy Pygg made for this task; Gemini CLI refuses an untrusted folder in headless mode
        // (geminicli.com/docs/cli/trusted-folders).
        command.env.push(("GEMINI_CLI_TRUST_WORKSPACE".into(), "true".into()));
    }
    if let Some(attempt) = request.attempt_id.as_deref().filter(|a| !a.is_empty()) {
        command.env.push((ATTEMPT_ENV.into(), attempt.into()));
        // Claude Code cannot read a variable into a header, so where it was pointed at a gateway it is given the
        // attempt as its session header directly, after any headers already set for it.
        if let Some(headers) = claude_session_headers(spec.id, attempt, |name| std::env::var(name).ok()) {
            command.env.push(("ANTHROPIC_CUSTOM_HEADERS".into(), headers));
        }
    }
    // What the agent does, line by line, for the task's screen while it runs (live.rs).
    let attempt = request.attempt_id.clone().filter(|a| !a.is_empty());
    let sink: Option<process::LineSink> = attempt.clone().map(|attempt| {
        let agent = spec.id;
        crate::live::push(&attempt, &format!("{} started", spec.name));
        std::sync::Arc::new(move |raw: &str| {
            if let Some(text) = crate::live::readable(agent, raw) {
                crate::live::push(&attempt, &text);
            }
        }) as process::LineSink
    });
    let outcome = process::run_with(&command, cancel, sink).await;
    if let Some(attempt) = &attempt {
        crate::live::finish(attempt);
    }

    let usage = read_usage(spec.id, &outcome.stdout);
    let stat = isolation::changes(&worktree).unwrap_or_default();
    let (status, exit) = match &outcome.exit {
        Exit::Code(0) if usage.error.is_none() => ("ok", fill(m::EXIT, &[&0])),
        Exit::Code(code) => ("failed", fill(m::EXIT, &[code])),
        Exit::Abnormal => ("failed", m::ABNORMAL.to_string()),
        Exit::TimedOut => ("timed_out", m::TIMED_OUT.to_string()),
        Exit::Canceled => ("canceled", m::CANCELED.to_string()),
        Exit::FailedToStart(e) => ("not_started", fill(m::COULD_NOT_START, &[e])),
    };
    let tail = |text: &str| text.chars().rev().take(1500).collect::<Vec<_>>().into_iter().rev().collect::<String>();
    Ok(TaskRun {
        id,
        agent: spec.id.into(),
        model: request.model.clone().filter(|m| !m.trim().is_empty()),
        workspace: request.workspace.to_string_lossy().into_owned(),
        worktree: worktree.path.to_string_lossy().into_owned(),
        base,
        outcome: status.into(),
        exit,
        duration_ms: outcome.duration.as_millis() as u64,
        files_changed: stat.files,
        insertions: stat.insertions,
        deletions: stat.deletions,
        usage,
        log_tail: crate::redact::redact(&tail(&format!("{}\n{}", outcome.stdout, outcome.stderr)), &[]),
    })
}

/// The ledger entry for a run: tokens as the agent reported them, and the cost as unknown, because the money
/// depends on how the agent is signed in (a subscription is not billed per request). The agent's own estimate,
/// when it gave one, is kept in the source.
pub fn record(conn: &Connection, run: &TaskRun) -> store::Result<()> {
    record_for(conn, run, None)
}

/// `record`, with the entry tied to a task attempt (`usage_events.attempt_id`), so a task's cost is the sum of
/// its own entries.
pub fn record_for(conn: &Connection, run: &TaskRun, attempt_id: Option<&str>) -> store::Result<()> {
    let source = match run.usage.estimate_usd {
        Some(usd) => format!("agent run with its own sign-in; its list-price estimate: ${usd:.4}"),
        None => "agent run with its own sign-in; it reported no cost".to_string(),
    };
    let cost = Cost::Unknown { source };
    let model = run.usage.model.clone().or_else(|| run.model.clone()).unwrap_or_else(|| "default".into());
    let error = run.usage.error.clone().or_else(|| (run.outcome != "ok").then(|| run.exit.clone()));
    let record = UsageRecord {
        client_id: None,
        provider_id: &format!("agent:{}", run.agent),
        model_id: &model,
        requested_model: "task",
        input_tokens: run.usage.input_tokens,
        output_tokens: run.usage.output_tokens,
        cache_tokens: run.usage.cache_read_tokens,
        cost: &cost,
        outcome: if run.outcome == "ok" { "ok" } else { "error" },
        http_status: None,
        latency_ms: run.duration_ms,
        error: error.as_deref(),
        cache_write_tokens: run.usage.cache_write_tokens,
        session_key: None,
        route_mode: Some("agent"),
        route_reason: None,
        facts: None,
        repeated_tokens: None,
    };
    let Some(attempt_id) = attempt_id else { return store::record_usage(conn, &record) };
    // The entry and its attempt link land together or not at all.
    conn.execute_batch("SAVEPOINT agent_usage").map_err(|e| e.to_string())?;
    let linked = store::record_usage(conn, &record).and_then(|()| {
        conn.execute("UPDATE usage_events SET attempt_id = ?1 WHERE rowid = last_insert_rowid()", [attempt_id])
            .map(|_| ())
            .map_err(|e| e.to_string())
    });
    let end = if linked.is_ok() { "RELEASE agent_usage" } else { "ROLLBACK TO agent_usage; RELEASE agent_usage" };
    conn.execute_batch(end).map_err(|e| e.to_string())?;
    linked
}

/// Keeps a run's result as a branch of the workspace, or discards it; either way the worktree is removed.
pub fn finish(run: &TaskRun, keep: Option<&str>) -> Result<Option<String>, String> {
    let worktree =
        Worktree { repo: PathBuf::from(&run.workspace), path: PathBuf::from(&run.worktree), base: run.base.clone() };
    let kept = match keep {
        Some(title) => {
            let branch = isolation::branch_name(title, &run.id);
            isolation::keep_as_branch(&worktree, &branch, title).map_err(|e| e.to_string())?;
            Some(branch)
        }
        None => None,
    };
    isolation::discard(&worktree).map_err(|e| e.to_string())?;
    Ok(kept)
}

pub fn diff(run: &TaskRun) -> Result<String, String> {
    let worktree =
        Worktree { repo: PathBuf::from(&run.workspace), path: PathBuf::from(&run.worktree), base: run.base.clone() };
    isolation::diff(&worktree).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_each_agents_headless_command() {
        assert_eq!(
            arguments("claude-code", "fix it", None, false, false).unwrap(),
            [
                "-p",
                "fix it",
                "--output-format",
                "json",
                "--permission-mode",
                "acceptEdits",
                "--allowedTools",
                "Read,Glob,Grep",
                "--disallowedTools",
                "Bash,WebFetch,WebSearch"
            ]
        );
        assert_eq!(
            arguments("codex", "fix it", Some("gpt-x"), false, false).unwrap(),
            [
                "-c",
                NO_PYGG_MCP_CODEX,
                "exec",
                "--json",
                "--sandbox",
                "workspace-write",
                "--ephemeral",
                "--model",
                "gpt-x",
                "fix it"
            ]
        );
        assert_eq!(
            arguments("gemini-cli", "fix it", None, true, false).unwrap(),
            ["-p", "fix it", "--output-format", "json", "--approval-mode", "yolo"]
        );
        assert_eq!(
            arguments("opencode", "fix it", Some("p/m"), false, false).unwrap(),
            ["run", "--format", "json", "fix it", "--model", "p/m"]
        );
        assert!(arguments("cursor", "x", None, false, false).is_err());
        assert_eq!(budget_arguments("claude-code", Some(0.5)), ["--max-budget-usd", "0.50"]);
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| pairs.iter().find(|(k, _)| *k == name).map(|(_, v)| v.to_string())
        };
        assert_eq!(claude_session_headers("claude-code", "a1", env(&[])), None, "straight to Anthropic: no header");
        assert_eq!(
            claude_session_headers("codex", "a1", env(&[("ANTHROPIC_BASE_URL", "http://127.0.0.1:8741")])),
            None
        );
        assert_eq!(
            claude_session_headers(
                "claude-code",
                "a1",
                env(&[
                    ("ANTHROPIC_BASE_URL", "http://127.0.0.1:8741"),
                    ("ANTHROPIC_CUSTOM_HEADERS", "x-team: blue\nX-SmartTaskRouter-Session: older\nx-pygg-session: old")
                ])
            )
            .as_deref(),
            Some("x-team: blue\nx-pygg-session: a1")
        );
        assert!(budget_arguments("codex", Some(0.5)).is_empty() && budget_arguments("claude-code", None).is_empty());
    }

    #[test]
    fn reads_what_each_agent_reports() {
        let claude = r#"{"type":"result","is_error":false,"result":"Done.","total_cost_usd":0.0123,
            "usage":{"input_tokens":10,"output_tokens":20,"cache_read_input_tokens":300,"cache_creation_input_tokens":40},
            "modelUsage":{"claude-sonnet-x":{}}}"#;
        let u = read_usage("claude-code", claude);
        assert_eq!(
            (u.input_tokens, u.output_tokens, u.cache_read_tokens, u.cache_write_tokens),
            (Some(10), Some(20), Some(300), Some(40))
        );
        assert_eq!(
            (u.estimate_usd, u.model.as_deref(), u.result.as_deref(), u.error),
            (Some(0.0123), Some("claude-sonnet-x"), Some("Done."), None)
        );

        let codex = "{\"type\":\"thread.started\"}\n{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"Fixed.\"}}\n\
            {\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":100,\"cached_input_tokens\":60,\"output_tokens\":7}}";
        let u = read_usage("codex", codex);
        assert_eq!(
            (u.input_tokens, u.cache_read_tokens, u.output_tokens, u.result.as_deref()),
            (Some(100), Some(60), Some(7), Some("Fixed."))
        );

        let gemini = "Loaded cached credentials.\n{\"response\":\"Ok\",\"stats\":{\"models\":{\"gemini-x\":{\"tokens\":{\"prompt\":50,\"candidates\":5,\"cached\":0}}}}}";
        let u = read_usage("gemini-cli", gemini);
        assert_eq!(
            (u.input_tokens, u.output_tokens, u.model.as_deref(), u.result.as_deref()),
            (Some(50), Some(5), Some("gemini-x"), Some("Ok"))
        );
    }

    // An npm shim is a Windows `.cmd` file; elsewhere npm links the script itself.
    #[cfg(windows)]
    #[test]
    fn an_npm_shim_is_started_through_its_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("node_modules").join("tool").join("cli.js");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, "").unwrap();
        let shim = dir.path().join("tool.cmd");
        std::fs::write(
            &shim,
            "@ECHO off\r\nIF EXIST \"%dp0%\\node.exe\" (\r\n  SET \"_prog=%dp0%\\node.exe\"\r\n)\r\n\
            \"%_prog%\"  \"%dp0%\\node_modules\\tool\\cli.js\" %*\r\n",
        )
        .unwrap();
        let launch = read_npm_shim(&shim).unwrap();
        assert_eq!(launch.program, "node");
        assert_eq!(launch.prefix, [target.to_string_lossy().into_owned()]);
    }
}

#[cfg(test)]
mod pygg_mcp_tests {
    use super::*;

    #[test]
    fn runs_pygg_starts_have_its_own_mcp_server_off() {
        let plain: serde_json::Value = serde_json::from_str(&opencode_inline_config(None, false, false)).unwrap();
        assert_eq!(plain["mcp"], serde_json::json!({"pygg": {"enabled": false}}));
        let kept: serde_json::Value = serde_json::from_str(&opencode_inline_config(
            Some(r#"{"theme":"x","mcp":{"other":{"enabled":true}}}"#),
            false,
            false,
        ))
        .unwrap();
        assert_eq!(
            (
                kept["theme"].as_str(),
                kept["mcp"]["other"]["enabled"].as_bool(),
                kept["mcp"]["pygg"]["enabled"].as_bool()
            ),
            (Some("x"), Some(true), Some(false))
        );
        assert_eq!(opencode_inline_config(Some("not json"), false, false), opencode_inline_config(None, false, false));
        assert_eq!(
            arguments("codex", "p", None, false, false).unwrap()[..2],
            ["-c".to_string(), NO_PYGG_MCP_CODEX.to_string()]
        );
    }
}
