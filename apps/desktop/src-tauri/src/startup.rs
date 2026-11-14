//! What the app does when its store cannot be opened at start-up (fault tests, 2026-11-14). Before, the app logged the
//! error and quit without a window, so a damaged store looked like Pygg not starting at all. Now it says why in a
//! Windows message box and, for a damaged store with a daily copy, offers to restore that copy (backup.rs); every
//! other case quits after saying what to do. Each case is also written to the app log.

use crate::{applog, backup, db};
use rusqlite::Connection;
use std::path::Path;

/// Opens the store after a failed first open, or returns the error the app quits with.
pub fn store_failed(dir: &Path, path: &Path, e: &rusqlite::Error) -> Result<Connection, Box<dyn std::error::Error>> {
    let copies = backup::list(dir);
    let problem = db::problem(e);
    applog::line(
        "error",
        &format!(
            "could not open {}: {e} ({problem:?}); {} daily copies in {}",
            path.display(),
            copies.len(),
            backup::folder(dir).display()
        ),
    );
    let newest = copies.last().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
    match (problem, newest) {
        (db::Problem::Damaged, Some(newest)) => {
            let restore = ask(&format!(
                "Pygg's store is damaged and cannot be opened:\n{e}\n\nRestore the newest daily copy ({newest})? Usage, \
                 connections and settings changed after that copy are lost. The damaged store is kept next to it, \
                 renamed, and no key is affected (keys stay in Windows Credential Manager).\n\nNo quits Pygg without \
                 changing anything."));
            if !restore {
                return Err(e.to_string().into());
            }
            let (copy, aside) = backup::restore_newest(dir, db::now_ms())?;
            applog::warn(&format!(
                "restored {} over the store; the damaged store is {}",
                copy.display(),
                aside.display()
            ));
            let conn = db::open(path)?;
            db::check(&conn)?;
            Ok(conn)
        }
        (problem, _) => {
            tell(&match problem {
                db::Problem::Damaged => format!(
                    "Pygg's store is damaged and cannot be opened:\n{e}\n\nThere is no daily copy \
                    to restore yet. The store is {}; report the problem with that file kept.",
                    path.display()
                ),
                db::Problem::Busy => "Another Pygg process holds the store (a `pygg gateway serve`, or a Pygg that is \
                    closing). Stop it and open Pygg again."
                    .to_string(),
                db::Problem::Newer => {
                    "This store was written by a newer version of Pygg. Install that version again; an \
                    older one does not write a newer store."
                        .to_string()
                }
                db::Problem::Full => {
                    "The disk holding Pygg's data is full. Free some space and open Pygg again.".to_string()
                }
                db::Problem::Other => {
                    format!("Pygg could not open its store:\n{e}\n\nThe store is {}.", path.display())
                }
            });
            Err(e.to_string().into())
        }
    }
}

#[cfg(windows)]
fn message(
    text: &str, buttons: windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_STYLE,
) -> windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_RESULT {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_SETFOREGROUND};
    let (text, title) = (HSTRING::from(text), HSTRING::from("Pygg"));
    unsafe {
        MessageBoxW(None, PCWSTR(text.as_ptr()), PCWSTR(title.as_ptr()), buttons | MB_ICONERROR | MB_SETFOREGROUND)
    }
}

#[cfg(windows)]
fn ask(text: &str) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{IDYES, MB_YESNO};
    message(text, MB_YESNO) == IDYES
}

#[cfg(windows)]
fn tell(text: &str) {
    message(text, windows::Win32::UI::WindowsAndMessaging::MB_OK);
}

// Other platforms: the log says it; nothing is restored without asking.
#[cfg(not(windows))]
fn ask(_text: &str) -> bool {
    false
}

#[cfg(not(windows))]
fn tell(_text: &str) {}
