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
    fetch_list(shared, provider).await.map_err(|e| e.message)
}

/// Why a model list could not be read: the provider's HTTP status when it answered, and Pygg's wording.
#[derive(Debug)]
pub struct ListError {
    pub status: Option<u16>,
    pub message: String,
}

/// How long reading a model list may take: it is a check, not a generation, and one slow provider must not hold up
/// the others (Codex's fourth review, A13).
pub const LIST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// `fetch_models`, keeping the status of a refusal.
pub async fn fetch_list(shared: &Shared, provider: &Provider) -> Result<Vec<Listed>, ListError> {
    let fail = |status: Option<u16>, message: String| ListError { status, message };
    let key = match &provider.credential_ref {
        Some(reference) => shared.secrets.read(reference).map_err(|e| fail(None, e))?,
        None => None,
    };
    let mut request = shared.http.get(format!("{}/models", provider.endpoint)).timeout(LIST_TIMEOUT);
    if let Some(key) = &key {
        request = if provider.wire == "anthropic" {
            request.header("x-api-key", key).header("anthropic-version", super::ANTHROPIC_VERSION)
        } else {
            request.bearer_auth(key)
        };
    }
    let secrets: Vec<&str> = key.as_deref().into_iter().collect();
    let response = request.send().await.map_err(|e| {
        fail(None, crate::redact::redact(&format!("Could not reach {}: {e}", provider.endpoint), &secrets))
    })?;
    let status = response.status();
    let text = response.text().await.map_err(|e| fail(None, e.to_string()))?;
    if !status.is_success() {
        let detail: String = crate::redact::redact(&super::upstream_message(text.as_bytes()), &secrets);
        return Err(fail(
            Some(status.as_u16()),
            match status.as_u16() {
                401 | 403 => format!("The provider refused the key (HTTP {}): {detail}", status.as_u16()),
                _ => format!("The provider answered HTTP {}: {detail}", status.as_u16()),
            },
        ));
    }
    let parsed: Value =
        serde_json::from_str(&text).map_err(|_| fail(None, "The provider's model list is not JSON".into()))?;
    // An answer without a list is not one, even an empty JSON object (Codex's fourth review, A11).
    if parsed["data"].as_array().or_else(|| parsed["models"].as_array()).is_none() {
        return Err(fail(None, "The provider's answer is not a model list".into()));
    }
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
    // Anthropic reports the input limit (`max_input_tokens`) apart from the output limit; taking the input limit
    // as the context is the stricter reading, since auto checks prompt plus output against it.
    // Mistral writes `max_context_length`; Moonshot and xAI `context_length`; DeepSeek and Groq `context_window`.
    let context_tokens = positive(&m["context_length"])
        .or_else(|| positive(&m["context_window"]))
        .or_else(|| positive(&m["max_context_length"]))
        .or_else(|| positive(&m["max_input_tokens"]));
    let max_output_tokens = positive(&m["top_provider"]["max_completion_tokens"])
        .or_else(|| positive(&m["max_completion_tokens"]))
        .or_else(|| positive(&m["max_output_tokens"]))
        .or_else(|| positive(&m["max_tokens"]));
    // Only an explicit field counts: Anthropic's list has none for tool use, so that stays unknown there.
    let supports_tools = m["supported_parameters"]
        .as_array()
        .map(|p| p.iter().any(|v| v.as_str() == Some("tools")))
        // Mistral: `capabilities.function_calling`.
        .or_else(|| m["capabilities"]["function_calling"].as_bool());
    let supports_vision = m["architecture"]["input_modalities"]
        .as_array()
        .map(|p| p.iter().any(|v| v.as_str() == Some("image")))
        .or_else(|| m["capabilities"]["image_input"]["supported"].as_bool())
        // Mistral: `capabilities.vision`; Moonshot: `supports_image_in`.
        .or_else(|| m["capabilities"]["vision"].as_bool())
        .or_else(|| m["supports_image_in"].as_bool());
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
    // Anthropic: `capabilities.thinking.types.<type>.supported` and `capabilities.effort.<level>.supported`.
    let thinking_types = supported_keys(&m["capabilities"]["thinking"]["types"]);
    let effort_levels = supported_keys(&m["capabilities"]["effort"]);
    Listed {
        id: id.to_string(),
        context_tokens,
        max_output_tokens,
        supports_tools,
        supports_vision,
        thinking_types,
        effort_levels,
        price,
    }
}

/// The keys of an object whose `supported` is true (`{"adaptive": {"supported": true}, ...}`); `None` when the
/// object is not there, so that what the provider did not say stays unknown.
fn supported_keys(v: &Value) -> Option<Vec<String>> {
    let object = v.as_object()?;
    Some(
        object
            .iter()
            .filter(|(k, v)| k.as_str() != "supported" && v["supported"] == true)
            .map(|(k, _)| k.clone())
            .collect(),
    )
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
        assert_eq!(
            (big.context_tokens, big.max_output_tokens, big.supports_tools, big.supports_vision),
            (Some(200_000), Some(64_000), Some(true), Some(true))
        );
        let price = big.price.as_ref().unwrap();
        assert_eq!(
            (price.input_micro_per_mtok, price.output_micro_per_mtok, price.cached_input_micro_per_mtok),
            (3_000_000, 15_000_000, Some(300_000))
        );
        assert!(out.iter().find(|m| m.id == "openrouter/auto").unwrap().price.is_none(), "router models have no price");
        assert_eq!(out.iter().find(|m| m.id == "plain").unwrap(), &Listed { id: "plain".into(), ..Listed::default() });
    }

    #[test]
    fn reads_anthropics_limits_and_image_support_but_not_tools() {
        let list = json!({ "data": [{ "type": "model", "id": "claude-x", "max_input_tokens": 1000000, "max_tokens": 128000,
            "capabilities": { "image_input": { "supported": true }, "structured_outputs": { "supported": true },
                "thinking": { "supported": true, "types": { "enabled": { "supported": false }, "adaptive": { "supported": true } } },
                "effort": { "supported": true, "low": { "supported": true }, "high": { "supported": true }, "max": { "supported": false } } } },
            { "type": "model", "id": "claude-old", "capabilities": { "thinking": { "supported": true, "types": { "enabled": { "supported": true },
                "adaptive": { "supported": false } } }, "effort": { "supported": false } } }] });
        let out = parse_list(&list, None);
        let m = out.iter().find(|m| m.id == "claude-x").unwrap();
        assert_eq!(
            (m.context_tokens, m.max_output_tokens, m.supports_vision, m.supports_tools),
            (Some(1_000_000), Some(128_000), Some(true), None)
        );
        assert_eq!(m.thinking_types.as_deref(), Some(&["adaptive".to_string()][..]));
        assert_eq!(m.effort_levels.as_deref(), Some(&["low".to_string(), "high".to_string()][..]));
        let old = out.iter().find(|m| m.id == "claude-old").unwrap();
        assert_eq!(
            (old.thinking_types.as_deref(), old.effort_levels.as_deref()),
            (Some(&["enabled".to_string()][..]), Some(&[][..]))
        );
    }

    #[test]
    fn reads_mistral_and_moonshot_fields() {
        let list = json!({ "data": [
            { "id": "mistral-small", "max_context_length": 32768, "capabilities": { "function_calling": true, "vision": false } },
            { "id": "kimi-k2", "context_length": 262144, "supports_image_in": true } ] });
        let out = parse_list(&list, None);
        assert_eq!(
            (out[1].context_tokens, out[1].supports_tools, out[1].supports_vision),
            (Some(32_768), Some(true), Some(false))
        );
        assert_eq!(
            (out[0].context_tokens, out[0].supports_tools, out[0].supports_vision),
            (Some(262_144), None, Some(true))
        );
    }

    #[test]
    fn reads_a_context_window_and_strips_a_prefix() {
        let out = parse_list(&json!({ "data": [{ "id": "models/x", "context_window": 131072 }] }), Some("models/"));
        assert_eq!((out[0].id.as_str(), out[0].context_tokens, out[0].supports_tools), ("x", Some(131_072), None));
    }
}
