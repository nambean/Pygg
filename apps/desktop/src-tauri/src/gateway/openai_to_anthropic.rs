//! OpenAI Chat Completions to an Anthropic connection (ROADMAP B5, N8), so Codex, the OpenAI SDK and other
//! OpenAI-format clients can reach Claude, and `auto` can rank Claude for them.
//!
//! - **Request:** system and developer messages become `system`; user and assistant turns keep their order, with
//!   consecutive turns of one role merged (Anthropic wants them to alternate); `tool` messages become
//!   `tool_result` blocks in a user turn; assistant `tool_calls` become `tool_use` blocks; images by URL or data
//!   URL cross; `tools`, `tool_choice`, `max_tokens` (Anthropic requires one: 4096 when the client set none),
//!   `temperature`, `top_p`, `stop` and `stream` cross. Everything else is named in `x-smarttaskrouter-dropped`.
//! - **Response:** text and tool calls come back as a chat completion, streamed or not, with the finish reason
//!   and usage (cached input counted in `prompt_tokens`, as OpenAI does). Thinking blocks are not carried.

use super::pricing::Usage;
use serde_json::{json, Value};

/// Output limit sent when the client set none: Anthropic requires `max_tokens`.
pub const DEFAULT_MAX_TOKENS: u64 = 4096;

fn text_of(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts.iter()
            .filter_map(|p| if p["type"] == "text" { p["text"].as_str().map(String::from) } else { None })
            .collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

fn image_block(part: &Value) -> Option<Value> {
    let url = part["image_url"]["url"].as_str().or_else(|| part["image_url"].as_str())?;
    if let Some(rest) = url.strip_prefix("data:") {
        let (media, data) = rest.split_once(";base64,")?;
        return Some(json!({ "type": "image", "source": { "type": "base64", "media_type": media, "data": data } }));
    }
    Some(json!({ "type": "image", "source": { "type": "url", "url": url } }))
}

fn user_blocks(content: &Value) -> Vec<Value> {
    match content {
        Value::String(s) if !s.is_empty() => vec![json!({ "type": "text", "text": s })],
        Value::Array(parts) => parts.iter().filter_map(|p| match p["type"].as_str() {
            Some("text") => p["text"].as_str().filter(|t| !t.is_empty()).map(|t| json!({ "type": "text", "text": t })),
            Some("image_url") => image_block(p),
            _ => None,
        }).collect(),
        _ => Vec::new(),
    }
}

fn arguments(raw: &Value) -> Value {
    match raw {
        Value::String(s) if s.trim().is_empty() => json!({}),
        Value::String(s) => serde_json::from_str(s).unwrap_or_else(|_| json!({ "_raw": s })),
        Value::Object(_) => raw.clone(),
        _ => json!({}),
    }
}

/// The Messages request for a Chat Completions request, and the names of what it could not carry.
pub fn to_anthropic(request: &Value) -> (Value, Vec<String>) {
    let mut dropped: Vec<String> = Vec::new();
    let mut system = Vec::new();
    let mut turns: Vec<(String, Vec<Value>)> = Vec::new();
    let push = |turns: &mut Vec<(String, Vec<Value>)>, role: &str, blocks: Vec<Value>| {
        if blocks.is_empty() {
            return;
        }
        match turns.last_mut() {
            Some((last, existing)) if last == role => existing.extend(blocks),
            _ => turns.push((role.to_string(), blocks)),
        }
    };
    for message in request["messages"].as_array().into_iter().flatten() {
        match message["role"].as_str().unwrap_or("user") {
            "system" | "developer" => {
                let text = text_of(&message["content"]);
                if !text.is_empty() {
                    system.push(text);
                }
            }
            "assistant" => {
                let mut blocks = Vec::new();
                let text = text_of(&message["content"]);
                if !text.is_empty() {
                    blocks.push(json!({ "type": "text", "text": text }));
                }
                for call in message["tool_calls"].as_array().into_iter().flatten() {
                    blocks.push(json!({ "type": "tool_use", "id": call["id"], "name": call["function"]["name"],
                        "input": arguments(&call["function"]["arguments"]) }));
                }
                push(&mut turns, "assistant", blocks);
            }
            "tool" => {
                let block = json!({ "type": "tool_result", "tool_use_id": message["tool_call_id"],
                    "content": text_of(&message["content"]) });
                push(&mut turns, "user", vec![block]);
            }
            _ => {
                let blocks = user_blocks(&message["content"]);
                push(&mut turns, "user", blocks);
            }
        }
    }
    // Tool results lead their user turn, as Anthropic expects them right after the calls.
    for (role, blocks) in turns.iter_mut() {
        if role == "user" {
            blocks.sort_by_key(|b| if b["type"] == "tool_result" { 0 } else { 1 });
        }
    }
    let messages: Vec<Value> = turns.into_iter().map(|(role, content)| json!({ "role": role, "content": content })).collect();
    let max_tokens = request["max_completion_tokens"].as_u64().or_else(|| request["max_tokens"].as_u64())
        .unwrap_or(DEFAULT_MAX_TOKENS);
    let mut out = json!({ "model": request["model"], "max_tokens": max_tokens, "messages": messages });
    if !system.is_empty() {
        out["system"] = json!(system.join("\n\n"));
    }
    for field in ["temperature", "top_p", "stream"] {
        if !request[field].is_null() {
            out[field] = request[field].clone();
        }
    }
    match &request["stop"] {
        Value::String(s) => out["stop_sequences"] = json!([s]),
        Value::Array(a) if !a.is_empty() => out["stop_sequences"] = json!(a),
        _ => {}
    }
    if let Some(tools) = request["tools"].as_array() {
        let tools: Vec<Value> = tools.iter().filter(|t| t["type"] == "function" || t.get("function").is_some())
            .map(|t| {
                let f = &t["function"];
                let mut tool = json!({ "name": f["name"], "input_schema": if f["parameters"].is_object() { f["parameters"].clone() } else { json!({ "type": "object" }) } });
                if let Some(d) = f["description"].as_str().filter(|d| !d.is_empty()) {
                    tool["description"] = json!(d);
                }
                tool
            }).collect();
        if !tools.is_empty() {
            out["tools"] = json!(tools);
            match &request["tool_choice"] {
                Value::String(s) if s == "required" => out["tool_choice"] = json!({ "type": "any" }),
                Value::String(s) if s == "none" => out["tool_choice"] = json!({ "type": "none" }),
                Value::Object(o) if o.get("function").is_some() =>
                    out["tool_choice"] = json!({ "type": "tool", "name": o["function"]["name"] }),
                _ => {}
            }
        }
    }
    for field in ["n", "logprobs", "top_logprobs", "presence_penalty", "frequency_penalty", "logit_bias", "seed",
        "response_format", "reasoning_effort", "modalities", "audio", "prediction", "web_search_options", "user",
        "service_tier", "store", "metadata", "parallel_tool_calls"] {
        if request.get(field).is_some_and(|v| !v.is_null()) {
            dropped.push(field.to_string());
        }
    }
    (out, dropped)
}

fn finish_reason(stop: Option<&str>) -> &'static str {
    match stop {
        Some("max_tokens") => "length",
        Some("tool_use") => "tool_calls",
        Some("refusal") => "content_filter",
        _ => "stop",
    }
}

/// OpenAI's usage object for Anthropic usage: cached input is part of `prompt_tokens`.
pub fn usage_json(usage: Option<&Usage>) -> Value {
    let u = usage.cloned().unwrap_or_default();
    json!({ "prompt_tokens": u.prompt, "completion_tokens": u.completion, "total_tokens": u.prompt + u.completion,
        "prompt_tokens_details": { "cached_tokens": u.cached } })
}

fn completion_id() -> String {
    format!("chatcmpl-{}", uuid::Uuid::new_v4().simple())
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// A chat completion for a Messages response, and its usage for the ledger.
pub fn from_anthropic(response: &Value, model: &str) -> (Value, Option<Usage>) {
    let mut text = String::new();
    let mut calls = Vec::new();
    for block in response["content"].as_array().into_iter().flatten() {
        match block["type"].as_str() {
            Some("text") => text.push_str(block["text"].as_str().unwrap_or("")),
            Some("tool_use") => calls.push(json!({ "id": block["id"], "type": "function", "function": {
                "name": block["name"], "arguments": serde_json::to_string(&block["input"]).unwrap_or_else(|_| "{}".into()) } })),
            _ => {}
        }
    }
    let usage = Usage::from_anthropic(&response["usage"]);
    let mut message = json!({ "role": "assistant", "content": if text.is_empty() && !calls.is_empty() { Value::Null } else { json!(text) } });
    if !calls.is_empty() {
        message["tool_calls"] = json!(calls);
    }
    let out = json!({ "id": completion_id(), "object": "chat.completion", "created": now_secs(), "model": model,
        "choices": [{ "index": 0, "message": message, "finish_reason": finish_reason(response["stop_reason"].as_str()) }],
        "usage": usage_json(usage.as_ref()) });
    (out, usage)
}

/// Turns Anthropic stream events into Chat Completions chunks.
pub struct ChunkTranslator {
    id: String,
    model: String,
    created: u64,
    /// The OpenAI tool-call index of each Anthropic content block that is a tool call.
    tool_index: std::collections::HashMap<u64, usize>,
    next_tool: usize,
    include_usage: bool,
    pub usage: Option<Usage>,
    finish: Option<String>,
}

impl ChunkTranslator {
    pub fn new(model: &str, include_usage: bool) -> ChunkTranslator {
        ChunkTranslator { id: completion_id(), model: model.into(), created: now_secs(), tool_index: Default::default(),
            next_tool: 0, include_usage, usage: None, finish: None }
    }

    fn chunk(&self, delta: Value, finish: Option<&str>) -> String {
        let data = json!({ "id": self.id, "object": "chat.completion.chunk", "created": self.created, "model": self.model,
            "choices": [{ "index": 0, "delta": delta, "finish_reason": finish }] });
        format!("data: {data}\n\n")
    }

    /// The chunks for one Anthropic event (its `data` object).
    pub fn push(&mut self, event: &Value) -> Vec<String> {
        let mut out = Vec::new();
        match event["type"].as_str() {
            Some("message_start") => {
                self.usage = Usage::from_anthropic(&event["message"]["usage"]);
                out.push(self.chunk(json!({ "role": "assistant", "content": "" }), None));
            }
            Some("content_block_start") if event["content_block"]["type"] == "tool_use" => {
                let index = self.next_tool;
                self.next_tool += 1;
                self.tool_index.insert(event["index"].as_u64().unwrap_or(0), index);
                let block = &event["content_block"];
                out.push(self.chunk(json!({ "tool_calls": [{ "index": index, "id": block["id"], "type": "function",
                    "function": { "name": block["name"], "arguments": "" } }] }), None));
            }
            Some("content_block_delta") => {
                let delta = &event["delta"];
                match delta["type"].as_str() {
                    Some("text_delta") => out.push(self.chunk(json!({ "content": delta["text"] }), None)),
                    Some("input_json_delta") => {
                        let index = self.tool_index.get(&event["index"].as_u64().unwrap_or(0)).copied().unwrap_or(0);
                        out.push(self.chunk(json!({ "tool_calls": [{ "index": index,
                            "function": { "arguments": delta["partial_json"] } }] }), None));
                    }
                    _ => {}
                }
            }
            Some("message_delta") => {
                match self.usage.as_mut() {
                    Some(usage) => usage.merge_anthropic(&event["usage"]),
                    None => self.usage = Usage::from_anthropic(&event["usage"]),
                }
                self.finish = event["delta"]["stop_reason"].as_str().map(String::from);
            }
            _ => {}
        }
        out
    }

    /// The closing chunks: the finish reason, the usage chunk when the client asked for it, and `[DONE]`.
    pub fn finish(&mut self) -> Vec<String> {
        let mut out = vec![self.chunk(json!({}), Some(finish_reason(self.finish.as_deref())))];
        if self.include_usage {
            let data = json!({ "id": self.id, "object": "chat.completion.chunk", "created": self.created, "model": self.model,
                "choices": [], "usage": usage_json(self.usage.as_ref()) });
            out.push(format!("data: {data}\n\n"));
        }
        out.push("data: [DONE]\n\n".into());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_a_request_with_tools_and_tool_results() {
        let request = json!({ "model": "anthropic/claude-haiku-5-5", "temperature": 0.2, "stop": "END", "seed": 7,
            "messages": [
                { "role": "system", "content": "Be brief." },
                { "role": "developer", "content": [{ "type": "text", "text": "Use tools." }] },
                { "role": "user", "content": [{ "type": "text", "text": "Read a.txt" },
                    { "type": "image_url", "image_url": { "url": "data:image/png;base64,QUJD" } }] },
                { "role": "assistant", "content": null, "tool_calls": [
                    { "id": "c1", "type": "function", "function": { "name": "read", "arguments": "{\"path\":\"a.txt\"}" } },
                    { "id": "c2", "type": "function", "function": { "name": "read", "arguments": "{\"path\":\"b.txt\"}" } }] },
                { "role": "tool", "tool_call_id": "c1", "content": "one" },
                { "role": "tool", "tool_call_id": "c2", "content": "two" },
                { "role": "user", "content": "Thanks" }],
            "tools": [{ "type": "function", "function": { "name": "read", "description": "Read a file",
                "parameters": { "type": "object", "properties": { "path": { "type": "string" } } } } }],
            "tool_choice": "required" });
        let (out, dropped) = to_anthropic(&request);
        assert_eq!(out["system"], "Be brief.\n\nUse tools.");
        assert_eq!(out["max_tokens"], DEFAULT_MAX_TOKENS);
        assert_eq!(out["stop_sequences"], json!(["END"]));
        assert_eq!(dropped, ["seed"]);
        let m = out["messages"].as_array().unwrap();
        assert_eq!(m.len(), 3, "user, assistant, then one user turn with both results and the text: {m:?}");
        assert_eq!(m[0]["content"][1]["source"], json!({ "type": "base64", "media_type": "image/png", "data": "QUJD" }));
        assert_eq!(m[1]["content"][0], json!({ "type": "tool_use", "id": "c1", "name": "read", "input": { "path": "a.txt" } }));
        assert_eq!(m[2]["content"][0]["type"], "tool_result");
        assert_eq!(m[2]["content"][1]["tool_use_id"], "c2");
        assert_eq!(m[2]["content"][2], json!({ "type": "text", "text": "Thanks" }));
        assert_eq!(out["tools"][0]["input_schema"]["properties"]["path"]["type"], "string");
        assert_eq!(out["tool_choice"], json!({ "type": "any" }));
    }

    #[test]
    fn translates_a_response_and_its_usage() {
        let response = json!({ "content": [{ "type": "thinking", "thinking": "hmm" }, { "type": "text", "text": "Reading." },
            { "type": "tool_use", "id": "toolu_1", "name": "read", "input": { "path": "a" } }], "stop_reason": "tool_use",
            "usage": { "input_tokens": 10, "cache_read_input_tokens": 90, "cache_creation_input_tokens": 0, "output_tokens": 5 } });
        let (out, usage) = from_anthropic(&response, "coding");
        let choice = &out["choices"][0];
        assert_eq!(choice["finish_reason"], "tool_calls");
        assert_eq!(choice["message"]["content"], "Reading.");
        assert_eq!(choice["message"]["tool_calls"][0]["function"]["arguments"], "{\"path\":\"a\"}");
        assert_eq!(out["usage"]["prompt_tokens"], 100);
        assert_eq!(out["usage"]["prompt_tokens_details"]["cached_tokens"], 90);
        assert_eq!(usage.unwrap().completion, 5);
    }

    #[test]
    fn translates_a_stream() {
        let mut t = ChunkTranslator::new("coding", true);
        let events = [
            json!({ "type": "message_start", "message": { "usage": { "input_tokens": 12, "output_tokens": 1 } } }),
            json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" } }),
            json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": "Hi" } }),
            json!({ "type": "content_block_start", "index": 1, "content_block": { "type": "tool_use", "id": "t1", "name": "read", "input": {} } }),
            json!({ "type": "content_block_delta", "index": 1, "delta": { "type": "input_json_delta", "partial_json": "{\"p\":" } }),
            json!({ "type": "content_block_delta", "index": 1, "delta": { "type": "input_json_delta", "partial_json": "1}" } }),
            json!({ "type": "message_delta", "delta": { "stop_reason": "tool_use" }, "usage": { "output_tokens": 7 } }),
        ];
        let mut chunks: Vec<String> = events.iter().flat_map(|e| t.push(e)).collect();
        chunks.extend(t.finish());
        let data: Vec<Value> = chunks.iter().filter_map(|c| c.strip_prefix("data: ")).map(str::trim)
            .filter(|d| *d != "[DONE]").map(|d| serde_json::from_str(d).unwrap()).collect();
        assert_eq!(data[1]["choices"][0]["delta"]["content"], "Hi");
        assert_eq!(data[2]["choices"][0]["delta"]["tool_calls"][0]["function"]["name"], "read");
        let args: String = data.iter().filter_map(|d| d["choices"][0]["delta"]["tool_calls"][0]["function"]["arguments"].as_str()).collect();
        assert_eq!(args, "{\"p\":1}");
        assert_eq!(data[5]["choices"][0]["finish_reason"], "tool_calls");
        assert_eq!(data[6]["usage"]["completion_tokens"], 7);
        assert_eq!(chunks.last().unwrap(), "data: [DONE]\n\n");
        assert_eq!(t.usage.unwrap().prompt, 12);
    }
}
