//! Phase 0 benchmark harness (ADR-013). Runs coding tasks with deterministic checks through an agent CLI,
//! one Git worktree per attempt, and records pass/fail, attempts and time for three kinds of run:
//! each rung alone (the "always T2", "always T4" baselines) and a cascade up the rungs on check failure.
//!
//! Cost is recorded as unknown for now: agent CLIs report no usage the harness can trust. Metering
//! through the gateway comes in Phase 1, and the report says so instead of showing $0.

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
    /// Always `unknown` until usage is metered through the gateway.
    pub cost_status: &'static str,
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
}

pub fn run_label(rungs: &[Rung], cascade: bool) -> String {
    if cascade {
        format!("cascade:{}", rungs.iter().map(|r| format!("T{}", r.tier)).collect::<Vec<_>>().join(">"))
    } else {
        format!("single:T{}", rungs[0].tier)
    }
}

/// Runs one case: each rung in order until an attempt passes all checks (cascade), or only the first rung.
pub async fn run_case(case: &Case, rungs: &[Rung], cascade: bool, config: &Config) -> Result<CaseRecord, String> {
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
    let ladder = if cascade { rungs } else { &rungs[..1] };
    for (index, rung) in ladder.iter().enumerate() {
        let number = index + 1;
        let worktree = isolation::create(&repo, &base, &slot.join(format!("attempt-{number}")))
            .map_err(|e| e.to_string())?;
        let command = config
            .agent
            .replace("{prompt_file}", &prompt_file.to_string_lossy())
            .replace("{model}", &rung.model)
            .replace("{workdir}", &worktree.path.to_string_lossy());
        let agent = process::run(&Spec::shell(&command, &worktree.path, config.agent_timeout), std::future::pending()).await;
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
            cost_status: "unknown",
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
    })
}

/// Pass rate and effort per run label, for the console report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summary {
    pub cases: usize,
    pub passed: usize,
    pub attempts: usize,
    pub total_ms: u64,
}

pub fn summarize<'a>(records: impl IntoIterator<Item = &'a CaseRecord>) -> Summary {
    let mut summary = Summary::default();
    for record in records {
        summary.cases += 1;
        summary.passed += record.passed as usize;
        summary.attempts += record.attempts.len();
        summary.total_ms += record.total_ms;
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
            'module.exports = (a, b) => a + b;\\n');\n").unwrap();
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
            scratch: dir.path().join("scratch") };
        let rungs = vec![Rung::parse("T0=bad").unwrap(), Rung::parse("T1=good").unwrap()];
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

        let cascade = rt.block_on(run_case(&cases[0], &rungs, true, &config)).unwrap();
        assert!(cascade.passed);
        assert_eq!(cascade.passing_tier, Some(1));
        assert_eq!(cascade.attempts.len(), 2);
        assert!(!cascade.attempts[0].passed && cascade.attempts[1].passed);
        assert_eq!(cascade.attempts[1].files_changed, 1);
        assert_eq!(cascade.run, "cascade:T0>T1");

        let single = rt.block_on(run_case(&cases[0], &rungs[..1], false, &config)).unwrap();
        assert!(!single.passed);
        assert_eq!(single.attempts.len(), 1);
        assert_eq!(single.attempts[0].checks[0].detail, "exited 1");

        let summary = summarize([&cascade, &single]);
        assert_eq!(summary, Summary { cases: 2, passed: 1, attempts: 3, total_ms: summary.total_ms });
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
