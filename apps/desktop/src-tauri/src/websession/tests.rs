//! The web-session safeguards (ADR-035), with a scripted page in place of the site's window: the conversation loop,
//! the loopback endpoint, the gateway's routing and ledger, the store's rules and the profile folders.

use super::*;
use crate::gateway::{self, store, Shared};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};

fn fast() -> Timing {
    Timing { poll: Duration::from_millis(1), ready: Duration::from_millis(150), send: Duration::from_millis(30),
        first_word: Duration::from_millis(200), first_word_per_100_tokens: Duration::from_millis(30),
        first_word_cap: Duration::from_secs(1), reply: Duration::from_secs(2), stable: 2, stable_busy: 6,
        queue: Duration::from_millis(100), gap: Duration::ZERO, backoff: Duration::ZERO }
}

fn muse() -> &'static Recipe {
    recipe("muse-web").unwrap()
}

/// A page that answers each kind of script from its own list; the last answer of a list repeats.
#[derive(Default)]
struct FakePage {
    states: Mutex<VecDeque<Value>>,
    reads: Mutex<VecDeque<Value>>,
    send: Mutex<Option<Value>>,
    /// How long each read takes, to hold the connection busy.
    read_delay: Duration,
    filled: Mutex<Option<String>>,
    navigations: AtomicUsize,
    denials: AtomicUsize,
}

impl FakePage {
    fn new(states: Vec<Value>, reads: Vec<Value>) -> Arc<FakePage> {
        Arc::new(FakePage { states: Mutex::new(states.into()), reads: Mutex::new(reads.into()), ..FakePage::default() })
    }

    fn next(list: &Mutex<VecDeque<Value>>) -> Value {
        let mut list = list.lock().unwrap();
        if list.len() > 1 { list.pop_front().unwrap() } else { list.front().cloned().unwrap_or(Value::Null) }
    }
}

impl Page for FakePage {
    fn navigate(&self, _url: &str) -> BoxFuture<'_, Result<(), WebError>> {
        self.navigations.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }

    fn eval(&self, script: String) -> BoxFuture<'_, Result<Value, WebError>> {
        Box::pin(async move {
            Ok(if script.contains("__pyggOld = true") {
                json!(true)
            } else if script.contains("hasInput") {
                FakePage::next(&self.states)
            } else if script.contains("execCommand") {
                // The prompt is the JSON string the script quotes; keep it as the page would see it.
                let start = script.find(".set.call(box, ").unwrap() + ".set.call(box, ".len();
                let end = script[start..].find(");\n").unwrap();
                *self.filled.lock().unwrap() = serde_json::from_str(&script[start..start + end]).ok();
                json!({ "ok": true })
            } else if script.contains("'clicked'") {
                self.send.lock().unwrap().clone().unwrap_or(json!("clicked"))
            } else if script.contains("denied++") {
                self.denials.fetch_add(1, Ordering::SeqCst);
                json!(1)
            } else if script.contains("replies.length") {
                if !self.read_delay.is_zero() {
                    tokio::time::sleep(self.read_delay).await;
                }
                FakePage::next(&self.reads)
            } else {
                Value::Null
            })
        })
    }
}

fn ready() -> Value {
    json!({ "fresh": true, "ready": true, "hasInput": true })
}

fn read(count: u64, text: &str, busy: bool) -> Value {
    json!({ "count": count, "text": text, "busy": busy })
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().unwrap()
}

// ---------- The conversation ----------

#[test]
fn a_reply_streams_as_it_appears_and_ends_when_it_stops_changing() {
    rt().block_on(async {
        let page = FakePage::new(vec![ready()], vec![read(2, "an old reply", false), read(3, "Hel", true),
            read(3, "Hello", true), read(3, "Hello", false)]);
        let mut pieces = Vec::new();
        let reply = converse(page.as_ref(), muse(), "Say \"hello\" </script>", &fast(), &mut |p: &str| pieces.push(p.to_string()))
            .await.unwrap();
        assert_eq!(reply, Reply { text: "Hello".into(), denied: 0 });
        assert_eq!(pieces, ["Hel", "lo"]);
        // The prompt reached the page exactly, quoted as data, not as script.
        assert_eq!(page.filled.lock().unwrap().as_deref(), Some("Say \"hello\" </script>"));
        assert_eq!(page.navigations.load(Ordering::SeqCst), 1, "a new chat for every request");
    });
}

#[test]
fn the_sites_requests_to_act_are_denied_and_counted() {
    rt().block_on(async {
        let page = FakePage::new(vec![ready()], vec![read(0, "", false),
            json!({ "count": 1, "text": "I can send that email", "approval": true }), read(1, "Done, without sending.", false)]);
        let reply = converse(page.as_ref(), muse(), "hi", &fast(), &mut |_: &str| {}).await.unwrap();
        assert_eq!(reply.denied, 1);
        assert_eq!(page.denials.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn a_check_on_the_page_stops_and_asks_for_the_user() {
    rt().block_on(async {
        let page = FakePage::new(vec![json!({ "fresh": true, "challenge": true })], vec![]);
        let error = converse(page.as_ref(), muse(), "hi", &fast(), &mut |_: &str| {}).await.unwrap_err();
        assert_eq!(error, WebError::NeedsYou { state: "needs_you", reason: CHALLENGE });
        assert!(page.filled.lock().unwrap().is_none(), "nothing typed into a page showing a check");

        // A check that appears while the reply is being read stops it too.
        let page = FakePage::new(vec![ready()], vec![read(0, "", false), json!({ "count": 1, "text": "x", "challenge": true })]);
        let error = converse(page.as_ref(), muse(), "hi", &fast(), &mut |_: &str| {}).await.unwrap_err();
        assert_eq!(error.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(error.state(), Some(("needs_you", CHALLENGE)));
    });
}

#[test]
fn a_suspended_account_is_not_taken_for_signed_out_and_its_connection_goes_off() {
    rt().block_on(async {
        // DeepSeek's notice of 2026-11-13: the prompt box is gone, so before this it read as "signed out".
        let page = FakePage::new(vec![json!({ "fresh": true, "signIn": true, "restricted": true })], vec![]);
        let error = converse(page.as_ref(), muse(), "hi", &fast(), &mut |_: &str| {}).await.unwrap_err();
        assert_eq!(error, WebError::NeedsYou { state: "needs_you", reason: RESTRICTED });
        assert!(page.filled.lock().unwrap().is_none(), "nothing typed into a suspended account");
    });
    assert!(state_script(muse()).contains("violation of user policies"));
    let (conn, muse_conn) = web_store();
    store::update_provider(&conn, &muse_conn.id, Some(true), None, None).unwrap();
    store::set_web_state(&conn, &muse_conn.id, "needs_you", RESTRICTED).unwrap();
    let after = store::get_provider(&conn, &muse_conn.id).unwrap().unwrap();
    assert!(!after.enabled, "turned off until the user turns it back on");
    assert_eq!(after.web_state.as_deref(), Some("needs_you"));
}

#[test]
fn a_signed_out_or_changed_page_says_which() {
    rt().block_on(async {
        let page = FakePage::new(vec![json!({ "fresh": true, "signIn": true })], vec![]);
        let error = converse(page.as_ref(), muse(), "hi", &fast(), &mut |_: &str| {}).await.unwrap_err();
        assert_eq!(error, WebError::NeedsYou { state: "signed_out", reason: SIGNED_OUT });

        // The old document still answering "ready" is not taken for the new chat.
        let page = FakePage::new(vec![json!({ "fresh": false, "ready": true, "hasInput": true })], vec![]);
        let error = converse(page.as_ref(), muse(), "hi", &fast(), &mut |_: &str| {}).await.unwrap_err();
        assert_eq!(error, WebError::PageChanged(NO_PROMPT_BOX));

        let page = FakePage::new(vec![ready()], vec![read(0, "", false)]);
        *page.send.lock().unwrap() = Some(json!("missing"));
        let error = converse(page.as_ref(), muse(), "hi", &fast(), &mut |_: &str| {}).await.unwrap_err();
        assert_eq!(error, WebError::PageChanged(NO_SEND_BUTTON));
    });
}

#[test]
fn no_reply_or_a_usage_limit_is_an_error_not_an_answer() {
    rt().block_on(async {
        let page = FakePage::new(vec![ready()], vec![read(0, "", false)]);
        let error = converse(page.as_ref(), muse(), "hi", &fast(), &mut |_: &str| {}).await.unwrap_err();
        assert_eq!(error, WebError::Timeout(NO_REPLY));

        let page = FakePage::new(vec![ready()], vec![read(0, "", false), read(1, "You have reached your usage limit.", false)]);
        let error = converse(page.as_ref(), muse(), "hi", &fast(), &mut |_: &str| {}).await.unwrap_err();
        assert_eq!(error, WebError::Quota);
    });
}

#[test]
fn the_first_word_wait_grows_with_the_prompt_and_is_capped() {
    let t = Timing::default();
    assert_eq!(t.first_word_wait("hi"), Duration::from_secs(60));
    // About 6,000 tokens (18,000 ASCII characters at three a token): a minute more.
    let long = "x".repeat(18_000);
    assert_eq!(t.first_word_wait(&long), Duration::from_secs(120));
    assert_eq!(t.reply_wait(&long), Duration::from_secs(360));
    // Claude Code's 90,000-token context, and more: never past the cap.
    assert_eq!(t.first_word_wait(&"x".repeat(270_000)), Duration::from_secs(240));
    assert_eq!(t.first_word_wait(&"x".repeat(2_000_000)), Duration::from_secs(240));
    assert_eq!(t.reply_wait(&"x".repeat(2_000_000)), Duration::from_secs(480));
}

#[test]
fn a_page_that_thinks_past_the_first_word_wait_is_waited_for() {
    rt().block_on(async {
        // Each read takes 100 ms, so the third empty read is past the 200 ms first-word wait of a short prompt.
        let slow = |reads: Vec<Value>| Arc::new(FakePage { states: Mutex::new(vec![ready()].into()), reads: Mutex::new(reads.into()),
            read_delay: Duration::from_millis(100), ..FakePage::default() });
        // The page shows it is answering (its "thinking" state): that is its first word, and the reply comes.
        let page = slow(vec![read(0, "", false), read(0, "", true), read(0, "", true), read(0, "", true), read(1, "Hello", false)]);
        let reply = converse(page.as_ref(), muse(), "hi", &fast(), &mut |_: &str| {}).await.unwrap();
        assert_eq!(reply.text, "Hello");
        // The same page showing nothing: a short prompt gives up, a long one (about 1,000 tokens: 300 ms more) waits.
        let silent = || vec![read(0, "", false), read(0, "", false), read(0, "", false), read(0, "", false), read(1, "Hello", false)];
        let error = converse(slow(silent()).as_ref(), muse(), "hi", &fast(), &mut |_: &str| {}).await.unwrap_err();
        assert_eq!(error, WebError::Timeout(NO_REPLY));
        let long = "x".repeat(3000);
        let reply = converse(slow(silent()).as_ref(), muse(), &long, &fast(), &mut |_: &str| {}).await.unwrap();
        assert_eq!(reply.text, "Hello");
    });
}

#[test]
fn a_visitors_chat_is_not_taken_for_the_users_account() {
    rt().block_on(async {
        // ChatGPT, Perplexity and Grok chat with visitors: the prompt box is there, and so are their Log in buttons.
        let chatgpt = recipe("chatgpt-web").unwrap();
        let page = FakePage::new(vec![json!({ "fresh": true, "ready": true, "hasInput": true, "signedOut": true })], vec![]);
        let error = converse(page.as_ref(), chatgpt, "hi", &fast(), &mut |_: &str| {}).await.unwrap_err();
        assert_eq!(error, WebError::NeedsYou { state: "signed_out", reason: SIGNED_OUT });
        assert!(page.filled.lock().unwrap().is_none(), "nothing typed into a visitor's chat");
    });
}

#[test]
fn enter_sends_where_the_site_has_no_labelled_send_button() {
    rt().block_on(async {
        let deepseek = recipe("deepseek-web").unwrap();
        assert!(send_script(deepseek).contains("'entered'"));
        assert!(send_script(muse()).contains("const box = false"), "Muse keeps its Send button");
        let page = FakePage::new(vec![ready()], vec![read(0, "", false), read(1, "Hello", false)]);
        *page.send.lock().unwrap() = Some(json!("entered"));
        let reply = converse(page.as_ref(), deepseek, "hi", &fast(), &mut |_: &str| {}).await.unwrap();
        assert_eq!(reply.text, "Hello");
    });
}

#[test]
fn without_a_busy_signal_a_reply_must_stay_still_longer() {
    rt().block_on(async {
        // A pause in the middle of an answer: with a "stop" button that is gone, two still reads end it (Muse); a site
        // without one (DeepSeek) waits for `stable_busy` still reads, and gets the rest.
        let reads = || vec![read(0, "", false), read(1, "Hel", false), read(1, "Hel", false), read(1, "Hel", false),
            read(1, "Hello", false)];
        let page = FakePage::new(vec![ready()], reads());
        assert_eq!(converse(page.as_ref(), muse(), "hi", &fast(), &mut |_: &str| {}).await.unwrap().text, "Hel");
        let page = FakePage::new(vec![ready()], reads());
        let deepseek = recipe("deepseek-web").unwrap();
        assert_eq!(converse(page.as_ref(), deepseek, "hi", &fast(), &mut |_: &str| {}).await.unwrap().text, "Hello");
    });
}

#[test]
fn every_web_preset_has_a_recipe_and_every_recipe_a_preset() {
    for r in RECIPES {
        let preset = crate::gateway::presets::find(r.preset).unwrap_or_else(|| panic!("{} has no preset", r.preset));
        assert!(preset.web && preset.web_acts == r.acts, "{}", r.preset);
        assert!(!r.models.is_empty() && host_allowed(r, r.new_chat_url) && host_allowed(r, &format!("{}/", r.site)), "{}", r.preset);
        // Only Muse's agent asks for approvals, and only a site with approvals needs a deny button.
        assert_eq!(r.approval.is_some(), r.acts, "{}", r.preset);
    }
}

#[test]
fn each_site_reaches_its_own_sign_in_and_nothing_else() {
    let cases: &[(&str, &[&str], &[&str])] = &[
        ("chatgpt-web", &["https://chatgpt.com/", "https://auth.openai.com/log-in", "https://accounts.google.com/o/oauth2/auth?x=1",
            "https://appleid.apple.com/auth/authorize", "https://login.live.com/oauth20_authorize.srf",
            "https://login.microsoftonline.com/common/oauth2/v2.0/authorize"],
            &["https://chat.openai.com.evil.example/", "https://grok.com/", "https://muse.ai/"]),
        ("perplexity-web", &["https://www.perplexity.ai/", "https://perplexity.ai/api/auth/signin", "https://accounts.google.com/x",
            "https://appleid.apple.com/x"], &["https://chatgpt.com/", "https://evilperplexity.ai/"]),
        ("deepseek-web", &["https://chat.deepseek.com/sign_in", "https://accounts.google.com/x", "https://appleid.apple.com/x"],
            &["https://chatgpt.com/", "https://deepseek.com.evil.example/"]),
        ("grok-web", &["https://grok.com/sign-in", "https://accounts.x.ai/sign-in", "https://x.com/i/oauth2/authorize",
            "https://api.twitter.com/oauth/authorize", "https://accounts.google.com/x"], &["https://chatgpt.com/", "http://grok.com/"]),
    ];
    for (preset, allowed, blocked) in cases {
        let r = recipe(preset).unwrap();
        for url in *allowed {
            assert!(host_allowed(r, url), "{preset} must reach {url}");
        }
        for url in *blocked {
            assert!(!host_allowed(r, url), "{preset} must not reach {url}");
        }
    }
    // A site's window never reaches another site, so one profile never signs in to two of them.
    for a in RECIPES {
        for b in RECIPES.iter().filter(|b| b.preset != a.preset) {
            assert!(!host_allowed(a, b.new_chat_url), "{} reaches {}", a.preset, b.preset);
        }
    }
}

#[test]
fn every_new_web_preset_starts_off_with_its_one_model_and_no_tools() {
    let conn = crate::db::open_in_memory().unwrap();
    for (preset, id, model) in [("chatgpt-web", "chatgpt", "chatgpt"), ("perplexity-web", "perplexity", "perplexity"),
        ("deepseek-web", "deepseek", "deepseek"), ("grok-web", "grok", "grok")] {
        let p = store::add_web_provider(&conn, preset, "", WARNING_VERSION).unwrap();
        assert_eq!((p.id.as_str(), p.enabled, p.default_billing.as_str()), (id, false, "free"));
        let models = store::list_models(&conn, Some(&p.id)).unwrap();
        assert_eq!(models.iter().map(|m| m.model_id.as_str()).collect::<Vec<_>>(), [model]);
        assert_eq!(models[0].capabilities.supports_tools, Some(false));
    }
}

// ---------- Requests ----------

#[test]
fn tools_in_any_api_shape_are_found() {
    assert!(uses_tools(&json!({ "tools": [{ "type": "function" }], "messages": [] })));
    assert!(uses_tools(&json!({ "tool_choice": "required", "messages": [] })));
    assert!(uses_tools(&json!({ "messages": [{ "role": "assistant", "tool_calls": [{ "id": "1" }] }] })));
    assert!(uses_tools(&json!({ "messages": [{ "role": "tool", "content": "42" }] })));
    assert!(uses_tools(&json!({ "messages": [{ "role": "user", "content": [{ "type": "tool_result", "content": "x" }] }] })));
    assert!(uses_tools(&json!({ "input": [{ "type": "function_call_output", "output": "x" }] })));
    assert!(!uses_tools(&json!({ "tools": [], "tool_choice": "none", "messages": [{ "role": "user", "content": "hi" }] })));
}

#[test]
fn a_conversation_is_flattened_into_one_prompt() {
    assert_eq!(flatten(&json!({ "messages": [{ "role": "user", "content": "hi" }] })).unwrap(), "hi");
    let prompt = flatten(&json!({ "messages": [{ "role": "system", "content": "Be brief." },
        { "role": "user", "content": [{ "type": "text", "text": "2+2?" }] }] })).unwrap();
    assert!(prompt.starts_with("[System]\nBe brief.\n\n[User]\n2+2?\n\n[Instruction]"), "{prompt}");
    assert!(flatten(&json!({ "messages": [{ "role": "user", "content": [{ "type": "image_url", "image_url": { "url": "x" } }] }] }))
        .unwrap_err().contains("text only"));
    assert!(flatten(&json!({ "messages": [] })).is_err());
}

#[test]
fn the_window_goes_only_to_the_sites_hosts() {
    let r = muse();
    assert!(host_allowed(r, "https://muse.ai/thread/new"));
    assert!(host_allowed(r, "https://www.facebook.com/login"));
    assert!(host_allowed(r, "https://auth.meta.com/x?y=1"));
    assert!(host_allowed(r, "https://www.instagram.com/aymh/redirect-cycle/?t=6"), "Meta's sign-in passes through Instagram");
    assert!(!host_allowed(r, "http://muse.ai/"), "HTTPS only");
    assert!(!host_allowed(r, "https://muse.ai.evil.com/"));
    // Google's country sign-in hosts, for recipes that sign in with Google, and nothing else of that shape.
    let chatgpt = recipe("chatgpt-web").unwrap();
    for ok in ["https://accounts.google.com.vn/accounts/SetSID", "https://accounts.google.co.jp/x", "https://accounts.google.de/x"] {
        assert!(host_allowed(chatgpt, ok), "{ok}");
    }
    for no in ["https://accounts.google.evil.com/", "https://accounts.google.com.vn.evil.com/", "https://www.google.com.vn/",
        "https://accounts.google.c0m/", "https://accounts.google.com.vnn/"] {
        assert!(!host_allowed(chatgpt, no), "{no}");
    }
    assert!(!host_allowed(r, "https://accounts.google.com.vn/accounts/SetSID"), "Muse does not sign in with Google");
    assert!(host_allowed(recipe("grok-web").unwrap(), "https://auth.grokusercontent.com/set-cookie"), "Grok's sign-in cookie hop");
    assert!(!host_allowed(r, "https://evilmuse.ai/"));
    assert!(!host_allowed(r, "https://muse.ai@evil.com/"));
    assert!(!host_allowed(r, "https://muse.ai:8443/"));
    assert!(!host_allowed(r, "https://example.com/"));
}

#[test]
fn no_script_reads_cookies_storage_or_the_network() {
    for r in RECIPES {
        let scripts = [mark_script(), state_script(r), fill_script(r, "x"), send_script(r), read_script(r), deny_script(r)];
        for script in scripts {
            let lower = script.to_lowercase();
            for forbidden in ["cookie", "localstorage", "sessionstorage", "indexeddb", "fetch(", "xmlhttprequest", "websocket",
                "credentials", "password"] {
                assert!(!lower.contains(forbidden), "{} script uses {forbidden}: {script}", r.preset);
            }
        }
    }
}

#[test]
fn errors_carry_pyggs_words_never_the_pages() {
    // Every state reason the store can be given comes from these constants.
    let errors = [WebError::NeedsYou { state: "needs_you", reason: CHALLENGE }, WebError::NeedsYou { state: "signed_out", reason: SIGNED_OUT },
        WebError::PageChanged(NO_PROMPT_BOX), WebError::PageChanged(NO_SEND_BUTTON)];
    for e in errors {
        let (state, reason) = e.state().unwrap();
        assert!(STATES.contains(&state));
        assert!([CHALLENGE, SIGNED_OUT, NO_PROMPT_BOX, NO_SEND_BUTTON].contains(&reason));
    }
}

// ---------- Profile folders ----------

#[test]
fn a_removed_connections_profile_is_deleted_and_nothing_else() {
    let root = tempfile::tempdir().unwrap();
    let keep = "p-0123456789abcdef0123456789abcdef".to_string();
    let gone = "p-fedcba9876543210fedcba9876543210".to_string();
    for name in [&keep, &gone, &"notes".to_string()] {
        std::fs::create_dir_all(root.path().join(name).join("EBWebView")).unwrap();
        std::fs::write(root.path().join(name).join("EBWebView").join("Cookies"), "x").unwrap();
    }
    assert_eq!(sweep(root.path(), &[keep.clone()]), 1);
    assert!(root.path().join(&keep).exists());
    assert!(!root.path().join(&gone).exists());
    assert!(root.path().join("notes").exists(), "a folder Pygg did not make is never deleted");

    assert!(remove_profile(root.path(), &keep));
    assert!(!root.path().join(&keep).exists());
    assert!(profile_dir(root.path(), "../p-0123456789abcdef0123456789abcdef").is_none());
    assert!(profile_dir(root.path(), "p-xyz").is_none());
}

// ---------- The store ----------

fn web_store() -> (Connection, Provider) {
    let conn = crate::db::open_in_memory().unwrap();
    let provider = store::add_web_provider(&conn, "muse-web", "Muse", WARNING_VERSION).unwrap();
    (conn, provider)
}

#[test]
fn a_web_session_needs_the_warning_and_starts_off_free_and_without_tools() {
    let conn = crate::db::open_in_memory().unwrap();
    assert!(store::add_web_provider(&conn, "muse-web", "Muse", WARNING_VERSION - 1).is_err());
    assert!(store::add_web_provider(&conn, "openai", "x", WARNING_VERSION).is_err());
    assert!(store::add_provider(&conn, &store::NewProvider { kind: "muse-web", label: "Muse", endpoint: "https://muse.ai",
        credential_ref: None, default_billing: "free", wire: "openai" }).is_err(), "no way around the warning");

    let (conn, muse) = web_store();
    assert!(!muse.enabled, "off until the user signs in and turns it on");
    assert_eq!(muse.default_billing, "free", "$0 a request by default (2026-11-13)");
    assert_eq!((muse.web_ack_version, muse.web_state.as_deref()), (Some(WARNING_VERSION), Some("signed_out")));
    assert!(muse.credential_ref.is_none(), "no key, no password");
    let profile = muse.web_profile.clone().unwrap();
    assert!(profile_dir(Path::new("x"), &profile).is_some());
    let models = store::list_models(&conn, Some(&muse.id)).unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!((models[0].capabilities.supports_tools, models[0].capabilities.supports_vision), (Some(false), Some(false)));
    assert_eq!(models[0].capabilities.tier, None, "no tier, so no auto either");

    // A second connection never shares the first one's folder.
    let other = store::add_web_provider(&conn, "muse-web", "Muse", WARNING_VERSION).unwrap();
    assert_ne!(other.web_profile, muse.web_profile);
    assert_eq!(store::web_profiles(&conn).unwrap().len(), 2);

    assert!(store::update_provider(&conn, &muse.id, None, Some("paid"), None).is_err(), "never paid per token");
    assert!(store::update_provider(&conn, &muse.id, None, Some("local"), None).is_err());
    store::update_provider(&conn, &muse.id, Some(true), Some("subscription"), Some(true)).unwrap();
    // Accounts of one site pool together (owner, 2026-11-13), each confirmed as its own account.
    store::set_pool(&conn, &muse.id, Some("muses")).unwrap();
    assert!(store::set_pool(&conn, &other.id, Some("muses")).unwrap_err().contains("own provider quota"));
    store::update_provider(&conn, &other.id, Some(true), Some("subscription"), Some(true)).unwrap();
    store::set_pool(&conn, &other.id, Some("muses")).unwrap();

    // The pool sends to the account used least today: what reached the site, the web guard's reservations. A refusal
    // Pygg made itself never got one.
    let sent = |p: &str| { store::web_guard_reserve(&conn, p, crate::db::now_ms(), 1, 0).unwrap(); };
    let order = || store::resolve(&conn, "muses/muse").unwrap().into_iter().map(|(p, _)| p.id).collect::<Vec<_>>();
    assert_eq!(order(), vec![muse.id.clone(), other.id.clone()], "a tie keeps the pool's order");
    sent(&muse.id);
    sent(&muse.id);
    assert_eq!(store::web_requests_today(&conn, &muse.id, crate::db::now_ms()).unwrap(), 2);
    assert_eq!(store::web_requests_today(&conn, &other.id, crate::db::now_ms()).unwrap(), 0);
    assert_eq!(order(), vec![other.id.clone(), muse.id.clone()], "the account used least today goes first");

    // The daily limit: 1 to 1,000, or Pygg's default.
    assert!(store::set_web_daily_limit(&conn, &muse.id, Some(0)).is_err());
    assert!(store::set_web_daily_limit(&conn, &muse.id, Some(1001)).is_err());
    store::set_web_daily_limit(&conn, &muse.id, Some(2)).unwrap();
    assert_eq!(store::get_provider(&conn, &muse.id).unwrap().unwrap().web_daily_limit, Some(2));
    store::set_web_daily_limit(&conn, &muse.id, None).unwrap();
    assert_eq!(store::get_provider(&conn, &muse.id).unwrap().unwrap().web_daily_limit, None);
}

#[test]
fn signing_in_clears_the_hold_a_signed_out_session_left() {
    let (conn, muse) = web_store();
    store::mark_health(&conn, &muse.id, "", "invalid_credentials", SIGNED_OUT, Some(401), None).unwrap();
    assert!(store::blocking_health(&conn, &muse.id, "muse").unwrap().is_some());
    store::set_web_state(&conn, &muse.id, "signed_in", SIGNED_IN).unwrap();
    assert!(store::blocking_health(&conn, &muse.id, "muse").unwrap().is_none());
    assert!(store::set_web_state(&conn, &muse.id, "something", "x").is_err());
}

#[test]
fn diagnostics_never_name_a_profile_folder() {
    let (conn, muse) = web_store();
    let context = crate::diagnostics::Context { made_by: "app", gateway_state: "running".into(), gateway_port: 8741,
        gateway_error: None, log_lines: vec![] };
    let files = crate::diagnostics::files(&conn, &context, 0).unwrap();
    let profile = muse.web_profile.unwrap();
    assert!(files.iter().all(|(_, text)| !text.contains(&profile)));
    // Nor does the connection the interface sees.
    assert!(!serde_json::to_string(&store::get_provider(&conn, &muse.id).unwrap().unwrap()).unwrap().contains(&profile));
}

// ---------- The endpoint and the gateway ----------

struct FakeWindows {
    page: Arc<FakePage>,
    forgotten: Mutex<Vec<String>>,
    signed_in: Mutex<Vec<String>>,
}

impl Windows for FakeWindows {
    fn page(&self, _provider: &Provider, _recipe: &'static Recipe) -> Result<Arc<dyn Page>, WebError> {
        Ok(self.page.clone())
    }
    fn show(&self, _provider: &Provider, _recipe: &'static Recipe) -> Result<(), String> {
        Ok(())
    }
    fn forget(&self, _provider_id: &str, profile: &str) {
        self.forgotten.lock().unwrap().push(profile.to_string());
    }
    fn signed_in(&self, provider_id: &str) {
        self.signed_in.lock().unwrap().push(provider_id.to_string());
    }
}

struct NoSecrets;

impl gateway::Secrets for NoSecrets {
    fn read(&self, _reference: &str) -> Result<Option<String>, String> {
        Ok(None)
    }
}

struct Rig {
    db: Arc<Mutex<Connection>>,
    link: Link,
    page: Arc<FakePage>,
    muse: Provider,
    http: reqwest::Client,
    windows: Arc<FakeWindows>,
    service: Arc<Service>,
}

async fn rig(page: Arc<FakePage>) -> Rig {
    let (conn, muse) = web_store();
    store::update_provider(&conn, &muse.id, Some(true), None, None).unwrap();
    let db = Arc::new(Mutex::new(conn));
    let windows = Arc::new(FakeWindows { page: page.clone(), forgotten: Mutex::default(), signed_in: Mutex::default() });
    let service = Arc::new(Service::new(db.clone(), windows.clone(), "web-token".into(), fast()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let link = Link { base: format!("http://{}", listener.local_addr().unwrap()), token: "web-token".into() };
    let service_ref = service.clone();
    tokio::spawn(serve(listener, service));
    Rig { db, link, page, muse, http: reqwest::Client::new(), windows, service: service_ref }
}

impl Rig {
    async fn post(&self, token: &str, body: Value) -> reqwest::Response {
        self.http.post(format!("{}/chat/completions", self.link.endpoint(&self.muse.id))).bearer_auth(token).json(&body)
            .send().await.unwrap()
    }

    fn muse(&self) -> Provider {
        store::get_provider(&self.db.lock().unwrap(), &self.muse.id).unwrap().unwrap()
    }
}

fn hello_page() -> Arc<FakePage> {
    FakePage::new(vec![ready()], vec![read(0, "", false), read(1, "Hi there", true), read(1, "Hi there", false)])
}

#[test]
fn the_endpoint_needs_its_token_and_refuses_tools_before_touching_the_page() {
    rt().block_on(async {
        let rig = rig(hello_page()).await;
        let body = json!({ "model": "muse", "messages": [{ "role": "user", "content": "hi" }] });
        assert_eq!(rig.post("wrong", body.clone()).await.status(), 401);
        let tools = json!({ "model": "muse", "messages": [{ "role": "user", "content": "hi" }], "tools": [{ "type": "function" }] });
        assert_eq!(rig.post("web-token", tools).await.status(), 400);
        assert_eq!(rig.page.navigations.load(Ordering::SeqCst), 0);

        let answer: Value = rig.post("web-token", body.clone()).await.json().await.unwrap();
        assert_eq!(answer["choices"][0]["message"]["content"], "Hi there");
        assert!(answer.get("usage").is_none(), "the site reports no usage, so none is made up");
        assert_eq!(rig.muse().web_state.as_deref(), Some("signed_in"));

        // A connection whose warning is not acknowledged in its current text is not driven.
        rig.db.lock().unwrap().execute("UPDATE providers SET web_ack_version = 0", []).unwrap();
        assert_eq!(rig.post("web-token", body).await.status(), 400);
    });
}

#[test]
fn the_web_guard_refuses_a_pasted_file_and_rests_after_trouble() {
    rt().block_on(async {
        let rig = rig(hello_page()).await;
        // A prompt no person would type into a chat box never reaches the site.
        let long = "x".repeat(guard::MAX_PROMPT_CHARS + 1);
        let response = rig.post("web-token", json!({ "model": "muse", "messages": [{ "role": "user", "content": long }] })).await;
        assert_eq!(response.status(), 413);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["error"]["code"], "prompt_too_long");
        assert_eq!(rig.page.navigations.load(Ordering::SeqCst), 0, "nothing reached the site");

        // A clean reply is kept with its weight; the account then rests.
        let ok = rig.post("web-token", json!({ "model": "muse", "messages": [{ "role": "user", "content": "hi" }] })).await;
        assert_eq!(ok.status(), 200);
        let now = crate::db::now_ms();
        let account = store::web_guard(&rig.db.lock().unwrap(), &rig.muse.id, now).unwrap();
        assert_eq!(account.sent.len(), 1);
        assert_eq!(account.sent[0].1, 1);

        // A back-off from earlier trouble refuses the next request with a Retry-After, before the page is touched.
        {
            let db = rig.db.lock().unwrap();
            store::web_guard_trouble(&db, &rig.muse.id, NO_REPLY, now, 60_000, now).unwrap();
        }
        let before = rig.page.navigations.load(Ordering::SeqCst);
        let refused = rig.post("web-token", json!({ "model": "muse", "messages": [{ "role": "user", "content": "hi" }] })).await;
        assert_eq!(refused.status(), 429);
        assert!(refused.headers().get("retry-after").is_some());
        let body: Value = refused.json().await.unwrap();
        assert_eq!(body["error"]["code"], "web_pace");
        assert_eq!(rig.page.navigations.load(Ordering::SeqCst), before);

        // The user lifts it; the next question goes to the page (this fake page has no second reply scripted).
        store::web_guard_resume(&rig.db.lock().unwrap(), &rig.muse.id).unwrap();
        let after = rig.post("web-token", json!({ "model": "muse", "messages": [{ "role": "user", "content": "hi" }] })).await;
        assert_ne!(after.status(), 429);
        assert!(rig.page.navigations.load(Ordering::SeqCst) > before, "the page was driven again");
    });
}

#[test]
fn the_endpoint_streams_the_reply() {
    rt().block_on(async {
        let rig = rig(hello_page()).await;
        let response = rig.post("web-token", json!({ "model": "muse", "stream": true,
            "messages": [{ "role": "user", "content": "hi" }] })).await;
        assert_eq!(response.status(), 200);
        let text = response.text().await.unwrap();
        assert!(text.contains("\"content\":\"Hi there\""), "{text}");
        assert!(text.trim_end().ends_with("data: [DONE]"), "{text}");
    });
}

#[test]
fn one_request_at_a_time_per_connection() {
    rt().block_on(async {
        let page = Arc::new(FakePage { states: Mutex::new(vec![ready()].into()),
            reads: Mutex::new(vec![read(0, "", false), read(1, "slow", false)].into()),
            read_delay: Duration::from_millis(60), ..FakePage::default() });
        let rig = rig(page).await;
        let body = json!({ "model": "muse", "messages": [{ "role": "user", "content": "hi" }] });
        let (a, b) = tokio::join!(rig.post("web-token", body.clone()), rig.post("web-token", body));
        let mut statuses = [a.status().as_u16(), b.status().as_u16()];
        statuses.sort();
        assert_eq!(statuses, [200, 429]);
        let busy = if a.status() == 429 { a } else { b };
        assert_eq!(busy.headers()[header::RETRY_AFTER], "30");
    });
}

#[test]
fn a_signed_out_session_is_recorded_in_pyggs_words() {
    rt().block_on(async {
        let rig = rig(FakePage::new(vec![json!({ "fresh": true, "signIn": true })], vec![])).await;
        let response = rig.post("web-token", json!({ "model": "muse", "messages": [{ "role": "user", "content": "hi" }] })).await;
        assert_eq!(response.status(), 401);
        let muse = rig.muse();
        assert_eq!((muse.web_state.as_deref(), muse.web_state_reason.as_deref()), (Some("signed_out"), Some(SIGNED_OUT)));
    });
}

/// The gateway in front of the endpoint, as the app runs them.
async fn gateway_rig(page: Arc<FakePage>, with_web: bool) -> (Rig, String, Shared) {
    let web = rig(page).await;
    let shared = Shared::new(web.db.clone(), Arc::new(NoSecrets));
    let shared = if with_web { shared.with_web(web.link.clone()) } else { shared };
    let (_, token) = shared.with_db(|db| store::create_client(db, "Tests", None)).unwrap();
    let server = gateway::start(shared.clone(), 0).await.unwrap();
    let base = format!("http://{}", server.addr);
    std::mem::forget(server);
    (web, format!("{base}|{token}"), shared)
}

async fn gateway_post(target: &str, body: Value) -> reqwest::Response {
    let (base, token) = target.split_once('|').unwrap();
    reqwest::Client::new().post(format!("{base}/v1/chat/completions")).bearer_auth(token).json(&body).send().await.unwrap()
}

#[test]
fn a_free_web_session_costs_nothing_even_when_the_request_fails() {
    use crate::gateway::pricing::Cost;
    let failed = || Cost::Unknown { source: "HTTP 504: the provider may have run the request".into() };
    match crate::gateway::quota_not_money(true, "free", failed()) {
        Cost::Estimated { micro_usd: 0, source } => assert!(source.starts_with("web session") && source.contains("504"), "{source}"),
        other => panic!("{other:?}"),
    }
    // A web session set to Subscription keeps saying quota, and a paid connection's unknown cost stays unknown.
    assert!(matches!(crate::gateway::quota_not_money(true, "subscription", failed()), Cost::Unknown { ref source } if source.starts_with("subscription")));
    assert!(matches!(crate::gateway::quota_not_money(false, "paid", failed()), Cost::Unknown { ref source } if source.starts_with("HTTP 504")));
}

#[test]
fn the_gateway_reaches_a_named_web_model_and_records_it_at_no_cost() {
    rt().block_on(async {
        let (web, gateway, shared) = gateway_rig(hello_page(), true).await;
        let response = gateway_post(&gateway, json!({ "model": "muse/muse", "messages": [{ "role": "user", "content": "hi" }] })).await;
        assert_eq!(response.status(), 200);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["choices"][0]["message"]["content"], "Hi there");
        let event = shared.with_db(|db| store::usage_summary(db, 0, 10)).unwrap().recent.remove(0);
        assert_eq!((event.provider.as_str(), event.model.as_str()), ("muse", "muse"));
        assert_eq!((event.cost_status.as_str(), event.cost_micro_usd), ("estimated", Some(0)), "$0 a request on the user's plan");
        assert!(event.cost_source.starts_with("web session"), "{}", event.cost_source);
        // The token that reaches the endpoint is never written anywhere.
        let ledger = serde_json::to_string(&shared.with_db(|db| store::usage_summary(db, 0, 10)).unwrap().recent).unwrap();
        assert!(!ledger.contains(&web.link.token));
    });
}

#[test]
fn auto_never_picks_a_web_session_and_tools_never_reach_one() {
    rt().block_on(async {
        let (web, gateway, _shared) = gateway_rig(hello_page(), true).await;
        // Even with a tier set by the user, so only the web-session rule keeps it out.
        web.db.lock().unwrap().execute("UPDATE models SET tier = 1, tier_source = 'user'", []).unwrap();
        for alias in ["auto", "auto/cheap"] {
            let response = gateway_post(&gateway, json!({ "model": alias, "messages": [{ "role": "user", "content": "hi" }] })).await;
            assert_eq!(response.status(), 422, "{alias}");
            assert!(response.text().await.unwrap().contains(NOT_FOR_AUTO));
        }
        let tools = json!({ "model": "muse/muse", "messages": [{ "role": "user", "content": "hi" }],
            "tools": [{ "type": "function", "function": { "name": "read", "parameters": {} } }] });
        let response = gateway_post(&gateway, tools).await;
        assert_eq!(response.status(), 400);
        assert!(response.text().await.unwrap().contains(NO_TOOLS));
        assert_eq!(web.page.navigations.load(Ordering::SeqCst), 0, "nothing reached the site");
    });
}

#[test]
fn without_the_app_a_web_session_is_skipped_with_the_reason() {
    rt().block_on(async {
        let (web, gateway, _shared) = gateway_rig(hello_page(), false).await;
        let response = gateway_post(&gateway, json!({ "model": "muse/muse", "messages": [{ "role": "user", "content": "hi" }] })).await;
        assert!(!response.status().is_success());
        assert!(response.text().await.unwrap().contains(APP_ONLY));
        assert_eq!(web.page.navigations.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn pygg_types_only_on_the_sites_own_page() {
    assert_eq!(site_host(recipe("chatgpt-web").unwrap()), "chatgpt.com");
    assert_eq!(site_host(recipe("perplexity-web").unwrap()), "perplexity.ai");
    assert_eq!(site_host(recipe("deepseek-web").unwrap()), "chat.deepseek.com");
    for r in RECIPES {
        let fill = fill_script(r, "x");
        let send = send_script(r);
        // The guard comes first, before the box is touched or a button is clicked.
        assert!(fill.find("offSite").unwrap() < fill.find("querySelector").unwrap(), "{}", r.preset);
        assert!(send.find("'offsite'").unwrap() < send.find("querySelectorAll").unwrap(), "{}", r.preset);
        assert!(fill.contains(&format!("\"{}\"", site_host(r))), "{}", r.preset);
    }
}

#[test]
fn the_daily_limit_refuses_before_the_site_is_touched() {
    rt().block_on(async {
        let rig = rig(hello_page()).await;
        {
            let conn = rig.db.lock().unwrap();
            store::set_web_daily_limit(&conn, &rig.muse.id, Some(1)).unwrap();
            store::web_guard_reserve(&conn, &rig.muse.id, crate::db::now_ms(), 1, 0).unwrap();
        }
        let response = rig.post("web-token", json!({ "model": "muse", "messages": [{ "role": "user", "content": "hi" }] })).await;
        assert_eq!(response.status(), 429);
        assert!(response.headers().get("retry-after").is_some(), "says when the limit resets");
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["error"]["code"], "daily_limit");
        assert_eq!(rig.page.navigations.load(Ordering::SeqCst), 0, "nothing reached the site");
    });
}

#[test]
fn a_check_that_finds_the_session_signed_in_hides_the_sign_in_window() {
    rt().block_on(async {
        let rig = rig(hello_page()).await;
        rig.service.check(&rig.muse.id).await.unwrap();
        assert_eq!(rig.muse().web_state.as_deref(), Some("signed_in"));
        assert_eq!(*rig.windows.signed_in.lock().unwrap(), vec![rig.muse.id.clone()]);
    });
}

// ---------- Codex's fourth review (2026-11-14) ----------

#[test]
fn a_suspension_notice_during_the_reply_turns_the_connection_off_and_marks_the_prompt_sent() {
    rt().block_on(async {
        let restricted = json!({ "count": 0, "text": "", "busy": false, "restricted": true });
        let rig = rig(FakePage::new(vec![ready()], vec![read(0, "", false), restricted])).await;
        let response = rig.post("web-token", json!({ "model": "muse", "messages": [{ "role": "user", "content": "hi" }] })).await;
        assert_eq!(response.status(), 401);
        assert_eq!(response.headers().get(SENT_HEADER).map(|v| v.to_str().unwrap()), Some("1"), "the site had the prompt");
        let muse = rig.muse();
        assert!(!muse.enabled, "a suspended account gets nothing more");
        assert_eq!(muse.web_state_reason.as_deref(), Some(RESTRICTED));
        // The request was kept before the site was touched, and the account is backed off.
        let now = crate::db::now_ms();
        assert_eq!(store::web_requests_today(&rig.db.lock().unwrap(), &rig.muse.id, now).unwrap(), 1);
        assert!(store::web_guard(&rig.db.lock().unwrap(), &rig.muse.id, now).unwrap().backoff_reason.is_some());
        // Turned off, it is not driven again, even by a caller that has its token.
        let before = rig.page.navigations.load(Ordering::SeqCst);
        let again = rig.post("web-token", json!({ "model": "muse", "messages": [{ "role": "user", "content": "hi" }] })).await;
        assert_eq!(again.status(), 400);
        assert_eq!(rig.page.navigations.load(Ordering::SeqCst), before);
    });
}

#[test]
fn a_notice_the_user_quoted_is_not_taken_for_the_sites() {
    rt().block_on(async {
        let quoted = json!({ "count": 1, "text": "It means the account is suspended.", "busy": false, "restricted": true });
        let rig = rig(FakePage::new(vec![ready()], vec![read(0, "", false), quoted])).await;
        let prompt = "What does 'account is suspended' mean?";
        let response = rig.post("web-token", json!({ "model": "muse", "messages": [{ "role": "user", "content": prompt }] })).await;
        assert_eq!(response.status(), 200);
        assert!(rig.muse().enabled);
    });
}

#[test]
fn a_failure_before_send_does_not_mark_the_prompt_sent() {
    rt().block_on(async {
        let signed_out = json!({ "fresh": true, "ready": false, "hasInput": true, "signedOut": true });
        let rig = rig(FakePage::new(vec![signed_out], vec![read(0, "", false)])).await;
        let response = rig.post("web-token", json!({ "model": "muse", "messages": [{ "role": "user", "content": "hi" }] })).await;
        assert_eq!(response.status(), 401);
        assert!(response.headers().get(SENT_HEADER).is_none(), "another account may still take it");
    });
}

#[test]
fn a_sign_in_check_keeps_the_guards_pace_and_a_skipped_one_is_not_a_check() {
    rt().block_on(async {
        let rig = rig(hello_page()).await;
        let now = crate::db::now_ms();
        store::web_guard_checked(&rig.db.lock().unwrap(), &rig.muse.id, now + 60_000).unwrap();
        let skipped = rig.service.check(&rig.muse.id).await.unwrap();
        assert!(matches!(&skipped, Checked::Skipped(why) if why.starts_with(CHECK_RESTING)), "{skipped:?}");
        assert_eq!(rig.page.navigations.load(Ordering::SeqCst), 0, "the page was not loaded");

        store::web_guard_resume(&rig.db.lock().unwrap(), &rig.muse.id).unwrap();
        store::web_guard_trouble(&rig.db.lock().unwrap(), &rig.muse.id, CHALLENGE, now, 60_000, now).unwrap();
        assert!(matches!(rig.service.check(&rig.muse.id).await.unwrap(), Checked::Skipped(why) if why.starts_with(CHECK_BACKED_OFF)));

        store::web_guard_resume(&rig.db.lock().unwrap(), &rig.muse.id).unwrap();
        assert_eq!(rig.service.check(&rig.muse.id).await.unwrap(), Checked::Seen);
        assert_eq!(rig.muse().web_state.as_deref(), Some("signed_in"));
    });
}

#[test]
fn a_prompt_that_reached_one_site_is_not_sent_to_another_web_session() {
    rt().block_on(async {
        // The site takes the prompt, then says the account reached its usage limit: an answer the gateway would
        // otherwise carry to the pool's next account.
        let limit = "You have reached your limit for today.";
        let page = FakePage::new(vec![ready()], vec![read(0, "", false), read(1, limit, false), read(1, limit, false)]);
        let (web, gateway, _shared) = gateway_rig(page, true).await;
        {
            let conn = web.db.lock().unwrap();
            let other = store::add_web_provider(&conn, "muse-web", "Muse", WARNING_VERSION).unwrap();
            for id in [&web.muse.id, &other.id] {
                store::update_provider(&conn, id, Some(true), Some("subscription"), Some(true)).unwrap();
                store::set_pool(&conn, id, Some("muses")).unwrap();
            }
        }
        let response = gateway_post(&gateway, json!({ "model": "muses/muse", "messages": [{ "role": "user", "content": "hi" }] })).await;
        assert!(!response.status().is_success());
        assert_eq!(response.status(), 429);
        assert_eq!(web.page.navigations.load(Ordering::SeqCst), 1, "one site saw the prompt, the other nothing");
        // The ledger has one attempt: the second account was passed over before its page was touched.
        assert_eq!(_shared.with_db(|db| store::usage_summary(db, 0, 10)).unwrap().recent.len(), 1);
    });
}
