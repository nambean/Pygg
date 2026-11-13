//! Pygg's own log (Codex's production review, 2026-11-14): what happened to the app (start-up, the gateway, backups,
//! a panic), in a file a user can read and that the diagnostics bundle includes, so a problem can be reported without
//! a debugger. Not the request ledger (`pygg logs`). Every line is redacted before it is written; the file is capped at
//! `MAX_BYTES` and rotated, keeping `KEEP` older files.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// The size at which the log is rotated.
pub const MAX_BYTES: u64 = 1_000_000;
/// Older files kept: `pygg.log.1`, `pygg.log.2`.
pub const KEEP: usize = 2;

static FILE: OnceLock<PathBuf> = OnceLock::new();
static WRITING: Mutex<()> = Mutex::new(());

/// Starts the log in `<data dir>/logs/pygg.log`, and records panics there as well as on the console.
pub fn init(data_dir: &Path) {
    let path = data_dir.join("logs").join("pygg.log");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if FILE.set(path).is_ok() {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            line("panic", &info.to_string());
            previous(info);
        }));
    }
}

/// Where the log is, once `init` ran.
pub fn path() -> Option<&'static Path> {
    FILE.get().map(PathBuf::as_path)
}

/// Writes one line, redacted, with the time in UTC; also to the console.
pub fn line(level: &str, message: &str) {
    let text = crate::redact::redact(message, &[]).replace(['\r', '\n'], " ");
    eprintln!("{level}: {text}");
    let Some(path) = FILE.get() else { return };
    let _guard = WRITING.lock().unwrap_or_else(|p| p.into_inner());
    if std::fs::metadata(path).is_ok_and(|m| m.len() > MAX_BYTES) {
        rotate(path);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{} {level} {text}", crate::gateway::store::iso_utc(crate::db::now_ms()));
    }
}

pub fn info(message: &str) {
    line("info", message);
}

pub fn warn(message: &str) {
    line("warn", message);
}

fn rotate(path: &Path) {
    let numbered = |n: usize| PathBuf::from(format!("{}.{n}", path.display()));
    let _ = std::fs::remove_file(numbered(KEEP));
    for n in (1..KEEP).rev() {
        let _ = std::fs::rename(numbered(n), numbered(n + 1));
    }
    let _ = std::fs::rename(path, numbered(1));
}

/// The last `n` lines of the current file, for the diagnostics bundle.
pub fn tail(n: usize) -> Vec<String> {
    let Some(path) = FILE.get() else { return Vec::new() };
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(n)..].iter().map(|l| l.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_keeps_two_older_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pygg.log");
        for n in 0..4 {
            std::fs::write(&path, format!("file {n}")).unwrap();
            rotate(&path);
        }
        assert!(!path.exists());
        assert_eq!(std::fs::read_to_string(dir.path().join("pygg.log.1")).unwrap(), "file 3");
        assert_eq!(std::fs::read_to_string(dir.path().join("pygg.log.2")).unwrap(), "file 2");
        assert!(!dir.path().join("pygg.log.3").exists());
    }
}
