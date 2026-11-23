//! Phase 0 benchmark harness (ADR-013). Runs coding tasks with deterministic checks through an agent CLI,
//! one Git worktree per attempt, and records pass/fail, attempts and time for three kinds of run:
//! each rung alone (the "always T2", "always T4" baselines) and a cascade up the rungs on check failure.
//!
//! Cost comes from the agent's own event stream when it reports one (OpenCode's `--format json`
//! `step_finish` events carry tokens and a cost computed from its model catalog). That is an estimate at
//! list price, not a provider bill, so it is recorded as `estimated` with its source; an agent that
//! reports nothing is recorded as `unknown`, never as $0. Metering through the gateway comes in Phase 1.

use crate::checks::{self, CheckResult};
use crate::isolation::{self};
use crate::process::{self, Exit, Spec};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// `benchmark/cases/<id>/case.json`.
#[derive(Clone, Debug, Deserialize)]
pub struct CaseFile {
    pub id: String,
    pub title: String,
    pub category: String,
    pub complexity: String,
    pub prompt: String,
    /// Commands that must all exit 0, run in the attempt's worktree.
    pub checks: Vec<String>,
    /// Folder with the fixture files, relative to the case folder.
    #[serde(default = "default_repo")]
    pub repo: String,
    /// Fixture files the agent must leave as they are (the tests, the check scripts). An attempt that
    /// changes or deletes one fails without running its checks, since the checks would prove nothing.
    #[serde(default)]
    pub protected: Vec<String>,
}

/// The protected files an attempt changed or deleted, compared with the fixture.
pub fn changed_protected(case: &Case, worktree: &Path) -> Vec<String> {
    case.file
        .protected
        .iter()
        .filter(|path| {
            let original = std::fs::read(case.fixture.join(path.as_str()));
            let now = std::fs::read(worktree.join(path.as_str()));
            // Git may have changed line endings on checkout; compare the text without them.
            let strip = |bytes: Vec<u8>| bytes.into_iter().filter(|b| *b != b'\r').collect::<Vec<_>>();
            match (original, now) {
                (Ok(a), Ok(b)) => strip(a) != strip(b),
                _ => true,
            }
        })
        .cloned()
        .collect()
}

fn default_repo() -> String {
    "repo".into()
}

#[derive(Clone, Debug)]
pub struct Case {
    pub file: CaseFile,
    pub fixture: PathBuf,
}

/// Loads every `*/case.json` under `dir`, sorted by id.
pub fn load_cases(dir: &Path) -> Result<Vec<Case>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    let mut cases = Vec::new();
    for entry in entries.flatten() {
        let folder = entry.path();
        let manifest = folder.join("case.json");
        if !manifest.is_file() {
            continue;
        }
        let text = std::fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
        let file: CaseFile = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", manifest.display()))?;
        if file.checks.is_empty() {
            return Err(format!("{}: a case needs at least one check", manifest.display()));
        }
        let fixture = folder.join(&file.repo);
        if !fixture.is_dir() {
            return Err(format!("{}: fixture folder {} is missing", manifest.display(), fixture.display()));
        }
        cases.push(Case { file, fixture });
    }
    cases.sort_by(|a, b| a.file.id.cmp(&b.file.id));
    Ok(cases)
}

/// One step of the ladder: a tier and the model the agent is told to use.
#[derive(Clone, Debug, Serialize)]
pub struct Rung {
    pub tier: u8,
    pub model: String,
}

impl Rung {
    /// `T2=openrouter/some-model` or `2=some-model`.
    pub fn parse(text: &str) -> Result<Rung, String> {
        let (tier, model) = text.split_once('=').ok_or_else(|| format!("expected TIER=MODEL, got {text}"))?;
        let tier: u8 = tier.trim_start_matches(['T', 't']).parse().map_err(|_| format!("bad tier in {text}"))?;
        if tier > 4 || model.trim().is_empty() {
            return Err(format!("bad rung {text}: tier must be 0–4 and the model non-empty"));
        }
        Ok(Rung { tier, model: model.trim().to_string() })
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    /// Agent command line with placeholders `{prompt_file}`, `{model}` and `{workdir}`, run in the worktree
    /// through the system shell.
    pub agent: String,
    pub agent_timeout: Duration,
    pub check_timeout: Duration,
    /// Where fixtures are copied and worktrees created; removed afterwards by the caller.
    pub scratch: PathBuf,
    /// Hard cap on estimated spend across the whole run, in micro-USD. With a cap, no attempt starts once
    /// the cap is reached, and the run stops after any attempt whose cost is unknown.
    pub budget_micro_usd: Option<u64>,
    /// The store of the gateway the agent is routed through. Each attempt runs with its own session key in
    /// `PYGG_BENCH_SESSION` (the agent's configuration sends it as `x-pygg-session`), and the
    /// attempt's cost is what that gateway's ledger recorded for the session, at the prices set there.
    pub gateway_db: Option<PathBuf>,
    /// Keep going after an attempt whose cost could not be read (a timed-out free model, say). Only for runs whose
    /// paid routes all go through a gateway client with a strict limit, which then bounds the spend.
    pub allow_unknown: bool,
    /// The task engine's handoff mode for `--engine` runs: none, tail or evidence (H1, ADR-030). None: the default.
    pub handoff: Option<String>,
}

/// The environment variable that carries an attempt's session key to the agent's configuration.
pub const SESSION_ENV: &str = "PYGG_BENCH_SESSION";

/// What the gateway's ledger recorded for one attempt's session.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayUsage {
    pub requests: usize,
    pub failed: usize,
    /// Known cost (estimated at the gateway's prices, or reported by the provider).
    pub known_micro_usd: u64,
    /// Requests whose money cost is unknown: no price, or a failure the provider may have billed.
    pub unknown_money: usize,
    /// Of those, requests cut short (the agent stopped at its time limit with a request in flight) whose tokens so
    /// far and price are known: each is counted at those tokens, a floor of what it cost.
    pub cut_short: usize,
    /// What the cut-short requests cost at least, at the gateway's prices, micro-USD.
    pub floor_micro_usd: u64,
    /// Requests on a subscription: they use a quota, not money.
    pub quota: usize,
    /// `connection/model` of each target that answered, in first-use order.
    pub models: Vec<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

/// Reads the ledger of the gateway at `db` for `session`. The store is opened read-only; the gateway keeps
/// serving it.
pub fn gateway_usage(db: &Path, session: &str) -> Result<GatewayUsage, String> {
    use rusqlite::OpenFlags;
    let conn =
        rusqlite::Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
            .map_err(|e| format!("{}: {e}", db.display()))?;
    conn.busy_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    let mut statement = conn.prepare(
        "SELECT u.provider, u.model, u.outcome, u.input_tokens, u.output_tokens, u.cache_tokens, u.cache_write_tokens, \
         u.cost_micro_usd, u.cost_status, u.source, m.price_json FROM usage_events u \
         LEFT JOIN models m ON m.provider_id = u.provider AND m.model_id = u.model \
         WHERE u.session_key = ?1 ORDER BY u.created_at").map_err(|e| e.to_string())?;
    let mut rows = statement.query([session]).map_err(|e| e.to_string())?;
    let mut usage = GatewayUsage::default();
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let get_u = |i: usize| row.get::<_, Option<i64>>(i).ok().flatten().unwrap_or(0).max(0) as u64;
        let target =
            format!("{}/{}", row.get::<_, String>(0).unwrap_or_default(), row.get::<_, String>(1).unwrap_or_default());
        let outcome: String = row.get(2).unwrap_or_default();
        let status: String = row.get(8).unwrap_or_default();
        let source: String = row.get(9).unwrap_or_default();
        usage.requests += 1;
        if outcome == "ok" {
            if !usage.models.contains(&target) {
                usage.models.push(target);
            }
        } else {
            usage.failed += 1;
        }
        usage.input_tokens += get_u(3);
        usage.output_tokens += get_u(4);
        usage.cache_read_tokens += get_u(5);
        usage.cache_write_tokens += get_u(6);
        match status.as_str() {
            "unknown" if source.starts_with("subscription") => usage.quota += 1,
            "unknown" => {
                usage.unknown_money += 1;
                // A stream cut short has its tokens so far; at the model's price they are a floor of the cost.
                let price: Option<crate::gateway::pricing::Price> =
                    row.get::<_, Option<String>>(10).ok().flatten().and_then(|j| serde_json::from_str(&j).ok());
                if let (true, Some(price)) = (source.contains("cut short"), price) {
                    let tokens = crate::gateway::pricing::Usage {
                        prompt: get_u(3),
                        completion: get_u(4),
                        cached: get_u(5),
                        cache_write: get_u(6),
                        ..Default::default()
                    };
                    if let crate::gateway::pricing::Cost::Estimated { micro_usd, .. } =
                        crate::gateway::pricing::resolve("paid", Some(&price), Some(&tokens))
                    {
                        usage.cut_short += 1;
                        usage.floor_micro_usd += micro_usd;
                    }
                }
            }
            _ => usage.known_micro_usd += get_u(7),
        }
    }
    Ok(usage)
}

/// Where an attempt's cost through the gateway came from: the ledger, or the ledger with cut-short requests counted at
/// their tokens so far (a floor).
fn cost_source_of(g: &GatewayUsage) -> &'static str {
    if g.cut_short > 0 {
        "gateway ledger at the gateway's prices; cut-short requests at their tokens so far, a floor"
    } else {
        "gateway ledger at the gateway's prices"
    }
}

/// `estimated`, `floor` (known, with cut-short requests counted at their tokens so far) or `unknown`.
fn status_of(known: bool, gateway: Option<&GatewayUsage>) -> &'static str {
    match (known, gateway) {
        (false, _) => "unknown",
        (true, Some(g)) if g.requests > 0 && g.cut_short > 0 => "floor",
        (true, _) => "estimated",
    }
}

/// Running total for a benchmark run, shared by every case.
#[derive(Clone, Debug, Default)]
pub struct Ledger {
    pub spent_micro_usd: u64,
    /// Set when an attempt's cost could not be read, so the cap can no longer be enforced.
    pub unknown_attempts: usize,
}

impl Ledger {
    /// Why no further attempt may start, if one may not.
    pub fn stop_reason(&self, config: &Config) -> Option<String> {
        let cap = config.budget_micro_usd?;
        if self.unknown_attempts > 0 && !config.allow_unknown {
            return Some("an attempt's cost was unknown, so the budget cap cannot be enforced".into());
        }
        (self.spent_micro_usd >= cap)
            .then(|| format!("budget cap reached ({} of {} estimated)", usd(self.spent_micro_usd), usd(cap)))
    }
}

pub fn usd(micro: u64) -> String {
    format!("${:.4}", micro as f64 / 1_000_000.0)
}

/// Token counts summed over an attempt.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub reasoning: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

/// What the agent's JSON event stream said about one attempt.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AgentReport {
    /// `None` when no event carried a cost.
    pub cost_micro_usd: Option<u64>,
    pub tokens: Tokens,
    pub steps: usize,
    /// The first error event's message, e.g. an unavailable model or a missing key.
    pub error: Option<String>,
}

/// Reads OpenCode-style JSON lines: `step_finish` events with `part.tokens` and `part.cost` (USD), and
/// `error` events. Other lines, including non-JSON output, are ignored.
pub fn read_agent_events(stdout: &str) -> AgentReport {
    let mut report = AgentReport::default();
    let mut cost_usd = 0f64;
    let mut saw_cost = false;
    for line in stdout.lines() {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line.trim()) else { continue };
        match event.get("type").and_then(|t| t.as_str()) {
            Some("step_finish") => {
                let part = &event["part"];
                report.steps += 1;
                if let Some(cost) = part.get("cost").and_then(|c| c.as_f64()) {
                    if cost.is_finite() && cost >= 0.0 {
                        cost_usd += cost;
                        saw_cost = true;
                    }
                }
                let tokens = &part["tokens"];
                let n = |v: &serde_json::Value| v.as_u64().unwrap_or(0);
                report.tokens.input += n(&tokens["input"]);
                report.tokens.output += n(&tokens["output"]);
                report.tokens.reasoning += n(&tokens["reasoning"]);
                report.tokens.cache_read += n(&tokens["cache"]["read"]);
                report.tokens.cache_write += n(&tokens["cache"]["write"]);
            }
            Some("error") if report.error.is_none() => {
                let error = &event["error"];
                let message = error["data"]["message"].as_str().or_else(|| error["message"].as_str());
                let name = error["name"].as_str().unwrap_or("error");
                report.error = Some(match message {
                    Some(message) => format!("{name}: {message}"),
                    None => name.to_string(),
                });
            }
            _ => {}
        }
    }
    if saw_cost {
        report.cost_micro_usd = Some((cost_usd * 1_000_000.0).round() as u64);
    }
    report
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptRecord {
    pub number: usize,
    pub tier: u8,
    pub model: String,
    pub agent_exit: String,
    pub agent_ms: u64,
    pub checks: Vec<CheckResult>,
    pub passed: bool,
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
    /// `estimated` (the agent reported a cost from its price catalog) or `unknown`.
    pub cost_status: &'static str,
    /// Present only when `cost_status` is `estimated`.
    pub cost_micro_usd: Option<u64>,
    pub cost_source: Option<&'static str>,
    pub tokens: Tokens,
    pub agent_steps: usize,
    /// An error the agent reported, such as an unavailable model. Such an attempt says nothing about the
    /// model's ability, and the summary counts it separately.
    pub agent_error: Option<String>,
    /// What the gateway recorded for this attempt, when the run goes through one.
    pub gateway: Option<GatewayUsage>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseRecord {
    pub case_id: String,
    pub category: String,
    pub complexity: String,
    /// `single:T2` or `cascade:T0>T1>T2`.
    pub run: String,
    pub passed: bool,
    pub passing_tier: Option<u8>,
    pub attempts: Vec<AttemptRecord>,
    pub total_ms: u64,
    /// Set when the budget stopped the case before its ladder was finished.
    pub stopped: Option<String>,
    /// With `BENCH_HIDDEN`: whether the passing result also passed the case's hidden checks, which the agent and the
    /// ladder never saw. `None` when there are none or nothing passed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden_passed: Option<bool>,
}

impl CaseRecord {
    /// Estimated cost of all attempts, or `None` if any attempt's cost is unknown.
    pub fn cost_micro_usd(&self) -> Option<u64> {
        self.attempts.iter().map(|a| a.cost_micro_usd).sum()
    }

    pub fn had_agent_error(&self) -> bool {
        self.attempts.iter().any(|a| a.agent_error.is_some())
    }
}

pub fn run_label(rungs: &[Rung], cascade: bool) -> String {
    if cascade {
        format!("cascade:{}", rungs.iter().map(|r| format!("T{}", r.tier)).collect::<Vec<_>>().join(">"))
    } else {
        format!("single:T{}={}", rungs[0].tier, rungs[0].model)
    }
}

/// Runs one case: each rung in order until an attempt passes all checks (cascade), or only the first rung.
pub async fn run_case(
    case: &Case, rungs: &[Rung], cascade: bool, config: &Config, ledger: &mut Ledger,
) -> Result<CaseRecord, String> {
    let started = Instant::now();
    let label = run_label(rungs, cascade);
    // One flat folder: a model name's `/` would nest folders, and an agent that guesses an absolute path from
    // them can land outside its worktree (seen with Qwen, ROADMAP N4).
    let slot = config.scratch.join(format!(
        "{}-{}",
        case.file.id,
        label.replace([':', '>', '='], "_").replace(['/', '\\'], "-")
    ));
    let repo = slot.join("repo");
    copy_dir(&case.fixture, &repo).map_err(|e| format!("copying fixture: {e}"))?;
    let base = isolation::init_repository(&repo, &format!("fixture {}", case.file.id)).map_err(|e| e.to_string())?;
    // The prompt lives outside the worktree, so it is never part of the attempt's diff.
    let prompt_file = slot.join("prompt.txt");
    std::fs::write(&prompt_file, &case.file.prompt).map_err(|e| e.to_string())?;

    let mut attempts = Vec::new();
    let mut passing_tier = None;
    let mut stopped = None;
    let ladder = if cascade { rungs } else { &rungs[..1] };
    for (index, rung) in ladder.iter().enumerate() {
        if let Some(reason) = ledger.stop_reason(config) {
            stopped = Some(reason);
            break;
        }
        let number = index + 1;
        let worktree =
            isolation::create(&repo, &base, &slot.join(format!("attempt-{number}"))).map_err(|e| e.to_string())?;
        let command = config
            .agent
            .replace("{prompt_file}", &prompt_file.to_string_lossy())
            .replace("{model}", &rung.model)
            .replace("{workdir}", &worktree.path.to_string_lossy());
        let mut spec = Spec::shell(&command, &worktree.path, config.agent_timeout);
        // Event streams include tool output; keep enough that usage events are not cut from the middle.
        spec.output_limit = 32 * 1024 * 1024;
        let session = format!("bench-{}", uuid::Uuid::new_v4().simple());
        spec.env.push((SESSION_ENV.into(), session.clone()));
        let agent = process::run(&spec, std::future::pending()).await;
        let report = read_agent_events(&agent.stdout);
        let gateway = match &config.gateway_db {
            Some(db) => {
                // The last request is settled as its response ends; give the gateway a moment to write it.
                tokio::time::sleep(Duration::from_millis(1500)).await;
                Some(gateway_usage(db, &session)?)
            }
            None => None,
        };
        // Traffic through the gateway is costed from its ledger; the agent cannot know which model a route
        // chose, nor the gateway's prices. An attempt that sent nothing there is costed from the agent.
        let routed = gateway.as_ref().filter(|g| g.requests > 0);
        let (cost_known, cost_micro_usd, cost_source) = match routed {
            Some(g) => {
                // Cut-short requests count at their tokens so far; only a request with no such floor leaves the
                // attempt's cost unknown and the cap unenforceable.
                ledger.spent_micro_usd += g.known_micro_usd + g.floor_micro_usd;
                if g.unknown_money > g.cut_short {
                    ledger.unknown_attempts += 1;
                }
                (g.unknown_money == g.cut_short, Some(g.known_micro_usd + g.floor_micro_usd), cost_source_of(g))
            }
            None => {
                match report.cost_micro_usd {
                    // A truncated stream may have lost usage events, so its total is a floor, not an estimate.
                    Some(cost) if !agent.truncated => ledger.spent_micro_usd += cost,
                    Some(cost) => {
                        ledger.spent_micro_usd += cost;
                        ledger.unknown_attempts += 1;
                    }
                    None => ledger.unknown_attempts += 1,
                }
                (
                    report.cost_micro_usd.is_some() && !agent.truncated,
                    report.cost_micro_usd,
                    "agent-reported, catalog list price",
                )
            }
        };
        let agent_exit = match &agent.exit {
            Exit::Code(code) => format!("exited {code}"),
            Exit::Abnormal => "ended abnormally".into(),
            Exit::TimedOut => "timed out".into(),
            Exit::Canceled => "canceled".into(),
            Exit::FailedToStart(e) => format!("could not start: {e}"),
        };

        // Checks run even when the agent exits non-zero: some agents do, after writing a correct change.
        // They stop at the first failure.
        let mut results = Vec::new();
        let tampered = changed_protected(case, &worktree.path);
        if !tampered.is_empty() {
            results.push(CheckResult {
                command: "protected files unchanged".into(),
                passed: false,
                detail: format!("changed {}", tampered.join(", ")),
                output_tail: String::new(),
                duration_ms: 0,
            });
        }
        for check in case.file.checks.iter().filter(|_| tampered.is_empty()) {
            let result = checks::run(check, &worktree.path, config.check_timeout, &[], std::future::pending()).await;
            let failed = !result.passed;
            results.push(result);
            if failed {
                break;
            }
        }
        let passed = tampered.is_empty() && results.len() == case.file.checks.len() && results.iter().all(|r| r.passed);
        let stat = isolation::changes(&worktree).unwrap_or_default();
        isolation::discard(&worktree).map_err(|e| e.to_string())?;
        attempts.push(AttemptRecord {
            number,
            tier: rung.tier,
            model: rung.model.clone(),
            agent_exit,
            agent_ms: agent.duration.as_millis() as u64,
            checks: results,
            passed,
            files_changed: stat.files,
            insertions: stat.insertions,
            deletions: stat.deletions,
            cost_status: status_of(cost_known, gateway.as_ref()),
            cost_micro_usd: cost_micro_usd.filter(|_| cost_known),
            cost_source: cost_known.then_some(cost_source),
            tokens: report.tokens,
            agent_steps: report.steps,
            agent_error: report.error.map(|e| crate::redact::redact(&e, &[])),
            gateway,
        });
        if passed {
            passing_tier = Some(rung.tier);
            break;
        }
    }
    Ok(CaseRecord {
        case_id: case.file.id.clone(),
        category: case.file.category.clone(),
        complexity: case.file.complexity.clone(),
        run: label,
        passed: passing_tier.is_some(),
        passing_tier,
        attempts,
        total_ms: started.elapsed().as_millis() as u64,
        stopped,
        hidden_passed: None,
    })
}

/// Runs one case through Pygg's own task engine (ROADMAP N3), as the app would: the fixture becomes a workspace,
/// its checks are approved, and the rungs become the task's steps for `agent`. A failed check hands its output to
/// the next step, as in the app; `run_case` instead sends every rung the same prompt. Protected files are guarded
/// by an approved check that runs first (`git diff --quiet HEAD -- …`). Each attempt runs with
/// `PYGG_ATTEMPT_ID` set to its id; with a gateway store, the attempt is costed from that session's entries.
/// The ladder runs to its end once started: the budget is checked between cases, so the gateway client's own
/// (strict) limit is what bounds a single case.
pub async fn run_case_engine(
    case: &Case, rungs: &[Rung], cascade: bool, agent: &str, config: &Config, ledger: &mut Ledger,
) -> Result<CaseRecord, String> {
    use crate::tasks;
    use std::sync::{Arc, Mutex};
    let started = Instant::now();
    let label = format!("engine:{}", run_label(rungs, cascade));
    if let Some(reason) = ledger.stop_reason(config) {
        return Ok(CaseRecord {
            case_id: case.file.id.clone(),
            category: case.file.category.clone(),
            complexity: case.file.complexity.clone(),
            run: label,
            passed: false,
            passing_tier: None,
            attempts: Vec::new(),
            total_ms: 0,
            stopped: Some(reason),
            hidden_passed: None,
        });
    }
    let slot =
        config.scratch.join(format!("{}-{}", case.file.id, label.replace([':', '>', '='], "_").replace('/', "-")));
    let repo = slot.join("repo");
    copy_dir(&case.fixture, &repo).map_err(|e| format!("copying fixture: {e}"))?;
    isolation::init_repository(&repo, &format!("fixture {}", case.file.id)).map_err(|e| e.to_string())?;
    let conn = crate::db::open(&slot.join("store.sqlite3")).map_err(|e| e.to_string())?;
    let workspace = tasks::add_workspace(&conn, &repo.to_string_lossy(), Some(&case.file.id))?;
    if !case.file.protected.is_empty() {
        let guard = format!("git diff --quiet HEAD -- {}", case.file.protected.join(" "));
        tasks::add_check(&conn, &workspace.id, &guard)?;
    }
    for check in &case.file.checks {
        tasks::add_check(&conn, &workspace.id, check)?;
    }
    let ladder = if cascade { rungs } else { &rungs[..1] };
    let steps = ladder
        .iter()
        .map(|r| tasks::Step {
            agent: agent.into(),
            model: Some(r.model.clone()),
            tier: r.tier,
            billing: "unknown".into(),
        })
        .collect();
    let new = tasks::NewTask {
        workspace_id: workspace.id.clone(),
        title: Some(case.file.title.clone()),
        prompt: case.file.prompt.clone(),
        strategy: "free_first".into(),
        steps,
        allow_shell: false,
        timeout_secs: Some(config.agent_timeout.as_secs()),
        handoff: config.handoff.clone(),
        answer: false,
        cap_usd: None,
        allow_web: false,
        checks: Vec::new(),
    };
    let id = tasks::create_task(&conn, &new)?;
    let db = Arc::new(Mutex::new(conn));
    let runner = tasks::Runner::new(db.clone(), slot.join("worktrees"), tasks::agent_executor());
    runner.start(&id).ok_or("the task did not start")?.await.map_err(|e| e.to_string())?;
    let detail = tasks::task_detail(&db.lock().unwrap_or_else(|p| p.into_inner()), &id)?;

    let mut attempts = Vec::new();
    for (i, a) in detail.attempt_details.iter().enumerate() {
        let gateway = match &config.gateway_db {
            Some(store) => {
                tokio::time::sleep(Duration::from_millis(500)).await;
                Some(gateway_usage(store, &a.id)?)
            }
            None => None,
        };
        let routed = gateway.as_ref().filter(|g| g.requests > 0);
        let agent_estimate = a.agent_estimate_usd.map(|usd| (usd * 1_000_000.0).round() as u64);
        let (known, micro, source) = match routed {
            Some(g) => (g.unknown_money == g.cut_short, Some(g.known_micro_usd + g.floor_micro_usd), cost_source_of(g)),
            None => (agent_estimate.is_some(), agent_estimate, "agent-reported, catalog list price"),
        };
        match (known, micro) {
            (true, Some(m)) => ledger.spent_micro_usd += m,
            (false, Some(m)) => {
                ledger.spent_micro_usd += m;
                ledger.unknown_attempts += 1;
            }
            _ => ledger.unknown_attempts += 1,
        }
        let tokens = Tokens {
            input: a.input_tokens.unwrap_or(0).max(0) as u64,
            output: a.output_tokens.unwrap_or(0).max(0) as u64,
            ..Tokens::default()
        };
        attempts.push(AttemptRecord {
            number: i + 1,
            tier: a.tier,
            model: a.agent_model.clone().or_else(|| ladder.get(a.step).map(|r| r.model.clone())).unwrap_or_default(),
            agent_exit: a.detail.clone().unwrap_or_default(),
            agent_ms: match (a.started_at, a.finished_at) {
                (Some(s), Some(f)) => (f - s).max(0) as u64,
                _ => 0,
            },
            passed: a.outcome == "passed",
            checks: a.checks.clone(),
            files_changed: a.files_changed.unwrap_or(0) as usize,
            insertions: a.insertions.unwrap_or(0) as usize,
            deletions: a.deletions.unwrap_or(0) as usize,
            cost_status: status_of(known, gateway.as_ref()),
            cost_micro_usd: micro.filter(|_| known),
            cost_source: known.then_some(source),
            tokens,
            agent_steps: 0,
            agent_error: (a.outcome != "passed" && a.checks.is_empty()).then(|| a.detail.clone().unwrap_or_default()),
            gateway,
        });
    }
    let passing_tier = detail.attempt_details.iter().find(|a| a.outcome == "passed").map(|a| a.tier);
    let hidden_passed = match (
        std::env::var_os("BENCH_HIDDEN"),
        detail.attempt_details.iter().find(|a| a.outcome == "passed").and_then(|a| a.worktree.clone()),
    ) {
        (Some(dir), Some(worktree)) => hidden_checks(case, Path::new(&dir), Path::new(&worktree), config).await,
        _ => None,
    };
    // The result's worktree waits for the user in the app; here it is only measured.
    let _ = tasks::discard(&db.lock().unwrap_or_else(|p| p.into_inner()), &id);
    Ok(CaseRecord {
        case_id: case.file.id.clone(),
        category: case.file.category.clone(),
        complexity: case.file.complexity.clone(),
        run: label,
        passed: passing_tier.is_some() && detail.summary.state == "checks_passed",
        passing_tier,
        attempts,
        total_ms: started.elapsed().as_millis() as u64,
        stopped: None,
        hidden_passed,
    })
}

/// The case's hidden checks on a passing result: the files under `<dir>/<case id>` (the full tests the agent saw only
/// part of) copied over the result, then the case's checks run again. `None` when the case has no hidden files.
async fn hidden_checks(case: &Case, dir: &Path, worktree: &Path, config: &Config) -> Option<bool> {
    let source = dir.join(&case.file.id);
    if !source.is_dir() {
        return None;
    }
    fn copy(from: &Path, to: &Path) -> std::io::Result<()> {
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            let target = to.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                std::fs::create_dir_all(&target)?;
                copy(&entry.path(), &target)?;
            } else {
                std::fs::copy(entry.path(), &target)?;
            }
        }
        Ok(())
    }
    if copy(&source, worktree).is_err() {
        return Some(false);
    }
    for check in &case.file.checks {
        if !checks::run(check, worktree, config.check_timeout, &[], std::future::pending()).await.passed {
            return Some(false);
        }
    }
    Some(true)
}

/// Pass rate and effort per run label, for the console report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summary {
    pub cases: usize,
    pub passed: usize,
    pub attempts: usize,
    pub total_ms: u64,
    /// Estimated total, or `None` when any attempt's cost is unknown.
    pub cost_micro_usd: Option<u64>,
    /// Cases with at least one attempt the agent reported as an error (unavailable model, missing key).
    pub agent_errors: usize,
    /// Cases the budget stopped before their ladder was finished.
    pub stopped: usize,
}

impl Summary {
    /// Estimated cost per passing case; `None` when nothing passed or the cost is unknown.
    pub fn cost_per_success(&self) -> Option<u64> {
        let cost = self.cost_micro_usd?;
        (self.passed > 0).then(|| cost / self.passed as u64)
    }
}

pub fn summarize<'a>(records: impl IntoIterator<Item = &'a CaseRecord>) -> Summary {
    let mut summary = Summary { cost_micro_usd: Some(0), ..Summary::default() };
    for record in records {
        summary.cases += 1;
        summary.passed += record.passed as usize;
        summary.attempts += record.attempts.len();
        summary.total_ms += record.total_ms;
        summary.cost_micro_usd = summary.cost_micro_usd.zip(record.cost_micro_usd()).map(|(a, b)| a + b);
        summary.agent_errors += record.had_agent_error() as usize;
        summary.stopped += record.stopped.is_some() as usize;
    }
    summary
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn node_available() -> bool {
        std::process::Command::new("node").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
    }

    /// A case whose fixture has a sign bug, and a fake agent that fixes it only when told the model "good".
    fn setup(dir: &Path) -> (Vec<Case>, String) {
        let case_dir = dir.join("cases").join("sum-bug");
        fs::create_dir_all(case_dir.join("repo")).unwrap();
        fs::write(
            case_dir.join("case.json"),
            r#"{"id":"sum-bug","title":"Fix sum","category":"bug_fix",
            "complexity":"low","prompt":"Make sum add.","checks":["node test.js"]}"#,
        )
        .unwrap();
        fs::write(case_dir.join("repo/sum.js"), "module.exports = (a, b) => a - b;\n").unwrap();
        fs::write(
            case_dir.join("repo/test.js"),
            "const sum = require('./sum');\nif (sum(2, 3) !== 5) { console.error('expected 5'); process.exit(1); }\n",
        )
        .unwrap();
        let fixer = dir.join("fixer.js");
        fs::write(&fixer, "if (process.argv[2] === 'good') require('fs').writeFileSync('sum.js', \
            'module.exports = (a, b) => a + b;\\n');\n\
            if (process.argv[3]) console.log(JSON.stringify({type: 'step_finish', part: {cost: Number(process.argv[3]), \
            tokens: {input: 100, output: 10, reasoning: 0, cache: {read: 5, write: 0}}}}));\n").unwrap();
        let cases = load_cases(&dir.join("cases")).unwrap();
        (cases, format!("node \"{}\" {{model}}", fixer.display()))
    }

    #[test]
    fn cascade_escalates_until_the_check_passes() {
        if !node_available() {
            eprintln!("skipped: node is not installed");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let (cases, agent) = setup(dir.path());
        let config = Config {
            agent,
            agent_timeout: Duration::from_secs(60),
            check_timeout: Duration::from_secs(60),
            scratch: dir.path().join("scratch"),
            budget_micro_usd: None,
            gateway_db: None,
            allow_unknown: false,
            handoff: None,
        };
        let rungs = vec![Rung::parse("T0=bad").unwrap(), Rung::parse("T1=good").unwrap()];
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let mut ledger = Ledger::default();

        let cascade = rt.block_on(run_case(&cases[0], &rungs, true, &config, &mut ledger)).unwrap();
        assert!(cascade.passed);
        assert_eq!(cascade.passing_tier, Some(1));
        assert_eq!(cascade.attempts.len(), 2);
        assert!(!cascade.attempts[0].passed && cascade.attempts[1].passed);
        assert_eq!(cascade.attempts[1].files_changed, 1);
        assert_eq!(cascade.run, "cascade:T0>T1");

        let single = rt.block_on(run_case(&cases[0], &rungs[..1], false, &config, &mut ledger)).unwrap();
        assert!(!single.passed);
        assert_eq!(single.attempts.len(), 1);
        assert_eq!(single.attempts[0].checks[0].detail, "exited 1");

        // The fake agent reports no usage, so cost stays unknown rather than $0.
        assert_eq!(single.attempts[0].cost_status, "unknown");
        let summary = summarize([&cascade, &single]);
        assert_eq!(
            summary,
            Summary {
                cases: 2,
                passed: 1,
                attempts: 3,
                total_ms: summary.total_ms,
                cost_micro_usd: None,
                agent_errors: 0,
                stopped: 0
            }
        );
    }

    #[test]
    fn budget_cap_stops_the_ladder() {
        if !node_available() {
            eprintln!("skipped: node is not installed");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let (cases, agent) = setup(dir.path());
        // Every attempt reports $0.50; the cap is $0.40, so the cascade stops after its first attempt.
        let config = Config {
            agent: format!("{agent} 0.5"),
            agent_timeout: Duration::from_secs(60),
            check_timeout: Duration::from_secs(60),
            scratch: dir.path().join("scratch"),
            budget_micro_usd: Some(400_000),
            gateway_db: None,
            allow_unknown: false,
            handoff: None,
        };
        let rungs = vec![Rung::parse("T0=bad").unwrap(), Rung::parse("T1=good").unwrap()];
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let mut ledger = Ledger::default();

        let record = rt.block_on(run_case(&cases[0], &rungs, true, &config, &mut ledger)).unwrap();
        assert_eq!(record.attempts.len(), 1);
        assert!(!record.passed);
        assert!(record.stopped.as_deref().unwrap().contains("budget cap reached"));
        assert_eq!(record.attempts[0].cost_status, "estimated");
        assert_eq!(record.attempts[0].cost_micro_usd, Some(500_000));
        assert_eq!(record.attempts[0].tokens.cache_read, 5);
        assert_eq!(ledger.spent_micro_usd, 500_000);
    }

    #[test]
    fn reads_opencode_events() {
        let stdout = concat!(
            "not json\n",
            r#"{"type":"step_start","part":{}}"#,
            "\n",
            r#"{"type":"step_finish","part":{"tokens":{"input":10,"output":2,"reasoning":1,"cache":{"read":3,"write":4}},"cost":0.0025}}"#,
            "\n",
            r#"{"type":"step_finish","part":{"tokens":{"input":5,"output":1,"reasoning":0,"cache":{"read":0,"write":0}},"cost":0.0005}}"#,
            "\n",
        );
        let report = read_agent_events(stdout);
        assert_eq!(report.cost_micro_usd, Some(3000));
        assert_eq!(report.steps, 2);
        assert_eq!(report.tokens, Tokens { input: 15, output: 3, reasoning: 1, cache_read: 3, cache_write: 4 });
        assert_eq!(report.error, None);

        let failed = read_agent_events(
            r#"{"type":"error","error":{"name":"APIError","data":{"message":"Model is unavailable."}}}"#,
        );
        assert_eq!(failed.cost_micro_usd, None);
        assert_eq!(failed.error.as_deref(), Some("APIError: Model is unavailable."));
    }

    #[test]
    fn an_attempt_that_edits_a_protected_file_fails() {
        if !node_available() {
            eprintln!("skipped: node is not installed");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        setup(dir.path());
        fs::write(
            dir.path().join("cases/sum-bug/case.json"),
            r#"{"id":"sum-bug","title":"Fix sum","category":"bug_fix",
            "complexity":"low","prompt":"Make sum add.","checks":["node test.js"],"protected":["test.js"]}"#,
        )
        .unwrap();
        let cases = load_cases(&dir.path().join("cases")).unwrap();
        // Empties the test instead of fixing the code: the check would pass.
        let cheat = dir.path().join("cheat.js");
        fs::write(&cheat, "require('fs').writeFileSync('test.js', '// nothing\\n');\n").unwrap();
        let config = Config {
            agent: format!("node \"{}\"", cheat.display()),
            agent_timeout: Duration::from_secs(60),
            check_timeout: Duration::from_secs(60),
            scratch: dir.path().join("scratch"),
            budget_micro_usd: None,
            gateway_db: None,
            allow_unknown: false,
            handoff: None,
        };
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let record = rt
            .block_on(run_case(&cases[0], &[Rung::parse("T0=x").unwrap()], false, &config, &mut Ledger::default()))
            .unwrap();
        assert!(!record.passed);
        assert_eq!(record.attempts[0].checks.len(), 1);
        assert_eq!(record.attempts[0].checks[0].detail, "changed test.js");
    }

    #[test]
    fn reads_an_attempts_cost_from_the_gateway_ledger() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.sqlite3");
        let conn = crate::db::open(&path).unwrap();
        let insert =
            |id: &str, session: &str, model: &str, outcome: &str, cost: Option<i64>, status: &str, source: &str| {
                conn.execute(
                    "INSERT INTO usage_events (id, provider, model, input_tokens, output_tokens, cache_tokens, \
                cache_write_tokens, cost_micro_usd, cost_status, source, created_at, session_key, outcome) \
                VALUES (?1, 'anthropic', ?2, 1000, 50, 800, 100, ?3, ?4, ?5, 1, ?6, ?7)",
                    rusqlite::params![id, model, cost, status, source, session, outcome],
                )
                .unwrap();
            };
        insert("a", "s1", "claude-haiku-5-5", "ok", Some(120), "estimated", "price: user");
        insert("b", "s1", "claude-haiku-5-5", "ok", Some(80), "estimated", "price: user");
        insert("c", "s1", "qwen3.8-flash", "ok", None, "unknown", "subscription: counts against a quota, not money");
        insert("d", "s1", "claude-sonnet-5-5", "error", Some(0), "estimated", "rejected by the provider (HTTP 400)");
        insert("e", "s2", "claude-haiku-5-5", "ok", None, "unknown", "no price recorded for this model");
        // A stream the agent cut at its time limit: unknown in the ledger, a floor here at the model's price.
        insert(
            "f",
            "s3",
            "claude-haiku-5-5",
            "ok",
            None,
            "unknown",
            "the response was cut short before usage was reported",
        );
        insert(
            "g",
            "s4",
            "claude-opus-9",
            "ok",
            None,
            "unknown",
            "the response was cut short before usage was reported",
        );
        crate::gateway::store::add_provider(
            &conn,
            &crate::gateway::store::NewProvider {
                kind: "anthropic",
                label: "anthropic",
                endpoint: "https://x.test",
                credential_ref: None,
                default_billing: "paid",
                wire: "anthropic",
            },
        )
        .unwrap();
        let provider: String = conn.query_row("SELECT id FROM providers", [], |r| r.get(0)).unwrap();
        crate::gateway::store::add_model(&conn, &provider, "claude-haiku-5-5").unwrap();
        conn.execute(
            "UPDATE models SET price_json = ?1, provider_id = 'anthropic' WHERE model_id = 'claude-haiku-5-5'",
            [r#"{"inputMicroPerMtok":1000000,"outputMicroPerMtok":5000000,"cachedInputMicroPerMtok":100000,"source":"t","observedAt":0}"#],
        )
        .unwrap();
        drop(conn);

        let one = gateway_usage(&path, "s1").unwrap();
        assert_eq!((one.requests, one.failed, one.known_micro_usd, one.unknown_money, one.quota), (4, 1, 200, 0, 1));
        assert_eq!(one.models, vec!["anthropic/claude-haiku-5-5", "anthropic/qwen3.8-flash"]);
        assert_eq!((one.input_tokens, one.cache_read_tokens, one.cache_write_tokens), (4000, 3200, 400));
        let two = gateway_usage(&path, "s2").unwrap();
        assert_eq!((two.requests, two.unknown_money), (1, 1));
        assert_eq!(gateway_usage(&path, "none").unwrap().requests, 0);
        // 1000 in (800 read from cache, 100 written), 50 out at haiku's price: 100 × $1 + 800 × $0.10 + 100 × $1
        // + 50 × $5 per million = 530 micro-USD, a floor; a model with no price leaves the request unknown.
        let three = gateway_usage(&path, "s3").unwrap();
        assert_eq!((three.unknown_money, three.cut_short, three.floor_micro_usd), (1, 1, 530));
        assert_eq!(status_of(three.unknown_money == three.cut_short, Some(&three)), "floor");
        let four = gateway_usage(&path, "s4").unwrap();
        assert_eq!((four.unknown_money, four.cut_short), (1, 0));
        assert_eq!(status_of(four.unknown_money == four.cut_short, Some(&four)), "unknown");
    }

    #[test]
    fn parses_rungs() {
        let rung = Rung::parse("T2=openrouter/model-x").unwrap();
        assert_eq!((rung.tier, rung.model.as_str()), (2, "openrouter/model-x"));
        assert!(Rung::parse("T9=x").is_err());
        assert!(Rung::parse("T1=").is_err());
        assert!(Rung::parse("nomodel").is_err());
    }

    #[test]
    fn rejects_cases_without_checks() {
        let dir = tempfile::tempdir().unwrap();
        let case_dir = dir.path().join("c");
        fs::create_dir_all(case_dir.join("repo")).unwrap();
        fs::write(
            case_dir.join("case.json"),
            r#"{"id":"c","title":"t","category":"x","complexity":"low",
            "prompt":"p","checks":[]}"#,
        )
        .unwrap();
        assert!(load_cases(dir.path()).is_err());
    }
}
