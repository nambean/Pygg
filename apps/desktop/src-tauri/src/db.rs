//! Local SQLite store. Opening runs every pending migration inside one transaction, so a database is
//! either at the old version or the new one, never in between.

use rusqlite::Connection;
use std::path::Path;

/// Migrations in order; the database's `user_version` is the number applied.
const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_init.sql")];

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

pub fn version(conn: &Connection) -> rusqlite::Result<u32> {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
}

fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    let current = version(conn)?;
    if current > SCHEMA_VERSION {
        // Opened by a newer build: refuse rather than write with an older understanding of the schema.
        return Err(rusqlite::Error::InvalidQuery);
    }
    if current == SCHEMA_VERSION {
        return Ok(());
    }
    let tx = conn.transaction()?;
    for sql in &MIGRATIONS[current as usize..] {
        tx.execute_batch(sql)?;
    }
    tx.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))?;
    tx.commit()
}

/// Unix milliseconds now.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_to_current_version() {
        let conn = open_in_memory().unwrap();
        assert_eq!(version(&conn).unwrap(), SCHEMA_VERSION);
        let tables: i64 = conn
            .query_row("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name IN \
                ('workspaces','tasks','attempts','usage_events','approvals')", [], |r| r.get(0))
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
