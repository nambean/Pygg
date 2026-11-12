//! What is left, as the provider states it (ROADMAP E, OmniRoute OR-03): rate-limit windows read from the headers a
//! provider last sent (PV-06, `rate_limits`), and an account's balance or free allowance read from the provider's own
//! account endpoint where it has one (OpenRouter's `/key`, DeepSeek's `/user/balance`). Nothing is estimated: a
//! provider that states nothing has no quota here, and unknown stays unknown.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// One limit a provider stated: how much of something it allows, how much is left, and when it resets.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    /// `requests`, `tokens`, `input_tokens`, `output_tokens`, `free_requests_per_day` or `retry` (a Retry-After).
    pub kind: String,
    pub limit: Option<u64>,
    pub remaining: Option<u64>,
    /// When it resets, in milliseconds.
    pub resets_at: Option<i64>,
}

impl Window {
    /// Used up until `resets_at`, as the provider said.
    pub fn exhausted_at(&self, now: i64) -> Option<i64> {
        (self.remaining == Some(0)).then_some(self.resets_at).flatten().filter(|at| *at > now)
    }
}

/// The windows in a provider's rate-limit headers, observed at `observed_at` (relative resets are counted from it).
/// Anthropic (`anthropic-ratelimit-<kind>-limit|remaining|reset`, an RFC 3339 time), OpenAI and Groq
/// (`x-ratelimit-limit|remaining|reset-<requests|tokens>`, a duration such as `6m0s`), OpenRouter
/// (`x-ratelimit-limit|remaining|reset`, epoch milliseconds) and `retry-after` (seconds or an HTTP date).
pub fn from_headers(headers: &BTreeMap<String, String>, observed_at: i64) -> Vec<Window> {
    let get = |name: &str| headers.get(name).map(|v| v.trim());
    let number = |name: &str| get(name).and_then(|v| v.parse::<f64>().ok()).filter(|v| *v >= 0.0).map(|v| v as u64);
    let mut out = Vec::new();
    for (kind, part) in [("requests", "requests"), ("tokens", "tokens"), ("input_tokens", "input-tokens"), ("output_tokens", "output-tokens")] {
        let window = Window {
            kind: kind.into(),
            limit: number(&format!("anthropic-ratelimit-{part}-limit")),
            remaining: number(&format!("anthropic-ratelimit-{part}-remaining")),
            resets_at: get(&format!("anthropic-ratelimit-{part}-reset")).and_then(rfc3339_ms),
        };
        if window.limit.is_some() || window.remaining.is_some() {
            out.push(window);
        }
    }
    for kind in ["requests", "tokens"] {
        let window = Window {
            kind: kind.into(),
            limit: number(&format!("x-ratelimit-limit-{kind}")),
            remaining: number(&format!("x-ratelimit-remaining-{kind}")),
            resets_at: get(&format!("x-ratelimit-reset-{kind}")).and_then(duration_ms).map(|d| observed_at + d),
        };
        if window.limit.is_some() || window.remaining.is_some() {
            out.push(window);
        }
    }
    if number("x-ratelimit-limit").is_some() || number("x-ratelimit-remaining").is_some() {
        // OpenRouter: epoch milliseconds; a small number is read as seconds.
        let reset = number("x-ratelimit-reset").map(|v| if v < 100_000_000_000 { v as i64 * 1000 } else { v as i64 });
        out.push(Window { kind: "requests".into(), limit: number("x-ratelimit-limit"), remaining: number("x-ratelimit-remaining"),
            resets_at: reset });
    }
    if let Some(value) = get("retry-after") {
        let at = value.parse::<f64>().ok().filter(|s| *s >= 0.0).map(|s| observed_at + (s * 1000.0) as i64)
            .or_else(|| httpdate::parse_http_date(value).ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64));
        if let Some(at) = at {
            out.push(Window { kind: "retry".into(), limit: None, remaining: Some(0), resets_at: Some(at) });
        }
    }
    out
}

/// `1s`, `6m0s`, `59.5s`, `2h3m`, `120ms` (OpenAI's and Groq's reset durations) in milliseconds.
pub fn duration_ms(text: &str) -> Option<i64> {
    let mut total = 0.0;
    let mut number = String::new();
    let mut chars = text.trim().chars().peekable();
    let mut any = false;
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() || c == '.' {
            number.push(c);
            continue;
        }
        let value: f64 = number.parse().ok()?;
        number.clear();
        let unit = match c {
            'h' => 3_600_000.0,
            'm' if chars.peek() == Some(&'s') => { chars.next(); 1.0 }
            'm' => 60_000.0,
            's' => 1000.0,
            _ => return None,
        };
        total += value * unit;
        any = true;
    }
    if !number.is_empty() {
        // A bare number is seconds.
        total += number.parse::<f64>().ok()? * 1000.0;
        any = true;
    }
    any.then_some(total.round() as i64)
}

/// An RFC 3339 time (`2026-11-12T08:00:00Z`, fractions and offsets allowed) in milliseconds.
pub fn rfc3339_ms(text: &str) -> Option<i64> {
    let t = text.trim();
    let (date, rest) = t.split_once(['T', 't', ' '])?;
    let mut d = date.split('-').map(|p| p.parse::<i64>());
    let (year, month, day) = (d.next()?.ok()?, d.next()?.ok()?, d.next()?.ok()?);
    let zone_at = rest.find(['Z', 'z', '+', '-']).unwrap_or(rest.len());
    let (clock, zone) = rest.split_at(zone_at);
    let mut c = clock.split(':');
    let (hour, minute) = (c.next()?.parse::<i64>().ok()?, c.next()?.parse::<i64>().ok()?);
    let seconds: f64 = c.next().unwrap_or("0").parse().ok()?;
    let offset_min = match zone.chars().next() {
        Some('+') | Some('-') => {
            let sign = if zone.starts_with('-') { -1 } else { 1 };
            let mut z = zone[1..].split(':');
            let h: i64 = z.next()?.parse().ok()?;
            let m: i64 = z.next().unwrap_or("0").parse().ok()?;
            sign * (h * 60 + m)
        }
        _ => 0,
    };
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Days from 1970-01-01 to a civil date (H. Hinnant's algorithm).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400_000 + (hour * 3600 + minute * 60 - offset_min * 60) * 1000 + (seconds * 1000.0).round() as i64)
}

/// An account's money or free allowance, as the provider's account endpoint states it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// Where it was read, e.g. `openrouter /key`.
    pub source: String,
    pub observed_at: i64,
    /// Money left to spend, in the provider's currency (USD for OpenRouter's credits).
    pub balance: Option<f64>,
    pub currency: Option<String>,
    /// The provider says the account can make paid calls now (DeepSeek's `is_available`).
    pub available: Option<bool>,
    /// Spent so far today (UTC), where stated.
    pub used_today: Option<f64>,
    /// Free requests per day and what is left of them (OpenRouter's `:free` models).
    pub free_requests: Option<Window>,
}

/// The next UTC midnight after `now`, when OpenRouter's daily counters reset.
fn next_utc_midnight(now: i64) -> i64 {
    (now.div_euclid(86_400_000) + 1) * 86_400_000
}

/// OpenRouter's `GET /api/v1/key`: the key's credit limit and what is left of it, today's usage, and the free models'
/// daily requests. A key without its own limit states no balance.
pub fn openrouter_key(body: &Value, now: i64) -> Option<Account> {
    let data = body.get("data")?;
    let n = |v: &Value| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok()));
    let free = &data["free_model_daily_requests"];
    let free_requests = (free.is_object()).then(|| Window {
        kind: "free_requests_per_day".into(), limit: n(&free["limit"]).map(|v| v as u64),
        remaining: n(&free["remaining"]).map(|v| v.max(0.0) as u64), resets_at: Some(next_utc_midnight(now)) });
    Some(Account {
        source: "openrouter /key".into(), observed_at: now, balance: n(&data["limit_remaining"]),
        currency: n(&data["limit_remaining"]).map(|_| "USD".into()), available: None, used_today: n(&data["usage_daily"]),
        free_requests,
    })
}

/// DeepSeek's `GET /user/balance`: whether paid calls are possible, and the balance in the first currency listed.
pub fn deepseek_balance(body: &Value, now: i64) -> Option<Account> {
    let info = body["balance_infos"].as_array()?.first();
    let total = info.and_then(|i| i["total_balance"].as_str()).and_then(|s| s.parse::<f64>().ok());
    Some(Account {
        source: "deepseek /user/balance".into(), observed_at: now, balance: total,
        currency: info.and_then(|i| i["currency"].as_str()).map(String::from), available: body["is_available"].as_bool(),
        used_today: None, free_requests: None,
    })
}

/// The account endpoint of a connection kind, relative to its endpoint, and how to read it.
pub fn account_endpoint(kind: &str) -> Option<(&'static str, fn(&Value, i64) -> Option<Account>)> {
    match kind {
        "openrouter" => Some(("/key", openrouter_key)),
        "deepseek" | "deepseek-anthropic" => Some(("/user/balance", deepseek_balance)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn headers(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn durations_and_times() {
        assert_eq!(duration_ms("6m0s"), Some(360_000));
        assert_eq!(duration_ms("59.5s"), Some(59_500));
        assert_eq!(duration_ms("2h3m"), Some(7_380_000));
        assert_eq!(duration_ms("120ms"), Some(120));
        assert_eq!(duration_ms("7"), Some(7000));
        assert_eq!(duration_ms("soon"), None);
        assert_eq!(rfc3339_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(rfc3339_ms("2026-11-12T07:47:21.058Z"), Some(1_794_469_641_058));
        assert_eq!(rfc3339_ms("2026-11-12T14:47:21.058+07:00"), Some(1_794_469_641_058));
    }

    #[test]
    fn windows_from_each_providers_headers() {
        let now = 1_794_469_641_000;
        let anthropic = from_headers(&headers(&[("anthropic-ratelimit-requests-limit", "50"),
            ("anthropic-ratelimit-requests-remaining", "0"), ("anthropic-ratelimit-requests-reset", "2026-11-12T07:48:21Z"),
            ("anthropic-ratelimit-input-tokens-limit", "40000"), ("anthropic-ratelimit-input-tokens-remaining", "39000")]), now);
        assert_eq!(anthropic.len(), 2);
        assert_eq!(anthropic[0].exhausted_at(now), Some(1_794_469_701_000));
        assert_eq!(anthropic[1].exhausted_at(now), None);

        let groq = from_headers(&headers(&[("x-ratelimit-limit-requests", "14400"), ("x-ratelimit-remaining-requests", "14370"),
            ("x-ratelimit-reset-requests", "2m59.56s"), ("x-ratelimit-limit-tokens", "6000"), ("x-ratelimit-remaining-tokens", "0"),
            ("x-ratelimit-reset-tokens", "7.66s")]), now);
        assert_eq!(groq[1].exhausted_at(now), Some(now + 7660));

        let openrouter = from_headers(&headers(&[("x-ratelimit-limit", "50"), ("x-ratelimit-remaining", "0"),
            ("x-ratelimit-reset", "1794528000000")]), now);
        assert_eq!(openrouter[0].exhausted_at(now), Some(1_794_528_000_000));

        let retry = from_headers(&headers(&[("retry-after", "30")]), now);
        assert_eq!(retry[0].exhausted_at(now), Some(now + 30_000));
        assert!(from_headers(&headers(&[("content-type", "x")]), now).is_empty(), "nothing stated, nothing known");
    }

    #[test]
    fn account_endpoints() {
        let now = 1_794_469_641_000;
        let or = openrouter_key(&json!({"data": {"limit": null, "limit_remaining": null, "usage_daily": 0.0123, "is_free_tier": true,
            "free_model_daily_requests": {"used": 12, "limit": 50, "remaining": 38}}}), now).unwrap();
        assert_eq!((or.balance, or.used_today), (None, Some(0.0123)), "no limit on the key: no balance stated");
        let free = or.free_requests.unwrap();
        assert_eq!((free.limit, free.remaining, free.resets_at), (Some(50), Some(38), Some(1_794_528_000_000)));
        let capped = openrouter_key(&json!({"data": {"limit": 10, "limit_remaining": 4.5}}), now).unwrap();
        assert_eq!((capped.balance, capped.currency.as_deref(), capped.free_requests), (Some(4.5), Some("USD"), None));

        let ds = deepseek_balance(&json!({"is_available": true, "balance_infos": [{"currency": "USD", "total_balance": "1.87",
            "granted_balance": "0.00", "topped_up_balance": "1.87"}]}), now).unwrap();
        assert_eq!((ds.balance, ds.currency.as_deref(), ds.available), (Some(1.87), Some("USD"), Some(true)));
        assert!(account_endpoint("gemini").is_none());
    }
}
