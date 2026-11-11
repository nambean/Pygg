//! Asking a connection which models it offers (`GET {endpoint}/models`), which doubles as its connection test.
//!
//! Where the list says more than the names, that is kept for auto routing (ROADMAP P0-5), with the list as
//! its source: OpenRouter reports each model's context length, maximum output, whether it takes tools and
//! images, and its list price. Groq and some others report a context window. Nothing is inferred from a
//! model's name.

use super::pricing::Price;
use super::store::{Listed, Provider};
use super::{presets, Shared};
use serde_json::Value;

pub async fn fetch_models(shared: &Shared, provider: &Provider) -> Result<Vec<Listed>, String> {
    let key = match &provider.credential_ref {
        Some(reference) => shared.secrets.read(reference)?,
        None => None,
    };
    let mut request = shared.http.get(format!("{}/models", provider.endpoint));
    if let Some(key) = &key {
        request = if provider.wire == "anthropic" {
            request.header("x-api-key", key).header("anthropic-version", super::ANTHROPIC_VERSION)
        } else {
            request.bearer_auth(key)
        };
    }
    let secrets: Vec<&str> = key.as_deref().into_iter().collect();
    let response = request
        .send()
        .await
        .map_err(|e| crate::redact::redact(&format!("Could not reach {}: {e}", provider.endpoint), &secrets))?;
    let status = response.status();
    let text = response.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        let detail: String = crate::redact::redact(&super::upstream_message(text.as_bytes()), &secrets);
        return Err(match status.as_u16() {
            401 | 403 => format!("The provider refused the key (HTTP {}): {detail}", status.as_u16()),
            _ => format!("The provider answered HTTP {}: {detail}", status.as_u16()),
        });
    }
    let parsed: Value = serde_json::from_str(&text).map_err(|_| "The provider's model list is not JSON".to_string())?;
    let prefix = presets::find(&provider.kind).and_then(|p| p.strip_model_prefix);
    Ok(parse_list(&parsed, prefix))
}

/// The source recorded with facts read from this connection's model list.
pub fn list_source(provider: &Provider) -> String {
    format!("provider: {} /models", provider.kind)
}

fn parse_list(parsed: &Value, prefix: Option<&str>) -> Vec<Listed> {
    let list = parsed["data"].as_array().or_else(|| parsed["models"].as_array()).cloned().unwrap_or_default();
    let mut out: Vec<Listed> = list
        .iter()
        .filter_map(|m| {
            let id = m["id"].as_str().or_else(|| m["name"].as_str())?;
            let id = match prefix {
                Some(prefix) => id.strip_prefix(prefix).unwrap_or(id),
                None => id,
            };
            Some(listed(id, m))
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out.dedup_by(|a, b| a.id == b.id);
    out
}

fn positive(v: &Value) -> Option<u64> {
    v.as_u64().filter(|n| *n > 0)
}

/// USD per token, as OpenRouter writes it ("0.000003"), to micro-USD per million tokens. Negative or missing
/// prices (OpenRouter's own router models report -1) give `None`.
fn per_token_usd(v: &Value) -> Option<u64> {
    let usd = v.as_str().and_then(|s| s.parse::<f64>().ok()).or_else(|| v.as_f64())?;
    (usd.is_finite() && usd >= 0.0).then(|| (usd * 1e12).round() as u64)
}

fn listed(id: &str, m: &Value) -> Listed {
    let context_tokens = positive(&m["context_length"]).or_else(|| positive(&m["context_window"]));
    let max_output_tokens = positive(&m["top_provider"]["max_completion_tokens"])
        .or_else(|| positive(&m["max_completion_tokens"]))
        .or_else(|| positive(&m["max_output_tokens"]));
    let supports_tools = m["supported_parameters"].as_array()
        .map(|p| p.iter().any(|v| v.as_str() == Some("tools")));
    let supports_vision = m["architecture"]["input_modalities"].as_array()
        .map(|p| p.iter().any(|v| v.as_str() == Some("image")));
    let pricing = &m["pricing"];
    let price = match (per_token_usd(&pricing["prompt"]), per_token_usd(&pricing["completion"])) {
        (Some(input), Some(output)) => Some(Price {
            input_micro_per_mtok: input,
            output_micro_per_mtok: output,
            cached_input_micro_per_mtok: per_token_usd(&pricing["input_cache_read"]),
            cache_write_micro_per_mtok: per_token_usd(&pricing["input_cache_write"]),
            source: "provider list price".into(),
            observed_at: crate::db::now_ms(),
        }),
        _ => None,
    };
    Listed { id: id.to_string(), context_tokens, max_output_tokens, supports_tools, supports_vision, price }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_what_openrouter_reports_and_nothing_more() {
        let list = json!({ "data": [
            { "id": "vendor/big", "context_length": 200000, "top_provider": { "max_completion_tokens": 64000 },
              "supported_parameters": ["tools", "temperature"], "architecture": { "input_modalities": ["text", "image"] },
              "pricing": { "prompt": "0.000003", "completion": "0.000015", "input_cache_read": "0.0000003" } },
            { "id": "openrouter/auto", "pricing": { "prompt": "-1", "completion": "-1" } },
            { "id": "plain" }
        ] });
        let out = parse_list(&list, None);
        let big = out.iter().find(|m| m.id == "vendor/big").unwrap();
        assert_eq!((big.context_tokens, big.max_output_tokens, big.supports_tools, big.supports_vision),
            (Some(200_000), Some(64_000), Some(true), Some(true)));
        let price = big.price.as_ref().unwrap();
        assert_eq!((price.input_micro_per_mtok, price.output_micro_per_mtok, price.cached_input_micro_per_mtok),
            (3_000_000, 15_000_000, Some(300_000)));
        assert!(out.iter().find(|m| m.id == "openrouter/auto").unwrap().price.is_none(), "router models have no price");
        assert_eq!(out.iter().find(|m| m.id == "plain").unwrap(), &Listed { id: "plain".into(), ..Listed::default() });
    }

    #[test]
    fn reads_a_context_window_and_strips_a_prefix() {
        let out = parse_list(&json!({ "data": [{ "id": "models/x", "context_window": 131072 }] }), Some("models/"));
        assert_eq!((out[0].id.as_str(), out[0].context_tokens, out[0].supports_tools), ("x", Some(131_072), None));
    }
}
