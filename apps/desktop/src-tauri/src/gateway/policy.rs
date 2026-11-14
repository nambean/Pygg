//! What a failed attempt means for the rest of the request and for later ones (ROADMAP P0-2,
//! docs/archive/ACCOUNT_API_ROTATION.md §5, counter-review CR-01, CR-04, CR-05). Two questions per failure:
//!
//! - **May another target answer this request?** Never after an ambiguous failure, where the provider may
//!   have run (and billed) the request: a dropped connection, a read timeout, and the HTTP statuses that say
//!   the same (408, 504, 524). Not a connection in the same quota group after 401, 402, 403 or 429: trying
//!   the next key of the same organization is the quota and ban evasion the terms forbid. Other targets of a
//!   combo were chosen by the user as fallbacks; they are the only ones ever tried.
//! - **Should this connection or model be skipped for a while?** A cooldown honours the provider's reset
//!   (`Retry-After` in seconds or as an HTTP date, `retry-after-ms`, Gemini's `RetryInfo`); a rejected key
//!   stays skipped until the user replaces it. Nothing is sent to probe a cooled-down target: it is simply
//!   not used until the time has passed or the user says to try again.
//!
//! The HTTP status alone does not say whether a limit is per model, per key or per organization. Where a
//! provider says more in its error body, that is used (see `ProviderError::from_response`); otherwise the
//! narrowest cooldown is set on the connection that refused, and the whole quota group is kept off that model
//! (`store::group_hold`). The full matrix is in docs/reviews/2026-11-11-counter-review-response.md.

use axum::http::HeaderMap;
use serde_json::Value;
use std::time::{Duration, SystemTime};

/// A provider's error answer in a normalized form.
#[derive(Clone, Debug, PartialEq)]
pub struct ProviderError {
    pub status: u16,
    /// The provider's own error code, when its body has one (`error.code`, `error.type` or `error.status`).
    pub code: Option<String>,
    /// When the provider said it may be tried again.
    pub retry_after: Option<Duration>,
    /// The provider's message, for the few cases where only it says what happened.
    pub message: Option<String>,
}

impl ProviderError {
    pub fn new(status: u16) -> ProviderError {
        ProviderError { status, code: None, retry_after: None, message: None }
    }

    /// Reads the status, the reset time and the provider's error code from an error response.
    pub fn from_response(status: u16, headers: &HeaderMap, body: &[u8]) -> ProviderError {
        let parsed: Option<Value> = serde_json::from_slice(body).ok();
        // Gemini's OpenAI-compatible endpoint answers with an array of one error.
        let root = parsed.as_ref().map(|v| if v.is_array() { &v[0] } else { v });
        let error = root.map(|v| &v["error"]);
        let code = error.and_then(|e| {
            ["code", "type", "status"].iter().find_map(|k| e[*k].as_str().map(str::to_string))
        });
        let retry_after = retry_after(headers, SystemTime::now()).or_else(|| error.and_then(gemini_retry_delay));
        let message = error.and_then(|e| e["message"].as_str()).map(|m| m.chars().take(300).collect());
        ProviderError { status, code, retry_after, message }
    }

    /// A rejected key sent as 400: Gemini answers an invalid key with `400 INVALID_ARGUMENT` and `Please pass a valid
    /// API key` (captured 2026-11-11, tests/fixtures/provider-errors/gemini-401.json).
    fn is_rejected_key(&self) -> bool {
        self.status == 400 && self.message.as_deref().is_some_and(|m| {
            let m = m.to_ascii_lowercase();
            m.contains("api key") && (m.contains("valid") || m.contains("expired"))
        })
    }

    /// Anthropic answers an account out of credit with HTTP 400 ("Your credit balance is too low to access the
    /// Anthropic API"), not 402. Found by the P0 benchmark on 2026-11-12.
    fn is_out_of_credit(&self) -> bool {
        self.status == 400 && self.message.as_deref().is_some_and(|m| m.to_ascii_lowercase().contains("credit balance is too low"))
    }

    /// Anthropic answers HTTP 400 "This model does not support assistant message prefill. The conversation must end
    /// with a user message." when a conversation ends with an assistant turn and the model takes no prefill (Claude
    /// Haiku 5.5 to OpenCode, CCHT benchmark 2026-11-13, tests/fixtures/provider-errors/anthropic-400-prefill.json).
    /// That is this model's refusal of the request's shape, not a malformed request: another model may answer it.
    fn is_prefill_refused(&self) -> bool {
        self.status == 400 && self.message.as_deref().is_some_and(|m| m.to_ascii_lowercase().contains("assistant message prefill"))
    }

    fn code_is(&self, codes: &[&str]) -> bool {
        self.code.as_deref().is_some_and(|c| codes.contains(&c))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Failure {
    /// The request never reached the provider: no connection could be made, or no key was available.
    NotSent,
    /// The request was sent but no clear answer came back (a read timeout, a dropped connection). The
    /// provider may have run it.
    Ambiguous,
    Http(ProviderError),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Next {
    /// Any later target may be tried.
    Any,
    /// Only targets outside this connection's quota group.
    OtherGroups,
    /// No other target: answer the client with this failure.
    Stop,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    /// Whether the entry covers one model or the whole connection.
    pub model_scope: bool,
    pub state: &'static str,
    /// `None`: until the user fixes it.
    pub cooldown: Option<Duration>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    pub next: Next,
    pub mark: Option<Mark>,
    /// The provider may have run the request, so its cost is unknown rather than $0.
    pub maybe_billed: bool,
}

const RATE_LIMIT_DEFAULT: Duration = Duration::from_secs(30);
/// A provider-stated reset is honoured as given, up to this sanity bound against a malformed value. The
/// cooldown only decides when the target is used again; it can be cleared in Connections at any time.
pub const COOLDOWN_CAP: Duration = Duration::from_secs(7 * 24 * 60 * 60);

pub fn decide(failure: &Failure) -> Decision {
    let mark = |model_scope, state, cooldown: Option<Duration>| Some(Mark { model_scope, state, cooldown });
    let decision = |next, mark, maybe_billed| Decision { next, mark, maybe_billed };
    match failure {
        Failure::NotSent => decision(Next::Any, mark(false, "unreachable", Some(Duration::from_secs(15))), false),
        Failure::Ambiguous => decision(Next::Stop, None, true),
        Failure::Http(e) => {
            let wait = |default: Duration| Some(e.retry_after.unwrap_or(default).min(COOLDOWN_CAP));
            match e.status {
                401 => decision(Next::OtherGroups, mark(false, "invalid_credentials", None), false),
                400 if e.is_rejected_key() => decision(Next::OtherGroups, mark(false, "invalid_credentials", None), false),
                // As a 402: ten minutes, since the same words also come back for a request sent with another account's
                // session (seen when Claude Code inherited the Claude desktop app's variables).
                400 if e.is_out_of_credit() => decision(Next::OtherGroups, mark(false, "payment_required", wait(Duration::from_secs(600))), false),
                402 => decision(Next::OtherGroups, mark(false, "payment_required", wait(Duration::from_secs(600))), false),
                // A model that takes no assistant-message prefill: the next target may, and nothing was run. No mark:
                // the model is well; it is this request's last turn it refuses.
                400 if e.is_prefill_refused() => decision(Next::Any, None, false),
                // OpenAI says "insufficient_quota" with a 429 when the account is out of credit: that is the
                // account, not one model, and it does not pass in 30 seconds.
                429 if e.code_is(&["insufficient_quota", "billing_hard_limit_reached"]) => {
                    decision(Next::OtherGroups, mark(false, "payment_required", wait(Duration::from_secs(3600))), false)
                }
                403 => decision(Next::OtherGroups, mark(true, "forbidden", wait(Duration::from_secs(600))), false),
                404 => decision(Next::Any, mark(true, "model_unavailable", wait(Duration::from_secs(300))), false),
                429 => decision(Next::OtherGroups, mark(true, "rate_limited", wait(RATE_LIMIT_DEFAULT)), false),
                // RFC 8470: 425 means the server declined to process a request sent as TLS early data, so it did
                // not run it.
                425 => decision(Next::Any, None, false),
                // A request timeout (some providers send 408 for a generation that ran too long), a conflict that
                // depends on the request's own state, and gateway timeouts in front of the provider (504,
                // Cloudflare's 524): the request may have run. Stop, and the cost is unknown.
                408 | 409 | 504 | 524 => decision(Next::Stop, None, !matches!(e.status, 409)),
                // Overloaded or failing before it answered: another provider may help. The attempt's cost stays
                // unknown (a provider could bill a failed generation); it is counted against a client's limit at
                // the amount held for it.
                500..=599 => decision(Next::Any, mark(true, "server_error", wait(Duration::from_secs(15))), true),
                // 400, 413, 422 and the rest: the request itself is the problem and fails the same way everywhere.
                _ => decision(Next::Stop, None, false),
            }
        }
    }
}

/// The reset a response asks for: OpenAI's `retry-after-ms`, then `Retry-After` in seconds or as an HTTP
/// date. A date is measured against the response's own `Date` header when it has one, so a skewed local
/// clock does not shorten or lengthen the wait.
pub fn retry_after(headers: &HeaderMap, now: SystemTime) -> Option<Duration> {
    let text = |name: &str| headers.get(name).and_then(|v| v.to_str().ok()).map(str::trim);
    if let Some(ms) = text("retry-after-ms").and_then(|v| v.parse::<f64>().ok()) {
        if ms.is_finite() && ms >= 0.0 {
            return Some(Duration::from_millis(ms.min(COOLDOWN_CAP.as_millis() as f64) as u64));
        }
    }
    let value = text("retry-after")?;
    if let Ok(seconds) = value.parse::<f64>() {
        return (seconds.is_finite() && seconds >= 0.0)
            .then(|| Duration::from_millis((seconds.min(COOLDOWN_CAP.as_secs() as f64) * 1000.0) as u64));
    }
    let at = httpdate::parse_http_date(value).ok()?;
    let reference = text("date").and_then(|d| httpdate::parse_http_date(d).ok()).unwrap_or(now);
    Some(at.duration_since(reference).unwrap_or(Duration::ZERO))
}

/// Gemini's `google.rpc.RetryInfo` detail: `{"@type": "...RetryInfo", "retryDelay": "37s"}`.
fn gemini_retry_delay(error: &Value) -> Option<Duration> {
    error["details"].as_array()?.iter()
        .filter(|d| d["@type"].as_str().is_some_and(|t| t.ends_with("RetryInfo")))
        .find_map(|d| d["retryDelay"].as_str()?.strip_suffix('s')?.parse::<f64>().ok())
        .filter(|s| s.is_finite() && *s >= 0.0)
        .map(|s| Duration::from_millis((s.min(COOLDOWN_CAP.as_secs() as f64) * 1000.0) as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http(status: u16) -> Failure {
        Failure::Http(ProviderError::new(status))
    }

    #[test]
    fn credential_and_quota_failures_do_not_jump_to_a_sibling_key() {
        for status in [401, 402, 403, 429] {
            assert_eq!(decide(&http(status)).next, Next::OtherGroups, "{status}");
        }
        assert_eq!(decide(&http(401)).mark.unwrap().cooldown, None);
    }

    #[test]
    fn an_ambiguous_failure_is_never_retried_elsewhere() {
        assert_eq!(decide(&Failure::Ambiguous).next, Next::Stop);
        assert!(decide(&Failure::Ambiguous).maybe_billed);
        assert_eq!(decide(&http(400)).next, Next::Stop);
        assert_eq!(decide(&Failure::NotSent).next, Next::Any);
        assert_eq!(decide(&http(503)).next, Next::Any);
        // Statuses that may come after the provider ran the request stop too.
        for status in [408, 504, 524] {
            let d = decide(&http(status));
            assert_eq!((d.next, d.maybe_billed), (Next::Stop, true), "{status}");
        }
        assert_eq!(decide(&http(409)).next, Next::Stop);
        assert_eq!(decide(&http(425)).next, Next::Any, "RFC 8470: not processed");
    }

    #[test]
    fn an_account_out_of_credit_is_not_a_per_model_rate_limit() {
        let body = br#"{"error":{"message":"You exceeded your current quota","type":"insufficient_quota","code":"insufficient_quota"}}"#;
        let e = ProviderError::from_response(429, &HeaderMap::new(), body);
        assert_eq!(e.code.as_deref(), Some("insufficient_quota"));
        let mark = decide(&Failure::Http(e)).mark.unwrap();
        assert_eq!((mark.model_scope, mark.state, mark.cooldown), (false, "payment_required", Some(Duration::from_secs(3600))));
        let anthropic = br#"{"type":"error","error":{"type":"invalid_request_error","message":"Your credit balance is too low to access the Anthropic API. Please go to Plans & Billing to upgrade or purchase credits."}}"#;
        let e = ProviderError::from_response(400, &HeaderMap::new(), anthropic);
        let decision = decide(&Failure::Http(e));
        assert_eq!(decision.next, Next::OtherGroups, "another provider may still answer");
        let mark = decision.mark.unwrap();
        assert_eq!((mark.model_scope, mark.state), (false, "payment_required"));
        // A plain 429 stays on the one model.
        let plain = decide(&http(429)).mark.unwrap();
        assert_eq!((plain.model_scope, plain.cooldown), (true, Some(RATE_LIMIT_DEFAULT)));
    }

    #[test]
    fn a_prefill_refusal_moves_to_the_next_target_without_a_mark() {
        let body = br#"{"type":"error","error":{"type":"invalid_request_error","message":"This model does not support assistant message prefill. The conversation must end with a user message."}}"#;
        let e = ProviderError::from_response(400, &HeaderMap::new(), body);
        let d = decide(&Failure::Http(e));
        assert_eq!((d.next, d.mark, d.maybe_billed), (Next::Any, None, false));
        // Any other 400 is still the request's own fault.
        let other = ProviderError::from_response(400, &HeaderMap::new(), br#"{"type":"error","error":{"type":"invalid_request_error","message":"max_tokens: Field required"}}"#);
        assert_eq!(decide(&Failure::Http(other)).next, Next::Stop);
    }

    #[test]
    fn reads_every_form_of_the_reset_time() {
        let now = httpdate::parse_http_date("Wed, 11 Nov 2026 03:00:00 GMT").unwrap();
        let headers = |pairs: &[(&'static str, &str)]| {
            let mut h = HeaderMap::new();
            for (k, v) in pairs {
                h.insert(*k, v.parse().unwrap());
            }
            h
        };
        assert_eq!(retry_after(&headers(&[("retry-after", "120")]), now), Some(Duration::from_secs(120)));
        assert_eq!(retry_after(&headers(&[("retry-after", "120"), ("retry-after-ms", "1500")]), now),
            Some(Duration::from_millis(1500)));
        // An HTTP date, 24 hours ahead.
        assert_eq!(retry_after(&headers(&[("retry-after", "Thu, 12 Nov 2026 03:00:00 GMT")]), now),
            Some(Duration::from_secs(86_400)));
        // Measured against the provider's Date, not a local clock that is an hour behind.
        let skewed = now - Duration::from_secs(3600);
        assert_eq!(retry_after(&headers(&[("retry-after", "Wed, 11 Nov 2026 03:05:00 GMT"),
            ("date", "Wed, 11 Nov 2026 03:00:00 GMT")]), skewed), Some(Duration::from_secs(300)));
        // A date in the past means now; a malformed value means no stated reset.
        assert_eq!(retry_after(&headers(&[("retry-after", "Tue, 10 Nov 2026 03:00:00 GMT")]), now), Some(Duration::ZERO));
        assert_eq!(retry_after(&headers(&[("retry-after", "soon")]), now), None);
        assert_eq!(retry_after(&headers(&[("retry-after", "-5")]), now), None);
    }

    #[test]
    fn a_long_stated_reset_is_honoured_not_cut_to_hours() {
        let day = ProviderError { retry_after: Some(Duration::from_secs(30 * 3600)), ..ProviderError::new(429) };
        assert_eq!(decide(&Failure::Http(day)).mark.unwrap().cooldown, Some(Duration::from_secs(30 * 3600)));
        let absurd = ProviderError { retry_after: Some(Duration::from_secs(10 * 365 * 86_400)), ..ProviderError::new(429) };
        assert_eq!(decide(&Failure::Http(absurd)).mark.unwrap().cooldown, Some(COOLDOWN_CAP));
    }

    #[test]
    fn reads_geminis_retry_info() {
        let body = br#"[{"error":{"code":429,"status":"RESOURCE_EXHAUSTED","message":"Quota exceeded",
            "details":[{"@type":"type.googleapis.com/google.rpc.QuotaFailure"},
                       {"@type":"type.googleapis.com/google.rpc.RetryInfo","retryDelay":"37s"}]}}]"#;
        let e = ProviderError::from_response(429, &HeaderMap::new(), body);
        assert_eq!((e.code.as_deref(), e.retry_after), (Some("RESOURCE_EXHAUSTED"), Some(Duration::from_secs(37))));
    }
}
