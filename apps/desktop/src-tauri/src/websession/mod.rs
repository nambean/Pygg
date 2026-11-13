//! Web-session connections (ADR-035): an AI site with no API, used through a Pygg window the user signs in to.
//!
//! - **The window** (`window.rs`, the app only) is a Tauri webview with its own WebView2 profile folder under
//!   `websessions/<profile>`. The user signs in there themselves; the profile keeps the session across restarts, as
//!   a browser does. Pygg never asks for a password, never imports cookies, and its scripts never read cookies or
//!   storage: they fill the prompt box, click Send, and read the reply's text.
//! - **The recipe** (`RECIPES`) says where the site's prompt box, Send button, reply and "still answering" signal are,
//!   and how a sign-in page, a challenge (CAPTCHA, bot check) or a request for approval looks.
//! - **The endpoint** (`router`) serves each connection as a Chat Completions endpoint on loopback, behind a token that
//!   lives only in memory, so the gateway reaches it as an ordinary OpenAI-format provider: translators, limits,
//!   health, combos and the ledger apply unchanged (`gateway::forward`).
//! - **Safeguards:** one request at a time per connection with a pause between requests; tools refused; never chosen
//!   by `auto`; cost unknown, never $0; a challenge stops the connection and tells the user to open its window; Muse's
//!   requests for approval are denied, so the agent never acts for the user through Pygg; nothing from the page but
//!   the reply leaves the window, and errors carry Pygg's own wording only.

pub mod window;

use crate::gateway::store::{self, Provider};
use axum::body::Body;
use axum::extract::{Path as UrlPath, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Router;
use bytes::Bytes;
use futures_util::future::BoxFuture;
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The text of the account-risk warning the user acknowledges per connection. A change to the warning's meaning
/// raises it, and connections acknowledged under an older text are not routed to until acknowledged again.
pub const WARNING_VERSION: i64 = 1;

/// Where the facts of a web session's models come from (`caps_source`).
pub const CAPS_SOURCE: &str = "web session: text only, no tools or images (ADR-035)";

/// Why `auto` never picks a web session.
pub const NOT_FOR_AUTO: &str = "web session (unofficial): used only when named or in a combo";

/// Why a request with tools is not sent to a web session.
pub const NO_TOOLS: &str = "a web session cannot call tools";

pub const STATES: [&str; 4] = ["signed_in", "signed_out", "needs_you", "page_changed"];

// Pygg's own wording for what the window showed. Never the page's text: that can hold the user's data.
pub const NOT_SIGNED_IN: &str = "Not signed in yet. Open the sign-in window and sign in on the site.";
pub const SIGNED_OUT: &str = "Signed out. Open the sign-in window and sign in again.";
/// The page Pygg was about to type into is not the site's own (ADR-035, 2026-11-13): it stops before sending anything.
pub const OFF_SITE: &str = "The window is not on the site's own page, so Pygg sent nothing. Open the window and go back to the site.";
/// Requests a web session sends to its site per UTC day unless the user sets another limit: a person's pace, not a
/// script's. Bursts of automated requests are what get accounts of tools like 9router flagged (owner, 2026-11-13).
pub const DEFAULT_DAILY_LIMIT: u32 = 50;
/// The rest between two requests on one account, after the reply ends: a person's pace (owner, 2026-11-13: rotation
/// "must avoid sending requests continuously"). Fixed, not randomised: Pygg slows down, it does not disguise itself.
pub const PACE_SECS: u64 = 10;
pub const CHALLENGE: &str = "The site is showing a check (a CAPTCHA, a bot check or a sign-in challenge). Open the window and deal with it yourself; Pygg does not answer such checks.";
pub const SIGNED_IN: &str = "Signed in.";
pub const NO_PROMPT_BOX: &str = "The page has changed: Pygg could not find its prompt box.";
pub const NO_SEND_BUTTON: &str = "The page has changed: Pygg could not find its Send button.";
pub const NO_REPLY: &str = "No reply appeared in time.";
pub const REPLY_TIMEOUT: &str = "The reply did not finish in time.";
pub const QUOTA: &str = "The site says this account has reached its usage limit.";
pub const NO_WINDOW: &str = "The site's window could not be opened.";
pub const APP_ONLY: &str = "a web session needs the Pygg app, which drives its window";
pub const NOT_ACKNOWLEDGED: &str = "its account-risk warning has not been acknowledged";

/// What Pygg knows about driving one site. Selectors are CSS; patterns are case-insensitive regular expressions in the
/// page's own JavaScript. Muse's selectors follow 9router's `muse-web` executor (MIT, see THIRD_PARTY_NOTICES.md).
pub struct Recipe {
    /// The preset this recipe serves.
    pub preset: &'static str,
    pub site: &'static str,
    /// Opened for every request, so requests never share a conversation.
    pub new_chat_url: &'static str,
    /// Hosts the window may navigate to (and their subdomains): the site and where it signs people in.
    pub hosts: &'static [&'static str],
    pub models: &'static [&'static str],
    pub input: &'static str,
    /// Matched against a visible button's `aria-label` and `data-testid`.
    pub send_pattern: &'static str,
    /// Without such a button, Enter in the prompt box sends (as typing would).
    pub enter_sends: bool,
    /// Each reply of the site's assistant; the last one is the answer.
    pub reply: &'static str,
    /// Present while the site is still answering. Without one, a reply counts as finished only after it has not
    /// changed for longer (`Timing::stable_busy`).
    pub busy: Option<&'static str>,
    /// A request to approve an action the site's agent wants to take, and the label of its "deny" button.
    pub approval: Option<&'static str>,
    pub deny_pattern: &'static str,
    /// The page has finished starting up when this attribute has this value (if the element exists).
    pub ready_attr: Option<(&'static str, &'static str, &'static str)>,
    /// Words of a page that wants the user to sign in, in lower case.
    pub sign_in_hints: &'static [&'static str],
    /// The exact text of buttons a page shows only to someone signed out, in lower case. For sites that also chat with
    /// visitors (ChatGPT, Perplexity, Grok): their prompt box is there either way, and a visitor's chat is not the
    /// user's account.
    pub signed_out_buttons: &'static [&'static str],
    /// The site's assistant can act for the user (send mail, buy), so the warning says so (Muse).
    pub acts: bool,
    /// A CAPTCHA, a bot check or a sign-in challenge: selectors, and words in lower case.
    pub challenge_selectors: &'static str,
    pub challenge_hints: &'static [&'static str],
    /// Words of a finished reply that says the account reached its limit, in lower case.
    pub quota_hints: &'static [&'static str],
}

/// A check the site runs on the browser: a CAPTCHA, Cloudflare's "Just a moment" page or Turnstile, or a bot check.
const CHALLENGE_SELECTORS: &str = "iframe[src*=\"captcha\" i], iframe[src*=\"challenges.cloudflare.com\"], iframe[title*=\"challenge\" i], \
    [id*=\"captcha\" i], #challenge-form, #challenge-stage, .cf-turnstile";
const CHALLENGE_HINTS: &[&str] = &["verify you're human", "verify you are human", "verifying you are human", "are you a robot",
    "security check", "checking your browser", "performing security verification", "unusual activity", "suspicious activity"];
const QUOTA_HINTS: &[&str] = &["out of credits", "usage limit", "limit reached", "token limit", "reached your limit", "rate limit"];
/// Google's sign-in, which several sites offer. Google may refuse to sign in inside an embedded window ("This browser
/// or app may not be secure"); Pygg does not hide what the window is, so the site's own email sign-in is then the way.
const GOOGLE: [&str; 2] = ["google.com", "youtube.com"];

/// Sites and their sign-in hosts. Muse's hosts are from its live redirect chain (2026-11-12). The others are the sites'
/// documented sign-in paths, and are checked against the live chain when the owner signs in with the debug navigation
/// log (`PYGG_WEB_DEVTOOLS=1`); their selectors were read from each site's public page where it has one, and follow
/// what 9router reads otherwise (docs/research/2026-11-12-web-session-routers.md).
pub const RECIPES: &[Recipe] = &[
    Recipe {
        preset: "muse-web",
        site: "https://muse.ai",
        new_chat_url: "https://muse.ai/thread/new",
        // Meta's sign-in checks for an existing account by passing through each of its sites in turn ("aymh" redirect
        // cycle: facebook.com, then instagram.com); blocking one of them aborts the sign-in and leaves the window blank.
        hosts: &["muse.ai", "meta.com", "meta.ai", "facebook.com", "instagram.com", "threads.com", "threads.net", "messenger.com"],
        models: &["muse"],
        input: "textarea",
        send_pattern: "send",
        enter_sends: false,
        reply: "div[class*=\"hatch-agent-bubble-bg\"]",
        busy: Some("button[aria-label*=\"Stop\" i]"),
        approval: Some("[data-testid=\"hatch-inline-approval-card\"]"),
        deny_pattern: "^(deny|decline|reject|don.t allow)$",
        ready_attr: Some(("[data-hatch-shell-hydration-state]", "data-hatch-shell-hydration-state", "hydrated")),
        sign_in_hints: &["log in", "sign in", "create an account", "use another account"],
        signed_out_buttons: &[],
        acts: true,
        challenge_selectors: CHALLENGE_SELECTORS,
        challenge_hints: CHALLENGE_HINTS,
        quota_hints: QUOTA_HINTS,
    },
    // ChatGPT chats with visitors too, so "signed in" needs its Log in buttons gone. Sign-in: auth.openai.com (email,
    // phone), Google, Apple, Microsoft. Public page, 2026-11-12: `textarea[name="prompt"]`, a "Send message" button.
    Recipe {
        preset: "chatgpt-web",
        site: "https://chatgpt.com",
        new_chat_url: "https://chatgpt.com/",
        hosts: &["chatgpt.com", "openai.com", GOOGLE[0], GOOGLE[1], "apple.com", "live.com", "microsoftonline.com", "microsoft.com"],
        models: &["chatgpt"],
        // Signed in (2026-11-13, live): a ProseMirror `div[contenteditable]` with no id; signed out, `#prompt-textarea`.
        input: "#prompt-textarea, div.ProseMirror[contenteditable=\"true\"], textarea[name=\"prompt\"]",
        send_pattern: "send",
        enter_sends: true,
        // 2026-11-13, live: a reply is a screen-reader heading `[data-conversation-role=assistant]` ("ChatGPT said:")
        // followed by its body; older pages marked the body `[data-message-author-role=assistant]`.
        reply: "[data-message-author-role=\"assistant\"], [data-conversation-role=\"assistant\"] + *",
        busy: Some("button[data-testid=\"stop-button\"], button[aria-label*=\"Stop\" i]"),
        approval: None,
        deny_pattern: "^(deny|decline|cancel)$",
        ready_attr: None,
        sign_in_hints: &["log in", "sign up", "welcome back"],
        signed_out_buttons: &["log in", "sign up for free", "sign up"],
        acts: false,
        challenge_selectors: CHALLENGE_SELECTORS,
        challenge_hints: CHALLENGE_HINTS,
        quota_hints: QUOTA_HINTS,
    },
    // Perplexity answers visitors too. Sign-in: an emailed code on perplexity.ai, Google, Apple. Public page,
    // 2026-11-12: a Lexical `div[contenteditable][role=textbox]`, a "Submit" button. Its "Computer" mode is an agent;
    // a new chat opens in its search mode.
    Recipe {
        preset: "perplexity-web",
        site: "https://www.perplexity.ai",
        new_chat_url: "https://www.perplexity.ai/",
        hosts: &["perplexity.ai", GOOGLE[0], GOOGLE[1], "apple.com"],
        models: &["perplexity"],
        input: "div[contenteditable=\"true\"][role=\"textbox\"], #ask-input, textarea",
        send_pattern: "submit|send",
        enter_sends: true,
        reply: "[id^=\"markdown-content-\"], .prose",
        busy: Some("button[aria-label*=\"Stop\" i]"),
        approval: None,
        deny_pattern: "^(deny|decline|cancel)$",
        ready_attr: None,
        sign_in_hints: &["sign in", "log in", "continue with email"],
        signed_out_buttons: &["sign in", "log in", "sign up"],
        acts: false,
        challenge_selectors: CHALLENGE_SELECTORS,
        challenge_hints: CHALLENGE_HINTS,
        quota_hints: QUOTA_HINTS,
    },
    // DeepSeek needs an account: chat.deepseek.com sends a visitor to /sign_in (email or phone with a password,
    // Google, Apple). Its Send control has no label, so Enter sends; it shows no reliable "stop" control either.
    Recipe {
        preset: "deepseek-web",
        site: "https://chat.deepseek.com",
        new_chat_url: "https://chat.deepseek.com/",
        hosts: &["chat.deepseek.com", "deepseek.com", GOOGLE[0], GOOGLE[1], "apple.com"],
        models: &["deepseek"],
        input: "textarea#chat-input, textarea",
        send_pattern: "send",
        enter_sends: true,
        reply: ".ds-markdown",
        busy: None,
        approval: None,
        deny_pattern: "^(deny|decline|cancel)$",
        ready_attr: None,
        sign_in_hints: &["log in", "sign up", "forgot your"],
        signed_out_buttons: &[],
        acts: false,
        challenge_selectors: CHALLENGE_SELECTORS,
        challenge_hints: CHALLENGE_HINTS,
        quota_hints: QUOTA_HINTS,
    },
    // Grok sits behind Cloudflare: on 2026-11-12 even its sign-in page answered with "Just a moment" in a normal
    // browser. That check stops the connection until the user clears it in the window; Pygg never answers it. Sign-in:
    // accounts.x.ai (email), X, Google, Apple. Grok chats with visitors too.
    Recipe {
        preset: "grok-web",
        site: "https://grok.com",
        new_chat_url: "https://grok.com/",
        // 2026-11-13, live: after its OAuth callback Grok sets its cookie on auth.grok.com and then auth.grokusercontent.com.
        hosts: &["grok.com", "grokusercontent.com", "x.ai", "x.com", "twitter.com", GOOGLE[0], GOOGLE[1], "apple.com"],
        models: &["grok"],
        input: "textarea, div.ProseMirror[contenteditable=\"true\"]",
        send_pattern: "submit|send",
        enter_sends: true,
        reply: ".response-content-markdown",
        busy: Some("button[aria-label*=\"Stop\" i]"),
        approval: None,
        deny_pattern: "^(deny|decline|cancel)$",
        ready_attr: None,
        sign_in_hints: &["sign in", "sign up", "log in"],
        signed_out_buttons: &["sign in", "sign up"],
        acts: false,
        challenge_selectors: CHALLENGE_SELECTORS,
        challenge_hints: CHALLENGE_HINTS,
        quota_hints: QUOTA_HINTS,
    },
];

pub fn recipe(preset: &str) -> Option<&'static Recipe> {
    RECIPES.iter().find(|r| r.preset == preset)
}

/// Whether the window may go to `url`: HTTPS on one of the recipe's hosts or their subdomains. Anything else is
/// blocked (the site's own links to elsewhere included), so the profile only ever holds the site's sign-in.
pub fn host_allowed(recipe: &Recipe, url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return url == "about:blank" };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    // No user info, and no port other than the default.
    if authority.contains('@') {
        return false;
    }
    let host = authority.strip_suffix(":443").unwrap_or(authority).to_ascii_lowercase();
    if host.contains(':') {
        return false;
    }
    recipe.hosts.iter().any(|h| host == *h || host.ends_with(&format!(".{h}")))
        || (recipe.hosts.contains(&GOOGLE[0]) && google_country_sign_in(&host))
}

/// Google's sign-in sets its cookie on the account's country domain too (`accounts.google.com.vn` for an account in
/// Viet Nam, after `accounts.youtube.com`); a blocked hop leaves the sign-in spinning (owner, 2026-11-13). Only that
/// one name on a country suffix: `accounts.google.` and then `xx`, `com.xx` or `co.xx`.
fn google_country_sign_in(host: &str) -> bool {
    let Some(suffix) = host.strip_prefix("accounts.google.") else { return false };
    let country = |s: &str| s.len() == 2 && s.bytes().all(|b| b.is_ascii_lowercase());
    match suffix.split_once('.') {
        None => country(suffix),
        Some((second, cc)) => (second == "com" || second == "co") && country(cc),
    }
}

// ---------- Requests ----------

/// Whether a request uses tools in any of the three API shapes: offered tools, a forced tool choice, or a tool call or
/// result in its history. A web session cannot take them, so such a request is not sent there.
pub fn uses_tools(body: &Value) -> bool {
    if body["tools"].as_array().is_some_and(|t| !t.is_empty()) || body.get("functions").is_some() {
        return true;
    }
    if body["tool_choice"].is_object() || matches!(body["tool_choice"].as_str(), Some("required" | "any" | "tool")) {
        return true;
    }
    const TOOL_ITEMS: [&str; 6] = ["tool_use", "tool_result", "function_call", "function_call_output", "custom_tool_call", "custom_tool_call_output"];
    let item_uses_tools = |item: &Value| {
        item["role"] == "tool" || item["role"] == "function" || item["tool_calls"].as_array().is_some_and(|c| !c.is_empty())
            || item["type"].as_str().is_some_and(|t| TOOL_ITEMS.contains(&t))
            || item["content"].as_array().is_some_and(|parts| parts.iter().any(|p| p["type"].as_str().is_some_and(|t| TOOL_ITEMS.contains(&t))))
    };
    let lists = [&body["messages"], &body["input"]];
    lists.iter().any(|list| list.as_array().is_some_and(|items| items.iter().any(item_uses_tools)))
}

fn text_of(content: &Value) -> Result<String, String> {
    match content {
        Value::Null => Ok(String::new()),
        Value::String(s) => Ok(s.clone()),
        Value::Array(parts) => {
            let mut out = Vec::new();
            for part in parts {
                match part["type"].as_str() {
                    Some("text" | "input_text" | "output_text") => out.extend(part["text"].as_str().map(str::to_string)),
                    Some("image_url" | "input_image" | "image" | "file" | "input_file" | "input_audio") =>
                        return Err("A web session takes text only; this request has an image or a file".into()),
                    _ => {}
                }
            }
            Ok(out.join("\n"))
        }
        _ => Ok(String::new()),
    }
}

/// The one prompt typed into the site's box (a Chat Completions body, which the gateway always sends here). A single
/// user turn goes as it is; anything richer is labelled turn by turn, ending with an instruction to answer the last one.
pub fn flatten(body: &Value) -> Result<String, String> {
    let messages = body["messages"].as_array().ok_or("The request has no messages")?;
    let mut turns: Vec<(&str, String)> = Vec::new();
    for message in messages {
        let role = message["role"].as_str().unwrap_or("user");
        let text = text_of(&message["content"])?.trim().to_string();
        if !text.is_empty() {
            turns.push((role, text));
        }
    }
    match turns.as_slice() {
        [] => Err("The request has no text to send".into()),
        [("user", text)] => Ok(text.clone()),
        _ => {
            let mut blocks: Vec<String> = turns.iter().map(|(role, text)| {
                let label = match *role { "system" | "developer" => "System", "assistant" => "Assistant", "user" => "User", other => other };
                format!("[{label}]\n{text}")
            }).collect();
            blocks.push("[Instruction]\nContinue the conversation above and reply as the Assistant to the last User message. \
                Output only the reply.".into());
            Ok(blocks.join("\n\n"))
        }
    }
}

/// Whether the gateway may send this request to this web-session connection, or why not. The second value says the
/// reason is tools, which makes the gateway's answer a 400 when nothing else could take the request.
pub fn routable(provider: &Provider, body: &Value, app_running: bool) -> Result<(), (&'static str, bool)> {
    if provider.web_ack_version != Some(WARNING_VERSION) {
        return Err((NOT_ACKNOWLEDGED, false));
    }
    if uses_tools(body) {
        return Err((NO_TOOLS, true));
    }
    if !app_running {
        return Err((APP_ONLY, false));
    }
    Ok(())
}

// ---------- Scripts run in the page ----------
// Each is one expression whose value comes back as JSON. They read and fill the page; none of them reads
// `document.cookie`, storage or the network, and only the reply's text leaves the page.

fn q(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// Marks the current document, so a page that has not navigated yet is not taken for the new one.
pub fn mark_script() -> String {
    "(() => { window.__pyggOld = true; return true; })()".into()
}

/// What the page is showing: ready for a prompt, asking the user to sign in, or showing a check.
pub fn state_script(r: &Recipe) -> String {
    let ready_attr = match r.ready_attr {
        Some((selector, attr, value)) => format!(
            "(() => {{ const e = document.querySelector({}); return !e || e.getAttribute({}) === {}; }})()", q(selector), q(attr), q(value)),
        None => "true".into(),
    };
    format!(r#"(() => {{
  const body = document.body ? document.body.innerText.slice(0, 4000).toLowerCase() : '';
  const input = document.querySelector({input});
  const signIn = !input && {sign_in}.some(h => body.includes(h));
  const signedOut = [...document.querySelectorAll('button,a')].some(b => b.offsetParent !== null
    && {signed_out}.includes((b.innerText || '').trim().toLowerCase()));
  const challenge = !!document.querySelector({challenge}) || {challenge_hints}.some(h => body.includes(h));
  return {{ fresh: !window.__pyggOld, ready: document.readyState === 'complete' && !!input && {ready_attr},
           hasInput: !!input, signIn, signedOut, challenge }};
}})()"#,
        input = q(r.input), sign_in = json!(r.sign_in_hints), signed_out = json!(r.signed_out_buttons), challenge = q(r.challenge_selectors),
        challenge_hints = json!(r.challenge_hints), ready_attr = ready_attr)
}

/// The host of the site's own page (`chatgpt.com`, `perplexity.ai`), without `www.`: where Pygg may type.
pub fn site_host(r: &Recipe) -> &'static str {
    let host = r.site.strip_prefix("https://").unwrap_or(r.site);
    let host = host.split(['/', '?', '#']).next().unwrap_or(host);
    host.strip_prefix("www.").unwrap_or(host)
}

/// The JavaScript test that the page is the site's own: its host or a subdomain of it, over https. Sign-in hosts
/// (Google, Meta, X) do not count, so a prompt is never typed into them, or into anything a redirect led to.
fn on_site(r: &Recipe) -> String {
    format!("(location.protocol === 'https:' && (location.hostname === {h} || location.hostname.endsWith('.' + {h})))", h = q(site_host(r)))
}

/// Puts the prompt in the box the way typing would, so the site's own code sees it. Off the site's own page it types
/// nothing and says so.
pub fn fill_script(r: &Recipe, text: &str) -> String {
    format!(r#"(() => {{
  if (!{on_site}) return {{ ok: false, offSite: true }};
  const box = document.querySelector({input});
  if (!box) return {{ ok: false }};
  box.focus();
  if (box instanceof HTMLTextAreaElement || box instanceof HTMLInputElement) {{
    const proto = box instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(proto, 'value').set.call(box, {text});
    box.dispatchEvent(new Event('input', {{ bubbles: true }}));
  }} else {{
    document.execCommand('selectAll', false);
    document.execCommand('insertText', false, {text});
  }}
  return {{ ok: true }};
}})()"#, input = q(r.input), text = q(text), on_site = on_site(r))
}

/// Clicks the visible Send button: `clicked`, `disabled` or `missing`. Where the recipe says Enter sends and there is
/// no such button, presses Enter in the prompt box instead: `entered`.
pub fn send_script(r: &Recipe) -> String {
    format!(r#"(() => {{
  if (!{on_site}) return 'offsite';
  const re = new RegExp({pattern}, 'i');
  const button = [...document.querySelectorAll('button,[role=button]')].filter(b => b.offsetParent !== null)
    .find(b => re.test((b.getAttribute('aria-label') || '') + ' ' + (b.getAttribute('data-testid') || '')));
  if (button) {{
    if (button.disabled || button.getAttribute('aria-disabled') === 'true') return 'disabled';
    button.click();
    return 'clicked';
  }}
  const box = {enter} ? document.querySelector({input}) : null;
  if (!box) return 'missing';
  box.focus();
  for (const type of ['keydown', 'keypress', 'keyup']) {{
    box.dispatchEvent(new KeyboardEvent(type, {{ key: 'Enter', code: 'Enter', keyCode: 13, which: 13, bubbles: true, cancelable: true }}));
  }}
  return 'entered';
}})()"#, pattern = q(r.send_pattern), enter = r.enter_sends, input = q(r.input), on_site = on_site(r))
}

/// The conversation as it stands: how many replies, the last one's text, and the signals around it.
pub fn read_script(r: &Recipe) -> String {
    format!(r#"(() => {{
  const replies = [...document.querySelectorAll({reply})];
  const last = replies.length ? (replies[replies.length - 1].innerText || '').trim() : '';
  const body = document.body ? document.body.innerText.slice(0, 4000).toLowerCase() : '';
  return {{ count: replies.length, text: last, busy: {busy},
           approval: {approval}, challenge: !!document.querySelector({challenge}) || {challenge_hints}.some(h => body.includes(h)) }};
}})()"#, reply = q(r.reply), busy = r.busy.map(|b| format!("!!document.querySelector({})", q(b))).unwrap_or_else(|| "false".into()),
        approval = r.approval.map(|a| format!("!!document.querySelector({})", q(a))).unwrap_or_else(|| "false".into()),
        challenge = q(r.challenge_selectors), challenge_hints = json!(r.challenge_hints))
}

/// Clicks the site's own "deny" on every pending request for approval; returns how many.
pub fn deny_script(r: &Recipe) -> String {
    let Some(approval) = r.approval else { return "0".into() };
    format!(r#"(() => {{
  const re = new RegExp({pattern}, 'i');
  let denied = 0;
  for (const card of document.querySelectorAll({approval})) {{
    const button = [...card.querySelectorAll('button,[role=button]')].find(b => re.test((b.innerText || '').trim()));
    if (button && !button.disabled) {{ button.click(); denied++; }}
  }}
  return denied;
}})()"#, pattern = q(r.deny_pattern), approval = q(approval))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PageState {
    pub fresh: bool,
    pub ready: bool,
    pub has_input: bool,
    pub sign_in: bool,
    /// Buttons only a visitor sees are on the page: the site may chat, but not as the user.
    pub signed_out: bool,
    pub challenge: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ChatState {
    pub count: u64,
    pub text: String,
    pub busy: bool,
    pub approval: bool,
    pub challenge: bool,
}

// ---------- Driving a page ----------

/// What went wrong, in Pygg's words, with the HTTP status the gateway's policy reads (`policy.rs`).
#[derive(Clone, Debug, PartialEq)]
pub enum WebError {
    /// Signed out, or a check the user must deal with: 401, held until the user signs in (the connection's health).
    NeedsYou { state: &'static str, reason: &'static str },
    /// Another request is using the connection: 429 with a wait.
    Busy { retry_after: u64 },
    /// The site says the account reached its limit: 429 as an account out of quota.
    Quota,
    /// The page no longer looks like the recipe: 503, tried again after a short cooldown.
    PageChanged(&'static str),
    /// No first word, or no end, in time; the site may have run the request: 504.
    Timeout(&'static str),
    /// The window could not be used: 503.
    Unavailable(&'static str),
    /// The request itself cannot be sent: 400.
    BadRequest(String),
    /// Pygg's own daily limit for this web session is used: 429 until 00:00 UTC; nothing reached the site.
    DailyLimit { limit: u32, retry_after: u64 },
}

impl WebError {
    pub fn status(&self) -> StatusCode {
        match self {
            WebError::NeedsYou { .. } => StatusCode::UNAUTHORIZED,
            WebError::Busy { .. } | WebError::Quota | WebError::DailyLimit { .. } => StatusCode::TOO_MANY_REQUESTS,
            WebError::PageChanged(_) | WebError::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            WebError::Timeout(_) => StatusCode::GATEWAY_TIMEOUT,
            WebError::BadRequest(_) => StatusCode::BAD_REQUEST,
        }
    }

    pub fn message(&self) -> String {
        match self {
            WebError::NeedsYou { reason, .. } => reason.to_string(),
            WebError::Busy { .. } => "The site is answering another request; a web session takes one at a time.".into(),
            WebError::Quota => QUOTA.into(),
            WebError::PageChanged(why) | WebError::Timeout(why) | WebError::Unavailable(why) => why.to_string(),
            WebError::BadRequest(why) => why.clone(),
            WebError::DailyLimit { limit, .. } => format!("Not sent: this web session reached its daily limit of {limit} \
                requests, which keeps its use at a person's pace. It resets at 00:00 UTC; change it in Pygg → Providers."),
        }
    }

    /// What the connection's state becomes, if this says anything about it.
    pub fn state(&self) -> Option<(&'static str, &'static str)> {
        match self {
            WebError::NeedsYou { state, reason } => Some((state, reason)),
            WebError::PageChanged(why) => Some(("page_changed", why)),
            _ => None,
        }
    }

    fn response(&self) -> Response {
        let kind = match self.status() {
            StatusCode::UNAUTHORIZED => "authentication_error",
            StatusCode::TOO_MANY_REQUESTS => "rate_limit_error",
            StatusCode::BAD_REQUEST => "invalid_request_error",
            _ => "api_error",
        };
        let code = match self { WebError::Quota => Some("insufficient_quota"), WebError::DailyLimit { .. } => Some("daily_limit"), _ => None };
        let mut response = (self.status(), axum::Json(json!({ "error": {
            "message": self.message(), "type": kind, "code": code } }))).into_response();
        if let WebError::Busy { retry_after } | WebError::DailyLimit { retry_after, .. } = self {
            response.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from(*retry_after));
        }
        response
    }
}

/// One site page Pygg can drive: the window's, or a scripted one in tests.
pub trait Page: Send + Sync {
    fn navigate(&self, url: &str) -> BoxFuture<'_, Result<(), WebError>>;
    /// Runs one script and returns its value.
    fn eval(&self, script: String) -> BoxFuture<'_, Result<Value, WebError>>;
}

/// The windows of the web sessions: the app's (`window.rs`), or scripted ones in tests.
pub trait Windows: Send + Sync {
    /// The connection's page, its window opened (hidden) if it is not.
    fn page(&self, provider: &Provider, recipe: &'static Recipe) -> Result<Arc<dyn Page>, WebError>;
    /// Shows the connection's window to the user, opening it on the site if it is not open.
    fn show(&self, provider: &Provider, recipe: &'static Recipe) -> Result<(), String>;
    /// Closes the connection's window and deletes its profile folder (or leaves it for the next start's sweep).
    fn forget(&self, provider_id: &str, profile: &str);
    /// Hides the window the user opened to sign in, once a check finds the session signed in: the sign-in is done, and
    /// Pygg goes on in the background. The window stays open, hidden; nothing is signed out.
    fn signed_in(&self, _provider_id: &str) {}
}

/// How long each step may take. Tests shorten them.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    pub poll: Duration,
    pub ready: Duration,
    pub send: Duration,
    /// How long the first word of a reply may take on a short prompt. A page thinks longer before a long one, so the
    /// wait grows by `first_word_per_100_tokens` for every 100 estimated prompt tokens, up to `first_word_cap`
    /// (`first_word_wait`); and while the page shows it is still answering, the thinking counts as the first word.
    /// The 2026-11-13 web coding benchmark lost 53 of 72 requests on Muse, Grok and Perplexity to a fixed 60 s.
    pub first_word: Duration,
    pub first_word_per_100_tokens: Duration,
    pub first_word_cap: Duration,
    /// How long the whole reply may take on a short prompt; the first-word wait's extra is added to it.
    pub reply: Duration,
    /// Unchanged reads before a reply counts as finished, and the same while the site still shows it is answering.
    pub stable: u32,
    pub stable_busy: u32,
    /// How long a second request waits for the connection, and the pause after each request.
    pub queue: Duration,
    pub gap: Duration,
}

impl Default for Timing {
    fn default() -> Timing {
        Timing { poll: Duration::from_millis(300), ready: Duration::from_secs(45), send: Duration::from_secs(5),
            first_word: Duration::from_secs(60), first_word_per_100_tokens: Duration::from_secs(1),
            first_word_cap: Duration::from_secs(240), reply: Duration::from_secs(300), stable: 4, stable_busy: 12,
            queue: Duration::from_secs(60), gap: Duration::from_secs(PACE_SECS) }
    }
}

impl Timing {
    /// The first-word wait for this prompt: `first_word` plus `first_word_per_100_tokens` for every 100 estimated
    /// tokens of it (the gateway's estimate, which leans high), never above `first_word_cap`.
    pub fn first_word_wait(&self, prompt: &str) -> Duration {
        let hundreds = crate::gateway::estimate_text_tokens(prompt) / 100;
        let extra = self.first_word_per_100_tokens.saturating_mul(hundreds.min(u32::MAX as u64) as u32);
        (self.first_word + extra).min(self.first_word_cap.max(self.first_word))
    }

    /// The whole reply's wait for this prompt: `reply`, extended by what the first-word wait grew.
    pub fn reply_wait(&self, prompt: &str) -> Duration {
        self.reply + self.first_word_wait(prompt).saturating_sub(self.first_word)
    }
}

async fn eval_as<T: for<'de> Deserialize<'de> + Default>(page: &dyn Page, script: String) -> Result<T, WebError> {
    let value = page.eval(script).await?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

/// Opens a new chat and waits until it takes a prompt. A sign-in page or a check stops here.
pub async fn open_chat(page: &dyn Page, recipe: &Recipe, timing: &Timing) -> Result<(), WebError> {
    let _ = page.eval(mark_script()).await;
    page.navigate(recipe.new_chat_url).await?;
    let started = Instant::now();
    let mut last = PageState::default();
    while started.elapsed() < timing.ready {
        tokio::time::sleep(timing.poll).await;
        // A page that is still loading may not answer; that is not an error yet.
        let Ok(state) = eval_as::<PageState>(page, state_script(recipe)).await else { continue };
        if state.fresh && state.challenge {
            return Err(WebError::NeedsYou { state: "needs_you", reason: CHALLENGE });
        }
        if state.fresh && state.signed_out && state.has_input {
            return Err(WebError::NeedsYou { state: "signed_out", reason: SIGNED_OUT });
        }
        if state.fresh && state.ready {
            return Ok(());
        }
        if state.fresh {
            last = state;
        }
    }
    if last.sign_in {
        Err(WebError::NeedsYou { state: "signed_out", reason: SIGNED_OUT })
    } else {
        Err(WebError::PageChanged(NO_PROMPT_BOX))
    }
}

/// A finished reply, and how many of the site's requests for approval were denied on the way.
#[derive(Debug, PartialEq)]
pub struct Reply {
    pub text: String,
    pub denied: u64,
}

/// Sends one prompt in a new chat and reads the reply as it appears, giving each new piece to `on_text`. The reply is
/// finished when its text has not changed for `stable` reads while the site no longer shows it is answering (or for
/// `stable_busy` reads whatever it shows, since such a signal can stick). The first word is waited for as long as the
/// prompt's size allows (`Timing::first_word_wait`), and not at all once the page has shown it is answering: a site
/// that thinks visibly before it writes has given its first word.
pub async fn converse(page: &dyn Page, recipe: &Recipe, prompt: &str, timing: &Timing,
    on_text: &mut (dyn FnMut(&str) + Send)) -> Result<Reply, WebError> {
    open_chat(page, recipe, timing).await?;
    let base: ChatState = eval_as(page, read_script(recipe)).await?;
    let filled: Value = page.eval(fill_script(recipe, prompt)).await?;
    if filled["offSite"] == true {
        return Err(WebError::NeedsYou { state: "needs_you", reason: OFF_SITE });
    }
    if filled["ok"] != true {
        return Err(WebError::PageChanged(NO_PROMPT_BOX));
    }
    let started = Instant::now();
    loop {
        match page.eval(send_script(recipe)).await?.as_str() {
            Some("clicked" | "entered") => break,
            Some("offsite") => return Err(WebError::NeedsYou { state: "needs_you", reason: OFF_SITE }),
            _ if started.elapsed() >= timing.send => return Err(WebError::PageChanged(NO_SEND_BUTTON)),
            _ => tokio::time::sleep(timing.poll).await,
        }
    }

    let started = Instant::now();
    let (first_word, reply_wait) = (timing.first_word_wait(prompt), timing.reply_wait(prompt));
    let (mut sent, mut last, mut stable, mut denied) = (String::new(), None::<String>, 0, 0u64);
    // The page has shown it is answering: its thinking is the first word.
    let mut answering = false;
    loop {
        let elapsed = started.elapsed();
        if elapsed > reply_wait {
            return Err(WebError::Timeout(REPLY_TIMEOUT));
        }
        if sent.is_empty() && !answering && elapsed > first_word {
            return Err(WebError::Timeout(NO_REPLY));
        }
        tokio::time::sleep(timing.poll).await;
        let Ok(state) = eval_as::<ChatState>(page, read_script(recipe)).await else { continue };
        if state.challenge {
            return Err(WebError::NeedsYou { state: "needs_you", reason: CHALLENGE });
        }
        answering |= state.busy;
        if state.approval {
            // The site's agent wants to act for the user (send mail, buy, publish). Through Pygg it never may.
            denied += page.eval(deny_script(recipe)).await.ok().and_then(|v| v.as_u64()).unwrap_or(0);
            continue;
        }
        let text = state.text;
        let new = state.count > base.count || (!text.is_empty() && text != base.text);
        if !new || text.is_empty() {
            continue;
        }
        if last.as_deref() != Some(text.as_str()) {
            // The site may rewrite what it showed (formatting); only text that extends what was sent goes out as it
            // comes, and the rest at the end.
            if let Some(more) = text.strip_prefix(sent.as_str()) {
                if !more.is_empty() {
                    on_text(more);
                    sent = text.clone();
                }
            }
            last = Some(text);
            stable = 0;
        } else {
            stable += 1;
            let settled = if recipe.busy.is_some() { timing.stable } else { timing.stable_busy };
            if (!state.busy && stable >= settled) || stable >= timing.stable_busy {
                let text = last.unwrap_or_default();
                let lower = text.to_lowercase();
                if recipe.quota_hints.iter().any(|h| lower.contains(h)) && text.len() < 600 {
                    return Err(WebError::Quota);
                }
                if text.len() > sent.len() {
                    if let Some(more) = text.strip_prefix(sent.as_str()) {
                        on_text(more);
                    }
                }
                return Ok(Reply { text, denied });
            }
        }
    }
}

/// Opens a new chat to see whether the session is signed in, for the Connections screen.
pub async fn check(page: &dyn Page, recipe: &Recipe, timing: &Timing) -> (&'static str, &'static str) {
    match open_chat(page, recipe, timing).await {
        Ok(()) => ("signed_in", SIGNED_IN),
        Err(e) => e.state().unwrap_or(("page_changed", NO_PROMPT_BOX)),
    }
}

// ---------- Profile folders ----------

/// A connection's profile folder, if its name is one Pygg made (`p-` and hex): nothing else is ever deleted.
pub fn profile_dir(root: &Path, profile: &str) -> Option<std::path::PathBuf> {
    let valid = profile.strip_prefix("p-").is_some_and(|hex| hex.len() == 32 && hex.chars().all(|c| c.is_ascii_hexdigit()));
    valid.then(|| root.join(profile))
}

/// Deletes a profile folder, trying a few times while WebView2 lets go of its files. Returns whether it is gone.
pub fn remove_profile(root: &Path, profile: &str) -> bool {
    let Some(dir) = profile_dir(root, profile) else { return true };
    for _ in 0..10 {
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => return true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return true,
            Err(_) => std::thread::sleep(Duration::from_millis(500)),
        }
    }
    false
}

/// At start-up, before any window opens: deletes every profile folder that belongs to no connection (one whose
/// connection was removed while its files were held). Returns how many were deleted.
pub fn sweep(root: &Path, keep: &[String]) -> usize {
    let Ok(entries) = std::fs::read_dir(root) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if profile_dir(root, &name).is_some() && !keep.contains(&name) && std::fs::remove_dir_all(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

// ---------- The loopback endpoint ----------

/// How the gateway reaches the endpoint: its address and the token it needs, both only in memory.
#[derive(Clone, Debug)]
pub struct Link {
    pub base: String,
    pub token: String,
}

impl Link {
    /// The connection's Chat Completions base, as a provider endpoint.
    pub fn endpoint(&self, provider_id: &str) -> String {
        format!("{}/web/{provider_id}/v1", self.base)
    }
}

/// One request at a time per connection, with a pause after each.
#[derive(Default)]
struct Slot {
    finished: Option<Instant>,
}

pub struct Service {
    db: Arc<Mutex<Connection>>,
    windows: Arc<dyn Windows>,
    token: String,
    timing: Timing,
    slots: Mutex<HashMap<String, Arc<tokio::sync::Mutex<Slot>>>>,
}

impl Service {
    pub fn new(db: Arc<Mutex<Connection>>, windows: Arc<dyn Windows>, token: String, timing: Timing) -> Service {
        Service { db, windows, token, timing, slots: Mutex::default() }
    }

    fn with_db<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
        f(&self.db.lock().unwrap_or_else(|p| p.into_inner()))
    }

    fn slot(&self, provider_id: &str) -> Arc<tokio::sync::Mutex<Slot>> {
        self.slots.lock().unwrap_or_else(|p| p.into_inner()).entry(provider_id.to_string()).or_default().clone()
    }

    fn web_provider(&self, provider_id: &str) -> Result<(Provider, &'static Recipe), WebError> {
        let provider = self.with_db(|db| store::get_provider(db, provider_id)).ok().flatten()
            .ok_or_else(|| WebError::BadRequest("No such web-session connection".into()))?;
        let recipe = recipe(&provider.kind).ok_or_else(|| WebError::BadRequest("No such web-session connection".into()))?;
        if provider.web_ack_version != Some(WARNING_VERSION) {
            return Err(WebError::BadRequest(format!("Not sent: {NOT_ACKNOWLEDGED}")));
        }
        Ok((provider, recipe))
    }

    fn record(&self, provider_id: &str, state: &str, reason: &str) {
        let _ = self.with_db(|db| store::set_web_state(db, provider_id, state, reason));
    }

    /// Waits its turn on the connection (or gives up with `Busy`), then the pause after the last request.
    async fn take_turn(&self, provider_id: &str) -> Result<tokio::sync::OwnedMutexGuard<Slot>, WebError> {
        let guard = tokio::time::timeout(self.timing.queue, self.slot(provider_id).lock_owned()).await
            .map_err(|_| WebError::Busy { retry_after: 30 })?;
        if let Some(wait) = guard.finished.map(|f| self.timing.gap.saturating_sub(f.elapsed())) {
            tokio::time::sleep(wait).await;
        }
        Ok(guard)
    }

    /// Checks whether the session is signed in, unless a request is using it; records and returns its state.
    pub async fn check(&self, provider_id: &str) -> Result<(), String> {
        let (provider, recipe) = self.web_provider(provider_id).map_err(|e| e.message())?;
        let Ok(mut turn) = self.slot(provider_id).try_lock_owned() else { return Ok(()) };
        let page = self.windows.page(&provider, recipe).map_err(|e| e.message())?;
        let (state, reason) = check(page.as_ref(), recipe, &self.timing).await;
        turn.finished = Some(Instant::now());
        self.record(provider_id, state, reason);
        if state == "signed_in" {
            self.windows.signed_in(provider_id);
        }
        Ok(())
    }

    pub fn show(&self, provider_id: &str) -> Result<(), String> {
        let (provider, recipe) = self.web_provider(provider_id).map_err(|e| e.message())?;
        self.windows.show(&provider, recipe)
    }

    pub fn forget(&self, provider_id: &str, profile: &str) {
        self.slots.lock().unwrap_or_else(|p| p.into_inner()).remove(provider_id);
        self.windows.forget(provider_id, profile);
    }

    fn authorized(&self, headers: &HeaderMap) -> bool {
        let given = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer "))
            .unwrap_or("");
        // Constant time over the expected length.
        given.len() == self.token.len() && given.bytes().zip(self.token.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
    }
}

pub fn router(service: Arc<Service>) -> Router {
    Router::new().route("/web/{provider}/v1/chat/completions", post(chat)).with_state(service)
}

/// Serves the endpoint on a loopback listener until the app ends.
pub async fn serve(listener: tokio::net::TcpListener, service: Arc<Service>) {
    if let Err(e) = axum::serve(listener, router(service)).await {
        eprintln!("web sessions: {e}");
    }
}

enum Event {
    Text(String),
    Done(Result<Reply, WebError>),
}

fn chunk(id: &str, model: &str, delta: Value, finish: Option<&str>) -> Bytes {
    let event = json!({ "id": id, "object": "chat.completion.chunk", "created": crate::db::now_ms() / 1000, "model": model,
        "choices": [{ "index": 0, "delta": delta, "finish_reason": finish }] });
    Bytes::from(format!("data: {event}\n\n"))
}

fn denied_note(denied: u64) -> String {
    if denied == 0 { String::new() } else {
        format!("\n\n[Pygg denied {denied} request(s) from the site to act on your behalf. Approve actions in the site's own app.]")
    }
}

async fn chat(State(service): State<Arc<Service>>, UrlPath(provider_id): UrlPath<String>, headers: HeaderMap, body: Bytes) -> Response {
    if !service.authorized(&headers) {
        return (StatusCode::UNAUTHORIZED, "").into_response();
    }
    match start(&service, &provider_id, &body).await {
        Ok(response) => response,
        Err(e) => e.response(),
    }
}

async fn start(service: &Arc<Service>, provider_id: &str, body: &[u8]) -> Result<Response, WebError> {
    let body: Value = serde_json::from_slice(body).map_err(|_| WebError::BadRequest("The body is not JSON".into()))?;
    let (provider, recipe) = service.web_provider(provider_id)?;
    if uses_tools(&body) {
        return Err(WebError::BadRequest(format!("Not sent: {NO_TOOLS}")));
    }
    let prompt = flatten(&body).map_err(WebError::BadRequest)?;
    let model = body["model"].as_str().unwrap_or(recipe.models[0]).to_string();
    let stream = body["stream"] == true;

    // Pygg's own pace limit, before anything reaches the site.
    let now = crate::db::now_ms();
    let limit = provider.web_daily_limit.unwrap_or(DEFAULT_DAILY_LIMIT);
    let used = service.with_db(|db| store::web_requests_today(db, provider_id, now)).unwrap_or(0);
    if used >= u64::from(limit) {
        let retry_after = ((86_400_000 - now.rem_euclid(86_400_000)) / 1000).max(1) as u64;
        return Err(WebError::DailyLimit { limit, retry_after });
    }
    let mut turn = service.take_turn(provider_id).await?;
    let page = service.windows.page(&provider, recipe)?;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    let task_service = service.clone();
    let id = provider_id.to_string();
    tokio::spawn(async move {
        let text_tx = tx.clone();
        let mut on_text = move |piece: &str| { let _ = text_tx.send(Event::Text(piece.to_string())); };
        let result = converse(page.as_ref(), recipe, &prompt, &task_service.timing, &mut on_text).await;
        match &result {
            Ok(_) => task_service.record(&id, "signed_in", SIGNED_IN),
            Err(e) => if let Some((state, reason)) = e.state() { task_service.record(&id, state, reason) },
        }
        turn.finished = Some(Instant::now());
        drop(turn);
        let _ = tx.send(Event::Done(result));
    });

    let completion_id = format!("chatcmpl-web-{}", uuid::Uuid::new_v4().simple());
    if !stream {
        let mut text = String::new();
        while let Some(event) = rx.recv().await {
            match event {
                Event::Text(_) => {}
                Event::Done(Ok(reply)) => { text = reply.text + &denied_note(reply.denied); break; }
                Event::Done(Err(e)) => return Err(e),
            }
        }
        return Ok(axum::Json(json!({ "id": completion_id, "object": "chat.completion", "created": crate::db::now_ms() / 1000,
            "model": model, "choices": [{ "index": 0, "message": { "role": "assistant", "content": text }, "finish_reason": "stop" }] }))
            .into_response());
    }

    // The status is decided by the first thing that happens: an error before any text is an ordinary error answer.
    let first = rx.recv().await.unwrap_or(Event::Done(Err(WebError::Unavailable(NO_WINDOW))));
    let mut pending = vec![chunk(&completion_id, &model, json!({ "role": "assistant", "content": "" }), None)];
    let mut finished = false;
    match first {
        Event::Done(Err(e)) => return Err(e),
        Event::Done(Ok(reply)) => {
            let text = reply.text + &denied_note(reply.denied);
            pending.push(chunk(&completion_id, &model, json!({ "content": text }), None));
            finished = true;
        }
        Event::Text(text) => pending.push(chunk(&completion_id, &model, json!({ "content": text }), None)),
    }
    let tail = futures_util::stream::unfold((rx, finished, completion_id.clone(), model.clone()), |(mut rx, finished, id, model)| async move {
        if finished {
            return None;
        }
        let out = match rx.recv().await {
            Some(Event::Text(text)) => (chunk(&id, &model, json!({ "content": text }), None), false),
            Some(Event::Done(Ok(reply))) => {
                let note = denied_note(reply.denied);
                let mut out = String::new();
                if !note.is_empty() {
                    out.push_str(std::str::from_utf8(&chunk(&id, &model, json!({ "content": note }), None)).unwrap_or_default());
                }
                out.push_str(std::str::from_utf8(&chunk(&id, &model, json!({}), Some("stop"))).unwrap_or_default());
                out.push_str("data: [DONE]\n\n");
                (Bytes::from(out), true)
            }
            Some(Event::Done(Err(e))) => (Bytes::from(format!("data: {}\n\n",
                json!({ "error": { "message": e.message(), "type": "api_error" } }))), true),
            None => return None,
        };
        Some((Ok::<Bytes, std::io::Error>(out.0), (rx, out.1, id, model)))
    });
    let head = if finished {
        pending.push(chunk(&completion_id, &model, json!({}), Some("stop")));
        pending.push(Bytes::from_static(b"data: [DONE]\n\n"));
        futures_util::stream::iter(pending.into_iter().map(Ok::<Bytes, std::io::Error>))
    } else {
        futures_util::stream::iter(pending.into_iter().map(Ok::<Bytes, std::io::Error>))
    };
    use futures_util::StreamExt;
    let body = Body::from_stream(head.chain(tail));
    Ok(Response::builder().status(StatusCode::OK).header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache").body(body).unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()))
}

#[cfg(test)]
mod tests;
