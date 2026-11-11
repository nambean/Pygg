//! SmartTaskRouter desktop core. One crate with modules (docs/DECISIONS.md, ADR-005): the interface
//! talks to it through Tauri commands (src/commands.rs), and the local gateway runs in the same process.

pub mod agents;
pub mod bench;
pub mod checks;
pub mod commands;
pub mod db;
pub mod gateway;
pub mod isolation;
pub mod mcp;
pub mod process;
pub mod redact;
pub mod shell;
pub mod tasks;
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
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| shell::show_main(app)))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(shell::Strings::default())
        .setup(|app| {
            // PYGG_DATA_DIR: another store, for trying the app without touching the real one.
            let dir = match std::env::var_os("PYGG_DATA_DIR") {
                Some(dir) => std::path::PathBuf::from(dir),
                None => app.path().app_data_dir()?,
            };
            std::fs::create_dir_all(&dir)?;
            let path = dir.join("store.sqlite3");
            let conn = db::open(&path)?;
            let worktrees = dir.join("worktrees");
            // Nothing runs here yet: a task left running was cut off when the app last closed, unless another process
            // (`gateway serve --tasks`) holds the store and may be running it.
            if gateway::store_in_use(&path) {
                eprintln!("another gateway holds the store; tasks are not recovered");
            } else if let Err(e) = tasks::recover(&conn, &worktrees) {
                eprintln!("could not recover tasks: {e}");
            }
            let db = Arc::new(Mutex::new(conn));
            let handle = app.handle().clone();
            let runner = tasks::Runner::new(db.clone(), worktrees, tasks::agent_executor())
                .with_notify(Arc::new(move |_task, state| shell::task_stopped(&handle, state)));
            // IDE agents start tasks through the gateway's task API (gateway/task_api.rs); they run here.
            let shared = gateway::Shared::for_store(db, Arc::new(gateway::OsSecrets), &path).with_tasks(runner.clone());
            let state = AppState { shared, server: tokio::sync::Mutex::new(None), last_error: Mutex::new(None),
                data_dir: dir.clone(), runs: Mutex::new(Default::default()),
                tasks: runner };
            if state.autostart() {
                // A busy port is reported in Endpoints rather than stopping the app from opening.
                let _ = tauri::async_runtime::block_on(state.start());
            }
            app.manage(state);
            shell::setup_tray(app)?;
            fit_to_screen(app);
            Ok(())
        })
        .on_window_event(shell::on_window_event)
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
            commands::model_set_capabilities,
            commands::auto_simulate,
            commands::agents_detect,
            commands::agent_run,
            commands::agent_diff,
            commands::agent_finish,
            commands::workspaces_list,
            commands::workspace_add,
            commands::workspace_remove,
            commands::workspace_detect_checks,
            commands::workspace_check,
            commands::tasks_list,
            commands::task_get,
            commands::task_create,
            commands::task_cancel,
            commands::task_decide,
            commands::task_retry,
            commands::task_diff,
            commands::task_finish,
            commands::mcp_setup,
            commands::task_budget,
            commands::workspace_set_privacy,
            commands::pick_folder,
            commands::task_set_budget,
            commands::workspace_set_budget,
            shell::app_preferences,
            shell::app_set_preference,
            shell::ui_strings,
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
