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

pub mod guard;
#[cfg(feature = "app")]
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
use std::sync::atomic::{AtomicBool, Ordering};
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
pub const OFF_SITE: &str =
    "The window is not on the site's own page, so Pygg sent nothing. Open the window and go back to the site.";
/// Requests a web session sends to its site per UTC day unless the user sets another limit: a person's pace, not a
/// script's. Bursts of automated requests are what get accounts of tools like 9router flagged (owner, 2026-11-13).
pub const DEFAULT_DAILY_LIMIT: u32 = 50;
/// The rest between two requests on one account, after the reply ends: a person's pace (owner, 2026-11-13: rotation
/// "must avoid sending requests continuously"). Fixed, not randomised: Pygg slows down, it does not disguise itself.
pub const PACE_SECS: u64 = 10;
/// The site says the account is suspended, banned or restricted (DeepSeek suspended the owner's account on 2026-11-13
/// after a stream of long prompts). Pygg turns the connection off when it records this (`store::set_web_state`).
pub const RESTRICTED: &str = "The site says this account is suspended or restricted. Pygg turned this connection off and sends it nothing. Open the window to read the site's notice; turn the connection back on only after the site lifts it.";
/// Words of such a notice, in lower case. Several sites' wording; a page that shows none of them is not taken for one.
const RESTRICTION_HINTS: &[&str] = &[
    "account has been suspended",
    "account is suspended",
    "account has been banned",
    "account is banned",
    "account has been restricted",
    "account is restricted",
    "account has been disabled",
    "account is disabled",
    "violation of user policies",
    "violated our usage policies",
    "account has been deactivated",
];
pub const CHALLENGE: &str = "The site is showing a check (a CAPTCHA, a bot check or a sign-in challenge). Open the window and deal with it yourself; Pygg does not answer such checks.";
pub const SIGNED_IN: &str = "Signed in.";
pub const CHECK_BUSY: &str = "Not checked: a request is using this session now.";
pub const CHECK_RESTING: &str = "Not checked: this account is resting after its last request or check, and a check loads the site's page as a request does.";
pub const CHECK_BACKED_OFF: &str =
    "Not checked: the site showed a sign of trouble on this account, so Pygg leaves it alone for a while.";
pub const CONNECTION_OFF: &str = "this connection is turned off.";
pub const GUARD_UNREADABLE: &str = "Not sent: Pygg could not read or keep its record of this account's pace, so it sends nothing to the site until it can.";

/// What a sign-in check did: looked at the page, or left it alone and why.
#[derive(Clone, Debug, PartialEq)]
pub enum Checked {
    Seen,
    Skipped(String),
}
pub const NO_PROMPT_BOX: &str = "The page has changed: Pygg could not find its prompt box.";
pub const NO_SEND_BUTTON: &str = "The page has changed: Pygg could not find its Send button.";
pub const NO_REPLY: &str = "No reply appeared in time.";
pub const REPLY_TIMEOUT: &str = "The reply did not finish in time.";
pub const QUOTA: &str = "The site says this account has reached its usage limit.";
pub const SLOW_DOWN: &str = "The site asked to slow down (a \"too many requests\" or \"server is busy\" notice), so Pygg rests this account for at least half an hour.";
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
    /// Where the site puts the pictures and videos it made, when it says so in its markup: media elsewhere on the page
    /// (a mascot, an avatar, a decoration) is not the reply's. `None` looks at the whole page.
    pub media_root: Option<&'static str>,
    /// The site's busy sign (`busy`) is trusted to the end: while it shows, the reply is not finished, however long
    /// its words have been still. Off where a busy sign can stick (the reply's time limit still applies either way).
    pub trust_busy: bool,
}

/// A check the site runs on the browser: a CAPTCHA, Cloudflare's "Just a moment" page or Turnstile, or a bot check.
const CHALLENGE_SELECTORS: &str =
    "iframe[src*=\"captcha\" i], iframe[src*=\"challenges.cloudflare.com\"], iframe[title*=\"challenge\" i], \
    [id*=\"captcha\" i], #challenge-form, #challenge-stage, .cf-turnstile";
const CHALLENGE_HINTS: &[&str] = &[
    "verify you're human",
    "verify you are human",
    "verifying you are human",
    "are you a robot",
    "security check",
    "checking your browser",
    "performing security verification",
    "unusual activity",
    "suspicious activity",
];
const QUOTA_HINTS: &[&str] =
    &["out of credits", "usage limit", "limit reached", "token limit", "reached your limit", "rate limit"];
/// Words of a short finished "reply" that is the site asking to slow down, in lower case (any site). DeepSeek answers a
/// busy server with "The server is busy. Please try again later."; others say "too many requests" or "too quickly".
pub const SLOW_DOWN_HINTS: &[&str] = &[
    "too many requests",
    "too many messages",
    "sending messages too quickly",
    "too quickly",
    "slow down",
    "server is busy",
    "please try again later",
    "try again in a few minutes",
];
/// Google's sign-in, which several sites offer. Google may refuse to sign in inside an embedded window ("This browser
/// or app may not be secure"); Pygg does not hide what the window is, so the site's own email sign-in is then the way.
const GOOGLE: [&str; 2] = ["google.com", "youtube.com"];

/// Sites and their sign-in hosts. Muse's hosts are from its live redirect chain (2026-11-12). The others are the sites'
/// documented sign-in paths, and are checked against the live chain when the owner signs in with the debug navigation
/// log (`PYGG_WEB_DEVTOOLS=1`); their selectors were read from each site's public page where it has one, and follow
/// what 9router reads otherwise.
pub const RECIPES: &[Recipe] = &[
    Recipe {
        preset: "muse-web",
        site: "https://muse.ai",
        new_chat_url: "https://muse.ai/thread/new",
        // Meta's sign-in checks for an existing account by passing through each of its sites in turn ("aymh" redirect
        // cycle: facebook.com, then instagram.com); blocking one of them aborts the sign-in and leaves the window blank.
        hosts: &[
            "muse.ai",
            "meta.com",
            "meta.ai",
            "facebook.com",
            "instagram.com",
            "threads.com",
            "threads.net",
            "messenger.com",
        ],
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
        // 2026-11-18, the owner's DevTools: a picture or video Muse made sits in
        // `[data-testid="hatch-chat-attachment-presentation-video"]` (and `…-image`), a video's own address in the
        // wrapper's `data-hatch-video-src`, with a poster picture beside it; its animated mascot is outside.
        media_root: Some("[data-testid^=\"hatch-chat-attachment-presentation\"]"),
        // 2026-11-18: while Muse makes a picture or video, its send button is a Stop button for as long as it takes; its
        // words ("I'll generate…") come first and stay still meanwhile. Its status avatar is a looping video that never
        // goes, so it is no sign of anything.
        trust_busy: true,
    },
    // ChatGPT chats with visitors too, so "signed in" needs its Log in buttons gone. Sign-in: auth.openai.com (email,
    // phone), Google, Apple, Microsoft. Public page, 2026-11-12: `textarea[name="prompt"]`, a "Send message" button.
    Recipe {
        preset: "chatgpt-web",
        site: "https://chatgpt.com",
        new_chat_url: "https://chatgpt.com/",
        hosts: &[
            "chatgpt.com",
            "openai.com",
            GOOGLE[0],
            GOOGLE[1],
            "apple.com",
            "live.com",
            "microsoftonline.com",
            "microsoft.com",
        ],
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
        media_root: None,
        trust_busy: false,
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
        media_root: None,
        trust_busy: false,
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
        media_root: None,
        trust_busy: false,
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
        media_root: None,
        trust_busy: false,
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
    const TOOL_ITEMS: [&str; 6] = [
        "tool_use",
        "tool_result",
        "function_call",
        "function_call_output",
        "custom_tool_call",
        "custom_tool_call_output",
    ];
    let item_uses_tools = |item: &Value| {
        item["role"] == "tool"
            || item["role"] == "function"
            || item["tool_calls"].as_array().is_some_and(|c| !c.is_empty())
            || item["type"].as_str().is_some_and(|t| TOOL_ITEMS.contains(&t))
            || item["content"]
                .as_array()
                .is_some_and(|parts| parts.iter().any(|p| p["type"].as_str().is_some_and(|t| TOOL_ITEMS.contains(&t))))
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
                    Some("text" | "input_text" | "output_text") => {
                        out.extend(part["text"].as_str().map(str::to_string))
                    }
                    Some("image_url" | "input_image" | "image" | "file" | "input_file" | "input_audio") => {
                        return Err("A web session takes text only; this request has an image or a file".into())
                    }
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
            let mut blocks: Vec<String> = turns
                .iter()
                .map(|(role, text)| {
                    let label = match *role {
                        "system" | "developer" => "System",
                        "assistant" => "Assistant",
                        "user" => "User",
                        other => other,
                    };
                    format!("[{label}]\n{text}")
                })
                .collect();
            blocks.push(
                "[Instruction]\nContinue the conversation above and reply as the Assistant to the last User message. \
                Output only the reply."
                    .into(),
            );
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
            "(() => {{ const e = document.querySelector({}); return !e || e.getAttribute({}) === {}; }})()",
            q(selector),
            q(attr),
            q(value)
        ),
        None => "true".into(),
    };
    format!(
        r#"(() => {{
  const body = document.body ? document.body.innerText.slice(0, 4000).toLowerCase() : '';
  const input = document.querySelector({input});
  const signIn = !input && {sign_in}.some(h => body.includes(h));
  const signedOut = [...document.querySelectorAll('button,a')].some(b => b.offsetParent !== null
    && {signed_out}.includes((b.innerText || '').trim().toLowerCase()));
  const challenge = !!document.querySelector({challenge}) || {challenge_hints}.some(h => body.includes(h));
  const restricted = {restricted}.some(h => body.includes(h));
  return {{ fresh: !window.__pyggOld, ready: document.readyState === 'complete' && !!input && {ready_attr},
           hasInput: !!input, signIn, signedOut, challenge, restricted }};
}})()"#,
        input = q(r.input),
        sign_in = json!(r.sign_in_hints),
        signed_out = json!(r.signed_out_buttons),
        challenge = q(r.challenge_selectors),
        challenge_hints = json!(r.challenge_hints),
        ready_attr = ready_attr,
        restricted = json!(RESTRICTION_HINTS)
    )
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
    format!(
        "(location.protocol === 'https:' && (location.hostname === {h} || location.hostname.endsWith('.' + {h})))",
        h = q(site_host(r))
    )
}

/// Puts the prompt in the box the way typing would, so the site's own code sees it. Off the site's own page it types
/// nothing and says so.
pub fn fill_script(r: &Recipe, text: &str) -> String {
    format!(
        r#"(() => {{
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
}})()"#,
        input = q(r.input),
        text = q(text),
        on_site = on_site(r)
    )
}

/// Clicks the visible Send button: `clicked`, `disabled` or `missing`. Where the recipe says Enter sends and there is
/// no such button, presses Enter in the prompt box instead: `entered`.
pub fn send_script(r: &Recipe) -> String {
    format!(
        r#"(() => {{
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
}})()"#,
        pattern = q(r.send_pattern),
        enter = r.enter_sends,
        input = q(r.input),
        on_site = on_site(r)
    )
}

/// The conversation as it stands: how many replies, the last one's text, and the signals around it.
pub fn read_script(r: &Recipe) -> String {
    format!(
        r#"(() => {{
  const replies = [...document.querySelectorAll({reply})];
  const last = replies.length ? (replies[replies.length - 1].innerText || '').trim() : '';
  const body = document.body ? document.body.innerText.slice(0, 4000).toLowerCase() : '';
  const said = replies.map(e => (e.innerText || '').toLowerCase()).join(' ');
  // Pictures and videos on the page (Muse makes both): their addresses only, never their bytes. A picture still
  // loading holds the reply open; small ones (avatars, icons) are left out.
  const media = [], seen = new Set();
  let pending = 0;
  const rootSel = {media_root};
  const scope = rootSel ? [...document.querySelectorAll(rootSel)] : [document];
  const inScope = sel => scope.flatMap(r => [...r.querySelectorAll(sel)]);
  // A video the site made, named on its wrapper before the <video> is even there (Muse: `data-hatch-video-src`, set
  // when the video is ready; the <video> comes on hover or play, its poster picture before it).
  const wrappers = inScope('[data-hatch-video-src]');
  for (const w of wrappers) {{
    const src = w.getAttribute('data-hatch-video-src') || '';
    if (!/^(https:|blob:)/.test(src) || seen.has(src)) continue;
    seen.add(src); media.push({{ kind: 'video', url: src }});
  }}
  for (const img of inScope('img')) {{
    // A video's poster sits beside it, or inside its wrapper: the video is what was made.
    if (img.parentElement && img.parentElement.querySelector('video')) continue;
    if (img.closest('[data-hatch-video-wrapper], [data-hatch-video-src], [data-hatch-video-poster]')) continue;
    const src = img.currentSrc || img.src || '';
    if (!/^(https:|data:image\/|blob:)/.test(src) || seen.has(src)) continue;
    const box = img.getBoundingClientRect();
    if (!img.complete) {{ if (box.width >= 160) pending++; continue; }}
    if (img.naturalWidth < 256 || img.naturalHeight < 256) continue;
    seen.add(src); media.push({{ kind: 'image', url: src }});
  }}
  for (const a of replies.flatMap(r => [...r.querySelectorAll('a[href]')])) {{
    const href = a.href || '';
    if (!href.startsWith('https:') || seen.has(href)) continue;
    if (!a.hasAttribute('download') && !/\.(pdf|docx?|xlsx?|pptx?|csv|tsv|zip|txt|md|json|xml|html?|mp3|wav|m4a|mp4|webm|mov|png|jpe?g|webp|gif|svg)([?#]|$)/i.test(new URL(href).pathname)) continue;
    seen.add(href); media.push({{ kind: 'file', url: href }});
  }}
  for (const v of inScope('video')) {{
    const source = v.querySelector('source[src]');
    const wrapper = v.closest('[data-hatch-video-src]');
    const src = (wrapper && wrapper.getAttribute('data-hatch-video-src')) || v.currentSrc || v.src || (source ? source.src : '') || '';
    if (!/^(https:|blob:)/.test(src) || seen.has(src)) continue;
    seen.add(src); media.push({{ kind: 'video', url: src }});
  }}
  for (const a of inScope('audio')) {{
    const source = a.querySelector('source[src]');
    const src = a.currentSrc || a.src || (source ? source.src : '') || '';
    if (!/^(https:|blob:)/.test(src) || seen.has(src)) continue;
    seen.add(src); media.push({{ kind: 'audio', url: src }});
  }}
  // Signs that the site is still making something, near the last reply and in its media blocks. Only an increase
  // over the page before the prompt holds the reply, so a sign that is always there does not.
  const lastReply = replies[replies.length - 1];
  const turn = lastReply ? (lastReply.parentElement && lastReply.parentElement.parentElement) || lastReply : null;
  const areas = (turn ? [turn] : []).concat(rootSel ? scope : []);
  const marks = '[aria-busy="true"], [role="progressbar"], progress, .animate-pulse, .animate-bounce, ' +
    '[class*="skeleton" i], [class*="shimmer" i], [class*="typing" i], [aria-label*="typing" i], ' +
    '[data-state="loading"], [data-loading="true"], [data-hatch-video-wrapper]:not([data-hatch-video-src])';
  const making = new Set(areas.flatMap(a => [...a.querySelectorAll(marks)])).size;
  // The last few replies' words: a site may answer in several bubbles (Muse: "Making your PDF now…", then the file).
  const tail = replies.slice(-8).map(e => (e.innerText || '').trim());
  return {{ count: replies.length, text: last, tail, busy: {busy}, media, pending, making,
           approval: {approval}, challenge: !!document.querySelector({challenge}) || {challenge_hints}.some(h => body.includes(h)),
           restricted: {restricted}.some(h => body.includes(h) && !said.includes(h)) }};
}})()"#,
        reply = q(r.reply),
        media_root = json!(r.media_root),
        restricted = json!(RESTRICTION_HINTS),
        busy = r.busy.map(|b| format!("!!document.querySelector({})", q(b))).unwrap_or_else(|| "false".into()),
        approval = r.approval.map(|a| format!("!!document.querySelector({})", q(a))).unwrap_or_else(|| "false".into()),
        challenge = q(r.challenge_selectors),
        challenge_hints = json!(r.challenge_hints)
    )
}

/// What the page holds in pictures and videos, for the app log after each reply: why a reply's pictures were or were
/// not taken. Counts, address kinds, sizes, a video's own settings (loop, muted, autoplay, controls, length), where each
/// sits against the last reply, and the first CSS class names of it and its parents: the site's markup, never an
/// address and never any text of the page.
pub fn media_probe_script(r: &Recipe) -> String {
    format!(
        r#"(() => {{
  const replies = [...document.querySelectorAll({reply})];
  const lastReply = replies[replies.length - 1];
  const where = e => !lastReply ? 'no-reply' : lastReply.contains(e) ? 'in-reply'
    : (lastReply.compareDocumentPosition(e) & Node.DOCUMENT_POSITION_FOLLOWING) ? 'after-reply' : 'before-reply';
  const classes = e => {{
    const c = e.className;
    const text = typeof c === 'string' ? c : (c && c.baseVal) || '';
    return text.split(/\s+/).filter(Boolean).slice(0, 3).map(x => x.slice(0, 40));
  }};
  const box = e => {{ const b = e.getBoundingClientRect(); return Math.round(b.width) + 'x' + Math.round(b.height); }};
  const up = e => {{
    const out = [];
    let p = e.parentElement;
    for (let i = 0; i < 4 && p; i++, p = p.parentElement) out.push(p.tagName.toLowerCase() + (classes(p)[0] ? '.' + classes(p)[0] : ''));
    return out;
  }};
  const scheme = s => ((s || '').match(/^[a-z]+:/) || ['none'])[0];
  const imgs = [...document.querySelectorAll('img')];
  const kinds = {{}};
  let large = 0, loading = 0, widest = 0;
  for (const i of imgs) {{
    const k = scheme(i.currentSrc || i.src);
    kinds[k] = (kinds[k] || 0) + 1;
    if (!i.complete) loading++;
    if (i.naturalWidth >= 256 && i.naturalHeight >= 256) large++;
    widest = Math.max(widest, i.naturalWidth || 0);
  }}
  const videos = [...document.querySelectorAll('video')].map(v => {{
    const source = v.querySelector('source[src]');
    return {{ src: scheme(v.currentSrc || v.src || (source ? source.src : '')), loop: v.loop, muted: v.muted,
             autoplay: v.autoplay, controls: v.controls, shown: box(v), frame: v.videoWidth + 'x' + v.videoHeight,
             seconds: Math.round(v.duration || 0), where: where(v), cls: classes(v), up: up(v) }};
  }});
  const pictures = imgs.filter(i => i.naturalWidth >= 256 && i.naturalHeight >= 256).map(i => ({{
    src: scheme(i.currentSrc || i.src), shown: box(i), where: where(i), cls: classes(i), up: up(i) }}));
  const wide = e => e.getBoundingClientRect().width >= 200;
  let backgrounds = 0;
  for (const e of document.querySelectorAll('div,span,a,button,figure')) {{
    const b = getComputedStyle(e).backgroundImage || '';
    if (b.startsWith('url(') && wide(e)) backgrounds++;
  }}
  const turn = lastReply ? (lastReply.parentElement && lastReply.parentElement.parentElement) || lastReply : null;
  const describe = (root, limit) => {{
    const out = [];
    for (const e of [root, ...root.querySelectorAll('*')].slice(0, limit)) {{
      const tag = e.tagName.toLowerCase();
      if (['path', 'g', 'span', 'p', 'br', 'strong', 'em', 'b', 'i', 'li', 'ul', 'ol', 'code', 'pre'].includes(tag)) continue;
      const bits = [tag];
      const id = e.getAttribute('data-testid');
      if (id) bits.push('#' + id.slice(0, 60));
      for (const a of ['role', 'aria-busy', 'data-state', 'data-loading']) {{ const v = e.getAttribute(a); if (v) bits.push('[' + a + '=' + v.slice(0, 20) + ']'); }}
      for (const a of e.getAttributeNames()) {{
        if (!a.startsWith('data-hatch') || a.endsWith('-src') || a.endsWith('-poster')) continue;
        const v = e.getAttribute(a) || '';
        bits.push('[' + a + (v && v.length <= 12 ? '=' + v : '') + ']');
      }}
      if (e.hasAttribute('href')) bits.push('[href=' + scheme(e.getAttribute('href')) + ']');
      if (e.hasAttribute('download')) bits.push('[download]');
      if (e.hasAttribute('aria-label')) {{
        // A control's label is the site's own words (Download, Share, More); a block's may be a file name: kept out.
        const own = ['button', 'a'].includes(tag) || ['button', 'menuitem'].includes(e.getAttribute('role'));
        bits.push(own ? '[aria-label=' + (e.getAttribute('aria-label') || '').slice(0, 30) + ']' : '[aria-label]');
      }}
      if (e.hasAttribute('aria-haspopup')) bits.push('[aria-haspopup=' + e.getAttribute('aria-haspopup') + ']');
      const c = classes(e)[0];
      if (c) bits.push('.' + c);
      out.push(bits.join(''));
    }}
    return out;
  }};
  const outline = turn ? describe(turn, 120) : [];
  // Blocks the site marks as attachments (Muse: hatch-chat-attachment-presentation-*), described the same way.
  const attachments = [...document.querySelectorAll('[data-testid*="attachment" i]')].slice(-4).map(a => describe(a, 40));
  return {{ busy: {busy}, attachments, replies: replies.length, imgs: imgs.length, kinds, large, loading, widest, videos, pictures, outline,
           canvases: [...document.querySelectorAll('canvas')].filter(wide).length,
           svgs: [...document.querySelectorAll('svg')].filter(wide).length,
           pictureTags: document.querySelectorAll('picture').length, backgrounds,
           audios: [...document.querySelectorAll('audio')].map(a => scheme(a.currentSrc || a.src)),
           videoWrappers: [...document.querySelectorAll('[data-hatch-video-src]')].map(w => scheme(w.getAttribute('data-hatch-video-src'))),
           links: [...document.querySelectorAll('a[href]')].filter(a => lastReply && (lastReply.contains(a) || (lastReply.compareDocumentPosition(a) & Node.DOCUMENT_POSITION_FOLLOWING))).map(a => scheme(a.href) + (a.hasAttribute('download') ? 'download' : '')) }};
}})()"#,
        reply = q(r.reply),
        busy = r.busy.map(|b| format!("!!document.querySelector({})", q(b))).unwrap_or_else(|| "false".into())
    )
}

/// A player's clock ("0:00", "1:05:30"), which is not words of the reply.
fn is_player_time(t: &str) -> bool {
    let parts: Vec<&str> = t.split(':').collect();
    (2..=3).contains(&parts.len())
        && parts.iter().all(|p| !p.is_empty() && p.len() <= 2 && p.chars().all(|c| c.is_ascii_digit()))
}

/// The words of every reply since the prompt, in order, one paragraph each: a site may answer in several bubbles.
/// Without the replies' words (a page that reports only the last), the last reply's.
fn reply_text(state: &ChatState, before: u64) -> String {
    if state.tail.is_empty() || state.count <= before {
        return state.text.clone();
    }
    let first = state.count.saturating_sub(state.tail.len() as u64);
    let parts: Vec<&str> = state
        .tail
        .iter()
        .enumerate()
        .filter(|(i, t)| first + *i as u64 >= before && !t.is_empty() && !is_player_time(t))
        .map(|(_, t)| t.as_str())
        .collect();
    if parts.is_empty() {
        state.text.clone()
    } else {
        parts.join("\n\n")
    }
}

/// The media a reply's page received from the network after the prompt: a sound or document as a data URL of its
/// kind, and the addresses of sounds and files named in the site's API answers (a player that loads its sound only on
/// Play still has it named there). What was taken and what was seen goes to the app log, by kind and host only.
fn network_media(received: Vec<Received>, preset: &str) -> Vec<Media> {
    let mut out: Vec<Media> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let mut summary: Vec<String> = Vec::new();
    for r in received {
        let mime = r.mime.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
        summary.push(format!("{mime} from {} ({} bytes)", r.host, r.body.len()));
        if mime.contains("json") {
            for url in media_addresses(&String::from_utf8_lossy(&r.body)) {
                if !seen.contains(&url.url) {
                    seen.push(url.url.clone());
                    out.push(url);
                }
            }
            continue;
        }
        if r.body.is_empty() || r.body.len() as u64 > BLOB_LIMIT {
            continue;
        }
        // A generic type is replaced by what the bytes say; a body that is none of the kept kinds is left.
        let generic =
            matches!(mime.as_str(), "" | "application/octet-stream" | "binary/octet-stream" | "application/x-binary");
        let (mime, kind) = match (generic, sniff(&r.body)) {
            (true, Some((m, k))) => (m.to_string(), k),
            (true, None) => continue,
            (false, _) => {
                let kind = if mime.starts_with("audio/") { "audio" } else { "file" };
                (mime, kind)
            }
        };
        let url = format!("data:{mime};base64,{}", base64_encode(&r.body));
        if !seen.contains(&url) {
            seen.push(url.clone());
            out.push(Media { kind: kind.to_string(), url });
        }
    }
    if !summary.is_empty() {
        crate::applog::info(&format!(
            "web reply on {preset}: the page received {}; {} sound(s) or file(s) taken from it",
            summary.join(", "),
            out.len()
        ));
    }
    out
}

/// https addresses of sounds and documents in a text (an API answer), by their extension; `\u002F` and `\/` escapes
/// undone.
fn media_addresses(text: &str) -> Vec<Media> {
    let text = text.replace("\\u002F", "/").replace("\\/", "/").replace("\\u0026", "&");
    let mut out = Vec::new();
    let mut rest = text.as_str();
    while let Some(i) = rest.find("https://") {
        let tail = &rest[i..];
        let end = tail
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '\\' | ')'))
            .unwrap_or(tail.len());
        let url = &tail[..end];
        rest = &tail[end..];
        let path = url.split(['?', '#']).next().unwrap_or("").to_ascii_lowercase();
        let kind = if [".mp3", ".wav", ".m4a", ".aac", ".ogg", ".oga", ".opus", ".weba", ".flac"]
            .iter()
            .any(|e| path.ends_with(e))
        {
            "audio"
        } else if [".pdf", ".docx", ".xlsx", ".pptx", ".csv", ".zip", ".txt", ".md"].iter().any(|e| path.ends_with(e)) {
            "file"
        } else {
            continue;
        };
        out.push(Media { kind: kind.to_string(), url: url.to_string() });
    }
    out
}

/// Standard base64.
fn base64_encode(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n =
            (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(A[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// How often a reply held open by the page's signs is described in the app log.
const PROBE_EVERY: Duration = Duration::from_secs(20);

/// The end of a reply: the words not yet given out, the page's media probe in the app log, and its `blob:` media
/// copied out.
#[allow(clippy::too_many_arguments)]
async fn finish(
    page: &dyn Page, recipe: &Recipe, timing: &Timing, text: String, sent: &str, denied: u64, media: Vec<Media>,
    on_text: &mut (dyn FnMut(&str) + Send), note: &str, before: u64,
) -> Reply {
    if text.len() > sent.len() {
        if let Some(more) = text.strip_prefix(sent) {
            on_text(more);
        }
    }
    // Why pictures were or were not taken, in counts and markup only (owner's report, 2026-11-18).
    let probe = page.eval(media_probe_script(recipe)).await.unwrap_or(Value::Null);
    crate::applog::info(&format!(
        "web reply on {}: {} picture(s), video(s) or file(s) taken{note}; page: {probe}",
        recipe.preset,
        media.len()
    ));
    let mut media = with_blob_media(page, media, timing.poll).await;
    // A response still being read when the words settled is given a few reads to arrive.
    let mut received = page.network();
    for _ in 0..3 {
        tokio::time::sleep(timing.poll).await;
        received.extend(page.network());
    }
    let _ = before;
    let seen = page.network_seen();
    if !seen.is_empty() {
        let mut counts: Vec<(String, usize)> = Vec::new();
        for s in seen {
            match counts.iter_mut().find(|(k, _)| *k == s) {
                Some((_, n)) => *n += 1,
                None => counts.push((s, 1)),
            }
        }
        let list: Vec<String> = counts.into_iter().map(|(k, n)| format!("{k} ×{n}")).collect();
        crate::applog::info(&format!(
            "web reply on {}: every response the page received: {}",
            recipe.preset,
            list.join(", ")
        ));
    }
    media.extend(network_media(received, recipe.preset));
    Reply { text, denied, media }
}

/// The most a `blob:` picture or video may be when it is copied out of the page.
const BLOB_LIMIT: u64 = 100 * 1024 * 1024;

/// Starts copying the `blob:` pictures and videos of a reply into data URLs, in the page (a `blob:` address means nothing
/// outside the window). It reads the page's own memory: `fetch` is called on `blob:` addresses only, never the network
/// (test `no_script_reads_cookies_storage_or_the_network`). The page cannot hand back a promise, so the results wait in
/// the page until `blob_poll_script` takes them.
pub fn blob_start_script(urls: &[String]) -> String {
    format!(
        r#"(() => {{
  const store = window.__pyggMedia = window.__pyggMedia || {{}};
  for (const u of {urls}) {{
    if (!u.startsWith('blob:') || store[u]) continue;
    store[u] = 'pending';
    fetch(u).then(r => r.blob()).then(b => {{
      if (b.size > {limit}) {{ store[u] = 'too-large'; return; }}
      const reader = new FileReader();
      reader.onload = () => {{ store[u] = String(reader.result); }};
      reader.onerror = () => {{ store[u] = 'error'; }};
      reader.readAsDataURL(b);
    }}).catch(() => {{ store[u] = 'error'; }});
  }}
  return true;
}})()"#,
        urls = json!(urls),
        limit = BLOB_LIMIT
    )
}

/// What became of each copy `blob_start_script` started: a data URL, `pending`, `too-large` or `error`. A finished copy
/// is handed back once and dropped from the page.
pub fn blob_poll_script(urls: &[String]) -> String {
    format!(
        r#"(() => {{
  const store = window.__pyggMedia || {{}};
  const out = {{}};
  for (const u of {urls}) {{
    out[u] = store[u] || 'missing';
    if (out[u] !== 'pending') delete store[u];
  }}
  return out;
}})()"#,
        urls = json!(urls)
    )
}

/// The reply's `blob:` pictures and videos as data URLs, waiting up to a minute for the copies; one that cannot be
/// copied is left out and said in the app log.
async fn with_blob_media(page: &dyn Page, media: Vec<Media>, poll: Duration) -> Vec<Media> {
    let blobs: Vec<String> = media.iter().filter(|m| m.url.starts_with("blob:")).map(|m| m.url.clone()).collect();
    if blobs.is_empty() {
        return media;
    }
    let _ = page.eval(blob_start_script(&blobs)).await;
    let started = Instant::now();
    let mut copies: HashMap<String, String> = HashMap::new();
    while copies.len() < blobs.len() && started.elapsed() < Duration::from_secs(60) {
        tokio::time::sleep(poll).await;
        let left: Vec<String> = blobs.iter().filter(|u| !copies.contains_key(*u)).cloned().collect();
        let Ok(Value::Object(found)) = page.eval(blob_poll_script(&left)).await else { continue };
        for (url, result) in found {
            match result.as_str() {
                Some("pending") | None => {}
                Some(done) => {
                    copies.insert(url, done.to_string());
                }
            }
        }
    }
    media
        .into_iter()
        .filter_map(|m| {
            if !m.url.starts_with("blob:") {
                return Some(m);
            }
            match copies.get(&m.url) {
                Some(data) if data.starts_with("data:") => Some(Media { url: data.clone(), ..m }),
                other => {
                    crate::applog::warn(&format!(
                        "a {} on the web page could not be copied: {}",
                        m.kind,
                        other.map(String::as_str).unwrap_or("no answer in a minute")
                    ));
                    None
                }
            }
        })
        .collect()
}

/// Clicks the site's own "deny" on every pending request for approval; returns how many.
pub fn deny_script(r: &Recipe) -> String {
    let Some(approval) = r.approval else { return "0".into() };
    format!(
        r#"(() => {{
  const re = new RegExp({pattern}, 'i');
  let denied = 0;
  for (const card of document.querySelectorAll({approval})) {{
    const button = [...card.querySelectorAll('button,[role=button]')].find(b => re.test((b.innerText || '').trim()));
    if (button && !button.disabled) {{ button.click(); denied++; }}
  }}
  return denied;
}})()"#,
        pattern = q(r.deny_pattern),
        approval = q(approval)
    )
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
    /// The site's notice that the account is suspended or restricted.
    pub restricted: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ChatState {
    pub count: u64,
    pub text: String,
    pub busy: bool,
    pub approval: bool,
    pub challenge: bool,
    /// A suspension or restriction notice outside the replies (`RESTRICTION_HINTS`).
    pub restricted: bool,
    /// Pictures and videos on the page, by address.
    pub media: Vec<Media>,
    /// Large pictures still loading.
    pub pending: u64,
    /// Signs near the reply that the site is still making something: `aria-busy`, a progress bar, a skeleton or
    /// shimmer, a loading state, or a media block whose video has no address yet.
    pub making: u64,
    /// The words of the last few replies, oldest first (at most eight).
    pub tail: Vec<String>,
}

/// A picture, video or file the site showed with its reply: its address only (an https address, or a picture's data
/// URL), never its bytes.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Media {
    /// `image`, `video`, `audio` or `file` (a download link in a reply).
    pub kind: String,
    pub url: String,
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
    /// The prompt is longer than a person would type into the site's chat box: 413; nothing reached the site.
    TooLong { chars: usize, max: usize },
    /// The site asked to slow down: 429; the web guard rests the account at least half an hour.
    SlowDown,
    /// The web guard says not now (a rest, the hour's or the day's share, a back-off): 429 with a wait; nothing reached
    /// the site.
    Pace { retry_after: u64, why: &'static str },
}

impl WebError {
    pub fn status(&self) -> StatusCode {
        match self {
            WebError::NeedsYou { .. } => StatusCode::UNAUTHORIZED,
            WebError::Busy { .. }
            | WebError::Quota
            | WebError::SlowDown
            | WebError::DailyLimit { .. }
            | WebError::Pace { .. } => StatusCode::TOO_MANY_REQUESTS,
            WebError::TooLong { .. } => StatusCode::PAYLOAD_TOO_LARGE,
            WebError::PageChanged(_) | WebError::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            WebError::Timeout(_) => StatusCode::GATEWAY_TIMEOUT,
            WebError::BadRequest(_) => StatusCode::BAD_REQUEST,
        }
    }

    pub fn message(&self) -> String {
        match self {
            WebError::NeedsYou { reason, .. } => reason.to_string(),
            WebError::Busy { .. } => "Not sent: the site is answering another request; a web session takes one at a time.".into(),
            WebError::Quota => QUOTA.into(),
            WebError::SlowDown => SLOW_DOWN.into(),
            WebError::PageChanged(why) | WebError::Timeout(why) | WebError::Unavailable(why) => why.to_string(),
            WebError::BadRequest(why) => why.clone(),
            WebError::DailyLimit { limit, .. } => format!("Not sent: this web session reached its daily limit of {limit} \
                requests, which keeps its use at a person's pace. It resets at 00:00 UTC; change it in Pygg → Providers."),
            WebError::TooLong { chars, max } => format!("Not sent: the prompt is {chars} characters, and this site's web \
                session takes at most {max} (about {} tokens), what a person would type into a chat box. Long work, such as \
                whole files, belongs on an API connection.", max / 4),
            WebError::Pace { why, .. } => why.to_string(),
        }
    }

    /// A sign of trouble on the page, after which the web guard rests the account (a back-off that doubles, with a floor
    /// for the site's own warnings: a usage limit an hour, a request to slow down or a check half an hour, a suspension
    /// six hours).
    pub fn trouble(&self) -> Option<guard::Trouble> {
        match self {
            WebError::Timeout(why) | WebError::PageChanged(why) => Some(guard::Trouble::new(why, 1)),
            WebError::Quota => Some(guard::Trouble::new(QUOTA, 12)),
            WebError::SlowDown => Some(guard::Trouble::new(SLOW_DOWN, 6)),
            WebError::NeedsYou { reason, .. } if *reason == CHALLENGE => Some(guard::Trouble::new(CHALLENGE, 6)),
            WebError::NeedsYou { reason, .. } if *reason == RESTRICTED => Some(guard::Trouble::new(RESTRICTED, 72)),
            _ => None,
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
            StatusCode::BAD_REQUEST | StatusCode::PAYLOAD_TOO_LARGE => "invalid_request_error",
            _ => "api_error",
        };
        let code = match self {
            WebError::Quota => Some("insufficient_quota"),
            WebError::SlowDown => Some("site_slow_down"),
            WebError::DailyLimit { .. } => Some("daily_limit"),
            WebError::Pace { .. } => Some("web_pace"),
            WebError::TooLong { .. } => Some("prompt_too_long"),
            _ => None,
        };
        let mut response = (
            self.status(),
            axum::Json(json!({ "error": {
            "message": self.message(), "type": kind, "code": code } })),
        )
            .into_response();
        if let WebError::Busy { retry_after }
        | WebError::DailyLimit { retry_after, .. }
        | WebError::Pace { retry_after, .. } = self
        {
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
    /// What the page received from the network since the last call, taken: sounds, documents and the site's own API
    /// answers (`window.rs` reads WebView2's response events). Never a request, a cookie or a header.
    fn network(&self) -> Vec<Received> {
        Vec::new()
    }
    /// Every response the page received since the last call, as `type from host` only (for the app log), taken.
    fn network_seen(&self) -> Vec<String> {
        Vec::new()
    }
}

/// One response the page received: its content type, the address's host, and its body (a sound or document, or the
/// text of the site's API answer).
#[derive(Clone, Debug, PartialEq)]
pub struct Received {
    pub mime: String,
    pub host: String,
    pub body: Vec<u8>,
}

/// Whether a response is kept for the reply: a sound, a document, or the site's own API answer in JSON (searched for
/// the addresses of what it made). Pictures and videos come from the page itself.
pub fn worth_keeping(mime: &str, host: &str, site_host: &str) -> bool {
    let mime = mime.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    mime.starts_with("audio/")
        || matches!(
            mime.as_str(),
            "application/pdf"
                | "application/zip"
                | "application/msword"
                | "text/csv"
                | "application/vnd.ms-excel"
                | "application/vnd.ms-powerpoint"
        )
        || mime.starts_with("application/vnd.openxmlformats-officedocument.")
        // A file sent with no type of its own is kept to be recognised by its first bytes (`sniff`).
        || matches!(mime.as_str(), "" | "application/octet-stream" | "binary/octet-stream" | "application/x-binary")
        || (mime.contains("json") && (host == site_host || host.ends_with(&format!(".{site_host}"))))
}

/// What a body is by its first bytes, whatever type it was sent with: a PDF, a sound, or a zip (Office documents are
/// zips). `None` for anything else.
pub fn sniff(body: &[u8]) -> Option<(&'static str, &'static str)> {
    let at = |i: usize, s: &[u8]| body.len() >= i + s.len() && &body[i..i + s.len()] == s;
    Some(if at(0, b"%PDF-") {
        ("application/pdf", "file")
    } else if at(0, b"ID3") || (body.len() > 1 && body[0] == 0xFF && body[1] & 0xE0 == 0xE0) {
        ("audio/mpeg", "audio")
    } else if at(0, b"RIFF") && at(8, b"WAVE") {
        ("audio/wav", "audio")
    } else if at(0, b"OggS") {
        ("audio/ogg", "audio")
    } else if at(0, b"fLaC") {
        ("audio/flac", "audio")
    } else if at(4, b"ftypM4A") || at(4, b"ftypm4a") {
        ("audio/mp4", "audio")
    } else if at(0, b"PK\x03\x04") {
        ("application/zip", "file")
    } else {
        return None;
    })
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
    /// How long a second request waits for the connection, and the pause after each request (the web guard's base
    /// rest, which grows with the request and its reply).
    pub queue: Duration,
    pub gap: Duration,
    /// The web guard's first back-off after a sign of trouble (`guard::BACKOFF_BASE_MS`).
    pub backoff: Duration,
}

impl Default for Timing {
    fn default() -> Timing {
        Timing {
            poll: Duration::from_millis(300),
            ready: Duration::from_secs(45),
            send: Duration::from_secs(5),
            first_word: Duration::from_secs(60),
            first_word_per_100_tokens: Duration::from_secs(1),
            first_word_cap: Duration::from_secs(240),
            reply: Duration::from_secs(300),
            stable: 4,
            stable_busy: 12,
            queue: Duration::from_secs(60),
            gap: Duration::from_secs(PACE_SECS),
            backoff: Duration::from_millis(guard::BACKOFF_BASE_MS as u64),
        }
    }
}

impl Timing {
    /// This timing with the web guard's floors: at least the base pace between requests (`PACE_SECS`) and the base
    /// back-off after trouble. The guard has no off switch: the app uses this, and only the crate's own tests run with
    /// less.
    pub fn guarded(self) -> Timing {
        let floor = Timing::default();
        Timing { gap: self.gap.max(floor.gap), backoff: self.backoff.max(floor.backoff), ..self }
    }

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
        if state.fresh && state.restricted {
            return Err(WebError::NeedsYou { state: "needs_you", reason: RESTRICTED });
        }
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
    /// Pictures and videos that appeared after the prompt was sent (Muse makes them), in page order.
    pub media: Vec<Media>,
}

/// Sends one prompt in a new chat and reads the reply as it appears, giving each new piece to `on_text`. The reply is
/// finished when its text has not changed for `stable` reads while the site no longer shows it is answering (or for
/// `stable_busy` reads whatever it shows, since such a signal can stick). The first word is waited for as long as the
/// prompt's size allows (`Timing::first_word_wait`), and not at all once the page has shown it is answering: a site
/// that thinks visibly before it writes has given its first word.
pub async fn converse(
    page: &dyn Page, recipe: &Recipe, prompt: &str, timing: &Timing, on_text: &mut (dyn FnMut(&str) + Send),
) -> Result<Reply, WebError> {
    converse_marked(page, recipe, prompt, timing, on_text, &AtomicBool::new(false)).await
}

/// `converse`, setting `submitted` once the site's Send was pressed: from then on the site may have the prompt, and the
/// gateway sends it to no other web session (Codex's fourth review, A8).
pub async fn converse_marked(
    page: &dyn Page, recipe: &Recipe, prompt: &str, timing: &Timing, on_text: &mut (dyn FnMut(&str) + Send),
    submitted: &AtomicBool,
) -> Result<Reply, WebError> {
    open_chat(page, recipe, timing).await?;
    // What the page received before the prompt is not the reply's.
    let _ = page.network();
    let _ = page.network_seen();
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
            Some("clicked" | "entered") => {
                submitted.store(true, Ordering::SeqCst);
                break;
            }
            Some("offsite") => return Err(WebError::NeedsYou { state: "needs_you", reason: OFF_SITE }),
            _ if started.elapsed() >= timing.send => return Err(WebError::PageChanged(NO_SEND_BUTTON)),
            _ => tokio::time::sleep(timing.poll).await,
        }
    }

    let started = Instant::now();
    let (first_word, reply_wait) = (timing.first_word_wait(prompt), timing.reply_wait(prompt));
    let (mut sent, mut last, mut stable, mut denied) = (String::new(), None::<String>, 0, 0u64);
    let mut media: Vec<Media> = Vec::new();
    let mut last_probe = Instant::now();
    // The page has shown it is answering: its thinking is the first word.
    let mut answering = false;
    // A notice in the prompt itself is the user's text, not the site's.
    let prompt_lower = prompt.to_lowercase();
    let quoted = RESTRICTION_HINTS.iter().any(|h| prompt_lower.contains(h));
    loop {
        let elapsed = started.elapsed();
        if elapsed > reply_wait {
            // What came is kept: words or media already on the page are the reply, only cut at the time limit.
            if last.as_deref().is_some_and(|t| !t.is_empty()) || !media.is_empty() {
                let text = last.unwrap_or_default();
                let note = ", ended at the time limit while the page still showed it was busy";
                return Ok(finish(page, recipe, timing, text, &sent, denied, media, on_text, note, base.count).await);
            }
            return Err(WebError::Timeout(REPLY_TIMEOUT));
        }
        if sent.is_empty() && media.is_empty() && !answering && elapsed > first_word {
            return Err(WebError::Timeout(NO_REPLY));
        }
        tokio::time::sleep(timing.poll).await;
        let Ok(state) = eval_as::<ChatState>(page, read_script(recipe)).await else { continue };
        if state.restricted && !quoted {
            return Err(WebError::NeedsYou { state: "needs_you", reason: RESTRICTED });
        }
        if state.challenge {
            return Err(WebError::NeedsYou { state: "needs_you", reason: CHALLENGE });
        }
        // The site shows it is still making something (a progress bar, a skeleton, a loading state) that was not on
        // the page before the prompt: the reply is not finished, however long its words have been still.
        let making = state.making > base.making;
        answering |= state.busy || making;
        if state.approval {
            // The site's agent wants to act for the user (send mail, buy, publish). Through Pygg it never may.
            denied += page.eval(deny_script(recipe)).await.ok().and_then(|v| v.as_u64()).unwrap_or(0);
            continue;
        }
        // What appeared since the prompt was sent; a change keeps the reply open, as new text does.
        let fresh: Vec<Media> = state.media.iter().filter(|m| !base.media.contains(m)).cloned().collect();
        if fresh != media {
            media = fresh;
            stable = 0;
        }
        let waiting = state.pending > 0 || making;
        let text = reply_text(&state, base.count);
        let new = state.count > base.count || (!text.is_empty() && text != base.text);
        // A reply may be a picture alone.
        if (!new || text.is_empty()) && media.is_empty() {
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
            let held = waiting || (recipe.trust_busy && state.busy);
            if held && stable >= timing.stable_busy && last_probe.elapsed() >= PROBE_EVERY {
                // What holds a still reply open, in the app log every 20 s: counts and markup only.
                last_probe = Instant::now();
                let probe = page.eval(media_probe_script(recipe)).await.unwrap_or(Value::Null);
                crate::applog::info(&format!(
                    "web reply on {} held after {} s (busy {}, making {}, loading {}): {probe}",
                    recipe.preset,
                    started.elapsed().as_secs(),
                    state.busy,
                    state.making,
                    state.pending
                ));
            }
            if !held && ((!state.busy && stable >= settled) || stable >= timing.stable_busy) {
                let text = last.unwrap_or_default();
                let lower = text.to_lowercase();
                if recipe.quota_hints.iter().any(|h| lower.contains(h)) && text.len() < 600 {
                    return Err(WebError::Quota);
                }
                // A short "reply" that asks to slow down is the site's notice, unless the user's own prompt said it.
                if text.len() < 600 && SLOW_DOWN_HINTS.iter().any(|h| lower.contains(h) && !prompt_lower.contains(h)) {
                    return Err(WebError::SlowDown);
                }
                return Ok(finish(page, recipe, timing, text, &sent, denied, media, on_text, "", base.count).await);
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
    let valid =
        profile.strip_prefix("p-").is_some_and(|hex| hex.len() == 32 && hex.chars().all(|c| c.is_ascii_hexdigit()));
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
        if profile_dir(root, &name).is_some() && !keep.contains(&name) && std::fs::remove_dir_all(entry.path()).is_ok()
        {
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
    /// Outside the crate's own tests the timing is held at or above the web guard's floors (`Timing::guarded`), so no
    /// caller can switch the pace or the back-off off by passing a shorter timing.
    pub fn new(db: Arc<Mutex<Connection>>, windows: Arc<dyn Windows>, token: String, timing: Timing) -> Service {
        #[cfg(not(test))]
        let timing = timing.guarded();
        Service { db, windows, token, timing, slots: Mutex::default() }
    }

    fn with_db<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
        f(&self.db.lock().unwrap_or_else(|p| p.into_inner()))
    }

    fn slot(&self, provider_id: &str) -> Arc<tokio::sync::Mutex<Slot>> {
        self.slots.lock().unwrap_or_else(|p| p.into_inner()).entry(provider_id.to_string()).or_default().clone()
    }

    fn web_provider(&self, provider_id: &str) -> Result<(Provider, &'static Recipe), WebError> {
        let provider = self
            .with_db(|db| store::get_provider(db, provider_id))
            .ok()
            .flatten()
            .ok_or_else(|| WebError::BadRequest("No such web-session connection".into()))?;
        let recipe =
            recipe(&provider.kind).ok_or_else(|| WebError::BadRequest("No such web-session connection".into()))?;
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
        let guard = tokio::time::timeout(self.timing.queue, self.slot(provider_id).lock_owned())
            .await
            .map_err(|_| WebError::Busy { retry_after: 30 })?;
        if let Some(wait) = guard.finished.map(|f| self.timing.gap.saturating_sub(f.elapsed())) {
            tokio::time::sleep(wait).await;
        }
        Ok(guard)
    }

    /// Checks whether the session is signed in and records its state, unless a request is using it or the web guard has
    /// the account resting or backed off: a check loads the site's page, so it keeps the same pace as a request (Codex's
    /// fourth review, A6 and A7). A check that sees trouble backs the account off as a request would.
    pub async fn check(&self, provider_id: &str) -> Result<Checked, String> {
        let (provider, recipe) = self.web_provider(provider_id).map_err(|e| e.message())?;
        let Ok(mut turn) = self.slot(provider_id).try_lock_owned() else {
            return Ok(Checked::Skipped(CHECK_BUSY.into()));
        };
        let now = crate::db::now_ms();
        let account =
            self.with_db(|db| store::web_guard(db, provider_id, now)).map_err(|_| GUARD_UNREADABLE.to_string())?;
        if let Some(until) = account.backoff_until.filter(|u| *u > now) {
            return Ok(Checked::Skipped(format!(
                "{CHECK_BACKED_OFF} ({} min. left; Resume Now lifts it.)",
                (until - now + 59_999) / 60_000
            )));
        }
        if let Some(next) = account.next_at.filter(|n| *n > now) {
            return Ok(Checked::Skipped(format!("{CHECK_RESTING} ({} s left.)", (next - now + 999) / 1000)));
        }
        let page = self.windows.page(&provider, recipe).map_err(|e| e.message())?;
        let (state, reason) = check(page.as_ref(), recipe, &self.timing).await;
        turn.finished = Some(Instant::now());
        self.record(provider_id, state, reason);
        let ended = crate::db::now_ms();
        let limits = guard::limits(&provider.kind);
        let rest = ended + guard::rest_ms(limits.rest_base(self.timing.gap.as_millis() as i64), 1, 0);
        let trouble = match reason {
            r if r == CHALLENGE => Some(guard::Trouble::new(CHALLENGE, 6)),
            r if r == RESTRICTED => Some(guard::Trouble::new(RESTRICTED, 72)),
            r if r == NO_PROMPT_BOX => Some(guard::Trouble::new(NO_PROMPT_BOX, 1)),
            _ => None,
        };
        let _ = self.with_db(|db| match trouble {
            Some(t) => {
                store::web_guard_trouble(db, provider_id, &t, rest, self.timing.backoff.as_millis() as i64, ended)
            }
            None => store::web_guard_checked(db, provider_id, rest),
        });
        if state == "signed_in" {
            self.windows.signed_in(provider_id);
        }
        Ok(Checked::Seen)
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
        let given = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .unwrap_or("");
        // Constant time over the expected length.
        given.len() == self.token.len()
            && given.bytes().zip(self.token.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
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

/// What follows a web session's reply text: its pictures and videos as Markdown links, then any denied action.
fn reply_tail(reply: &Reply) -> String {
    media_note(&reply.media) + &denied_note(reply.denied)
}

/// Pictures as Markdown images, videos and files as links, so a client shows or saves them (`pygg chat` saves them).
pub fn media_note(media: &[Media]) -> String {
    let (mut images, mut videos, mut files, mut audios) = (0, 0, 0, 0);
    let mut out = String::new();
    for m in media {
        match m.kind.as_str() {
            "video" => {
                videos += 1;
                out.push_str(&format!("\n\n[video {videos}]({})", m.url));
            }
            "file" => {
                files += 1;
                out.push_str(&format!("\n\n[file {files}]({})", m.url));
            }
            "audio" => {
                audios += 1;
                out.push_str(&format!("\n\n[audio {audios}]({})", m.url));
            }
            _ => {
                images += 1;
                out.push_str(&format!("\n\n![image {images}]({})", m.url));
            }
        }
    }
    out
}

fn denied_note(denied: u64) -> String {
    if denied == 0 {
        String::new()
    } else {
        format!("\n\n[Pygg denied {denied} request(s) from the site to act on your behalf. Approve actions in the site's own app.]")
    }
}

async fn chat(
    State(service): State<Arc<Service>>, UrlPath(provider_id): UrlPath<String>, headers: HeaderMap, body: Bytes,
) -> Response {
    if !service.authorized(&headers) {
        return (StatusCode::UNAUTHORIZED, "").into_response();
    }
    match start(&service, &provider_id, &body).await {
        Ok(response) => response,
        Err(failed) => {
            let mut response = failed.error.response();
            if failed.sent {
                response.headers_mut().insert(SENT_HEADER, HeaderValue::from_static("1"));
            }
            response
        }
    }
}

/// On a failed answer: the site's Send was pressed, so the site may have the prompt. The gateway then tries no other web
/// session with it (Codex's fourth review, A8).
pub const SENT_HEADER: &str = "x-pygg-web-sent";

/// A request that failed, and whether its prompt had reached the site.
struct Failed {
    error: WebError,
    sent: bool,
}

impl From<WebError> for Failed {
    fn from(error: WebError) -> Failed {
        Failed { error, sent: false }
    }
}

/// The web guard's verdict for one request, read under the connection's lock and read again after any wait, so a call
/// that waited in line sees what happened meanwhile: the connection turned off, a suspension, a back-off, the day's
/// requests used (Codex's fourth review, A3 and A4). A record Pygg cannot read refuses: no pace known, nothing sent.
async fn admit(service: &Service, provider_id: &str, weight: u32) -> Result<(Provider, &'static Recipe), WebError> {
    let unreadable = || WebError::Pace { retry_after: 60, why: GUARD_UNREADABLE };
    for round in 0..3 {
        let (provider, recipe) = service.web_provider(provider_id)?;
        if !provider.enabled {
            return Err(WebError::BadRequest(format!("Not sent: {CONNECTION_OFF}")));
        }
        let now = crate::db::now_ms();
        let limits = guard::limits(&provider.kind);
        let limit = limits.daily_limit(provider.web_daily_limit.unwrap_or(DEFAULT_DAILY_LIMIT));
        let used = service.with_db(|db| store::web_requests_today(db, provider_id, now)).map_err(|_| unreadable())?;
        if used >= u64::from(limit) {
            let retry_after = ((86_400_000 - now.rem_euclid(86_400_000)) / 1000).max(1) as u64;
            return Err(WebError::DailyLimit { limit, retry_after });
        }
        let account = service.with_db(|db| store::web_guard(db, provider_id, now)).map_err(|_| unreadable())?;
        match guard::decide(&account, weight, now, limit, &limits) {
            guard::Verdict::Refuse { retry_after_ms, why } => {
                return Err(WebError::Pace { retry_after: ((retry_after_ms + 999) / 1000).max(1) as u64, why })
            }
            guard::Verdict::Go { wait_ms } if wait_ms > 0 && round < 2 => {
                tokio::time::sleep(Duration::from_millis(wait_ms as u64)).await
            }
            guard::Verdict::Go { .. } => return Ok((provider, recipe)),
        }
    }
    Err(WebError::Pace { retry_after: 30, why: guard::WHY_REST })
}

async fn start(service: &Arc<Service>, provider_id: &str, body: &[u8]) -> Result<Response, Failed> {
    let body: Value = serde_json::from_slice(body).map_err(|_| WebError::BadRequest("The body is not JSON".into()))?;
    let (provider, recipe) = service.web_provider(provider_id)?;
    if uses_tools(&body) {
        return Err(WebError::BadRequest(format!("Not sent: {NO_TOOLS}")).into());
    }
    let prompt = flatten(&body).map_err(WebError::BadRequest)?;
    let model = body["model"].as_str().unwrap_or(recipe.models[0]).to_string();
    let stream = body["stream"] == true;
    // The web guard: a prompt no person would type into a chat box never reaches the site.
    let limits = guard::limits(&provider.kind);
    let chars = prompt.chars().count();
    if chars > limits.max_prompt_chars {
        return Err(WebError::TooLong { chars, max: limits.max_prompt_chars }.into());
    }

    // Pygg's own pace limit, before anything reaches the site.
    let now = crate::db::now_ms();
    let limit = limits.daily_limit(provider.web_daily_limit.unwrap_or(DEFAULT_DAILY_LIMIT));
    let used = service.with_db(|db| store::web_requests_today(db, provider_id, now)).unwrap_or(0);
    if used >= u64::from(limit) {
        let retry_after = ((86_400_000 - now.rem_euclid(86_400_000)) / 1000).max(1) as u64;
        return Err(WebError::DailyLimit { limit, retry_after }.into());
    }
    let mut turn = service.take_turn(provider_id).await?;
    // The web guard, while this request holds the connection: its rest, the hour's and the day's share by weight, and a
    // back-off after trouble. Refused here, nothing reaches the site and the caller gets a Retry-After.
    let weight = guard::weight(chars);
    let (provider, recipe) = admit(service, provider_id, weight).await?;
    // The request's weight and a first rest are kept before the site is touched (Codex's fourth review, A1).
    let sent_at = crate::db::now_ms();
    let gap = limits.rest_base(service.timing.gap.as_millis() as i64);
    let row = service
        .with_db(|db| {
            store::web_guard_reserve(db, provider_id, sent_at, weight, sent_at + guard::rest_ms(gap, weight, 0))
        })
        .map_err(|_| WebError::Pace { retry_after: 60, why: GUARD_UNREADABLE })?;
    let page = service.windows.page(&provider, recipe)?;
    let submitted = Arc::new(AtomicBool::new(false));
    let task_submitted = submitted.clone();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    let task_service = service.clone();
    let id = provider_id.to_string();
    tokio::spawn(async move {
        let text_tx = tx.clone();
        let mut on_text = move |piece: &str| {
            let _ = text_tx.send(Event::Text(piece.to_string()));
        };
        let result =
            converse_marked(page.as_ref(), recipe, &prompt, &task_service.timing, &mut on_text, &task_submitted).await;
        // The page again 90 seconds later, in the app log: whether a picture or video the site was still making turned
        // up after the reply ended (owner's report, 2026-11-18: Muse's videos came back as its mascot). Counts and markup
        // only, as after the reply.
        if result.is_ok() {
            let (later, preset) = (page.clone(), recipe.preset);
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(90)).await;
                if let Ok(probe) = later.eval(media_probe_script(recipe)).await {
                    crate::applog::info(&format!("web page on {preset}, 90 s after the reply: {probe}"));
                }
            });
        }
        match &result {
            Ok(_) => task_service.record(&id, "signed_in", SIGNED_IN),
            Err(e) => {
                if let Some((state, reason)) = e.state() {
                    task_service.record(&id, state, reason)
                }
            }
        }
        // The web guard keeps the request's weight, rests the account as long as a person would read the reply, and
        // backs off after a sign of trouble (or clears the back-off after a clean reply).
        let reply_chars = result.as_ref().map(|r| r.text.chars().count()).unwrap_or(0);
        let ended = crate::db::now_ms();
        let rest = guard::rest_ms(limits.rest_base(task_service.timing.gap.as_millis() as i64), weight, reply_chars);
        let trouble = result.as_ref().err().and_then(WebError::trouble);
        let backoff = task_service.timing.backoff.as_millis() as i64;
        let _ = task_service.with_db(|db| {
            store::web_guard_after(
                db,
                &id,
                row,
                if result.is_ok() { "ok" } else { "error" },
                ended + rest,
                trouble.as_ref(),
                backoff,
                ended,
            )
        });
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
                Event::Done(Ok(reply)) => {
                    let tail = reply_tail(&reply);
                    text = reply.text + &tail;
                    break;
                }
                Event::Done(Err(e)) => return Err(Failed { error: e, sent: submitted.load(Ordering::SeqCst) }),
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
        Event::Done(Err(e)) => return Err(Failed { error: e, sent: submitted.load(Ordering::SeqCst) }),
        Event::Done(Ok(reply)) => {
            let tail = reply_tail(&reply);
            let text = reply.text + &tail;
            pending.push(chunk(&completion_id, &model, json!({ "content": text }), None));
            finished = true;
        }
        Event::Text(text) => pending.push(chunk(&completion_id, &model, json!({ "content": text }), None)),
    }
    let tail = futures_util::stream::unfold(
        (rx, finished, completion_id.clone(), model.clone()),
        |(mut rx, finished, id, model)| async move {
            if finished {
                return None;
            }
            let out = match rx.recv().await {
                Some(Event::Text(text)) => (chunk(&id, &model, json!({ "content": text }), None), false),
                Some(Event::Done(Ok(reply))) => {
                    let note = reply_tail(&reply);
                    let mut out = String::new();
                    if !note.is_empty() {
                        out.push_str(
                            std::str::from_utf8(&chunk(&id, &model, json!({ "content": note }), None))
                                .unwrap_or_default(),
                        );
                    }
                    out.push_str(std::str::from_utf8(&chunk(&id, &model, json!({}), Some("stop"))).unwrap_or_default());
                    out.push_str("data: [DONE]\n\n");
                    (Bytes::from(out), true)
                }
                Some(Event::Done(Err(e))) => (
                    Bytes::from(format!(
                        "data: {}\n\n",
                        json!({ "error": { "message": e.message(), "type": "api_error" } })
                    )),
                    true,
                ),
                None => return None,
            };
            Some((Ok::<Bytes, std::io::Error>(out.0), (rx, out.1, id, model)))
        },
    );
    let head = if finished {
        pending.push(chunk(&completion_id, &model, json!({}), Some("stop")));
        pending.push(Bytes::from_static(b"data: [DONE]\n\n"));
        futures_util::stream::iter(pending.into_iter().map(Ok::<Bytes, std::io::Error>))
    } else {
        futures_util::stream::iter(pending.into_iter().map(Ok::<Bytes, std::io::Error>))
    };
    use futures_util::StreamExt;
    let body = Body::from_stream(head.chain(tail));
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(body)
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()))
}

#[cfg(test)]
mod tests;
