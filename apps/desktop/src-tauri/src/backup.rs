//! Daily copies of the store (Codex's production review, 2026-11-14): the ledger, connections and settings are in one
//! SQLite file, and nothing else keeps them. At start-up and once a day while Pygg runs, a consistent copy is written
//! with `VACUUM INTO` to `<data dir>/backups/store-<date>.sqlite3`, at most once in `EVERY_MS`, and the newest `KEEP` are
//! kept. Keys are not in the store (they stay in the Windows Credential Manager), so a copy holds none.
//!
//! To restore: quit Pygg (and any `pygg gateway serve`), then copy a backup over `store.sqlite3` in the data folder.

use rusqlite::Connection;
use std::path::{Path, PathBuf};

/// The least time between two copies.
pub const EVERY_MS: i64 = 20 * 3_600_000;
/// Copies kept.
pub const KEEP: usize = 3;

pub fn folder(data_dir: &Path) -> PathBuf {
    data_dir.join("backups")
}

/// The copies there, oldest first.
pub fn list(data_dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(folder(data_dir)).into_iter().flatten().flatten().map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("store-") && n.ends_with(".sqlite3")))
        .collect();
    found.sort();
    found
}

/// Writes a copy unless the newest is younger than `EVERY_MS`; returns the new copy, if one was made.
pub fn daily(conn: &Connection, data_dir: &Path, now_ms: i64) -> Result<Option<PathBuf>, String> {
    let dir = folder(data_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let newest = list(data_dir).last().and_then(|p| std::fs::metadata(p).ok()).and_then(|m| m.modified().ok());
    if let Some(modified) = newest {
        let age = std::time::SystemTime::now().duration_since(modified).map(|d| d.as_millis() as i64).unwrap_or(0);
        if age < EVERY_MS {
            return Ok(None);
        }
    }
    let stamp = crate::gateway::store::iso_utc(now_ms)[..16].replace([':', '-'], "").replace('T', "-");
    let path = dir.join(format!("store-{stamp}.sqlite3"));
    if path.exists() {
        return Ok(None);
    }
    conn.execute("VACUUM INTO ?1", [path.to_string_lossy().as_ref()]).map_err(|e| format!("backing up the store: {e}"))?;
    let all = list(data_dir);
    for old in all.iter().take(all.len().saturating_sub(KEEP)) {
        let _ = std::fs::remove_file(old);
    }
    Ok(Some(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_copy_a_day_and_the_newest_three_kept() {
        let dir = tempfile::tempdir().unwrap();
        let conn = crate::db::open(&dir.path().join("store.sqlite3")).unwrap();
        crate::gateway::store::set_setting(&conn, "marker", "kept").unwrap();
        let first = daily(&conn, dir.path(), 1_794_469_641_058).unwrap().expect("a first copy");
        let copy = Connection::open(&first).unwrap();
        assert_eq!(crate::gateway::store::setting(&copy, "marker").unwrap().as_deref(), Some("kept"), "the copy is the store");
        assert_eq!(daily(&conn, dir.path(), 1_794_469_641_058 + 3_600_000).unwrap(), None, "not twice in a day");
        // Older copies, as if from earlier days: only the newest KEEP stay.
        for day in 1..=4 {
            std::fs::write(folder(dir.path()).join(format!("store-2026010{day}-0000.sqlite3")), b"old").unwrap();
        }
        let newer = filetime_now_minus(&folder(dir.path()), 30 * 3_600_000);
        assert!(newer);
        assert!(daily(&conn, dir.path(), 1_794_469_641_058 + 86_400_000).unwrap().is_some());
        assert_eq!(list(dir.path()).len(), KEEP);
    }

    /// Ages every copy in `dir` by `ms`, so the next `daily` sees them as old.
    fn filetime_now_minus(dir: &Path, ms: u64) -> bool {
        let when = std::time::SystemTime::now() - std::time::Duration::from_millis(ms);
        std::fs::read_dir(dir).unwrap().flatten().all(|e| {
            std::fs::File::options().write(true).open(e.path()).and_then(|f| f.set_modified(when)).is_ok()
        })
    }
}
