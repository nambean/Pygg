//! Gemini 3 signs each tool call it makes (`extra_content.google.thought_signature`) and refuses a later
//! request whose history contains that call without its signature. Clients drop the field: Anthropic
//! clients have nowhere to keep it, and most OpenAI clients ignore unknown fields. So the gateway remembers
//! signatures by tool-call id as responses pass through, and puts them back on requests to Gemini.

use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};

const CAPACITY: usize = 20_000;

/// Google's documented placeholder for a call whose signature is not known, e.g. one another model made.
const PLACEHOLDER: &str = "skip_thought_signature_validator";

#[derive(Default)]
pub struct SignatureCache {
    by_id: HashMap<String, String>,
    order: VecDeque<String>,
}

impl SignatureCache {
    pub fn insert(&mut self, id: &str, signature: &str) {
        if self.by_id.insert(id.to_string(), signature.to_string()).is_none() {
            self.order.push_back(id.to_string());
            while self.order.len() > CAPACITY {
                if let Some(old) = self.order.pop_front() {
                    self.by_id.remove(&old);
                }
            }
        }
    }

    pub fn get(&self, id: &str) -> Option<&str> {
        self.by_id.get(id).map(String::as_str)
    }

    /// Remembers the signatures in a response or stream chunk, and returns them (to be stored).
    pub fn capture(&mut self, value: &Value) -> Vec<(String, String)> {
        let mut found = Vec::new();
        for choice in value["choices"].as_array().into_iter().flatten() {
            for part in ["message", "delta"] {
                for call in choice[part]["tool_calls"].as_array().into_iter().flatten() {
                    if let (Some(id), Some(signature)) =
                        (call["id"].as_str(), call["extra_content"]["google"]["thought_signature"].as_str())
                    {
                        self.insert(id, signature);
                        found.push((id.to_string(), signature.to_string()));
                    }
                }
            }
        }
        found
    }

    /// The tool-call ids in a request whose signature is not in memory.
    pub fn missing(&self, body: &Value) -> Vec<String> {
        body["messages"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|m| m["tool_calls"].as_array().into_iter().flatten())
            .filter(|c| c["extra_content"]["google"]["thought_signature"].is_null())
            .filter_map(|c| c["id"].as_str())
            .filter(|id| !id.is_empty() && self.get(id).is_none())
            .map(String::from)
            .collect()
    }

    /// Prepares a request body for one target: Gemini gets every tool call signed, others get no
    /// `extra_content` at all, since strict APIs reject fields they do not know.
    pub fn apply(&self, body: &mut Value, gemini: bool) {
        // `get_mut`, not indexing: indexing a missing key would add it as null, and a provider may refuse a
        // `"tool_calls": null` on every message, or a `"messages": null` on a Responses API body.
        let Some(messages) = body.get_mut("messages").and_then(Value::as_array_mut) else { return };
        for message in messages {
            let Some(calls) = message.get_mut("tool_calls").and_then(Value::as_array_mut) else { continue };
            for call in calls {
                let Some(call) = call.as_object_mut() else { continue };
                if !gemini {
                    call.remove("extra_content");
                    continue;
                }
                let signed =
                    call.get("extra_content").and_then(|e| e["google"]["thought_signature"].as_str()).is_some();
                if !signed {
                    let id = call.get("id").and_then(Value::as_str).unwrap_or("");
                    let signature = self.get(id).unwrap_or(PLACEHOLDER).to_string();
                    call.insert("extra_content".into(), json!({ "google": { "thought_signature": signature } }));
                }
            }
        }
    }
}

/// How long a stored signature is kept: a conversation picked up after a restart is rarely older.
pub const KEEP_MS: i64 = 30 * 86_400_000;

/// Stores signatures so they outlast a restart (review GW-8), and forgets those older than `KEEP_MS`.
pub fn store(conn: &rusqlite::Connection, pairs: &[(String, String)]) -> rusqlite::Result<()> {
    let now = crate::db::now_ms();
    for (id, signature) in pairs {
        conn.execute(
            "INSERT OR IGNORE INTO tool_signatures (call_id, signature, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![id, signature, now],
        )?;
    }
    conn.execute("DELETE FROM tool_signatures WHERE created_at < ?1", [now - KEEP_MS])?;
    Ok(())
}

/// The stored signatures of `ids`.
pub fn load(conn: &rusqlite::Connection, ids: &[String]) -> rusqlite::Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    let mut statement = conn.prepare("SELECT signature FROM tool_signatures WHERE call_id = ?1")?;
    for id in ids {
        if let Some(signature) =
            rusqlite::OptionalExtension::optional(statement.query_row([id], |r| r.get::<_, String>(0)))?
        {
            out.push((id.clone(), signature));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_signature_outlasts_a_restart() {
        let conn = crate::db::open_in_memory().unwrap();
        let mut before = SignatureCache::default();
        let found = before.capture(&json!({ "choices": [{ "message": { "tool_calls": [{ "id": "call_9",
            "extra_content": { "google": { "thought_signature": "SIG9" } } }] } }] }));
        store(&conn, &found).unwrap();
        // A new process: an empty cache, the same store.
        let mut after = SignatureCache::default();
        let mut request =
            json!({ "messages": [{ "role": "assistant", "tool_calls": [{ "id": "call_9" }, { "id": "call_x" }] }] });
        let missing = after.missing(&request);
        assert_eq!(missing, ["call_9", "call_x"]);
        for (id, signature) in load(&conn, &missing).unwrap() {
            after.insert(&id, &signature);
        }
        after.apply(&mut request, true);
        assert_eq!(request["messages"][0]["tool_calls"][0]["extra_content"]["google"]["thought_signature"], "SIG9");
        assert_eq!(
            request["messages"][0]["tool_calls"][1]["extra_content"]["google"]["thought_signature"],
            PLACEHOLDER
        );
    }

    use super::*;

    #[test]
    fn signs_known_calls_and_strips_for_others() {
        let mut cache = SignatureCache::default();
        cache.capture(&json!({ "choices": [{ "delta": { "tool_calls": [{ "id": "call_1",
            "extra_content": { "google": { "thought_signature": "SIG" } } }] } }] }));
        let request =
            json!({ "messages": [{ "role": "assistant", "tool_calls": [{ "id": "call_1" }, { "id": "call_2" }] }] });

        let mut for_gemini = request.clone();
        cache.apply(&mut for_gemini, true);
        assert_eq!(for_gemini["messages"][0]["tool_calls"][0]["extra_content"]["google"]["thought_signature"], "SIG");
        assert_eq!(
            for_gemini["messages"][0]["tool_calls"][1]["extra_content"]["google"]["thought_signature"],
            PLACEHOLDER
        );

        let mut for_others = for_gemini.clone();
        cache.apply(&mut for_others, false);
        assert!(for_others["messages"][0]["tool_calls"][0].get("extra_content").is_none());
    }

    #[test]
    fn forgets_the_oldest_beyond_capacity() {
        let mut cache = SignatureCache::default();
        for i in 0..CAPACITY + 1 {
            cache.insert(&i.to_string(), "s");
        }
        assert!(cache.get("0").is_none());
        assert!(cache.get(&CAPACITY.to_string()).is_some());
    }
}
