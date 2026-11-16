//! Token reduction in shadow (ROADMAP P1-4): what a request repeats, measured and never changed. An agent that
//! reads the same file twice sends its contents twice in every later request; deduplicating that would save input
//! tokens, but it rewrites the agent's history and breaks the prompt cache's prefix, so Pygg only measures it until
//! the figures say whether it is worth doing (and the cache cost is weighed).

use super::Surface;
use serde_json::Value;
use std::collections::HashSet;

/// What a request carries before its conversation: its tool definitions and its system prompt (ROADMAP item 4), as
/// estimated tokens and a fingerprint of both. Numbers only; the text is not kept.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Prefix {
    pub tools_tokens: u64,
    pub system_tokens: u64,
    /// The first eight bytes of a SHA-256 of the tools and the system prompt: equal fingerprints, same prefix.
    pub hash: i64,
}

/// The system prompt's text in any of the three formats: Anthropic's `system`, the leading `system` and `developer`
/// messages of Chat Completions, the Responses API's `instructions` and leading `system` and `developer` items.
fn system_text(surface: Surface, body: &Value) -> String {
    let leading = |items: &Value, content: &str| -> Vec<String> {
        items
            .as_array()
            .into_iter()
            .flatten()
            .take_while(|m| matches!(m["role"].as_str(), Some("system" | "developer")))
            .map(|m| text_of(&m[content]))
            .collect()
    };
    let parts = match surface {
        Surface::Anthropic => vec![text_of(&body["system"])],
        Surface::OpenAi => leading(&body["messages"], "content"),
        Surface::Responses => {
            let mut parts = vec![body["instructions"].as_str().unwrap_or_default().to_string()];
            parts.extend(leading(&body["input"], "content"));
            parts
        }
    };
    parts.into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join("\n")
}

/// The request's tool definitions and system prompt, measured. `None` when it carries neither.
pub fn prefix(surface: Surface, body: &Value) -> Option<Prefix> {
    use sha2::{Digest, Sha256};
    let tools = body["tools"].as_array().filter(|t| !t.is_empty()).map(|t| Value::Array(t.clone()).to_string());
    let system = system_text(surface, body);
    if tools.is_none() && system.is_empty() {
        return None;
    }
    let tools = tools.unwrap_or_default();
    let digest = Sha256::digest(format!("{tools}\0{system}").as_bytes());
    let mut first = [0u8; 8];
    first.copy_from_slice(&digest[..8]);
    Some(Prefix {
        tools_tokens: super::estimate_text_tokens(&tools),
        system_tokens: super::estimate_text_tokens(&system),
        hash: i64::from_be_bytes(first),
    })
}

/// Tool output shorter than this is not counted: short results ("ok", an exit code) repeat by nature.
const MIN_CHARS: usize = 200;

/// Text of a tool result in any of the three formats.
fn text_of(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

/// The tool outputs of a request, in order.
fn tool_outputs(surface: Surface, body: &Value) -> Vec<String> {
    let mut out = Vec::new();
    match surface {
        Surface::Anthropic => {
            for message in body["messages"].as_array().into_iter().flatten() {
                for block in message["content"].as_array().into_iter().flatten().filter(|b| b["type"] == "tool_result")
                {
                    out.push(text_of(&block["content"]));
                }
            }
        }
        Surface::OpenAi => {
            for message in body["messages"].as_array().into_iter().flatten().filter(|m| m["role"] == "tool") {
                out.push(text_of(&message["content"]));
            }
        }
        Surface::Responses => {
            for item in body["input"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|i| i["type"] == "function_call_output" || i["type"] == "custom_tool_call_output")
            {
                out.push(text_of(&item["output"]));
            }
        }
    }
    out
}

/// Anthropic's context editing at its defaults (`clear_tool_uses_20250919`): once the prompt passes this many tokens,
/// clear the old tool results.
pub const CLEAR_TRIGGER_TOKENS: u64 = 100_000;
/// … keeping this many of the most recent.
pub const CLEAR_KEEP: usize = 3;

/// What clearing old tool results would do to one request (ROADMAP item 4, shadow). Estimated tokens; nothing is cleared.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Clearing {
    /// Tool results that would be cleared; 0 below the trigger.
    pub count: u64,
    /// Their tokens, which would no longer be sent.
    pub tokens: u64,
    /// The tokens from the newest cleared result to the end of the conversation. When a request clears one more result
    /// than the one before it, this part no longer matches what the provider cached, and is written to the cache again.
    pub tail_tokens: u64,
}

/// What Anthropic's context editing, at its defaults, would clear from this request. Only for OpenAI-format clients
/// (Chat Completions and the Responses API): an Anthropic-format request is Claude Code's own, and its history is never
/// touched (ADR-036). `None` without tool output.
pub fn clearing(surface: Surface, body: &Value) -> Option<Clearing> {
    let (items, is_output, content): (&Value, fn(&Value) -> bool, &str) = match surface {
        Surface::Anthropic => return None,
        Surface::OpenAi => (&body["messages"], |m| m["role"] == "tool", "content"),
        Surface::Responses => (
            &body["input"],
            |i| i["type"] == "function_call_output" || i["type"] == "custom_tool_call_output",
            "output",
        ),
    };
    let items = items.as_array()?;
    let outputs: Vec<usize> = (0..items.len()).filter(|i| is_output(&items[*i])).collect();
    if outputs.is_empty() {
        return None;
    }
    if super::estimate_text_tokens(&body.to_string()) < CLEAR_TRIGGER_TOKENS || outputs.len() <= CLEAR_KEEP {
        return Some(Clearing::default());
    }
    let cleared = &outputs[..outputs.len() - CLEAR_KEEP];
    let newest = *cleared.last().expect("at least one result is cleared");
    Some(Clearing {
        count: cleared.len() as u64,
        tokens: cleared.iter().map(|i| super::estimate_text_tokens(&text_of(&items[*i][content]))).sum(),
        tail_tokens: items[newest..].iter().map(|m| super::estimate_text_tokens(&m.to_string())).sum(),
    })
}

/// About how many tokens of tool output the request repeats from earlier in itself: each output of at least
/// `MIN_CHARS` that is identical to an earlier one counts in full, at four characters a token (an estimate, as
/// named in the interface). `None` when the request has no tool output at all.
pub fn repeated_tokens(surface: Surface, body: &Value) -> Option<u64> {
    let outputs = tool_outputs(surface, body);
    if outputs.is_empty() {
        return None;
    }
    let mut seen = HashSet::new();
    let mut chars = 0usize;
    for text in outputs.into_iter().filter(|t| t.chars().count() >= MIN_CHARS) {
        let length = text.chars().count();
        if !seen.insert(text) {
            chars += length;
        }
    }
    Some(chars.div_ceil(4) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn measures_the_tools_and_system_prompt_in_each_format() {
        let tools = json!([{ "name": "read", "description": "Read a file", "input_schema": { "type": "object" } }]);
        let anthropic = json!({ "system": [{ "type": "text", "text": "You are terse." }], "tools": tools,
            "messages": [{ "role": "user", "content": "hi" }] });
        let a = prefix(Surface::Anthropic, &anthropic).unwrap();
        assert!(a.tools_tokens > 10 && a.system_tokens == 5, "{a:?}");

        // Chat Completions: only the leading system and developer messages are the system prompt.
        let chat = |system: &str| {
            json!({ "messages": [
            { "role": "system", "content": system }, { "role": "developer", "content": "Use tools." },
            { "role": "user", "content": "hi" }, { "role": "system", "content": "a late note, part of the history" }] })
        };
        let c = prefix(Surface::OpenAi, &chat("You are terse.")).unwrap();
        assert_eq!((c.tools_tokens, c.system_tokens), (0, 9));
        assert_eq!(c.hash, prefix(Surface::OpenAi, &chat("You are terse.")).unwrap().hash);
        assert_ne!(c.hash, prefix(Surface::OpenAi, &chat("You are brief.")).unwrap().hash);

        let responses = json!({ "instructions": "Be brief.", "tools": tools,
            "input": [{ "type": "message", "role": "developer", "content": [{ "type": "input_text", "text": "Use tools." }] },
                { "type": "message", "role": "user", "content": [{ "type": "input_text", "text": "hi" }] }] });
        let r = prefix(Surface::Responses, &responses).unwrap();
        assert_eq!(r.system_tokens, 7);
        assert_eq!(prefix(Surface::OpenAi, &json!({ "messages": [{ "role": "user", "content": "hi" }] })), None);
    }

    #[test]
    fn measures_what_clearing_old_tool_results_would_take_out() {
        let file = "x".repeat(30_000); // about 10,000 tokens each at the gateway's estimate
        let chat = |results: usize| {
            let mut messages = vec![json!({ "role": "user", "content": "go" })];
            for n in 0..results {
                messages.push(json!({ "role": "assistant", "tool_calls": [{ "id": format!("c{n}"), "type": "function",
                    "function": { "name": "read", "arguments": "{}" } }] }));
                messages.push(json!({ "role": "tool", "tool_call_id": format!("c{n}"), "content": file }));
            }
            json!({ "messages": messages })
        };
        // Under the trigger, or past it with no more than three results: nothing would be cleared.
        assert_eq!(clearing(Surface::OpenAi, &chat(4)), Some(Clearing::default()));
        assert_eq!(clearing(Surface::OpenAi, &json!({ "messages": [{ "role": "user", "content": "hi" }] })), None);
        // Twelve results of about 10,000 tokens: the oldest nine go, and the tail runs from the ninth to the end.
        let c = clearing(Surface::OpenAi, &chat(12)).unwrap();
        assert_eq!((c.count, c.tokens), (9, 90_000));
        assert!(c.tail_tokens > 40_000 && c.tail_tokens < 41_000, "{c:?}");

        let responses = json!({ "input": (0..12).map(|n| json!({ "type": "function_call_output", "call_id": format!("c{n}"),
            "output": file })).collect::<Vec<_>>() });
        assert_eq!(clearing(Surface::Responses, &responses).unwrap().count, 9);
        // Claude Code's own requests are never measured for clearing.
        let anthropic = json!({ "messages": [{ "role": "user", "content": [{ "type": "tool_result", "tool_use_id": "c",
            "content": file }] }] });
        assert_eq!(clearing(Surface::Anthropic, &anthropic), None);
    }

    #[test]
    fn counts_tool_output_that_a_request_repeats() {
        let file = "x".repeat(400);
        let anthropic = json!({"messages": [
            {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "a", "content": file}]},
            {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "b", "content": [{"type": "text", "text": file}]}]},
            {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "c", "content": "ok"}]},
            {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "d", "content": "ok"}]}]});
        assert_eq!(
            repeated_tokens(Surface::Anthropic, &anthropic),
            Some(100),
            "the second read of the file; short results not counted"
        );
        let openai = json!({"messages": [{"role": "tool", "content": file}, {"role": "user", "content": "go"}]});
        assert_eq!(repeated_tokens(Surface::OpenAi, &openai), Some(0));
        let responses = json!({"input": [{"type": "function_call_output", "output": file}, {"type": "function_call_output", "output": file}]});
        assert_eq!(repeated_tokens(Surface::Responses, &responses), Some(100));
        assert_eq!(repeated_tokens(Surface::OpenAi, &json!({"messages": [{"role": "user", "content": "hi"}]})), None);
    }
}
