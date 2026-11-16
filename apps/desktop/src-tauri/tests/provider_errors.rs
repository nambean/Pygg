//! The gateway's error policy against real providers' error answers (ROADMAP C1, P0-2 "fixtures per real provider"),
//! captured with scripts/capture-provider-errors.ps1 (an invalid key everywhere, and an unknown model and a malformed
//! request with real keys) and scripts/capture-provider-limits.py (an empty balance, a refused model or file, a free
//! model rate-limited upstream, and a request given up on after it was sent). None of them was billed.

use axum::http::{HeaderMap, HeaderName, HeaderValue};
use pygg_desktop_lib::gateway::policy::{decide, Decision, Failure, Next, ProviderError};
use serde_json::Value;
use std::time::Duration;

fn load(case: &str) -> Value {
    let path = format!("{}/tests/fixtures/provider-errors/{case}.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap()
}

fn fixture(case: &str) -> (u16, HeaderMap, Vec<u8>) {
    let f = load(case);
    let mut headers = HeaderMap::new();
    for (k, v) in f["headers"].as_object().into_iter().flatten() {
        headers
            .insert(HeaderName::from_bytes(k.as_bytes()).unwrap(), HeaderValue::from_str(v.as_str().unwrap()).unwrap());
    }
    let body = match &f["body"] {
        Value::String(s) => s.clone().into_bytes(),
        other => serde_json::to_vec(other).unwrap(),
    };
    (f["status"].as_u64().unwrap() as u16, headers, body)
}

fn decision(case: &str) -> (Next, Option<&'static str>, bool) {
    let (status, headers, body) = fixture(case);
    let d = decide(&Failure::Http(ProviderError::from_response(status, &headers, &body)));
    (d.next, d.mark.as_ref().map(|m| m.state), d.mark.as_ref().map(|m| m.model_scope).unwrap_or(false))
}

#[test]
fn every_providers_rejected_key_marks_the_connection_and_skips_its_group() {
    // Gemini says it with a 400, the others with a 401.
    for case in [
        "anthropic-401",
        "gemini-401",
        "openrouter-401",
        "openai-401",
        "groq-401",
        "deepseek-401",
        "mistral-401",
        "qwen-401",
    ] {
        assert_eq!(decision(case), (Next::OtherGroups, Some("invalid_credentials"), false), "{case}");
    }
}

#[test]
fn an_unknown_model_cools_that_model_and_lets_others_answer() {
    for case in ["anthropic-404-model", "gemini-404-model", "qwen-404-model"] {
        assert_eq!(decision(case), (Next::Any, Some("model_unavailable"), true), "{case}");
    }
}

#[test]
fn a_malformed_request_stops_without_marking_anything() {
    assert_eq!(decision("anthropic-400-request"), (Next::Stop, None, false));
}

#[test]
fn a_prefill_refusal_lets_the_next_target_answer_and_marks_nothing() {
    assert_eq!(decision("anthropic-400-prefill"), (Next::Any, None, false));
}

/// The whole decision for a captured HTTP answer.
fn full(case: &str) -> (ProviderError, Decision) {
    let (status, headers, body) = fixture(case);
    let e = ProviderError::from_response(status, &headers, &body);
    (e.clone(), decide(&Failure::Http(e)))
}

#[test]
fn an_empty_balance_is_the_whole_account_and_waits_ten_minutes() {
    // Requesty and SiliconFlow both say it with a 402: another account may answer, no key of this one.
    for case in ["requesty-402-quota", "siliconflow-402-quota"] {
        let (_, d) = full(case);
        assert_eq!(d.next, Next::OtherGroups, "{case}");
        let mark = d.mark.unwrap();
        assert_eq!((mark.state, mark.model_scope), ("payment_required", false), "{case}");
        assert_eq!(mark.cooldown, Some(Duration::from_secs(600)), "{case}");
        assert!(!d.maybe_billed, "{case}");
    }
}

#[test]
fn a_refused_model_or_file_cools_that_model_and_leaves_the_account_alone() {
    // OpenRouter refuses a free model outside agent harnesses; Gemini refuses a file the key may not read. Neither says
    // whether another key of the same account would be let in, so only other accounts are tried.
    for case in ["openrouter-403-harness-only", "gemini-403-file"] {
        let (e, d) = full(case);
        assert!(e.message.is_some(), "{case}: the provider's words reach the user");
        assert_eq!(d.next, Next::OtherGroups, "{case}");
        let mark = d.mark.unwrap();
        assert_eq!((mark.state, mark.model_scope), ("forbidden", true), "{case}");
        assert_eq!(mark.cooldown, Some(Duration::from_secs(600)), "{case}");
        assert!(!d.maybe_billed, "{case}");
    }
    assert_eq!(full("gemini-403-file").0.code.as_deref(), Some("PERMISSION_DENIED"));
}

#[test]
fn a_free_model_rate_limited_upstream_cools_only_that_model_for_the_default_wait() {
    // OpenRouter's 429 carries no Retry-After: the default 30 seconds, on the one model.
    let (e, d) = full("openrouter-429-free");
    assert_eq!(e.retry_after, None);
    assert_eq!(d.next, Next::OtherGroups);
    let mark = d.mark.unwrap();
    assert_eq!((mark.state, mark.model_scope), ("rate_limited", true));
    assert_eq!(mark.cooldown, Some(Duration::from_secs(30)));
    assert!(!d.maybe_billed);
}

#[test]
fn a_request_given_up_on_after_it_was_sent_stops_with_its_cost_unknown() {
    // No answer at all: the provider may still run (and bill) it, so no other target is tried. The gateway turns its
    // own read timeout into this failure (gateway::tests::an_ambiguous_timeout_is_not_retried_on_another_model).
    let f = load("openrouter-timeout");
    assert!(f["status"].is_null());
    assert_eq!((f["failure"].as_str(), f["request_sent"].as_bool()), (Some("timeout"), Some(true)));
    let d = decide(&Failure::Ambiguous);
    assert_eq!((d.next, d.mark, d.maybe_billed), (Next::Stop, None, true));
}
