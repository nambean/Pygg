//! The board (ADR-042, docs/design/BRAIN.md §3): an approved brainstorm's cards run as a strict Kanban. Columns
//! are fixed; a card is pulled into Running only while Running has room under the workspace's WIP limit (one by
//! default, never removed), so at most that many paid steps are in flight; leaving a column is defined by the
//! task engine's states, never judged; Blocked is a column with a reason and the time spent there; nothing times
//! out or approves itself.
//!
//! A card has a task only once the board pulled it: before that it sits in Discovery (a question or a card it
//! depends on is not done, or the user held it) or Ready. The tick (`tick`) runs in the app every few seconds and
//! after every board action: it reads each card's task, settles the lanes, and pulls Ready cards while there is
//! room, making one task per card with one step per model of its ladder (the card's model first, its fallbacks
//! after), so a failed attempt moves the task to the next model as the engine already does.
use crate::db::now_ms;
use crate::gateway::brain;
use crate::gateway::store;
use crate::tasks::{self, NewTask, Step, TaskSummary};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

pub type Result<T> = std::result::Result<T, String>;

pub const LANES: &[&str] = &["discovery", "ready", "running", "checking", "blocked", "review", "done", "discarded"];
/// Settings keys for the Brain Storming defaults (BRAIN.md 4.7b); a workspace's `board_settings` row wins.
pub const SETTING_WIP_RUNNING: &str = "brain.wip_running";
pub const SETTING_WIP_CHECKING: &str = "brain.wip_checking";
pub const SETTING_RUNNER: &str = "brain.runner";
pub const SETTING_REMEMBER_PLANNER: &str = "brain.remember_planner";
pub const SETTING_CHANGE_SCOPE: &str = "brain.change_scope";
/// Measures are shown once this many cards are done (BRAIN.md 4.6).
pub const MEASURED_FROM: usize = 5;
/// The task states from which a blocked card may run again (`act`'s `rerun`).
const RERUNNABLE: &[&str] = &["check_failed", "failed", "blocked", "canceled"];

fn err(e: rusqlite::Error) -> String {
    e.to_string()
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub id: String,
    pub workspace_id: String,
    pub brainstorm_id: Option<String>,
    pub card_id: String,
    pub title: String,
    pub role: String,
    pub done: String,
    pub prompt: String,
    pub depends_on: Vec<String>,
    pub ladder: Vec<String>,
    pub agent: String,
    pub runner: String,
    pub class: String,
    pub lane: String,
    pub held: bool,
    pub task_id: Option<String>,
    pub blocked_reason: Option<String>,
    pub blocked_since: Option<i64>,
    pub blocked_ms: i64,
    pub ready_at: Option<i64>,
    pub started_at: Option<i64>,
    pub done_at: Option<i64>,
    pub position: i64,
    pub created_at: i64,
    pub updated_at: i64,
    /// The card's task as the Tasks list shows it, when it has one.
    #[serde(skip_deserializing)]
    pub task: Option<TaskSummary>,
}

const COLUMNS: &str = "id, workspace_id, brainstorm_id, card_id, title, role, done, prompt, depends_on_json, ladder_json, agent, runner, \
                       class, lane, held, task_id, blocked_reason, blocked_since, blocked_ms, ready_at, started_at, done_at, position, \
                       created_at, updated_at";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Card> {
    let deps: String = r.get(8)?;
    let ladder: String = r.get(9)?;
    Ok(Card {
        id: r.get(0)?,
        workspace_id: r.get(1)?,
        brainstorm_id: r.get(2)?,
        card_id: r.get(3)?,
        title: r.get(4)?,
        role: r.get(5)?,
        done: r.get(6)?,
        prompt: r.get(7)?,
        depends_on: serde_json::from_str(&deps).unwrap_or_default(),
        ladder: serde_json::from_str(&ladder).unwrap_or_default(),
        agent: r.get(10)?,
        runner: r.get(11)?,
        class: r.get(12)?,
        lane: r.get(13)?,
        held: r.get::<_, i64>(14)? != 0,
        task_id: r.get(15)?,
        blocked_reason: r.get(16)?,
        blocked_since: r.get(17)?,
        blocked_ms: r.get(18)?,
        ready_at: r.get(19)?,
        started_at: r.get(20)?,
        done_at: r.get(21)?,
        position: r.get(22)?,
        created_at: r.get(23)?,
        updated_at: r.get(24)?,
        task: None,
    })
}

pub fn card(conn: &Connection, id: &str) -> Result<Option<Card>> {
    conn.query_row(&format!("SELECT {COLUMNS} FROM board_cards WHERE id = ?1"), [id], row).optional().map_err(err)
}

/// A workspace's cards in board order: by lane, expedite first, then position, then age.
pub fn cards_of(conn: &Connection, workspace_id: &str) -> Result<Vec<Card>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {COLUMNS} FROM board_cards WHERE workspace_id = ?1 \
             ORDER BY CASE class WHEN 'expedite' THEN 0 ELSE 1 END, position, created_at"
        ))
        .map_err(err)?;
    let rows = stmt.query_map([workspace_id], row).map_err(err)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(err)
}

// ---------- settings ----------

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    pub wip_running: u32,
    pub wip_checking: u32,
    /// Whether this workspace has limits of its own (else the Brain Storming defaults).
    pub own: bool,
}

fn setting_u32(conn: &Connection, key: &str, default: u32) -> u32 {
    store::setting(conn, key).ok().flatten().and_then(|v| v.parse().ok()).filter(|v| *v >= 1).unwrap_or(default)
}

/// The workspace's limits: its own row, else the defaults in `settings`, else 1 and 1.
pub fn limits(conn: &Connection, workspace_id: &str) -> Result<Limits> {
    let own: Option<(i64, i64)> = conn
        .query_row(
            "SELECT wip_running, wip_checking FROM board_settings WHERE workspace_id = ?1",
            [workspace_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(err)?;
    Ok(match own {
        Some((r, c)) => Limits { wip_running: r.max(1) as u32, wip_checking: c.max(1) as u32, own: true },
        None => Limits {
            wip_running: setting_u32(conn, SETTING_WIP_RUNNING, 1),
            wip_checking: setting_u32(conn, SETTING_WIP_CHECKING, 1),
            own: false,
        },
    })
}

/// Sets a workspace's own limits (`None` returns it to the defaults). A limit is raised or set, never below 1.
pub fn set_limits(conn: &Connection, workspace_id: &str, wanted: Option<(u32, u32)>) -> Result<Limits> {
    match wanted {
        Some((r, c)) => {
            if r < 1 || c < 1 {
                return Err("A WIP limit is never below 1".into());
            }
            conn.execute(
                "INSERT INTO board_settings (workspace_id, wip_running, wip_checking, updated_at) VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT(workspace_id) DO UPDATE SET wip_running = ?2, wip_checking = ?3, updated_at = ?4",
                params![workspace_id, r, c, now_ms()],
            )
            .map_err(err)?;
        }
        None => {
            conn.execute("DELETE FROM board_settings WHERE workspace_id = ?1", [workspace_id]).map_err(err)?;
        }
    }
    limits(conn, workspace_id)
}

// ---------- from a brainstorm ----------

/// An approved brainstorm's kept cards become board cards of the workspace, in Discovery; the tick settles them.
/// Every card needs a model; the ladder is the card's model then its fallbacks.
/// The branch a brainstorm's cards build on: `str/brain-<requirement>-<id>`.
pub fn branch_for(b: &brain::Brainstorm) -> String {
    let first = b.requirement.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("brainstorm");
    crate::isolation::branch_name(&format!("brain {first}"), &b.id)
}

/// The branch of a card's brainstorm, if it has one.
fn branch_of(conn: &Connection, brainstorm_id: Option<&str>) -> Result<Option<String>> {
    let Some(id) = brainstorm_id else { return Ok(None) };
    conn.query_row("SELECT branch FROM brainstorms WHERE id = ?1", [id], |r| r.get(0))
        .optional()
        .map_err(err)
        .map(Option::flatten)
}

pub fn add_cards(conn: &Connection, b: &brain::Brainstorm, workspace_id: &str, agent: &str) -> Result<Vec<String>> {
    let now = now_ms();
    // One branch for the brainstorm, at the workspace's HEAD: each card starts from its tip, and keeping a card adds
    // to it (ADR-042 as amended), so a card sees what the earlier ones built. The user's checkout is not touched.
    let ws = tasks::workspace(conn, workspace_id)?;
    let branch = branch_for(b);
    crate::isolation::ensure_branch(std::path::Path::new(&ws.path), &branch).map_err(|e| e.to_string())?;
    conn.execute("UPDATE brainstorms SET branch = ?2 WHERE id = ?1", params![b.id, branch]).map_err(err)?;
    let mut ids = Vec::new();
    for (i, c) in b.proposal.cards.iter().filter(|c| !c.dropped).enumerate() {
        let model = c.model.clone().ok_or_else(|| format!("card {} has no eligible model", c.id))?;
        let ladder: Vec<String> =
            std::iter::once(model).chain(c.fallbacks.iter().map(|f| f.model.clone())).take(tasks::MAX_STEPS).collect();
        let prompt = format!(
            "{}\n\nThis card: {} (role: {}).\nDone when: {}\n\nThe whole requirement, for context:\n{}",
            c.title, c.title, c.role, c.done, b.requirement
        );
        let id = format!("bc_{}", &uuid::Uuid::new_v4().simple().to_string()[..10]);
        conn.execute(
            "INSERT INTO board_cards (id, workspace_id, brainstorm_id, card_id, title, role, done, prompt, depends_on_json, ladder_json, agent, \
             runner, class, lane, position, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 'discovery', ?14, ?15, ?15)",
            params![
                id,
                workspace_id,
                b.id,
                c.id,
                c.title,
                c.role,
                c.done,
                prompt,
                serde_json::to_string(&c.depends_on).map_err(|e| e.to_string())?,
                serde_json::to_string(&ladder).map_err(|e| e.to_string())?,
                agent,
                c.runner,
                c.class,
                (c.wave as i64) * 1000 + i as i64,
                now
            ],
        )
        .map_err(err)?;
        ids.push(id);
    }
    Ok(ids)
}

// ---------- the tick ----------

/// One task's state as the board reads it.
struct TaskState {
    state: String,
    phase: Option<String>,
    result: Option<String>,
}

fn task_state(conn: &Connection, id: &str) -> Result<Option<TaskState>> {
    conn.query_row("SELECT state, phase, result FROM tasks WHERE id = ?1", [id], |r| {
        Ok(TaskState { state: r.get(0)?, phase: r.get(1)?, result: r.get(2)? })
    })
    .optional()
    .map_err(err)
}

/// The lane a task's state puts its card in, and the reason when it is Blocked (BRAIN.md 4.6).
fn lane_of(t: &TaskState) -> (&'static str, Option<&'static str>) {
    if t.result.as_deref() == Some("kept") {
        return ("done", None);
    }
    if t.result.as_deref() == Some("discarded") {
        return ("discarded", None);
    }
    match t.state.as_str() {
        // A run held before its checks (Checking full) is still Running.
        "running" => (if t.phase.as_deref() == Some("checking") { "checking" } else { "running" }, None),
        "waiting_approval" => ("blocked", Some("Waiting for approval")),
        "needs_revalidation" => ("blocked", Some("Approval needs a new look")),
        "blocked" => ("blocked", Some("Over the cap or the budget")),
        "interrupted" => ("blocked", Some("Interrupted")),
        "canceled" => ("blocked", Some("Stopped")),
        "check_failed" => ("blocked", Some("Check failed on every model")),
        "checks_passed" | "needs_review" | "manually_accepted" => ("review", None),
        _ => ("blocked", Some("Unknown state")),
    }
}

/// Whether a card's dependencies are met: every question answered, every card it names done. `Err` names what is
/// missing for good (a card dropped before approval); `Ok(false)` says it waits.
fn dependencies_met(
    card: &Card, siblings: &[Card], questions: &[brain::Question],
) -> std::result::Result<bool, String> {
    for dep in &card.depends_on {
        if dep.starts_with('q') {
            match questions.iter().find(|q| q.id == *dep) {
                Some(q) if q.answered => {}
                Some(_) => return Ok(false),
                None => {}
            }
        } else {
            match siblings.iter().find(|s| s.card_id == *dep && s.brainstorm_id == card.brainstorm_id) {
                Some(s) if s.lane == "done" => {}
                Some(s) if s.lane == "discarded" => return Err(format!("Needs {dep}, whose result was discarded")),
                Some(_) => return Ok(false),
                None => return Err(format!("Needs {dep}, which was dropped before approval")),
            }
        }
    }
    Ok(true)
}

/// Writes a card's lane, keeping the Blocked bookkeeping: the time it entered Blocked, and the time it had spent
/// there before.
fn set_lane(conn: &Connection, card: &Card, lane: &str, reason: Option<&str>) -> Result<()> {
    let now = now_ms();
    if card.lane == lane && card.blocked_reason.as_deref() == reason {
        return Ok(());
    }
    let (since, ms) = match (card.lane.as_str(), lane) {
        ("blocked", "blocked") => (card.blocked_since, card.blocked_ms),
        (_, "blocked") => (Some(now), card.blocked_ms),
        ("blocked", _) => (None, card.blocked_ms + card.blocked_since.map(|s| now - s).unwrap_or(0)),
        _ => (None, card.blocked_ms),
    };
    let ready_at = if lane == "ready" { card.ready_at.or(Some(now)) } else { card.ready_at };
    let done_at = if lane == "done" { card.done_at.or(Some(now)) } else { card.done_at };
    conn.execute(
        "UPDATE board_cards SET lane = ?2, blocked_reason = ?3, blocked_since = ?4, blocked_ms = ?5, ready_at = ?6, done_at = ?7, \
         updated_at = ?8 WHERE id = ?1",
        params![card.id, lane, reason, since, ms, ready_at, done_at, now],
    )
    .map(|_| ())
    .map_err(err)
}

/// The task for a card: one step per model of its ladder, the card's model first. Only OpenCode through its Pygg
/// provider runs on the picked models; another agent gets one step and is told the picks.
fn task_for(conn: &Connection, card: &Card) -> Result<NewTask> {
    let models = store::list_models(conn, None)?;
    let providers = store::list_providers(conn)?;
    let mut steps = Vec::new();
    let mut off = Vec::new();
    for model in &card.ladder {
        let row =
            model.split_once('/').and_then(|(p, m)| models.iter().find(|x| x.provider_id == p && x.model_id == m));
        // Frozen at approval, the ladder may name a model since turned off (a catalog rule, a connection removed):
        // the gateway would refuse it and the agent would fail with nothing to say, so the step is left out here.
        if let Some(m) = row {
            let on = m.enabled && providers.iter().any(|p| p.id == m.provider_id && p.enabled);
            if !on && card.agent == "opencode" {
                off.push(model.clone());
                continue;
            }
        }
        let tier = row
            .and_then(|m| m.capabilities.tier.as_deref())
            .and_then(|t| t.strip_prefix('T'))
            .and_then(|t| t.parse().ok())
            .unwrap_or(0);
        let (step_model, billing) = if card.agent == "opencode" {
            (Some(format!("pygg/{model}")), row.map(|m| m.billing.clone()).unwrap_or_else(|| "unknown".into()))
        } else {
            (None, "unknown".to_string())
        };
        steps.push(Step { agent: card.agent.clone(), model: step_model, tier, billing });
    }
    let mut prompt = card.prompt.clone();
    if card.agent != "opencode" {
        steps.truncate(1);
        prompt.push_str(&format!(
            "\n\nPygg picked {} for this card; this agent runs with its own sign-in and model.",
            card.ladder.join(", then ")
        ));
    }
    if steps.is_empty() {
        return Err(if off.is_empty() {
            format!("card {} has no model", card.card_id)
        } else {
            format!("No enabled model: {}", off.join(", "))
        });
    }
    Ok(NewTask {
        workspace_id: card.workspace_id.clone(),
        title: Some(card.title.clone()),
        prompt,
        strategy: "balanced".into(),
        steps,
        allow_shell: false,
        timeout_secs: None,
        handoff: None,
    })
}

/// Whether a task may enter its checks now (ADR-042, Checking's WIP limit): Checking has room under the
/// workspace's limit, less the held tasks of cards ahead of this one (expedite first, then position). A task that
/// is not a card's is never held. The runner asks before its checks and, told no, sets the phase `held` and asks
/// again until yes.
pub fn checking_room(conn: &Connection, task_id: &str) -> Result<bool> {
    let card: Option<(String, String, String, i64)> = conn
        .query_row("SELECT workspace_id, id, class, position FROM board_cards WHERE task_id = ?1", [task_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
        .optional()
        .map_err(err)?;
    let Some((workspace_id, card_id, class, position)) = card else { return Ok(true) };
    let limit = limits(conn, &workspace_id)?.wip_checking as i64;
    let checking: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE workspace_id = ?1 AND state = 'running' AND phase = 'checking'",
            [&workspace_id],
            |r| r.get(0),
        )
        .map_err(err)?;
    let ahead: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM board_cards c JOIN tasks t ON t.id = c.task_id \
             WHERE c.workspace_id = ?1 AND c.id <> ?2 AND t.state = 'running' AND t.phase = 'held' \
             AND ((c.class = 'expedite') > (?3 = 'expedite') OR ((c.class = 'expedite') = (?3 = 'expedite') AND c.position < ?4))",
            params![workspace_id, card_id, class, position],
            |r| r.get(0),
        )
        .map_err(err)?;
    Ok(ahead < limit - checking)
}

/// Settles every workspace's lanes and pulls Ready cards into Running under the limit, making their tasks. Returns
/// the ids of the tasks to start (the caller holds the runner).
pub fn tick(conn: &Connection) -> Result<Vec<String>> {
    tick_with(conn, true)
}

/// The tick without the pull: lanes follow their tasks, Ready cards stay in Ready. The app runs this while the
/// gateway is stopped, since a card's steps run through it (`pygg/<model>`) and would fail at once.
pub fn settle(conn: &Connection) -> Result<()> {
    tick_with(conn, false).map(|_| ())
}

fn tick_with(conn: &Connection, pull: bool) -> Result<Vec<String>> {
    let workspaces: Vec<String> = conn
        .prepare("SELECT DISTINCT workspace_id FROM board_cards WHERE lane NOT IN ('done', 'discarded')")
        .map_err(err)?
        .query_map([], |r| r.get(0))
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;
    let mut to_start = Vec::new();
    for ws in workspaces {
        to_start.extend(tick_workspace(conn, &ws, pull)?);
    }
    Ok(to_start)
}

fn tick_workspace(conn: &Connection, workspace_id: &str, pull: bool) -> Result<Vec<String>> {
    let mut questions: std::collections::HashMap<String, Vec<brain::Question>> = std::collections::HashMap::new();
    // Lanes from the tasks and the dependencies, in two passes so a card done this tick frees the ones behind it.
    for _pass in 0..2 {
        let current = cards_of(conn, workspace_id)?;
        for card in &current {
            if card.lane == "done" || card.lane == "discarded" {
                continue;
            }
            if let Some(task_id) = &card.task_id {
                match task_state(conn, task_id)? {
                    Some(t) => {
                        let (lane, reason) = lane_of(&t);
                        set_lane(conn, card, lane, reason)?;
                    }
                    None => set_lane(conn, card, "blocked", Some("Its task is gone"))?,
                }
                continue;
            }
            let qs = match &card.brainstorm_id {
                Some(b) => questions
                    .entry(b.clone())
                    .or_insert_with(|| {
                        brain::get(conn, b).ok().flatten().map(|b| b.proposal.questions).unwrap_or_default()
                    })
                    .clone(),
                None => Vec::new(),
            };
            match dependencies_met(card, &current, &qs) {
                Err(missing) => set_lane(conn, card, "blocked", Some(&missing))?,
                Ok(false) => set_lane(conn, card, "discovery", None)?,
                Ok(true) if card.held => set_lane(conn, card, "discovery", None)?,
                Ok(true) if card.runner == "ide" => set_lane(conn, card, "blocked", Some("Waiting for your agent"))?,
                Ok(true) => set_lane(conn, card, "ready", None)?,
            }
        }
    }
    // Pull: expedite first, then position, while Running has room.
    if !pull {
        return Ok(Vec::new());
    }
    let limits = limits(conn, workspace_id)?;
    let current = cards_of(conn, workspace_id)?;
    let running = current.iter().filter(|c| c.lane == "running").count() as u32;
    let room = limits.wip_running.saturating_sub(running) as usize;
    let mut started = Vec::new();
    for card in current.iter().filter(|c| c.lane == "ready" && c.runner == "engine").take(room) {
        let new = match task_for(conn, card) {
            Ok(new) => new,
            Err(reason) => {
                set_lane(conn, card, "blocked", Some(&reason))?;
                continue;
            }
        };
        let base = branch_of(conn, card.brainstorm_id.as_deref())?;
        let task_id = tasks::create_task_at(conn, &new, None, base.as_deref())?;
        conn.execute(
            "UPDATE tasks SET brainstorm_id = ?2, card_id = ?3, card_role = ?4 WHERE id = ?1",
            params![task_id, card.brainstorm_id, card.card_id, card.role],
        )
        .map_err(err)?;
        conn.execute(
            "UPDATE board_cards SET task_id = ?2, lane = 'running', started_at = ?3, updated_at = ?3 WHERE id = ?1",
            params![card.id, task_id, now_ms()],
        )
        .map_err(err)?;
        started.push(task_id);
    }
    Ok(started)
}

// ---------- actions ----------

/// What the user may do to a card by button (BRAIN.md 4.6): `expedite` (one at a time in the workspace),
/// `standard`, `hold` (send back to Discovery), `release` (to Ready), `drop` (discard a card without a task), and
/// `engine` (hand an IDE card to the engine).
pub fn act(conn: &Connection, card_id: &str, action: &str) -> Result<Card> {
    let c = card(conn, card_id)?.ok_or("Card not found")?;
    let now = now_ms();
    match action {
        "expedite" => {
            conn.execute("UPDATE board_cards SET class = 'standard', updated_at = ?2 WHERE workspace_id = ?1 AND class = 'expedite'", params![c.workspace_id, now])
                .map_err(err)?;
            conn.execute(
                "UPDATE board_cards SET class = 'expedite', updated_at = ?2 WHERE id = ?1",
                params![c.id, now],
            )
            .map_err(err)?;
        }
        "standard" => {
            conn.execute(
                "UPDATE board_cards SET class = 'standard', updated_at = ?2 WHERE id = ?1",
                params![c.id, now],
            )
            .map_err(err)?;
        }
        "hold" => {
            if c.task_id.is_some() {
                return Err("A card with a task cannot go back to Discovery; stop its task first".into());
            }
            conn.execute(
                "UPDATE board_cards SET held = 1, lane = 'discovery', updated_at = ?2 WHERE id = ?1",
                params![c.id, now],
            )
            .map_err(err)?;
        }
        "release" => {
            conn.execute("UPDATE board_cards SET held = 0, updated_at = ?2 WHERE id = ?1", params![c.id, now])
                .map_err(err)?;
        }
        "drop" => {
            if c.task_id.is_some() {
                return Err(
                    "A card with a task is not dropped here; discard its result or stop it in task detail".into()
                );
            }
            conn.execute(
                "UPDATE board_cards SET lane = 'discarded', updated_at = ?2 WHERE id = ?1",
                params![c.id, now],
            )
            .map_err(err)?;
        }
        "engine" => {
            conn.execute("UPDATE board_cards SET runner = 'engine', updated_at = ?2 WHERE id = ?1", params![c.id, now])
                .map_err(err)?;
        }
        // A card whose task failed (every step, a spending limit, a stop) runs again as a new task once the user has
        // fixed what blocked it: back to Ready, pulled under the limit like any card, from the brainstorm's branch as
        // it is now. The failed task stays in the history. Without this a failed card was a dead end.
        "rerun" => {
            let failed = match &c.task_id {
                Some(task) => task_state(conn, task)?
                    .is_some_and(|t| RERUNNABLE.contains(&t.state.as_str()) && t.result.is_none()),
                None => false,
            };
            if c.lane != "blocked" || !failed {
                return Err("Only a blocked card whose task failed or was stopped can run again".into());
            }
            let blocked = c.blocked_since.map_or(0, |since| (now - since).max(0));
            conn.execute(
                "UPDATE board_cards SET task_id = NULL, lane = 'ready', blocked_reason = NULL, blocked_since = NULL, \
                 blocked_ms = blocked_ms + ?2, started_at = NULL, updated_at = ?3 WHERE id = ?1",
                params![c.id, blocked, now],
            )
            .map_err(err)?;
        }
        other => return Err(format!("{other} is not a board action")),
    }
    card(conn, card_id)?.ok_or_else(|| "Card not found".into())
}

// ---------- the view ----------

#[derive(Clone, Debug, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Measures {
    /// Shown only once `MEASURED_FROM` cards are done.
    pub measured: bool,
    pub done: usize,
    pub done_today: usize,
    pub cycle_time_median_ms: Option<i64>,
    /// How many done cards the cycle time covers: those that passed through Ready with a time.
    pub cycle_timed: usize,
    pub lead_time_median_ms: Option<i64>,
    pub blocked_today_ms: i64,
    /// Known money per kept result, by role (ADR-029): what the done cards' tasks cost through the gateway.
    pub money: Vec<RoleMoney>,
}

/// What the kept results of one role cost: the done cards' tasks' priced requests, and how many had no price.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoleMoney {
    pub role: String,
    pub kept: usize,
    pub known_usd: f64,
    pub unknown_calls: u32,
    /// `known_usd / kept`; None when nothing is priced.
    pub usd_per_kept: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Board {
    pub workspace_id: String,
    pub workspace_name: String,
    pub workspace_path: String,
    pub limits: Limits,
    pub cards: Vec<Card>,
    pub measures: Measures,
}

fn median(mut v: Vec<i64>) -> Option<i64> {
    if v.is_empty() {
        return None;
    }
    v.sort_unstable();
    Some(v[v.len() / 2])
}

/// The workspace's board: its cards with their tasks, its limits, and what the board measures from what is
/// recorded already (BRAIN.md §3).
pub fn board(conn: &Connection, workspace_id: &str) -> Result<Board> {
    let ws = tasks::workspace(conn, workspace_id)?;
    let mut cards = cards_of(conn, workspace_id)?;
    for c in &mut cards {
        if let Some(id) = &c.task_id {
            c.task = tasks::task_summary(conn, id).ok();
        }
    }
    let now = now_ms();
    let day = now - now.rem_euclid(86_400_000);
    let done: Vec<&Card> = cards.iter().filter(|c| c.lane == "done").collect();
    let measures = Measures {
        measured: done.len() >= MEASURED_FROM,
        done: done.len(),
        done_today: done.iter().filter(|c| c.done_at.is_some_and(|d| d >= day)).count(),
        cycle_time_median_ms: median(done.iter().filter_map(|c| Some(c.done_at? - c.ready_at?)).collect()),
        cycle_timed: done.iter().filter(|c| c.ready_at.is_some() && c.done_at.is_some()).count(),
        lead_time_median_ms: median(done.iter().filter_map(|c| Some(c.done_at? - c.created_at)).collect()),
        blocked_today_ms: cards
            .iter()
            .map(|c| c.blocked_ms + c.blocked_since.map(|s| now - s.max(day)).unwrap_or(0).max(0))
            .sum(),
        money: {
            let mut by_role: std::collections::BTreeMap<String, RoleMoney> = std::collections::BTreeMap::new();
            for c in &done {
                let Some(task) = &c.task else { continue };
                let m = by_role.entry(c.role.clone()).or_insert_with(|| RoleMoney {
                    role: c.role.clone(),
                    kept: 0,
                    known_usd: 0.0,
                    unknown_calls: 0,
                    usd_per_kept: None,
                });
                m.kept += 1;
                m.known_usd += task.cost.actual_usd + task.cost.estimated_usd;
                m.unknown_calls += task.cost.unknown_calls;
            }
            by_role
                .into_values()
                .map(|mut m| {
                    m.usd_per_kept = (m.known_usd > 0.0).then(|| m.known_usd / m.kept as f64);
                    m
                })
                .collect()
        },
    };
    Ok(Board {
        workspace_id: ws.id,
        workspace_name: ws.name,
        workspace_path: ws.path,
        limits: limits(conn, workspace_id)?,
        cards,
        measures,
    })
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BoardSummary {
    pub workspace_id: String,
    pub workspace_name: String,
    pub cards: usize,
    pub attention: usize,
    /// The workspace's own WIP limits, when it set any (`board_settings`); None means the defaults.
    pub limits: Option<OwnLimits>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OwnLimits {
    pub wip_running: u32,
    pub wip_checking: u32,
}

/// The workspaces that have a board, with how many cards wait on the user (Blocked and Review).
pub fn list(conn: &Connection) -> Result<Vec<BoardSummary>> {
    let mut stmt = conn
        .prepare(
            "SELECT w.id, w.name, COUNT(c.id), SUM(CASE WHEN c.lane IN ('blocked', 'review') THEN 1 ELSE 0 END), \
             s.wip_running, s.wip_checking \
             FROM workspaces w JOIN board_cards c ON c.workspace_id = w.id AND c.lane <> 'discarded' \
             LEFT JOIN board_settings s ON s.workspace_id = w.id \
             GROUP BY w.id, w.name, s.wip_running, s.wip_checking ORDER BY w.name",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(BoardSummary {
                workspace_id: r.get(0)?,
                workspace_name: r.get(1)?,
                cards: r.get::<_, i64>(2)? as usize,
                attention: r.get::<_, Option<i64>>(3)?.unwrap_or(0) as usize,
                limits: match (r.get::<_, Option<i64>>(4)?, r.get::<_, Option<i64>>(5)?) {
                    (Some(running), Some(checking)) => {
                        Some(OwnLimits { wip_running: running.max(1) as u32, wip_checking: checking.max(1) as u32 })
                    }
                    _ => None,
                },
            })
        })
        .map_err(err)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::brain::{Brainstorm, Card as BrainCard, Fallback, Proposal, Question};

    fn workspace(conn: &Connection) -> String {
        let dir = std::env::temp_dir().join(format!("pygg-board-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = std::process::Command::new("git").args(["init", "-q"]).current_dir(&dir).output().unwrap();
        assert!(out.status.success());
        std::fs::write(dir.join("README.md"), "x").unwrap();
        for args in [vec!["add", "."], vec!["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", "start"]]
        {
            assert!(std::process::Command::new("git").args(&args).current_dir(&dir).output().unwrap().status.success());
        }
        tasks::add_workspace(conn, dir.to_str().unwrap(), Some("shop")).unwrap().id
    }

    fn brainstorm(cards: Vec<BrainCard>, questions: Vec<Question>) -> Brainstorm {
        Brainstorm {
            id: "b_1".into(),
            client_id: Some("k".into()),
            session: "s".into(),
            workspace_id: None,
            state: "proposed".into(),
            requirement: "Build a shop".into(),
            planner: Some("p/m".into()),
            prompt_version: "brain-v1".into(),
            revision: 1,
            proposal: Proposal { questions, cards, dropped_cards: 0 },
            raw_reply: None,
            note: None,
            error: None,
            created_at: 0,
            updated_at: 0,
            branch: None,
        }
    }

    fn bcard(id: &str, deps: &[&str], wave: u32, runner: &str) -> BrainCard {
        BrainCard {
            id: id.into(),
            title: format!("Card {id}"),
            role: "code".into(),
            done: "tests pass".into(),
            depends_on: deps.iter().map(|d| d.to_string()).collect(),
            model: Some("paid/mid".into()),
            reason: Some("tier".into()),
            estimate_micro_usd: Some(46_000),
            fallbacks: vec![Fallback { model: "free/gift".into(), reason: "tier".into() }],
            runner: runner.into(),
            class: "standard".into(),
            dropped: false,
            wave,
        }
    }

    fn insert_brainstorm(conn: &Connection, b: &Brainstorm) {
        conn.execute(
            "INSERT INTO brainstorms (id, client_id, session, state, requirement, prompt_version, revision, proposal_json, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, 0)",
            params![b.id, b.client_id, b.session, b.state, b.requirement, b.prompt_version, b.revision as i64, serde_json::to_string(&b.proposal).unwrap()],
        )
        .unwrap();
    }

    fn lane(conn: &Connection, ws: &str, card_id: &str) -> (String, Option<String>) {
        let c = cards_of(conn, ws).unwrap().into_iter().find(|c| c.card_id == card_id).unwrap();
        (c.lane, c.blocked_reason)
    }

    #[test]
    fn a_blocked_card_whose_task_failed_runs_again_as_a_new_task() {
        let conn = crate::db::open_in_memory().unwrap();
        let ws = workspace(&conn);
        let b = brainstorm(vec![bcard("c1", &[], 1, "engine")], vec![]);
        insert_brainstorm(&conn, &b);
        add_cards(&conn, &b, &ws, "opencode").unwrap();
        let first = tick(&conn).unwrap().remove(0);
        let card_id = cards_of(&conn, &ws).unwrap()[0].id.clone();
        assert!(act(&conn, &card_id, "rerun").is_err(), "a running card does not run again");
        conn.execute("UPDATE tasks SET state = 'check_failed' WHERE id = ?1", [&first]).unwrap();
        settle(&conn).unwrap();
        assert_eq!(lane(&conn, &ws, "c1").0, "blocked");
        let c = act(&conn, &card_id, "rerun").unwrap();
        assert_eq!((c.lane.as_str(), c.task_id.as_deref()), ("ready", None));
        let second = tick(&conn).unwrap().remove(0);
        assert_ne!(second, first);
        assert_eq!(cards_of(&conn, &ws).unwrap()[0].task_id.as_deref(), Some(second.as_str()));
        let kept: i64 = conn.query_row("SELECT count(*) FROM tasks WHERE id = ?1", [&first], |r| r.get(0)).unwrap();
        assert_eq!(kept, 1, "the failed task stays in the history");
    }

    #[test]
    fn approval_makes_the_brainstorms_branch_and_a_card_starts_from_its_tip() {
        let conn = crate::db::open_in_memory().unwrap();
        let ws = workspace(&conn);
        let b = brainstorm(vec![bcard("c1", &[], 1, "engine")], vec![]);
        insert_brainstorm(&conn, &b);
        add_cards(&conn, &b, &ws, "opencode").unwrap();
        let branch: String =
            conn.query_row("SELECT branch FROM brainstorms WHERE id = ?1", [&b.id], |r| r.get(0)).unwrap();
        assert_eq!(branch, branch_for(&b));
        assert!(branch.starts_with("str/brain-"), "{branch}");
        // The branch moves ahead, as when an earlier card's result was kept: the next card starts there.
        let repo = std::path::PathBuf::from(tasks::workspace(&conn, &ws).unwrap().path);
        let dir = std::env::temp_dir().join(format!("pygg-board-ahead-{}", uuid::Uuid::new_v4().simple()));
        let w = crate::isolation::create(&repo, &branch, &dir).unwrap();
        std::fs::write(w.path.join("built.txt"), "earlier card").unwrap();
        let commit = crate::isolation::commit_result(&w, "earlier card").unwrap();
        crate::isolation::add_to_branch(&repo, &branch, &commit, &w.base).unwrap().unwrap();
        crate::isolation::discard(&w).unwrap();
        let started = tick(&conn).unwrap();
        assert_eq!(started.len(), 1);
        let base: String =
            conn.query_row("SELECT base_revision FROM tasks WHERE id = ?1", [&started[0]], |r| r.get(0)).unwrap();
        assert_eq!(base, commit);
    }

    #[test]
    fn checking_has_a_limit_and_held_runs_take_the_room_expedite_first_then_in_order() {
        let conn = crate::db::open_in_memory().unwrap();
        let ws = workspace(&conn);
        let b = brainstorm(
            vec![bcard("c1", &[], 1, "engine"), bcard("c2", &[], 1, "engine"), bcard("c3", &[], 2, "engine")],
            vec![],
        );
        insert_brainstorm(&conn, &b);
        add_cards(&conn, &b, &ws, "opencode").unwrap();
        set_limits(&conn, &ws, Some((3, 1))).unwrap();
        assert_eq!(tick(&conn).unwrap().len(), 3);
        let task_of = |card_id: &str| {
            cards_of(&conn, &ws).unwrap().into_iter().find(|c| c.card_id == card_id).unwrap().task_id.unwrap()
        };
        let phase = |task: &str, phase: &str| {
            conn.execute("UPDATE tasks SET state = 'running', phase = ?2 WHERE id = ?1", params![task, phase]).unwrap();
        };
        let (t1, t2, t3) = (task_of("c1"), task_of("c2"), task_of("c3"));
        // One in Checking fills the limit of 1; room with the limit raised.
        phase(&t1, "checking");
        assert!(!checking_room(&conn, &t2).unwrap());
        set_limits(&conn, &ws, Some((3, 2))).unwrap();
        assert!(checking_room(&conn, &t2).unwrap());
        set_limits(&conn, &ws, Some((3, 1))).unwrap();
        // Checking empty, two held: the first by position goes, the other waits; an expedited card goes first.
        phase(&t1, "generating");
        phase(&t2, "held");
        phase(&t3, "held");
        assert!(checking_room(&conn, &t2).unwrap());
        assert!(!checking_room(&conn, &t3).unwrap());
        let c3 = cards_of(&conn, &ws).unwrap().into_iter().find(|c| c.card_id == "c3").unwrap();
        act(&conn, &c3.id, "expedite").unwrap();
        assert!(checking_room(&conn, &t3).unwrap());
        assert!(!checking_room(&conn, &t2).unwrap());
        // A task that is no card's is never held.
        let plain = tasks::create_task(&conn, &task_for(&conn, &c3).unwrap()).unwrap();
        assert!(checking_room(&conn, &plain).unwrap());
        // A held run is still in Running on the board.
        assert_eq!(lane(&conn, &ws, "c2").0, "running");
        let board = board(&conn, &ws).unwrap();
        assert!(board.measures.money.is_empty(), "no card is done yet");
    }

    #[test]
    fn a_model_turned_off_since_approval_is_left_out_of_the_ladder_and_a_ladder_of_none_blocks() {
        let conn = crate::db::open_in_memory().unwrap();
        let ws = workspace(&conn);
        let p = crate::gateway::store::add_provider(
            &conn,
            &crate::gateway::store::NewProvider {
                kind: "openrouter",
                label: "openrouter",
                endpoint: "https://openrouter.ai/api/v1",
                credential_ref: None,
                default_billing: "paid",
                wire: "openai",
            },
        )
        .unwrap();
        for m in ["google/gemini-4-neon", "google/gemini-4-carbon"] {
            crate::gateway::store::add_model(&conn, &p.id, m).unwrap();
        }
        conn.execute("UPDATE models SET enabled = 0 WHERE model_id = 'google/gemini-4-neon'", []).unwrap();
        let mut c1 = bcard("c1", &[], 1, "engine");
        c1.model = Some(format!("{}/google/gemini-4-neon", p.id));
        c1.fallbacks = vec![Fallback { model: format!("{}/google/gemini-4-carbon", p.id), reason: "feed".into() }];
        let mut c2 = bcard("c2", &[], 1, "engine");
        c2.model = Some(format!("{}/google/gemini-4-neon", p.id));
        c2.fallbacks = vec![];
        let b = brainstorm(vec![c1, c2], vec![]);
        insert_brainstorm(&conn, &b);
        add_cards(&conn, &b, &ws, "opencode").unwrap();
        set_limits(&conn, &ws, Some((2, 1))).unwrap();
        let started = tick(&conn).unwrap();
        assert_eq!(started.len(), 1, "c1 runs on what is left of its ladder; c2 has nothing to run on");
        let task = tasks::task_detail(&conn, &started[0]).unwrap();
        assert_eq!(
            task.policy.steps.iter().map(|s| s.model.clone()).collect::<Vec<_>>(),
            [Some(format!("pygg/{}/google/gemini-4-carbon", p.id))]
        );
        assert_eq!(lane(&conn, &ws, "c1").0, "running");
        assert_eq!(
            lane(&conn, &ws, "c2"),
            ("blocked".into(), Some(format!("No enabled model: {}/google/gemini-4-neon", p.id)))
        );
    }

    #[test]
    fn cards_wait_for_their_questions_and_cards_and_are_pulled_under_the_limit() {
        let conn = crate::db::open_in_memory().unwrap();
        let ws = workspace(&conn);
        let mut b = brainstorm(
            vec![
                bcard("c1", &[], 1, "engine"),
                bcard("c2", &["c1", "q1"], 2, "engine"),
                bcard("c3", &[], 1, "engine"),
                bcard("c4", &["c9"], 1, "engine"),
                bcard("c5", &[], 1, "ide"),
            ],
            vec![Question { id: "q1".into(), text: "?".into(), answered: false }],
        );
        insert_brainstorm(&conn, &b);
        let ids = add_cards(&conn, &b, &ws, "opencode").unwrap();
        assert_eq!(ids.len(), 5);
        // Nothing is running until the tick; then one card is pulled (the limit is 1), the rest settle.
        assert!(cards_of(&conn, &ws).unwrap().iter().all(|c| c.lane == "discovery" && c.task_id.is_none()));
        let started = tick(&conn).unwrap();
        assert_eq!(started.len(), 1, "Running's limit is 1 by default");
        assert_eq!(lane(&conn, &ws, "c1"), ("running".into(), None));
        assert_eq!(lane(&conn, &ws, "c2"), ("discovery".into(), None), "waits for c1 and q1");
        assert_eq!(lane(&conn, &ws, "c3"), ("ready".into(), None));
        assert_eq!(
            lane(&conn, &ws, "c4"),
            ("blocked".into(), Some("Needs c9, which was dropped before approval".into()))
        );
        assert_eq!(lane(&conn, &ws, "c5"), ("blocked".into(), Some("Waiting for your agent".into())));
        // The pulled card's task has one step per model of the ladder, on the Pygg provider.
        let task = tasks::task_detail(&conn, &started[0]).unwrap();
        assert_eq!(
            task.policy.steps.iter().map(|s| s.model.as_deref()).collect::<Vec<_>>(),
            [Some("pygg/paid/mid"), Some("pygg/free/gift")]
        );
        // A second tick with the task still running pulls nothing more; raising the limit pulls c3.
        assert!(tick(&conn).unwrap().is_empty());
        set_limits(&conn, &ws, Some((2, 1))).unwrap();
        assert_eq!(tick(&conn).unwrap().len(), 1);
        assert_eq!(lane(&conn, &ws, "c3").0, "running");
        // c1's task kept: c1 is done; c2 still waits for q1 until the brainstorm says it is answered.
        conn.execute("UPDATE tasks SET state = 'checks_passed', result = 'kept' WHERE id = ?1", [&started[0]]).unwrap();
        tick(&conn).unwrap();
        assert_eq!(lane(&conn, &ws, "c1").0, "done");
        assert_eq!(lane(&conn, &ws, "c2").0, "discovery");
        b.proposal.questions[0].answered = true;
        conn.execute(
            "UPDATE brainstorms SET proposal_json = ?2 WHERE id = ?1",
            params!["b_1", serde_json::to_string(&b.proposal).unwrap()],
        )
        .unwrap();
        tick(&conn).unwrap();
        assert_eq!(
            lane(&conn, &ws, "c2").0,
            "running",
            "c2 is pulled: room under the limit of 2 (c3 runs, c1 is done)"
        );
        let board = board(&conn, &ws).unwrap();
        assert_eq!((board.measures.done, board.measures.measured), (1, false));
        assert!(board.cards.iter().find(|c| c.card_id == "c1").unwrap().task.is_some());
        // Hand the IDE card to the engine; hold a ready card; expedite is one at a time.
        let c5 = cards_of(&conn, &ws).unwrap().into_iter().find(|c| c.card_id == "c5").unwrap();
        assert_eq!(act(&conn, &c5.id, "engine").unwrap().runner, "engine");
        let c4 = cards_of(&conn, &ws).unwrap().into_iter().find(|c| c.card_id == "c4").unwrap();
        assert!(act(&conn, &c4.id, "hold").unwrap().held);
        act(&conn, &c5.id, "expedite").unwrap();
        act(&conn, &c4.id, "expedite").unwrap();
        let classes: Vec<(String, String)> =
            cards_of(&conn, &ws).unwrap().into_iter().map(|c| (c.card_id, c.class)).collect();
        assert_eq!(
            classes.iter().filter(|(_, k)| k == "expedite").map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
            ["c4"]
        );
        assert_eq!(list(&conn).unwrap()[0].cards, 5);
        // Money per kept result: c1's task had one priced request through the gateway and one without a price.
        let stage: String =
            conn.query_row("SELECT id FROM task_stages WHERE task_id = ?1", [&started[0]], |r| r.get(0)).unwrap();
        conn.execute(
            "INSERT INTO attempts (id, stage_id, number, agent, model_ref, tier, billing, state) VALUES ('a_c1', ?1, 1, 'opencode', 'pygg/paid/mid', 1, 'paid', 'done')",
            [&stage],
        )
        .unwrap();
        for (id, status, micro) in [("u1", "estimated", 150_000), ("u2", "unknown", 0)] {
            conn.execute(
                "INSERT INTO usage_events (id, attempt_id, provider, model, cost_status, cost_micro_usd, source, created_at) VALUES (?1, 'a_c1', 'paid', 'mid', ?2, ?3, 'gateway', 1)",
                params![id, status, micro],
            )
            .unwrap();
        }
        let money = super::board(&conn, &ws).unwrap().measures.money;
        assert_eq!(money.len(), 1);
        assert_eq!(
            (money[0].role.as_str(), money[0].kept, money[0].unknown_calls, money[0].usd_per_kept),
            ("code", 1, 1, Some(0.15))
        );
    }

    #[test]
    fn a_blocked_stay_is_timed_and_a_task_state_maps_to_its_lane() {
        assert_eq!(
            lane_of(&TaskState { state: "running".into(), phase: Some("checking".into()), result: None }).0,
            "checking"
        );
        assert_eq!(
            lane_of(&TaskState { state: "waiting_approval".into(), phase: None, result: None }),
            ("blocked", Some("Waiting for approval"))
        );
        assert_eq!(lane_of(&TaskState { state: "needs_review".into(), phase: None, result: None }).0, "review");
        assert_eq!(
            lane_of(&TaskState { state: "checks_passed".into(), phase: None, result: Some("kept".into()) }).0,
            "done"
        );
        assert_eq!(
            lane_of(&TaskState { state: "checks_passed".into(), phase: None, result: Some("discarded".into()) }).0,
            "discarded"
        );
        let conn = crate::db::open_in_memory().unwrap();
        let ws = workspace(&conn);
        let b = brainstorm(vec![bcard("c1", &["c9"], 1, "engine")], vec![]);
        insert_brainstorm(&conn, &b);
        add_cards(&conn, &b, &ws, "codex").unwrap();
        tick(&conn).unwrap();
        let c = cards_of(&conn, &ws).unwrap().remove(0);
        assert_eq!(c.lane, "blocked");
        assert!(c.blocked_since.is_some() && c.blocked_ms == 0);
        // Leaving Blocked banks the time spent there.
        conn.execute("UPDATE board_cards SET blocked_since = blocked_since - 5000", []).unwrap();
        let c = cards_of(&conn, &ws).unwrap().remove(0);
        set_lane(&conn, &c, "ready", None).unwrap();
        let c = cards_of(&conn, &ws).unwrap().remove(0);
        assert!(c.blocked_ms >= 5000 && c.blocked_since.is_none(), "{c:?}");
        // Another agent gets one step and the picks in its prompt.
        let new = task_for(&conn, &c).unwrap();
        assert_eq!(new.steps.len(), 1);
        assert!(new.prompt.contains("Pygg picked paid/mid, then free/gift for this card"));
    }
}
