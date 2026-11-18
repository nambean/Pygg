//! `pygg run` launches and what they bought (cost plan A1, docs/AGENTS.md "The session receipt"): the Git state of the
//! folder before the tool starts and after it exits, the launch's row in the store (migration 0038), and the receipt
//! and status line built from the ledger rows the launch sent with its session header.
//!
//! The receipt says what is known and nothing more. Money is the ledger's known costs; requests whose cost is unknown
//! or on a subscription are counted, never taken as $0. Git is read with plain `git` commands and only the
//! repository's folder, branch, commit ids and counts are kept, never file names or code. A change in Git is not proof
//! that the work was good: the receipt shows it so cost can be set against what came of it.

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::gateway::hints::RequestClasses;
use crate::gateway::store::{LimitScope, SessionLimits};

/// A repository's state at one moment, read from a folder inside it.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitState {
    /// The working tree's top folder.
    pub root: String,
    /// `None` on a detached HEAD.
    pub branch: Option<String>,
    /// `None` before the first commit.
    pub head: Option<String>,
    /// Tracked files with changes not committed.
    pub dirty: u64,
}

/// What changed from a launch's start to its end.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitChange {
    pub head: Option<String>,
    pub dirty: u64,
    /// Commits from the starting HEAD to the new one; `None` when the new HEAD does not descend from it (a reset, a
    /// switch to another branch).
    pub commits: Option<u64>,
    /// The starting HEAD against the working tree at the end, tracked files only.
    pub files: u64,
    pub insertions: u64,
    pub deletions: u64,
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    crate::isolation::git(dir, args).ok()
}

/// The repository `dir` is in, or `None` outside one (or without Git).
pub fn git_state(dir: &Path) -> Option<GitState> {
    let root = git(dir, &["rev-parse", "--show-toplevel"]).filter(|r| !r.is_empty())?;
    let head = git(dir, &["rev-parse", "--verify", "--quiet", "HEAD"]).filter(|h| !h.is_empty());
    let branch = git(dir, &["symbolic-ref", "--quiet", "--short", "HEAD"]).filter(|b| !b.is_empty());
    let dirty = git(dir, &["--no-optional-locks", "status", "--porcelain", "--untracked-files=no"])
        .map_or(0, |s| s.lines().filter(|l| !l.trim().is_empty()).count() as u64);
    Some(GitState { root, branch, head, dirty })
}

/// What changed in `dir`'s repository since `before`.
pub fn git_change(dir: &Path, before: &GitState) -> Option<GitChange> {
    let after = git_state(dir)?;
    let commits = match (before.head.as_deref(), after.head.as_deref()) {
        (Some(b), Some(a)) if a == b => Some(0),
        (Some(b), Some(a)) => crate::isolation::git(dir, &["merge-base", "--is-ancestor", b, a])
            .ok()
            .and_then(|_| git(dir, &["rev-list", "--count", &format!("{b}..{a}")]))
            .and_then(|n| n.parse().ok()),
        (None, Some(a)) => git(dir, &["rev-list", "--count", a]).and_then(|n| n.parse().ok()),
        (_, None) => Some(0),
    };
    let (files, insertions, deletions) = match before.head.as_deref() {
        Some(b) => git(dir, &["--no-optional-locks", "diff", "--shortstat", "--no-ext-diff", b])
            .map(|s| shortstat(&s))
            .unwrap_or_default(),
        None => (0, 0, 0),
    };
    Some(GitChange { head: after.head, dirty: after.dirty, commits, files, insertions, deletions })
}

/// `git diff --shortstat`'s line: " 3 files changed, 10 insertions(+), 2 deletions(-)", any part missing.
fn shortstat(line: &str) -> (u64, u64, u64) {
    let mut out = (0, 0, 0);
    for part in line.split(',') {
        let mut words = part.split_whitespace();
        let (Some(n), Some(what)) = (words.next().and_then(|n| n.parse::<u64>().ok()), words.next()) else {
            continue;
        };
        if what.starts_with("file") {
            out.0 = n;
        } else if what.starts_with("insertion") {
            out.1 = n;
        } else if what.starts_with("deletion") {
            out.2 = n;
        }
    }
    out
}

/// Whether the store has the launches table: a store the app has not migrated to v38 yet does not, and `pygg run`
/// never migrates it.
pub fn can_record(conn: &Connection) -> bool {
    conn.query_row("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'launches'", [], |_| Ok(()))
        .optional()
        .ok()
        .flatten()
        .is_some()
}

pub fn record_start(
    conn: &Connection, session: &str, tool: &str, model: Option<&str>, at: i64, git: Option<&GitState>,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO launches (session_key, tool, model, started_at, repo, branch, head_before, dirty_before) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            session,
            tool,
            model,
            at,
            git.map(|g| g.root.as_str()),
            git.and_then(|g| g.branch.as_deref()),
            git.and_then(|g| g.head.as_deref()),
            git.map(|g| g.dirty as i64),
        ],
    )
    .map(|_| ())
}

pub fn record_end(
    conn: &Connection, session: &str, at: i64, exit_code: i32, change: Option<&GitChange>,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE launches SET finished_at = ?2, exit_code = ?3, head_after = ?4, dirty_after = ?5, commits = ?6, \
         files_changed = ?7, insertions = ?8, deletions = ?9 WHERE session_key = ?1",
        params![
            session,
            at,
            exit_code,
            change.and_then(|c| c.head.as_deref()),
            change.map(|c| c.dirty as i64),
            change.and_then(|c| c.commits.map(|n| n as i64)),
            change.map(|c| c.files as i64),
            change.map(|c| c.insertions as i64),
            change.map(|c| c.deletions as i64),
        ],
    )
    .map(|_| ())
}

/// Unix milliseconds at the start of the UTC week (Monday 00:00) that `at` falls in.
pub fn week_start(at: i64) -> i64 {
    let days = at.div_euclid(86_400_000);
    // 1970-01-01 was a Thursday: three days after a Monday.
    (days - (days + 3).rem_euclid(7)) * 86_400_000
}

/// One repository's launches in one UTC week (cost plan B5): their known cost and what changed in Git. Observations
/// only: a commit is not a unit of work and a launch that changed nothing may have answered a question (ADR-036).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoWeek {
    /// The repository's folder; `None` for launches outside a Git repository.
    pub repo: Option<String>,
    pub week_start: i64,
    pub launches: u64,
    /// Launches still running, or cut off before `pygg run` could settle them: what they changed is not known.
    pub unsettled: u64,
    pub requests: u64,
    pub known_micro_usd: u64,
    /// Requests with no known cost (subscription requests not included), counted beside the money, never as $0.
    pub unknown_cost: u64,
    pub commits: u64,
    /// Settled launches in a repository that made no commit and left no change against their starting commit.
    pub changed_nothing: u64,
    pub changed_nothing_micro_usd: u64,
}

impl RepoWeek {
    /// Known cost divided by the commits made, when there were commits.
    pub fn per_commit(&self) -> Option<u64> {
        (self.commits > 0).then(|| self.known_micro_usd / self.commits)
    }
}

/// Launches started at or after `since`, grouped by repository and UTC week, newest week first within a repository.
pub fn by_repo(conn: &Connection, since: i64) -> rusqlite::Result<Vec<RepoWeek>> {
    type Row = (String, i64, Option<i64>, Option<String>, Option<i64>, Option<i64>);
    let rows: Vec<Row> = conn
        .prepare(
            "SELECT session_key, started_at, finished_at, repo, commits, files_changed FROM launches              WHERE started_at >= ?1 ORDER BY repo, started_at",
        )?
        .query_map([since], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut weeks: Vec<RepoWeek> = Vec::new();
    for (session, started, finished, repo, commits, files) in rows {
        let cost = session_cost(conn, &session)?;
        let week = week_start(started);
        let at = match weeks.iter().position(|w| w.repo == repo && w.week_start == week) {
            Some(at) => at,
            None => {
                weeks.push(RepoWeek { repo: repo.clone(), week_start: week, ..Default::default() });
                weeks.len() - 1
            }
        };
        let w = &mut weeks[at];
        w.launches += 1;
        w.requests += cost.answered + cost.failed;
        w.known_micro_usd += cost.known_micro_usd;
        w.unknown_cost += cost.unknown_cost;
        if finished.is_none() {
            w.unsettled += 1;
            continue;
        }
        w.commits += commits.unwrap_or(0).max(0) as u64;
        // `commits` is NULL when HEAD moved off the starting commit's line: something changed.
        if repo.is_some() && commits == Some(0) && files.unwrap_or(0) == 0 {
            w.changed_nothing += 1;
            w.changed_nothing_micro_usd += cost.known_micro_usd;
        }
    }
    weeks.sort_by(|a, b| a.repo.cmp(&b.repo).then(b.week_start.cmp(&a.week_start)));
    Ok(weeks)
}

/// How many requests the depth line averages at each end of a session, and the fewest answered requests it needs.
const DEPTH_WINDOW: usize = 10;

/// One session's requests through the gateway, from the ledger.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCost {
    /// Requests answered.
    pub answered: u64,
    /// Attempts that failed, including the ones another model then answered (`fell_back`).
    pub failed: u64,
    pub fell_back: u64,
    /// Known money: answered requests and failed attempts with a known cost.
    pub known_micro_usd: u64,
    /// Entries whose cost is unknown, subscription ones left out.
    pub unknown_cost: u64,
    /// Answered on a subscription: a plan, not a price per request.
    pub subscription: u64,
    pub prompt_tokens: u64,
    pub cached_tokens: u64,
    pub written_tokens: u64,
    pub output_tokens: u64,
    /// Models that answered, most requests first.
    pub models: Vec<(String, u64)>,
    /// Average known cost of the first and last requests with a known, non-zero cost, when there are enough of them:
    /// every turn sends the conversation again, so a long session's later requests cost more.
    pub depth: Option<(u64, u64)>,
}

pub fn session_cost(conn: &Connection, session: &str) -> rusqlite::Result<SessionCost> {
    let mut stmt = conn.prepare(
        "SELECT outcome, input_tokens, output_tokens, cache_tokens, cache_write_tokens, cost_micro_usd, cost_status, \
         model, billing FROM usage_events WHERE session_key = ?1 AND created_at >= 0 ORDER BY created_at",
    )?;
    let mut out = SessionCost::default();
    let mut priced: Vec<u64> = Vec::new();
    let rows = stmt.query_map([session], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<i64>>(1)?,
            r.get::<_, Option<i64>>(2)?,
            r.get::<_, Option<i64>>(3)?,
            r.get::<_, Option<i64>>(4)?,
            r.get::<_, Option<i64>>(5)?,
            r.get::<_, String>(6)?,
            r.get::<_, String>(7)?,
            r.get::<_, Option<String>>(8)?,
        ))
    })?;
    let n = |v: Option<i64>| v.unwrap_or(0).max(0) as u64;
    for row in rows {
        let (outcome, input, output, cached, written, cost, status, model, billing) = row?;
        let subscription = billing.as_deref() == Some("subscription");
        if status == "unknown" {
            if !subscription {
                out.unknown_cost += 1;
            }
        } else {
            out.known_micro_usd += n(cost);
        }
        if outcome != "ok" {
            out.failed += 1;
            out.fell_back += u64::from(outcome == "fallback");
            continue;
        }
        out.answered += 1;
        out.subscription += u64::from(subscription);
        out.prompt_tokens += n(input);
        out.output_tokens += n(output);
        out.cached_tokens += n(cached);
        out.written_tokens += n(written);
        match out.models.iter_mut().find(|(m, _)| *m == model) {
            Some((_, count)) => *count += 1,
            None => out.models.push((model, 1)),
        }
        if status != "unknown" && n(cost) > 0 {
            priced.push(n(cost));
        }
    }
    out.models.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    if priced.len() >= 2 * DEPTH_WINDOW {
        let avg = |s: &[u64]| s.iter().sum::<u64>() / s.len() as u64;
        out.depth = Some((avg(&priced[..DEPTH_WINDOW]), avg(&priced[priced.len() - DEPTH_WINDOW..])));
    }
    Ok(out)
}

/// The share of the prompt read from the provider's cache, in percent; `None` without prompt tokens.
fn cache_percent(cost: &SessionCost) -> Option<u64> {
    (cost.prompt_tokens > 0).then(|| (cost.cached_tokens * 100 + cost.prompt_tokens / 2) / cost.prompt_tokens)
}

fn money(micro: u64) -> String {
    crate::cli::read::money(micro)
}

/// What the launch cost and what changed, as `pygg run` prints it when the tool exits.
pub fn receipt(
    tool: &str, session: &str, cost: &SessionCost, compared: Option<(&str, u64, u64)>,
    classes: Option<&RequestClasses>, git: Option<(&GitState, &GitChange)>, note: Option<&str>,
    limits: Option<&SessionLimits>,
) -> String {
    let mut lines = vec![format!("Pygg · this launch of {tool} ({session})")];
    let row = |label: &str, text: String| format!("  {label:<10} {text}");
    if cost.answered + cost.failed == 0 {
        lines.push(row("Requests", "none reached Pygg's gateway".into()));
    } else {
        let mut text =
            format!("{} for {} answered request{}", money(cost.known_micro_usd), cost.answered, plural(cost.answered));
        let mut extra = Vec::new();
        if cost.unknown_cost > 0 {
            extra.push(format!("{} with no known cost", cost.unknown_cost));
        }
        if cost.subscription > 0 {
            extra.push(format!("{} on a subscription", cost.subscription));
        }
        if !extra.is_empty() {
            text.push_str(&format!(" ({}, not counted)", extra.join("; ")));
        }
        lines.push(row("Cost", text));
        if let Some(pct) = cache_percent(cost) {
            let written = (cost.written_tokens * 100 + cost.prompt_tokens / 2) / cost.prompt_tokens.max(1);
            lines.push(row("Cache", format!("{pct}% of the prompt read from cache, {written}% written to it")));
        }
        if !cost.models.is_empty() {
            let mut text = cost.models.iter().map(|(m, n)| format!("{m} ×{n}")).collect::<Vec<_>>().join(", ");
            if cost.failed > 0 {
                text.push_str(&format!(
                    " · {} failed attempt{}, {} answered by the next model",
                    cost.failed,
                    plural(cost.failed),
                    cost.fell_back
                ));
            }
            lines.push(row("Models", text));
        }
        if let Some((first, last)) = cost.depth {
            lines.push(row(
                "Depth",
                format!(
                    "a request cost {} at the start and {} at the end (each turn sends the conversation again)",
                    money(first),
                    money(last)
                ),
            ));
        }
        if let Some(c) = classes.filter(|c| c.requests > 0) {
            lines.push(row("Classes", class_line(c)));
            if c.most_agents_one_prompt >= FAN_OUT_WARNING {
                lines.push(row(
                    "Fan-out",
                    format!(
                        "one prompt started {} subagents; each starts its own conversation and writes its own cache",
                        c.most_agents_one_prompt
                    ),
                ));
            }
        }
        if let Some((baseline, routed, at_baseline)) = compared {
            lines.push(row(
                "Compared",
                format!(
                    "{} at {baseline} with the same tokens, against {} (an estimate)",
                    money(at_baseline),
                    money(routed)
                ),
            ));
        }
    }
    if let Some(text) = limits.and_then(limit_text) {
        lines.push(row("Limit", text));
    }
    match git {
        None => lines.push(row("Git", "not in a Git repository".into())),
        Some((before, change)) => {
            let place = match &before.branch {
                Some(b) => format!("{} ({b})", before.root),
                None => before.root.clone(),
            };
            let mut parts = Vec::new();
            match change.commits {
                Some(0) => {}
                Some(n) => parts.push(format!("{n} commit{}", plural(n))),
                None => parts.push("HEAD moved to a commit that does not follow the starting one".into()),
            }
            if change.files > 0 {
                parts.push(format!(
                    "{} file{} changed since the start, +{} −{}",
                    change.files,
                    plural(change.files),
                    change.insertions,
                    change.deletions
                ));
            }
            if change.dirty > 0 {
                parts.push(format!("{} not committed", change.dirty));
            }
            let mut text = if parts.is_empty() {
                format!("{place}: nothing changed")
            } else {
                format!("{place}: {}", parts.join(", "))
            };
            if before.dirty > 0 {
                text.push_str(&format!(
                    " · {} file{} already had changes at the start",
                    before.dirty,
                    plural(before.dirty)
                ));
            }
            lines.push(row("Git", text));
        }
    }
    if let Some(note) = note {
        lines.push(row("", note.to_string()));
    }
    lines.join("\n")
}

/// From this many subagents started by one prompt, the receipt points it out.
pub const FAN_OUT_WARNING: u64 = 10;

/// Each request class's money and requests, and for subagents how many there were and how many worked at once.
fn class_line(c: &RequestClasses) -> String {
    c.groups
        .iter()
        .map(|g| {
            let mut s = format!("{} {} ×{}", g.key, money(g.known_micro_usd), g.requests);
            if g.unknown_cost > 0 {
                s.push_str(&format!(" +{} unpriced", g.unknown_cost));
            }
            if g.key == "subagent" && g.agents > 0 {
                s.push_str(&format!(
                    " ({} agent{}, at most {} at once)",
                    g.agents,
                    plural(g.agents),
                    c.peak_parallel_agents
                ));
            }
            s
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// The session and day limits of the key a launch used (cost plan A3), for its receipt: what each has counted, and
/// whether Pygg now refuses the launch's requests.
fn limit_text(limits: &SessionLimits) -> Option<String> {
    if let Some(hit) = limits.reached() {
        let after = match hit.scope {
            LimitScope::Session => "Pygg refuses this session's further requests (402)",
            _ => "Pygg refuses this key's requests (402) until 00:00 UTC",
        };
        return Some(format!(
            "the {} limit of {} on key \"{}\" is reached ({} counted): {after}",
            hit.scope.label(),
            money(hit.limit),
            limits.client,
            money(hit.counted)
        ));
    }
    let mut parts = Vec::new();
    if let Some((counted, limit)) = limits.session {
        parts.push(format!("{} of the session limit {}", money(counted), money(limit)));
    }
    if let Some((counted, limit)) = limits.day {
        parts.push(format!("{} of the daily limit {} (UTC)", money(counted), money(limit)));
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// The one line `pygg statusline` prints for Claude Code; `agents` is the session's subagents so far, `limits` its
/// key's session and day limits.
pub fn status_line(cost: &SessionCost, agents: u64, limits: Option<&SessionLimits>) -> String {
    if cost.answered + cost.failed == 0 {
        return "Pygg · no request yet".into();
    }
    let mut parts =
        vec![format!("Pygg {}{}", money(cost.known_micro_usd), if cost.unknown_cost > 0 { "+?" } else { "" })];
    if let Some(pct) = cache_percent(cost) {
        parts.push(format!("{pct}% cache"));
    }
    if let Some((m, _)) = cost.models.first() {
        parts.push(if cost.models.len() > 1 { format!("{m} +{}", cost.models.len() - 1) } else { m.clone() });
    }
    if agents > 0 {
        parts.push(format!("{agents} subagent{}", plural(agents)));
    }
    if cost.fell_back > 0 {
        parts.push(format!("{} fallback{}", cost.fell_back, plural(cost.fell_back)));
    }
    if let Some(limits) = limits {
        match limits.reached() {
            Some(hit) => parts.insert(1, format!("{} limit {} reached", hit.scope.label(), money(hit.limit))),
            None => {
                if let Some((_, limit)) = limits.session {
                    parts[0].push_str(&format!(" of {}", money(limit)));
                }
            }
        }
    }
    parts.join(" · ")
}

/// The launch session a status line belongs to: Claude Code started by `pygg run` sends it in its custom headers;
/// OpenCode and Codex get it as `PYGG_ATTEMPT_ID`.
pub fn session_from_env(custom_headers: Option<&str>, attempt: Option<&str>) -> Option<String> {
    let from_headers = custom_headers.and_then(|h| {
        h.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.trim().eq_ignore_ascii_case(crate::gateway::SESSION_HEADER).then(|| value.trim().to_string())
        })
    });
    from_headers.or_else(|| attempt.map(|a| a.trim().to_string())).filter(|s| !s.is_empty())
}

fn plural(n: u64) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn row(conn: &Connection, id: &str, at: i64, outcome: &str, cost: Option<i64>, model: &str, billing: &str) {
        let status = if cost.is_some() { "estimated" } else { "unknown" };
        conn.execute(
            "INSERT INTO usage_events (id, client_id, provider, model, input_tokens, output_tokens, cache_tokens, \
             cache_write_tokens, cost_micro_usd, cost_status, source, created_at, outcome, session_key, billing) \
             VALUES (?1, 'c', 'p', ?2, 1000, 50, 800, 100, ?3, ?4, 'price: test', ?5, ?6, 'launch-a', ?7)",
            params![id, model, cost, status, at, outcome, billing],
        )
        .unwrap();
    }

    #[test]
    fn a_session_is_summed_from_its_own_rows() {
        let conn = crate::db::open_in_memory().unwrap();
        for i in 0..20 {
            row(&conn, &format!("r{i}"), i, "ok", Some(100 + i * 10), "haiku", "paid");
        }
        row(&conn, "f1", 30, "fallback", Some(5), "glm", "paid");
        row(&conn, "s1", 31, "ok", None, "glm", "subscription");
        row(&conn, "u1", 32, "ok", None, "glm", "paid");
        conn.execute("UPDATE usage_events SET session_key = 'other' WHERE id = 'u1'", []).unwrap();
        let cost = session_cost(&conn, "launch-a").unwrap();
        assert_eq!((cost.answered, cost.failed, cost.fell_back), (21, 1, 1));
        assert_eq!(cost.subscription, 1);
        assert_eq!(cost.unknown_cost, 0, "a subscription is counted apart, not as unknown");
        // 20 priced answers 100..290 plus the failed attempt's 5.
        assert_eq!(cost.known_micro_usd, (0..20).map(|i| 100 + i * 10).sum::<u64>() + 5);
        assert_eq!(cost.models, vec![("haiku".to_string(), 20), ("glm".to_string(), 1)]);
        assert_eq!(cost.depth, Some((145, 245)));
        assert_eq!(cache_percent(&cost), Some(80));
        let line = status_line(&cost, 2, None);
        assert!(line.starts_with("Pygg $0.0039 · 80% cache · haiku +1 · 2 subagents · 1 fallback"), "{line}");
    }

    #[test]
    fn a_session_or_day_limit_shows_in_the_status_line_and_the_receipt() {
        let cost = SessionCost { answered: 2, known_micro_usd: 2_000_000, ..Default::default() };
        let open = SessionLimits { client: "Claude Code".into(), session: Some((2_000_000, 5_000_000)), day: None };
        let line = status_line(&cost, 0, Some(&open));
        assert!(line.starts_with("Pygg $2.00 of $5.00"), "{line}");
        let text = receipt("Claude Code", "s", &cost, None, None, None, None, Some(&open));
        assert!(text.contains("Limit") && text.contains("$2.00 of the session limit $5.00"), "{text}");

        let spent = SessionCost { known_micro_usd: 5_100_000, ..cost };
        let reached = SessionLimits { session: Some((5_100_000, 5_000_000)), ..open.clone() };
        let line = status_line(&spent, 0, Some(&reached));
        assert!(line.starts_with("Pygg $5.10 · session limit $5.00 reached"), "{line}");
        let text = receipt("Claude Code", "s", &spent, None, None, None, None, Some(&reached));
        assert!(text.contains("refuses this session's further requests (402)"), "{text}");

        let day = SessionLimits { session: None, day: Some((20_000_000, 20_000_000)), ..open };
        assert!(status_line(&spent, 0, Some(&day)).contains("daily limit $20.00 reached"));
        let text = receipt("Claude Code", "s", &spent, None, None, None, None, Some(&day));
        assert!(text.contains("until 00:00 UTC"), "{text}");
    }

    #[test]
    fn the_receipt_says_what_is_not_known() {
        let cost = SessionCost {
            answered: 3,
            unknown_cost: 1,
            subscription: 1,
            known_micro_usd: 420_000,
            prompt_tokens: 1000,
            cached_tokens: 900,
            models: vec![("m".into(), 3)],
            ..Default::default()
        };
        let before = GitState { root: "C:/repo".into(), branch: Some("main".into()), head: Some("a".into()), dirty: 1 };
        let change =
            GitChange { head: Some("b".into()), commits: Some(2), files: 5, insertions: 120, deletions: 14, dirty: 0 };
        let classes = RequestClasses {
            requests: 40,
            groups: vec![
                crate::gateway::hints::ClassGroup {
                    key: "main".into(),
                    requests: 20,
                    known_micro_usd: 300_000,
                    ..Default::default()
                },
                crate::gateway::hints::ClassGroup {
                    key: "subagent".into(),
                    requests: 20,
                    known_micro_usd: 120_000,
                    unknown_cost: 1,
                    agents: 12,
                    ..Default::default()
                },
            ],
            agents: 12,
            most_agents_one_prompt: 12,
            peak_parallel_agents: 5,
            ..Default::default()
        };
        let text = receipt(
            "Claude Code",
            "launch-x",
            &cost,
            Some(("anthropic/opus", 420_000, 2_100_000)),
            Some(&classes),
            Some((&before, &change)),
            None,
            None,
        );
        assert!(
            text.contains("main $0.30 ×20 · subagent $0.12 ×20 +1 unpriced (12 agents, at most 5 at once)"),
            "{text}"
        );
        assert!(text.contains("one prompt started 12 subagents"), "{text}");
        assert!(
            text.contains("$0.42 for 3 answered requests (1 with no known cost; 1 on a subscription, not counted)"),
            "{text}"
        );
        assert!(text.contains("90% of the prompt read from cache"), "{text}");
        assert!(text.contains("$2.10 at anthropic/opus with the same tokens, against $0.42"), "{text}");
        assert!(text.contains("C:/repo (main): 2 commits, 5 files changed since the start, +120 −14"), "{text}");
        assert!(text.contains("1 file already had changes at the start"), "{text}");
        let none = receipt("Codex", "launch-y", &SessionCost::default(), None, None, None, Some("not saved"), None);
        assert!(none.contains("none reached Pygg's gateway") && none.contains("not in a Git repository"), "{none}");
    }

    #[test]
    fn shortstat_reads_each_part() {
        assert_eq!(shortstat(" 3 files changed, 10 insertions(+), 2 deletions(-)"), (3, 10, 2));
        assert_eq!(shortstat(" 1 file changed, 1 insertion(+)"), (1, 1, 0));
        assert_eq!(shortstat(" 1 file changed, 4 deletions(-)"), (1, 0, 4));
        assert_eq!(shortstat(""), (0, 0, 0));
    }

    #[test]
    fn the_session_comes_from_the_headers_or_the_attempt_variable() {
        assert_eq!(session_from_env(Some("x-pygg-session: launch-1"), None).as_deref(), Some("launch-1"));
        assert_eq!(
            session_from_env(Some("X-Other: a\nX-Pygg-Session:launch-2"), Some("t")).as_deref(),
            Some("launch-2")
        );
        assert_eq!(session_from_env(None, Some("launch-3")).as_deref(), Some("launch-3"));
        assert_eq!(session_from_env(Some("X-Other: a"), Some(" ")), None);
    }

    #[test]
    fn weeks_start_on_monday_utc() {
        let day = 86_400_000;
        // Day 4 (1970-01-05) was a Monday; day 3 a Sunday, in the week from 1969-12-29.
        assert_eq!(week_start(3 * day), -3 * day);
        assert_eq!(week_start(4 * day - 1), -3 * day);
        assert_eq!(week_start(4 * day), 4 * day);
        assert_eq!(week_start(10 * day + day - 1), 4 * day);
        assert_eq!(week_start(11 * day + 1), 11 * day);
    }

    #[test]
    fn launches_are_summed_by_repository_and_week() {
        let conn = crate::db::open_in_memory().unwrap();
        let day = 86_400_000;
        let monday = 11 * day;
        let git =
            |root: &str| GitState { root: root.into(), branch: Some("main".into()), head: Some("a".into()), dirty: 0 };
        let change = |commits: Option<u64>, files: u64| GitChange {
            head: Some("b".into()),
            commits,
            files,
            insertions: files,
            deletions: 0,
            dirty: 0,
        };
        let launch = |session: &str, at: i64, repo: Option<&str>, change: Option<GitChange>, cost: i64| {
            record_start(&conn, session, "claude", None, at, repo.map(git).as_ref()).unwrap();
            if let Some(change) = change {
                record_end(&conn, session, at + 1000, 0, Some(&change)).unwrap();
            }
            conn.execute(
                "INSERT INTO usage_events (id, client_id, provider, model, cost_micro_usd, cost_status, source,                  created_at, outcome, session_key, billing)                  VALUES (?1, 'c', 'p', 'm', ?2, 'estimated', 'price: test', ?3, 'ok', ?1, 'paid')",
                params![session, cost, at + 10],
            )
            .unwrap();
        };
        launch("a1", monday + day, Some("F:/app"), Some(change(Some(2), 3)), 400_000);
        launch("a2", monday + 2 * day, Some("F:/app"), Some(change(Some(0), 0)), 100_000);
        launch("a3", monday + 3 * day, Some("F:/app"), Some(change(Some(0), 2)), 50_000);
        launch("a4", monday + 4 * day, Some("F:/app"), None, 10_000);
        launch("b1", monday - day, Some("F:/app"), Some(change(Some(1), 1)), 70_000);
        launch("n1", monday + day, None, Some(change(Some(0), 0)), 5_000);
        launch("old", monday - 30 * day, Some("F:/app"), Some(change(Some(0), 0)), 1);

        let weeks = by_repo(&conn, monday - 7 * day).unwrap();
        assert_eq!(weeks.len(), 3, "{weeks:?}");
        let this = &weeks[1];
        assert_eq!((this.repo.as_deref(), this.week_start), (Some("F:/app"), monday));
        assert_eq!((this.launches, this.unsettled, this.requests), (4, 1, 4));
        assert_eq!((this.known_micro_usd, this.commits), (560_000, 2));
        assert_eq!(this.per_commit(), Some(280_000));
        // a2 only: a3 left uncommitted changes, a4 is not settled.
        assert_eq!((this.changed_nothing, this.changed_nothing_micro_usd), (1, 100_000));
        let before = &weeks[2];
        assert_eq!((before.week_start, before.launches, before.commits), (monday - 7 * day, 1, 1));
        // Outside Git, a launch is counted but never "changed nothing": there is nothing to compare.
        let outside = &weeks[0];
        assert_eq!((outside.repo.as_deref(), outside.changed_nothing, outside.per_commit()), (None, 0, None));
    }

    #[test]
    fn a_launch_is_recorded_and_settled() {
        let conn = crate::db::open_in_memory().unwrap();
        assert!(can_record(&conn));
        let git = GitState { root: "r".into(), branch: None, head: Some("a".into()), dirty: 2 };
        record_start(&conn, "launch-z", "codex", None, 10, Some(&git)).unwrap();
        let change =
            GitChange { head: Some("b".into()), commits: None, files: 1, insertions: 2, deletions: 3, dirty: 0 };
        record_end(&conn, "launch-z", 20, 0, Some(&change)).unwrap();
        let got: (Option<i64>, Option<String>, Option<i64>, Option<i64>) = conn
            .query_row(
                "SELECT dirty_before, head_after, commits, deletions FROM launches WHERE session_key = 'launch-z'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(got, (Some(2), Some("b".into()), None, Some(3)));
    }

    /// Git itself, in a scratch repository: commits and a working-tree change since the start are both seen.
    #[test]
    fn git_change_counts_commits_and_the_working_tree() {
        let dir = std::env::temp_dir().join(format!("pygg-launch-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| {
            let ok = Command::new("git").arg("-C").arg(&dir).args(args).output().map(|o| o.status.success());
            ok.unwrap_or(false)
        };
        if !run(&["init", "-q"]) {
            return; // no Git on this machine
        }
        for args in [["config", "user.email", "t@example.invalid"], ["config", "user.name", "t"]] {
            run(&args);
        }
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        assert!(run(&["add", "."]) && run(&["commit", "-q", "-m", "first"]));
        let before = git_state(&dir).unwrap();
        assert!(before.head.is_some() && before.dirty == 0);
        std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
        assert!(run(&["commit", "-q", "-am", "second"]));
        std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
        let change = git_change(&dir, &before).unwrap();
        assert_eq!(change.commits, Some(1));
        assert_eq!((change.files, change.insertions, change.deletions, change.dirty), (1, 2, 0, 1));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
