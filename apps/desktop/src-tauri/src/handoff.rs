//! H1 (ADR-030): the evidence a task's next step is given, built deterministically from what Pygg recorded about
//! earlier attempts at the same task. It is per task, bounded and shown to the user (ADR-028 keeps exactly this);
//! nothing is inferred, summarized by a model or carried to another task.
//!
//! Every line says where it comes from, and how far it can be trusted:
//! - "verified by a check": a check Pygg ran, with the end of its output;
//! - "changed in a discarded attempt": paths from that attempt's diff, none of which are in the new copy;
//! - "that agent's own account, unverified": the agent's final message, never promoted to a requirement.
//!
//! What Pygg cannot see it does not claim: which files an agent read, its reasoning, its tool calls.

/// How a task hands evidence from a failed attempt to the next step.
pub const MODES: [&str; 3] = ["none", "tail", "evidence"];
/// Today's behaviour, and the default until the A/B says otherwise (ADR-030).
pub const DEFAULT_MODE: &str = "tail";

pub const VERSION: &str = "pygg.task-handoff.v1";
/// The whole evidence block, at most; the oldest attempts are dropped first, and the drop is said.
const BUDGET_CHARS: usize = 6000;
/// The most recent failed check's output keeps this much; earlier ones less.
const LATEST_TAIL: usize = 1500;
const EARLIER_TAIL: usize = 400;
const SUMMARY_CHARS: usize = 500;
const PATHS_LISTED: usize = 20;

/// One earlier attempt, as the task engine recorded it.
pub struct Attempt<'a> {
    pub number: u32,
    pub label: &'a str,
    /// failed, skipped (others are not handed on).
    pub outcome: &'a str,
    /// The agent's exit as recorded: `exit 1`, `timed out`, or the no-changes marker.
    pub detail: Option<&'a str>,
    pub escalation_reason: Option<&'a str>,
    /// The first failing check: command, detail (`exited 1`), the end of its output.
    pub failed_check: Option<(&'a str, &'a str, &'a str)>,
    pub summary: Option<&'a str>,
    pub changed_paths: Vec<String>,
    pub no_changes: bool,
}

/// Removes terminal colour and cursor codes, and progress lines rewritten with a carriage return.
pub fn clean(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // CSI: ESC [ parameters final-byte
            if chars.peek() == Some(&'[') {
                chars.next();
                for d in chars.by_ref() {
                    if ('@'..='~').contains(&d) {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    // A line redrawn with \r keeps only what was drawn last.
    out.lines().map(|l| l.rsplit('\r').find(|s| !s.trim().is_empty()).unwrap_or("")).collect::<Vec<_>>().join("\n")
}

fn tail(text: &str, n: usize) -> (String, bool) {
    let chars: Vec<char> = text.chars().collect();
    let cut = chars.len() > n;
    (chars[chars.len().saturating_sub(n)..].iter().collect(), cut)
}

fn indent(text: &str) -> String {
    text.lines().map(|l| format!("    {l}")).collect::<Vec<_>>().join("\n")
}

fn section(a: &Attempt, latest: bool, earlier: &[&Attempt]) -> String {
    let mut s = String::new();
    if a.outcome == "skipped" {
        s.push_str(&format!("Attempt {} · {} — skipped: it was not answering.\n", a.number, a.label));
        return s;
    }
    let what = if a.failed_check.is_some() {
        "did not pass the checks"
    } else if a.no_changes {
        "ended without changing any file"
    } else {
        "did not finish"
    };
    s.push_str(&format!("Attempt {} · {} — {what}.\n", a.number, a.label));
    if let Some((command, detail, output)) = a.failed_check {
        let same = earlier
            .iter()
            .find(|e| e.failed_check.map(|(c, _, o)| c == command && clean(o) == clean(output)).unwrap_or(false));
        if let Some(e) = same {
            s.push_str(&format!(
                "- Verified by the check `{command}`: {detail}, with the same output as attempt {}.\n",
                e.number
            ));
        } else {
            let (text, cut) = tail(&clean(output), if latest { LATEST_TAIL } else { EARLIER_TAIL });
            s.push_str(&format!(
                "- Verified by the check `{command}`: {detail}. {}:\n{}\n",
                if cut { "The end of its output" } else { "Its output" },
                indent(text.trim_end())
            ));
        }
    } else if !a.no_changes {
        if let Some(reason) = a.escalation_reason.or(a.detail) {
            s.push_str(&format!("- Recorded by Pygg: {}\n", tail(reason, 300).0));
        }
    }
    if !a.changed_paths.is_empty() {
        let mut list: Vec<&str> = a.changed_paths.iter().map(String::as_str).take(PATHS_LISTED).collect();
        list.sort_unstable();
        let more = a.changed_paths.len().saturating_sub(PATHS_LISTED);
        s.push_str(&format!(
            "- Changed in that discarded attempt, and not in your copy: {}{}\n",
            list.join(", "),
            if more > 0 { format!(" and {more} more") } else { String::new() }
        ));
    }
    if let Some(summary) = a.summary.map(str::trim).filter(|t| !t.is_empty()) {
        // The start of a message says what the agent thinks it did; keep the head, not the tail.
        let summary = clean(summary);
        let head: String = summary.chars().take(SUMMARY_CHARS).collect();
        let shown = if summary.chars().count() > SUMMARY_CHARS { format!("{}…", head.trim_end()) } else { head };
        s.push_str(&format!("- That agent's own account, unverified: \"{}\"\n", shown.replace('\n', " ")));
    }
    s
}

/// The evidence block for the next step, or None when no earlier attempt failed.
pub fn evidence(attempts: &[Attempt]) -> Option<String> {
    let relevant: Vec<&Attempt> = attempts.iter().filter(|a| a.outcome == "failed" || a.outcome == "skipped").collect();
    if relevant.is_empty() || relevant.iter().all(|a| a.outcome == "skipped") {
        return None;
    }
    let last = relevant.len() - 1;
    let sections: Vec<String> =
        relevant.iter().enumerate().map(|(i, a)| section(a, i == last, &relevant[..i])).collect();
    // Newest first in the budget, then printed oldest first.
    let mut kept: Vec<usize> = Vec::new();
    let mut used = 0;
    for i in (0..sections.len()).rev() {
        if used + sections[i].len() > BUDGET_CHARS && !kept.is_empty() {
            break;
        }
        used += sections[i].len();
        kept.push(i);
    }
    kept.reverse();
    let omitted = sections.len() - kept.len();
    let mut out = format!(
        "{VERSION}: what earlier attempts at this task found. Each started from the same original code, \
        and none of their changes are in your copy.\n"
    );
    if omitted > 0 {
        out.push_str(&format!("({omitted} earlier attempt(s) omitted to keep this short.)\n"));
    }
    for i in kept {
        out.push('\n');
        out.push_str(&sections[i]);
    }
    Some(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failed<'a>(number: u32, output: &'a str, paths: &[&str], summary: Option<&'a str>) -> Attempt<'a> {
        Attempt {
            number,
            label: "OpenCode · ollama/qwen3:8b",
            outcome: "failed",
            detail: Some("exit 0"),
            escalation_reason: None,
            failed_check: Some(("node test.js", "exited 1", output)),
            summary,
            changed_paths: paths.iter().map(|p| p.to_string()).collect(),
            no_changes: false,
        }
    }

    #[test]
    fn says_where_each_line_comes_from_and_how_far_to_trust_it() {
        let attempts = [failed(
            1,
            "\u{1b}[31mFAIL\u{1b}[0m parse('a=1')\nexpected 1, got undefined\n",
            &["src/ini.js"],
            Some("I fixed the parser and all tests pass."),
        )];
        let text = evidence(&attempts).unwrap();
        assert!(text.starts_with(VERSION), "{text}");
        assert!(text.contains("none of their changes are in your copy"));
        assert!(text.contains("Verified by the check `node test.js`: exited 1"));
        assert!(text.contains("FAIL parse('a=1')") && !text.contains('\u{1b}'), "colour codes are removed: {text}");
        assert!(text.contains("Changed in that discarded attempt, and not in your copy: src/ini.js"));
        assert!(text.contains("That agent's own account, unverified: \"I fixed the parser and all tests pass.\""));
    }

    #[test]
    fn nothing_to_hand_on_without_a_failure() {
        assert!(evidence(&[]).is_none());
        let skipped = Attempt {
            number: 1,
            label: "x",
            outcome: "skipped",
            detail: None,
            escalation_reason: None,
            failed_check: None,
            summary: None,
            changed_paths: vec![],
            no_changes: false,
        };
        assert!(evidence(&[skipped]).is_none());
    }

    #[test]
    fn repeats_are_named_not_copied_and_old_attempts_give_way_to_the_budget() {
        let long = "x".repeat(5000);
        let a = [failed(1, &long, &[], None), failed(2, &long, &[], None), failed(3, "other", &[], None)];
        let text = evidence(&a).unwrap();
        assert!(text.contains("with the same output as attempt 1"), "{text}");
        assert!(text.len() <= BUDGET_CHARS + 400, "{}", text.len());
        let many: Vec<String> = (0..30).map(|i| format!("fail {i} {}", "y".repeat(1400))).collect();
        let attempts: Vec<Attempt> = many.iter().enumerate().map(|(i, o)| failed(i as u32 + 1, o, &[], None)).collect();
        let text = evidence(&attempts).unwrap();
        assert!(text.contains("omitted to keep this short") && text.contains("Attempt 30"), "the newest is kept");
    }

    #[test]
    fn a_progress_line_keeps_what_was_drawn_last() {
        assert_eq!(clean("10%\r50%\r100% done\nok"), "100% done\nok");
    }
}
