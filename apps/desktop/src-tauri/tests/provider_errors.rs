//! The gateway's error policy against real providers' error answers (ROADMAP C1, P0-2 "fixtures per real provider"),
//! captured with scripts/capture-provider-errors.ps1: an invalid key everywhere, and an unknown model and a malformed
//! request with real keys. None of them could generate text, so none was billed.

use axum::http::{HeaderMap, HeaderName, HeaderValue};
use pygg_desktop_lib::gateway::policy::{decide, Failure, Next, ProviderError};
use serde_json::Value;

fn fixture(case: &str) -> (u16, HeaderMap, Vec<u8>) {
    let path = format!("{}/tests/fixtures/provider-errors/{case}.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let f: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap();
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
