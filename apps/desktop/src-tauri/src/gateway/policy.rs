//! What a failed attempt means for the rest of the request and for later ones (ROADMAP P0-2,
//! ACCOUNT_API_ROTATION §5). Two questions per failure:
//!
//! - **May another target answer this request?** Never after an ambiguous failure, where the provider may
//!   have run (and billed) the request. Not a connection in the same quota group after 401, 402, 403 or 429:
//!   trying the next key of the same organization is the quota and ban evasion the terms forbid.
//! - **Should this connection or model be skipped for a while?** A cooldown honours `Retry-After`; a rejected
//!   key stays skipped until the user replaces it.

use axum::http::HeaderMap;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub enum Failure {
    /// The request never reached the provider: no connection could be made, or no key was available.
    NotSent,
    /// The request was sent but no clear answer came back (a read timeout, a dropped connection). The
    /// provider may have run it.
    Ambiguous,
    Http { status: u16, retry_after: Option<Duration> },
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
}

const RATE_LIMIT_DEFAULT: Duration = Duration::from_secs(30);
/// A provider asking for more than this is treated as exhausted for the day; the cooldown is capped so a
/// connection is re-checked eventually.
const COOLDOWN_CAP: Duration = Duration::from_secs(6 * 60 * 60);

pub fn decide(failure: &Failure) -> Decision {
    let mark = |model_scope, state, cooldown: Option<Duration>| Some(Mark { model_scope, state, cooldown });
    match failure {
        Failure::NotSent => Decision { next: Next::Any, mark: mark(false, "unreachable", Some(Duration::from_secs(15))) },
        Failure::Ambiguous => Decision { next: Next::Stop, mark: None },
        Failure::Http { status, retry_after } => match status {
            401 => Decision { next: Next::OtherGroups, mark: mark(false, "invalid_credentials", None) },
            402 => Decision { next: Next::OtherGroups, mark: mark(false, "payment_required", Some(Duration::from_secs(600))) },
            403 => Decision { next: Next::OtherGroups, mark: mark(true, "forbidden", Some(Duration::from_secs(600))) },
            404 => Decision { next: Next::Any, mark: mark(true, "model_unavailable", Some(Duration::from_secs(300))) },
            429 => {
                let wait = retry_after.unwrap_or(RATE_LIMIT_DEFAULT).min(COOLDOWN_CAP);
                Decision { next: Next::OtherGroups, mark: mark(true, "rate_limited", Some(wait)) }
            }
            // Not processed: a request timeout, a conflict or "too early" may well succeed elsewhere.
            408 | 409 | 425 => Decision { next: Next::Any, mark: None },
            500..=599 => {
                let wait = retry_after.unwrap_or(Duration::from_secs(15)).min(COOLDOWN_CAP);
                Decision { next: Next::Any, mark: mark(true, "server_error", Some(wait)) }
            }
            // 400, 413, 422 and the rest: the request itself is the problem and fails the same way everywhere.
            _ => Decision { next: Next::Stop, mark: None },
        },
    }
}

/// `Retry-After` in seconds (an HTTP date is ignored, and the default cooldown applies), or OpenAI's
/// `retry-after-ms`.
pub fn retry_after(headers: &HeaderMap) -> Option<Duration> {
    let text = |name: &str| headers.get(name).and_then(|v| v.to_str().ok()).map(str::trim);
    if let Some(ms) = text("retry-after-ms").and_then(|v| v.parse::<f64>().ok()) {
        if ms.is_finite() && ms >= 0.0 {
            return Some(Duration::from_millis(ms as u64));
        }
    }
    text("retry-after").and_then(|v| v.parse::<f64>().ok()).filter(|s| s.is_finite() && *s >= 0.0)
        .map(|s| Duration::from_millis((s * 1000.0) as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_and_quota_failures_do_not_jump_to_a_sibling_key() {
        for status in [401, 402, 403, 429] {
            assert_eq!(decide(&Failure::Http { status, retry_after: None }).next, Next::OtherGroups, "{status}");
        }
        assert_eq!(decide(&Failure::Http { status: 401, retry_after: None }).mark.unwrap().cooldown, None);
    }

    #[test]
    fn an_ambiguous_failure_is_never_retried_elsewhere() {
        assert_eq!(decide(&Failure::Ambiguous).next, Next::Stop);
        assert_eq!(decide(&Failure::Http { status: 400, retry_after: None }).next, Next::Stop);
        assert_eq!(decide(&Failure::NotSent).next, Next::Any);
        assert_eq!(decide(&Failure::Http { status: 503, retry_after: None }).next, Next::Any);
    }

    #[test]
    fn honours_retry_after() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", "120".parse().unwrap());
        let wait = retry_after(&headers);
        assert_eq!(wait, Some(Duration::from_secs(120)));
        let mark = decide(&Failure::Http { status: 429, retry_after: wait }).mark.unwrap();
        assert_eq!((mark.model_scope, mark.state, mark.cooldown), (true, "rate_limited", Some(Duration::from_secs(120))));
        headers.insert("retry-after-ms", "1500".parse().unwrap());
        assert_eq!(retry_after(&headers), Some(Duration::from_millis(1500)));
        let huge = decide(&Failure::Http { status: 429, retry_after: Some(Duration::from_secs(999_999)) });
        assert_eq!(huge.mark.unwrap().cooldown, Some(COOLDOWN_CAP));
    }
}
