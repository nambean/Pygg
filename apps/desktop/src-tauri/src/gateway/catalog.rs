//! Asking a connection which models it offers (`GET {endpoint}/models`), which doubles as its connection test.

use super::presets;
use super::store::Provider;
use super::Shared;
use serde_json::Value;

pub async fn fetch_models(shared: &Shared, provider: &Provider) -> Result<Vec<String>, String> {
    let key = match &provider.credential_ref {
        Some(reference) => shared.secrets.read(reference)?,
        None => None,
    };
    let mut request = shared.http.get(format!("{}/models", provider.endpoint));
    if let Some(key) = &key {
        request = request.bearer_auth(key);
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
    let list = parsed["data"].as_array().or_else(|| parsed["models"].as_array()).cloned().unwrap_or_default();
    let mut ids: Vec<String> = list
        .iter()
        .filter_map(|m| m["id"].as_str().or_else(|| m["name"].as_str()))
        .map(|id| match prefix {
            Some(prefix) => id.strip_prefix(prefix).unwrap_or(id).to_string(),
            None => id.to_string(),
        })
        .collect();
    ids.sort();
    ids.dedup();
    Ok(ids)
}
