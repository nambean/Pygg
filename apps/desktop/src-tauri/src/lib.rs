//! SmartTaskRouter desktop core. One crate with modules (docs/DECISIONS.md, ADR-005): the interface
//! talks to it through Tauri commands; the local gateway (not built yet) will run in the same process.

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthStatus {
    pub app: &'static str,
    pub version: &'static str,
    /// `running`, `stopped` or `not_built`: the gateway is not part of this build yet.
    pub gateway: &'static str,
}

#[tauri::command]
fn health_check() -> HealthStatus {
    HealthStatus { app: "SmartTaskRouter", version: env!("CARGO_PKG_VERSION"), gateway: "not_built" }
}

pub fn run() {
    tauri::Builder::default()
        // First: a second launch only brings the running window forward.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            use tauri::Manager;
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        // The window starts hidden (tauri.conf.json) and appears once the page with its start-up screen has
        // loaded, so it never opens blank.
        .on_page_load(|webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                let _ = webview.window().show();
            }
        })
        .invoke_handler(tauri::generate_handler![health_check])
        .run(tauri::generate_context!())
        .expect("error while running SmartTaskRouter");
}
