//! Data retention (SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md §2.7): how long the usage ledger and route decisions are kept.
//! Settings → Data: 30, 90 or 365 days, or forever (the default, so a store from before keeps everything). Older rows are
//! deleted at start-up and once a day while the app runs, except
//! - a ledger row a task attempt still refers to (`usage_events.attempt_id`, or its `session_key` while the gateway rows
//!   of a running attempt wait to be linked, and an agent's own run once linking moved it there: `tasks::link_gateway_usage`),
//!   so a task's cost stays the sum of its rows;
//! - a row of the current calendar month (UTC), which a client's monthly limit is counted from (`store::month_start`).

use crate::gateway::store;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

const KEY: &str = "retention.usage_days";
const LAST_RUN_KEY: &str = "retention.last_run";
/// The periods the setting offers, in days.
pub const CHOICES: [u32; 3] = [30, 90, 365];
/// How often the running app deletes what has aged out.
pub const EVERY: std::time::Duration = std::time::Duration::from_secs(86_400);

/// Ledger rows that may go: old enough, and no task attempt refers to them.
const USAGE_GONE: &str = "created_at < ?1 AND attempt_id IS NULL \
    AND (session_key IS NULL OR NOT EXISTS (SELECT 1 FROM attempts a WHERE a.id = usage_events.session_key))";
/// Quality signals about a single request that is no longer in the ledger.
const FEEDBACK_GONE: &str = "target_kind = 'request' AND created_at < ?1 \
    AND NOT EXISTS (SELECT 1 FROM usage_events u WHERE u.id = feedback.target_id)";
const DECISIONS_GONE: &str = "created_at < ?1";

/// What one clean-up deleted.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pruned {
    pub at: i64,
    /// Rows older than this were considered.
    pub before: i64,
    pub usage_rows: u64,
    pub route_decisions: u64,
}

/// The setting as the interface shows it.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Retention {
    /// `None`: kept forever.
    pub days: Option<u32>,
    pub last_run: Option<Pruned>,
}

/// How long usage history is kept; `None` is forever. A value this build does not offer counts as forever, so nothing is
/// deleted by mistake.
pub fn days(conn: &Connection) -> store::Result<Option<u32>> {
    Ok(store::setting(conn, KEY)?.and_then(|v| v.parse().ok()).filter(|d| CHOICES.contains(d)))
}

pub fn set_days(conn: &Connection, days: Option<u32>) -> store::Result<()> {
    match days {
        Some(d) if !CHOICES.contains(&d) => Err(format!("Usage history can be kept for 30, 90 or 365 days, or forever, not {d} days.")),
        Some(d) => store::set_setting(conn, KEY, &d.to_string()),
        None => store::set_setting(conn, KEY, "forever"),
    }
}

pub fn view(conn: &Connection) -> store::Result<Retention> {
    let last_run = store::setting(conn, LAST_RUN_KEY)?.and_then(|v| serde_json::from_str(&v).ok());
    Ok(Retention { days: days(conn)?, last_run })
}

/// Rows created before this time may go: `days` ago, or the start of this month when that is earlier.
pub fn cutoff(days: u32, now: i64) -> i64 {
    (now - days as i64 * 86_400_000).min(store::month_start(now))
}

/// How many ledger rows a period of `days` would delete now; nothing is deleted.
pub fn would_delete(conn: &Connection, days: u32, now: i64) -> store::Result<u64> {
    conn.query_row(&format!("SELECT COUNT(*) FROM usage_events WHERE {USAGE_GONE}"), [cutoff(days, now)], |r| r.get::<_, i64>(0))
        .map(|n| n.max(0) as u64).map_err(|e| e.to_string())
}

/// Deletes what is older than the setting allows, in one transaction, and records what it did. `None` when history is
/// kept forever.
pub fn prune(conn: &Connection, now: i64) -> store::Result<Option<Pruned>> {
    let Some(days) = days(conn)? else { return Ok(None) };
    let before = cutoff(days, now);
    let err = |e: rusqlite::Error| e.to_string();
    let tx = conn.unchecked_transaction().map_err(err)?;
    let usage_rows = tx.execute(&format!("DELETE FROM usage_events WHERE {USAGE_GONE}"), params![before]).map_err(err)? as u64;
    tx.execute(&format!("DELETE FROM feedback WHERE {FEEDBACK_GONE}"), params![before]).map_err(err)?;
    let route_decisions = tx.execute(&format!("DELETE FROM route_decisions WHERE {DECISIONS_GONE}"), params![before]).map_err(err)? as u64;
    let pruned = Pruned { at: now, before, usage_rows, route_decisions };
    tx.execute("INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = ?2",
        params![LAST_RUN_KEY, serde_json::to_string(&pruned).unwrap_or_default()]).map_err(err)?;
    tx.commit().map_err(err)?;
    Ok(Some(pruned))
}

/// Whether a row with this id is still in the ledger.
pub fn has_usage_row(conn: &Connection, id: &str) -> store::Result<bool> {
    conn.query_row("SELECT 1 FROM usage_events WHERE id = ?1", [id], |_| Ok(())).optional().map(|r| r.is_some())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    const DAY: i64 = 86_400_000;
    /// 2026-11-20T12:00:00Z: the month started 19.5 days ago.
    const NOW: i64 = 1_795_176_000_000;

    fn row(conn: &Connection, id: &str, created_at: i64, attempt: Option<&str>, session: Option<&str>) {
        conn.execute("INSERT INTO usage_events (id, attempt_id, client_id, provider, model, cost_micro_usd, cost_status, source, \
             created_at, session_key) VALUES (?1, ?2, 'c1', 'p', 'm', 100, 'estimated', 'price: user', ?3, ?4)",
            params![id, attempt, created_at, session]).unwrap();
    }

    #[test]
    fn forever_is_the_default_and_deletes_nothing() {
        let conn = db::open_in_memory().unwrap();
        row(&conn, "ancient", 1, None, None);
        assert_eq!(days(&conn).unwrap(), None);
        assert_eq!(prune(&conn, NOW).unwrap(), None);
        assert!(has_usage_row(&conn, "ancient").unwrap());
        assert!(set_days(&conn, Some(7)).is_err(), "only the offered periods");
        store::set_setting(&conn, KEY, "12").unwrap();
        assert_eq!(days(&conn).unwrap(), None, "an unknown value keeps everything");
    }

    #[test]
    fn old_rows_go_but_not_a_task_attempts_or_this_months() {
        // A client's monthly spend is counted from the real month, so the rows are placed around today.
        let now = db::now_ms();
        let month = store::month_start(now);
        let conn = db::open_in_memory().unwrap();
        conn.execute("INSERT INTO workspaces (id, name, path, created_at) VALUES ('w1', 'repo', 'C:/src/repo', 1)", []).unwrap();
        conn.execute("INSERT INTO tasks (id, workspace_id, title, prompt, strategy, state, created_at, updated_at) \
             VALUES ('t1', 'w1', 'x', 'x', 'free_first', 'done', 1, 1)", []).unwrap();
        conn.execute("INSERT INTO task_stages (id, task_id, position, name, state) VALUES ('s1', 't1', 0, 'main', 'done')", []).unwrap();
        conn.execute("INSERT INTO attempts (id, stage_id, number, agent, model_ref, tier, billing, state) \
             VALUES ('a1', 's1', 1, 'opencode', 'x', 1, 'paid', 'passed')", []).unwrap();
        conn.execute("INSERT INTO route_decisions (id, stage_id, eligible_json, excluded_json, created_at) VALUES \
             ('old', 's1', '[]', '[]', ?1), ('new', 's1', '[]', '[]', ?2)", params![now - 200 * DAY, now - DAY]).unwrap();
        row(&conn, "old", now - 200 * DAY, None, None);
        row(&conn, "old-of-an-attempt", now - 200 * DAY, Some("a1"), None);
        // A running attempt's gateway rows, and an agent's own run, point at it through the session until linked.
        row(&conn, "old-session-of-an-attempt", now - 200 * DAY, None, Some("a1"));
        row(&conn, "old-other-session", now - 200 * DAY, None, Some("someone's session"));
        // Within 90 days, but more than 30 days old.
        row(&conn, "last-month", month - 40 * DAY, None, None);
        // The first moment of this month, which a client's monthly limit counts, however short the period.
        row(&conn, "this-month", month, None, None);
        row(&conn, "now", now, None, None);
        conn.execute("INSERT INTO feedback (id, metric_name, target_kind, target_id, value_bool, source, created_at) VALUES \
             ('f1', 'checks_passed', 'request', 'old', 1, 'client', ?1)", [now - 200 * DAY]).unwrap();
        let (client, _) = store::create_client(&conn, "Codex", Some(5_000_000)).unwrap();
        conn.execute("UPDATE usage_events SET client_id = ?1", [&client.id]).unwrap();
        let month_spend = |conn: &Connection| store::list_clients(conn).unwrap()[0].month_spend_micro_usd;
        let spend_before = month_spend(&conn);
        assert_eq!(spend_before, 200, "this-month and now");

        set_days(&conn, Some(90)).unwrap();
        assert_eq!(would_delete(&conn, 90, now).unwrap(), 2);
        let pruned = prune(&conn, now).unwrap().unwrap();
        assert_eq!((pruned.usage_rows, pruned.route_decisions, pruned.before), (2, 1, now - 90 * DAY));
        for (id, kept) in [("old", false), ("old-other-session", false), ("old-of-an-attempt", true),
            ("old-session-of-an-attempt", true), ("last-month", true), ("this-month", true), ("now", true)] {
            assert_eq!(has_usage_row(&conn, id).unwrap(), kept, "{id}");
        }
        let feedback: i64 = conn.query_row("SELECT COUNT(*) FROM feedback", [], |r| r.get(0)).unwrap();
        assert_eq!(feedback, 0, "a signal about a deleted request goes with it");
        assert_eq!(view(&conn).unwrap().last_run, Some(pruned));

        set_days(&conn, Some(30)).unwrap();
        prune(&conn, now).unwrap();
        assert!(!has_usage_row(&conn, "last-month").unwrap());
        assert!(has_usage_row(&conn, "this-month").unwrap() && has_usage_row(&conn, "old-of-an-attempt").unwrap());
        assert_eq!(month_spend(&conn), spend_before, "the monthly limit still counts the same spend");
        let task_rows: i64 = conn.query_row("SELECT COUNT(*) FROM usage_events WHERE attempt_id = 'a1' OR session_key = 'a1'",
            [], |r| r.get(0)).unwrap();
        assert_eq!(task_rows, 2, "the task's cost is unchanged");
    }

    #[test]
    fn a_short_period_never_reaches_into_the_month() {
        assert_eq!(store::month_start(NOW), NOW - 19 * DAY - DAY / 2);
        assert_eq!(cutoff(30, NOW), NOW - 30 * DAY);
        // 2026-10-31T23:00Z: 30 days back is 1 October 23:00, inside October, so the month's start is the limit.
        let end_of_october = store::month_start(NOW) - 3_600_000;
        assert_eq!(cutoff(30, end_of_october), store::month_start(end_of_october));
        for now in [NOW, end_of_october, store::month_start(NOW)] {
            for days in CHOICES {
                assert!(cutoff(days, now) <= store::month_start(now));
            }
        }
    }
}
