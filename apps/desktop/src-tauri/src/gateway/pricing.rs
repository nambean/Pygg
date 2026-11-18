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
    /// Prompt tokens written to the cache (Anthropic charges extra); the input price when absent.
    #[serde(default)]
    pub cache_write_micro_per_mtok: Option<u64>,
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
    /// Part of `prompt` written to the provider's cache.
    pub cache_write: u64,
    /// The provider said how much of the prompt came from its cache, even if none did. Without it a zero `cached`
    /// means "not reported", not "missed".
    pub cache_reported: bool,
    /// OpenRouter and some others report the charge itself, in USD.
    pub provider_cost_usd: Option<f64>,
}

impl Usage {
    /// An Anthropic `usage` object. `input_tokens` there excludes cached and cache-written tokens, so
    /// `prompt` adds them back to mean the whole prompt, as in the OpenAI format.
    pub fn from_anthropic(usage: &serde_json::Value) -> Option<Usage> {
        if !usage.is_object() {
            return None;
        }
        let n = |key: &str| usage[key].as_u64().unwrap_or(0);
        let (read, write) = (n("cache_read_input_tokens"), n("cache_creation_input_tokens"));
        Some(Usage {
            prompt: n("input_tokens") + read + write,
            completion: n("output_tokens"),
            cached: read,
            cache_write: write,
            cache_reported: usage.get("cache_read_input_tokens").is_some_and(|v| v.is_u64())
                || usage.get("cache_creation_input_tokens").is_some_and(|v| v.is_u64()),
            provider_cost_usd: None,
        })
    }

    /// Adds a later Anthropic stream event's usage (`message_delta` carries the final output count and may
    /// repeat the input counts).
    pub fn merge_anthropic(&mut self, usage: &serde_json::Value) {
        if let Some(output) = usage["output_tokens"].as_u64() {
            self.completion = output;
        }
        if usage.get("input_tokens").and_then(|v| v.as_u64()).is_some() {
            if let Some(later) = Usage::from_anthropic(usage) {
                if later.prompt > 0 {
                    self.prompt = later.prompt;
                    self.cached = later.cached;
                    self.cache_write = later.cache_write;
                    self.cache_reported |= later.cache_reported;
                }
            }
        }
    }

    /// A Responses API `usage` object: `input_tokens` is the whole prompt, with the cached part in its details.
    pub fn from_responses(usage: &serde_json::Value) -> Option<Usage> {
        if !usage.is_object() {
            return None;
        }
        let n = |v: &serde_json::Value| v.as_u64().unwrap_or(0);
        Some(Usage {
            prompt: n(&usage["input_tokens"]),
            completion: n(&usage["output_tokens"]),
            cached: n(&usage["input_tokens_details"]["cached_tokens"]),
            cache_write: 0,
            cache_reported: usage["input_tokens_details"]["cached_tokens"].is_u64(),
            provider_cost_usd: None,
        })
    }

    pub fn from_json(usage: &serde_json::Value) -> Option<Usage> {
        if !usage.is_object() {
            return None;
        }
        let n = |v: &serde_json::Value| v.as_u64().unwrap_or(0);
        // OpenAI's own field, or DeepSeek's (`prompt_cache_hit_tokens`, with the rest in `prompt_cache_miss_tokens`).
        let cached = [&usage["prompt_tokens_details"]["cached_tokens"], &usage["prompt_cache_hit_tokens"]]
            .into_iter()
            .find(|v| v.is_u64());
        Some(Usage {
            prompt: n(&usage["prompt_tokens"]),
            completion: n(&usage["completion_tokens"]),
            cached: cached.map_or(0, n),
            // OpenRouter reports what it wrote to an Anthropic model's cache (live check, 2026-11-16); Alibaba Model
            // Studio, what a client's explicit `cache_control` wrote, under Anthropic's name (its context-cache docs).
            cache_write: [
                &usage["prompt_tokens_details"]["cache_write_tokens"],
                &usage["prompt_tokens_details"]["cache_creation_input_tokens"],
            ]
            .into_iter()
            .find(|v| v.is_u64())
            .map_or(0, n),
            cache_reported: cached.is_some(),
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
    let written = usage.cache_write.min(usage.prompt - cached);
    let micro = per_mtok(usage.prompt - cached - written, price.input_micro_per_mtok)
        + per_mtok(cached, price.cached_input_micro_per_mtok.unwrap_or(price.input_micro_per_mtok))
        + per_mtok(written, price.cache_write_micro_per_mtok.unwrap_or(price.input_micro_per_mtok))
        + per_mtok(usage.completion, price.output_micro_per_mtok);
    // Say so when cache writes were priced as plain input for lack of a cache-write price.
    let source = if written > 0 && price.cache_write_micro_per_mtok.is_none() {
        format!("price: {}; cache writes at input price", price.source)
    } else {
        format!("price: {}", price.source)
    };
    Cost::Estimated { micro_usd: micro, source }
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
        Price {
            input_micro_per_mtok: 100_000,
            output_micro_per_mtok: 400_000,
            cached_input_micro_per_mtok: Some(25_000),
            cache_write_micro_per_mtok: None,
            source: "user".into(),
            observed_at: 0,
        }
    }

    #[test]
    fn estimates_from_a_price_and_counts_cached_tokens_apart() {
        // 1M prompt tokens of which 400k cached, 500k output: 0.6 × $0.10 + 0.4 × $0.025 + 0.5 × $0.40.
        let usage = Usage { prompt: 1_000_000, completion: 500_000, cached: 400_000, ..Usage::default() };
        let cost = resolve("paid", Some(&price()), Some(&usage));
        assert_eq!(cost, Cost::Estimated { micro_usd: 60_000 + 10_000 + 200_000, source: "price: user".into() });
    }

    #[test]
    fn reads_anthropic_usage_and_prices_cache_writes() {
        // Anthropic reports uncached input apart from cache reads and writes.
        let mut usage =
            Usage::from_anthropic(&serde_json::json!({"input_tokens": 100_000, "cache_read_input_tokens": 600_000,
            "cache_creation_input_tokens": 300_000, "output_tokens": 1}))
            .unwrap();
        usage.merge_anthropic(&serde_json::json!({"output_tokens": 500_000}));
        assert_eq!(
            (usage.prompt, usage.cached, usage.cache_write, usage.completion),
            (1_000_000, 600_000, 300_000, 500_000)
        );
        // Without a cache-write price, writes are priced as input and the source says so.
        let cost = resolve("paid", Some(&price()), Some(&usage));
        assert_eq!(
            cost,
            Cost::Estimated {
                micro_usd: 10_000 + 15_000 + 30_000 + 200_000,
                source: "price: user; cache writes at input price".into()
            }
        );
        let with_write = Price { cache_write_micro_per_mtok: Some(125_000), ..price() };
        assert_eq!(
            resolve("paid", Some(&with_write), Some(&usage)).micro_usd(),
            Some(10_000 + 15_000 + 37_500 + 200_000)
        );
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

    /// A zero read from the cache means "missed" only when the provider said so (ROADMAP item 4).
    #[test]
    fn says_whether_the_provider_reported_its_cache() {
        let silent = Usage::from_json(&serde_json::json!({"prompt_tokens": 5000, "completion_tokens": 7})).unwrap();
        assert!(!silent.cache_reported);
        let openai = Usage::from_json(&serde_json::json!({"prompt_tokens": 5000, "completion_tokens": 7,
            "prompt_tokens_details": {"cached_tokens": 0}}))
        .unwrap();
        assert!(openai.cache_reported && openai.cached == 0);
        // DeepSeek reports its own fields.
        let deepseek = Usage::from_json(&serde_json::json!({"prompt_tokens": 5000, "completion_tokens": 7,
            "prompt_cache_hit_tokens": 4096, "prompt_cache_miss_tokens": 904}))
        .unwrap();
        assert_eq!((deepseek.cache_reported, deepseek.cached), (true, 4096));
        let anthropic = Usage::from_anthropic(&serde_json::json!({"input_tokens": 5000, "output_tokens": 7})).unwrap();
        assert!(!anthropic.cache_reported);
        let anthropic = Usage::from_anthropic(&serde_json::json!({"input_tokens": 5000, "output_tokens": 7,
            "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0}))
        .unwrap();
        assert!(anthropic.cache_reported);
        let responses = Usage::from_responses(&serde_json::json!({"input_tokens": 5000, "output_tokens": 7,
            "input_tokens_details": {"cached_tokens": 0}}))
        .unwrap();
        assert!(responses.cache_reported);
    }

    #[test]
    fn provider_figures_win() {
        let usage = Usage::from_json(&serde_json::json!({"prompt_tokens": 5, "completion_tokens": 7,
            "prompt_tokens_details": {"cached_tokens": 2}, "cost": 0.00123}))
        .unwrap();
        assert_eq!((usage.prompt, usage.completion, usage.cached), (5, 7, 2));
        // OpenRouter's answer for an Anthropic model, as seen live: its cache write is in the same details.
        let written = Usage::from_json(&serde_json::json!({"prompt_tokens": 11937, "completion_tokens": 20,
            "prompt_tokens_details": {"cached_tokens": 0, "cache_write_tokens": 11933}, "cost": 0.001502025}))
        .unwrap();
        assert_eq!((written.cached, written.cache_write, written.cache_reported), (0, 11933, true));
        // Alibaba Model Studio's OpenAI-compatible answer to an explicit cache marker: both inside `prompt_tokens`.
        let qwen = Usage::from_json(&serde_json::json!({"prompt_tokens": 3000, "completion_tokens": 9,
            "prompt_tokens_details": {"cached_tokens": 1200, "cache_creation_input_tokens": 1790}}))
        .unwrap();
        assert_eq!((qwen.prompt, qwen.cached, qwen.cache_write), (3000, 1200, 1790));
        assert_eq!(
            crate::gateway::anthropic::usage_json(Some(&written)),
            serde_json::json!({ "input_tokens": 4, "output_tokens": 20, "cache_read_input_tokens": 0,
                "cache_creation_input_tokens": 11933 })
        );
        assert_eq!(
            resolve("paid", None, Some(&usage)),
            Cost::Actual { micro_usd: 1230, source: "provider response".into() }
        );
        assert_eq!(resolve("local", None, Some(&Usage::default())).micro_usd(), Some(0));
        assert_eq!(resolve_failure(Some(429)).micro_usd(), Some(0));
    }
}
