//! Task engine v1 (ROADMAP P1-1). A task is a ladder of agent steps fixed when it is created: each attempt runs
//! one step with the agent's own sign-in (agents.rs) in a fresh worktree of the task's base commit
//! (isolation.rs, ADR-008), and is judged by the workspace's approved checks (checks.rs, ADR-007). A failed
//! attempt is discarded and the next step starts from the same code, with a bounded summary of what failed.
//! A paid step waits for the user's approval. A passing result waits for the user to keep it as a branch or
//! discard it (ADR-009).
//!
//! States are the interface's (src/lib/tasks.ts): running, waiting_approval, checks_passed, needs_review
//! (finished, but nothing confirmed it), manually_accepted, check_failed, blocked, canceled, interrupted.

use crate::agents::{self, TaskRun};
use crate::checks::{self, CheckResult};
use crate::db::now_ms;
use crate::isolation::{self, Worktree};
use futures_util::future::BoxFuture;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::watch;

pub type Result<T> = std::result::Result<T, String>;

fn err(e: rusqlite::Error) -> String {
    e.to_string()
}

pub const STRATEGIES: [&str; 3] = ["free_first", "balanced", "quality"];
const BILLINGS: [&str; 5] = ["free", "local", "subscription", "paid", "unknown"];
pub const MAX_STEPS: usize = 6;
const DEFAULT_TIMEOUT_SECS: u64 = 900;
const CHECK_TIMEOUT: Duration = Duration::from_secs(600);
/// How much of a failed check's output the next step is given.
const HANDOFF_CHARS: usize = 1500;
const NO_CHANGES: &str = "The agent made no changes";
const NO_CHANGES_HANDOFF: &str = "The previous attempt ended without changing any file. The task needs changes to the files.";

fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

fn lock(db: &Mutex<Connection>) -> std::sync::MutexGuard<'_, Connection> {
    db.lock().unwrap_or_else(|p| p.into_inner())
}

fn audit(conn: &Connection, action: &str, entity: &str, metadata: serde_json::Value) -> rusqlite::Result<()> {
    conn.execute("INSERT INTO audit_events (id, actor, action, entity, metadata_json, created_at) VALUES (?1, 'user', ?2, ?3, ?4, ?5)",
        params![new_id(), action, entity, metadata.to_string(), now_ms()]).map(|_| ())
}

fn last_chars(text: &str, n: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    chars[chars.len().saturating_sub(n)..].iter().collect()
}

// ---------- Workspaces ----------

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCheck {
    pub command: String,
    /// The file it was found in (`package.json`, `Cargo.toml`), or `user`.
    pub source: String,
    /// Only approved checks run. A detected check is a suggestion until then.
    pub approved: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceView {
    pub id: String,
    pub name: String,
    pub path: String,
    pub checks: Vec<WorkspaceCheck>,
    pub tasks: u32,
}

/// Adds a Git repository as a workspace and suggests the checks it already has.
pub fn add_workspace(conn: &Connection, path: &str, name: Option<&str>) -> Result<WorkspaceView> {
    let path = path.trim().trim_matches('"').trim();
    if path.is_empty() {
        return Err("Choose the workspace folder".into());
    }
    let dir = Path::new(path);
    if !dir.is_dir() {
        return Err(format!("{path} is not a folder"));
    }
    if !isolation::is_repository(dir) {
        return Err("The workspace must be a Git repository: every attempt works on its own worktree, so your files and branch are never touched.".into());
    }
    let name = name.map(str::trim).filter(|n| !n.is_empty()).map(String::from)
        .or_else(|| dir.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| path.to_string());
    let id = new_id();
    conn.execute("INSERT INTO workspaces (id, name, path, created_at) VALUES (?1, ?2, ?3, ?4)", params![id, name, path, now_ms()])
        .map_err(|e| match e {
            rusqlite::Error::SqliteFailure(f, _) if f.code == rusqlite::ErrorCode::ConstraintViolation =>
                "That folder is already a workspace".to_string(),
            e => err(e),
        })?;
    suggest_checks(conn, &id, dir)?;
    workspace(conn, &id)
}

fn suggest_checks(conn: &Connection, id: &str, dir: &Path) -> Result<()> {
    for check in checks::detect(dir) {
        conn.execute("INSERT OR IGNORE INTO workspace_checks (id, workspace_id, command, source, approved) VALUES (?1, ?2, ?3, ?4, 0)",
            params![new_id(), id, check.command, check.source]).map_err(err)?;
    }
    Ok(())
}

pub fn workspace(conn: &Connection, id: &str) -> Result<WorkspaceView> {
    let (name, path): (String, String) = conn.query_row("SELECT name, path FROM workspaces WHERE id = ?1", [id],
        |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(err)?.ok_or("That workspace no longer exists")?;
    let mut statement = conn.prepare("SELECT command, source, approved FROM workspace_checks WHERE workspace_id = ?1 ORDER BY rowid")
        .map_err(err)?;
    let checks = statement.query_map([id], |r| Ok(WorkspaceCheck { command: r.get(0)?, source: r.get(1)?, approved: r.get(2)? }))
        .map_err(err)?.collect::<rusqlite::Result<Vec<_>>>().map_err(err)?;
    let tasks: u32 = conn.query_row("SELECT COUNT(*) FROM tasks WHERE workspace_id = ?1", [id], |r| r.get(0)).map_err(err)?;
    Ok(WorkspaceView { id: id.into(), name, path, checks, tasks })
}

pub fn list_workspaces(conn: &Connection) -> Result<Vec<WorkspaceView>> {
    let ids: Vec<String> = conn.prepare("SELECT id FROM workspaces ORDER BY name COLLATE NOCASE").map_err(err)?
        .query_map([], |r| r.get(0)).map_err(err)?.collect::<rusqlite::Result<_>>().map_err(err)?;
    ids.iter().map(|id| workspace(conn, id)).collect()
}

/// Looks for checks again (a script was added since); new ones arrive as suggestions.
pub fn detect_checks(conn: &Connection, id: &str) -> Result<WorkspaceView> {
    let found = workspace(conn, id)?;
    suggest_checks(conn, id, Path::new(&found.path))?;
    workspace(conn, id)
}

pub fn set_check(conn: &Connection, id: &str, command: &str, approved: bool) -> Result<()> {
    let changed = conn.execute("UPDATE workspace_checks SET approved = ?3 WHERE workspace_id = ?1 AND command = ?2",
        params![id, command, approved]).map_err(err)?;
    if changed == 0 {
        return Err("That check no longer exists".into());
    }
    audit(conn, if approved { "check.approved" } else { "check.unapproved" }, id, serde_json::json!({"command": command})).map_err(err)
}

/// A check the user typed. Typing it is approving it.
pub fn add_check(conn: &Connection, id: &str, command: &str) -> Result<()> {
    let command = command.trim();
    if command.is_empty() {
        return Err("Type the command, e.g. pnpm test".into());
    }
    if command.chars().count() > 500 || command.contains('\n') {
        return Err("A check is one command line of at most 500 characters".into());
    }
    workspace(conn, id)?;
    conn.execute("INSERT INTO workspace_checks (id, workspace_id, command, source, approved) VALUES (?1, ?2, ?3, 'user', 1) \
                  ON CONFLICT (workspace_id, command) DO UPDATE SET approved = 1", params![new_id(), id, command]).map_err(err)?;
    audit(conn, "check.approved", id, serde_json::json!({"command": command})).map_err(err)
}

pub fn remove_check(conn: &Connection, id: &str, command: &str) -> Result<()> {
    conn.execute("DELETE FROM workspace_checks WHERE workspace_id = ?1 AND command = ?2", params![id, command]).map_err(err)?;
    Ok(())
}

pub fn remove_workspace(conn: &Connection, id: &str) -> Result<()> {
    if workspace(conn, id)?.tasks > 0 {
        return Err("This workspace has task history, so it stays.".into());
    }
    conn.execute("DELETE FROM workspaces WHERE id = ?1", [id]).map_err(err)?;
    Ok(())
}

// ---------- Creating a task ----------

/// One step of a task's ladder: an agent, the model it is asked to use, and how the user classes it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub agent: String,
    /// In the agent's own naming; empty: the agent's default.
    #[serde(default)]
    pub model: Option<String>,
    /// T0–T4, the user's classing of this step (an agent's sign-in decides the real model).
    pub tier: u8,
    /// How this step is paid for, as the user knows it. `paid` waits for approval before it runs.
    pub billing: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PolicyCheck {
    pub command: String,
    pub source: String,
}

/// Fixed when the task is created (tasks.policy_snapshot_json): later changes to the workspace's checks do not
/// change what judged a task.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    pub steps: Vec<Step>,
    pub checks: Vec<PolicyCheck>,
    pub allow_shell: bool,
    pub timeout_secs: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTask {
    pub workspace_id: String,
    #[serde(default)]
    pub title: Option<String>,
    pub prompt: String,
    pub strategy: String,
    pub steps: Vec<Step>,
    #[serde(default)]
    pub allow_shell: bool,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

fn title_of(prompt: &str) -> String {
    let line = prompt.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("Task");
    let mut title: String = line.chars().take(80).collect();
    if line.chars().count() > 80 {
        title.push('…');
    }
    title
}

/// Records a task in state `running`; the runner then takes it (`Runner::start`).
pub fn create_task(conn: &Connection, new: &NewTask) -> Result<String> {
    create_task_for(conn, new, None)
}

/// `create_task` for a gateway client (the task API): the task remembers the client, which sees only its own.
pub fn create_task_for(conn: &Connection, new: &NewTask, client_id: Option<&str>) -> Result<String> {
    let prompt = new.prompt.trim();
    if prompt.is_empty() {
        return Err("Describe the task".into());
    }
    if !STRATEGIES.contains(&new.strategy.as_str()) {
        return Err(format!("Unknown strategy {}", new.strategy));
    }
    if new.steps.is_empty() {
        return Err("Add at least one step".into());
    }
    if new.steps.len() > MAX_STEPS {
        return Err(format!("A task has at most {MAX_STEPS} steps"));
    }
    for step in &new.steps {
        agents::find(&step.agent).ok_or_else(|| format!("Unknown agent {}", step.agent))?;
        if step.tier > 4 {
            return Err("A step's tier is T0 to T4".into());
        }
        if !BILLINGS.contains(&step.billing.as_str()) {
            return Err(format!("Unknown billing {}", step.billing));
        }
    }
    let ws = workspace(conn, &new.workspace_id)?;
    let base = isolation::head(Path::new(&ws.path)).map_err(|e| e.to_string())?;
    let steps = new.steps.iter().map(|s| Step {
        model: s.model.as_deref().map(str::trim).filter(|m| !m.is_empty()).map(String::from), ..s.clone()
    }).collect();
    let policy = Policy {
        steps,
        checks: ws.checks.iter().filter(|c| c.approved)
            .map(|c| PolicyCheck { command: c.command.clone(), source: c.source.clone() }).collect(),
        allow_shell: new.allow_shell,
        timeout_secs: new.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS).clamp(60, 3600),
    };
    let title = new.title.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(String::from).unwrap_or_else(|| title_of(prompt));
    let id = new_id();
    let now = now_ms();
    let tx = conn.unchecked_transaction().map_err(err)?;
    tx.execute("INSERT INTO tasks (id, workspace_id, title, prompt, strategy, state, base_revision, policy_snapshot_json, \
                created_at, updated_at, phase, client_id) VALUES (?1, ?2, ?3, ?4, ?5, 'running', ?6, ?7, ?8, ?8, 'preparing', ?9)",
        params![id, ws.id, title, prompt, new.strategy, base, serde_json::to_string(&policy).map_err(|e| e.to_string())?, now, client_id])
        .map_err(err)?;
    tx.execute("INSERT INTO task_stages (id, task_id, position, name, check_spec, state) VALUES (?1, ?2, 0, 'change', ?3, 'running')",
        params![new_id(), id, serde_json::to_string(&policy.checks).map_err(|e| e.to_string())?]).map_err(err)?;
    audit(&tx, "task.created", &id, serde_json::json!({"steps": policy.steps.len(), "checks": policy.checks.len()})).map_err(err)?;
    tx.commit().map_err(err)?;
    Ok(id)
}

// ---------- Reading tasks ----------

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptChip {
    pub tier: u8,
    pub billing: String,
    pub model: String,
    pub outcome: String,
}

/// Money kept apart by certainty: unknown is never folded into a total (SCREENS.md §3.3).
#[derive(Clone, Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskCost {
    pub actual_usd: f64,
    pub estimated_usd: f64,
    pub unknown_calls: u32,
    pub local_only: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummary {
    pub id: String,
    pub title: String,
    pub workspace_id: String,
    pub state: String,
    pub phase: Option<String>,
    pub attempts: Vec<AttemptChip>,
    pub cost: TaskCost,
    pub started_at: i64,
    pub updated_at: i64,
    /// The gateway client that started it (an IDE agent through the MCP server); `None`: the app.
    pub source: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptView {
    pub id: String,
    pub number: u32,
    /// The step of the ladder it ran (0-based).
    pub step: usize,
    pub agent: String,
    /// `Codex CLI · gpt-5` or `Codex CLI · default model`.
    pub label: String,
    pub agent_model: Option<String>,
    pub tier: u8,
    pub billing: String,
    /// running, waiting, passed, done (finished, no checks), failed, canceled.
    pub outcome: String,
    pub detail: Option<String>,
    pub escalation_reason: Option<String>,
    pub checks: Vec<CheckResult>,
    pub files_changed: Option<u32>,
    pub insertions: Option<u32>,
    pub deletions: Option<u32>,
    pub summary: Option<String>,
    pub log_tail: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    /// Its worktree is still there: the result waits for the user.
    pub open: bool,
    #[serde(skip)]
    worktree: Option<String>,
    #[serde(skip)]
    base: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDetail {
    #[serde(flatten)]
    pub summary: TaskSummary,
    pub prompt: String,
    pub strategy: String,
    pub reason: Option<String>,
    pub result: Option<String>,
    pub result_branch: Option<String>,
    pub base_revision: Option<String>,
    pub workspace_name: String,
    pub workspace_path: String,
    pub policy: Policy,
    /// Every attempt in full (the summary's `attempts` are the chips).
    pub attempt_details: Vec<AttemptView>,
}

struct TaskRow {
    title: String,
    workspace_id: String,
    workspace_name: String,
    workspace_path: String,
    prompt: String,
    strategy: String,
    state: String,
    phase: Option<String>,
    reason: Option<String>,
    result: Option<String>,
    result_branch: Option<String>,
    base_revision: Option<String>,
    policy: Policy,
    stage_id: String,
    created_at: i64,
    updated_at: i64,
    client_id: Option<String>,
    client_name: Option<String>,
}

fn task_row(conn: &Connection, id: &str) -> Result<TaskRow> {
    let row = conn.query_row(
        "SELECT t.title, t.workspace_id, w.name, w.path, t.prompt, t.strategy, t.state, t.phase, t.reason, t.result, \
                t.result_branch, t.base_revision, t.policy_snapshot_json, s.id, t.created_at, t.updated_at, t.client_id, c.name \
         FROM tasks t JOIN workspaces w ON w.id = t.workspace_id JOIN task_stages s ON s.task_id = t.id AND s.position = 0 \
         LEFT JOIN gateway_clients c ON c.id = t.client_id WHERE t.id = ?1", [id],
        |r| Ok((TaskRow {
            title: r.get(0)?, workspace_id: r.get(1)?, workspace_name: r.get(2)?, workspace_path: r.get(3)?, prompt: r.get(4)?,
            strategy: r.get(5)?, state: r.get(6)?, phase: r.get(7)?, reason: r.get(8)?, result: r.get(9)?,
            result_branch: r.get(10)?, base_revision: r.get(11)?,
            policy: Policy { steps: vec![], checks: vec![], allow_shell: false, timeout_secs: DEFAULT_TIMEOUT_SECS },
            stage_id: r.get(13)?, created_at: r.get(14)?, updated_at: r.get(15)?, client_id: r.get(16)?, client_name: r.get(17)?,
        }, r.get::<_, String>(12)?)),
    ).optional().map_err(err)?;
    let (mut row, policy) = row.ok_or("That task no longer exists")?;
    row.policy = serde_json::from_str(&policy).map_err(|e| format!("The task's policy could not be read: {e}"))?;
    Ok(row)
}

fn attempts_of(conn: &Connection, task_id: &str) -> Result<Vec<AttemptView>> {
    let mut statement = conn.prepare(
        "SELECT a.id, a.number, a.rung, a.agent, a.model_ref, a.agent_model, a.tier, a.billing, a.state, a.detail, \
                a.escalation_reason, a.check_results_json, a.files_changed, a.insertions, a.deletions, a.summary, a.log_tail, \
                (SELECT SUM(input_tokens) FROM usage_events u WHERE u.attempt_id = a.id), \
                (SELECT SUM(output_tokens) FROM usage_events u WHERE u.attempt_id = a.id), \
                a.started_at, a.finished_at, a.worktree_path, a.base_revision \
         FROM attempts a JOIN task_stages s ON s.id = a.stage_id WHERE s.task_id = ?1 ORDER BY a.number").map_err(err)?;
    let rows = statement.query_map([task_id], |r| {
        let checks: Option<String> = r.get(11)?;
        let worktree: Option<String> = r.get(21)?;
        Ok(AttemptView {
            id: r.get(0)?, number: r.get(1)?, step: r.get::<_, i64>(2)? as usize, agent: r.get(3)?, label: r.get(4)?,
            agent_model: r.get(5)?, tier: r.get(6)?, billing: r.get(7)?, outcome: r.get(8)?, detail: r.get(9)?,
            escalation_reason: r.get(10)?,
            checks: checks.and_then(|c| serde_json::from_str(&c).ok()).unwrap_or_default(),
            files_changed: r.get(12)?, insertions: r.get(13)?, deletions: r.get(14)?, summary: r.get(15)?, log_tail: r.get(16)?,
            input_tokens: r.get(17)?, output_tokens: r.get(18)?, started_at: r.get(19)?, finished_at: r.get(20)?,
            open: worktree.is_some(), worktree, base: r.get(22)?,
        })
    }).map_err(err)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(err)
}

fn cost_of(conn: &Connection, task_id: &str, attempts: &[AttemptView]) -> Result<TaskCost> {
    let mut cost = TaskCost {
        local_only: !attempts.is_empty() && attempts.iter().all(|a| a.billing == "local"),
        ..TaskCost::default()
    };
    let mut statement = conn.prepare(
        "SELECT u.cost_status, COALESCE(SUM(u.cost_micro_usd), 0), COUNT(*) FROM usage_events u \
         JOIN attempts a ON a.id = u.attempt_id JOIN task_stages s ON s.id = a.stage_id WHERE s.task_id = ?1 GROUP BY u.cost_status")
        .map_err(err)?;
    let rows = statement.query_map([task_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, u32>(2)?))).map_err(err)?;
    for row in rows {
        let (status, micro, count) = row.map_err(err)?;
        match status.as_str() {
            "actual" => cost.actual_usd += micro as f64 / 1e6,
            "estimated" => cost.estimated_usd += micro as f64 / 1e6,
            _ => cost.unknown_calls += count,
        }
    }
    Ok(cost)
}

fn summary_of(conn: &Connection, id: &str, row: &TaskRow, attempts: &[AttemptView]) -> Result<TaskSummary> {
    Ok(TaskSummary {
        id: id.into(), title: row.title.clone(), workspace_id: row.workspace_id.clone(), state: row.state.clone(),
        phase: row.phase.clone(),
        attempts: attempts.iter().map(|a| AttemptChip { tier: a.tier, billing: a.billing.clone(), model: a.label.clone(),
            outcome: a.outcome.clone() }).collect(),
        cost: cost_of(conn, id, attempts)?,
        started_at: row.created_at, updated_at: row.updated_at, source: row.client_name.clone(),
    })
}

pub fn list_tasks(conn: &Connection) -> Result<Vec<TaskSummary>> {
    let ids: Vec<String> = conn.prepare("SELECT id FROM tasks ORDER BY updated_at DESC LIMIT 500").map_err(err)?
        .query_map([], |r| r.get(0)).map_err(err)?.collect::<rusqlite::Result<_>>().map_err(err)?;
    ids.iter().map(|id| {
        let row = task_row(conn, id)?;
        let attempts = attempts_of(conn, id)?;
        summary_of(conn, id, &row, &attempts)
    }).collect()
}

pub fn task_detail(conn: &Connection, id: &str) -> Result<TaskDetail> {
    task_detail_for(conn, id, None)
}

/// A client sees only the tasks it started; `None`: the app, which sees every task.
pub fn task_detail_for(conn: &Connection, id: &str, client_id: Option<&str>) -> Result<TaskDetail> {
    let row = task_row(conn, id)?;
    if client_id.is_some() && row.client_id.as_deref() != client_id {
        return Err("That task no longer exists".into());
    }
    let attempts = attempts_of(conn, id)?;
    let summary = summary_of(conn, id, &row, &attempts)?;
    Ok(TaskDetail {
        summary, prompt: row.prompt, strategy: row.strategy, reason: row.reason, result: row.result,
        result_branch: row.result_branch, base_revision: row.base_revision, workspace_name: row.workspace_name,
        workspace_path: row.workspace_path, policy: row.policy, attempt_details: attempts,
    })
}

// ---------- State changes ----------

fn set_task(conn: &Connection, id: &str, state: &str, phase: Option<&str>, reason: Option<&str>) -> rusqlite::Result<()> {
    conn.execute("UPDATE tasks SET state = ?2, phase = ?3, reason = ?4, updated_at = ?5 WHERE id = ?1",
        params![id, state, phase, reason, now_ms()])?;
    conn.execute("UPDATE task_stages SET state = ?2 WHERE task_id = ?1", params![id, state])?;
    Ok(())
}

fn set_phase(conn: &Connection, id: &str, phase: &str) -> rusqlite::Result<()> {
    conn.execute("UPDATE tasks SET phase = ?2, updated_at = ?3 WHERE id = ?1 AND state = 'running'", params![id, phase, now_ms()])
        .map(|_| ())
}

fn step_label(step: &Step) -> String {
    let name = agents::find(&step.agent).map(|a| a.name).unwrap_or(step.agent.as_str());
    format!("{name} · {}", step.model.as_deref().unwrap_or("default model"))
}

enum Opened {
    Run(String),
    /// A paid step: the task now waits for the user's approval.
    Waiting,
    Declined,
}

fn open_attempt(conn: &Connection, task_id: &str, stage_id: &str, index: usize, step: &Step) -> rusqlite::Result<Opened> {
    let now = now_ms();
    if step.billing == "paid" {
        let waiting: Option<String> = conn.query_row(
            "SELECT id FROM attempts WHERE stage_id = ?1 AND rung = ?2 AND state = 'waiting' ORDER BY number DESC LIMIT 1",
            params![stage_id, index as i64], |r| r.get(0)).optional()?;
        if let Some(attempt) = waiting {
            let decision: Option<String> = conn.query_row(
                "SELECT state FROM approvals WHERE attempt_id = ?1 ORDER BY created_at DESC LIMIT 1", [&attempt], |r| r.get(0)).optional()?;
            return match decision.as_deref() {
                Some("approved") => {
                    conn.execute("UPDATE attempts SET state = 'running', started_at = ?2 WHERE id = ?1", params![attempt, now])?;
                    Ok(Opened::Run(attempt))
                }
                Some("pending") | None => {
                    set_task(conn, task_id, "waiting_approval", None, Some(&paid_reason(index, step)))?;
                    Ok(Opened::Waiting)
                }
                Some(_) => Ok(Opened::Declined),
            };
        }
    }
    let number: u32 = conn.query_row("SELECT COALESCE(MAX(number), 0) + 1 FROM attempts WHERE stage_id = ?1", [stage_id], |r| r.get(0))?;
    let id = new_id();
    let paid = step.billing == "paid";
    conn.execute("INSERT INTO attempts (id, stage_id, number, agent, model_ref, tier, billing, state, started_at, rung) \
                  VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![id, stage_id, number, step.agent, step_label(step), step.tier, step.billing, if paid { "waiting" } else { "running" },
            if paid { None } else { Some(now) }, index as i64])?;
    if paid {
        // Agents report no cost before they run, so the approval carries no cap: it approves the step itself.
        conn.execute("INSERT INTO approvals (id, attempt_id, cap_micro_usd, policy_version, state, created_at) \
                      VALUES (?1, ?2, 0, 'tasks-v1', 'pending', ?3)", params![new_id(), id, now])?;
        set_task(conn, task_id, "waiting_approval", None, Some(&paid_reason(index, step)))?;
        return Ok(Opened::Waiting);
    }
    Ok(Opened::Run(id))
}

fn paid_reason(index: usize, step: &Step) -> String {
    format!("Step {} ({}) is marked paid, so it waits for your approval.", index + 1, step_label(step))
}

fn close_attempt(conn: &Connection, id: &str, outcome: &str, escalation: Option<&str>) -> rusqlite::Result<()> {
    conn.execute("UPDATE attempts SET state = ?2, escalation_reason = COALESCE(?3, escalation_reason), finished_at = ?4 WHERE id = ?1",
        params![id, outcome, escalation, now_ms()]).map(|_| ())
}

fn save_run(conn: &Connection, id: &str, run: &TaskRun) -> rusqlite::Result<()> {
    let detail = run.usage.error.clone().filter(|_| run.outcome != "ok").unwrap_or_else(|| run.exit.clone());
    conn.execute("UPDATE attempts SET worktree_path = ?2, base_revision = ?3, agent_model = ?4, detail = ?5, files_changed = ?6, \
                  insertions = ?7, deletions = ?8, summary = ?9, log_tail = ?10 WHERE id = ?1",
        params![id, run.worktree, run.base, run.usage.model, detail, run.files_changed as i64, run.insertions as i64,
            run.deletions as i64, run.usage.result, run.log_tail]).map(|_| ())
}

fn forget_worktree(conn: &Connection, attempt: &str) -> rusqlite::Result<()> {
    conn.execute("UPDATE attempts SET worktree_path = NULL WHERE id = ?1", [attempt]).map(|_| ())
}

/// What the next step is told about a failed check.
fn handoff(check: &CheckResult) -> String {
    format!("The check `{}` {} on that attempt's changes. The end of its output:\n{}", check.command, check.detail,
        last_chars(&check.output_tail, HANDOFF_CHARS))
}

fn handoff_of(attempt: &AttemptView) -> Option<String> {
    if attempt.detail.as_deref() == Some(NO_CHANGES) {
        return Some(NO_CHANGES_HANDOFF.into());
    }
    attempt.checks.iter().find(|c| !c.passed).map(handoff)
}

/// The prompt for an attempt: the task, plus why the previous attempt failed its checks.
pub fn compose(prompt: &str, handoff: Option<&str>) -> String {
    match handoff {
        None => prompt.to_string(),
        Some(why) => format!("{prompt}\n\n---\nA previous attempt at this task, started from the same code, did not pass the \
            workspace's checks. You start again from the original code.\n{why}"),
    }
}

/// Where a task continues: the step after the last failure, or the same step after a stop.
fn resume_point(attempts: &[AttemptView]) -> Option<(usize, Option<String>)> {
    let last_failed = attempts.iter().rev().find(|a| a.outcome == "failed");
    let carried = last_failed.and_then(handoff_of);
    match attempts.last() {
        None => Some((0, None)),
        Some(a) if a.outcome == "failed" => Some((a.step + 1, carried)),
        Some(a) if a.outcome == "passed" || a.outcome == "done" => None,
        Some(a) => Some((a.step, carried)),
    }
}

// ---------- Running ----------

pub type Cancel = watch::Receiver<bool>;

/// Resolves once the task is stopped; never, if whoever could stop it is gone.
pub fn canceled(mut rx: Cancel) -> impl std::future::Future<Output = ()> + Send + 'static {
    async move {
        if rx.wait_for(|stopped| *stopped).await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExecRequest {
    pub agent: String,
    pub model: Option<String>,
    pub workspace: PathBuf,
    pub base: String,
    pub prompt: String,
    pub allow_shell: bool,
    pub timeout_secs: u64,
}

/// Runs one attempt and returns its run, worktree included. The app's runs the agent CLI (`agent_executor`);
/// tests substitute their own.
pub type Executor = Arc<dyn Fn(ExecRequest, PathBuf, Cancel) -> BoxFuture<'static, Result<TaskRun>> + Send + Sync>;

pub fn agent_executor() -> Executor {
    Arc::new(|request: ExecRequest, worktrees: PathBuf, cancel: Cancel| -> BoxFuture<'static, Result<TaskRun>> {
        Box::pin(async move {
            let request = agents::TaskRequest {
                agent: request.agent, workspace: request.workspace, prompt: request.prompt, model: request.model,
                allow_shell: request.allow_shell, timeout_secs: Some(request.timeout_secs), base: Some(request.base),
            };
            agents::run(&request, &worktrees, canceled(cancel)).await
        })
    })
}

/// Told when a task's runner stops, with the task and the state it stopped in (`waiting_approval`,
/// `checks_passed`, `needs_review`, `check_failed`, `blocked`, `canceled`). The app notifies the user from it.
pub type Notify = Arc<dyn Fn(&str, &str) + Send + Sync>;

/// Runs tasks in the background, one runner per task, each with its own stop signal.
#[derive(Clone)]
pub struct Runner {
    db: Arc<Mutex<Connection>>,
    worktrees: PathBuf,
    exec: Executor,
    live: Arc<Mutex<HashMap<String, watch::Sender<bool>>>>,
    notify: Option<Notify>,
}

/// Takes a task off the live list however its runner ends.
struct Live {
    live: Arc<Mutex<HashMap<String, watch::Sender<bool>>>>,
    id: String,
}

impl Drop for Live {
    fn drop(&mut self) {
        self.live.lock().unwrap_or_else(|p| p.into_inner()).remove(&self.id);
    }
}

impl Runner {
    pub fn new(db: Arc<Mutex<Connection>>, worktrees: PathBuf, exec: Executor) -> Runner {
        Runner { db, worktrees, exec, live: Arc::default(), notify: None }
    }

    /// Calls `notify` whenever a task's runner stops.
    pub fn with_notify(self, notify: Notify) -> Runner {
        Runner { notify: Some(notify), ..self }
    }

    fn with_db<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
        f(&lock(&self.db))
    }

    pub fn is_live(&self, id: &str) -> bool {
        self.live.lock().unwrap_or_else(|p| p.into_inner()).contains_key(id)
    }

    /// Runs (or continues) a task in the background. Needs a Tokio runtime. `None` when it already runs.
    pub fn start(&self, id: &str) -> Option<tokio::task::JoinHandle<()>> {
        let (tx, rx) = watch::channel(false);
        {
            let mut live = self.live.lock().unwrap_or_else(|p| p.into_inner());
            if live.contains_key(id) {
                return None;
            }
            live.insert(id.to_string(), tx);
        }
        let guard = Live { live: self.live.clone(), id: id.to_string() };
        let runner = self.clone();
        let id = id.to_string();
        Some(tokio::spawn(async move {
            let _guard = guard;
            if let Err(e) = runner.drive(&id, &rx).await {
                // The store or the workspace failed under it: the task cannot continue on its own.
                runner.with_db(|c| set_task(c, &id, "blocked", None, Some(&e))).ok();
            }
            if let Some(notify) = &runner.notify {
                if let Ok(state) = runner.with_db(|c| task_row(c, &id)).map(|row| row.state) {
                    notify(&id, &state);
                }
            }
        }))
    }

    /// Stops a task: a running one ends its agent or check (the whole process tree); a waiting or interrupted
    /// one is closed.
    pub fn cancel(&self, id: &str) -> Result<()> {
        let live = self.live.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(tx) = live.get(id) {
            let _ = tx.send(true);
            return Ok(());
        }
        self.with_db(|c| {
            let row = task_row(c, id)?;
            if !matches!(row.state.as_str(), "waiting_approval" | "interrupted" | "needs_revalidation" | "running") {
                return Err("Only a running or waiting task can be stopped".into());
            }
            let tx = c.unchecked_transaction().map_err(err)?;
            tx.execute("UPDATE approvals SET state = 'invalidated', decided_at = ?2 WHERE state = 'pending' AND attempt_id IN \
                        (SELECT id FROM attempts WHERE stage_id = ?1)", params![row.stage_id, now_ms()]).map_err(err)?;
            tx.execute("UPDATE attempts SET state = 'canceled', finished_at = ?2 WHERE stage_id = ?1 AND state IN ('waiting', 'running')",
                params![row.stage_id, now_ms()]).map_err(err)?;
            set_task(&tx, id, "canceled", None, Some("You stopped it")).map_err(err)?;
            audit(&tx, "task.canceled", id, serde_json::json!({})).map_err(err)?;
            tx.commit().map_err(err)
        })
    }

    /// Approves or declines the paid step a task waits on. Approving continues the task.
    pub fn decide(&self, id: &str, approve: bool) -> Result<Option<tokio::task::JoinHandle<()>>> {
        self.with_db(|c| {
            let row = task_row(c, id)?;
            if row.state != "waiting_approval" {
                return Err("This task is not waiting for an approval".into());
            }
            let attempt: String = c.query_row("SELECT id FROM attempts WHERE stage_id = ?1 AND state = 'waiting' ORDER BY number DESC LIMIT 1",
                [&row.stage_id], |r| r.get(0)).optional().map_err(err)?.ok_or("This task is not waiting for an approval")?;
            let tx = c.unchecked_transaction().map_err(err)?;
            tx.execute("UPDATE approvals SET state = ?2, decided_at = ?3 WHERE attempt_id = ?1 AND state = 'pending'",
                params![attempt, if approve { "approved" } else { "denied" }, now_ms()]).map_err(err)?;
            if approve {
                set_task(&tx, id, "running", Some("preparing"), None).map_err(err)?;
            } else {
                close_attempt(&tx, &attempt, "canceled", Some("You declined this paid step")).map_err(err)?;
                set_task(&tx, id, "blocked", None, Some("You declined the paid step, and no step after it is free to run")).map_err(err)?;
            }
            audit(&tx, if approve { "approval.approved" } else { "approval.denied" }, id, serde_json::json!({"attempt": attempt})).map_err(err)?;
            tx.commit().map_err(err)
        })?;
        Ok(if approve { self.start(id) } else { None })
    }

    /// Runs an interrupted task again from the step that was running.
    pub fn retry(&self, id: &str) -> Result<Option<tokio::task::JoinHandle<()>>> {
        self.with_db(|c| {
            let row = task_row(c, id)?;
            if row.state != "interrupted" {
                return Err("Only an interrupted task can be retried".into());
            }
            set_task(c, id, "running", Some("preparing"), None).map_err(err)
        })?;
        Ok(self.start(id))
    }

    async fn drive(&self, id: &str, cancel: &Cancel) -> Result<()> {
        let row = self.with_db(|c| task_row(c, id))?;
        if row.state != "running" {
            return Ok(());
        }
        let base = row.base_revision.clone().ok_or("The task has no base commit")?;
        let attempts = self.with_db(|c| attempts_of(c, id))?;
        let Some((mut index, mut carried)) = resume_point(&attempts) else { return Ok(()) };
        let mut last_failure = attempts.iter().rev().find(|a| a.outcome == "failed").and_then(|a| a.escalation_reason.clone());
        let stop = |runner: &Runner| runner.with_db(|c| set_task(c, id, "canceled", None, Some("You stopped it"))).map_err(err);

        loop {
            if *cancel.borrow() {
                return stop(self);
            }
            let Some(step) = row.policy.steps.get(index) else {
                let reason = format!("Every step failed. Last: {}", last_failure.as_deref().unwrap_or("no attempt ran"));
                return self.with_db(|c| set_task(c, id, "check_failed", None, Some(&reason))).map_err(err);
            };
            let attempt = match self.with_db(|c| open_attempt(c, id, &row.stage_id, index, step)).map_err(err)? {
                Opened::Run(attempt) => attempt,
                Opened::Waiting | Opened::Declined => return Ok(()),
            };
            self.with_db(|c| set_phase(c, id, "generating")).map_err(err)?;
            let request = ExecRequest {
                agent: step.agent.clone(), model: step.model.clone(), workspace: PathBuf::from(&row.workspace_path),
                base: base.clone(), prompt: compose(&row.prompt, carried.as_deref()), allow_shell: row.policy.allow_shell,
                timeout_secs: row.policy.timeout_secs,
            };
            let name = agents::find(&step.agent).map(|a| a.name).unwrap_or(step.agent.as_str());
            let run = match (self.exec)(request, self.worktrees.clone(), cancel.clone()).await {
                Ok(run) => run,
                Err(e) => {
                    // The agent could not start (not installed, the workspace moved): try the next step.
                    let reason = format!("{name} could not run: {e}");
                    self.with_db(|c| {
                        c.execute("UPDATE attempts SET detail = ?2 WHERE id = ?1", params![attempt, e])?;
                        close_attempt(c, &attempt, "failed", Some(&reason))
                    }).map_err(err)?;
                    last_failure = Some(reason);
                    index += 1;
                    continue;
                }
            };
            self.with_db(|c| -> Result<()> {
                agents::record_for(c, &run, Some(&attempt))?;
                save_run(c, &attempt, &run).map_err(err)
            })?;
            let worktree = Worktree { repo: PathBuf::from(&row.workspace_path), path: PathBuf::from(&run.worktree), base: run.base.clone() };
            let drop_worktree = |runner: &Runner| {
                let _ = isolation::discard(&worktree);
                runner.with_db(|c| forget_worktree(c, &attempt)).map_err(err)
            };

            if run.outcome == "canceled" || *cancel.borrow() {
                drop_worktree(self)?;
                self.with_db(|c| close_attempt(c, &attempt, "canceled", None)).map_err(err)?;
                return stop(self);
            }
            if run.outcome != "ok" {
                let detail = run.usage.error.clone().unwrap_or_else(|| run.exit.clone());
                let reason = format!("{name} did not finish: {}", last_chars(&detail, 300));
                drop_worktree(self)?;
                self.with_db(|c| close_attempt(c, &attempt, "failed", Some(&reason))).map_err(err)?;
                // The agent's own failure says nothing about the code, so the next step starts with the task alone.
                carried = None;
                last_failure = Some(reason);
                index += 1;
                continue;
            }
            if run.files_changed == 0 {
                drop_worktree(self)?;
                self.with_db(|c| {
                    c.execute("UPDATE attempts SET detail = ?2 WHERE id = ?1", params![attempt, NO_CHANGES])?;
                    close_attempt(c, &attempt, "failed", Some(NO_CHANGES))
                }).map_err(err)?;
                carried = Some(NO_CHANGES_HANDOFF.into());
                last_failure = Some(format!("{name}: {NO_CHANGES}"));
                index += 1;
                continue;
            }
            if row.policy.checks.is_empty() {
                return self.with_db(|c| {
                    close_attempt(c, &attempt, "done", None)?;
                    set_task(c, id, "needs_review", None,
                        Some("No checks are approved for this workspace, so nothing confirmed the result. Review the changes yourself."))
                }).map_err(err);
            }

            self.with_db(|c| set_phase(c, id, "checking")).map_err(err)?;
            let mut results: Vec<CheckResult> = Vec::new();
            for check in &row.policy.checks {
                let result = checks::run(&check.command, &worktree.path, CHECK_TIMEOUT, &[], canceled(cancel.clone())).await;
                let passed = result.passed;
                results.push(result);
                if !passed || *cancel.borrow() {
                    break;
                }
            }
            let json = serde_json::to_string(&results).map_err(|e| e.to_string())?;
            self.with_db(|c| c.execute("UPDATE attempts SET check_results_json = ?2 WHERE id = ?1", params![attempt, json])).map_err(err)?;
            if *cancel.borrow() {
                drop_worktree(self)?;
                self.with_db(|c| close_attempt(c, &attempt, "canceled", None)).map_err(err)?;
                return stop(self);
            }
            match results.iter().find(|r| !r.passed) {
                None => {
                    // An attempt that edits the file defining a check can make that check pass without doing the task.
                    let changed = isolation::changed_files(&worktree).unwrap_or_default();
                    let mut touched: Vec<&str> = row.policy.checks.iter().map(|c| c.source.as_str())
                        .filter(|s| *s != "user" && changed.iter().any(|p| p == s || p.ends_with(&format!("/{s}")))).collect();
                    touched.dedup();
                    return self.with_db(|c| {
                        close_attempt(c, &attempt, "passed", None)?;
                        if touched.is_empty() {
                            set_task(c, id, "checks_passed", None, None)
                        } else {
                            set_task(c, id, "needs_review", None, Some(&format!(
                                "The checks passed, but this attempt changed {}, which defines them. Review the changes before trusting the result.",
                                touched.join(", "))))
                        }
                    }).map_err(err);
                }
                Some(failed) => {
                    let reason = format!("`{}` {}", failed.command, failed.detail);
                    carried = Some(handoff(failed));
                    drop_worktree(self)?;
                    self.with_db(|c| close_attempt(c, &attempt, "failed", Some(&reason))).map_err(err)?;
                    last_failure = Some(format!("{name}: {reason}"));
                    index += 1;
                }
            }
        }
    }
}

// ---------- Results ----------

fn result_attempt(conn: &Connection, id: &str) -> Result<(TaskRow, AttemptView, Worktree)> {
    let row = task_row(conn, id)?;
    if row.result.is_some() || !matches!(row.state.as_str(), "checks_passed" | "needs_review") {
        return Err("This task has no result waiting for you".into());
    }
    let attempt = attempts_of(conn, id)?.into_iter().rev().find(|a| a.open && (a.outcome == "passed" || a.outcome == "done"))
        .ok_or("The result's worktree is gone")?;
    let worktree = Worktree {
        repo: PathBuf::from(&row.workspace_path), path: PathBuf::from(attempt.worktree.clone().unwrap_or_default()),
        base: attempt.base.clone().or_else(|| row.base_revision.clone()).unwrap_or_default(),
    };
    Ok((row, attempt, worktree))
}

pub fn result_diff(conn: &Connection, id: &str) -> Result<String> {
    let (_, _, worktree) = result_attempt(conn, id)?;
    isolation::diff(&worktree).map_err(|e| e.to_string())
}

/// Keeps the result as a new branch of the workspace (the user's checkout is untouched) and removes the worktree.
pub fn keep(conn: &Connection, id: &str, title: Option<&str>) -> Result<String> {
    let (row, attempt, worktree) = result_attempt(conn, id)?;
    let title = title.map(str::trim).filter(|t| !t.is_empty()).unwrap_or(&row.title);
    let branch = isolation::branch_name(title, id);
    isolation::keep_as_branch(&worktree, &branch, title).map_err(|e| e.to_string())?;
    let _ = isolation::discard(&worktree);
    let tx = conn.unchecked_transaction().map_err(err)?;
    forget_worktree(&tx, &attempt.id).map_err(err)?;
    tx.execute("UPDATE tasks SET result = 'kept', result_branch = ?2, \
                state = CASE state WHEN 'needs_review' THEN 'manually_accepted' ELSE state END, \
                reason = CASE state WHEN 'needs_review' THEN NULL ELSE reason END, updated_at = ?3 WHERE id = ?1",
        params![id, branch, now_ms()]).map_err(err)?;
    audit(&tx, "task.kept", id, serde_json::json!({"branch": branch})).map_err(err)?;
    tx.commit().map_err(err)?;
    Ok(branch)
}

/// Discards the result and its worktree.
pub fn discard(conn: &Connection, id: &str) -> Result<()> {
    let (_, attempt, worktree) = result_attempt(conn, id)?;
    isolation::discard(&worktree).map_err(|e| e.to_string())?;
    let tx = conn.unchecked_transaction().map_err(err)?;
    forget_worktree(&tx, &attempt.id).map_err(err)?;
    tx.execute("UPDATE tasks SET result = 'discarded', \
                state = CASE state WHEN 'needs_review' THEN 'canceled' ELSE state END, \
                reason = CASE state WHEN 'needs_review' THEN 'You discarded the result' ELSE reason END, updated_at = ?2 WHERE id = ?1",
        params![id, now_ms()]).map_err(err)?;
    audit(&tx, "task.discarded", id, serde_json::json!({})).map_err(err)?;
    tx.commit().map_err(err)
}

// ---------- Tasks from gateway clients (the task API, the MCP server) ----------

/// Refuses a client access to a task it did not start, in the same words as a task that does not exist.
pub fn check_owner(conn: &Connection, id: &str, client_id: &str) -> Result<()> {
    let owner: Option<Option<String>> = conn.query_row("SELECT client_id FROM tasks WHERE id = ?1", [id], |r| r.get(0))
        .optional().map_err(err)?;
    match owner {
        Some(Some(owner)) if owner == client_id => Ok(()),
        _ => Err("That task no longer exists".into()),
    }
}

/// The workspace for a folder an IDE agent works in: the repository it belongs to, added as a workspace if it is
/// not one yet. Checks found there arrive as suggestions; until the user approves them in the app, results wait
/// for review.
pub fn workspace_for_folder(conn: &Connection, folder: &str) -> Result<WorkspaceView> {
    let folder = folder.trim().trim_matches('"').trim();
    let root = isolation::toplevel(Path::new(folder)).map_err(|e| e.to_string())?;
    let found = list_workspaces(conn)?.into_iter().find(|w| same_path(Path::new(&w.path), &root));
    match found {
        Some(w) => Ok(w),
        None => add_workspace(conn, &root.to_string_lossy(), None),
    }
}

/// The order each strategy tries the agents in (the same as New Task in the app).
fn strategy_order(strategy: &str) -> [&'static str; 4] {
    match strategy {
        "balanced" => ["codex", "opencode", "claude-code", "gemini-cli"],
        "quality" => ["claude-code", "codex", "gemini-cli", "opencode"],
        _ => ["opencode", "gemini-cli", "codex", "claude-code"],
    }
}

/// A starting guess for how capable an agent's default model is.
pub fn tier_guess(agent: &str) -> u8 {
    match agent {
        "opencode" => 1,
        "gemini-cli" => 2,
        _ => 3,
    }
}

/// Up to three installed agents in the strategy's order, with billing unknown: Pygg cannot see how each is
/// signed in.
pub fn default_steps(strategy: &str, installed: &[&str]) -> Vec<Step> {
    strategy_order(strategy).into_iter().filter(|id| installed.contains(id)).take(3)
        .map(|id| Step { agent: id.into(), model: None, tier: tier_guess(id), billing: "unknown".into() }).collect()
}

/// The agents on this computer's PATH (no programs started).
pub fn installed_agents() -> Vec<&'static str> {
    agents::AGENTS.iter().filter(|a| a.programs.iter().any(|p| agents::locate(p).is_some())).map(|a| a.id).collect()
}

pub fn list_tasks_for(conn: &Connection, client_id: &str) -> Result<Vec<TaskSummary>> {
    let ids: Vec<String> = conn.prepare("SELECT id FROM tasks WHERE client_id = ?1 ORDER BY updated_at DESC LIMIT 50").map_err(err)?
        .query_map([client_id], |r| r.get(0)).map_err(err)?.collect::<rusqlite::Result<_>>().map_err(err)?;
    ids.iter().map(|id| {
        let row = task_row(conn, id)?;
        let attempts = attempts_of(conn, id)?;
        summary_of(conn, id, &row, &attempts)
    }).collect()
}

// ---------- Start-up ----------

/// At start-up nothing runs yet: a task left `running` was cut off when the app closed. It becomes
/// `interrupted` (Retry runs that step again), its attempt is closed as canceled, and every worktree under
/// `worktrees` that no waiting result needs is removed.
pub fn recover(conn: &Connection, worktrees: &Path) -> Result<usize> {
    let now = now_ms();
    let tx = conn.unchecked_transaction().map_err(err)?;
    tx.execute("UPDATE attempts SET state = 'canceled', detail = COALESCE(detail, 'The app closed while it ran'), finished_at = ?1 \
                WHERE state = 'running'", [now]).map_err(err)?;
    let interrupted = tx.execute("UPDATE tasks SET state = 'interrupted', phase = NULL, \
                reason = 'The app closed while this task ran. Retry runs the step again from the same code.', updated_at = ?1 \
                WHERE state = 'running'", [now]).map_err(err)?;
    tx.execute("UPDATE task_stages SET state = 'interrupted' WHERE state = 'running'", []).map_err(err)?;
    tx.commit().map_err(err)?;

    // Worktrees whose result still waits for the user stay; everything else in the app's folder is a leftover.
    let keep: Vec<PathBuf> = conn.prepare("SELECT a.worktree_path FROM attempts a WHERE a.worktree_path IS NOT NULL").map_err(err)?
        .query_map([], |r| r.get::<_, String>(0)).map_err(err)?
        .filter_map(|p| p.ok()).map(PathBuf::from).collect();
    if let Ok(entries) = std::fs::read_dir(worktrees) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && !keep.iter().any(|k| same_path(k, &path)) {
                let _ = std::fs::remove_dir_all(&path);
            }
        }
    }
    conn.execute("UPDATE attempts SET worktree_path = NULL WHERE state NOT IN ('passed', 'done') AND worktree_path IS NOT NULL", [])
        .map_err(err)?;
    let repos: Vec<String> = conn.prepare("SELECT path FROM workspaces").map_err(err)?
        .query_map([], |r| r.get(0)).map_err(err)?.filter_map(|p| p.ok()).collect();
    for repo in repos {
        let _ = isolation::prune(Path::new(&repo));
    }
    Ok(interrupted)
}

fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    norm(a) == norm(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::AgentUsage;
    use crate::isolation::tests::repo_with_commit;

    #[derive(Clone)]
    enum Act {
        Write(&'static str, &'static str),
        Fail,
        Nothing,
        WaitForStop,
    }

    struct Rig {
        _dir: tempfile::TempDir,
        db: Arc<Mutex<Connection>>,
        repo: PathBuf,
        worktrees: PathBuf,
        workspace: String,
        prompts: Arc<Mutex<Vec<String>>>,
    }

    fn rig(checks: &[&str]) -> Rig {
        let dir = tempfile::tempdir().unwrap();
        let repo = repo_with_commit(dir.path());
        let conn = crate::db::open_in_memory().unwrap();
        let ws = add_workspace(&conn, &repo.to_string_lossy(), Some("repo")).unwrap();
        for check in checks {
            add_check(&conn, &ws.id, check).unwrap();
        }
        Rig { worktrees: dir.path().join("worktrees"), _dir: dir, db: Arc::new(Mutex::new(conn)), repo, workspace: ws.id,
            prompts: Arc::default() }
    }

    /// An executor that does `acts` in order, one per attempt, in a real worktree.
    fn fake(acts: Vec<Act>, prompts: Arc<Mutex<Vec<String>>>) -> Executor {
        let acts = Arc::new(Mutex::new(std::collections::VecDeque::from(acts)));
        Arc::new(move |request: ExecRequest, worktrees: PathBuf, cancel: Cancel| -> BoxFuture<'static, Result<TaskRun>> {
            let act = acts.lock().unwrap().pop_front().unwrap_or(Act::Fail);
            prompts.lock().unwrap().push(request.prompt.clone());
            Box::pin(async move {
                let id = new_id();
                std::fs::create_dir_all(&worktrees).unwrap();
                let worktree = isolation::create(&request.workspace, &request.base, &worktrees.join(&id[..12])).map_err(|e| e.to_string())?;
                let mut outcome = "ok";
                match act {
                    Act::Write(file, text) => std::fs::write(worktree.path.join(file), text).unwrap(),
                    Act::Fail => outcome = "failed",
                    Act::Nothing => {}
                    Act::WaitForStop => {
                        canceled(cancel).await;
                        outcome = "canceled";
                    }
                }
                let stat = isolation::changes(&worktree).unwrap();
                Ok(TaskRun {
                    id, agent: request.agent, model: request.model, workspace: request.workspace.to_string_lossy().into_owned(),
                    worktree: worktree.path.to_string_lossy().into_owned(), base: worktree.base, outcome: outcome.into(),
                    exit: if outcome == "ok" { "exit 0".into() } else { "exit 1".into() }, duration_ms: 5,
                    files_changed: stat.files, insertions: stat.insertions, deletions: stat.deletions,
                    usage: AgentUsage { input_tokens: Some(100), output_tokens: Some(10), ..AgentUsage::default() },
                    log_tail: String::new(),
                })
            })
        })
    }

    fn step(agent: &str, billing: &str, tier: u8) -> Step {
        Step { agent: agent.into(), model: None, tier, billing: billing.into() }
    }

    fn create(rig: &Rig, steps: Vec<Step>) -> String {
        let new = NewTask { workspace_id: rig.workspace.clone(), title: None, prompt: "Make result.txt say good".into(),
            strategy: "free_first".into(), steps, allow_shell: false, timeout_secs: None };
        create_task(&lock(&rig.db), &new).unwrap()
    }

    /// A check that passes when result.txt contains "good".
    fn good_check() -> &'static str {
        if cfg!(windows) { "findstr /c:good result.txt" } else { "grep -q good result.txt" }
    }

    fn detail(rig: &Rig, id: &str) -> TaskDetail {
        task_detail(&lock(&rig.db), id).unwrap()
    }

    #[tokio::test]
    async fn a_failed_check_moves_to_the_next_step_with_what_failed() {
        let rig = rig(&[good_check()]);
        let runner = Runner::new(rig.db.clone(), rig.worktrees.clone(),
            fake(vec![Act::Write("result.txt", "bad\n"), Act::Write("result.txt", "good\n")], rig.prompts.clone()));
        let id = create(&rig, vec![step("opencode", "free", 1), step("codex", "subscription", 3)]);
        runner.start(&id).unwrap().await.unwrap();

        let task = detail(&rig, &id);
        assert_eq!(task.summary.state, "checks_passed", "{:?}", task.reason);
        let outcomes: Vec<&str> = task.attempt_details.iter().map(|a| a.outcome.as_str()).collect();
        assert_eq!(outcomes, ["failed", "passed"]);
        assert!(!task.attempt_details[0].open && task.attempt_details[1].open, "the failed attempt's worktree is gone, the result's stays");
        assert_eq!(task.attempt_details[0].escalation_reason.as_deref(), Some(format!("`{}` exited 1", good_check()).as_str()));
        let prompts = rig.prompts.lock().unwrap().clone();
        assert_eq!(prompts[0], "Make result.txt say good");
        assert!(prompts[1].starts_with("Make result.txt say good\n\n---\n") && prompts[1].contains("did not pass"), "{}", prompts[1]);
        // Each attempt has its own ledger entry, with an unknown cost.
        assert_eq!(task.summary.cost.unknown_calls, 2);
        assert_eq!(task.attempt_details[1].input_tokens, Some(100));

        let diff = result_diff(&lock(&rig.db), &id).unwrap();
        assert!(diff.contains("+good"), "{diff}");
        let branch = keep(&lock(&rig.db), &id, None).unwrap();
        assert!(branch.starts_with("str/make-result-txt-say-good-"), "{branch}");
        let task = detail(&rig, &id);
        assert_eq!((task.summary.state.as_str(), task.result.as_deref()), ("checks_passed", Some("kept")));
        assert!(task.attempt_details.iter().all(|a| !a.open));
        assert_eq!(std::fs::read_to_string(rig.repo.join("app.txt")).unwrap(), "one\n", "the user's checkout is untouched");
    }

    #[tokio::test]
    async fn every_step_failing_ends_in_check_failed() {
        let rig = rig(&[good_check()]);
        let runner = Runner::new(rig.db.clone(), rig.worktrees.clone(),
            fake(vec![Act::Fail, Act::Nothing, Act::Write("result.txt", "bad\n")], rig.prompts.clone()));
        let id = create(&rig, vec![step("opencode", "free", 1), step("opencode", "free", 1), step("codex", "subscription", 3)]);
        runner.start(&id).unwrap().await.unwrap();
        let task = detail(&rig, &id);
        assert_eq!(task.summary.state, "check_failed");
        assert!(task.reason.as_deref().unwrap().starts_with("Every step failed"), "{:?}", task.reason);
        assert_eq!(task.attempt_details[0].escalation_reason.as_deref(), Some("OpenCode did not finish: exit 1"));
        assert_eq!(task.attempt_details[1].detail.as_deref(), Some(NO_CHANGES));
        let prompts = rig.prompts.lock().unwrap().clone();
        assert_eq!(prompts[1], "Make result.txt say good", "an agent's own failure is not handed on");
        assert!(prompts[2].contains(NO_CHANGES_HANDOFF));
        assert!(std::fs::read_dir(&rig.worktrees).unwrap().next().is_none(), "no worktree is left behind");
    }

    #[tokio::test]
    async fn without_checks_the_result_needs_review_and_keeping_it_accepts_it() {
        let rig = rig(&[]);
        let runner = Runner::new(rig.db.clone(), rig.worktrees.clone(), fake(vec![Act::Write("new.txt", "x\n")], rig.prompts.clone()));
        let id = create(&rig, vec![step("codex", "subscription", 3)]);
        runner.start(&id).unwrap().await.unwrap();
        let task = detail(&rig, &id);
        assert_eq!((task.summary.state.as_str(), task.attempt_details[0].outcome.as_str()), ("needs_review", "done"));
        keep(&lock(&rig.db), &id, Some("Add new file")).unwrap();
        assert_eq!(detail(&rig, &id).summary.state, "manually_accepted");
    }

    #[tokio::test]
    async fn changing_the_file_that_defines_a_check_needs_review() {
        let rig = rig(&[]);
        {
            let conn = lock(&rig.db);
            conn.execute("INSERT INTO workspace_checks (id, workspace_id, command, source, approved) VALUES ('c', ?1, 'exit 0', 'package.json', 1)",
                [&rig.workspace]).unwrap();
        }
        let runner = Runner::new(rig.db.clone(), rig.worktrees.clone(),
            fake(vec![Act::Write("package.json", "{\"scripts\":{\"test\":\"exit 0\"}}")], rig.prompts.clone()));
        let id = create(&rig, vec![step("codex", "subscription", 3)]);
        runner.start(&id).unwrap().await.unwrap();
        let task = detail(&rig, &id);
        assert_eq!(task.summary.state, "needs_review");
        assert!(task.reason.unwrap().contains("package.json"));
        discard(&lock(&rig.db), &id).unwrap();
        let task = detail(&rig, &id);
        assert_eq!((task.summary.state.as_str(), task.result.as_deref()), ("canceled", Some("discarded")));
    }

    #[tokio::test]
    async fn a_paid_step_waits_for_approval() {
        let rig = rig(&[good_check()]);
        let runner = Runner::new(rig.db.clone(), rig.worktrees.clone(),
            fake(vec![Act::Write("result.txt", "bad\n"), Act::Write("result.txt", "good\n")], rig.prompts.clone()));
        let id = create(&rig, vec![step("opencode", "free", 1), step("claude-code", "paid", 3)]);
        runner.start(&id).unwrap().await.unwrap();
        let task = detail(&rig, &id);
        assert_eq!(task.summary.state, "waiting_approval");
        assert_eq!(task.attempt_details.iter().map(|a| a.outcome.as_str()).collect::<Vec<_>>(), ["failed", "waiting"]);
        assert_eq!(rig.prompts.lock().unwrap().len(), 1, "the paid step did not start");

        runner.decide(&id, true).unwrap().unwrap().await.unwrap();
        let task = detail(&rig, &id);
        assert_eq!(task.summary.state, "checks_passed");
        assert_eq!(task.attempt_details.len(), 2, "the approved attempt is the one that ran");
        assert!(rig.prompts.lock().unwrap()[1].contains("did not pass"), "the failure is still handed on after the wait");
    }

    #[tokio::test]
    async fn the_app_is_told_where_each_runner_stopped() {
        let rig = rig(&[]);
        let seen: Arc<Mutex<Vec<String>>> = Arc::default();
        let log = seen.clone();
        let runner = Runner::new(rig.db.clone(), rig.worktrees.clone(),
            fake(vec![Act::Write("x.txt", "x")], rig.prompts.clone()))
            .with_notify(Arc::new(move |_, state| log.lock().unwrap().push(state.to_string())));
        let paid = create(&rig, vec![step("claude-code", "paid", 3)]);
        runner.start(&paid).unwrap().await.unwrap();
        let free = create(&rig, vec![step("codex", "subscription", 3)]);
        runner.start(&free).unwrap().await.unwrap();
        assert_eq!(*seen.lock().unwrap(), ["waiting_approval", "needs_review"]);
    }

    #[tokio::test]
    async fn declining_a_paid_step_blocks_the_task() {
        let rig = rig(&[]);
        let runner = Runner::new(rig.db.clone(), rig.worktrees.clone(), fake(vec![], rig.prompts.clone()));
        let id = create(&rig, vec![step("claude-code", "paid", 3)]);
        runner.start(&id).unwrap().await.unwrap();
        assert!(runner.decide(&id, false).unwrap().is_none());
        let task = detail(&rig, &id);
        assert_eq!((task.summary.state.as_str(), task.attempt_details[0].outcome.as_str()), ("blocked", "canceled"));
        assert!(rig.prompts.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn stopping_a_running_task_ends_its_attempt() {
        let rig = rig(&[]);
        let runner = Runner::new(rig.db.clone(), rig.worktrees.clone(), fake(vec![Act::WaitForStop], rig.prompts.clone()));
        let id = create(&rig, vec![step("codex", "subscription", 3), step("opencode", "free", 1)]);
        let handle = runner.start(&id).unwrap();
        while rig.prompts.lock().unwrap().is_empty() {
            tokio::task::yield_now().await;
        }
        assert!(runner.is_live(&id) && runner.start(&id).is_none(), "a task runs once");
        runner.cancel(&id).unwrap();
        handle.await.unwrap();
        let task = detail(&rig, &id);
        assert_eq!((task.summary.state.as_str(), task.attempt_details[0].outcome.as_str()), ("canceled", "canceled"));
        assert_eq!(task.attempt_details.len(), 1, "the next step does not start");
        assert!(!runner.is_live(&id));
    }

    #[tokio::test]
    async fn a_task_cut_off_by_a_restart_is_interrupted_and_can_be_retried() {
        let rig = rig(&[]);
        let id = create(&rig, vec![step("codex", "subscription", 3)]);
        let leftover = rig.worktrees.join("leftover");
        std::fs::create_dir_all(&leftover).unwrap();
        {
            let conn = lock(&rig.db);
            let row = task_row(&conn, &id).unwrap();
            let Opened::Run(_) = open_attempt(&conn, &id, &row.stage_id, 0, &row.policy.steps[0]).unwrap() else { panic!() };
            assert_eq!(recover(&conn, &rig.worktrees).unwrap(), 1);
        }
        assert!(!leftover.exists(), "a worktree nobody needs is removed");
        let task = detail(&rig, &id);
        assert_eq!((task.summary.state.as_str(), task.attempt_details[0].outcome.as_str()), ("interrupted", "canceled"));

        let runner = Runner::new(rig.db.clone(), rig.worktrees.clone(), fake(vec![Act::Write("x.txt", "x")], rig.prompts.clone()));
        runner.retry(&id).unwrap().unwrap().await.unwrap();
        let task = detail(&rig, &id);
        assert_eq!(task.summary.state, "needs_review");
        assert_eq!(task.attempt_details.iter().map(|a| (a.number, a.step)).collect::<Vec<_>>(), [(1, 0), (2, 0)], "the same step runs again");
    }

    #[tokio::test]
    async fn stopping_a_waiting_task_closes_it() {
        let rig = rig(&[]);
        let runner = Runner::new(rig.db.clone(), rig.worktrees.clone(), fake(vec![], rig.prompts.clone()));
        let id = create(&rig, vec![step("claude-code", "paid", 3)]);
        runner.start(&id).unwrap().await.unwrap();
        runner.cancel(&id).unwrap();
        let task = detail(&rig, &id);
        assert_eq!((task.summary.state.as_str(), task.attempt_details[0].outcome.as_str()), ("canceled", "canceled"));
        let approval: String = lock(&rig.db).query_row("SELECT state FROM approvals", [], |r| r.get(0)).unwrap();
        assert_eq!(approval, "invalidated");
        assert!(runner.cancel(&id).is_err(), "a finished task cannot be stopped");
    }

    #[test]
    fn creating_a_task_checks_what_it_is_given() {
        let rig = rig(&[good_check()]);
        let conn = lock(&rig.db);
        let base = NewTask { workspace_id: rig.workspace.clone(), title: None, prompt: "  ".into(), strategy: "free_first".into(),
            steps: vec![step("codex", "subscription", 3)], allow_shell: false, timeout_secs: None };
        assert_eq!(create_task(&conn, &base).unwrap_err(), "Describe the task");
        let prompt = "Fix it".to_string();
        assert!(create_task(&conn, &NewTask { prompt: prompt.clone(), steps: vec![], ..base.clone() }).is_err());
        assert!(create_task(&conn, &NewTask { prompt: prompt.clone(), steps: vec![step("cursor", "paid", 3)], ..base.clone() }).is_err());
        assert!(create_task(&conn, &NewTask { prompt: prompt.clone(), steps: vec![step("codex", "paid", 7)], ..base.clone() }).is_err());
        assert!(create_task(&conn, &NewTask { prompt: prompt.clone(), strategy: "yolo".into(), ..base.clone() }).is_err());
        let id = create_task(&conn, &NewTask { prompt: "Fix the timeout\nand more".into(), ..base.clone() }).unwrap();
        let task = task_detail(&conn, &id).unwrap();
        assert_eq!(task.summary.title, "Fix the timeout");
        assert_eq!(task.policy.checks, [PolicyCheck { command: good_check().into(), source: "user".into() }]);
        // Checks are fixed for the task: approving another one later does not change what judges it.
        add_check(&conn, &rig.workspace, "exit 0").unwrap();
        assert_eq!(task_detail(&conn, &id).unwrap().policy.checks.len(), 1);
    }

    #[test]
    fn workspaces_are_repositories_with_suggested_checks() {
        let dir = tempfile::tempdir().unwrap();
        let conn = crate::db::open_in_memory().unwrap();
        assert!(add_workspace(&conn, &dir.path().to_string_lossy(), None).unwrap_err().contains("Git repository"));
        let repo = repo_with_commit(dir.path());
        std::fs::write(repo.join("Cargo.toml"), "[package]").unwrap();
        let ws = add_workspace(&conn, &format!("\"{}\"", repo.display()), None).unwrap();
        assert_eq!(ws.name, "repo");
        assert_eq!(ws.checks.iter().map(|c| (c.command.as_str(), c.approved)).collect::<Vec<_>>(),
            [("cargo check", false), ("cargo test", false)], "detected checks are suggestions");
        assert!(add_workspace(&conn, &repo.to_string_lossy(), None).unwrap_err().contains("already"));
        set_check(&conn, &ws.id, "cargo test", true).unwrap();
        add_check(&conn, &ws.id, "cargo clippy").unwrap();
        assert!(add_check(&conn, &ws.id, "a\nb").is_err());
        let ws = workspace(&conn, &ws.id).unwrap();
        assert_eq!(ws.checks.iter().filter(|c| c.approved).map(|c| c.command.as_str()).collect::<Vec<_>>(), ["cargo test", "cargo clippy"]);
        remove_workspace(&conn, &ws.id).unwrap();
        assert!(list_workspaces(&conn).unwrap().is_empty());
    }
}
