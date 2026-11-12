//! The app's windows for web sessions (ADR-035): one Tauri webview per connection, with its own WebView2 profile folder,
//! hidden while Pygg drives it and shown when the user opens it to sign in.
//!
//! The window has no Tauri capability (`capabilities/default.json` names only `main`), and Tauri refuses every
//! command from a remote page without one, so the site's scripts cannot call Pygg. It may navigate only to the
//! recipe's hosts and opens no other windows. Nothing here reads cookies: WebView2 keeps them in the profile folder,
//! encrypted for the Windows user, and the site renews its own session.

use super::{host_allowed, Page, Recipe, WebError, Windows, NO_WINDOW};
use crate::gateway::store::Provider;
use futures_util::future::BoxFuture;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// A hidden window unused this long is closed; the profile (and the sign-in) stays.
const IDLE: Duration = Duration::from_secs(600);
/// How long one script may take before the page counts as not answering.
const EVAL: Duration = Duration::from_secs(15);
/// WebView2's defaults in wry, and timers that keep running while the window is hidden, so a reply is read as it
/// comes. Nothing here changes how the browser presents itself to the site.
const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
    --disable-background-timer-throttling --disable-renderer-backgrounding --disable-backgrounding-occluded-windows";

pub fn label(provider_id: &str) -> String {
    format!("web-{provider_id}")
}

/// Whether a window is a web session's (so the main window's own handling leaves it alone).
pub fn is_web_window(label: &str) -> bool {
    label.starts_with("web-")
}

pub struct AppWindows<R: Runtime> {
    app: AppHandle<R>,
    root: PathBuf,
    last_used: Arc<Mutex<HashMap<String, Instant>>>,
}

impl<R: Runtime> AppWindows<R> {
    pub fn new(app: AppHandle<R>, root: PathBuf) -> Arc<AppWindows<R>> {
        let windows = Arc::new(AppWindows { app, root, last_used: Arc::default() });
        // Closes hidden windows nobody has used for a while.
        let reaper = windows.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                reaper.close_idle();
            }
        });
        windows
    }

    fn close_idle(&self) {
        let idle: Vec<String> = self.last_used.lock().unwrap_or_else(|p| p.into_inner()).iter()
            .filter(|(_, at)| at.elapsed() > IDLE).map(|(id, _)| id.clone()).collect();
        for id in idle {
            if let Some(window) = self.app.get_webview_window(&label(&id)) {
                if !window.is_visible().unwrap_or(true) {
                    let _ = window.destroy();
                    self.last_used.lock().unwrap_or_else(|p| p.into_inner()).remove(&id);
                }
            }
        }
    }

    /// The connection's window, opened (hidden, on the site) if it is not.
    fn ensure(&self, provider: &Provider, recipe: &'static Recipe) -> Result<WebviewWindow<R>, String> {
        self.last_used.lock().unwrap_or_else(|p| p.into_inner()).insert(provider.id.clone(), Instant::now());
        if let Some(window) = self.app.get_webview_window(&label(&provider.id)) {
            return Ok(window);
        }
        let profile = provider.web_profile.as_deref().and_then(|p| super::profile_dir(&self.root, p))
            .ok_or("This connection has no profile folder")?;
        std::fs::create_dir_all(&profile).map_err(|e| format!("Could not create the profile folder: {e}"))?;
        let url = recipe.site.parse().map_err(|_| "The site's address is not valid")?;
        WebviewWindowBuilder::new(&self.app, label(&provider.id), WebviewUrl::External(url))
            .title(format!("{} — web session (unofficial), Pygg", provider.label))
            .inner_size(1100.0, 820.0)
            .visible(false)
            .data_directory(profile)
            .additional_browser_args(BROWSER_ARGS)
            // Only the site and its sign-in hosts; a blocked address is not loaded, and nothing is logged about it.
            .on_navigation(move |url| host_allowed(recipe, url.as_str()))
            .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
            .build()
            .map_err(|e| format!("Could not open the window: {e}"))
    }
}

impl<R: Runtime> Windows for AppWindows<R> {
    fn page(&self, provider: &Provider, recipe: &'static Recipe) -> Result<Arc<dyn Page>, WebError> {
        let window = self.ensure(provider, recipe).map_err(|_| WebError::Unavailable(NO_WINDOW))?;
        Ok(Arc::new(WindowPage { window }))
    }

    fn show(&self, provider: &Provider, recipe: &'static Recipe) -> Result<(), String> {
        let window = self.ensure(provider, recipe)?;
        window.show().map_err(|e| e.to_string())?;
        let _ = window.unminimize();
        let _ = window.set_focus();
        Ok(())
    }

    fn forget(&self, provider_id: &str, profile: &str) {
        self.last_used.lock().unwrap_or_else(|p| p.into_inner()).remove(provider_id);
        if let Some(window) = self.app.get_webview_window(&label(provider_id)) {
            let _ = window.destroy();
        }
        // WebView2 lets go of the folder shortly after its window is gone; what is still held is deleted at the next
        // start, before any window opens (`sweep`).
        let root = self.root.clone();
        let profile = profile.to_string();
        std::thread::spawn(move || { super::remove_profile(&root, &profile); });
    }
}

struct WindowPage<R: Runtime> {
    window: WebviewWindow<R>,
}

impl<R: Runtime> Page for WindowPage<R> {
    fn navigate(&self, url: &str) -> BoxFuture<'_, Result<(), WebError>> {
        let result = url.parse().map_err(|_| WebError::Unavailable(NO_WINDOW))
            .and_then(|url| self.window.navigate(url).map_err(|_| WebError::Unavailable(NO_WINDOW)));
        Box::pin(async move { result })
    }

    fn eval(&self, script: String) -> BoxFuture<'_, Result<Value, WebError>> {
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        let tx = Mutex::new(Some(tx));
        let sent = self.window.eval_with_callback(script, move |result| {
            if let Some(tx) = tx.lock().unwrap_or_else(|p| p.into_inner()).take() {
                let _ = tx.send(result);
            }
        });
        Box::pin(async move {
            sent.map_err(|_| WebError::Unavailable(NO_WINDOW))?;
            let text = tokio::time::timeout(EVAL, rx).await
                .map_err(|_| WebError::PageChanged(super::NO_PROMPT_BOX))?
                .map_err(|_| WebError::Unavailable(NO_WINDOW))?;
            Ok(serde_json::from_str(&text).unwrap_or(Value::Null))
        })
    }
}
