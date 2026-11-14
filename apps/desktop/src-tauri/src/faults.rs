//! Fault tests for the store and the ledger's settlements (production readiness, 2026-11-14): a writer killed mid-
//! transaction, a store locked by another process, a full disk, and a store damaged before start-up. Each checks that
//! what was committed stays whole, what was not leaves no trace, and the app can say what happened.

use crate::gateway::pricing::Cost;
use crate::gateway::store::{self, AttemptStart, UsageRecord, Unsettled};
use crate::{backup, db};
use rusqlite::Connection;
use std::path::Path;
use std::time::Duration;

fn start<'a>(client: &'a str, generation: &'a str, hold: u64) -> AttemptStart<'a> {
    AttemptStart { client_id: client, provider_id: "main", model_id: "fast", requested_model: "coding", session_key: None,
        route_mode: "native", hold_micro: hold, owner_generation: generation }
}

fn answered(client: &str) -> UsageRecord<'_> {
    static COST: std::sync::OnceLock<Cost> = std::sync::OnceLock::new();
    let cost = COST.get_or_init(|| Cost::Unknown { source: "fault test".into() });
    UsageRecord::failed(client, "main", "fast", "coding", cost, "ok", Some(200), 10, "")
}

fn count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap()
}

/// The child half of `a_killed_writer_leaves_the_store_whole`: run only when that test starts this binary with
/// `PYGG_FAULT_STORE` set. It journals one attempt (committed), then writes many ledger entries in a transaction and
/// aborts the process before committing, as a crash or Task Manager's End Task would.
#[test]
fn killed_writer_child() {
    let Some(path) = std::env::var_os("PYGG_FAULT_STORE") else { return };
    let conn = db::open(Path::new(&path)).unwrap();
    let client: String = conn.query_row("SELECT id FROM gateway_clients LIMIT 1", [], |r| r.get(0)).unwrap();
    store::open_attempt(&conn, &start(&client, "child", 2_500)).unwrap().unwrap();
    conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    for _ in 0..500 {
        store::record_usage(&conn, &answered(&client)).unwrap();
    }
    std::process::abort();
}

#[test]
fn a_killed_writer_leaves_the_store_whole() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite3");
    {
        let conn = db::open(&path).unwrap();
        store::create_client(&conn, "Tests", Some(1_000_000)).unwrap();
    }
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["faults::killed_writer_child", "--exact", "--nocapture", "--test-threads=1"])
        .env("PYGG_FAULT_STORE", &path)
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
        .status().unwrap();
    assert!(!status.success(), "the child must die mid-transaction");

    let conn = db::open(&path).unwrap();
    db::check(&conn).unwrap();
    assert_eq!(count(&conn, "usage_events"), 0, "the uncommitted transaction left nothing");
    assert_eq!(count(&conn, "budget_reservations"), 1, "the committed journal entry survived");
    // The next gateway turns it into a ledger entry whose held amount stays counted as at risk.
    assert_eq!(store::settle_interrupted(&conn, Unsettled::NotOf("parent")).unwrap(), 1);
    let event = store::usage_summary(&conn, 0, 10).unwrap().recent.remove(0);
    assert_eq!((event.outcome.as_str(), event.at_risk_micro_usd), ("interrupted", Some(2_500)));
}

#[test]
fn a_locked_store_waits_for_the_lock_then_writes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite3");
    let conn = db::open(&path).unwrap();
    let (client, _) = store::create_client(&conn, "Tests", None).unwrap();
    let id = store::open_attempt(&conn, &start(&client.id, "g1", 0)).unwrap().unwrap();

    // Another process (here a second connection on another thread) holds the write lock for a moment.
    let holder = db::open(&path).unwrap();
    holder.execute_batch("BEGIN IMMEDIATE").unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        holder.execute_batch("COMMIT").unwrap();
    });
    // The settlement waits (busy timeout) instead of failing, and is written once.
    store::settle(&conn, &id, &answered(&client.id)).unwrap();
    release.join().unwrap();
    assert_eq!((count(&conn, "usage_events"), count(&conn, "budget_reservations")), (1, 0));
}

#[test]
fn a_store_locked_too_long_refuses_the_settlement_whole() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite3");
    let conn = db::open(&path).unwrap();
    let (client, _) = store::create_client(&conn, "Tests", None).unwrap();
    let id = store::open_attempt(&conn, &start(&client.id, "g1", 0)).unwrap().unwrap();
    conn.busy_timeout(Duration::from_millis(50)).unwrap();

    let holder = db::open(&path).unwrap();
    holder.execute_batch("BEGIN IMMEDIATE").unwrap();
    let refused = store::settle(&conn, &id, &answered(&client.id)).unwrap_err();
    assert!(refused.contains("locked") || refused.contains("busy"), "{refused}");
    let lock = holder.execute_batch("BEGIN IMMEDIATE");
    assert!(lock.is_err(), "the holder still has its transaction");
    holder.execute_batch("COMMIT").unwrap();
    // Neither half was written; the gateway keeps it pending (review FR-03) and settles it once the lock is free.
    assert_eq!((count(&conn, "usage_events"), count(&conn, "budget_reservations")), (0, 1));
    store::settle(&conn, &id, &answered(&client.id)).unwrap();
    assert_eq!((count(&conn, "usage_events"), count(&conn, "budget_reservations")), (1, 0));

    // At start-up a lock held past the timeout is named as such.
    let other = Connection::open(&path).unwrap();
    other.busy_timeout(Duration::ZERO).unwrap();
    holder.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let e = other.execute_batch("BEGIN IMMEDIATE").unwrap_err();
    assert_eq!(db::problem(&e), db::Problem::Busy);
    holder.execute_batch("COMMIT").unwrap();
}

#[test]
fn a_full_disk_refuses_writes_whole_and_recovers() {
    let conn = db::open_in_memory().unwrap();
    let (client, _) = store::create_client(&conn, "Tests", Some(1_000_000)).unwrap();
    let id = store::open_attempt(&conn, &start(&client.id, "g1", 1_000)).unwrap().unwrap();
    let pages: i64 = conn.query_row("PRAGMA page_count", [], |r| r.get(0)).unwrap();
    // SQLite's page limit stands in for a full disk: a write that needs a new page fails with SQLITE_FULL.
    conn.query_row(&format!("PRAGMA max_page_count = {pages}"), [], |_| Ok(())).unwrap();
    let mut written = 0;
    let full = loop {
        match store::record_usage(&conn, &answered(&client.id)) {
            Ok(()) => written += 1,
            Err(e) => break e,
        }
        assert!(written < 100_000, "the page limit never filled");
    };
    assert!(full.contains("full"), "{full}");
    let e = conn.execute_batch("CREATE TABLE filler (x)").unwrap_err();
    assert_eq!(db::problem(&e), db::Problem::Full);
    // A settlement that cannot be written changes nothing: the journal entry stays, so its hold still counts.
    let before = count(&conn, "usage_events");
    assert!(store::settle(&conn, &id, &answered(&client.id)).is_err());
    assert_eq!((count(&conn, "usage_events"), count(&conn, "budget_reservations")), (before, 1));
    db::check(&conn).unwrap();
    // Space again: it settles once.
    conn.query_row("PRAGMA max_page_count = 1073741823", [], |_| Ok(())).unwrap();
    store::settle(&conn, &id, &answered(&client.id)).unwrap();
    assert_eq!((count(&conn, "usage_events"), count(&conn, "budget_reservations")), (before + 1, 0));
}

#[test]
fn a_store_that_is_not_a_database_is_named_damaged_and_restored_from_its_copy() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite3");
    {
        let conn = db::open(&path).unwrap();
        store::create_client(&conn, "Kept", None).unwrap();
        backup::daily(&conn, dir.path(), db::now_ms()).unwrap().unwrap();
    }
    std::fs::write(&path, b"this is not a SQLite database, as after a disk error or a bad copy").unwrap();
    let e = db::open(&path).unwrap_err();
    assert_eq!(db::problem(&e), db::Problem::Damaged, "{e}");

    let (copy, aside) = backup::restore_newest(dir.path(), db::now_ms()).unwrap();
    assert!(copy.starts_with(backup::folder(dir.path())));
    assert!(std::fs::read(&aside).unwrap().starts_with(b"this is not"), "the damaged store is kept, not deleted");
    let conn = db::open(&path).unwrap();
    db::check(&conn).unwrap();
    let name: String = conn.query_row("SELECT name FROM gateway_clients", [], |r| r.get(0)).unwrap();
    assert_eq!(name, "Kept");
}

#[test]
fn damaged_pages_behind_a_good_header_are_found_at_start_up() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite3");
    {
        let conn = db::open(&path).unwrap();
        let (client, _) = store::create_client(&conn, "Tests", None).unwrap();
        for _ in 0..200 {
            store::record_usage(&conn, &answered(&client.id)).unwrap();
        }
    }
    // Overwrite pages in the middle of the file, leaving the header (and so the schema version) readable.
    let mut bytes = std::fs::read(&path).unwrap();
    let page: usize = u16::from_be_bytes([bytes[16], bytes[17]]) as usize;
    assert!(bytes.len() > page * 8, "the store has pages to damage");
    for b in &mut bytes[page * 3..page * 6] {
        *b = 0xA5;
    }
    std::fs::write(&path, &bytes).unwrap();
    // Opening alone does not notice (the header is fine); the start-up check does, before a request trips on it.
    match db::open(&path) {
        Ok(conn) => {
            let e = db::check(&conn).unwrap_err();
            assert_eq!(db::problem(&e), db::Problem::Damaged, "{e}");
        }
        Err(e) => assert_eq!(db::problem(&e), db::Problem::Damaged, "{e}"),
    }
}

#[test]
fn restoring_without_a_copy_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite3");
    std::fs::write(&path, b"damaged").unwrap();
    assert!(backup::restore_newest(dir.path(), db::now_ms()).unwrap_err().contains("no daily copy"));
    assert_eq!(std::fs::read(&path).unwrap(), b"damaged");
}

#[test]
fn a_newer_store_is_named_as_such() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite3");
    {
        let conn = db::open(&path).unwrap();
        conn.execute_batch(&format!("PRAGMA user_version = {}", db::SCHEMA_VERSION + 1)).unwrap();
    }
    assert_eq!(db::problem(&db::open(&path).unwrap_err()), db::Problem::Newer);
}
