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
use std::collections::{HashMap, HashSet};
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
    /// Windows the user has open to sign in (ADR-035, 2026-11-13): while they drive it, the window goes anywhere on
    /// https, like a browser, since sign-in passes through hosts no recipe can list. Pygg's own typing is held to the
    /// site's page either way (`fill_script`, `send_script`).
    signing_in: Arc<Mutex<HashSet<String>>>,
    /// What each connection's page received (sounds, documents, the site's API answers), until a reply takes it.
    received: Arc<Mutex<HashMap<String, Vec<super::Received>>>>,
    /// Every response each page received, as `type from host`, until a reply takes it (for the app log).
    seen: Arc<Mutex<HashMap<String, Vec<String>>>>,
}

impl<R: Runtime> AppWindows<R> {
    pub fn new(app: AppHandle<R>, root: PathBuf) -> Arc<AppWindows<R>> {
        let windows = Arc::new(AppWindows {
            app,
            root,
            last_used: Arc::default(),
            signing_in: Arc::default(),
            received: Arc::default(),
            seen: Arc::default(),
        });
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
        let idle: Vec<String> = self
            .last_used
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .filter(|(_, at)| at.elapsed() > IDLE)
            .map(|(id, _)| id.clone())
            .collect();
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
        let profile = provider
            .web_profile
            .as_deref()
            .and_then(|p| super::profile_dir(&self.root, p))
            .ok_or("This connection has no profile folder")?;
        std::fs::create_dir_all(&profile).map_err(|e| format!("Could not create the profile folder: {e}"))?;
        let url = recipe.site.parse().map_err(|_| "The site's address is not valid")?;
        let (app, provider_id) = (self.app.clone(), provider.id.clone());
        let signing_in = self.signing_in.clone();
        let (app2, provider_id2) = (self.app.clone(), provider.id.clone());
        // Debug builds only, and only when asked: DevTools for the web window, to see why a page does not load.
        let args = if cfg!(debug_assertions) && std::env::var_os("PYGG_WEB_DEVTOOLS").is_some() {
            format!("{BROWSER_ARGS} --remote-debugging-port=9462")
        } else {
            BROWSER_ARGS.to_string()
        };
        let window = WebviewWindowBuilder::new(&self.app, label(&provider.id), WebviewUrl::External(url))
            .title(format!("{} — web session (unofficial), Pygg", provider.label))
            .inner_size(1100.0, 820.0)
            .visible(false)
            .data_directory(profile)
            .additional_browser_args(&args)
            // Only the site and its sign-in hosts; a blocked address is not loaded. Its host (only) is kept on the connection.
            .on_navigation(move |url| {
                let user_driving = signing_in.lock().unwrap_or_else(|p| p.into_inner()).contains(&provider_id);
                let allowed = if user_driving {
                    url.scheme() == "https" || url.as_str() == "about:blank"
                } else {
                    host_allowed(recipe, url.as_str())
                };
                // The host it refused, kept on the connection, so a stalled sign-in says why (nothing else of the address).
                if !allowed && url.scheme() != "about" {
                    // An https address by its host; any other kind (http, data, a custom scheme) by its scheme only.
                    let what = if url.scheme() == "https" {
                        url.host_str().unwrap_or("").to_string()
                    } else {
                        format!("a {}: address", url.scheme())
                    };
                    if let Some(state) = app.try_state::<crate::commands::AppState>() {
                        let _ =
                            state.shared.with_db(|db| crate::gateway::store::set_web_blocked(db, &provider_id, &what));
                    }
                }
                // Debug builds, when asked: which addresses the window was refused (the address only, nothing from the page).
                if cfg!(debug_assertions) && std::env::var_os("PYGG_WEB_DEVTOOLS").is_some() {
                    eprintln!(
                        "web-session navigation {} -> {}",
                        url.as_str(),
                        if allowed { "allowed" } else { "blocked" }
                    );
                }
                allowed
            })
            // No popups: a site that signs in through one is told so on the connection (host only), as for a refused address.
            .on_new_window(move |url, _| {
                if cfg!(debug_assertions) && std::env::var_os("PYGG_WEB_DEVTOOLS").is_some() {
                    eprintln!("web-session popup {} -> denied", url.as_str());
                }
                if let Some(state) = app2.try_state::<crate::commands::AppState>() {
                    let what = format!("a popup window ({})", url.host_str().unwrap_or(url.scheme()));
                    let _ = state.shared.with_db(|db| crate::gateway::store::set_web_blocked(db, &provider_id2, &what));
                }
                tauri::webview::NewWindowResponse::Deny
            })
            .build()
            .map_err(|e| format!("Could not open the window: {e}"))?;
        #[cfg(windows)]
        watch_responses(&window, self.received.clone(), self.seen.clone(), provider.id.clone(), recipe.site);
        // Closing the window (it hides) ends the user's turn: back to the site's hosts only.
        let (signing_in, id) = (self.signing_in.clone(), provider.id.clone());
        window.on_window_event(move |event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed) {
                signing_in.lock().unwrap_or_else(|p| p.into_inner()).remove(&id);
            }
        });
        Ok(window)
    }
}

impl<R: Runtime> Windows for AppWindows<R> {
    fn page(&self, provider: &Provider, recipe: &'static Recipe) -> Result<Arc<dyn Page>, WebError> {
        let window = self.ensure(provider, recipe).map_err(|_| WebError::Unavailable(NO_WINDOW))?;
        Ok(Arc::new(WindowPage {
            window,
            provider_id: provider.id.clone(),
            received: self.received.clone(),
            seen: self.seen.clone(),
        }))
    }

    fn show(&self, provider: &Provider, recipe: &'static Recipe) -> Result<(), String> {
        self.signing_in.lock().unwrap_or_else(|p| p.into_inner()).insert(provider.id.clone());
        let window = self.ensure(provider, recipe)?;
        window.show().map_err(|e| e.to_string())?;
        let _ = window.unminimize();
        let _ = window.set_focus();
        Ok(())
    }

    fn signed_in(&self, provider_id: &str) {
        // Only a window the user opened to sign in: back to the site's hosts only, and out of sight.
        if self.signing_in.lock().unwrap_or_else(|p| p.into_inner()).remove(provider_id) {
            if let Some(window) = self.app.get_webview_window(&label(provider_id)) {
                let _ = window.hide();
            }
        }
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
        std::thread::spawn(move || {
            super::remove_profile(&root, &profile);
        });
    }
}

struct WindowPage<R: Runtime> {
    window: WebviewWindow<R>,
    provider_id: String,
    received: Arc<Mutex<HashMap<String, Vec<super::Received>>>>,
    seen: Arc<Mutex<HashMap<String, Vec<String>>>>,
}

/// How many responses a connection keeps until its reply takes them, and the most one may be.
const KEPT: usize = 24;
const KEPT_BYTES: usize = 100 * 1024 * 1024;

/// Reads what the page receives (2026-11-18, the owner's call for another way than the page's markup): WebView2 says
/// when each response arrives, and a sound, a document or the site's own API answer is read and kept for the reply
/// (`super::worth_keeping`). Only responses: no request is changed or made, and no cookie or header is kept.
#[cfg(windows)]
fn watch_responses<R: Runtime>(
    window: &WebviewWindow<R>, received: Arc<Mutex<HashMap<String, Vec<super::Received>>>>,
    seen: Arc<Mutex<HashMap<String, Vec<String>>>>, id: String, site: &'static str,
) {
    let site_host = site.trim_start_matches("https://").trim_end_matches('/').to_string();
    let _ = window.with_webview(move |webview| unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            ICoreWebView2WebResourceResponseReceivedEventArgs, ICoreWebView2_2,
        };
        use windows_core::Interface;
        let Ok(core) = webview.controller().CoreWebView2() else { return };
        let Ok(core) = core.cast::<ICoreWebView2_2>() else { return };
        let handler = webview2_com::WebResourceResponseReceivedEventHandler::create(Box::new(
            move |_sender, args: Option<ICoreWebView2WebResourceResponseReceivedEventArgs>| {
                let Some(args) = args else { return Ok(()) };
                let mut uri = windows_core::PWSTR::null();
                args.Request()?.Uri(&mut uri)?;
                let uri = webview2_com::take_pwstr(uri);
                let host = uri
                    .split("://")
                    .nth(1)
                    .and_then(|r| r.split(['/', '?', '#', ':']).next())
                    .unwrap_or("")
                    .to_string();
                let response = args.Response()?;
                let mut mime = windows_core::PWSTR::null();
                let _ = response.Headers()?.GetHeader(windows_core::w!("Content-Type"), &mut mime);
                let mime = webview2_com::take_pwstr(mime);
                {
                    let mut all = seen.lock().unwrap_or_else(|p| p.into_inner());
                    let list = all.entry(id.clone()).or_default();
                    if list.len() < 400 {
                        let kind = mime.split(';').next().unwrap_or("").trim();
                        list.push(format!("{} from {host}", if kind.is_empty() { "(no type)" } else { kind }));
                    }
                }
                if !super::worth_keeping(&mime, &host, &site_host) {
                    return Ok(());
                }
                let (received, id) = (received.clone(), id.clone());
                let done = webview2_com::WebResourceResponseViewGetContentCompletedHandler::create(Box::new(
                    move |result, stream: Option<windows::Win32::System::Com::IStream>| {
                        let Some(stream) = stream.filter(|_| result.is_ok()) else { return Ok(()) };
                        let mut body = Vec::new();
                        let mut chunk = vec![0u8; 64 * 1024];
                        loop {
                            let mut read = 0u32;
                            let hr = stream.Read(chunk.as_mut_ptr() as *mut _, chunk.len() as u32, Some(&mut read));
                            if hr.is_err() || read == 0 {
                                break;
                            }
                            body.extend_from_slice(&chunk[..read as usize]);
                            if body.len() > KEPT_BYTES {
                                return Ok(());
                            }
                        }
                        let mut all = received.lock().unwrap_or_else(|p| p.into_inner());
                        let list = all.entry(id.clone()).or_default();
                        if list.len() >= KEPT {
                            list.remove(0);
                        }
                        list.push(super::Received { mime: mime.clone(), host: host.clone(), body });
                        Ok(())
                    },
                ));
                let _ = response.GetContent(&done);
                Ok(())
            },
        ));
        let mut token = Default::default();
        let _ = core.add_WebResourceResponseReceived(&handler, &mut token);
    });
}

impl<R: Runtime> Page for WindowPage<R> {
    fn network(&self) -> Vec<super::Received> {
        self.received.lock().unwrap_or_else(|p| p.into_inner()).remove(&self.provider_id).unwrap_or_default()
    }

    fn network_seen(&self) -> Vec<String> {
        self.seen.lock().unwrap_or_else(|p| p.into_inner()).remove(&self.provider_id).unwrap_or_default()
    }

    fn navigate(&self, url: &str) -> BoxFuture<'_, Result<(), WebError>> {
        let result = url
            .parse()
            .map_err(|_| WebError::Unavailable(NO_WINDOW))
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
            let text = tokio::time::timeout(EVAL, rx)
                .await
                .map_err(|_| WebError::PageChanged(super::NO_PROMPT_BOX))?
                .map_err(|_| WebError::Unavailable(NO_WINDOW))?;
            Ok(serde_json::from_str(&text).unwrap_or(Value::Null))
        })
    }
}
