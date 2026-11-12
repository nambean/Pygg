//! Test Model (SCREENS §5.4 and §6.2: "Testing a model is a separate step"): one short request to one model, through
//! the same routing, holds and ledger as any client's, so its cost is recorded like any other. The request asks for
//! one word with a small output limit; on a paid model it costs a fraction of a cent, and the interface says so first.

use super::pricing::Usage;
use super::{forward, record_success, store, Incoming, Mode, Shared, Surface};
use axum::http::HeaderMap;
use serde::Serialize;
use serde_json::{json, Value};

/// The client the tests are recorded under, so their cost shows in Usage like any other request.
pub const CLIENT: &str = "Pygg (model tests)";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub ok: bool,
    /// What the model answered, cut to 200 characters, or why it failed.
    pub text: String,
    pub latency_ms: u64,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    /// `native` or `translated:…`.
    pub route: String,
    /// The model that answered, as `connection/model`: for a combo, the member that took it.
    pub answered_by: String,
    /// Upstream calls the request made, the answering one included: more than one means members before it failed.
    pub attempts: usize,
}

fn test_client(shared: &Shared) -> Result<store::Client, String> {
    shared.with_db(|db| {
        if let Some(client) = store::list_clients(db)?.into_iter().find(|c| c.name == CLIENT && c.revoked_at.is_none()) {
            return Ok(client);
        }
        store::create_client(db, CLIENT, None).map(|(client, _token)| client)
    })
}

/// The answer's text in either format.
fn text_of(body: &Value) -> String {
    body["choices"][0]["message"]["content"].as_str()
        .map(String::from)
        .or_else(|| body["content"].as_array().map(|blocks| blocks.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("")))
        .unwrap_or_default()
}

/// Sends "Reply with exactly: ok" to `<provider>/<model>` and reports what came back.
pub async fn run(shared: &Shared, provider_id: &str, model_id: &str) -> Result<TestResult, String> {
    run_requested(shared, &format!("{provider_id}/{model_id}")).await
}

/// The same short request for any model name a client may send: a combo's name, `auto`, or `connection/model`. A combo
/// goes through its real route, so the answer says which member took it and how many calls it needed.
pub async fn run_requested(shared: &Shared, requested: &str) -> Result<TestResult, String> {
    let client = test_client(shared)?;
    let requested = requested.to_string();
    let body = json!({ "model": requested, "max_tokens": 16,
        "messages": [{ "role": "user", "content": "Reply with exactly: ok" }] });
    let headers = HeaderMap::new();
    let incoming = Incoming { surface: Surface::OpenAi, body: &body, headers: &headers, requested: &requested, session: None, stream: false };
    let started = std::time::Instant::now();
    let forwarded = match forward(shared, &client, &incoming).await {
        Ok(forwarded) => forwarded,
        Err(refusal) => return Ok(TestResult { ok: false, text: refusal.message, latency_ms: started.elapsed().as_millis() as u64,
            input_tokens: None, output_tokens: None, route: String::new(), answered_by: String::new(), attempts: 0 }),
    };
    let route = forwarded.mode.label().to_string();
    let answered_by = format!("{}/{}", forwarded.provider.id, forwarded.model_id);
    let attempts = forwarded.attempt;
    let anthropic_answer = forwarded.mode == Mode::OpenAiToAnthropic;
    let status = forwarded.response.status();
    let mut forwarded = forwarded;
    let response = std::mem::replace(&mut forwarded.response, super::status_only(status));
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    let parsed: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    let usage = if anthropic_answer { Usage::from_anthropic(&parsed["usage"]) } else { Usage::from_json(&parsed["usage"]) };
    let text: String = text_of(&parsed).chars().take(200).collect();
    let latency_ms = started.elapsed().as_millis() as u64;
    let (input_tokens, output_tokens) = (usage.as_ref().map(|u| u.prompt), usage.as_ref().map(|u| u.completion));
    record_success(shared, forwarded, usage.as_ref(), "ok", None);
    Ok(TestResult { ok: !text.trim().is_empty(), text: if text.trim().is_empty() { "The model answered with no text".into() } else { text },
        latency_ms, input_tokens, output_tokens, route, answered_by, attempts })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_answer_in_either_format() {
        assert_eq!(text_of(&json!({"choices": [{"message": {"content": "ok"}}]})), "ok");
        assert_eq!(text_of(&json!({"content": [{"type": "thinking", "thinking": "x"}, {"type": "text", "text": "ok"}]})), "ok");
        assert_eq!(text_of(&json!({})), "");
    }
}
