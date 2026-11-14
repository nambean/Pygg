//! Connecting with the browser, where the provider itself offers it (ADR-034): the user signs in on the provider's own
//! page, and the provider issues an ordinary API key for its API, which Pygg keeps in the credential store like a typed
//! one. Not a session or cookie bridge (ADR-026): what Pygg holds is the key the provider's API documents.
//!
//! OpenRouter (PKCE, openrouter.ai/docs/use-cases/oauth-pkce): open `https://openrouter.ai/auth` with a callback on
//! `http://localhost:<port>`, a S256 code challenge and a state; the browser comes back with a one-time code (valid ten
//! minutes), exchanged with the code verifier at `POST /api/v1/auth/keys` for `{ "key": … }`.

use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub const OPENROUTER_AUTH: &str = "https://openrouter.ai/auth";
pub const OPENROUTER_KEYS: &str = "https://openrouter.ai/api/v1/auth/keys";
/// How long Pygg waits for the user to finish signing in.
pub const WAIT: Duration = Duration::from_secs(300);

/// Whether a provider kind can be connected with the browser.
pub fn supports(kind: &str) -> bool {
    kind == "openrouter"
}

/// Base64url without padding (RFC 4648 §5), for the PKCE challenge.
fn b64url(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len() * 4 / 3 + 2);
    for chunk in bytes.chunks(3) {
        let n =
            (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..(chunk.len() + 1) {
            out.push(ABC[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

/// The S256 code challenge of a verifier (RFC 7636).
pub fn challenge(verifier: &str) -> String {
    b64url(&Sha256::digest(verifier.as_bytes()))
}

/// A fresh verifier or state: 64 characters from the unreserved set.
fn random_token() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub fn auth_url(port: u16, challenge: &str, state: &str) -> String {
    format!("{OPENROUTER_AUTH}?callback_url={}&code_challenge={challenge}&code_challenge_method=S256&state={state}&key_label=Pygg",
        encode(&format!("http://localhost:{port}/callback")))
}

/// Opens a URL in the user's default browser.
fn open_browser(url: &str) -> Result<(), String> {
    let mut command = if cfg!(windows) {
        let mut c = std::process::Command::new("rundll32");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg(url);
        c
    } else {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };
    command.spawn().map(|_| ()).map_err(|e| format!("Could not open the browser: {e}"))
}

/// Waits for the browser's request to `/callback?code=…&state=…`, answers it with a page saying the tab can close,
/// and returns the code. A request with another state, an error, or none within `wait` fails.
pub async fn wait_for_code(listener: TcpListener, state: &str, wait: Duration) -> Result<String, String> {
    let accept = async {
        loop {
            let (mut socket, _) = listener.accept().await.map_err(|e| e.to_string())?;
            let mut buf = vec![0u8; 8192];
            let n = socket.read(&mut buf).await.map_err(|e| e.to_string())?;
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            let target = request.split_whitespace().nth(1).unwrap_or("").to_string();
            if !target.starts_with("/callback") {
                let _ =
                    socket.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                continue;
            }
            let query = target.split_once('?').map(|(_, q)| q).unwrap_or("");
            let param = |name: &str| {
                query
                    .split('&')
                    .find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == name).map(|(_, v)| v.to_string()))
            };
            let (body, result) = match (param("code"), param("state")) {
                (Some(code), Some(got)) if got == state && !code.is_empty() => {
                    ("Pygg is connected. You can close this tab.", Ok(code))
                }
                (_, Some(got)) if got != state => (
                    "This sign-in was not started by Pygg.",
                    Err("The sign-in answered with another state".to_string()),
                ),
                _ => (
                    "The sign-in did not finish. You can close this tab and try again in Pygg.",
                    Err(param("error")
                        .map(|e| format!("The provider answered: {e}"))
                        .unwrap_or_else(|| "The sign-in did not finish".into())),
                ),
            };
            let html = format!("<!doctype html><meta charset=utf-8><title>Pygg</title><body style=\"font-family:system-ui;padding:40px\">{body}</body>");
            let _ = socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",
                html.len()).as_bytes()).await;
            return result;
        }
    };
    tokio::time::timeout(wait, accept).await.map_err(|_| "No sign-in came back within five minutes".to_string())?
}

/// Exchanges the one-time code for the key.
pub async fn exchange(http: &reqwest::Client, url: &str, code: &str, verifier: &str) -> Result<String, String> {
    let response = http
        .post(url)
        .json(&serde_json::json!({"code": code, "code_verifier": verifier, "code_challenge_method": "S256"}))
        .send()
        .await
        .map_err(|e| format!("Could not reach the provider: {e}"))?;
    let status = response.status();
    let body: serde_json::Value = response.json().await.unwrap_or_default();
    if !status.is_success() {
        let message = body["error"]["message"].as_str().or(body["message"].as_str()).unwrap_or("no detail");
        return Err(format!("The provider refused the sign-in (HTTP {}): {message}", status.as_u16()));
    }
    body["key"]
        .as_str()
        .filter(|k| !k.is_empty())
        .map(String::from)
        .ok_or_else(|| "The provider sent no key".to_string())
}

/// The whole OpenRouter flow: a local callback, the browser, the user's sign-in, the key.
pub async fn openrouter_key(http: &reqwest::Client) -> Result<String, String> {
    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let (verifier, state) = (random_token(), random_token());
    open_browser(&auth_url(port, &challenge(&verifier), &state))?;
    let code = wait_for_code(listener, &state, WAIT).await?;
    exchange(http, OPENROUTER_KEYS, &code, &verifier).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_matches_rfc_7636() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert_eq!(b64url(b"f"), "Zg");
        assert_eq!(b64url(b"fo"), "Zm8");
        assert_eq!(b64url(b"foo"), "Zm9v");
        let url = auth_url(51423, "abc", "st");
        assert!(url.starts_with(
            "https://openrouter.ai/auth?callback_url=http%3A%2F%2Flocalhost%3A51423%2Fcallback&code_challenge=abc"
        ));
        assert!(random_token().len() == 64);
    }

    #[test]
    fn the_callback_gives_the_code_only_for_our_state() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let waiting = tokio::spawn(wait_for_code(listener, "good", Duration::from_secs(5)));
            let page = reqwest::get(format!("http://127.0.0.1:{port}/callback?code=c0de&state=good"))
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            assert!(page.contains("You can close this tab"));
            assert_eq!(waiting.await.unwrap(), Ok("c0de".to_string()));

            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let waiting = tokio::spawn(wait_for_code(listener, "good", Duration::from_secs(5)));
            reqwest::get(format!("http://127.0.0.1:{port}/callback?code=c0de&state=forged")).await.unwrap();
            assert!(waiting.await.unwrap().unwrap_err().contains("another state"));
        });
    }

    #[test]
    fn the_code_is_exchanged_for_the_key() {
        use axum::{routing::post, Json, Router};
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let app = Router::new().route(
                "/keys",
                post(|Json(body): Json<serde_json::Value>| async move {
                    assert_eq!(
                        (body["code"].as_str(), body["code_challenge_method"].as_str()),
                        (Some("c0de"), Some("S256"))
                    );
                    assert!(body["code_verifier"].as_str().is_some());
                    Json(serde_json::json!({"key": "sk-or-v1-test"}))
                }),
            );
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
            let key =
                exchange(&reqwest::Client::new(), &format!("http://127.0.0.1:{port}/keys"), "c0de", "v").await.unwrap();
            assert_eq!(key, "sk-or-v1-test");
        });
    }
}
