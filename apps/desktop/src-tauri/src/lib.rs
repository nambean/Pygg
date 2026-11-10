//! SmartTaskRouter desktop core. One crate with modules (docs/DECISIONS.md, ADR-005): the interface
//! talks to it through Tauri commands (src/commands.rs), and the local gateway runs in the same process.

pub mod bench;
pub mod checks;
pub mod commands;
pub mod db;
pub mod gateway;
pub mod isolation;
pub mod process;
pub mod redact;
pub mod vault;

use commands::AppState;
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::Manager;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthStatus {
    pub app: &'static str,
    pub version: &'static str,
    /// `running`, `stopped` or `failed`.
    pub gateway: &'static str,
}

#[tauri::command]
async fn health_check(state: tauri::State<'_, AppState>) -> Result<HealthStatus, String> {
    let gateway = commands::gateway_status(state).await?.state;
    Ok(HealthStatus { app: "SmartTaskRouter", version: env!("CARGO_PKG_VERSION"), gateway })
}

/// The configured 1440×900 is a preference, not a requirement: on a smaller or scaled screen the window
/// shrinks to fit the monitor's work area (never below the 1024×640 minimum) and is centered.
fn fit_to_screen(app: &tauri::App) {
    let Some(window) = app.get_webview_window("main") else { return };
    let Ok(Some(monitor)) = window.current_monitor() else { return };
    let scale = monitor.scale_factor();
    let area = monitor.work_area().size.to_logical::<f64>(scale);
    let width = (area.width - 48.0).min(1440.0).max(1024.0);
    let height = (area.height - 48.0).min(900.0).max(640.0);
    let _ = window.set_size(tauri::LogicalSize::new(width, height));
    let _ = window.center();
}

pub fn run() {
    tauri::Builder::default()
        // First: a second launch only brings the running window forward.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = db::open(&dir.join("store.sqlite3"))?;
            let shared = gateway::Shared::new(Arc::new(Mutex::new(conn)), Arc::new(gateway::OsSecrets));
            let state = AppState { shared, server: tokio::sync::Mutex::new(None), last_error: Mutex::new(None) };
            if state.autostart() {
                // A busy port is reported in Endpoints rather than stopping the app from opening.
                let _ = tauri::async_runtime::block_on(state.start());
            }
            app.manage(state);
            fit_to_screen(app);
            Ok(())
        })
        // The window starts hidden (tauri.conf.json) and appears once the page with its start-up screen has
        // loaded, so it never opens blank.
        .on_page_load(|webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                let _ = webview.window().show();
            }
        })
        .invoke_handler(tauri::generate_handler![
            health_check,
            commands::gateway_status,
            commands::gateway_start,
            commands::gateway_stop,
            commands::gateway_configure,
            commands::presets_list,
            commands::connections_list,
            commands::connection_add,
            commands::connection_update,
            commands::connection_set_key,
            commands::connection_remove,
            commands::connection_refresh_models,
            commands::models_list,
            commands::model_add,
            commands::model_update,
            commands::model_remove,
            commands::combos_list,
            commands::combo_save,
            commands::combo_delete,
            commands::clients_list,
            commands::client_create,
            commands::client_revoke,
            commands::client_set_budget,
            commands::client_set_policy,
            commands::connection_clear_health,
            commands::usage_summary,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SmartTaskRouter");
}
