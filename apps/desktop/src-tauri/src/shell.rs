//! Pygg's place in the system: a tray icon that keeps the gateway and tasks running when the window is closed
//! (review GW-9), and notifications about tasks that need the user (SCREENS.md §2.5). Per ADR-010 a
//! notification never names a task or a path, and never carries an approve button: approving happens in the app.

use crate::commands::AppState;
use crate::gateway::store;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Window, WindowEvent, Wry};
use tauri_plugin_notification::NotificationExt;

const KEEP_IN_TRAY: &str = "app.keep_in_tray";
const NOTIFY_TASKS: &str = "app.notify_tasks";
const NOTIFY_SPENDING: &str = "app.notify_spending";
/// The alerts already notified, so each is told once a day (`<kind>|<client>` → UTC day).
const SPENDING_NOTIFIED: &str = "app.spending_notified";
const TRAY_HINT_SHOWN: &str = "app.tray_hint_shown";

/// The argument Pygg is started with at sign-in: it opens in the tray, without its window.
pub const BACKGROUND_ARG: &str = "--background";

/// Whether this process was started at sign-in, to stay in the tray.
pub fn started_in_background() -> bool {
    std::env::args().any(|a| a == BACKGROUND_ARG)
}

/// Opening Pygg when the user signs in to Windows: a value in the user's own Run key, pointing at this program with
/// `--background`. The key is the truth, so a change made in Windows' Startup apps shows here too.
#[cfg(windows)]
mod sign_in {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
    use winreg::RegKey;

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE: &str = "Pygg";

    pub fn enabled() -> bool {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(RUN, KEY_READ)
            .and_then(|key| key.get_value::<String, _>(VALUE))
            .is_ok_and(|v| !v.trim().is_empty())
    }

    pub fn set(on: bool) -> Result<(), String> {
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN).map_err(|e| e.to_string())?;
        if on {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            key.set_value(VALUE, &format!("\"{}\" {}", exe.display(), super::BACKGROUND_ARG)).map_err(|e| e.to_string())
        } else {
            match key.delete_value(VALUE) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            }
        }
    }
}

/// Writes the real Run key of whoever runs it, so it is not run by default:
/// `cargo test --lib shell::tests -- --ignored`.
#[cfg(all(test, windows))]
mod tests {
    #[test]
    #[ignore]
    fn opening_at_sign_in_is_a_run_value_pointing_here_in_the_background() {
        let before = super::sign_in::enabled();
        super::sign_in::set(true).unwrap();
        assert!(super::sign_in::enabled());
        let value: String = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
            .unwrap()
            .get_value("Pygg")
            .unwrap();
        let exe = std::env::current_exe().unwrap();
        assert_eq!(value, format!("\"{}\" --background", exe.display()));
        super::sign_in::set(false).unwrap();
        assert!(!super::sign_in::enabled());
        super::sign_in::set(false).unwrap();
        if before {
            super::sign_in::set(true).unwrap();
        }
    }
}

/// Elsewhere it is not offered yet (ADR-001: Windows first).
#[cfg(not(windows))]
mod sign_in {
    pub fn enabled() -> bool {
        false
    }
    pub fn set(_on: bool) -> Result<(), String> {
        Err("Opening at sign-in is available on Windows only".into())
    }
}

/// The tray's and the notifications' words in the user's language. The interface sends them when its language
/// is set (`ui_strings`); until then they are English.
#[derive(Default)]
pub struct Strings(Mutex<HashMap<String, String>>);

impl Strings {
    fn get(&self, key: &str, english: &str) -> String {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).get(key).cloned().unwrap_or_else(|| english.to_string())
    }
}

/// The tray menu's items, kept to relabel them when the language changes.
pub struct TrayItems {
    open: MenuItem<Wry>,
    quit: MenuItem<Wry>,
}

fn preference(app: &AppHandle, key: &str) -> bool {
    let state = app.state::<AppState>();
    state.shared.with_db(|db| store::setting(db, key)).ok().flatten().as_deref() != Some("false")
}

/// Brings the window back. Shown before it is unminimized: unminimizing a hidden window first leaves it hidden.
pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Stops the gateway (requests in flight are let finish, as on Ctrl+C) and quits. Tasks still running become
/// interrupted at the next start, with Retry.
fn quit(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        app.state::<AppState>().stop().await;
        app.exit(0);
    });
}

pub fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Pygg", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit Pygg", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &quit_item])?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Pygg")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "quit" => quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    app.manage(TrayItems { open, quit: quit_item });
    Ok(())
}

/// Closing the window hides it while "keep running in the tray" is on, so the gateway and tasks go on; the
/// first time, a notification says where Pygg went.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    let WindowEvent::CloseRequested { api, .. } = event else { return };
    // A web session's window (ADR-035) only hides: a request may be using it, and it closes itself once unused.
    if crate::websession::window::is_web_window(window.label()) {
        api.prevent_close();
        let _ = window.hide();
        return;
    }
    let app = window.app_handle();
    if !preference(app, KEEP_IN_TRAY) {
        return;
    }
    api.prevent_close();
    let _ = window.hide();
    let state = app.state::<AppState>();
    let shown = state.shared.with_db(|db| store::setting(db, TRAY_HINT_SHOWN)).ok().flatten().is_some();
    if !shown {
        let _ = state.shared.with_db(|db| store::set_setting(db, TRAY_HINT_SHOWN, "true"));
        let s = app.state::<Strings>();
        let _ = app
            .notification()
            .builder()
            .title(s.get("tray.hint.title", "Pygg is still running"))
            .body(s.get(
                "tray.hint.body",
                "The gateway and your tasks keep running. Open Pygg or quit it from the tray icon.",
            ))
            .show();
    }
}

/// A task's runner stopped: the interface reloads, and the user is told when the task needs them.
/// The spending alerts worth a notification now (store::usage_alerts): a client near or over its monthly limit, a spend
/// spike, a burst of failures. Unknown costs are left to the Usage screen. Each is notified once per UTC day; the
/// notification names no client or amount (as ADR-010 keeps task notifications plain), only what to look at.
pub fn spending_notifications(app: &AppHandle) {
    if !preference(app, NOTIFY_SPENDING) {
        return;
    }
    let state = app.state::<AppState>();
    let now = crate::db::now_ms();
    let day = now / 86_400_000;
    let (alerts, mut notified) = state.shared.with_db(|db| {
        let alerts = store::usage_alerts(db, now).unwrap_or_default();
        let notified: HashMap<String, i64> = store::setting(db, SPENDING_NOTIFIED)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        (alerts, notified)
    });
    notified.retain(|_, d| *d == day);
    let s = app.state::<Strings>();
    for alert in alerts {
        let key = format!("{}|{}", alert.kind, alert.client.clone().unwrap_or_default());
        if notified.contains_key(&key) {
            continue;
        }
        let (title, body) = match alert.kind.as_str() {
            "over_limit" => (
                s.get("notify.over_limit.title", "A client has reached its monthly limit"),
                s.get(
                    "notify.over_limit.body",
                    "Its requests are refused until next month or a higher limit. Open Pygg to see Usage.",
                ),
            ),
            "near_limit" => (
                s.get("notify.near_limit.title", "A client is near its monthly limit"),
                s.get("notify.near_limit.body", "It has used 80% or more of it. Open Pygg to see Usage."),
            ),
            "spend_spike" => (
                s.get("notify.spike.title", "Spending is up"),
                s.get(
                    "notify.spike.body",
                    "The last 24 hours cost three times the week's daily average or more. Open Pygg to see Usage.",
                ),
            ),
            "failures" => (
                s.get("notify.failures.title", "Many requests are failing"),
                s.get(
                    "notify.failures.body",
                    "Half or more of the last hour's requests failed. Open Pygg to see Usage.",
                ),
            ),
            _ => continue,
        };
        let _ = app.notification().builder().title(title).body(body).show();
        notified.insert(key, day);
    }
    let text = serde_json::to_string(&notified).unwrap_or_default();
    let _ = state.shared.with_db(|db| store::set_setting(db, SPENDING_NOTIFIED, &text));
}

pub fn task_stopped(app: &AppHandle, state: &str) {
    let _ = app.emit("tasks-changed", ());
    if !preference(app, NOTIFY_TASKS) {
        return;
    }
    let s = app.state::<Strings>();
    let (title, body) = match state {
        "waiting_approval" => (
            s.get("notify.approval.title", "A task is waiting for your approval"),
            s.get("notify.approval.body", "Open Pygg to approve or decline its paid step."),
        ),
        "checks_passed" => (
            s.get("notify.passed.title", "A task is ready"),
            s.get("notify.passed.body", "Its checks passed. Open Pygg to keep or discard the result."),
        ),
        "needs_review" => (
            s.get("notify.review.title", "A task needs your review"),
            s.get("notify.review.body", "It finished, but no check confirmed it. Open Pygg to review the changes."),
        ),
        "check_failed" | "blocked" => (
            s.get("notify.failed.title", "A task did not finish"),
            s.get("notify.failed.body", "Open Pygg to see why each step stopped."),
        ),
        // Stopped by the user: they know.
        _ => return,
    };
    let _ = app.notification().builder().title(title).body(body).show();
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    pub keep_in_tray: bool,
    pub notify_tasks: bool,
    /// Notify when a client nears its limit, spending spikes or requests keep failing.
    pub notify_spending: bool,
    /// Pygg opens in the tray when the user signs in to Windows.
    pub open_at_sign_in: bool,
}

#[tauri::command]
pub fn app_preferences(app: AppHandle) -> Preferences {
    Preferences {
        keep_in_tray: preference(&app, KEEP_IN_TRAY),
        notify_tasks: preference(&app, NOTIFY_TASKS),
        notify_spending: preference(&app, NOTIFY_SPENDING),
        open_at_sign_in: sign_in::enabled(),
    }
}

#[tauri::command]
pub fn app_set_preference(app: AppHandle, key: String, value: bool) -> Result<Preferences, String> {
    if key == "openAtSignIn" {
        sign_in::set(value)?;
        return Ok(app_preferences(app));
    }
    let key = match key.as_str() {
        "keepInTray" => KEEP_IN_TRAY,
        "notifyTasks" => NOTIFY_TASKS,
        "notifySpending" => NOTIFY_SPENDING,
        other => return Err(format!("unknown preference {other}")),
    };
    app.state::<AppState>().shared.with_db(|db| store::set_setting(db, key, if value { "true" } else { "false" }))?;
    Ok(app_preferences(app))
}

/// The tray's and the notifications' words in the interface's language.
#[tauri::command]
pub fn ui_strings(app: AppHandle, strings: HashMap<String, String>) {
    if let Some(items) = app.try_state::<TrayItems>() {
        if let Some(open) = strings.get("tray.open") {
            let _ = items.open.set_text(open);
        }
        if let Some(quit) = strings.get("tray.quit") {
            let _ = items.quit.set_text(quit);
        }
    }
    *app.state::<Strings>().0.lock().unwrap_or_else(|p| p.into_inner()) = strings;
}
