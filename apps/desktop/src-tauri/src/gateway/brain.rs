//! `pygg/brain` (ADR-041, docs/design/BRAIN.md §2): the router is the brain. A client names the virtual model
//! `pygg/brain` with a requirement; the gateway picks a planner by rule (auto's filters, the `plan` role's facts,
//! the user's saved pick first), asks it for clarifying questions and a board of cards with Pygg's own prompt
//! (the starter skill `brain-v3`), picks a model per card by rule (never the planner's word), stores the proposal
//! for the app, and answers the client with the proposal in plain text. A paid planner on first use waits for the
//! user's pick in the app (ADR-010); a later call in the same session refines the same brainstorm.
//!
//! What this keeps (ADR-025, ADR-028, ADR-030): only a request naming `pygg/brain` gets Pygg's prompt; every call
//! the brain makes is a ledger entry under the client's key and session with route kind `brain`; nothing from a
//! brainstorm is kept as memory or sent with any later request; the app shows and approves, and has no reply box.
use super::auto::{self, Needs, Role, Strategy};
use super::store::{Client, Result};
use super::{forward, record_success, Incoming, Mode, Refusal, Shared, Surface};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

/// The model name a client asks for.
pub const MODEL: &str = "pygg/brain";
/// The response header naming the brainstorm the answer belongs to.
pub const HEADER: &str = "x-pygg-brainstorm";
/// The ledger's route kind for the calls the brain makes.
pub const ROUTE_KIND: &str = "brain";
/// The starter skill that holds the prompt (skills.rs, ADR-039).
pub const PROMPT_VERSION: &str = "brain-v3";
/// The quality signal written per planner reply (ADR-029, migration 0042).
pub const METRIC: &str = "brain_schema_ok";
pub const MAX_QUESTIONS: usize = 7;
pub const MAX_CARDS: usize = 40;
/// Where a saved pick applies to every project (BRAIN.md 4.7b).
pub const GLOBAL: &str = "global";

pub fn is_brain(requested: &str) -> bool {
    requested == MODEL
}

// ---------- the proposal ----------

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    pub id: String,
    pub text: String,
    /// Answered in the client that asked: the planner stopped asking it in a later revision.
    #[serde(default)]
    pub answered: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub id: String,
    pub title: String,
    pub role: String,
    pub done: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// `connection/model`, picked by rule for the card's role; `None` when nothing was eligible.
    #[serde(default)]
    pub model: Option<String>,
    /// Why that model: auto's reason for the role alias (the fact, or the tier).
    #[serde(default)]
    pub reason: Option<String>,
    /// What one run of the card on `model` would cost by the route's estimate (BRAIN.md §4.1, honest values):
    /// `Some(0)` for a free or local model, `None` when the model has no price (unpriced), so the approval line
    /// sums the known estimates and counts the unpriced cards.
    #[serde(default)]
    pub estimate_micro_usd: Option<u64>,
    /// Models tried after `model` when an attempt on it fails (a connection error, a spent quota, a failed check),
    /// each a step of the card's task: the next of the role's route by rule, or the user's own picks.
    #[serde(default)]
    pub fallbacks: Vec<Fallback>,
    /// `engine` (Pygg's task engine) or `ide` (the user's own agent), ADR-041.
    #[serde(default = "engine")]
    pub runner: String,
    /// `standard` or `expedite` (ADR-042).
    #[serde(default = "standard")]
    pub class: String,
    #[serde(default)]
    pub dropped: bool,
    /// 1 for a card that depends on no other card, 2 for one that depends on a wave-1 card, and so on.
    #[serde(default)]
    pub wave: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Fallback {
    /// `connection/model`.
    pub model: String,
    /// Why it is here: auto's reason for the role alias, or `your pick`.
    pub reason: String,
}

/// How many fallbacks the rule suggests for a card (the user may add more, up to the task engine's six steps).
pub const SUGGESTED_FALLBACKS: usize = 2;

fn engine() -> String {
    "engine".into()
}

fn standard() -> String {
    "standard".into()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    #[serde(default)]
    pub questions: Vec<Question>,
    #[serde(default)]
    pub cards: Vec<Card>,
    /// Cards the planner wrote that did not fit the schema and were left out.
    #[serde(default)]
    pub dropped_cards: u32,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Brainstorm {
    pub id: String,
    pub client_id: Option<String>,
    pub session: String,
    pub workspace_id: Option<String>,
    /// `waiting_pick`, `thinking`, `proposed`, `approved`, `declined`, `failed`.
    pub state: String,
    pub requirement: String,
    /// `connection/model`.
    pub planner: Option<String>,
    pub prompt_version: String,
    pub revision: u32,
    pub proposal: Proposal,
    pub raw_reply: Option<String>,
    pub note: Option<String>,
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    /// The branch its cards build on, once approved onto a board (migration 0045).
    pub branch: Option<String>,
}

// ---------- the store ----------

const COLUMNS: &str = "id, client_id, session, workspace_id, state, requirement, planner, prompt_version, revision, \
                       proposal_json, raw_reply, note, error, created_at, updated_at, branch";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Brainstorm> {
    let proposal: String = r.get(9)?;
    Ok(Brainstorm {
        id: r.get(0)?,
        client_id: r.get(1)?,
        session: r.get(2)?,
        workspace_id: r.get(3)?,
        state: r.get(4)?,
        requirement: r.get(5)?,
        planner: r.get(6)?,
        prompt_version: r.get(7)?,
        revision: r.get::<_, i64>(8)? as u32,
        proposal: serde_json::from_str(&proposal).unwrap_or_default(),
        raw_reply: r.get(10)?,
        note: r.get(11)?,
        error: r.get(12)?,
        created_at: r.get(13)?,
        updated_at: r.get(14)?,
        branch: r.get(15)?,
    })
}

fn err(e: rusqlite::Error) -> String {
    e.to_string()
}

pub fn get(conn: &Connection, id: &str) -> Result<Option<Brainstorm>> {
    conn.query_row(&format!("SELECT {COLUMNS} FROM brainstorms WHERE id = ?1"), [id], row).optional().map_err(err)
}

/// The newest brainstorm of a client's session.
pub fn by_session(conn: &Connection, client_id: &str, session: &str) -> Result<Option<Brainstorm>> {
    conn.query_row(
        &format!(
            "SELECT {COLUMNS} FROM brainstorms WHERE client_id = ?1 AND session = ?2 ORDER BY created_at DESC LIMIT 1"
        ),
        params![client_id, session],
        row,
    )
    .optional()
    .map_err(err)
}

/// Every brainstorm, newest first; a client sees only its own when `client_id` is given.
pub fn list(conn: &Connection, client_id: Option<&str>) -> Result<Vec<Brainstorm>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {COLUMNS} FROM brainstorms WHERE ?1 IS NULL OR client_id = ?1 ORDER BY created_at DESC"
        ))
        .map_err(err)?;
    let rows = stmt.query_map([client_id], row).map_err(err)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(err)
}

fn new_id() -> String {
    format!("b_{}", &uuid::Uuid::new_v4().simple().to_string()[..10])
}

/// The session of a brainstorm started in the app (ADR-027 amended) begins with this; one from a coding agent has the
/// agent's session.
pub const APP_SESSION: &str = "app-";

/// A brainstorm started in the app for a project, with the requirement as the user wrote it. Its planner is chosen
/// next (`choose_planner`), then run.
pub fn create_in_app(conn: &Connection, client_id: &str, workspace_id: &str, requirement: &str) -> Result<Brainstorm> {
    let requirement = requirement.trim();
    if requirement.is_empty() {
        return Err("Describe what to build".into());
    }
    let session = format!("{APP_SESSION}{}", uuid::Uuid::new_v4().simple());
    let mut b = create(conn, client_id, &session, requirement, "thinking")?;
    b.workspace_id = Some(workspace_id.to_string());
    save(conn, &b)?;
    Ok(b)
}

/// Sets the planner of a brainstorm that has none: the saved pick, or the rule's when it costs nothing. `false` when
/// the brainstorm now waits for the user's pick or has failed for want of a model (saved either way).
pub fn choose_planner(conn: &Connection, client: &Client, b: &mut Brainstorm) -> Result<bool> {
    if b.planner.is_some() {
        return Ok(true);
    }
    match pick(conn, client)? {
        Pick::Saved(name) | Pick::Rule(name) => {
            b.planner = Some(name);
            Ok(true)
        }
        Pick::WaitsForPick(name) => {
            b.state = "waiting_pick".into();
            b.note = Some(format!("the rule would pick {name}, which costs money; pick a planner in Pygg"));
            save(conn, b)?;
            Ok(false)
        }
        Pick::Nothing(why) => {
            b.state = "failed".into();
            b.error = Some(format!("no connected model is eligible as a planner: {why}"));
            save(conn, b)?;
            Ok(false)
        }
    }
}

fn create(conn: &Connection, client_id: &str, session: &str, requirement: &str, state: &str) -> Result<Brainstorm> {
    let id = new_id();
    let now = crate::db::now_ms();
    conn.execute(
        "INSERT INTO brainstorms (id, client_id, session, state, requirement, prompt_version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![id, client_id, session, state, requirement, PROMPT_VERSION, now],
    )
    .map_err(err)?;
    get(conn, &id)?.ok_or_else(|| "the brainstorm was not written".into())
}

pub fn save(conn: &Connection, b: &Brainstorm) -> Result<()> {
    conn.execute(
        "UPDATE brainstorms SET workspace_id = ?2, state = ?3, planner = ?4, revision = ?5, proposal_json = ?6, raw_reply = ?7, \
         note = ?8, error = ?9, updated_at = ?10 WHERE id = ?1",
        params![
            b.id,
            b.workspace_id,
            b.state,
            b.planner,
            b.revision as i64,
            serde_json::to_string(&b.proposal).map_err(|e| e.to_string())?,
            b.raw_reply,
            b.note,
            b.error,
            crate::db::now_ms()
        ],
    )
    .map(|_| ())
    .map_err(err)
}

/// The quality signal for a planner reply (ADR-029): true when every card fit the schema.
fn signal(conn: &Connection, brainstorm: &str, ok: bool) {
    let _ = conn.execute(
        "INSERT INTO feedback (id, metric_name, target_kind, target_id, value_bool, source, created_at) \
         VALUES (?1, ?2, 'brainstorm', ?3, ?4, 'brain', ?5)",
        params![uuid::Uuid::new_v4().to_string(), METRIC, brainstorm, ok as i64, crate::db::now_ms()],
    );
}

/// A pick the user saved for a role and scope (`global` or a workspace id): `connection/model`.
pub fn saved_pick(conn: &Connection, scope: &str, role: &str) -> Result<Option<(String, String)>> {
    conn.query_row(
        "SELECT provider_id, model_id FROM brain_picks WHERE scope = ?1 AND role = ?2",
        params![scope, role],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
    .map_err(err)
}

pub fn save_pick(conn: &Connection, scope: &str, role: &str, provider_id: &str, model_id: &str) -> Result<()> {
    if !super::facts::ROUTED_ROLES.contains(&role) {
        return Err(format!("{role} is not a role"));
    }
    conn.execute(
        "INSERT INTO brain_picks (scope, role, provider_id, model_id, saved_at) VALUES (?1, ?2, ?3, ?4, ?5) \
         ON CONFLICT(scope, role) DO UPDATE SET provider_id = ?3, model_id = ?4, saved_at = ?5",
        params![scope, role, provider_id, model_id, crate::db::now_ms()],
    )
    .map(|_| ())
    .map_err(err)
}

pub fn clear_pick(conn: &Connection, scope: &str, role: &str) -> Result<()> {
    conn.execute("DELETE FROM brain_picks WHERE scope = ?1 AND role = ?2", params![scope, role])
        .map(|_| ())
        .map_err(err)
}

// ---------- the planner ----------

/// What a planner call needs from a model: a few thousand prompt tokens, room for a board.
fn planner_needs() -> Needs {
    Needs { input_tokens: 3_000, max_tokens: Some(6_000), ..Needs::default() }
}

/// The output a planner call asks for: the board is a few thousand tokens, but a reasoning model thinks first. On a
/// one-page specification DeepSeek V4 Pro spent all of 6,000 thinking and wrote nothing, and with brain-v2's longer
/// rules twice all of 16,000 (2026-11-21). Up to the model's own maximum.
const PLANNER_MAX_TOKENS: u64 = 32_000;

fn planner_max_tokens(conn: &Connection, planner: &str) -> u64 {
    let most = planner.split_once('/').and_then(|(provider, model)| {
        super::store::list_models(conn, Some(provider))
            .ok()?
            .into_iter()
            .find(|m| m.model_id == model)
            .and_then(|m| m.capabilities.max_output_tokens)
    });
    most.map_or(PLANNER_MAX_TOKENS, |m| m.clamp(6_000, PLANNER_MAX_TOKENS))
}

/// The models eligible as a planner, ranked for the `plan` role under the client's policy (auto's filters).
pub fn candidates(conn: &Connection, client: &Client) -> Result<auto::Plan> {
    let feed = super::facts::for_conn(conn);
    auto::plan_with(conn, client, "auto/plan", Strategy::Role(Role::Plan), &planner_needs(), feed.as_deref())
}

/// The models eligible for a card of `role` under the client's policy, ranked as `pick_models` ranks them (the role
/// sheet, BRAIN.md §4.5): the plan role with the planner's needs, the others with a card's.
pub fn candidates_for(conn: &Connection, client: &Client, role: &str) -> Result<auto::Plan> {
    let role = match role {
        "plan" => return candidates(conn, client),
        "code" => Role::Code,
        "test" => Role::Test,
        "review" => Role::Review,
        other => return Err(format!("unknown role {other}")),
    };
    let feed = super::facts::for_conn(conn);
    let needs = Needs { input_tokens: 8_000, tools: true, ..Needs::default() };
    auto::plan_with(conn, client, &format!("auto/{}", role.as_str()), Strategy::Role(role), &needs, feed.as_deref())
}

/// How the planner was chosen.
#[derive(Clone, Debug, PartialEq)]
pub enum Pick {
    /// The user's saved pick, still eligible.
    Saved(String),
    /// The rule's pick, free or local or on a subscription: it runs at once.
    Rule(String),
    /// The rule's pick costs money and nothing is saved: the user picks in the app first (ADR-010).
    WaitsForPick(String),
    /// Nothing is eligible, with auto's reasons.
    Nothing(String),
}

pub fn pick(conn: &Connection, client: &Client) -> Result<Pick> {
    let plan = candidates(conn, client)?;
    if let Some((provider, model)) = saved_pick(conn, GLOBAL, "plan")? {
        if plan.ranked.iter().any(|r| r.provider_id == provider && r.model_id == model) {
            return Ok(Pick::Saved(format!("{provider}/{model}")));
        }
    }
    let Some(best) = plan.ranked.first() else {
        return Ok(Pick::Nothing(auto::summarize(&plan.excluded, 6)));
    };
    let name = format!("{}/{}", best.provider_id, best.model_id);
    Ok(if matches!(best.billing.as_str(), "free" | "local" | "subscription") {
        Pick::Rule(name)
    } else {
        Pick::WaitsForPick(name)
    })
}

/// The messages the planner is sent: Pygg's prompt, then the requirement (and, on a refinement, the previous
/// proposal and what the user answered). Nothing from the user's repository, nothing about their connections.
fn planner_messages(b: &Brainstorm, answers: Option<&str>, checks: Option<&[String]>) -> Vec<Value> {
    let prompt = crate::skills::starter_body(PROMPT_VERSION).unwrap_or_default();
    let mut user = format!("Requirement:\n{}", b.requirement.trim());
    // Whether a card's result is tested at all decides how cards are cut and what "done" can promise.
    user.push_str(&match checks {
        Some([]) => {
            "\n\nThe repository has no approved checks: nothing tests a card's result, a person reviews each diff."
                .to_string()
        }
        Some(list) => format!(
            "\n\nThe repository's approved checks, run after every card: {}.",
            list.iter().map(|c| format!("`{c}`")).collect::<Vec<_>>().join(", ")
        ),
        None => "\n\nThe repository's checks are not known yet.".to_string(),
    });
    if b.revision > 0 {
        let previous = json!({
            "questions": b.proposal.questions.iter().filter(|q| !q.answered).map(|q| json!({"id": q.id, "text": q.text})).collect::<Vec<_>>(),
            "cards": b.proposal.cards.iter().map(|c| json!({"id": c.id, "title": c.title, "role": c.role, "done": c.done, "dependsOn": c.depends_on})).collect::<Vec<_>>(),
        });
        user.push_str(&format!("\n\nYour previous proposal (JSON):\n{previous}"));
        if let Some(answers) = answers.map(str::trim).filter(|a| !a.is_empty()) {
            user.push_str(&format!("\n\nThe user's answers and changes:\n{answers}"));
        }
        user.push_str("\n\nReturn the whole updated board; leave out every question the user has answered.");
    }
    vec![json!({"role": "system", "content": prompt}), json!({"role": "user", "content": user})]
}

/// The planner's reply as text, from a Chat Completions answer (an Anthropic answer is translated first).
fn reply_text(body: &Value) -> Option<String> {
    let content = &body["choices"][0]["message"]["content"];
    if let Some(s) = content.as_str() {
        return Some(s.to_string());
    }
    content.as_array().map(|parts| parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join(""))
}

/// Runs the planner for a brainstorm whose planner is set, through the gateway as the client (its key, session and
/// limits), and stores the outcome: `proposed` with a new revision, or `failed` with the reason.
pub async fn run_planner(
    shared: &Shared, client: &Client, b: &mut Brainstorm, answers: Option<&str>,
) -> std::result::Result<(), String> {
    let Some(planner) = b.planner.clone() else { return Err("no planner is set".into()) };
    b.state = "thinking".into();
    shared.with_db(|db| save(db, b))?;
    let max_tokens = shared.with_db(|db| planner_max_tokens(db, &planner));
    let checks: Option<Vec<String>> = b.workspace_id.as_deref().and_then(|w| {
        shared
            .with_db(|db| crate::tasks::workspace(db, w))
            .ok()
            .map(|ws| ws.checks.into_iter().filter(|c| c.approved).map(|c| c.command).collect())
    });
    let body = json!({ "model": planner, "messages": planner_messages(b, answers, checks.as_deref()), "stream": false, "max_tokens": max_tokens, "temperature": 0.2 });
    let mut headers = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(&b.session) {
        headers.insert(super::SESSION_HEADER, v);
    }
    let incoming = Incoming {
        surface: Surface::OpenAi,
        body: &body,
        headers: &headers,
        requested: &planner,
        session: Some(&b.session),
        stream: false,
        route_kind: Some(ROUTE_KIND),
    };
    let outcome = call(shared, client, &incoming).await;
    match outcome {
        Err(e) => {
            b.state = "failed".into();
            b.error = Some(e.clone());
            shared.with_db(|db| save(db, b))?;
            Err(e)
        }
        Ok(text) => {
            b.raw_reply = Some(text.clone());
            match parse_reply(&text) {
                Err(e) => {
                    b.state = "failed".into();
                    b.error = Some(format!("the planner did not answer in the board schema: {e}"));
                    shared.with_db(|db| {
                        signal(db, &b.id, false);
                        save(db, b)
                    })?;
                    Err(b.error.clone().unwrap_or_default())
                }
                Ok(mut proposal) => {
                    merge_questions(&b.proposal.questions, &mut proposal);
                    // What the user set on a card by its id survives a refinement: the model, the runner, dropped.
                    for card in &mut proposal.cards {
                        if let Some(old) = b.proposal.cards.iter().find(|c| c.id == card.id) {
                            card.runner = old.runner.clone();
                            card.fallbacks =
                                old.fallbacks.iter().filter(|f| f.reason == "your pick").cloned().collect();
                            card.class = old.class.clone();
                            card.dropped = old.dropped;
                        }
                    }
                    let ok = proposal.dropped_cards == 0;
                    shared.with_db(|db| -> std::result::Result<(), String> {
                        pick_models(db, client, b.workspace_id.as_deref(), &mut proposal)?;
                        signal(db, &b.id, ok);
                        Ok(())
                    })?;
                    b.proposal = proposal;
                    b.revision += 1;
                    b.state = "proposed".into();
                    b.error = None;
                    b.note = (!ok).then(|| {
                        format!(
                            "{} card(s) the planner wrote did not fit the schema and were left out",
                            b.proposal.dropped_cards
                        )
                    });
                    shared.with_db(|db| save(db, b))
                }
            }
        }
    }
}

/// One call through the gateway, accounted like any request of the client, its text returned.
async fn call(shared: &Shared, client: &Client, incoming: &Incoming<'_>) -> std::result::Result<String, String> {
    let mut forwarded = forward(shared, client, incoming).await.map_err(|r| format!("{} ({})", r.message, r.kind))?;
    let status = forwarded.response.status();
    let upstream = std::mem::replace(&mut forwarded.response, super::status_only(status));
    let bytes = match super::read_body(upstream).await {
        Ok(bytes) => bytes,
        Err(e) => {
            let message = crate::redact::redact(&e.to_string(), &[]);
            record_success(shared, forwarded, None, "stream_error", Some(&message));
            return Err(format!("the planner's answer broke off: {message}"));
        }
    };
    let parsed: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    let (chat, usage) = if forwarded.mode == Mode::OpenAiToAnthropic {
        super::openai_to_anthropic::from_anthropic(&parsed, incoming.requested)
    } else {
        let usage = super::pricing::Usage::from_json(&parsed["usage"]);
        (parsed, usage)
    };
    let model = forwarded.model_id.clone();
    record_success(shared, forwarded, usage.as_ref(), "ok", None);
    if !status.is_success() {
        return Err(format!("{model} answered HTTP {}", status.as_u16()));
    }
    reply_text(&chat).filter(|t| !t.trim().is_empty()).ok_or_else(|| {
        if chat["choices"][0]["finish_reason"].as_str() == Some("length") {
            format!("{model} used its whole output limit before writing the board (a thinking model can); pick another planner")
        } else {
            format!("{model} answered with no text")
        }
    })
}

/// The model for each card, by rule: the role alias's ranking under the client's policy (never the planner's).
/// Picks each card's model: the pick the user saved for the card's role on Brain Storming (the workspace's own when
/// the brainstorm has one, else the default), when it is eligible under the client's policy; otherwise the rule's,
/// the first of the role's route. The suggested fallbacks are the route's next models either way.
pub fn pick_models(
    conn: &Connection, client: &Client, workspace: Option<&str>, proposal: &mut Proposal,
) -> std::result::Result<(), String> {
    let feed = super::facts::for_conn(conn);
    let mut per_role: HashMap<String, Option<(String, String, Option<u64>, Vec<Fallback>)>> = HashMap::new();
    for card in &mut proposal.cards {
        let entry = match per_role.get(&card.role) {
            Some(e) => e.clone(),
            None => {
                let role = match card.role.as_str() {
                    "plan" => Role::Plan,
                    "code" => Role::Code,
                    "test" => Role::Test,
                    _ => Role::Review,
                };
                let needs = Needs { input_tokens: 8_000, tools: role != Role::Plan, ..Needs::default() };
                let plan = auto::plan_with(
                    conn,
                    client,
                    &format!("auto/{}", role.as_str()),
                    Strategy::Role(role),
                    &needs,
                    feed.as_deref(),
                )?;
                // The route as auto would try it (at most two models of one connection while others are eligible):
                // the first is the card's model, the next ones its suggested fallbacks.
                let saved = match workspace {
                    Some(ws) => saved_pick(conn, ws, role.as_str())?,
                    None => None,
                }
                .or(saved_pick(conn, GLOBAL, role.as_str())?);
                let saved = saved.and_then(|(provider, model)| {
                    plan.ranked.iter().find(|r| r.provider_id == provider && r.model_id == model).map(|r| {
                        (
                            format!("{provider}/{model}"),
                            format!("your pick for {} on Brain Storming; {}", role.as_str(), r.reason),
                            Some(r.estimate_micro_usd),
                        )
                    })
                });
                let saved_id = saved.as_ref().map(|(model, _, _)| model.clone());
                let mut route = plan
                    .route
                    .iter()
                    .map(|r| {
                        (format!("{}/{}", r.provider_id, r.model_id), r.reason.clone(), Some(r.estimate_micro_usd))
                    })
                    .filter(|(model, _, _)| saved_id.as_deref() != Some(model.as_str()));
                let first = saved.or_else(|| route.next());
                let best = first.map(|(model, reason, estimate)| {
                    let fallbacks =
                        route.take(SUGGESTED_FALLBACKS).map(|(model, reason, _)| Fallback { model, reason }).collect();
                    (model, reason, estimate, fallbacks)
                });
                per_role.insert(card.role.clone(), best.clone());
                best
            }
        };
        match entry {
            Some((model, reason, estimate, suggested)) => {
                card.model = Some(model);
                card.reason = Some(reason);
                card.estimate_micro_usd = estimate;
                // The user's own fallbacks (kept across a refinement) come first; the rule's suggestions fill the rest.
                let mut fallbacks: Vec<Fallback> =
                    card.fallbacks.iter().filter(|f| f.reason == "your pick").cloned().collect();
                for f in suggested {
                    if f.model != card.model.clone().unwrap_or_default()
                        && !fallbacks.iter().any(|x| x.model == f.model)
                    {
                        fallbacks.push(f);
                    }
                }
                card.fallbacks = fallbacks;
            }
            None => {
                card.model = None;
                card.reason = Some(format!("nothing eligible for {} under this client's policy", card.role));
                card.estimate_micro_usd = None;
            }
        }
    }
    Ok(())
}

/// Carries the earlier questions into a refined proposal: a question the planner asks again (same text) keeps its
/// id; one it stopped asking is marked answered in the client; a new question the planner gave an old id (seen live:
/// Gemini numbered its new question `q1` again) gets a fresh id, and the cards that named it follow.
fn merge_questions(old: &[Question], proposal: &mut Proposal) {
    let mut merged: Vec<Question> = Vec::new();
    let mut renamed: HashMap<String, String> = HashMap::new();
    let mut next = old.len() + proposal.questions.len() + 1;
    for mut q in proposal.questions.drain(..) {
        if let Some(same) = old.iter().find(|o| o.text == q.text) {
            if same.id != q.id {
                renamed.insert(q.id.clone(), same.id.clone());
                q.id = same.id.clone();
            }
        } else if old.iter().any(|o| o.id == q.id) {
            let fresh = loop {
                let id = format!("q{next}");
                next += 1;
                if !old.iter().any(|o| o.id == id) && !merged.iter().any(|m| m.id == id) {
                    break id;
                }
            };
            renamed.insert(q.id.clone(), fresh.clone());
            q.id = fresh;
        }
        merged.push(q);
    }
    for o in old {
        if !merged.iter().any(|m| m.text == o.text) {
            merged.push(Question { answered: true, ..o.clone() });
        }
    }
    if !renamed.is_empty() {
        for card in &mut proposal.cards {
            for dep in &mut card.depends_on {
                if let Some(to) = renamed.get(dep) {
                    *dep = to.clone();
                }
            }
        }
    }
    proposal.questions = merged;
}

// ---------- parsing the planner ----------

/// The planner's text as a proposal: JSON, fenced or not, in the board schema. A card that does not fit is dropped
/// and counted; a cycle between cards, or no card at all, refuses the whole reply.
pub fn parse_reply(text: &str) -> std::result::Result<Proposal, String> {
    let json = extract_json(text).ok_or("no JSON object in the reply")?;
    let value: Value = serde_json::from_str(json).map_err(|e| format!("not valid JSON: {e}"))?;
    let mut questions = Vec::new();
    for (i, q) in value["questions"].as_array().into_iter().flatten().enumerate() {
        let text = q["text"].as_str().map(str::trim).unwrap_or("");
        if text.is_empty() || questions.len() >= MAX_QUESTIONS {
            continue;
        }
        let id = q["id"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .unwrap_or_else(|| format!("q{}", i + 1));
        questions.push(Question { id, text: text.chars().take(400).collect(), answered: false });
    }
    let mut cards: Vec<Card> = Vec::new();
    let mut dropped = 0u32;
    for (i, c) in value["cards"].as_array().into_iter().flatten().enumerate() {
        let title = c["title"].as_str().map(str::trim).unwrap_or("");
        let role = c["role"].as_str().map(str::trim).unwrap_or("");
        let done = c["done"].as_str().or_else(|| c["doneCriterion"].as_str()).map(str::trim).unwrap_or("");
        if title.is_empty()
            || done.is_empty()
            || !super::facts::ROUTED_ROLES.contains(&role)
            || cards.len() >= MAX_CARDS
        {
            dropped += 1;
            continue;
        }
        let id = c["id"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .unwrap_or_else(|| format!("c{}", i + 1));
        if cards.iter().any(|k| k.id == id) {
            dropped += 1;
            continue;
        }
        let depends_on = c["dependsOn"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        cards.push(Card {
            id,
            title: title.chars().take(200).collect(),
            role: role.into(),
            done: done.chars().take(600).collect(),
            depends_on,
            model: None,
            reason: None,
            estimate_micro_usd: None,
            fallbacks: Vec::new(),
            runner: engine(),
            class: standard(),
            dropped: false,
            wave: 0,
        });
    }
    if cards.is_empty() {
        return Err(format!("no card fit the schema ({dropped} dropped)"));
    }
    // A dependency on nothing known is dropped from the card; a cycle between cards refuses the reply.
    let known: HashSet<String> =
        questions.iter().map(|q| q.id.clone()).chain(cards.iter().map(|c| c.id.clone())).collect();
    for card in &mut cards {
        card.depends_on.retain(|d| known.contains(d));
    }
    waves(&mut cards)?;
    Ok(Proposal { questions, cards, dropped_cards: dropped })
}

/// Each card's wave: 1 with no card dependency, else one more than the deepest card it depends on. A cycle is an
/// error, so the board never holds one (BRAIN.md 4.6).
fn waves(cards: &mut [Card]) -> std::result::Result<(), String> {
    let ids: Vec<String> = cards.iter().map(|c| c.id.clone()).collect();
    let deps: HashMap<String, Vec<String>> = cards
        .iter()
        .map(|c| (c.id.clone(), c.depends_on.iter().filter(|d| ids.contains(d)).cloned().collect()))
        .collect();
    fn depth(
        id: &str, deps: &HashMap<String, Vec<String>>, seen: &mut Vec<String>, memo: &mut HashMap<String, u32>,
    ) -> std::result::Result<u32, String> {
        if let Some(d) = memo.get(id) {
            return Ok(*d);
        }
        if seen.iter().any(|s| s == id) {
            return Err(format!("cards depend on each other in a cycle through {id}"));
        }
        seen.push(id.to_string());
        let mut d = 1;
        for dep in deps.get(id).into_iter().flatten() {
            d = d.max(depth(dep, deps, seen, memo)? + 1);
        }
        seen.pop();
        memo.insert(id.to_string(), d);
        Ok(d)
    }
    let mut memo = HashMap::new();
    for card in cards.iter_mut() {
        card.wave = depth(&card.id, &deps, &mut Vec::new(), &mut memo)?;
    }
    Ok(())
}

/// The JSON object in a reply: the whole text, a fenced block, or the outermost braces.
fn extract_json(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed);
    }
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (end > start).then(|| &text[start..=end])
}

// ---------- the client's side ----------

/// The last user message's text, from any of the three request shapes.
pub fn last_user_text(surface: Surface, body: &Value) -> Option<String> {
    let turns = match surface {
        Surface::Responses => body["input"]
            .as_array()
            .cloned()
            .or_else(|| body["input"].as_str().map(|s| vec![json!({"role": "user", "content": s})])),
        _ => body["messages"].as_array().cloned(),
    }?;
    let last = turns.iter().rev().find(|m| m["role"] == "user")?;
    let content = &last["content"];
    let text = match content.as_str() {
        Some(s) => s.to_string(),
        None => content
            .as_array()?
            .iter()
            .filter(|p| matches!(p["type"].as_str(), Some("text" | "input_text")))
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    };
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// What the gateway answers: the text the client reads, and the brainstorm it is about.
#[derive(Clone, Debug, PartialEq)]
pub struct Reply {
    pub text: String,
    pub brainstorm_id: String,
}

/// Answers a request that names `pygg/brain`: finds or starts the session's brainstorm, picks or waits for the
/// planner, runs it, and writes the proposal in plain text for the client.
pub async fn answer(shared: &Shared, client: &Client, incoming: &Incoming<'_>) -> std::result::Result<Reply, Refusal> {
    let text = last_user_text(incoming.surface, incoming.body).ok_or_else(|| {
        Refusal::new(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            "pygg/brain needs the requirement in the last user message",
        )
    })?;
    let session =
        incoming.session.map(String::from).unwrap_or_else(|| format!("brain-{}", uuid::Uuid::new_v4().simple()));
    let existing = shared.with_db(|db| by_session(db, &client.id, &session)).map_err(internal)?;
    let (mut b, answers) = match existing {
        // A later call in the same session refines the brainstorm; an approved or declined one is finished.
        Some(b) if matches!(b.state.as_str(), "proposed" | "waiting_pick" | "failed" | "thinking") => (b, Some(text)),
        _ => (shared.with_db(|db| create(db, &client.id, &session, &text, "thinking")).map_err(internal)?, None),
    };
    if !shared.with_db(|db| choose_planner(db, client, &mut b)).map_err(internal)? {
        let text = if b.state == "waiting_pick" { waiting_text(&b) } else { failed_text(&b) };
        return Ok(Reply { text, brainstorm_id: b.id });
    }
    let answers = if b.revision > 0 { answers } else { None };
    match run_planner(shared, client, &mut b, answers.as_deref()).await {
        Ok(()) => Ok(Reply { text: proposal_text(&b), brainstorm_id: b.id }),
        Err(_) => Ok(Reply { text: failed_text(&b), brainstorm_id: b.id }),
    }
}

fn internal(e: String) -> Refusal {
    Refusal::new(StatusCode::INTERNAL_SERVER_ERROR, "api_error", e)
}

fn link(b: &Brainstorm) -> String {
    format!("pygg://brainstorm/{}", b.id)
}

fn waiting_text(b: &Brainstorm) -> String {
    format!(
        "Brainstorm {} waits for your pick in Pygg ({}).\n{}. Pick one in Pygg; your next message here, with or without answers, continues the brainstorm.",
        b.id,
        link(b),
        b.note.as_deref().unwrap_or("No planner is saved and the eligible candidates are paid")
    )
}

fn failed_text(b: &Brainstorm) -> String {
    format!(
        "Brainstorm {} could not be planned ({}).\n{}\nFix it in Pygg (pick another planner, or connect a model), then send your next message here.",
        b.id,
        link(b),
        b.error.as_deref().unwrap_or("unknown reason")
    )
}

/// The proposal in a fixed plain-text shape, read by a model and a person (BRAIN.md 4.8).
pub fn proposal_text(b: &Brainstorm) -> String {
    let mut out = format!(
        "Brainstorm {} · revision {} · open it in Pygg: {}\nPlanner: {} ({})\n",
        b.id,
        b.revision,
        link(b),
        b.planner.as_deref().unwrap_or("?"),
        b.prompt_version
    );
    let open: Vec<&Question> = b.proposal.questions.iter().filter(|q| !q.answered).collect();
    if open.is_empty() {
        out.push_str("\nNo open questions.\n");
    } else {
        out.push_str("\nQuestions (answer any in your next message; the brainstorm is kept):\n");
        for (i, q) in open.iter().enumerate() {
            out.push_str(&format!("{}. {}\n", i + 1, q.text));
        }
    }
    out.push_str("\nProposed cards (models picked by Pygg's rules; change them in Pygg):\n");
    let mut waves: Vec<u32> = b.proposal.cards.iter().filter(|c| !c.dropped).map(|c| c.wave).collect();
    waves.sort_unstable();
    waves.dedup();
    for wave in waves {
        out.push_str(&format!("Wave {wave}:\n"));
        for c in b.proposal.cards.iter().filter(|c| !c.dropped && c.wave == wave) {
            let deps =
                if c.depends_on.is_empty() { String::new() } else { format!(" (after {})", c.depends_on.join(", ")) };
            let then = if c.fallbacks.is_empty() {
                String::new()
            } else {
                format!(", then {}", c.fallbacks.iter().map(|f| f.model.as_str()).collect::<Vec<_>>().join(", "))
            };
            out.push_str(&format!(
                "- {} [{}] {} — auto/{} -> {}{}{}\n  done when: {}\n",
                c.id,
                c.role,
                c.title,
                c.role,
                c.model.as_deref().unwrap_or("nothing eligible"),
                then,
                deps,
                c.done
            ));
        }
    }
    if let Some(note) = &b.note {
        out.push_str(&format!("\nNote: {note}.\n"));
    }
    out.push_str("\nApprove in Pygg; this client cannot approve. Paid cards still wait for approval one by one when they start.\n");
    out
}

// ---------- the response, in the client's format ----------

fn chat_completion(text: &str) -> Value {
    json!({ "id": format!("brain-{}", uuid::Uuid::new_v4().simple()), "object": "chat.completion", "created": now_secs(), "model": MODEL,
        "choices": [{ "index": 0, "message": { "role": "assistant", "content": text }, "finish_reason": "stop" }],
        "usage": { "prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0 } })
}

fn chat_chunks(text: &str) -> Vec<Value> {
    let id = format!("brain-{}", uuid::Uuid::new_v4().simple());
    vec![
        json!({ "id": id, "object": "chat.completion.chunk", "created": now_secs(), "model": MODEL,
            "choices": [{ "index": 0, "delta": { "role": "assistant", "content": text }, "finish_reason": null }] }),
        json!({ "id": id, "object": "chat.completion.chunk", "created": now_secs(), "model": MODEL,
            "choices": [{ "index": 0, "delta": {}, "finish_reason": "stop" }],
            "usage": { "prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0 } }),
    ]
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn sse(events: Vec<String>) -> Response {
    let mut out = (StatusCode::OK, events.concat()).into_response();
    out.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
    out.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    out
}

/// The reply in the client's own format and streaming choice, with the brainstorm's id in a header.
pub fn respond(incoming: &Incoming<'_>, reply: &Reply) -> Response {
    let text = reply.text.as_str();
    let mut out = match (incoming.surface, incoming.stream) {
        (Surface::OpenAi, false) => axum::Json(chat_completion(text)).into_response(),
        (Surface::OpenAi, true) => {
            let mut events: Vec<String> = chat_chunks(text).iter().map(|c| format!("data: {c}\n\n")).collect();
            events.push("data: [DONE]\n\n".into());
            sse(events)
        }
        (Surface::Anthropic, false) => {
            axum::Json(super::anthropic::from_openai(&chat_completion(text), MODEL)).into_response()
        }
        (Surface::Anthropic, true) => {
            let mut translator = super::anthropic::StreamTranslator::new(MODEL);
            let mut events = Vec::new();
            for chunk in chat_chunks(text) {
                events.extend(translator.push(&chunk));
            }
            events.extend(translator.finish());
            sse(events)
        }
        (Surface::Responses, false) => {
            let ctx = super::responses::Context::new(incoming.body, MODEL);
            axum::Json(super::responses::from_chat(&chat_completion(text), &ctx).0).into_response()
        }
        (Surface::Responses, true) => {
            let ctx = super::responses::Context::new(incoming.body, MODEL);
            let mut translator = super::responses::Stream::new(ctx);
            let mut events = Vec::new();
            for chunk in chat_chunks(text) {
                events.extend(translator.push(&chunk));
            }
            events.extend(translator.finish());
            sse(events)
        }
    };
    if let Ok(v) = HeaderValue::from_str(&reply.brainstorm_id) {
        out.headers_mut().insert(HEADER, v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_planner_is_told_whether_the_repository_has_checks() {
        let conn = crate::db::open_in_memory().unwrap();
        let (client, _) = crate::gateway::store::create_client(&conn, "app", None).unwrap();
        let b = create(&conn, &client.id, "s1", "Add CSV export", "thinking").unwrap();
        let user =
            |checks: Option<&[String]>| planner_messages(&b, None, checks)[1]["content"].as_str().unwrap().to_string();
        assert!(user(Some(&[])).contains("no approved checks"));
        assert!(user(Some(&["npm test".to_string()])).contains("`npm test`"));
        assert!(user(None).contains("not known yet"));
        assert!(planner_messages(&b, None, None)[0]["content"].as_str().unwrap().contains("Every card changes files"));
    }

    #[test]
    fn a_brainstorm_started_in_the_app_belongs_to_its_project() {
        let conn = crate::db::open_in_memory().unwrap();
        let (client, _) = crate::gateway::store::create_client(&conn, "app", None).unwrap();
        conn.execute("INSERT INTO workspaces (id, name, path, created_at) VALUES ('w1', 'repo', 'C:/src/repo', 1)", [])
            .unwrap();
        assert!(create_in_app(&conn, &client.id, "w1", "   ").is_err(), "an empty requirement is refused");
        let mut b = create_in_app(&conn, &client.id, "w1", "  Add CSV export  ").unwrap();
        assert_eq!((b.workspace_id.as_deref(), b.requirement.as_str()), (Some("w1"), "Add CSV export"));
        assert!(b.session.starts_with(APP_SESSION));
        // No model is connected: nothing can plan, and it says so.
        assert!(!choose_planner(&conn, &client, &mut b).unwrap());
        assert_eq!(get(&conn, &b.id).unwrap().unwrap().state, "failed");
        // A planner already set is kept.
        b.planner = Some("p/m".into());
        assert!(choose_planner(&conn, &client, &mut b).unwrap());
    }

    fn board(cards: &str) -> String {
        format!(r#"{{"questions": [{{"id": "q1", "text": "Which payment providers?"}}], "cards": [{cards}]}}"#)
    }

    #[test]
    fn a_cards_model_is_the_pick_saved_for_its_role_when_eligible() {
        use crate::gateway::pricing::Price;
        use crate::gateway::store::{self, Capabilities, NewProvider};
        let conn = crate::db::open_in_memory().unwrap();
        let p = store::add_provider(
            &conn,
            &NewProvider {
                kind: "custom",
                label: "x",
                endpoint: "https://x.test/v1",
                credential_ref: None,
                default_billing: "paid",
                wire: "openai",
            },
        )
        .unwrap();
        for (model, tier, cost) in [("strong", "T3", 3_000_000), ("cheap", "T2", 200_000), ("other", "T2", 300_000)] {
            store::add_model(&conn, &p.id, model).unwrap();
            let price = Price {
                input_micro_per_mtok: cost,
                output_micro_per_mtok: cost * 4,
                cached_input_micro_per_mtok: None,
                cache_write_micro_per_mtok: None,
                source: "user".into(),
                observed_at: 0,
            };
            store::update_model(&conn, &p.id, model, None, None, Some(Some(price))).unwrap();
            let caps = Capabilities {
                tier: Some(tier.into()),
                context_tokens: Some(128_000),
                max_output_tokens: Some(16_000),
                supports_tools: Some(true),
                supports_vision: Some(false),
                ..Capabilities::default()
            };
            store::set_capabilities(&conn, &p.id, model, &caps).unwrap();
        }
        let client = store::create_client(&conn, "Tests", None).unwrap().0;
        let fresh = || {
            parse_reply(&board(
                r#"{"id": "c1", "title": "Build it", "role": "code", "done": "tests pass", "dependsOn": []}"#,
            ))
            .unwrap()
        };
        let id = |m: &str| format!("{}/{m}", p.id);
        // Nothing saved: the rule's pick.
        let mut by_rule = fresh();
        pick_models(&conn, &client, None, &mut by_rule).unwrap();
        let rule = by_rule.cards[0].model.clone().unwrap();
        // The default pick wins over the rule, and is not also a fallback.
        save_pick(&conn, GLOBAL, "code", &p.id, "cheap").unwrap();
        let mut saved = fresh();
        pick_models(&conn, &client, None, &mut saved).unwrap();
        let card = &saved.cards[0];
        assert_eq!(card.model.as_deref(), Some(id("cheap").as_str()), "rule picked {rule}");
        assert!(
            card.reason.as_deref().unwrap().starts_with("your pick for code on Brain Storming"),
            "{:?}",
            card.reason
        );
        assert!(card.fallbacks.iter().all(|f| f.model != id("cheap")));
        // A workspace's own pick wins over the default.
        save_pick(&conn, "ws1", "code", &p.id, "other").unwrap();
        let mut own = fresh();
        pick_models(&conn, &client, Some("ws1"), &mut own).unwrap();
        assert_eq!(own.cards[0].model.as_deref(), Some(id("other").as_str()));
        // A saved model that is no longer eligible gives way to the rule.
        save_pick(&conn, GLOBAL, "code", &p.id, "gone").unwrap();
        let mut stale = fresh();
        pick_models(&conn, &client, None, &mut stale).unwrap();
        assert_eq!(stale.cards[0].model.as_deref(), Some(rule.as_str()));
    }

    #[test]
    fn a_planner_gets_room_to_think_up_to_its_own_maximum() {
        use crate::gateway::store::{self, Capabilities, NewProvider};
        let conn = crate::db::open_in_memory().unwrap();
        let p = store::add_provider(
            &conn,
            &NewProvider {
                kind: "custom",
                label: "x",
                endpoint: "https://x.test/v1",
                credential_ref: None,
                default_billing: "paid",
                wire: "openai",
            },
        )
        .unwrap();
        for (model, most) in [("small", Some(8_000)), ("big", Some(64_000)), ("unknown", None)] {
            store::add_model(&conn, &p.id, model).unwrap();
            let caps = Capabilities { max_output_tokens: most, ..Capabilities::default() };
            store::set_capabilities(&conn, &p.id, model, &caps).unwrap();
        }
        let at = |m: &str| planner_max_tokens(&conn, &format!("{}/{m}", p.id));
        assert_eq!((at("small"), at("big"), at("unknown"), at("gone")), (8_000, 32_000, 32_000, 32_000));
    }

    #[test]
    fn a_reply_is_parsed_fenced_or_not_and_cards_get_waves() {
        let text = format!(
            "Here is the board:\n```json\n{}\n```",
            board(
                r#"{"id": "c1", "title": "Data model", "role": "plan", "done": "a written model", "dependsOn": []},
                    {"id": "c2", "title": "Catalogue", "role": "code", "done": "pnpm test passes", "dependsOn": ["c1", "q1"]},
                    {"id": "c3", "title": "Checkout", "role": "code", "done": "a test charge", "dependsOn": ["c2", "q9"]},
                    {"id": "c4", "title": "Pictures", "role": "image", "done": "x", "dependsOn": []},
                    {"title": "no id, no done", "role": "code"}"#
            )
        );
        let p = parse_reply(&text).unwrap();
        assert_eq!(p.questions.len(), 1);
        assert_eq!(
            p.cards.iter().map(|c| (c.id.as_str(), c.wave)).collect::<Vec<_>>(),
            [("c1", 1), ("c2", 2), ("c3", 3)]
        );
        assert_eq!(p.dropped_cards, 2, "an image card and a card without a done criterion are dropped");
        assert_eq!(p.cards[2].depends_on, ["c2"], "a dependency on nothing known is dropped");
        assert_eq!(p.cards[1].depends_on, ["c1", "q1"]);
    }

    #[test]
    fn a_cycle_or_an_empty_board_is_refused() {
        let cycle = board(
            r#"{"id": "a", "title": "A", "role": "code", "done": "x", "dependsOn": ["b"]}, {"id": "b", "title": "B", "role": "code", "done": "y", "dependsOn": ["a"]}"#,
        );
        assert!(parse_reply(&cycle).unwrap_err().contains("cycle"));
        assert!(parse_reply(&board("")).unwrap_err().contains("no card"));
        assert!(parse_reply("Sure, I will plan it.").unwrap_err().contains("no JSON"));
    }

    #[test]
    fn the_last_user_text_is_read_from_every_request_shape() {
        let chat = json!({"messages": [{"role": "user", "content": "first"}, {"role": "assistant", "content": "x"}, {"role": "user", "content": [{"type": "text", "text": "build a shop"}]}]});
        assert_eq!(last_user_text(Surface::OpenAi, &chat).as_deref(), Some("build a shop"));
        let anthropic = json!({"messages": [{"role": "user", "content": [{"type": "text", "text": "build a shop"}]}]});
        assert_eq!(last_user_text(Surface::Anthropic, &anthropic).as_deref(), Some("build a shop"));
        let responses =
            json!({"input": [{"role": "user", "content": [{"type": "input_text", "text": "build a shop"}]}]});
        assert_eq!(last_user_text(Surface::Responses, &responses).as_deref(), Some("build a shop"));
        assert_eq!(
            last_user_text(Surface::Responses, &json!({"input": "build a shop"})).as_deref(),
            Some("build a shop")
        );
        assert_eq!(last_user_text(Surface::OpenAi, &json!({"messages": []})), None);
    }

    #[test]
    fn a_refinement_keeps_answered_questions_and_renames_a_reused_id() {
        let old = vec![
            Question { id: "q1".into(), text: "Which payment providers?".into(), answered: false },
            Question { id: "q2".into(), text: "Is there an admin area?".into(), answered: false },
        ];
        let mut refined = parse_reply(&format!(
            r#"{{"questions": [{{"id": "q1", "text": "Magic links or passwords?"}}, {{"id": "q9", "text": "Is there an admin area?"}}], "cards": [{}]}}"#,
            r#"{"id": "c1", "title": "Auth", "role": "code", "done": "sign-in works", "dependsOn": ["q1", "q9"]}"#
        ))
        .unwrap();
        merge_questions(&old, &mut refined);
        let ids: Vec<(&str, &str, bool)> =
            refined.questions.iter().map(|q| (q.id.as_str(), q.text.as_str(), q.answered)).collect();
        assert_eq!(
            ids,
            [
                ("q5", "Magic links or passwords?", false),
                ("q2", "Is there an admin area?", false),
                ("q1", "Which payment providers?", true)
            ]
        );
        assert_eq!(refined.cards[0].depends_on, ["q5", "q2"], "the card follows the renamed and the kept ids");
    }

    #[test]
    fn the_proposal_text_has_the_fixed_shape() {
        let mut p = parse_reply(&board(r#"{"id": "c1", "title": "Data model", "role": "plan", "done": "a written model"}, {"id": "c2", "title": "Catalogue", "role": "code", "done": "tests pass", "dependsOn": ["c1"]}"#)).unwrap();
        p.cards[1].model = Some("openrouter/anthropic/claude-sonnet-5".into());
        let b = Brainstorm {
            id: "b_1".into(),
            client_id: None,
            session: "s".into(),
            workspace_id: None,
            state: "proposed".into(),
            requirement: "build a shop".into(),
            planner: Some("gemini/gemini-3.1-pro".into()),
            prompt_version: PROMPT_VERSION.into(),
            revision: 1,
            proposal: p,
            raw_reply: None,
            note: None,
            error: None,
            created_at: 0,
            updated_at: 0,
            branch: None,
        };
        let text = proposal_text(&b);
        assert!(text.starts_with("Brainstorm b_1 · revision 1 · open it in Pygg: pygg://brainstorm/b_1\nPlanner: gemini/gemini-3.1-pro (brain-v3)\n"), "{text}");
        assert!(text.contains("1. Which payment providers?"));
        assert!(
            text.contains(
                "Wave 1:\n- c1 [plan] Data model — auto/plan -> nothing eligible\n  done when: a written model\n"
            ),
            "{text}"
        );
        assert!(
            text.contains(
                "Wave 2:\n- c2 [code] Catalogue — auto/code -> openrouter/anthropic/claude-sonnet-5 (after c1)\n"
            ),
            "{text}"
        );
        assert!(text.ends_with("Approve in Pygg; this client cannot approve. Paid cards still wait for approval one by one when they start.\n"));
    }

    #[test]
    fn the_reply_takes_the_clients_shape() {
        let body = json!({"model": MODEL, "messages": [{"role": "user", "content": "build a shop"}]});
        let headers = HeaderMap::new();
        let reply = Reply { text: "Brainstorm b_1".into(), brainstorm_id: "b_1".into() };
        for (surface, stream) in [
            (Surface::OpenAi, false),
            (Surface::OpenAi, true),
            (Surface::Anthropic, false),
            (Surface::Anthropic, true),
            (Surface::Responses, false),
            (Surface::Responses, true),
        ] {
            let incoming = Incoming {
                surface,
                body: &body,
                headers: &headers,
                requested: MODEL,
                session: None,
                stream,
                route_kind: None,
            };
            let out = respond(&incoming, &reply);
            assert_eq!(out.status(), 200);
            assert_eq!(out.headers()[HEADER], "b_1");
            let content_type = out.headers()[header::CONTENT_TYPE].to_str().unwrap().to_string();
            assert_eq!(content_type.starts_with("text/event-stream"), stream, "{surface:?} {stream}");
        }
    }
}
