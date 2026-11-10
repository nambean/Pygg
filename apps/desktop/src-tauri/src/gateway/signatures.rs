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

    /// Remembers the signatures in a response or stream chunk.
    pub fn capture(&mut self, value: &Value) {
        for choice in value["choices"].as_array().into_iter().flatten() {
            for part in ["message", "delta"] {
                for call in choice[part]["tool_calls"].as_array().into_iter().flatten() {
                    if let (Some(id), Some(signature)) =
                        (call["id"].as_str(), call["extra_content"]["google"]["thought_signature"].as_str())
                    {
                        self.insert(id, signature);
                    }
                }
            }
        }
    }

    /// Prepares a request body for one target: Gemini gets every tool call signed, others get no
    /// `extra_content` at all, since strict APIs reject fields they do not know.
    pub fn apply(&self, body: &mut Value, gemini: bool) {
        let Some(messages) = body["messages"].as_array_mut() else { return };
        for message in messages {
            let Some(calls) = message["tool_calls"].as_array_mut() else { continue };
            for call in calls {
                let Some(call) = call.as_object_mut() else { continue };
                if !gemini {
                    call.remove("extra_content");
                    continue;
                }
                let signed = call.get("extra_content").and_then(|e| e["google"]["thought_signature"].as_str()).is_some();
                if !signed {
                    let id = call.get("id").and_then(Value::as_str).unwrap_or("");
                    let signature = self.get(id).unwrap_or(PLACEHOLDER).to_string();
                    call.insert("extra_content".into(), json!({ "google": { "thought_signature": signature } }));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signs_known_calls_and_strips_for_others() {
        let mut cache = SignatureCache::default();
        cache.capture(&json!({ "choices": [{ "delta": { "tool_calls": [{ "id": "call_1",
            "extra_content": { "google": { "thought_signature": "SIG" } } }] } }] }));
        let request = json!({ "messages": [{ "role": "assistant", "tool_calls": [{ "id": "call_1" }, { "id": "call_2" }] }] });

        let mut for_gemini = request.clone();
        cache.apply(&mut for_gemini, true);
        assert_eq!(for_gemini["messages"][0]["tool_calls"][0]["extra_content"]["google"]["thought_signature"], "SIG");
        assert_eq!(for_gemini["messages"][0]["tool_calls"][1]["extra_content"]["google"]["thought_signature"], PLACEHOLDER);

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
