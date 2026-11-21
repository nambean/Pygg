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
use crate::messages::{self as m, fill, NO_CHANGES};
/// What an answer task's agent is told after the question.
const ANSWER_BRIEF: &str = "This is a question about the code, not a change. Read whatever you need in this \
    repository and answer it. Do not create, edit or delete any file. End with your whole answer as your last \
    message, in the language of the question, naming the files you rely on as path:line.";

const NO_CHANGES_HANDOFF: &str =
    "The previous attempt ended without changing any file. The task needs changes to the files.";

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
    /// This workspace's own daily limit for paid steps, on top of the overall one.
    pub daily_budget_usd: Option<f64>,
    /// `standard` or `sensitive`: in a sensitive workspace every step asks before it runs.
    pub privacy_class: String,
}

/// Adds a Git repository as a workspace and suggests the checks it already has.
pub fn add_workspace(conn: &Connection, path: &str, name: Option<&str>) -> Result<WorkspaceView> {
    let path = path.trim().trim_matches('"').trim();
    if path.is_empty() {
        return Err(m::CHOOSE_FOLDER.into());
    }
    let dir = Path::new(path);
    if !dir.is_dir() {
        return Err(fill(m::NOT_A_FOLDER, &[&path]));
    }
    if !isolation::is_repository(dir) {
        return Err(m::WORKSPACE_NOT_REPOSITORY.into());
    }
    let name = name
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(String::from)
        .or_else(|| dir.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| path.to_string());
    let id = new_id();
    conn.execute(
        "INSERT INTO workspaces (id, name, path, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![id, name, path, now_ms()],
    )
    .map_err(|e| match e {
        rusqlite::Error::SqliteFailure(f, _) if f.code == rusqlite::ErrorCode::ConstraintViolation => {
            "That folder is already a workspace".to_string()
        }
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
    let (name, path, budget, privacy_class): (String, String, Option<i64>, String) = conn
        .query_row(
            "SELECT name, path, daily_budget_micro_usd, privacy_class FROM workspaces WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(err)?
        .ok_or(m::WORKSPACE_GONE)?;
    let mut statement = conn
        .prepare("SELECT command, source, approved FROM workspace_checks WHERE workspace_id = ?1 ORDER BY rowid")
        .map_err(err)?;
    let checks = statement
        .query_map([id], |r| Ok(WorkspaceCheck { command: r.get(0)?, source: r.get(1)?, approved: r.get(2)? }))
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err)?;
    let tasks: u32 =
        conn.query_row("SELECT COUNT(*) FROM tasks WHERE workspace_id = ?1", [id], |r| r.get(0)).map_err(err)?;
    Ok(WorkspaceView {
        id: id.into(),
        name,
        path,
        checks,
        tasks,
        daily_budget_usd: budget.filter(|b| *b > 0).map(|b| b as f64 / 1e6),
        privacy_class,
    })
}

pub fn list_workspaces(conn: &Connection) -> Result<Vec<WorkspaceView>> {
    let ids: Vec<String> = conn
        .prepare("SELECT id FROM workspaces ORDER BY name COLLATE NOCASE")
        .map_err(err)?
        .query_map([], |r| r.get(0))
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;
    ids.iter().map(|id| workspace(conn, id)).collect()
}

/// Looks for checks again (a script was added since); new ones arrive as suggestions.
pub fn detect_checks(conn: &Connection, id: &str) -> Result<WorkspaceView> {
    let found = workspace(conn, id)?;
    suggest_checks(conn, id, Path::new(&found.path))?;
    workspace(conn, id)
}

pub fn set_check(conn: &Connection, id: &str, command: &str, approved: bool) -> Result<()> {
    let changed = conn
        .execute(
            "UPDATE workspace_checks SET approved = ?3 WHERE workspace_id = ?1 AND command = ?2",
            params![id, command, approved],
        )
        .map_err(err)?;
    if changed == 0 {
        return Err(m::CHECK_GONE.into());
    }
    audit(
        conn,
        if approved { "check.approved" } else { "check.unapproved" },
        id,
        serde_json::json!({"command": command}),
    )
    .map_err(err)
}

/// A check the user typed. Typing it is approving it.
pub fn add_check(conn: &Connection, id: &str, command: &str) -> Result<()> {
    let command = command.trim();
    if command.is_empty() {
        return Err(m::TYPE_COMMAND.into());
    }
    if command.chars().count() > 500 || command.contains('\n') {
        return Err(m::CHECK_TOO_LONG.into());
    }
    workspace(conn, id)?;
    conn.execute(
        "INSERT INTO workspace_checks (id, workspace_id, command, source, approved) VALUES (?1, ?2, ?3, 'user', 1) \
                  ON CONFLICT (workspace_id, command) DO UPDATE SET approved = 1",
        params![new_id(), id, command],
    )
    .map_err(err)?;
    audit(conn, "check.approved", id, serde_json::json!({"command": command})).map_err(err)
}

pub fn remove_check(conn: &Connection, id: &str, command: &str) -> Result<()> {
    conn.execute("DELETE FROM workspace_checks WHERE workspace_id = ?1 AND command = ?2", params![id, command])
        .map_err(err)?;
    Ok(())
}

/// `standard` or `sensitive`. A sensitive workspace's steps all ask before they run: Pygg cannot yet check where an
/// agent sends the code (data-use terms, ROADMAP B2), so the user decides each time.
pub fn set_privacy(conn: &Connection, id: &str, class: &str) -> Result<()> {
    if !matches!(class, "standard" | "sensitive") {
        return Err(fill(m::UNKNOWN_PRIVACY, &[&class]));
    }
    let changed =
        conn.execute("UPDATE workspaces SET privacy_class = ?2 WHERE id = ?1", params![id, class]).map_err(err)?;
    if changed == 0 {
        return Err(m::WORKSPACE_GONE.into());
    }
    audit(conn, "workspace.privacy", id, serde_json::json!({"class": class})).map_err(err)
}

fn is_sensitive(conn: &Connection, workspace_id: &str) -> rusqlite::Result<bool> {
    conn.query_row("SELECT privacy_class = 'sensitive' FROM workspaces WHERE id = ?1", [workspace_id], |r| r.get(0))
}

pub fn remove_workspace(conn: &Connection, id: &str) -> Result<()> {
    if workspace(conn, id)?.tasks > 0 {
        return Err(m::WORKSPACE_HAS_HISTORY.into());
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
    /// What the next step is told after a failed attempt (handoff.rs, ADR-030): `none`, `tail` or `evidence`.
    #[serde(default = "default_handoff")]
    pub handoff: String,
    /// A question about the code, not a change: the agent reads and answers, nothing is kept or checked, and its
    /// final message is the result (an answer task).
    #[serde(default)]
    pub answer: bool,
}

fn default_handoff() -> String {
    crate::handoff::DEFAULT_MODE.into()
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
    /// `none`, `tail` (the default) or `evidence` (H1, off unless asked for).
    #[serde(default)]
    pub handoff: Option<String>,
    /// An answer task: a question about the code; see `Policy::answer`.
    #[serde(default)]
    pub answer: bool,
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
    create_task_at(conn, new, client_id, None)
}

/// `create_task_for` from a named commit or branch of the workspace instead of its HEAD: a board card starts from
/// its brainstorm's branch, so it sees what the earlier cards built (ADR-042).
pub fn create_task_at(conn: &Connection, new: &NewTask, client_id: Option<&str>, base: Option<&str>) -> Result<String> {
    let prompt = new.prompt.trim();
    if prompt.is_empty() {
        return Err(m::DESCRIBE.into());
    }
    if !STRATEGIES.contains(&new.strategy.as_str()) {
        return Err(fill(m::UNKNOWN_STRATEGY, &[&new.strategy]));
    }
    if new.steps.is_empty() {
        return Err(m::ADD_STEP.into());
    }
    if new.steps.len() > MAX_STEPS {
        return Err(fill(m::TOO_MANY_STEPS, &[&MAX_STEPS]));
    }
    for step in &new.steps {
        agents::find(&step.agent).ok_or_else(|| fill(m::UNKNOWN_STEP_AGENT, &[&step.agent]))?;
        if step.tier > 4 {
            return Err(m::STEP_TIER.into());
        }
        if !BILLINGS.contains(&step.billing.as_str()) {
            return Err(fill(m::UNKNOWN_BILLING, &[&step.billing]));
        }
    }
    let handoff =
        new.handoff.as_deref().map(str::trim).filter(|h| !h.is_empty()).unwrap_or(crate::handoff::DEFAULT_MODE);
    if !crate::handoff::MODES.contains(&handoff) {
        return Err(fill(m::UNKNOWN_HANDOFF, &[&handoff]));
    }
    let ws = workspace(conn, &new.workspace_id)?;
    let base = match base {
        Some(base) => isolation::git(Path::new(&ws.path), &["rev-parse", "--verify", &format!("{base}^{{commit}}")])
            .map_err(|e| e.to_string())?,
        None => isolation::head(Path::new(&ws.path)).map_err(|e| e.to_string())?,
    };
    let steps = new
        .steps
        .iter()
        .map(|s| Step {
            model: s.model.as_deref().map(str::trim).filter(|m| !m.is_empty()).map(String::from),
            ..s.clone()
        })
        .collect();
    let policy = Policy {
        steps,
        checks: ws
            .checks
            .iter()
            .filter(|c| c.approved)
            .map(|c| PolicyCheck { command: c.command.clone(), source: c.source.clone() })
            .collect(),
        allow_shell: new.allow_shell,
        timeout_secs: new.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS).clamp(60, 3600),
        handoff: handoff.into(),
        answer: new.answer,
    };
    let title = new
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(String::from)
        .unwrap_or_else(|| title_of(prompt));
    let id = new_id();
    let now = now_ms();
    let tx = conn.unchecked_transaction().map_err(err)?;
    tx.execute("INSERT INTO tasks (id, workspace_id, title, prompt, strategy, state, base_revision, policy_snapshot_json, \
                created_at, updated_at, phase, client_id) VALUES (?1, ?2, ?3, ?4, ?5, 'running', ?6, ?7, ?8, ?8, 'preparing', ?9)",
        params![id, ws.id, title, prompt, new.strategy, base, serde_json::to_string(&policy).map_err(|e| e.to_string())?, now, client_id])
        .map_err(err)?;
    tx.execute("INSERT INTO task_stages (id, task_id, position, name, check_spec, state) VALUES (?1, ?2, 0, 'change', ?3, 'running')",
        params![new_id(), id, serde_json::to_string(&policy.checks).map_err(|e| e.to_string())?]).map_err(err)?;
    audit(&tx, "task.created", &id, serde_json::json!({"steps": policy.steps.len(), "checks": policy.checks.len()}))
        .map_err(err)?;
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
    /// The brainstorm and card it was made from; `None` for a solo task.
    pub brainstorm_id: Option<String>,
    pub card_id: Option<String>,
    /// `kept` or `discarded` once decided, and the branch a kept result is on.
    pub result: Option<String>,
    pub result_branch: Option<String>,
    /// The gateway client that started it (an IDE agent through the MCP server); `None`: the app.
    pub source: Option<String>,
    /// An answer task (Ask): a question about the code, nothing changed.
    pub answer: bool,
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
    /// The step's cost as Pygg knows it (`cost_source` says from where), when it is known.
    pub agent_estimate_usd: Option<f64>,
    /// `pygg`: Pygg's ledger, for requests the agent sent through Pygg's gateway; `agent`: the agent's own
    /// list-price estimate; None: recorded before v19, the agent's estimate.
    pub cost_source: Option<String>,
    /// Requests this step sent through Pygg's gateway that had no price (ADR-011: unknown, never $0).
    pub unpriced_requests: u32,
    /// The cap the user approved, for a paid step.
    pub cap_usd: Option<f64>,
    /// What was added to this attempt's prompt about earlier attempts (H1); None: the task alone.
    pub handoff_sent: Option<String>,
    /// Paths this attempt changed, recorded before a discarded worktree is removed.
    pub changed_paths: Vec<String>,
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
    /// The paid step waiting for the user's decision, if any.
    pub approval: Option<ApprovalView>,
}

/// A pending approval as the user sees it: what would run, what it cost before, and a cap to start from.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalView {
    pub step: usize,
    /// The step spends (a cap is needed); otherwise it asks only because the workspace is sensitive.
    pub paid: bool,
    pub label: String,
    pub estimate: Estimate,
    pub suggested_cap_usd: f64,
    /// The agent stops at the cap itself (Claude Code); otherwise it is checked against the cost reported afterwards.
    pub enforced_by_agent: bool,
    /// What today's budgets leave for paid steps in this workspace; `None` without a budget. The cap cannot be more.
    pub remaining_usd: Option<f64>,
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
    brainstorm_id: Option<String>,
    card_id: Option<String>,
}

fn task_row(conn: &Connection, id: &str) -> Result<TaskRow> {
    let row = conn.query_row(
        "SELECT t.title, t.workspace_id, w.name, w.path, t.prompt, t.strategy, t.state, t.phase, t.reason, t.result, \
                t.result_branch, t.base_revision, t.policy_snapshot_json, s.id, t.created_at, t.updated_at, t.client_id, c.name, t.brainstorm_id, t.card_id \
         FROM tasks t JOIN workspaces w ON w.id = t.workspace_id JOIN task_stages s ON s.task_id = t.id AND s.position = 0 \
         LEFT JOIN gateway_clients c ON c.id = t.client_id WHERE t.id = ?1", [id],
        |r| Ok((TaskRow {
            title: r.get(0)?, workspace_id: r.get(1)?, workspace_name: r.get(2)?, workspace_path: r.get(3)?, prompt: r.get(4)?,
            strategy: r.get(5)?, state: r.get(6)?, phase: r.get(7)?, reason: r.get(8)?, result: r.get(9)?,
            result_branch: r.get(10)?, base_revision: r.get(11)?,
            policy: Policy { steps: vec![], checks: vec![], allow_shell: false, timeout_secs: DEFAULT_TIMEOUT_SECS, handoff: default_handoff(), answer: false },
            stage_id: r.get(13)?, created_at: r.get(14)?, updated_at: r.get(15)?, client_id: r.get(16)?, client_name: r.get(17)?,
            brainstorm_id: r.get(18)?, card_id: r.get(19)?,
        }, r.get::<_, String>(12)?)),
    ).optional().map_err(err)?;
    let (mut row, policy) = row.ok_or(m::TASK_GONE)?;
    row.policy = serde_json::from_str(&policy).map_err(|e| format!("The task's policy could not be read: {e}"))?;
    Ok(row)
}

fn attempts_of(conn: &Connection, task_id: &str) -> Result<Vec<AttemptView>> {
    let mut statement = conn.prepare(
        "SELECT a.id, a.number, a.rung, a.agent, a.model_ref, a.agent_model, a.tier, a.billing, a.state, a.detail, \
                a.escalation_reason, a.check_results_json, a.files_changed, a.insertions, a.deletions, a.summary, a.log_tail, \
                (SELECT SUM(input_tokens) FROM usage_events u WHERE u.attempt_id = a.id), \
                (SELECT SUM(output_tokens) FROM usage_events u WHERE u.attempt_id = a.id), \
                a.started_at, a.finished_at, a.worktree_path, a.base_revision, a.agent_estimate_micro_usd, \
                (SELECT cap_micro_usd FROM approvals p WHERE p.attempt_id = a.id AND p.state = 'approved' ORDER BY p.rowid DESC LIMIT 1), \
                a.handoff_sent, a.changed_paths_json, a.cost_source, \
                (SELECT COUNT(*) FROM usage_events u WHERE u.attempt_id = a.id AND u.cost_status = 'unknown' \
                 AND COALESCE(u.route_mode, '') <> 'agent') \
         FROM attempts a JOIN task_stages s ON s.id = a.stage_id WHERE s.task_id = ?1 ORDER BY a.number").map_err(err)?;
    let rows = statement
        .query_map([task_id], |r| {
            let checks: Option<String> = r.get(11)?;
            let worktree: Option<String> = r.get(21)?;
            Ok(AttemptView {
                id: r.get(0)?,
                number: r.get(1)?,
                step: r.get::<_, i64>(2)? as usize,
                agent: r.get(3)?,
                label: r.get(4)?,
                agent_model: r.get(5)?,
                tier: r.get(6)?,
                billing: r.get(7)?,
                outcome: r.get(8)?,
                detail: r.get(9)?,
                escalation_reason: r.get(10)?,
                checks: checks.and_then(|c| serde_json::from_str(&c).ok()).unwrap_or_default(),
                files_changed: r.get(12)?,
                insertions: r.get(13)?,
                deletions: r.get(14)?,
                summary: r.get(15)?,
                log_tail: r.get(16)?,
                input_tokens: r.get(17)?,
                output_tokens: r.get(18)?,
                started_at: r.get(19)?,
                finished_at: r.get(20)?,
                open: worktree.is_some(),
                worktree,
                base: r.get(22)?,
                agent_estimate_usd: r.get::<_, Option<i64>>(23)?.map(|m| m as f64 / 1e6),
                cap_usd: r.get::<_, Option<i64>>(24)?.filter(|c| *c > 0).map(|c| c as f64 / 1e6),
                handoff_sent: r.get(25)?,
                changed_paths: r
                    .get::<_, Option<String>>(26)?
                    .and_then(|j| serde_json::from_str(&j).ok())
                    .unwrap_or_default(),
                cost_source: r.get(27)?,
                unpriced_requests: r.get(28)?,
            })
        })
        .map_err(err)?;
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
    let rows = statement
        .query_map([task_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, u32>(2)?)))
        .map_err(err)?;
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
        id: id.into(),
        title: row.title.clone(),
        workspace_id: row.workspace_id.clone(),
        state: row.state.clone(),
        phase: row.phase.clone(),
        attempts: attempts
            .iter()
            .map(|a| AttemptChip {
                tier: a.tier,
                billing: a.billing.clone(),
                model: a.label.clone(),
                outcome: a.outcome.clone(),
            })
            .collect(),
        cost: cost_of(conn, id, attempts)?,
        started_at: row.created_at,
        updated_at: row.updated_at,
        source: row.client_name.clone(),
        brainstorm_id: row.brainstorm_id.clone(),
        card_id: row.card_id.clone(),
        result: row.result.clone(),
        result_branch: row.result_branch.clone(),
        answer: row.policy.answer,
    })
}

pub fn list_tasks(conn: &Connection) -> Result<Vec<TaskSummary>> {
    let ids: Vec<String> = conn
        .prepare("SELECT id FROM tasks ORDER BY updated_at DESC LIMIT 500")
        .map_err(err)?
        .query_map([], |r| r.get(0))
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;
    ids.iter()
        .map(|id| {
            let row = task_row(conn, id)?;
            let attempts = attempts_of(conn, id)?;
            summary_of(conn, id, &row, &attempts)
        })
        .collect()
}

/// One task's summary, as the list shows it (for a board card's task, board.rs).
pub fn task_summary(conn: &Connection, id: &str) -> Result<TaskSummary> {
    let row = task_row(conn, id)?;
    let attempts = attempts_of(conn, id)?;
    summary_of(conn, id, &row, &attempts)
}

pub fn task_detail(conn: &Connection, id: &str) -> Result<TaskDetail> {
    task_detail_for(conn, id, None)
}

/// A client sees only the tasks it started; `None`: the app, which sees every task.
pub fn task_detail_for(conn: &Connection, id: &str, client_id: Option<&str>) -> Result<TaskDetail> {
    let row = task_row(conn, id)?;
    if client_id.is_some() && row.client_id.as_deref() != client_id {
        return Err(m::TASK_GONE.into());
    }
    let attempts = attempts_of(conn, id)?;
    let summary = summary_of(conn, id, &row, &attempts)?;
    let approval = approval_of(conn, &row, &attempts)?;
    Ok(TaskDetail {
        summary,
        prompt: row.prompt,
        strategy: row.strategy,
        reason: row.reason,
        result: row.result,
        result_branch: row.result_branch,
        base_revision: row.base_revision,
        workspace_name: row.workspace_name,
        approval,
        workspace_path: row.workspace_path,
        policy: row.policy,
        attempt_details: attempts,
    })
}

fn approval_of(conn: &Connection, row: &TaskRow, attempts: &[AttemptView]) -> Result<Option<ApprovalView>> {
    if row.state != "waiting_approval" {
        return Ok(None);
    }
    let Some(waiting) = attempts.iter().rev().find(|a| a.outcome == "waiting") else { return Ok(None) };
    let Some(step) = row.policy.steps.get(waiting.step) else { return Ok(None) };
    let estimate: Option<(Option<i64>, Option<i64>, String)> = conn.query_row(
        "SELECT estimate_low_micro, estimate_high_micro, policy_version FROM approvals WHERE attempt_id = ?1 AND state = 'pending' \
         ORDER BY rowid DESC LIMIT 1", [&waiting.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional().map_err(err)?;
    let Some((low, high, asked_on)) = estimate else { return Ok(None) };
    // The number of runs the estimate was made from, as recorded in its basis.
    let runs = asked_on.split('|').nth(5).and_then(|n| n.parse().ok()).unwrap_or(0);
    let estimate = Estimate { runs, low_micro_usd: low.map(|v| v as u64), high_micro_usd: high.map(|v| v as u64) };
    let left = remaining(conn, &row.workspace_id).map_err(err)?;
    let suggested = match left {
        Some(left) if left > 0 => suggested_cap_micro(&estimate).min(left),
        _ => suggested_cap_micro(&estimate),
    };
    Ok(Some(ApprovalView {
        step: waiting.step,
        paid: step.billing == "paid",
        label: waiting.label.clone(),
        suggested_cap_usd: usd(suggested),
        enforced_by_agent: agents::enforces_budget(&step.agent),
        remaining_usd: left.map(usd),
        estimate,
    }))
}

// ---------- State changes ----------

fn set_task(
    conn: &Connection, id: &str, state: &str, phase: Option<&str>, reason: Option<&str>,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE tasks SET state = ?2, phase = ?3, reason = ?4, updated_at = ?5 WHERE id = ?1",
        params![id, state, phase, reason, now_ms()],
    )?;
    conn.execute("UPDATE task_stages SET state = ?2 WHERE task_id = ?1", params![id, state])?;
    Ok(())
}

fn set_phase(conn: &Connection, id: &str, phase: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE tasks SET phase = ?2, updated_at = ?3 WHERE id = ?1 AND state = 'running'",
        params![id, phase, now_ms()],
    )
    .map(|_| ())
}

fn step_label(step: &Step) -> String {
    let name = agents::find(&step.agent).map(|a| a.name).unwrap_or(step.agent.as_str());
    fill(m::LABEL, &[&name, &step.model.as_deref().unwrap_or(m::DEFAULT_MODEL)])
}

enum Opened {
    Run(String),
    /// A paid step: the task now waits for the user's approval.
    Waiting,
    Declined,
}

fn open_attempt(
    conn: &Connection, task_id: &str, stage_id: &str, index: usize, step: &Step,
) -> rusqlite::Result<Opened> {
    let now = now_ms();
    let paid = step.billing == "paid";
    let workspace = task_workspace(conn, task_id)?;
    // A paid step asks because it spends; in a sensitive workspace every step asks, because Pygg cannot yet check
    // where the agent sends the code.
    let sensitive = is_sensitive(conn, &workspace)?;
    let asks = paid || sensitive;
    if asks {
        let waiting: Option<String> = conn.query_row(
            "SELECT id FROM attempts WHERE stage_id = ?1 AND rung = ?2 AND state = 'waiting' ORDER BY number DESC LIMIT 1",
            params![stage_id, index as i64], |r| r.get(0)).optional()?;
        if let Some(attempt) = waiting {
            // The latest approval by insertion order (rowid), not by `created_at`: an approval asked again after an
            // invalidation can share the millisecond of the one it replaced, and the older, invalidated one would
            // then read as this attempt's decision.
            let decision: Option<String> = conn
                .query_row(
                    "SELECT state FROM approvals WHERE attempt_id = ?1 ORDER BY rowid DESC LIMIT 1",
                    [&attempt],
                    |r| r.get(0),
                )
                .optional()?;
            return match decision.as_deref() {
                Some("approved") => {
                    conn.execute(
                        "UPDATE attempts SET state = 'running', started_at = ?2 WHERE id = ?1",
                        params![attempt, now],
                    )?;
                    Ok(Opened::Run(attempt))
                }
                Some("pending") | None => {
                    set_task(conn, task_id, "waiting_approval", None, Some(&ask_reason(index, step, paid)))?;
                    Ok(Opened::Waiting)
                }
                Some(_) => Ok(Opened::Declined),
            };
        }
    }
    if paid {
        if let Some(0) = remaining(conn, &workspace)? {
            set_task(conn, task_id, "blocked", None, Some(&fill(m::BUDGET_USED, &[&(index + 1), &step_label(step)])))?;
            return Ok(Opened::Declined);
        }
    }
    let number: u32 =
        conn.query_row("SELECT COALESCE(MAX(number), 0) + 1 FROM attempts WHERE stage_id = ?1", [stage_id], |r| {
            r.get(0)
        })?;
    let id = new_id();
    conn.execute(
        "INSERT INTO attempts (id, stage_id, number, agent, model_ref, tier, billing, state, started_at, rung) \
                  VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            id,
            stage_id,
            number,
            step.agent,
            step_label(step),
            step.tier,
            step.billing,
            if asks { "waiting" } else { "running" },
            if asks { None } else { Some(now) },
            index as i64
        ],
    )?;
    if asks {
        ask_approval(conn, &id, step, &workspace)?;
        set_task(conn, task_id, "waiting_approval", None, Some(&ask_reason(index, step, paid)))?;
        return Ok(Opened::Waiting);
    }
    Ok(Opened::Run(id))
}

fn ask_reason(index: usize, step: &Step, paid: bool) -> String {
    if paid {
        fill(m::WAITS_PAID, &[&(index + 1), &step_label(step)])
    } else {
        fill(m::WAITS_SENSITIVE, &[&(index + 1), &step_label(step)])
    }
}
/// What earlier runs of an agent and model cost by the agent's own report (its list-price estimate): the basis of
/// an approval's estimate. Agents report a cost only after a run, so there is nothing better before one.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Estimate {
    pub runs: u32,
    pub low_micro_usd: Option<u64>,
    pub high_micro_usd: Option<u64>,
}

/// How many recent runs an estimate looks at.
const ESTIMATE_RUNS: i64 = 10;

fn estimate_for(conn: &Connection, step: &Step) -> rusqlite::Result<Estimate> {
    let mut statement = conn.prepare(
        "SELECT agent_estimate_micro_usd FROM attempts WHERE agent = ?1 AND model_ref = ?2 \
         AND agent_estimate_micro_usd IS NOT NULL ORDER BY finished_at DESC LIMIT ?3",
    )?;
    let values: Vec<i64> = statement
        .query_map(params![step.agent, step_label(step), ESTIMATE_RUNS], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Estimate {
        runs: values.len() as u32,
        low_micro_usd: values.iter().min().map(|v| (*v).max(0) as u64),
        high_micro_usd: values.iter().max().map(|v| (*v).max(0) as u64),
    })
}

/// What an approval was asked on (`approvals.policy_version`): the step and its estimate. When it is different at
/// the moment the user decides, the approval no longer describes what would run (ADR-010).
fn basis(step: &Step, estimate: &Estimate, limits: &Limits) -> String {
    format!(
        "tasks-v3|{}|{}|T{}|{}|{}|{:?}|{:?}|{:?}|{:?}",
        step.agent,
        step.model.as_deref().unwrap_or(""),
        step.tier,
        step.billing,
        estimate.runs,
        estimate.low_micro_usd,
        estimate.high_micro_usd,
        limits.daily,
        limits.workspace
    )
}

/// A pending approval for one attempt, with the estimate and the budgets as they are now. The cap is the user's,
/// set when approving.
fn ask_approval(conn: &Connection, attempt: &str, step: &Step, workspace_id: &str) -> rusqlite::Result<()> {
    let estimate = estimate_for(conn, step)?;
    let limits = limits(conn, workspace_id)?;
    conn.execute("INSERT INTO approvals (id, attempt_id, cap_micro_usd, estimate_low_micro, estimate_high_micro, policy_version, \
                  state, created_at) VALUES (?1, ?2, 0, ?3, ?4, ?5, 'pending', ?6)",
        params![new_id(), attempt, estimate.low_micro_usd.map(|v| v as i64), estimate.high_micro_usd.map(|v| v as i64),
            basis(step, &estimate, &limits), now_ms()]).map(|_| ())
}

/// The cap the user approved for an attempt.
fn approved_cap(conn: &Connection, attempt: &str) -> rusqlite::Result<Option<u64>> {
    conn.query_row(
        "SELECT cap_micro_usd FROM approvals WHERE attempt_id = ?1 AND state = 'approved' ORDER BY rowid DESC LIMIT 1",
        [attempt],
        |r| r.get::<_, i64>(0),
    )
    .optional()
    .map(|v| v.filter(|c| *c > 0).map(|c| c as u64))
}

/// A cap to offer: half again the highest recent cost, rounded up to 5 cents; 50 cents with no history.
fn suggested_cap_micro(estimate: &Estimate) -> u64 {
    match estimate.high_micro_usd {
        Some(high) => (high * 3 / 2).div_ceil(50_000).max(1) * 50_000,
        None => 500_000,
    }
}

fn usd(micro: u64) -> f64 {
    micro as f64 / 1e6
}

// ---------- Budgets (ROADMAP A3) ----------

/// The daily limit for paid steps across all tasks, in micro-USD (settings).
pub const DAILY_BUDGET_KEY: &str = "tasks.daily_budget_micro_usd";

/// Midnight UTC of the day `now` is in; budgets reset then, like the clients' monthly limits.
fn day_start(now: i64) -> i64 {
    now - now.rem_euclid(86_400_000)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Limits {
    daily: Option<u64>,
    workspace: Option<u64>,
}

fn limits(conn: &Connection, workspace_id: &str) -> rusqlite::Result<Limits> {
    let daily = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [DAILY_BUDGET_KEY], |r| r.get::<_, String>(0))
        .optional()?
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v > 0);
    let workspace = conn
        .query_row("SELECT daily_budget_micro_usd FROM workspaces WHERE id = ?1", [workspace_id], |r| {
            r.get::<_, Option<i64>>(0)
        })
        .optional()?
        .flatten()
        .filter(|v| *v > 0)
        .map(|v| v as u64);
    Ok(Limits { daily, workspace })
}

/// Today's paid steps, everywhere or in one workspace: (spent, held for steps still running, runs that reported no
/// cost). A finished step counts at the agent's own estimate, or at its approved cap when it reported none, since
/// it may have cost that much; a running one holds its cap. Free, local and subscription steps are not billed per
/// request and do not count (ADR-011).
fn used_today(conn: &Connection, workspace_id: Option<&str>) -> rusqlite::Result<(u64, u64, u32)> {
    conn.query_row(
        "SELECT COALESCE(SUM(CASE WHEN a.state = 'running' THEN 0 ELSE COALESCE(a.agent_estimate_micro_usd, p.cap_micro_usd, 0) END), 0), \
                COALESCE(SUM(CASE WHEN a.state = 'running' THEN COALESCE(p.cap_micro_usd, 0) ELSE 0 END), 0), \
                COALESCE(SUM(CASE WHEN a.state <> 'running' AND a.agent_estimate_micro_usd IS NULL THEN 1 ELSE 0 END), 0) \
         FROM attempts a JOIN task_stages s ON s.id = a.stage_id JOIN tasks t ON t.id = s.task_id \
         LEFT JOIN approvals p ON p.attempt_id = a.id AND p.state = 'approved' \
         WHERE a.billing = 'paid' AND a.started_at IS NOT NULL AND a.started_at >= ?1 AND (?2 IS NULL OR t.workspace_id = ?2)",
        params![day_start(now_ms()), workspace_id],
        |r| Ok((r.get::<_, i64>(0)?.max(0) as u64, r.get::<_, i64>(1)?.max(0) as u64, r.get::<_, i64>(2)?.max(0) as u32)))
}

/// What is left today for paid steps in a workspace: the smaller of what the daily and the workspace's limits leave;
/// `None` without either limit.
fn remaining(conn: &Connection, workspace_id: &str) -> rusqlite::Result<Option<u64>> {
    let l = limits(conn, workspace_id)?;
    let mut left: Option<u64> = None;
    if let Some(daily) = l.daily {
        let (spent, held, _) = used_today(conn, None)?;
        left = Some(daily.saturating_sub(spent + held));
    }
    if let Some(limit) = l.workspace {
        let (spent, held, _) = used_today(conn, Some(workspace_id))?;
        let here = limit.saturating_sub(spent + held);
        left = Some(left.map_or(here, |l| l.min(here)));
    }
    Ok(left)
}

fn task_workspace(conn: &Connection, task_id: &str) -> rusqlite::Result<String> {
    conn.query_row("SELECT workspace_id FROM tasks WHERE id = ?1", [task_id], |r| r.get(0))
}

/// Today's paid steps for the toolbar (SCREENS.md §2.3): spend apart from what running steps hold.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DayBudget {
    pub recorded_usd: f64,
    pub reserved_usd: f64,
    pub limit_usd: Option<f64>,
    /// Paid runs today that reported no cost, counted at their cap.
    pub unknown_calls: u32,
}

pub fn day_budget(conn: &Connection) -> Result<DayBudget> {
    let (spent, held, unknown) = used_today(conn, None).map_err(err)?;
    let limit = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [DAILY_BUDGET_KEY], |r| r.get::<_, String>(0))
        .optional()
        .map_err(err)?
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v > 0);
    Ok(DayBudget {
        recorded_usd: usd(spent),
        reserved_usd: usd(held),
        limit_usd: limit.map(usd),
        unknown_calls: unknown,
    })
}

/// Sets (or, with `None`, removes) the daily limit for paid steps. Pending approvals then no longer match and are
/// asked again when the user decides (ADR-010).
pub fn set_daily_budget(conn: &Connection, micro_usd: Option<u64>) -> Result<()> {
    match micro_usd.filter(|v| *v > 0) {
        Some(v) => conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![DAILY_BUDGET_KEY, v.to_string()],
        ),
        None => conn.execute("DELETE FROM settings WHERE key = ?1", [DAILY_BUDGET_KEY]),
    }
    .map_err(err)?;
    audit(conn, "budget.daily", "tasks", serde_json::json!({"microUsd": micro_usd})).map_err(err)
}

pub fn set_workspace_budget(conn: &Connection, id: &str, micro_usd: Option<u64>) -> Result<()> {
    let changed = conn
        .execute(
            "UPDATE workspaces SET daily_budget_micro_usd = ?2 WHERE id = ?1",
            params![id, micro_usd.filter(|v| *v > 0).map(|v| v as i64)],
        )
        .map_err(err)?;
    if changed == 0 {
        return Err(m::WORKSPACE_GONE.into());
    }
    audit(conn, "budget.workspace", id, serde_json::json!({"microUsd": micro_usd})).map_err(err)
}

/// Records one quality signal (ROADMAP C4, ADR-029; migration 0015). Only known signals are written.
fn signal(
    conn: &Connection, metric: &str, kind: &str, target: &str, value: bool, source: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO feedback (id, metric_name, target_kind, target_id, value_bool, source, created_at) \
                  VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![new_id(), metric, kind, target, value as i64, source, now_ms()],
    )
    .map(|_| ())
}

/// How tasks turned out, and the money they cost per result the user kept (ROADMAP C4): every paid step of every
/// task counts, kept or not, so a cheap step that failed is in the cost of the result that followed.
#[derive(Clone, Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Quality {
    pub tasks: u64,
    /// Results kept as a branch or a patch.
    pub kept: u64,
    pub discarded: u64,
    pub retried: u64,
    pub cancelled: u64,
    /// Attempts whose approved checks ran, and how many of them passed.
    pub checked_attempts: u64,
    pub passed_attempts: u64,
    /// Known money of all tasks' ledger entries, and the entries whose cost is unknown (not counted).
    pub known_micro_usd: u64,
    pub unknown_entries: u64,
    /// Known money per kept result; `None` when nothing was kept.
    pub micro_usd_per_kept: Option<u64>,
}

pub fn quality(conn: &Connection, since_ms: i64) -> Result<Quality> {
    let count = |metric: &str, value: i64| -> Result<u64> {
        conn.query_row("SELECT COUNT(DISTINCT target_id) FROM feedback WHERE metric_name = ?1 AND value_bool = ?2 AND created_at >= ?3",
            params![metric, value, since_ms], |r| r.get::<_, i64>(0)).map(|n| n.max(0) as u64).map_err(err)
    };
    let tasks: i64 =
        conn.query_row("SELECT COUNT(*) FROM tasks WHERE created_at >= ?1", [since_ms], |r| r.get(0)).map_err(err)?;
    let (known, unknown): (i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(u.cost_micro_usd), 0), COALESCE(SUM(u.cost_status = 'unknown'), 0) FROM usage_events u \
         JOIN attempts a ON a.id = u.attempt_id JOIN task_stages s ON s.id = a.stage_id JOIN tasks t ON t.id = s.task_id \
         WHERE t.created_at >= ?1", [since_ms], |r| Ok((r.get(0)?, r.get(1)?))).map_err(err)?;
    let kept = count("result_kept", 1)?;
    let mut q = Quality {
        tasks: tasks.max(0) as u64,
        kept,
        discarded: count("result_kept", 0)?,
        retried: count("retried", 1)?,
        cancelled: count("cancelled", 1)?,
        passed_attempts: count("checks_passed", 1)?,
        known_micro_usd: known.max(0) as u64,
        unknown_entries: unknown.max(0) as u64,
        ..Quality::default()
    };
    q.checked_attempts = q.passed_attempts + count("checks_passed", 0)?;
    q.micro_usd_per_kept = (kept > 0).then(|| q.known_micro_usd / kept);
    Ok(q)
}

fn close_attempt(conn: &Connection, id: &str, outcome: &str, escalation: Option<&str>) -> rusqlite::Result<()> {
    conn.execute("UPDATE attempts SET state = ?2, escalation_reason = COALESCE(?3, escalation_reason), finished_at = ?4 WHERE id = ?1",
        params![id, outcome, escalation, now_ms()]).map(|_| ())
}

fn save_run(conn: &Connection, id: &str, run: &TaskRun) -> rusqlite::Result<()> {
    let detail = run.usage.error.clone().filter(|_| run.outcome != "ok").unwrap_or_else(|| run.exit.clone());
    conn.execute("UPDATE attempts SET worktree_path = ?2, base_revision = ?3, agent_model = ?4, detail = ?5, files_changed = ?6, \
                  insertions = ?7, deletions = ?8, summary = ?9, log_tail = ?10, agent_estimate_micro_usd = ?11 WHERE id = ?1",
        params![id, run.worktree, run.base, run.usage.model, detail, run.files_changed as i64, run.insertions as i64,
            run.deletions as i64, run.usage.result, run.log_tail,
            run.usage.estimate_usd.map(|usd| (usd * 1e6).round().max(0.0) as i64)])?;
    conn.execute(
        "UPDATE attempts SET cost_source = 'agent' WHERE id = ?1 AND agent_estimate_micro_usd IS NOT NULL",
        [id],
    )
    .map(|_| ())
}

/// What a step's requests through Pygg's own gateway cost. The agent sends its attempt id as its session header
/// (`PYGG_ATTEMPT_ID`, docs/AGENTS.md), so the gateway's entries made since the attempt started carry it; they are
/// tied to the attempt here. Those entries are then the step's cost, and the agent's own entry for the run (its
/// tokens, cost unknown) is taken off the attempt so nothing is counted twice; it stays in the ledger. Returns None
/// when no request went through Pygg, else (the priced entries' sum, how many had no price).
fn link_gateway_usage(conn: &Connection, attempt: &str) -> rusqlite::Result<Option<(u64, u32)>> {
    let started: i64 =
        conn.query_row("SELECT COALESCE(started_at, 0) FROM attempts WHERE id = ?1", [attempt], |r| r.get(0))?;
    conn.execute(
        "UPDATE usage_events SET attempt_id = ?1 WHERE attempt_id IS NULL AND session_key = ?1 \
                  AND created_at >= ?2 AND COALESCE(route_mode, '') <> 'agent'",
        params![attempt, started],
    )?;
    let (count, known, unknown): (i64, i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(CASE WHEN cost_status <> 'unknown' THEN cost_micro_usd END), 0), \
                COALESCE(SUM(CASE WHEN cost_status = 'unknown' THEN 1 ELSE 0 END), 0) \
         FROM usage_events WHERE attempt_id = ?1 AND COALESCE(route_mode, '') <> 'agent'",
        [attempt],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    if count == 0 {
        return Ok(None);
    }
    conn.execute(
        "UPDATE usage_events SET attempt_id = NULL, session_key = ?1 WHERE attempt_id = ?1 AND route_mode = 'agent'",
        [attempt],
    )?;
    let cost = (unknown == 0).then_some(known.max(0));
    conn.execute(
        "UPDATE attempts SET agent_estimate_micro_usd = ?2, cost_source = 'pygg' WHERE id = ?1",
        params![attempt, cost],
    )?;
    Ok(Some((known.max(0) as u64, unknown as u32)))
}

fn forget_worktree(conn: &Connection, attempt: &str) -> rusqlite::Result<()> {
    conn.execute("UPDATE attempts SET worktree_path = NULL WHERE id = ?1", [attempt]).map(|_| ())
}

/// What the next step is told about a failed check.
fn handoff(check: &CheckResult) -> String {
    format!(
        "The check `{}` {} on that attempt's changes. The end of its output:\n{}",
        check.command,
        check.detail,
        last_chars(&check.output_tail, HANDOFF_CHARS)
    )
}

fn handoff_of(attempt: &AttemptView) -> Option<String> {
    if attempt.detail.as_deref() == Some(NO_CHANGES) {
        return Some(NO_CHANGES_HANDOFF.into());
    }
    attempt.checks.iter().find(|c| !c.passed).map(handoff)
}

/// H1's evidence block (handoff.rs) from a task's recorded attempts.
fn evidence_of(attempts: &[AttemptView]) -> Option<String> {
    let rows: Vec<crate::handoff::Attempt> = attempts
        .iter()
        .map(|a| crate::handoff::Attempt {
            number: a.number,
            label: &a.label,
            outcome: &a.outcome,
            detail: a.detail.as_deref(),
            escalation_reason: a.escalation_reason.as_deref(),
            failed_check: a
                .checks
                .iter()
                .find(|c| !c.passed)
                .map(|c| (c.command.as_str(), c.detail.as_str(), c.output_tail.as_str())),
            summary: a.summary.as_deref(),
            changed_paths: a.changed_paths.clone(),
            no_changes: a.detail.as_deref() == Some(NO_CHANGES),
        })
        .collect();
    crate::handoff::evidence(&rows)
}

/// What an attempt is told about earlier ones, by the task's handoff mode: nothing, the last failure's tail
/// (`carried`, today's behaviour), or H1's evidence from every earlier attempt.
fn handoff_for(mode: &str, carried: Option<&str>, attempts: &[AttemptView]) -> Option<String> {
    match mode {
        "none" => None,
        "evidence" => evidence_of(attempts).map(|e| {
            format!(
                "{e}\n\nYou start again from the original code. Use what was \
            verified; treat the agents' own accounts as unconfirmed."
            )
        }),
        _ => carried.map(previous_attempt),
    }
}

/// The prompt for an attempt: the task, then what it is told about earlier attempts, if anything.
fn compose_with(prompt: &str, handoff: Option<&str>) -> String {
    match handoff {
        None => prompt.to_string(),
        Some(text) => format!("{prompt}\n\n---\n{text}"),
    }
}

/// The prompt for an attempt: the task, plus why the previous attempt failed.
pub fn compose(prompt: &str, handoff: Option<&str>) -> String {
    match handoff {
        None => prompt.to_string(),
        Some(why) => format!("{prompt}\n\n---\n{}", previous_attempt(why)),
    }
}

/// What the next attempt is told about the last one: it failed its checks, or it changed nothing; never a check
/// failure that did not happen (the owner's run: "did not pass the checks" for an attempt that made no change).
fn previous_attempt(why: &str) -> String {
    let what =
        if why == NO_CHANGES_HANDOFF { "did not change any file" } else { "did not pass the workspace's checks" };
    format!("A previous attempt at this task, started from the same code, {what}. You start again from the original code.\n{why}")
}

/// Why a step cannot run now: its model goes through Pygg's gateway (`pygg/<connection>/<model>`) and that connection
/// is held for a reason that waiting does not fix (no credit, a rejected key), so the gateway would refuse it.
fn held_account(conn: &Connection, step: &Step) -> rusqlite::Result<Option<String>> {
    let Some(model) = step.model.as_deref().and_then(|m| m.strip_prefix("pygg/")) else { return Ok(None) };
    let Some((provider, model_id)) = model.split_once('/') else { return Ok(None) };
    let health = crate::gateway::store::blocking_health(conn, provider, model_id)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e))))?;
    Ok(health
        .filter(|h| !crate::gateway::policy::lifts_by_waiting(&h.state))
        .map(|h| fill(m::ACCOUNT_HELD, &[&model, &h.state.replace('_', " "), &h.reason])))
}

/// How recently a step must have timed out, twice in a row without answering, to be skipped.
const UNRESPONSIVE_WINDOW_MS: i64 = 30 * 60_000;

/// Why a step should be skipped: its agent and model timed out on their last two runs, both within
/// `UNRESPONSIVE_WINDOW_MS`, without a single output token, so the service is most likely not answering.
/// Waiting out its time limit again would only delay the next step (found by the benchmark, ROADMAP N2).
fn unresponsive(conn: &Connection, step: &Step) -> rusqlite::Result<Option<String>> {
    let since = now_ms() - UNRESPONSIVE_WINDOW_MS;
    let mut statement = conn.prepare(
        "SELECT a.detail, a.finished_at, (SELECT SUM(output_tokens) FROM usage_events u WHERE u.attempt_id = a.id) \
         FROM attempts a WHERE a.agent = ?1 AND a.model_ref = ?2 AND a.state IN ('failed', 'passed', 'done') \
         ORDER BY a.finished_at DESC LIMIT 2",
    )?;
    let last: Vec<(Option<String>, Option<i64>, Option<i64>)> = statement
        .query_map(params![step.agent, step_label(step)], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let silent_timeout = |(detail, finished, output): &(Option<String>, Option<i64>, Option<i64>)| {
        detail.as_deref() == Some(m::TIMED_OUT) && finished.is_some_and(|f| f >= since) && output.unwrap_or(0) == 0
    };
    Ok((last.len() == 2 && last.iter().all(silent_timeout)).then(|| fill(m::SKIPPED, &[&step_label(step)])))
}

/// Records a step that was not run, so the task's history shows why the ladder moved on.
fn skip_attempt(
    conn: &Connection, task_id: &str, stage_id: &str, index: usize, step: &Step, why: &str,
) -> rusqlite::Result<()> {
    let now = now_ms();
    let number: u32 =
        conn.query_row("SELECT COALESCE(MAX(number), 0) + 1 FROM attempts WHERE stage_id = ?1", [stage_id], |r| {
            r.get(0)
        })?;
    conn.execute("INSERT INTO attempts (id, stage_id, number, agent, model_ref, tier, billing, state, started_at, finished_at, \
                  escalation_reason, detail, rung) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'skipped', ?8, ?8, ?9, 'skipped', ?10)",
        params![new_id(), stage_id, number, step.agent, step_label(step), step.tier, step.billing, now, why, index as i64])?;
    audit(conn, "attempt.skipped", task_id, serde_json::json!({"step": index, "reason": why}))
}

/// Where a task continues: the step after the last failure, or the same step after a stop.
fn resume_point(attempts: &[AttemptView]) -> Option<(usize, Option<String>)> {
    let last_failed = attempts.iter().rev().find(|a| a.outcome == "failed");
    let carried = last_failed.and_then(handoff_of);
    match attempts.last() {
        None => Some((0, None)),
        Some(a) if a.outcome == "failed" || a.outcome == "skipped" => Some((a.step + 1, carried)),
        Some(a) if a.outcome == "passed" || a.outcome == "done" => None,
        Some(a) => Some((a.step, carried)),
    }
}

// ---------- Running ----------

pub type Cancel = watch::Receiver<bool>;

/// Resolves once the task is stopped; never, if whoever could stop it is gone.
pub async fn canceled(mut rx: Cancel) {
    if rx.wait_for(|stopped| *stopped).await.is_err() {
        std::future::pending::<()>().await;
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
    /// The cap the user approved for a paid step; the agent enforces it where it can (`agents::enforces_budget`).
    pub max_budget_usd: Option<f64>,
    /// The attempt's id, handed to the agent as `PYGG_ATTEMPT_ID`.
    pub attempt_id: String,
}

/// Runs one attempt and returns its run, worktree included. The app's runs the agent CLI (`agent_executor`);
/// tests substitute their own.
pub type Executor = Arc<dyn Fn(ExecRequest, PathBuf, Cancel) -> BoxFuture<'static, Result<TaskRun>> + Send + Sync>;

pub fn agent_executor() -> Executor {
    Arc::new(|request: ExecRequest, worktrees: PathBuf, cancel: Cancel| -> BoxFuture<'static, Result<TaskRun>> {
        Box::pin(async move {
            let request = agents::TaskRequest {
                agent: request.agent,
                workspace: request.workspace,
                prompt: request.prompt,
                model: request.model,
                allow_shell: request.allow_shell,
                timeout_secs: Some(request.timeout_secs),
                base: Some(request.base),
                max_budget_usd: request.max_budget_usd,
                attempt_id: Some(request.attempt_id),
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
                return Err(m::CANNOT_STOP.into());
            }
            let tx = c.unchecked_transaction().map_err(err)?;
            tx.execute("UPDATE approvals SET state = 'invalidated', decided_at = ?2 WHERE state = 'pending' AND attempt_id IN \
                        (SELECT id FROM attempts WHERE stage_id = ?1)", params![row.stage_id, now_ms()]).map_err(err)?;
            tx.execute("UPDATE attempts SET state = 'canceled', finished_at = ?2 WHERE stage_id = ?1 AND state IN ('waiting', 'running')",
                params![row.stage_id, now_ms()]).map_err(err)?;
            set_task(&tx, id, "canceled", None, Some(m::STOPPED)).map_err(err)?;
            signal(&tx, "cancelled", "task", id, true, "user").map_err(err)?;
            audit(&tx, "task.canceled", id, serde_json::json!({})).map_err(err)?;
            tx.commit().map_err(err)
        })
    }

    /// Approves the paid step a task waits on with a spending cap (micro-USD), or declines it (ADR-010: an approval
    /// names one attempt and a cap). If the step's estimate changed since the approval was asked, the approval is
    /// invalidated, a new one is asked with the new estimate, and this returns an error saying so. Approving
    /// continues the task.
    ///
    /// A step that asks only because its workspace is sensitive is not paid, so it needs no cap.
    pub fn decide(
        &self, id: &str, approve: bool, cap_micro_usd: Option<u64>,
    ) -> Result<Option<tokio::task::JoinHandle<()>>> {
        let cap = cap_micro_usd.filter(|c| *c > 0 && approve);
        self.with_db(|c| {
            let row = task_row(c, id)?;
            if row.state != "waiting_approval" {
                return Err(m::NOT_WAITING.into());
            }
            let (attempt, rung): (String, i64) = c.query_row(
                "SELECT id, rung FROM attempts WHERE stage_id = ?1 AND state = 'waiting' ORDER BY number DESC LIMIT 1",
                [&row.stage_id], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(err)?
                .ok_or(m::NOT_WAITING)?;
            let step = row.policy.steps.get(rung as usize).ok_or(m::STEP_NOT_IN_POLICY)?;
            if approve && step.billing == "paid" && cap.is_none() {
                return Err(m::SET_CAP.into());
            }
            let asked_on: Option<String> = c.query_row(
                "SELECT policy_version FROM approvals WHERE attempt_id = ?1 AND state = 'pending' ORDER BY rowid DESC LIMIT 1",
                [&attempt], |r| r.get(0)).optional().map_err(err)?;
            let tx = c.unchecked_transaction().map_err(err)?;
            if approve {
                let now_basis = basis(step, &estimate_for(&tx, step).map_err(err)?, &limits(&tx, &row.workspace_id).map_err(err)?);
                if asked_on.as_deref() != Some(now_basis.as_str()) {
                    tx.execute("UPDATE approvals SET state = 'invalidated', decided_at = ?2 WHERE attempt_id = ?1 AND state = 'pending'",
                        params![attempt, now_ms()]).map_err(err)?;
                    ask_approval(&tx, &attempt, step, &row.workspace_id).map_err(err)?;
                    audit(&tx, "approval.invalidated", id, serde_json::json!({"attempt": attempt})).map_err(err)?;
                    tx.commit().map_err(err)?;
                    return Err(m::APPROVAL_STALE.into());
                }
                let left = if step.billing == "paid" { remaining(&tx, &row.workspace_id).map_err(err)? } else { None };
                if let (Some(cap), Some(left)) = (cap, left) {
                    if cap > left {
                        return Err(fill(m::CAP_OVER_BUDGET, &[&format!("{:.2}", usd(left))]));
                    }
                }
            }
            tx.execute("UPDATE approvals SET state = ?2, cap_micro_usd = COALESCE(?3, cap_micro_usd), decided_at = ?4 \
                        WHERE attempt_id = ?1 AND state = 'pending'",
                params![attempt, if approve { "approved" } else { "denied" }, cap.map(|c| c as i64), now_ms()]).map_err(err)?;
            if approve {
                set_task(&tx, id, "running", Some("preparing"), None).map_err(err)?;
            } else {
                close_attempt(&tx, &attempt, "canceled", Some(m::DECLINED_STEP)).map_err(err)?;
                signal(&tx, "declined", "attempt", &attempt, true, "user").map_err(err)?;
                set_task(&tx, id, "blocked", None, Some(m::DECLINED_TASK)).map_err(err)?;
            }
            audit(&tx, if approve { "approval.approved" } else { "approval.denied" }, id,
                serde_json::json!({"attempt": attempt, "capMicroUsd": cap})).map_err(err)?;
            tx.commit().map_err(err)
        })?;
        Ok(if approve { self.start(id) } else { None })
    }

    /// Runs an interrupted task again from the step that was running.
    pub fn retry(&self, id: &str) -> Result<Option<tokio::task::JoinHandle<()>>> {
        self.with_db(|c| {
            let row = task_row(c, id)?;
            if row.state != "interrupted" {
                return Err(m::CANNOT_RETRY.into());
            }
            set_task(c, id, "running", Some("preparing"), None).map_err(err)?;
            signal(c, "retried", "task", id, true, "user").map_err(err)
        })?;
        Ok(self.start(id))
    }

    async fn drive(&self, id: &str, cancel: &Cancel) -> Result<()> {
        let row = self.with_db(|c| task_row(c, id))?;
        if row.state != "running" {
            return Ok(());
        }
        let base = row.base_revision.clone().ok_or(m::NO_BASE)?;
        let attempts = self.with_db(|c| attempts_of(c, id))?;
        let Some((mut index, mut carried)) = resume_point(&attempts) else { return Ok(()) };
        let mut last_failure =
            attempts.iter().rev().find(|a| a.outcome == "failed").and_then(|a| a.escalation_reason.clone());
        let stop = |runner: &Runner| {
            runner
                .with_db(|c| {
                    set_task(c, id, "canceled", None, Some(m::STOPPED))?;
                    signal(c, "cancelled", "task", id, true, "user")
                })
                .map_err(err)
        };
        // Set when a paid step went over its cap: the ladder stops there instead of trying further steps.
        let mut halt: Option<String> = None;

        loop {
            if *cancel.borrow() {
                return stop(self);
            }
            if let Some(why) = halt.take() {
                let reason = fill(m::HALTED, &[&why]);
                return self.with_db(|c| set_task(c, id, "blocked", None, Some(&reason))).map_err(err);
            }
            let Some(step) = row.policy.steps.get(index) else {
                let reason = fill(m::ALL_FAILED, &[&last_failure.as_deref().unwrap_or(m::NO_ATTEMPT)]);
                return self.with_db(|c| set_task(c, id, "check_failed", None, Some(&reason))).map_err(err);
            };
            // A step whose model's account is held for good (no credit, a rejected key) cannot run: it is skipped
            // before it asks for approval or starts the agent (the owner's run lost 10 minutes on such a step).
            if let Some(why) = self.with_db(|c| held_account(c, step)).map_err(err)? {
                self.with_db(|c| skip_attempt(c, id, &row.stage_id, index, step, &why)).map_err(err)?;
                last_failure = Some(why);
                index += 1;
                continue;
            }
            // A step whose service has stopped answering is skipped, unless it is the last one left to try.
            if index + 1 < row.policy.steps.len() {
                if let Some(why) = self.with_db(|c| unresponsive(c, step)).map_err(err)? {
                    self.with_db(|c| skip_attempt(c, id, &row.stage_id, index, step, &why)).map_err(err)?;
                    last_failure = Some(why);
                    index += 1;
                    continue;
                }
            }
            let attempt = match self.with_db(|c| open_attempt(c, id, &row.stage_id, index, step)).map_err(err)? {
                Opened::Run(attempt) => attempt,
                Opened::Waiting | Opened::Declined => return Ok(()),
            };
            self.with_db(|c| set_phase(c, id, "generating")).map_err(err)?;
            let cap = self.with_db(|c| approved_cap(c, &attempt)).map_err(err)?;
            // What this attempt is told about earlier ones, recorded as sent so the user can see it.
            let earlier =
                if row.policy.handoff == "evidence" { self.with_db(|c| attempts_of(c, id))? } else { Vec::new() };
            let given = handoff_for(&row.policy.handoff, carried.as_deref(), &earlier);
            self.with_db(|c| c.execute("UPDATE attempts SET handoff_sent = ?2 WHERE id = ?1", params![attempt, given]))
                .map_err(err)?;
            let request = ExecRequest {
                agent: step.agent.clone(),
                model: step.model.clone(),
                workspace: PathBuf::from(&row.workspace_path),
                base: base.clone(),
                prompt: compose_with(
                    &if row.policy.answer {
                        format!("{}\n\n---\n{ANSWER_BRIEF}", row.prompt)
                    } else {
                        row.prompt.clone()
                    },
                    given.as_deref(),
                ),
                allow_shell: row.policy.allow_shell,
                timeout_secs: row.policy.timeout_secs,
                max_budget_usd: cap.map(usd),
                attempt_id: attempt.clone(),
            };
            let name = agents::find(&step.agent).map(|a| a.name).unwrap_or(step.agent.as_str());
            let run = match (self.exec)(request, self.worktrees.clone(), cancel.clone()).await {
                Ok(run) => run,
                Err(e) => {
                    // The agent could not start (not installed, the workspace moved): try the next step.
                    let reason = fill(m::COULD_NOT_RUN, &[&name, &e]);
                    self.with_db(|c| {
                        c.execute("UPDATE attempts SET detail = ?2 WHERE id = ?1", params![attempt, e])?;
                        close_attempt(c, &attempt, "failed", Some(&reason))
                    })
                    .map_err(err)?;
                    last_failure = Some(reason);
                    index += 1;
                    continue;
                }
            };
            let through_pygg = self.with_db(|c| -> Result<Option<(u64, u32)>> {
                agents::record_for(c, &run, Some(&attempt))?;
                save_run(c, &attempt, &run).map_err(err)?;
                link_gateway_usage(c, &attempt).map_err(err)
            })?;
            // A paid step's cap, checked against what the run cost. Requests the agent sent through Pygg are priced by
            // Pygg's ledger, whatever the agent estimates; otherwise the agent's own estimate is all there is. Claude
            // Code stops at the cap itself; the others can only be checked afterwards.
            let (reported, whose) = match through_pygg {
                Some((known, 0)) => (Some(known), m::PYGG_LEDGER.to_string()),
                Some(_) => (None, String::new()),
                None => {
                    (run.usage.estimate_usd.map(|u| (u * 1e6).round().max(0.0) as u64), fill(m::OWN_ESTIMATE, &[&name]))
                }
            };
            let over_cap = match (cap, reported) {
                (Some(cap), Some(cost)) if cost > cap => {
                    Some(fill(m::OVER_CAP, &[&whose, &format!("{:.2}", usd(cost)), &format!("{:.2}", usd(cap))]))
                }
                _ => None,
            };
            let unchecked = match through_pygg {
                Some((_, unpriced)) if unpriced > 0 => Some(fill(m::UNPRICED, &[&unpriced])),
                _ => (reported.is_none() && !agents::enforces_budget(&step.agent)).then(|| fill(m::NO_COST, &[&name])),
            };
            let cost_note = over_cap.clone().or_else(|| unchecked.filter(|_| cap.is_some()));
            halt = over_cap;
            let worktree = Worktree {
                repo: PathBuf::from(&row.workspace_path),
                path: PathBuf::from(&run.worktree),
                base: run.base.clone(),
            };
            // Which paths it changed, kept after a discarded worktree is gone, so a later step can be told (H1).
            if run.files_changed > 0 {
                let changed = isolation::changed_files(&worktree).unwrap_or_default();
                let json = serde_json::to_string(&changed).map_err(|e| e.to_string())?;
                self.with_db(|c| {
                    c.execute("UPDATE attempts SET changed_paths_json = ?2 WHERE id = ?1", params![attempt, json])
                })
                .map_err(err)?;
            }
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
                let reason = fill(m::DID_NOT_FINISH, &[&name, &last_chars(&detail, 300)]);
                drop_worktree(self)?;
                self.with_db(|c| close_attempt(c, &attempt, "failed", Some(&reason))).map_err(err)?;
                // The agent's own failure says nothing about the code, so the next step starts with the task alone.
                carried = None;
                last_failure = Some(reason);
                index += 1;
                continue;
            }
            if row.policy.answer {
                drop_worktree(self)?;
                let answered = run.usage.result.as_deref().is_some_and(|t| !t.trim().is_empty());
                if answered {
                    return self
                        .with_db(|c| {
                            close_attempt(c, &attempt, "done", None)?;
                            set_task(c, id, "answered", None, Some(m::ANSWERED))
                        })
                        .map_err(err);
                }
                self.with_db(|c| close_attempt(c, &attempt, "failed", Some(m::NO_ANSWER))).map_err(err)?;
                carried = None;
                last_failure = Some(fill(m::LAST_FAILURE, &[&name, &m::NO_ANSWER]));
                index += 1;
                continue;
            }
            if run.files_changed == 0 {
                drop_worktree(self)?;
                self.with_db(|c| {
                    c.execute("UPDATE attempts SET detail = ?2 WHERE id = ?1", params![attempt, NO_CHANGES])?;
                    close_attempt(c, &attempt, "failed", Some(NO_CHANGES))
                })
                .map_err(err)?;
                carried = Some(NO_CHANGES_HANDOFF.into());
                last_failure = Some(fill(m::LAST_FAILURE, &[&name, &NO_CHANGES]));
                index += 1;
                continue;
            }
            if row.policy.checks.is_empty() {
                return self
                    .with_db(|c| {
                        close_attempt(c, &attempt, "done", None)?;
                        set_task(
                            c,
                            id,
                            "needs_review",
                            None,
                            Some(&match &cost_note {
                                Some(note) => fill(m::NO_CHECKS_NOTE, &[note]),
                                None => m::NO_CHECKS.to_string(),
                            }),
                        )
                    })
                    .map_err(err);
            }

            // Checking's WIP limit (ADR-042): a card's run waits here, phase `held`, while Checking is full.
            if !self.with_db(|c| crate::board::checking_room(c, id))? {
                self.with_db(|c| set_phase(c, id, "held")).map_err(err)?;
                loop {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    if *cancel.borrow() || self.with_db(|c| crate::board::checking_room(c, id))? {
                        break;
                    }
                }
                if *cancel.borrow() {
                    drop_worktree(self)?;
                    self.with_db(|c| close_attempt(c, &attempt, "canceled", None)).map_err(err)?;
                    return stop(self);
                }
            }
            self.with_db(|c| set_phase(c, id, "checking")).map_err(err)?;
            let mut results: Vec<CheckResult> = Vec::new();
            for check in &row.policy.checks {
                let result =
                    checks::run(&check.command, &worktree.path, CHECK_TIMEOUT, &[], canceled(cancel.clone())).await;
                let passed = result.passed;
                results.push(result);
                if !passed || *cancel.borrow() {
                    break;
                }
            }
            let json = serde_json::to_string(&results).map_err(|e| e.to_string())?;
            self.with_db(|c| {
                c.execute("UPDATE attempts SET check_results_json = ?2 WHERE id = ?1", params![attempt, json])
            })
            .map_err(err)?;
            if *cancel.borrow() {
                drop_worktree(self)?;
                self.with_db(|c| close_attempt(c, &attempt, "canceled", None)).map_err(err)?;
                return stop(self);
            }
            match results.iter().find(|r| !r.passed) {
                None => {
                    // An attempt that edits the file defining a check can make that check pass without doing the task.
                    let changed = isolation::changed_files(&worktree).unwrap_or_default();
                    let mut touched: Vec<&str> = row
                        .policy
                        .checks
                        .iter()
                        .map(|c| c.source.as_str())
                        .filter(|s| *s != "user" && changed.iter().any(|p| p == s || p.ends_with(&format!("/{s}"))))
                        .collect();
                    touched.dedup();
                    return self
                        .with_db(|c| {
                            close_attempt(c, &attempt, "passed", None)?;
                            signal(c, "checks_passed", "attempt", &attempt, true, "check")?;
                            if touched.is_empty() {
                                set_task(
                                    c,
                                    id,
                                    "checks_passed",
                                    None,
                                    cost_note.as_ref().map(|n| fill(m::NOTE, &[n])).as_deref(),
                                )
                            } else {
                                let files = touched.join(", ");
                                set_task(
                                    c,
                                    id,
                                    "needs_review",
                                    None,
                                    Some(&match &cost_note {
                                        Some(note) => fill(m::CHANGED_DEFINITIONS_NOTE, &[&files, note]),
                                        None => fill(m::CHANGED_DEFINITIONS, &[&files]),
                                    }),
                                )
                            }
                        })
                        .map_err(err);
                }
                Some(failed) => {
                    let reason = fill(m::CHECK_FAILED, &[&failed.command, &failed.detail]);
                    carried = Some(handoff(failed));
                    drop_worktree(self)?;
                    self.with_db(|c| {
                        close_attempt(c, &attempt, "failed", Some(&reason))?;
                        signal(c, "checks_passed", "attempt", &attempt, false, "check")
                    })
                    .map_err(err)?;
                    last_failure = Some(fill(m::LAST_FAILURE, &[&name, &reason]));
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
        return Err(m::NO_RESULT.into());
    }
    let attempt = attempts_of(conn, id)?
        .into_iter()
        .rev()
        .find(|a| a.open && (a.outcome == "passed" || a.outcome == "done"))
        .ok_or(m::WORKTREE_GONE)?;
    let worktree = Worktree {
        repo: PathBuf::from(&row.workspace_path),
        path: PathBuf::from(attempt.worktree.clone().unwrap_or_default()),
        base: attempt.base.clone().or_else(|| row.base_revision.clone()).unwrap_or_default(),
    };
    Ok((row, attempt, worktree))
}

pub fn result_diff(conn: &Connection, id: &str) -> Result<String> {
    match result_attempt(conn, id) {
        Ok((_, _, worktree)) => isolation::diff(&worktree).map_err(|e| e.to_string()),
        // A solo result kept as its own branch: its changes are that branch against where the task started. A plan
        // card's branch holds the other cards too, so it has no diff of its own here (the plan's page has the whole).
        Err(e) => {
            let row = task_row(conn, id)?;
            match (row.result.as_deref(), row.result_branch.as_deref(), row.base_revision.as_deref()) {
                (Some("kept"), Some(branch), Some(base)) if !branch.starts_with("str/brain-") => {
                    isolation::branch_diff(Path::new(&row.workspace_path), branch, Some(base), 512 * 1024)
                        .map_err(|e| e.to_string())
                }
                _ => Err(e),
            }
        }
    }
}

/// The result as a patch file's name and text (ADR-009, Export Patch). A short header names the task and the commit it
/// applies to; `git apply` skips it. The result stays open: keep or discard it afterwards.
pub fn result_patch(conn: &Connection, id: &str) -> Result<(String, String)> {
    let (row, _, worktree) = result_attempt(conn, id)?;
    let diff = isolation::patch(&worktree).map_err(|e| e.to_string())?;
    let name = format!("{}.patch", isolation::branch_name(&row.title, id).trim_start_matches("str/"));
    let header = format!(
        "Pygg task: {}\nApplies to: {}\nApply in the repository with: git apply {name}\n\n",
        row.title, worktree.base
    );
    Ok((name, header + &diff))
}

pub fn record_export(conn: &Connection, id: &str, path: &str) -> Result<()> {
    signal(conn, "result_kept", "task", id, true, "user").map_err(err)?;
    audit(conn, "task.exported", id, serde_json::json!({"file": path})).map_err(err)
}

/// Keeps the result as a new branch of the workspace (the user's checkout is untouched) and removes the worktree.
pub fn keep(conn: &Connection, id: &str, title: Option<&str>) -> Result<String> {
    let (row, attempt, worktree) = result_attempt(conn, id)?;
    let title = title.map(str::trim).filter(|t| !t.is_empty()).unwrap_or(&row.title);
    let own = isolation::branch_name(title, id);
    // A board card's result goes onto its brainstorm's branch, where the next card starts (ADR-042); when it cannot
    // (the branch is checked out, or moved and the result conflicts), it is kept as a branch of its own, as any task.
    let shared: Option<String> = conn
        .query_row(
            "SELECT b.branch FROM tasks t JOIN brainstorms b ON b.id = t.brainstorm_id WHERE t.id = ?1",
            [id],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?
        .flatten();
    let (branch, not_added) = match shared {
        Some(shared) => {
            let commit = isolation::commit_result(&worktree, title).map_err(|e| e.to_string())?;
            match isolation::add_to_branch(&worktree.repo, &shared, &commit, &worktree.base)
                .map_err(|e| e.to_string())?
            {
                Ok(_) => (shared, None),
                Err(why) => {
                    isolation::create_branch(&worktree.repo, &own, &commit).map_err(|e| e.to_string())?;
                    (own, Some((shared, why)))
                }
            }
        }
        None => {
            isolation::keep_as_branch(&worktree, &own, title).map_err(|e| e.to_string())?;
            (own, None)
        }
    };
    let _ = isolation::discard(&worktree);
    let tx = conn.unchecked_transaction().map_err(err)?;
    forget_worktree(&tx, &attempt.id).map_err(err)?;
    tx.execute(
        "UPDATE tasks SET result = 'kept', result_branch = ?2, \
                state = CASE state WHEN 'needs_review' THEN 'manually_accepted' ELSE state END, \
                reason = CASE state WHEN 'needs_review' THEN NULL ELSE reason END, updated_at = ?3 WHERE id = ?1",
        params![id, branch, now_ms()],
    )
    .map_err(err)?;
    signal(&tx, "result_kept", "task", id, true, "user").map_err(err)?;
    let note = not_added.map(|(shared, why)| {
        let why = match why {
            isolation::NotAdded::CheckedOut => "it is checked out",
            isolation::NotAdded::Conflict => "it conflicts with what the brainstorm's branch has now",
        };
        format!("not added to {shared}: {why}")
    });
    audit(&tx, "task.kept", id, serde_json::json!({"branch": branch, "notAdded": note})).map_err(err)?;
    tx.commit().map_err(err)?;
    Ok(branch)
}

/// Discards the result and its worktree.
pub fn discard(conn: &Connection, id: &str) -> Result<()> {
    let (_, attempt, worktree) = result_attempt(conn, id)?;
    isolation::discard(&worktree).map_err(|e| e.to_string())?;
    let tx = conn.unchecked_transaction().map_err(err)?;
    forget_worktree(&tx, &attempt.id).map_err(err)?;
    tx.execute(
        "UPDATE tasks SET result = 'discarded', \
                state = CASE state WHEN 'needs_review' THEN 'canceled' ELSE state END, \
                reason = CASE state WHEN 'needs_review' THEN ?3 ELSE reason END, updated_at = ?2 WHERE id = ?1",
        params![id, now_ms(), m::DISCARDED],
    )
    .map_err(err)?;
    signal(&tx, "result_kept", "task", id, false, "user").map_err(err)?;
    audit(&tx, "task.discarded", id, serde_json::json!({})).map_err(err)?;
    tx.commit().map_err(err)
}

// ---------- Tasks from gateway clients (the task API, the MCP server) ----------

/// Refuses a client access to a task it did not start, in the same words as a task that does not exist.
pub fn check_owner(conn: &Connection, id: &str, client_id: &str) -> Result<()> {
    let owner: Option<Option<String>> =
        conn.query_row("SELECT client_id FROM tasks WHERE id = ?1", [id], |r| r.get(0)).optional().map_err(err)?;
    match owner {
        Some(Some(owner)) if owner == client_id => Ok(()),
        _ => Err(m::TASK_GONE.into()),
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
    strategy_order(strategy)
        .into_iter()
        .filter(|id| installed.contains(id))
        .take(3)
        .map(|id| Step { agent: id.into(), model: None, tier: tier_guess(id), billing: "unknown".into() })
        .collect()
}

/// The agents on this computer's PATH (no programs started).
pub fn installed_agents() -> Vec<&'static str> {
    agents::AGENTS.iter().filter(|a| a.programs.iter().any(|p| agents::locate(p).is_some())).map(|a| a.id).collect()
}

pub fn list_tasks_for(conn: &Connection, client_id: &str) -> Result<Vec<TaskSummary>> {
    let ids: Vec<String> = conn
        .prepare("SELECT id FROM tasks WHERE client_id = ?1 ORDER BY updated_at DESC LIMIT 50")
        .map_err(err)?
        .query_map([client_id], |r| r.get(0))
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;
    ids.iter()
        .map(|id| {
            let row = task_row(conn, id)?;
            let attempts = attempts_of(conn, id)?;
            summary_of(conn, id, &row, &attempts)
        })
        .collect()
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
    let interrupted = tx
        .execute(
            "UPDATE tasks SET state = 'interrupted', phase = NULL, reason = ?2, updated_at = ?1 \
                WHERE state = 'running'",
            params![now, m::INTERRUPTED],
        )
        .map_err(err)?;
    tx.execute("UPDATE task_stages SET state = 'interrupted' WHERE state = 'running'", []).map_err(err)?;
    tx.commit().map_err(err)?;

    // Worktrees whose result still waits for the user stay; everything else in the app's folder is a leftover.
    let keep: Vec<PathBuf> = conn
        .prepare("SELECT a.worktree_path FROM attempts a WHERE a.worktree_path IS NOT NULL")
        .map_err(err)?
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(err)?
        .filter_map(|p| p.ok())
        .map(PathBuf::from)
        .collect();
    if let Ok(entries) = std::fs::read_dir(worktrees) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && !keep.iter().any(|k| same_path(k, &path)) {
                let _ = std::fs::remove_dir_all(&path);
            }
        }
    }
    conn.execute(
        "UPDATE attempts SET worktree_path = NULL WHERE state NOT IN ('passed', 'done') AND worktree_path IS NOT NULL",
        [],
    )
    .map_err(err)?;
    let repos: Vec<String> = conn
        .prepare("SELECT path FROM workspaces")
        .map_err(err)?
        .query_map([], |r| r.get(0))
        .map_err(err)?
        .filter_map(|p| p.ok())
        .collect();
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
        /// The agent hit its time limit without a single output token, as a service that does not answer.
        TimeOut,
        Write(&'static str, &'static str),
        /// `Write`, and the agent reports this list-price estimate (USD) for the run.
        Costly(&'static str, &'static str, f64),
        Fail,
        Nothing,
        /// Changes nothing and ends with this final message (an answer task's answer).
        Answer(&'static str),
        WaitForStop,
        /// `Write`, with one request through Pygg's gateway tagged with the attempt (its session header), priced at
        /// this many micro-USD or unpriced; the agent itself estimates $0, as OpenCode does for a custom provider.
        ThroughPygg(&'static str, &'static str, Option<u64>, Arc<Mutex<Connection>>),
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
        Rig {
            worktrees: dir.path().join("worktrees"),
            _dir: dir,
            db: Arc::new(Mutex::new(conn)),
            repo,
            workspace: ws.id,
            prompts: Arc::default(),
        }
    }

    /// An executor that does `acts` in order, one per attempt, in a real worktree.
    fn fake(acts: Vec<Act>, prompts: Arc<Mutex<Vec<String>>>) -> Executor {
        let acts = Arc::new(Mutex::new(std::collections::VecDeque::from(acts)));
        Arc::new(
            move |request: ExecRequest, worktrees: PathBuf, cancel: Cancel| -> BoxFuture<'static, Result<TaskRun>> {
                let act = acts.lock().unwrap().pop_front().unwrap_or(Act::Fail);
                prompts.lock().unwrap().push(request.prompt.clone());
                Box::pin(async move {
                    let id = new_id();
                    std::fs::create_dir_all(&worktrees).unwrap();
                    let worktree = isolation::create(&request.workspace, &request.base, &worktrees.join(&id[..12]))
                        .map_err(|e| e.to_string())?;
                    let mut outcome = "ok";
                    let mut estimate = None;
                    let mut said: Option<String> = None;
                    let mut output_tokens = Some(10);
                    match act {
                        Act::TimeOut => {
                            outcome = "timed_out";
                            output_tokens = None;
                        }
                        Act::Write(file, text) => std::fs::write(worktree.path.join(file), text).unwrap(),
                        Act::Costly(file, text, usd) => {
                            std::fs::write(worktree.path.join(file), text).unwrap();
                            estimate = Some(usd);
                        }
                        Act::ThroughPygg(file, text, price, db) => {
                            std::fs::write(worktree.path.join(file), text).unwrap();
                            estimate = Some(0.0);
                            use crate::gateway::pricing::Cost;
                            let cost = match price {
                                Some(micro_usd) => {
                                    Cost::Estimated { micro_usd, source: "a price the user entered".into() }
                                }
                                None => Cost::Unknown { source: "no price recorded for this model".into() },
                            };
                            crate::gateway::store::record_usage(
                                &lock(&db),
                                &crate::gateway::store::UsageRecord {
                                    client_id: None,
                                    provider_id: "anthropic",
                                    model_id: "claude-haiku-5-5",
                                    requested_model: "anthropic/claude-haiku-5-5",
                                    input_tokens: Some(1000),
                                    output_tokens: Some(50),
                                    cache_tokens: None,
                                    cost: &cost,
                                    outcome: "ok",
                                    http_status: Some(200),
                                    latency_ms: 5,
                                    error: None,
                                    cache_write_tokens: None,
                                    session_key: Some(&request.attempt_id),
                                    route_mode: Some("native"),
                                    route_reason: None,
                                    facts: None,
                                    repeated_tokens: None,
                                },
                            )
                            .unwrap();
                        }
                        Act::Fail => outcome = "failed",
                        Act::Nothing => {}
                        Act::Answer(text) => said = Some(text.into()),
                        Act::WaitForStop => {
                            canceled(cancel).await;
                            outcome = "canceled";
                        }
                    }
                    let stat = isolation::changes(&worktree).unwrap();
                    Ok(TaskRun {
                        id,
                        agent: request.agent,
                        model: request.model,
                        workspace: request.workspace.to_string_lossy().into_owned(),
                        worktree: worktree.path.to_string_lossy().into_owned(),
                        base: worktree.base,
                        outcome: outcome.into(),
                        exit: match outcome {
                            "ok" => "exit 0".into(),
                            "timed_out" => "timed out".into(),
                            _ => "exit 1".into(),
                        },
                        duration_ms: 5,
                        files_changed: stat.files,
                        insertions: stat.insertions,
                        deletions: stat.deletions,
                        usage: AgentUsage {
                            input_tokens: Some(100),
                            output_tokens,
                            estimate_usd: estimate,
                            result: said,
                            ..AgentUsage::default()
                        },
                        // The attempt id the engine handed over, so a test can check it is the attempt's own.
                        log_tail: request.attempt_id.clone(),
                    })
                })
            },
        )
    }

    fn step(agent: &str, billing: &str, tier: u8) -> Step {
        Step { agent: agent.into(), model: None, tier, billing: billing.into() }
    }

    fn create(rig: &Rig, steps: Vec<Step>) -> String {
        let new = NewTask {
            workspace_id: rig.workspace.clone(),
            title: None,
            prompt: "Make result.txt say good".into(),
            strategy: "free_first".into(),
            steps,
            allow_shell: false,
            timeout_secs: None,
            handoff: None,
            answer: false,
        };
        create_task(&lock(&rig.db), &new).unwrap()
    }

    /// A check that passes when result.txt contains "good".
    fn good_check() -> &'static str {
        if cfg!(windows) {
            "findstr /c:good result.txt"
        } else {
            "grep -q good result.txt"
        }
    }

    fn detail(rig: &Rig, id: &str) -> TaskDetail {
        task_detail(&lock(&rig.db), id).unwrap()
    }

    #[tokio::test]
    async fn an_answer_task_ends_with_the_agents_answer_and_keeps_nothing() {
        let rig = rig(&[good_check()]);
        // The first agent says nothing; the second answers.
        let runner = Runner::new(
            rig.db.clone(),
            rig.worktrees.clone(),
            fake(vec![Act::Nothing, Act::Answer("The app starts in src/main.rs:12.")], rig.prompts.clone()),
        );
        let new = NewTask {
            workspace_id: rig.workspace.clone(),
            title: None,
            prompt: "Explain how the app starts".into(),
            strategy: "free_first".into(),
            steps: vec![step("opencode", "free", 1), step("codex", "subscription", 3)],
            allow_shell: false,
            timeout_secs: None,
            handoff: None,
            answer: true,
        };
        let id = create_task(&lock(&rig.db), &new).unwrap();
        runner.start(&id).unwrap().await.unwrap();
        let task = detail(&rig, &id);
        assert_eq!(task.summary.state, "answered", "{:?}", task.reason);
        assert_eq!(task.reason.as_deref(), Some(m::ANSWERED));
        let last = task.attempt_details.last().unwrap();
        assert_eq!(
            (last.outcome.as_str(), last.summary.as_deref()),
            ("done", Some("The app starts in src/main.rs:12."))
        );
        assert_eq!(task.attempt_details[0].escalation_reason.as_deref(), Some(m::NO_ANSWER));
        let prompts = rig.prompts.lock().unwrap().clone();
        assert!(prompts[0].contains(ANSWER_BRIEF), "{}", prompts[0]);
        assert!(task.result.is_none(), "nothing to keep");
    }

    #[tokio::test]
    async fn a_step_whose_account_has_no_credit_is_skipped_before_it_runs() {
        let rig = rig(&[good_check()]);
        let gemini_conn = crate::gateway::store::add_provider(
            &lock(&rig.db),
            &crate::gateway::store::NewProvider {
                kind: "openrouter",
                label: "gemini",
                endpoint: "https://example.invalid/v1",
                credential_ref: None,
                default_billing: "paid",
                wire: "openai",
            },
        )
        .unwrap();
        crate::gateway::store::mark_health(
            &lock(&rig.db),
            &gemini_conn.id,
            "",
            "payment_required",
            "prepayment credits are depleted",
            Some(402),
            Some(now_ms() + 600_000),
        )
        .unwrap();
        let runner = Runner::new(
            rig.db.clone(),
            rig.worktrees.clone(),
            fake(vec![Act::Write("result.txt", "good\n")], rig.prompts.clone()),
        );
        let gemini =
            Step { model: Some(format!("pygg/{}/gemini-4-carbon", gemini_conn.id)), ..step("opencode", "paid", 3) };
        let id = create(&rig, vec![gemini, step("codex", "subscription", 3)]);
        runner.start(&id).unwrap().await.unwrap();
        let task = detail(&rig, &id);
        assert_eq!(task.summary.state, "checks_passed", "{:?}", task.reason);
        let skipped = &task.attempt_details[0];
        assert_eq!(skipped.outcome, "skipped");
        assert!(skipped.escalation_reason.as_deref().unwrap_or("").contains("payment required"), "{skipped:?}");
        assert_eq!(
            rig.prompts.lock().unwrap().len(),
            1,
            "only the second step ran; no approval was asked for the first"
        );
    }

    #[test]
    fn the_handoff_says_what_failed() {
        assert!(previous_attempt(NO_CHANGES_HANDOFF).contains("did not change any file"));
        assert!(!previous_attempt(NO_CHANGES_HANDOFF).contains("checks"));
        assert!(previous_attempt("The check `x` exited 1").contains("did not pass the workspace's checks"));
    }

    #[tokio::test]
    async fn the_handoff_mode_decides_what_the_next_step_is_told_and_it_is_recorded_as_sent() {
        for (mode, expect) in
            [("none", None), ("tail", Some("did not pass the")), ("evidence", Some(crate::handoff::VERSION))]
        {
            let rig = rig(&[good_check()]);
            let runner = Runner::new(
                rig.db.clone(),
                rig.worktrees.clone(),
                fake(vec![Act::Write("result.txt", "bad\n"), Act::Write("result.txt", "good\n")], rig.prompts.clone()),
            );
            let new = NewTask {
                workspace_id: rig.workspace.clone(),
                title: None,
                prompt: "Make result.txt say good".into(),
                strategy: "free_first".into(),
                steps: vec![step("opencode", "free", 1), step("codex", "subscription", 3)],
                allow_shell: false,
                timeout_secs: None,
                handoff: Some(mode.into()),
                answer: false,
            };
            let id = create_task(&lock(&rig.db), &new).unwrap();
            runner.start(&id).unwrap().await.unwrap();

            let task = detail(&rig, &id);
            assert_eq!(task.summary.state, "checks_passed", "{mode}: {:?}", task.reason);
            assert_eq!(task.policy.handoff, mode);
            let second = rig.prompts.lock().unwrap()[1].clone();
            let sent = task.attempt_details[1].handoff_sent.clone();
            assert!(task.attempt_details[0].handoff_sent.is_none(), "the first step is given the task alone");
            assert_eq!(task.attempt_details[0].changed_paths, ["result.txt"], "kept after its worktree was removed");
            match expect {
                None => {
                    assert_eq!(second, "Make result.txt say good");
                    assert!(sent.is_none());
                }
                Some(marker) => {
                    let sent = sent.unwrap_or_else(|| panic!("{mode}: nothing recorded"));
                    assert!(sent.contains(marker), "{mode}: {sent}");
                    assert!(second.ends_with(&sent), "{mode}: what was recorded is what was sent");
                }
            }
            if mode == "evidence" {
                let sent = task.attempt_details[1].handoff_sent.clone().unwrap();
                assert!(sent.contains(&format!("Verified by the check `{}`", good_check())), "{sent}");
                assert!(sent.contains("Changed in that discarded attempt, and not in your copy: result.txt"), "{sent}");
            }
        }
        let rig = rig(&[good_check()]);
        let bad = NewTask {
            workspace_id: rig.workspace.clone(),
            title: None,
            prompt: "x".into(),
            strategy: "free_first".into(),
            steps: vec![step("codex", "subscription", 3)],
            allow_shell: false,
            timeout_secs: None,
            handoff: Some("memory".into()),
            answer: false,
        };
        assert!(create_task(&lock(&rig.db), &bad).unwrap_err().contains("Unknown handoff"));
    }

    #[tokio::test]
    async fn a_step_that_twice_timed_out_without_answering_is_skipped_unless_it_is_the_last() {
        let rig = rig(&[good_check()]);
        let good = || Act::Write("result.txt", "good\n");
        let runner = Runner::new(
            rig.db.clone(),
            rig.worktrees.clone(),
            fake(
                vec![
                    Act::TimeOut,
                    good(), // task 1
                    Act::TimeOut,
                    good(),       // task 2
                    good(),       // task 3: its first step is skipped, so the next act is step 2's
                    Act::TimeOut, // task 4: a one-step ladder still tries its only step
                ],
                rig.prompts.clone(),
            ),
        );
        let ladder = || vec![step("opencode", "free", 0), step("codex", "subscription", 3)];
        for expected in [["failed", "passed"], ["failed", "passed"]] {
            let id = create(&rig, ladder());
            runner.start(&id).unwrap().await.unwrap();
            let outcomes: Vec<String> = detail(&rig, &id).attempt_details.iter().map(|a| a.outcome.clone()).collect();
            assert_eq!(outcomes, expected);
        }

        let id = create(&rig, ladder());
        runner.start(&id).unwrap().await.unwrap();
        let task = detail(&rig, &id);
        assert_eq!(task.summary.state, "checks_passed", "{:?}", task.reason);
        let outcomes: Vec<&str> = task.attempt_details.iter().map(|a| a.outcome.as_str()).collect();
        assert_eq!(outcomes, ["skipped", "passed"]);
        let why = task.attempt_details[0].escalation_reason.as_deref().unwrap();
        assert!(why.starts_with("Skipped: OpenCode") && why.contains("timed out on its last two runs"), "{why}");
        assert_eq!(rig.prompts.lock().unwrap().len(), 5, "the skipped step never ran");

        let only = create(&rig, vec![step("opencode", "free", 0)]);
        runner.start(&only).unwrap().await.unwrap();
        let task = detail(&rig, &only);
        let outcomes: Vec<&str> = task.attempt_details.iter().map(|a| a.outcome.as_str()).collect();
        assert_eq!(outcomes, ["failed"], "the last step left is run, not skipped");
        assert_eq!(rig.prompts.lock().unwrap().len(), 6);
    }

    #[tokio::test]
    async fn a_failed_check_moves_to_the_next_step_with_what_failed() {
        let rig = rig(&[good_check()]);
        let runner = Runner::new(
            rig.db.clone(),
            rig.worktrees.clone(),
            fake(vec![Act::Write("result.txt", "bad\n"), Act::Write("result.txt", "good\n")], rig.prompts.clone()),
        );
        let id = create(&rig, vec![step("opencode", "free", 1), step("codex", "subscription", 3)]);
        runner.start(&id).unwrap().await.unwrap();

        let task = detail(&rig, &id);
        assert_eq!(task.summary.state, "checks_passed", "{:?}", task.reason);
        let outcomes: Vec<&str> = task.attempt_details.iter().map(|a| a.outcome.as_str()).collect();
        assert_eq!(outcomes, ["failed", "passed"]);
        assert!(
            !task.attempt_details[0].open && task.attempt_details[1].open,
            "the failed attempt's worktree is gone, the result's stays"
        );
        assert_eq!(
            task.attempt_details[0].escalation_reason.as_deref(),
            Some(format!("`{}` exited 1", good_check()).as_str())
        );
        let prompts = rig.prompts.lock().unwrap().clone();
        assert_eq!(prompts[0], "Make result.txt say good");
        assert!(
            prompts[1].starts_with("Make result.txt say good\n\n---\n") && prompts[1].contains("did not pass"),
            "{}",
            prompts[1]
        );
        // Each attempt has its own ledger entry, with an unknown cost.
        assert_eq!(task.summary.cost.unknown_calls, 2);
        // Each agent run was told which attempt it is (PYGG_ATTEMPT_ID), so a gateway can tie requests to it.
        for a in &task.attempt_details {
            assert_eq!(a.log_tail.as_deref(), Some(a.id.as_str()));
        }
        assert_eq!(task.attempt_details[1].input_tokens, Some(100));

        let diff = result_diff(&lock(&rig.db), &id).unwrap();
        assert!(diff.contains("+good"), "{diff}");
        let branch = keep(&lock(&rig.db), &id, None).unwrap();
        assert!(branch.starts_with("str/make-result-txt-say-good-"), "{branch}");
        let task = detail(&rig, &id);
        assert_eq!((task.summary.state.as_str(), task.result.as_deref()), ("checks_passed", Some("kept")));
        assert!(task.attempt_details.iter().all(|a| !a.open));
        // Quality signals (ROADMAP C4): one attempt failed its checks, one passed, and the result was kept.
        let q = quality(&lock(&rig.db), 0).unwrap();
        assert_eq!((q.tasks, q.checked_attempts, q.passed_attempts, q.kept, q.discarded), (1, 2, 1, 1, 0));
        assert_eq!(
            (q.unknown_entries, q.micro_usd_per_kept),
            (2, Some(0)),
            "agent runs cost unknown, never counted as $0 spent"
        );
        assert_eq!(
            std::fs::read_to_string(rig.repo.join("app.txt")).unwrap(),
            "one\n",
            "the user's checkout is untouched"
        );
    }

    #[tokio::test]
    async fn a_brainstorms_cards_build_on_its_branch_one_after_another() {
        let rig = rig(&[good_check()]);
        let branch = "str/brain-shop-b1";
        crate::isolation::ensure_branch(&rig.repo, branch).unwrap();
        lock(&rig.db)
            .execute(
                "INSERT INTO brainstorms (id, session, state, requirement, prompt_version, revision, proposal_json, created_at, updated_at, branch)                  VALUES ('b1', 's1', 'approved', 'A shop', 'brain-v1', 1, '{}', 0, 0, ?1)",
                [branch],
            )
            .unwrap();
        // Card 1 writes the file the check reads; card 2 writes another file only, so its check passes only if it
        // started from card 1's kept result.
        let runner = Runner::new(
            rig.db.clone(),
            rig.worktrees.clone(),
            fake(
                vec![
                    Act::Write(
                        "result.txt",
                        "good
",
                    ),
                    Act::Write(
                        "page.txt", "two
",
                    ),
                ],
                rig.prompts.clone(),
            ),
        );
        let card = |n: &str| {
            let new = NewTask {
                workspace_id: rig.workspace.clone(),
                title: Some(format!("Card {n}")),
                prompt: format!("Card {n}"),
                strategy: "free_first".into(),
                steps: vec![step("opencode", "free", 0)],
                allow_shell: false,
                timeout_secs: None,
                handoff: None,
                answer: false,
            };
            let db = lock(&rig.db);
            let id = create_task_at(&db, &new, None, Some(branch)).unwrap();
            db.execute("UPDATE tasks SET brainstorm_id = 'b1', card_id = ?2 WHERE id = ?1", params![id, n]).unwrap();
            id
        };
        let first = card("c1");
        runner.start(&first).unwrap().await.unwrap();
        assert_eq!(keep(&lock(&rig.db), &first, None).unwrap(), branch);
        let second = card("c2");
        runner.start(&second).unwrap().await.unwrap();
        assert_eq!(detail(&rig, &second).summary.state, "checks_passed", "card 2 sees card 1's result.txt");
        assert_eq!(keep(&lock(&rig.db), &second, None).unwrap(), branch);
        let files =
            crate::isolation::git(&rig.repo, &["ls-tree", "--name-only", &format!("refs/heads/{branch}")]).unwrap();
        assert!(files.contains("result.txt") && files.contains("page.txt"), "{files}");
        assert_eq!(
            std::fs::read_to_string(rig.repo.join("app.txt")).unwrap(),
            "one
",
            "the user's checkout is untouched"
        );
    }

    #[tokio::test]
    async fn every_step_failing_ends_in_check_failed() {
        let rig = rig(&[good_check()]);
        let runner = Runner::new(
            rig.db.clone(),
            rig.worktrees.clone(),
            fake(vec![Act::Fail, Act::Nothing, Act::Write("result.txt", "bad\n")], rig.prompts.clone()),
        );
        let id = create(
            &rig,
            vec![step("opencode", "free", 1), step("opencode", "free", 1), step("codex", "subscription", 3)],
        );
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
        let runner = Runner::new(
            rig.db.clone(),
            rig.worktrees.clone(),
            fake(vec![Act::Write("new.txt", "x\n")], rig.prompts.clone()),
        );
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
        let runner = Runner::new(
            rig.db.clone(),
            rig.worktrees.clone(),
            fake(vec![Act::Write("package.json", "{\"scripts\":{\"test\":\"exit 0\"}}")], rig.prompts.clone()),
        );
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
        let runner = Runner::new(
            rig.db.clone(),
            rig.worktrees.clone(),
            fake(vec![Act::Write("result.txt", "bad\n"), Act::Write("result.txt", "good\n")], rig.prompts.clone()),
        );
        let id = create(&rig, vec![step("opencode", "free", 1), step("claude-code", "paid", 3)]);
        runner.start(&id).unwrap().await.unwrap();
        let task = detail(&rig, &id);
        assert_eq!(task.summary.state, "waiting_approval");
        assert_eq!(task.attempt_details.iter().map(|a| a.outcome.as_str()).collect::<Vec<_>>(), ["failed", "waiting"]);
        assert_eq!(rig.prompts.lock().unwrap().len(), 1, "the paid step did not start");

        runner.decide(&id, true, Some(500_000)).unwrap().unwrap().await.unwrap();
        let task = detail(&rig, &id);
        assert_eq!(task.summary.state, "checks_passed");
        assert_eq!(task.attempt_details.len(), 2, "the approved attempt is the one that ran");
        assert!(
            rig.prompts.lock().unwrap()[1].contains("did not pass"),
            "the failure is still handed on after the wait"
        );
    }

    #[tokio::test]
    async fn the_app_is_told_where_each_runner_stopped() {
        let rig = rig(&[]);
        let seen: Arc<Mutex<Vec<String>>> = Arc::default();
        let log = seen.clone();
        let runner = Runner::new(
            rig.db.clone(),
            rig.worktrees.clone(),
            fake(vec![Act::Write("x.txt", "x")], rig.prompts.clone()),
        )
        .with_notify(Arc::new(move |_, state| log.lock().unwrap().push(state.to_string())));
        let paid = create(&rig, vec![step("claude-code", "paid", 3)]);
        runner.start(&paid).unwrap().await.unwrap();
        let free = create(&rig, vec![step("codex", "subscription", 3)]);
        runner.start(&free).unwrap().await.unwrap();
        assert_eq!(*seen.lock().unwrap(), ["waiting_approval", "needs_review"]);
    }

    #[tokio::test]
    async fn an_approval_needs_a_cap_and_shows_what_the_agent_cost_before() {
        let rig = rig(&[]);
        let runner = Runner::new(
            rig.db.clone(),
            rig.worktrees.clone(),
            fake(vec![Act::Costly("x.txt", "x", 0.12), Act::Costly("y.txt", "y", 0.30)], rig.prompts.clone()),
        );
        let first = create(&rig, vec![step("claude-code", "paid", 3)]);
        runner.start(&first).unwrap().await.unwrap();
        let task = detail(&rig, &first);
        let approval = task.approval.unwrap();
        assert_eq!(
            (approval.estimate.runs, approval.estimate.high_micro_usd),
            (0, None),
            "no earlier run: no estimate"
        );
        assert_eq!(approval.suggested_cap_usd, 0.5);
        assert!(approval.enforced_by_agent, "Claude Code stops at --max-budget-usd itself");
        assert_eq!(runner.decide(&first, true, None).unwrap_err(), "Set a spending cap for this paid step");
        runner.decide(&first, true, Some(500_000)).unwrap().unwrap().await.unwrap();
        let task = detail(&rig, &first);
        assert_eq!(task.attempt_details[0].cap_usd, Some(0.5));
        assert_eq!(task.attempt_details[0].agent_estimate_usd, Some(0.12));

        let second = create(&rig, vec![step("claude-code", "paid", 3)]);
        runner.start(&second).unwrap().await.unwrap();
        let approval = detail(&rig, &second).approval.unwrap();
        assert_eq!(
            (approval.estimate.runs, approval.estimate.low_micro_usd, approval.estimate.high_micro_usd),
            (1, Some(120_000), Some(120_000))
        );
        assert_eq!(approval.suggested_cap_usd, 0.2, "half again the highest, rounded up to 5 cents");

        // Another run of the same agent is recorded while the approval waits: the estimate it showed is stale.
        lock(&rig.db)
            .execute(
                "UPDATE attempts SET agent_estimate_micro_usd = 900000 WHERE agent_estimate_micro_usd = 120000",
                [],
            )
            .unwrap();
        assert!(runner.decide(&second, true, Some(200_000)).unwrap_err().contains("changed since you were asked"));
        let approval = detail(&rig, &second).approval.unwrap();
        assert_eq!(approval.estimate.high_micro_usd, Some(900_000), "a new approval is asked with the new estimate");
        let states: Vec<String> = lock(&rig.db)
            .prepare("SELECT state FROM approvals ORDER BY created_at")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|s| s.unwrap())
            .collect();
        assert_eq!(states, ["approved", "invalidated", "pending"]);
        runner.decide(&second, true, Some(1_500_000)).unwrap().unwrap().await.unwrap();
        assert_eq!(detail(&rig, &second).summary.state, "needs_review");
    }

    #[tokio::test]
    async fn a_paid_step_over_its_cap_stops_the_ladder() {
        let rig = rig(&[good_check()]);
        let runner = Runner::new(
            rig.db.clone(),
            rig.worktrees.clone(),
            fake(
                vec![Act::Costly("result.txt", "bad\n", 0.40), Act::Write("result.txt", "good\n")],
                rig.prompts.clone(),
            ),
        );
        let id = create(&rig, vec![step("codex", "paid", 3), step("opencode", "free", 1)]);
        runner.start(&id).unwrap().await.unwrap();
        runner.decide(&id, true, Some(100_000)).unwrap().unwrap().await.unwrap();
        let task = detail(&rig, &id);
        assert_eq!(task.summary.state, "blocked");
        let reason = task.reason.unwrap();
        assert!(reason.contains("$0.40") && reason.contains("$0.10") && reason.contains("stopped"), "{reason}");
        assert_eq!(task.attempt_details.len(), 1, "the free step after it did not run");
    }

    #[tokio::test]
    async fn a_step_through_pyggs_gateway_is_costed_and_capped_by_pyggs_ledger_not_by_the_agent() {
        // Priced: the ledger's $0.40 is the step's cost, over a $0.10 cap, though the agent estimated $0.
        let rig = rig(&[good_check()]);
        let runner = Runner::new(
            rig.db.clone(),
            rig.worktrees.clone(),
            fake(
                vec![
                    Act::ThroughPygg("result.txt", "bad\n", Some(400_000), rig.db.clone()),
                    Act::Write("result.txt", "good\n"),
                ],
                rig.prompts.clone(),
            ),
        );
        let id = create(&rig, vec![step("opencode", "paid", 2), step("opencode", "free", 1)]);
        runner.start(&id).unwrap().await.unwrap();
        runner.decide(&id, true, Some(100_000)).unwrap().unwrap().await.unwrap();
        let task = detail(&rig, &id);
        assert_eq!(task.summary.state, "blocked");
        let reason = task.reason.clone().unwrap();
        assert!(reason.contains("Pygg's ledger") && reason.contains("$0.40") && reason.contains("$0.10"), "{reason}");
        let a = &task.attempt_details[0];
        assert_eq!(
            (a.cost_source.as_deref(), a.agent_estimate_usd, a.unpriced_requests),
            (Some("pygg"), Some(0.40), 0)
        );
        assert_eq!(a.input_tokens, Some(1000), "the gateway's entry, not the agent's own as well");
        assert_eq!((task.summary.cost.estimated_usd, task.summary.cost.unknown_calls), (0.40, 0));
        // The agent's own entry for the run stays in the ledger, off the attempt.
        let agent_rows: i64 = lock(&rig.db)
            .query_row(
                "SELECT COUNT(*) FROM usage_events WHERE route_mode = 'agent' \
            AND attempt_id IS NULL AND session_key = ?1",
                [&a.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(agent_rows, 1);

        // Unpriced: unknown, never $0; the cap could not be checked, and today's budget counts the whole cap.
        drop(rig);
        let rig = self::rig(&[good_check()]);
        let runner = Runner::new(
            rig.db.clone(),
            rig.worktrees.clone(),
            fake(vec![Act::ThroughPygg("result.txt", "good\n", None, rig.db.clone())], rig.prompts.clone()),
        );
        let id = create(&rig, vec![step("opencode", "paid", 2)]);
        runner.start(&id).unwrap().await.unwrap();
        runner.decide(&id, true, Some(100_000)).unwrap().unwrap().await.unwrap();
        let task = detail(&rig, &id);
        let reason = task.reason.clone().unwrap();
        assert!(
            reason.contains("1 of this step's requests through Pygg had no price")
                && reason.contains("could not be checked"),
            "{reason}"
        );
        let a = &task.attempt_details[0];
        assert_eq!((a.cost_source.as_deref(), a.agent_estimate_usd, a.unpriced_requests), (Some("pygg"), None, 1));
        assert_eq!(task.summary.cost.unknown_calls, 1);
        let (spent, _, unknown) = used_today(&lock(&rig.db), None).unwrap();
        assert_eq!((spent, unknown), (100_000, 1));
    }

    #[tokio::test]
    async fn a_cap_that_cannot_be_checked_is_said_so() {
        let rig = rig(&[]);
        let runner = Runner::new(
            rig.db.clone(),
            rig.worktrees.clone(),
            fake(vec![Act::Write("x.txt", "x")], rig.prompts.clone()),
        );
        let id = create(&rig, vec![step("codex", "paid", 3)]);
        runner.start(&id).unwrap().await.unwrap();
        runner.decide(&id, true, Some(100_000)).unwrap().unwrap().await.unwrap();
        let task = detail(&rig, &id);
        assert!(task.reason.unwrap().contains("reported no cost, so your cap for this step could not be checked"));
    }

    #[tokio::test]
    async fn the_daily_budget_counts_paid_steps_and_bounds_approvals() {
        let rig = rig(&[]);
        let runner = Runner::new(
            rig.db.clone(),
            rig.worktrees.clone(),
            fake(
                vec![Act::Costly("x.txt", "x", 0.30), Act::Write("y.txt", "y"), Act::Write("z.txt", "z")],
                rig.prompts.clone(),
            ),
        );
        set_daily_budget(&lock(&rig.db), Some(1_000_000)).unwrap();

        // A paid step that reported its cost counts at it; a free one does not count.
        let first = create(&rig, vec![step("claude-code", "paid", 3)]);
        runner.start(&first).unwrap().await.unwrap();
        runner.decide(&first, true, Some(500_000)).unwrap().unwrap().await.unwrap();
        let free = create(&rig, vec![step("opencode", "free", 1)]);
        runner.start(&free).unwrap().await.unwrap();
        assert_eq!(
            day_budget(&lock(&rig.db)).unwrap(),
            DayBudget { recorded_usd: 0.30, reserved_usd: 0.0, limit_usd: Some(1.0), unknown_calls: 0 }
        );

        // A paid step that reported nothing counts at its whole cap.
        let second = create(&rig, vec![step("codex", "paid", 3)]);
        runner.start(&second).unwrap().await.unwrap();
        let approval = detail(&rig, &second).approval.unwrap();
        assert_eq!(approval.remaining_usd, Some(0.70));
        assert!(
            runner.decide(&second, true, Some(800_000)).unwrap_err().contains("$0.70"),
            "a cap cannot pass what is left"
        );
        runner.decide(&second, true, Some(700_000)).unwrap().unwrap().await.unwrap();
        let budget = day_budget(&lock(&rig.db)).unwrap();
        assert_eq!((budget.recorded_usd, budget.unknown_calls), (1.0, 1));

        // Used up: the next paid step is refused before it asks.
        let third = create(&rig, vec![step("claude-code", "paid", 3)]);
        runner.start(&third).unwrap().await.unwrap();
        let task = detail(&rig, &third);
        assert_eq!(task.summary.state, "blocked");
        assert!(task.reason.unwrap().contains("today's budget for paid steps is used"));
        assert!(task.attempt_details.is_empty(), "nothing was asked or run");
    }

    #[tokio::test]
    async fn changing_a_budget_invalidates_a_waiting_approval() {
        let rig = rig(&[]);
        let runner = Runner::new(
            rig.db.clone(),
            rig.worktrees.clone(),
            fake(vec![Act::Write("x.txt", "x")], rig.prompts.clone()),
        );
        let id = create(&rig, vec![step("claude-code", "paid", 3)]);
        runner.start(&id).unwrap().await.unwrap();
        set_workspace_budget(&lock(&rig.db), &rig.workspace, Some(2_000_000)).unwrap();
        assert!(runner.decide(&id, true, Some(500_000)).unwrap_err().contains("budget changed"));
        assert_eq!(detail(&rig, &id).approval.unwrap().remaining_usd, Some(2.0));
        runner.decide(&id, true, Some(500_000)).unwrap().unwrap().await.unwrap();
        assert_eq!(detail(&rig, &id).summary.state, "needs_review");
        assert_eq!(workspace(&lock(&rig.db), &rig.workspace).unwrap().daily_budget_usd, Some(2.0));
    }

    #[tokio::test]
    async fn in_a_sensitive_workspace_every_step_asks_first() {
        let rig = rig(&[]);
        let runner = Runner::new(
            rig.db.clone(),
            rig.worktrees.clone(),
            fake(vec![Act::Write("x.txt", "x")], rig.prompts.clone()),
        );
        assert!(set_privacy(&lock(&rig.db), &rig.workspace, "secret").is_err());
        set_privacy(&lock(&rig.db), &rig.workspace, "sensitive").unwrap();
        let id = create(&rig, vec![step("opencode", "free", 1)]);
        runner.start(&id).unwrap().await.unwrap();
        let task = detail(&rig, &id);
        assert_eq!(task.summary.state, "waiting_approval");
        assert!(task.reason.unwrap().contains("sensitive"));
        let approval = task.approval.unwrap();
        assert!(!approval.paid, "a free step asks only because of the workspace");
        assert!(rig.prompts.lock().unwrap().is_empty(), "no agent saw the code before the user agreed");
        runner.decide(&id, true, None).unwrap().unwrap().await.unwrap();
        assert_eq!(detail(&rig, &id).summary.state, "needs_review");
        assert_eq!(workspace(&lock(&rig.db), &rig.workspace).unwrap().privacy_class, "sensitive");
    }

    #[tokio::test]
    async fn the_result_exports_as_a_patch_git_applies() {
        let rig = rig(&[]);
        let runner = Runner::new(
            rig.db.clone(),
            rig.worktrees.clone(),
            fake(vec![Act::Write("app.txt", "two\n")], rig.prompts.clone()),
        );
        let id = create(&rig, vec![step("codex", "subscription", 3)]);
        runner.start(&id).unwrap().await.unwrap();
        let (name, patch) = result_patch(&lock(&rig.db), &id).unwrap();
        assert!(name.starts_with("make-result-txt-say-good-") && name.ends_with(".patch"), "{name}");
        assert!(patch.starts_with("Pygg task: Make result.txt say good\n") && patch.contains("+two"), "{patch}");
        let file = rig.repo.parent().unwrap().join(&name);
        std::fs::write(&file, &patch).unwrap();
        let apply = std::process::Command::new("git")
            .arg("-C")
            .arg(&rig.repo)
            .args(["apply", "--check"])
            .arg(&file)
            .output()
            .unwrap();
        assert!(apply.status.success(), "{}", String::from_utf8_lossy(&apply.stderr));
        assert_eq!(detail(&rig, &id).summary.state, "needs_review", "exporting leaves the result open");
    }

    #[tokio::test]
    async fn declining_a_paid_step_blocks_the_task() {
        let rig = rig(&[]);
        let runner = Runner::new(rig.db.clone(), rig.worktrees.clone(), fake(vec![], rig.prompts.clone()));
        let id = create(&rig, vec![step("claude-code", "paid", 3)]);
        runner.start(&id).unwrap().await.unwrap();
        assert!(runner.decide(&id, false, None).unwrap().is_none());
        let task = detail(&rig, &id);
        assert_eq!((task.summary.state.as_str(), task.attempt_details[0].outcome.as_str()), ("blocked", "canceled"));
        assert!(rig.prompts.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn stopping_a_running_task_ends_its_attempt() {
        let rig = rig(&[]);
        let runner =
            Runner::new(rig.db.clone(), rig.worktrees.clone(), fake(vec![Act::WaitForStop], rig.prompts.clone()));
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
            let Opened::Run(_) = open_attempt(&conn, &id, &row.stage_id, 0, &row.policy.steps[0]).unwrap() else {
                panic!()
            };
            assert_eq!(recover(&conn, &rig.worktrees).unwrap(), 1);
        }
        assert!(!leftover.exists(), "a worktree nobody needs is removed");
        let task = detail(&rig, &id);
        assert_eq!(
            (task.summary.state.as_str(), task.attempt_details[0].outcome.as_str()),
            ("interrupted", "canceled")
        );

        let runner = Runner::new(
            rig.db.clone(),
            rig.worktrees.clone(),
            fake(vec![Act::Write("x.txt", "x")], rig.prompts.clone()),
        );
        runner.retry(&id).unwrap().unwrap().await.unwrap();
        let task = detail(&rig, &id);
        assert_eq!(task.summary.state, "needs_review");
        assert_eq!(
            task.attempt_details.iter().map(|a| (a.number, a.step)).collect::<Vec<_>>(),
            [(1, 0), (2, 0)],
            "the same step runs again"
        );
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
        let base = NewTask {
            workspace_id: rig.workspace.clone(),
            title: None,
            prompt: "  ".into(),
            strategy: "free_first".into(),
            steps: vec![step("codex", "subscription", 3)],
            allow_shell: false,
            timeout_secs: None,
            handoff: None,
            answer: false,
        };
        assert_eq!(create_task(&conn, &base).unwrap_err(), "Describe the task");
        let prompt = "Fix it".to_string();
        assert!(create_task(&conn, &NewTask { prompt: prompt.clone(), steps: vec![], ..base.clone() }).is_err());
        assert!(create_task(
            &conn,
            &NewTask { prompt: prompt.clone(), steps: vec![step("cursor", "paid", 3)], ..base.clone() }
        )
        .is_err());
        assert!(create_task(
            &conn,
            &NewTask { prompt: prompt.clone(), steps: vec![step("codex", "paid", 7)], ..base.clone() }
        )
        .is_err());
        assert!(
            create_task(&conn, &NewTask { prompt: prompt.clone(), strategy: "yolo".into(), ..base.clone() }).is_err()
        );
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
        assert_eq!(
            ws.checks.iter().map(|c| (c.command.as_str(), c.approved)).collect::<Vec<_>>(),
            [("cargo check", false), ("cargo test", false)],
            "detected checks are suggestions"
        );
        assert!(add_workspace(&conn, &repo.to_string_lossy(), None).unwrap_err().contains("already"));
        set_check(&conn, &ws.id, "cargo test", true).unwrap();
        add_check(&conn, &ws.id, "cargo clippy").unwrap();
        assert!(add_check(&conn, &ws.id, "a\nb").is_err());
        let ws = workspace(&conn, &ws.id).unwrap();
        assert_eq!(
            ws.checks.iter().filter(|c| c.approved).map(|c| c.command.as_str()).collect::<Vec<_>>(),
            ["cargo test", "cargo clippy"]
        );
        remove_workspace(&conn, &ws.id).unwrap();
        assert!(list_workspaces(&conn).unwrap().is_empty());
    }
}
