//! Token reduction in shadow (ROADMAP P1-4): what a request repeats, measured and never changed. An agent that
//! reads the same file twice sends its contents twice in every later request; deduplicating that would save input
//! tokens, but it rewrites the agent's history and breaks the prompt cache's prefix, so Pygg only measures it until
//! the figures say whether it is worth doing (and the cache cost is weighed).

use super::Surface;
use serde_json::Value;
use std::collections::HashSet;

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
