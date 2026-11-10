//! Phase 0 benchmark harness (ADR-013). Runs coding tasks with deterministic checks through an agent CLI,
//! one Git worktree per attempt, and records pass/fail, attempts and time for three kinds of run:
//! each rung alone (the "always T2", "always T4" baselines) and a cascade up the rungs on check failure.
//!
//! Cost comes from the agent's own event stream when it reports one (OpenCode's `--format json`
//! `step_finish` events carry tokens and a cost computed from its model catalog). That is an estimate at
//! list price, not a provider bill, so it is recorded as `estimated` with its source; an agent that
//! reports nothing is recorded as `unknown`, never as $0. Metering through the gateway comes in Phase 1.

use crate::checks::{self, CheckResult};
use crate::isolation::{self, DiffStat};
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
        if self.unknown_attempts > 0 {
            return Some("an attempt's cost was unknown, so the budget cap cannot be enforced".into());
        }
        (self.spent_micro_usd >= cap).then(|| {
            format!("budget cap reached ({} of {} estimated)", usd(self.spent_micro_usd), usd(cap))
        })
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
        format!("single:T{}", rungs[0].tier)
    }
}

/// Runs one case: each rung in order until an attempt passes all checks (cascade), or only the first rung.
pub async fn run_case(case: &Case, rungs: &[Rung], cascade: bool, config: &Config, ledger: &mut Ledger)
    -> Result<CaseRecord, String> {
    let started = Instant::now();
    let label = run_label(rungs, cascade);
    let slot = config.scratch.join(format!("{}-{}", case.file.id, label.replace([':', '>'], "_")));
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
        let worktree = isolation::create(&repo, &base, &slot.join(format!("attempt-{number}")))
            .map_err(|e| e.to_string())?;
        let command = config
            .agent
            .replace("{prompt_file}", &prompt_file.to_string_lossy())
            .replace("{model}", &rung.model)
            .replace("{workdir}", &worktree.path.to_string_lossy());
        let mut spec = Spec::shell(&command, &worktree.path, config.agent_timeout);
        // Event streams include tool output; keep enough that usage events are not cut from the middle.
        spec.output_limit = 32 * 1024 * 1024;
        let agent = process::run(&spec, std::future::pending()).await;
        let report = read_agent_events(&agent.stdout);
        match report.cost_micro_usd {
            // A truncated stream may have lost usage events, so its total is a floor, not an estimate.
            Some(cost) if !agent.truncated => ledger.spent_micro_usd += cost,
            Some(cost) => {
                ledger.spent_micro_usd += cost;
                ledger.unknown_attempts += 1;
            }
            None => ledger.unknown_attempts += 1,
        }
        let cost_known = report.cost_micro_usd.is_some() && !agent.truncated;
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
        for check in &case.file.checks {
            let result = checks::run(check, &worktree.path, config.check_timeout, &[], std::future::pending()).await;
            let failed = !result.passed;
            results.push(result);
            if failed {
                break;
            }
        }
        let passed = results.len() == case.file.checks.len() && results.iter().all(|r| r.passed);
        let stat = isolation::changes(&worktree).unwrap_or(DiffStat::default());
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
            cost_status: if cost_known { "estimated" } else { "unknown" },
            cost_micro_usd: report.cost_micro_usd.filter(|_| cost_known),
            cost_source: cost_known.then_some("agent-reported, catalog list price"),
            tokens: report.tokens,
            agent_steps: report.steps,
            agent_error: report.error.map(|e| crate::redact::redact(&e, &[])),
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
    })
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
        fs::write(case_dir.join("case.json"), r#"{"id":"sum-bug","title":"Fix sum","category":"bug_fix",
            "complexity":"low","prompt":"Make sum add.","checks":["node test.js"]}"#).unwrap();
        fs::write(case_dir.join("repo/sum.js"), "module.exports = (a, b) => a - b;\n").unwrap();
        fs::write(case_dir.join("repo/test.js"),
            "const sum = require('./sum');\nif (sum(2, 3) !== 5) { console.error('expected 5'); process.exit(1); }\n").unwrap();
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
        let config = Config { agent, agent_timeout: Duration::from_secs(60), check_timeout: Duration::from_secs(60),
            scratch: dir.path().join("scratch"), budget_micro_usd: None };
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
        assert_eq!(summary, Summary { cases: 2, passed: 1, attempts: 3, total_ms: summary.total_ms,
            cost_micro_usd: None, agent_errors: 0, stopped: 0 });
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
        let config = Config { agent: format!("{agent} 0.5"), agent_timeout: Duration::from_secs(60),
            check_timeout: Duration::from_secs(60), scratch: dir.path().join("scratch"),
            budget_micro_usd: Some(400_000) };
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
            r#"{"type":"step_start","part":{}}"#, "\n",
            r#"{"type":"step_finish","part":{"tokens":{"input":10,"output":2,"reasoning":1,"cache":{"read":3,"write":4}},"cost":0.0025}}"#, "\n",
            r#"{"type":"step_finish","part":{"tokens":{"input":5,"output":1,"reasoning":0,"cache":{"read":0,"write":0}},"cost":0.0005}}"#, "\n",
        );
        let report = read_agent_events(stdout);
        assert_eq!(report.cost_micro_usd, Some(3000));
        assert_eq!(report.steps, 2);
        assert_eq!(report.tokens, Tokens { input: 15, output: 3, reasoning: 1, cache_read: 3, cache_write: 4 });
        assert_eq!(report.error, None);

        let failed = read_agent_events(r#"{"type":"error","error":{"name":"APIError","data":{"message":"Model is unavailable."}}}"#);
        assert_eq!(failed.cost_micro_usd, None);
        assert_eq!(failed.error.as_deref(), Some("APIError: Model is unavailable."));
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
        fs::write(case_dir.join("case.json"), r#"{"id":"c","title":"t","category":"x","complexity":"low",
            "prompt":"p","checks":[]}"#).unwrap();
        assert!(load_cases(dir.path()).is_err());
    }
}
