//! Local SQLite store. Opening runs every pending migration inside one transaction, so a database is
//! either at the old version or the new one, never in between.

use rusqlite::Connection;
use std::path::Path;

/// Migrations in order; the database's `user_version` is the number applied.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_init.sql"),
    include_str!("../migrations/0002_gateway.sql"),
    include_str!("../migrations/0003_cost_safe_gateway.sql"),
    include_str!("../migrations/0004_counter_review.sql"),
    include_str!("../migrations/0005_client_scope.sql"),
    include_str!("../migrations/0006_owner_generation.sql"),
    include_str!("../migrations/0007_auto_routing.sql"),
    include_str!("../migrations/0008_task_engine.sql"),
    include_str!("../migrations/0009_tasks_from_clients.sql"),
    include_str!("../migrations/0010_approval_estimates.sql"),
    include_str!("../migrations/0011_task_budgets.sql"),
    include_str!("../migrations/0012_request_features.sql"),
    include_str!("../migrations/0013_data_use_terms.sql"),
    include_str!("../migrations/0014_strict_budget_and_probes.sql"),
    include_str!("../migrations/0015_quality_signals.sql"),
    include_str!("../migrations/0016_tool_signatures.sql"),
    include_str!("../migrations/0017_task_handoff.sql"),
    include_str!("../migrations/0018_rate_limits.sql"),
    include_str!("../migrations/0019_step_cost_source.sql"),
    include_str!("../migrations/0020_model_enabled_source.sql"),
    include_str!("../migrations/0021_repeated_tokens.sql"),
    include_str!("../migrations/0022_connection_pools.sql"),
    include_str!("../migrations/0023_client_token_copies.sql"),
    include_str!("../migrations/0024_web_sessions.sql"),
    include_str!("../migrations/0025_combo_strategy.sql"),
    include_str!("../migrations/0026_web_daily_limit.sql"),
    include_str!("../migrations/0027_web_free_billing.sql"),
    include_str!("../migrations/0028_web_guard.sql"),
    include_str!("../migrations/0029_connection_checks.sql"),
    include_str!("../migrations/0030_request_facts.sql"),
    include_str!("../migrations/0031_key_rings.sql"),
    include_str!("../migrations/0032_skills.sql"),
    include_str!("../migrations/0033_session_home.sql"),
    include_str!("../migrations/0034_cache_miss.sql"),
    include_str!("../migrations/0035_skill_projects.sql"),
    include_str!("../migrations/0036_prefix_meter.sql"),
    include_str!("../migrations/0037_clearing_shadow.sql"),
    include_str!("../migrations/0038_launches.sql"),
    include_str!("../migrations/0039_request_class.sql"),
    include_str!("../migrations/0040_session_day_limits.sql"),
    include_str!("../migrations/0041_web_account.sql"),
    include_str!("../migrations/0042_brainstorms.sql"),
    include_str!("../migrations/0043_feedback_brainstorm.sql"),
    include_str!("../migrations/0044_board.sql"),
    include_str!("../migrations/0045_brainstorm_branch.sql"),
    include_str!("../migrations/0046_card_check.sql"),
];

pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let mut conn = Connection::open(path)?;
    configure(&conn)?;
    // WAL lets the interface read while a task writes. In-memory databases report "memory" instead.
    conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
    migrate(&mut conn)?;
    Ok(conn)
}

pub fn open_in_memory() -> rusqlite::Result<Connection> {
    let mut conn = Connection::open_in_memory()?;
    configure(&conn)?;
    migrate(&mut conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
}

/// Why a store could not be opened or used, for what the app tells the user (fault tests in `faults.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    /// Not a database, or its pages are damaged: a daily copy can replace it.
    Damaged,
    /// Another process holds the write lock longer than the busy timeout.
    Busy,
    /// The disk (or the store's page limit) is full.
    Full,
    /// Written by a newer Pygg: an older one must not write it.
    Newer,
    Other,
}

pub fn problem(e: &rusqlite::Error) -> Problem {
    use rusqlite::ErrorCode::*;
    match e {
        rusqlite::Error::InvalidQuery => Problem::Newer,
        rusqlite::Error::SqliteFailure(f, _) => match f.code {
            NotADatabase | DatabaseCorrupt => Problem::Damaged,
            DatabaseBusy | DatabaseLocked => Problem::Busy,
            DiskFull => Problem::Full,
            _ => Problem::Other,
        },
        _ => Problem::Other,
    }
}

/// A quick structural check (`PRAGMA quick_check`): a store whose header opens can still have damaged pages, which
/// would fail later, mid-request. The app runs it once at start-up; it reads every page, without checking indexes'
/// contents, so it stays fast for a store of this size.
pub fn check(conn: &Connection) -> rusqlite::Result<()> {
    let result: String = conn.query_row("PRAGMA quick_check(1)", [], |r| r.get(0))?;
    if result == "ok" {
        return Ok(());
    }
    Err(rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CORRUPT), Some(result)))
}

pub fn version(conn: &Connection) -> rusqlite::Result<u32> {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
}

fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    if version(conn)? == SCHEMA_VERSION {
        return Ok(());
    }
    // IMMEDIATE takes the write lock first, and the version is read again inside it: the app and the CLI
    // opening a fresh store at the same moment cannot both run the same migration.
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let current: u32 = tx.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if current > SCHEMA_VERSION {
        // Opened by a newer build: refuse rather than write with an older understanding of the schema.
        return Err(rusqlite::Error::InvalidQuery);
    }
    if current == SCHEMA_VERSION {
        return Ok(());
    }
    for sql in &MIGRATIONS[current as usize..] {
        tx.execute_batch(sql)?;
    }
    tx.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))?;
    tx.commit()
}

/// Unix milliseconds now.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_to_current_version() {
        let conn = open_in_memory().unwrap();
        assert_eq!(version(&conn).unwrap(), SCHEMA_VERSION);
        let tables: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name IN \
                ('workspaces','tasks','attempts','usage_events','approvals')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tables, 5);
    }

    #[test]
    fn reopening_a_file_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.sqlite3");
        drop(open(&path).unwrap());
        let conn = open(&path).unwrap();
        assert_eq!(version(&conn).unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let conn = open_in_memory().unwrap();
        let orphan = conn.execute(
            "INSERT INTO tasks (id, workspace_id, title, prompt, strategy, state, created_at, updated_at) \
             VALUES ('t1', 'missing', 'x', 'x', 'free_first', 'draft', 0, 0)",
            [],
        );
        assert!(orphan.is_err());
    }

    #[test]
    fn known_cost_requires_an_amount() {
        let conn = open_in_memory().unwrap();
        let bad = conn.execute(
            "INSERT INTO usage_events (id, provider, model, cost_status, source, created_at) \
             VALUES ('u1', 'p', 'm', 'actual', 'provider_api', 0)",
            [],
        );
        assert!(bad.is_err(), "an actual cost without an amount must be rejected");
        conn.execute(
            "INSERT INTO usage_events (id, provider, model, cost_status, source, created_at) \
             VALUES ('u2', 'p', 'm', 'unknown', 'agent', 0)",
            [],
        )
        .unwrap();
    }

    #[test]
    fn refuses_a_newer_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.sqlite3");
        {
            let conn = open(&path).unwrap();
            conn.execute_batch(&format!("PRAGMA user_version = {}", SCHEMA_VERSION + 1)).unwrap();
        }
        assert!(open(&path).is_err());
    }
}
