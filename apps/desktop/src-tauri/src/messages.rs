//! Text the task engine writes for people: a task's reason, an attempt's escalation reason, a check's detail and a
//! step's label. It is stored and sent in English (the MCP server and the CLI pass it on as it is), with numbered
//! placeholders filled by `fill`. The app shows it in the interface language by matching these templates
//! (`src/lib/engineText.ts`) and translating each one; a test below keeps the two lists the same, so a sentence
//! changed here cannot silently stop being translated.
//!
//! Some of these are also compared by the engine (`TIMED_OUT`, `NO_CHANGES`): the stored text is the same in every
//! language, so those comparisons hold.

use std::fmt::Display;

// ---------- A task's reason ----------
pub const BUDGET_USED: &str = "Step {0} ({1}) is paid, and today's budget for paid steps is used; it resets at 00:00 UTC. Raise the budget in Settings, or start the task again after the reset.";
pub const WAITS_PAID: &str = "Step {0} ({1}) is marked paid, so it waits for your approval.";
pub const WAITS_SENSITIVE: &str = "Step {0} ({1}) waits for your approval: the workspace is sensitive, so every step asks before an agent sees its code.";
pub const STOPPED: &str = "You stopped it";
pub const DECLINED_TASK: &str = "You declined the step it was waiting on";
pub const HALTED: &str = "{0}. Pygg stopped before trying another step.";
pub const ALL_FAILED: &str = "Every step failed. Last: {0}";
pub const NO_ATTEMPT: &str = "no attempt ran";
pub const TASK_CAP: &str = "Stopped at this task's cap of {0}: it has cost {1} so far. No further step was started.";
pub const ANSWERED: &str = "Answered. Nothing was changed; the answer is below.";
pub const NO_ANSWER: &str = "The agent finished without an answer";
pub const ACCOUNT_HELD: &str = "Skipped {0}: its account is {1} ({2}). Fix it under Providers to use this model again.";
pub const NO_CHECKS: &str =
    "No checks are approved for this workspace, so nothing confirmed the result. Review the changes yourself.";
pub const NO_CHECKS_NOTE: &str =
    "No checks are approved for this workspace, so nothing confirmed the result. Review the changes yourself. {0}.";
pub const CHANGED_DEFINITIONS: &str = "The checks passed, but this attempt changed {0}, which defines them. Review the changes before trusting the result.";
pub const CHANGED_DEFINITIONS_NOTE: &str = "The checks passed, but this attempt changed {0}, which defines them. Review the changes before trusting the result. {1}.";
pub const NOTE: &str = "{0}.";
pub const DISCARDED: &str = "You discarded the result";
pub const INTERRUPTED: &str = "The app closed while this task ran. Retry runs the step again from the same code.";

// ---------- What a paid step cost against its cap ----------
pub const OVER_CAP: &str = "{0} for this step, ${1}, is over your cap of ${2}";
pub const PYGG_LEDGER: &str = "Pygg's ledger";
pub const OWN_ESTIMATE: &str = "{0}'s own estimate";
pub const UNPRICED: &str = "{0} of this step's requests through Pygg had no price (enter the model's price in Connections), so your cap for this step could not be checked";
pub const NO_COST: &str = "{0} reported no cost, so your cap for this step could not be checked";

// ---------- Why an attempt did not succeed ----------
pub const SKIPPED: &str = "Skipped: {0} timed out on its last two runs in the past 30 minutes without answering, so Pygg did not wait for it again";
pub const DECLINED_STEP: &str = "You declined this step";
pub const COULD_NOT_RUN: &str = "{0} could not run: {1}";
pub const DID_NOT_FINISH: &str = "{0} did not finish: {1}";
pub const NO_CHANGES: &str = "The agent made no changes";
pub const CHECK_FAILED: &str = "`{0}` {1}";
pub const LAST_FAILURE: &str = "{0}: {1}";

// ---------- How a run or a check ended ----------
pub const EXIT: &str = "exit {0}";
pub const EXITED: &str = "exited {0}";
pub const ABNORMAL: &str = "ended abnormally";
pub const TIMED_OUT: &str = "timed out";
pub const CANCELED: &str = "canceled";
pub const COULD_NOT_START: &str = "could not start: {0}";
pub const AGENT_ERROR: &str = "the agent reported an error";

// ---------- Why an agent could not start ----------
pub const UNKNOWN_AGENT: &str = "unknown agent {0}";
pub const DESCRIBE: &str = "Describe the task";
pub const NOT_A_REPOSITORY_WORKSPACE: &str =
    "The workspace must be a Git repository: the agent works on a separate copy of it.";
pub const NOT_INSTALLED: &str = "{0} is not installed, or not on PATH. See {1}";
pub const NOT_A_REPOSITORY: &str = "{0} is not a Git repository";
pub const GIT_MISSING: &str = "Git was not found on this computer";
pub const BRANCH_EXISTS: &str = "A branch named {0} already exists";
pub const GIT_FAILED: &str = "git {0} failed: {1}";

// ---------- What a task or workspace action refused ----------
pub const CHOOSE_FOLDER: &str = "Choose the workspace folder";
pub const NOT_A_FOLDER: &str = "{0} is not a folder";
pub const WORKSPACE_NOT_REPOSITORY: &str = "The workspace must be a Git repository: every attempt works on its own worktree, so your files and branch are never touched.";
pub const WORKSPACE_GONE: &str = "That workspace no longer exists";
pub const CHECK_GONE: &str = "That check no longer exists";
pub const TYPE_COMMAND: &str = "Type the command, e.g. pnpm test";
pub const CHECK_TOO_LONG: &str = "A check is one command line of at most 500 characters";
pub const UNKNOWN_PRIVACY: &str = "Unknown privacy class {0}";
pub const WORKSPACE_HAS_HISTORY: &str = "This workspace has task history, so it stays.";
pub const UNKNOWN_STRATEGY: &str = "Unknown strategy {0}";
pub const ADD_STEP: &str = "Add at least one step";
pub const TOO_MANY_STEPS: &str = "A task has at most {0} steps";
pub const UNKNOWN_STEP_AGENT: &str = "Unknown agent {0}";
pub const STEP_TIER: &str = "A step's tier is T0 to T4";
pub const UNKNOWN_BILLING: &str = "Unknown billing {0}";
pub const UNKNOWN_HANDOFF: &str = "Unknown handoff {0}: use none, tail or evidence";
pub const TASK_GONE: &str = "That task no longer exists";
pub const CANNOT_STOP: &str = "Only a running or waiting task can be stopped";
pub const NOT_WAITING: &str = "This task is not waiting for an approval";
pub const STEP_NOT_IN_POLICY: &str = "The waiting step is not in the task's policy";
pub const SET_CAP: &str = "Set a spending cap for this paid step";
pub const APPROVAL_STALE: &str =
    "This step's estimate or your budget changed since you were asked. Check the new estimate and approve again.";
pub const CAP_OVER_BUDGET: &str = "The cap is more than what is left of today's budget for paid steps (${0})";
pub const CANNOT_RETRY: &str = "Only an interrupted task can be retried";
pub const NO_BASE: &str = "The task has no base commit";
pub const NO_RESULT: &str = "This task has no result waiting for you";
pub const WORKTREE_GONE: &str = "The result's worktree is gone";

// ---------- A step's label ----------
pub const DEFAULT_MODEL: &str = "default model";
pub const LABEL: &str = "{0} · {1}";

/// Every template, for the test that compares them with the app's list.
pub const ALL: &[&str] = &[
    BUDGET_USED,
    WAITS_PAID,
    WAITS_SENSITIVE,
    STOPPED,
    DECLINED_TASK,
    HALTED,
    ALL_FAILED,
    NO_ATTEMPT,
    TASK_CAP,
    ANSWERED,
    NO_ANSWER,
    ACCOUNT_HELD,
    NO_CHECKS,
    NO_CHECKS_NOTE,
    CHANGED_DEFINITIONS,
    CHANGED_DEFINITIONS_NOTE,
    NOTE,
    DISCARDED,
    INTERRUPTED,
    OVER_CAP,
    PYGG_LEDGER,
    OWN_ESTIMATE,
    UNPRICED,
    NO_COST,
    SKIPPED,
    DECLINED_STEP,
    COULD_NOT_RUN,
    DID_NOT_FINISH,
    NO_CHANGES,
    CHECK_FAILED,
    LAST_FAILURE,
    EXIT,
    EXITED,
    ABNORMAL,
    TIMED_OUT,
    CANCELED,
    COULD_NOT_START,
    AGENT_ERROR,
    UNKNOWN_AGENT,
    DESCRIBE,
    NOT_A_REPOSITORY_WORKSPACE,
    NOT_INSTALLED,
    NOT_A_REPOSITORY,
    GIT_MISSING,
    BRANCH_EXISTS,
    GIT_FAILED,
    DEFAULT_MODEL,
    LABEL,
    CHOOSE_FOLDER,
    NOT_A_FOLDER,
    WORKSPACE_NOT_REPOSITORY,
    WORKSPACE_GONE,
    CHECK_GONE,
    TYPE_COMMAND,
    CHECK_TOO_LONG,
    UNKNOWN_PRIVACY,
    WORKSPACE_HAS_HISTORY,
    UNKNOWN_STRATEGY,
    ADD_STEP,
    TOO_MANY_STEPS,
    UNKNOWN_STEP_AGENT,
    STEP_TIER,
    UNKNOWN_BILLING,
    UNKNOWN_HANDOFF,
    TASK_GONE,
    CANNOT_STOP,
    NOT_WAITING,
    STEP_NOT_IN_POLICY,
    SET_CAP,
    APPROVAL_STALE,
    CAP_OVER_BUDGET,
    CANNOT_RETRY,
    NO_BASE,
    NO_RESULT,
    WORKTREE_GONE,
];

/// The template with `{0}`, `{1}` … replaced by the arguments, in one pass (an argument's own braces stay).
pub fn fill(template: &str, args: &[&dyn Display]) -> String {
    let mut out = String::with_capacity(template.len() + 32);
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}').and_then(|close| after[..close].parse::<usize>().ok().map(|n| (n, close))) {
            Some((n, close)) if n < args.len() => {
                out.push_str(&args[n].to_string());
                rest = &after[close + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_replaces_numbered_placeholders_once() {
        assert_eq!(
            fill(OVER_CAP, &[&PYGG_LEDGER, &"0.12", &"0.10"]),
            "Pygg's ledger for this step, $0.12, is over your cap of $0.10"
        );
        assert_eq!(fill(LAST_FAILURE, &[&"Codex", &"{0} stays"]), "Codex: {0} stays");
        assert_eq!(fill("{x} {1}", &[&"a"]), "{x} {1}");
    }

    /// The app translates engine text by matching these templates; one changed here without the app's list would show
    /// in English only.
    #[test]
    fn the_app_knows_every_template_and_no_other() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/lib/engineText.ts");
        let source = std::fs::read_to_string(&path).unwrap();
        let block =
            source.split("// templates: start").nth(1).and_then(|s| s.split("// templates: end").next()).unwrap();
        let listed: Vec<String> = block
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with('"'))
            .map(|l| serde_json::from_str(l.trim_end_matches(',')).unwrap())
            .collect();
        let mut ours: Vec<String> = ALL.iter().map(|s| s.to_string()).collect();
        let mut theirs = listed.clone();
        ours.sort();
        theirs.sort();
        assert_eq!(theirs, ours);
    }
}
