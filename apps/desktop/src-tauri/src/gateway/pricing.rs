//! Turning a response's token usage into a cost with provenance (ADR-011). An amount is only stated when
//! something backs it: the provider's own figure, a price the user entered, or a free or local billing
//! type. Everything else is unknown, never $0.

use serde::{Deserialize, Serialize};

/// A price in micro-USD per million tokens, with where it came from.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Price {
    pub input_micro_per_mtok: u64,
    pub output_micro_per_mtok: u64,
    /// Cached prompt tokens; the input price when absent.
    pub cached_input_micro_per_mtok: Option<u64>,
    /// `user` for prices entered in Connections.
    pub source: String,
    pub observed_at: i64,
}

/// Token counts from an OpenAI-style `usage` object.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Usage {
    pub prompt: u64,
    pub completion: u64,
    /// Part of `prompt` served from the provider's cache.
    pub cached: u64,
    /// OpenRouter and some others report the charge itself, in USD.
    pub provider_cost_usd: Option<f64>,
}

impl Usage {
    pub fn from_json(usage: &serde_json::Value) -> Option<Usage> {
        if !usage.is_object() {
            return None;
        }
        let n = |v: &serde_json::Value| v.as_u64().unwrap_or(0);
        Some(Usage {
            prompt: n(&usage["prompt_tokens"]),
            completion: n(&usage["completion_tokens"]),
            cached: n(&usage["prompt_tokens_details"]["cached_tokens"]),
            provider_cost_usd: usage["cost"].as_f64().filter(|c| c.is_finite() && *c >= 0.0),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Cost {
    Actual { micro_usd: u64, source: String },
    Estimated { micro_usd: u64, source: String },
    Unknown { source: String },
}

impl Cost {
    pub fn status(&self) -> &'static str {
        match self {
            Cost::Actual { .. } => "actual",
            Cost::Estimated { .. } => "estimated",
            Cost::Unknown { .. } => "unknown",
        }
    }

    pub fn micro_usd(&self) -> Option<u64> {
        match self {
            Cost::Actual { micro_usd, .. } | Cost::Estimated { micro_usd, .. } => Some(*micro_usd),
            Cost::Unknown { .. } => None,
        }
    }

    pub fn source(&self) -> &str {
        match self {
            Cost::Actual { source, .. } | Cost::Estimated { source, .. } | Cost::Unknown { source } => source,
        }
    }
}

fn per_mtok(tokens: u64, micro_per_mtok: u64) -> u64 {
    ((tokens as u128 * micro_per_mtok as u128 + 500_000) / 1_000_000) as u64
}

/// The cost of a successful response.
pub fn resolve(billing: &str, price: Option<&Price>, usage: Option<&Usage>) -> Cost {
    if let Some(usd) = usage.and_then(|u| u.provider_cost_usd) {
        return Cost::Actual { micro_usd: (usd * 1_000_000.0).round() as u64, source: "provider response".into() };
    }
    match billing {
        "free" => return Cost::Estimated { micro_usd: 0, source: "connection marked free".into() },
        "local" => return Cost::Estimated { micro_usd: 0, source: "local model".into() },
        "subscription" => return Cost::Unknown { source: "subscription: counts against a quota, not money".into() },
        _ => {}
    }
    let Some(usage) = usage else {
        return Cost::Unknown { source: "the provider reported no usage".into() };
    };
    let Some(price) = price else {
        return Cost::Unknown { source: "no price recorded for this model".into() };
    };
    let cached = usage.cached.min(usage.prompt);
    let micro = per_mtok(usage.prompt - cached, price.input_micro_per_mtok)
        + per_mtok(cached, price.cached_input_micro_per_mtok.unwrap_or(price.input_micro_per_mtok))
        + per_mtok(usage.completion, price.output_micro_per_mtok);
    Cost::Estimated { micro_usd: micro, source: format!("price: {}", price.source) }
}

/// The cost of a request the provider did not complete. A 4xx rejection is not billed by the providers
/// the presets cover; a server error or a dropped connection may have been, so it stays unknown.
pub fn resolve_failure(http_status: Option<u16>) -> Cost {
    match http_status {
        Some(status) if (400..500).contains(&status) => {
            Cost::Estimated { micro_usd: 0, source: format!("rejected by the provider (HTTP {status})") }
        }
        _ => Cost::Unknown { source: "the request failed after reaching the provider".into() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn price() -> Price {
        Price { input_micro_per_mtok: 100_000, output_micro_per_mtok: 400_000, cached_input_micro_per_mtok: Some(25_000),
            source: "user".into(), observed_at: 0 }
    }

    #[test]
    fn estimates_from_a_price_and_counts_cached_tokens_apart() {
        // 1M prompt tokens of which 400k cached, 500k output: 0.6 × $0.10 + 0.4 × $0.025 + 0.5 × $0.40.
        let usage = Usage { prompt: 1_000_000, completion: 500_000, cached: 400_000, provider_cost_usd: None };
        let cost = resolve("paid", Some(&price()), Some(&usage));
        assert_eq!(cost, Cost::Estimated { micro_usd: 60_000 + 10_000 + 200_000, source: "price: user".into() });
    }

    #[test]
    fn unknown_is_never_zero() {
        let usage = Usage { prompt: 10, completion: 10, ..Usage::default() };
        assert_eq!(resolve("paid", None, Some(&usage)).micro_usd(), None);
        assert_eq!(resolve("unknown", Some(&price()), None).micro_usd(), None);
        assert_eq!(resolve("subscription", Some(&price()), Some(&usage)).status(), "unknown");
        assert_eq!(resolve_failure(Some(503)).micro_usd(), None);
        assert_eq!(resolve_failure(None).micro_usd(), None);
    }

    #[test]
    fn provider_figures_win() {
        let usage = Usage::from_json(&serde_json::json!({"prompt_tokens": 5, "completion_tokens": 7,
            "prompt_tokens_details": {"cached_tokens": 2}, "cost": 0.00123})).unwrap();
        assert_eq!((usage.prompt, usage.completion, usage.cached), (5, 7, 2));
        assert_eq!(resolve("paid", None, Some(&usage)), Cost::Actual { micro_usd: 1230, source: "provider response".into() });
        assert_eq!(resolve("local", None, Some(&Usage::default())).micro_usd(), Some(0));
        assert_eq!(resolve_failure(Some(429)).micro_usd(), Some(0));
    }
}
