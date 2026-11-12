//! Pygg desktop core (repository: SmartTaskRouter). One crate with modules (docs/DECISIONS.md, ADR-005): the interface
//! talks to it through Tauri commands (src/commands.rs), and the local gateway runs in the same process.

pub mod agents;
pub mod bench;
pub mod checks;
pub mod cli;
pub mod commands;
pub mod db;
pub mod diagnostics;
pub mod gateway;
pub mod handoff;
pub mod integrations;
pub mod isolation;
pub mod launcher;
pub mod mcp;
pub mod messages;
pub mod process;
pub mod redact;
pub mod retention;
pub mod shell;
pub mod snap;
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
    Ok(HealthStatus { app: "Pygg", version: env!("CARGO_PKG_VERSION"), gateway })
}

/// The configured 1440×900 is a preference, not a requirement: on a smaller or scaled screen the window
/// shrinks to fit the monitor's work area and is centered. The 1024×640 minimum gives way too when the work area
/// itself is smaller, as on a 1920×1080 screen at 175% or 200%, or 1366×768 at 125% (D12 release checks): a window
/// larger than the screen cannot be moved or resized back into view.
fn fit_to_screen(app: &tauri::App) {
    let Some(window) = app.get_webview_window("main") else { return };
    let Ok(Some(monitor)) = window.current_monitor() else { return };
    let scale = monitor.scale_factor();
    let area = monitor.work_area().size.to_logical::<f64>(scale);
    let (min_width, min_height) = fitted_minimum(area.width, area.height);
    let _ = window.set_min_size(Some(tauri::LogicalSize::new(min_width, min_height)));
    let width = (area.width - 48.0).min(1440.0).max(min_width);
    let height = (area.height - 48.0).min(900.0).max(min_height);
    let _ = window.set_size(tauri::LogicalSize::new(width, height));
    let _ = window.center();
}

/// The window's minimum size on a work area of this logical size: 1024×640, or the whole work area when it is smaller.
fn fitted_minimum(area_width: f64, area_height: f64) -> (f64, f64) {
    (1024f64.min(area_width), 640f64.min(area_height))
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_minimum_window_fits_a_scaled_screen() {
        assert_eq!(super::fitted_minimum(1920.0, 1040.0), (1024.0, 640.0), "1080p at 100%");
        // 1080p at 200%: 960×540 logical, less a 24 px taskbar.
        assert_eq!(super::fitted_minimum(960.0, 516.0), (960.0, 516.0));
        assert_eq!(super::fitted_minimum(1097.0, 590.0), (1024.0, 590.0), "1080p at 175%");
    }
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
            // Models from before the tier table (ROADMAP B3) get its starting tiers where nobody set one.
            if let Err(e) = gateway::store::fill_curated_tiers(&conn) {
                eprintln!("could not fill tiers: {e}");
            }
            // Paid models on a connection used for its free models start off (ADR-032), stores from before included.
            if let Err(e) = gateway::store::apply_free_connection_rule(&conn) {
                eprintln!("could not apply the free-connection rule: {e}");
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
            // Facts the providers' lists leave out are looked up in the reference catalog (ADR-031), at most once a
            // day, in the background; the cached copy serves offline.
            {
                let shared = state.shared.clone();
                let cache = gateway::reference::cache_path(&path);
                tauri::async_runtime::spawn(async move {
                    if let Some(catalog) = gateway::reference::current(&shared.http, &cache).await {
                        shared.with_db(|db| match gateway::reference::apply(db, &catalog) {
                            Ok(_) => gateway::reference::mark_applied(db),
                            Err(e) => eprintln!("reference catalog: {e}"),
                        });
                    }
                    // What each account has left, where the provider has an endpoint for it (quota.rs).
                    gateway::quota::refresh_accounts(&shared).await;
                    // OpenCode's list of Pygg's models follows what the catalog filled (integrations.rs).
                    if let Err(e) = integrations::sync_opencode_models(&shared, &integrations::Homes::from_env()) {
                        eprintln!("OpenCode's model list: {e}");
                    }
                });
            }
            // Usage history older than the user keeps (Settings → Data) goes at start-up and once a day; forever by default.
            {
                let shared = state.shared.clone();
                tauri::async_runtime::spawn(async move {
                    loop {
                        if let Err(e) = shared.with_db(|db| retention::prune(db, db::now_ms())) {
                            eprintln!("usage history clean-up: {e}");
                        }
                        tokio::time::sleep(retention::EVERY).await;
                    }
                });
            }
            if state.autostart() {
                // A busy port is reported in Endpoints rather than stopping the app from opening.
                let _ = tauri::async_runtime::block_on(state.start());
            }
            app.manage(state);
            shell::setup_tray(app)?;
            // Spending alerts as notifications, checked every ten minutes while Pygg runs (shell.rs).
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    loop {
                        tokio::time::sleep(std::time::Duration::from_secs(600)).await;
                        shell::spending_notifications(&handle);
                    }
                });
            }
            fit_to_screen(app);
            Ok(())
        })
        .on_window_event(shell::on_window_event)
        // The window starts hidden (tauri.conf.json) and appears once the page with its start-up screen has
        // loaded, so it never opens blank.
        .on_page_load(|webview, payload| {
            // Opened at sign-in, Pygg stays in the tray until the user opens it.
            if payload.event() == tauri::webview::PageLoadEvent::Finished && !shell::started_in_background() {
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
            commands::connection_sign_in,
            commands::connection_update,
            commands::connection_set_key,
            commands::connection_remove,
            commands::connection_set_terms,
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
            commands::integrations_list,
            commands::integration_plan,
            commands::integration_apply,
            commands::integration_remove,
            commands::task_budget,
            commands::tasks_quality,
            commands::workspace_set_privacy,
            commands::pick_folder,
            commands::task_export_patch,
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
            commands::usage_over_time,
            commands::usage_export,
            commands::usage_alerts,
            commands::usage_shadow,
            commands::usage_subscriptions,
            commands::usage_local,
            commands::retention_get,
            commands::retention_set,
            commands::retention_preview,
            commands::connection_check_account,
            commands::connection_set_pool,
            commands::model_test,
            commands::connection_reliability,
            commands::client_latest_requests,
            commands::usage_savings,
            commands::usage_set_savings_baseline,
            commands::launcher_state,
            commands::launcher_set_up,
            commands::launcher_remove,
            commands::connection_move_in_pool,
            commands::diagnostics_export,
            snap::window_snap_area,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pygg");
}
