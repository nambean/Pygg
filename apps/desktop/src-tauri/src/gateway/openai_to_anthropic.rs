//! OpenAI Chat Completions to an Anthropic connection (ROADMAP B5, N8), so Codex, the OpenAI SDK and other
//! OpenAI-format clients can reach Claude, and `auto` can rank Claude for them.
//!
//! - **Request:** system and developer messages become `system`; user and assistant turns keep their order, with
//!   consecutive turns of one role merged (Anthropic wants them to alternate); `tool` messages become
//!   `tool_result` blocks in a user turn; assistant `tool_calls` become `tool_use` blocks; images by URL or data
//!   URL cross; `tools`, `tool_choice`, `max_tokens` (Anthropic requires one: 4096 when the client set none),
//!   `temperature`, `top_p`, `stop` and `stream` cross. Everything else is named in `x-smarttaskrouter-dropped`.
//! - **Reasoning asked for (ROADMAP B5):** `reasoning_effort` (`minimal`, `low`, `medium`, `high`, `xhigh`, `max`)
//!   and DeepSeek's switch `thinking: {"type": "enabled"}` become Anthropic's `thinking`, per what the model takes
//!   (`ThinkingSupport`, from Pygg's model facts): `adaptive` with `display: "summarized"` (Claude 4.7 and later
//!   return empty thinking text without it) and the level as `output_config.effort` (the nearest lower level the
//!   model lists), or, on a model that takes only `enabled`, a `budget_tokens` for the level inside `max_tokens`
//!   (raised by the budget when the client set no limit). `temperature` and `top_p` are then left out (Anthropic
//!   takes neither with thinking), and thinking is not asked for with a forced tool choice, which Anthropic
//!   refuses with it. `reasoning_effort: "none"` and `thinking: {"type": "disabled"}` send nothing: Claude's own
//!   default applies, and the field is named as dropped. A model that does not think, or whose facts are unknown,
//!   gets no thinking, and the field is named.
//! - **Response:** text and tool calls come back as a chat completion, streamed or not, with the finish reason
//!   and usage (cached input counted in `prompt_tokens`, as OpenAI does). Thinking blocks come back as
//!   `reasoning_content` on the message (and in `delta`, from `thinking_delta`), the field DeepSeek, Qwen and
//!   OpenRouter clients read; `redacted_thinking` is dropped.
//! - **Not carried back:** a thinking block is only valid with its signature, which has no place in Chat
//!   Completions, so the `reasoning_content` a client sends back on an earlier assistant turn is not sent to
//!   Anthropic (it is named as dropped). Claude thinks afresh each turn.

use super::pricing::Usage;
use serde_json::{json, Value};

/// Output limit sent when the client set none and the model's own is unknown: Anthropic requires `max_tokens`, while an
/// OpenAI-format client (Codex) that sets none expects the model's own room. 4096 cut agents off mid-file (the cache
/// benchmark of 2026-11-14: "Incomplete response returned, reason: max_output_tokens"), so a streamed request gets
/// `DEFAULT_STREAMED_MAX_TOKENS` and an unstreamed one this, either capped at the model's maximum output where known.
pub const DEFAULT_MAX_TOKENS: u64 = 16_000;
pub const DEFAULT_STREAMED_MAX_TOKENS: u64 = 64_000;

/// The output limit for a request whose client set none.
fn default_max_tokens(request: &Value, model_max: Option<u64>) -> u64 {
    let room = if request["stream"] == true { DEFAULT_STREAMED_MAX_TOKENS } else { DEFAULT_MAX_TOKENS };
    model_max.filter(|m| *m > 0).map_or(room, |m| m.min(room))
}

/// What a Claude model takes for thinking, from Pygg's model facts (`store::Capabilities`); `None` is unknown.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThinkingSupport {
    /// `adaptive`, `enabled`.
    pub types: Option<Vec<String>>,
    /// `low` to `max`.
    pub efforts: Option<Vec<String>>,
    /// The model's maximum output, where known: the room a request without a limit of its own gets, at most.
    pub max_output: Option<u64>,
}

/// Effort levels from least to most.
const LEVELS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// The thinking budget for a level, on a model that takes only `budget_tokens`; `None` is the model's default level.
fn budget_for(level: Option<&str>) -> u64 {
    match level {
        Some("low") => 2048,
        Some("high") => 16_384,
        Some("xhigh" | "max") => 32_000,
        _ => 8192,
    }
}

/// Anthropic's minimum `budget_tokens`.
const MIN_BUDGET: u64 = 1024;

/// What the client asked for: `Some(None)` is thinking at the model's default level, `Some(Some(level))` a level.
/// The names are the fields that asked; `off` names a field that asked for no thinking.
struct Asked {
    want: Option<Option<&'static str>>,
    fields: Vec<&'static str>,
    off: Vec<String>,
    unknown: Vec<String>,
}

fn asked(request: &Value) -> Asked {
    let mut a = Asked { want: None, fields: Vec::new(), off: Vec::new(), unknown: Vec::new() };
    let level = match request["reasoning_effort"].as_str() {
        None => None,
        Some("none") => { a.off.push("reasoning_effort (none)".into()); None }
        Some("minimal" | "low") => Some("low"),
        Some("medium") => Some("medium"),
        Some("high") => Some("high"),
        Some("xhigh") => Some("xhigh"),
        Some("max") => Some("max"),
        Some(_) => { a.unknown.push("reasoning_effort".into()); None }
    };
    if level.is_some() {
        a.fields.push("reasoning_effort");
    }
    // DeepSeek (and GLM): `thinking: {"type": "enabled" | "disabled"}`.
    let switch = match request["thinking"]["type"].as_str() {
        Some("enabled") => { a.fields.push("thinking"); Some(true) }
        Some("disabled") => { a.off.push("thinking (disabled)".into()); Some(false) }
        Some(_) => { a.unknown.push("thinking".into()); None }
        None if !request["thinking"].is_null() => { a.unknown.push("thinking".into()); None }
        None => None,
    };
    a.want = match (level, switch) {
        (Some(_), Some(false)) => { a.off.push("reasoning_effort (thinking disabled)".into()); None }
        (_, Some(false)) => None,
        (Some(level), _) => Some(Some(level)),
        (None, Some(true)) if request["reasoning_effort"] != "none" => Some(None),
        _ => None,
    };
    if a.want.is_none() {
        a.fields.clear();
    }
    a
}

/// Sets `thinking` (and `output_config.effort`) on a Messages request, or names why not.
fn apply_thinking(request: &Value, out: &mut Value, support: &ThinkingSupport, client_max: Option<u64>, dropped: &mut Vec<String>) {
    let a = asked(request);
    dropped.extend(a.off);
    dropped.extend(a.unknown);
    let Some(level) = a.want else { return };
    let name = |why: &str| a.fields.iter().map(|f| format!("{f} ({why})")).collect::<Vec<_>>();
    if matches!(out["tool_choice"]["type"].as_str(), Some("any" | "tool")) {
        dropped.extend(name("with a forced tool choice"));
        return;
    }
    let Some(types) = &support.types else {
        dropped.extend(name("unknown whether the model thinks"));
        return;
    };
    let takes = |t: &str| types.iter().any(|x| x == t);
    // The level, or the nearest lower one the model lists.
    let effort = level.and_then(|level| {
        let efforts = support.efforts.as_ref()?;
        let at = LEVELS.iter().position(|l| *l == level)?;
        LEVELS[..=at].iter().rev().find(|l| efforts.iter().any(|e| e == *l)).copied()
    });
    if takes("adaptive") {
        out["thinking"] = json!({ "type": "adaptive", "display": "summarized" });
        match effort {
            Some(effort) => out["output_config"]["effort"] = json!(effort),
            None if level.is_some() => dropped.push("reasoning_effort (level)".into()),
            None => {}
        }
    } else if takes("enabled") {
        let wanted = budget_for(level);
        let (budget, max_tokens) = match client_max {
            Some(max) => (wanted.min(max.saturating_sub(MIN_BUDGET)), max),
            None => (wanted, default_max_tokens(request, support.max_output) + wanted),
        };
        if budget < MIN_BUDGET {
            dropped.extend(name("max_tokens too small for thinking"));
            return;
        }
        out["max_tokens"] = json!(max_tokens);
        out["thinking"] = json!({ "type": "enabled", "budget_tokens": budget });
        if let Some(effort) = effort {
            out["output_config"]["effort"] = json!(effort);
        }
    } else {
        dropped.extend(name("the model does not think"));
        return;
    }
    // Anthropic takes neither with thinking.
    for field in ["temperature", "top_p"] {
        if out.as_object_mut().and_then(|o| o.remove(field)).is_some() {
            dropped.push(format!("{field} (with thinking)"));
        }
    }
}

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

/// The Messages request for a Chat Completions request, and the names of what it could not carry. `thinking` is
/// what the target model takes, for `reasoning_effort`.
pub fn to_anthropic(request: &Value, thinking: &ThinkingSupport) -> (Value, Vec<String>) {
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
                // Without its signature a thinking block is refused, so earlier reasoning stays behind.
                if message["reasoning_content"].as_str().is_some_and(|r| !r.is_empty()) && !dropped.iter().any(|d| d == "reasoning_content") {
                    dropped.push("reasoning_content".into());
                }
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
    let client_max = request["max_completion_tokens"].as_u64().or_else(|| request["max_tokens"].as_u64());
    let max_tokens = client_max.unwrap_or_else(|| default_max_tokens(request, thinking.max_output));
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
        "response_format", "modalities", "audio", "prediction", "web_search_options", "user",
        "service_tier", "store", "metadata", "parallel_tool_calls"] {
        if request.get(field).is_some_and(|v| !v.is_null()) {
            dropped.push(field.to_string());
        }
    }
    apply_thinking(request, &mut out, thinking, client_max, &mut dropped);
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
    let mut reasoning = String::new();
    let mut calls = Vec::new();
    for block in response["content"].as_array().into_iter().flatten() {
        match block["type"].as_str() {
            Some("text") => text.push_str(block["text"].as_str().unwrap_or("")),
            // Empty when the model was not asked to show it (`display: "omitted"`).
            Some("thinking") => {
                let thought = block["thinking"].as_str().unwrap_or("");
                if !thought.is_empty() {
                    if !reasoning.is_empty() {
                        reasoning.push('\n');
                    }
                    reasoning.push_str(thought);
                }
            }
            Some("tool_use") => calls.push(json!({ "id": block["id"], "type": "function", "function": {
                "name": block["name"], "arguments": serde_json::to_string(&block["input"]).unwrap_or_else(|_| "{}".into()) } })),
            _ => {}
        }
    }
    let usage = Usage::from_anthropic(&response["usage"]);
    let mut message = json!({ "role": "assistant", "content": if text.is_empty() && !calls.is_empty() { Value::Null } else { json!(text) } });
    if !reasoning.is_empty() {
        message["reasoning_content"] = json!(reasoning);
    }
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
    /// Reasoning has been sent, and a later thinking block has started since.
    thought: bool,
    new_thought: bool,
}

impl ChunkTranslator {
    pub fn new(model: &str, include_usage: bool) -> ChunkTranslator {
        ChunkTranslator { id: completion_id(), model: model.into(), created: now_secs(), tool_index: Default::default(),
            next_tool: 0, include_usage, usage: None, finish: None, thought: false, new_thought: false }
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
            Some("content_block_start") if event["content_block"]["type"] == "thinking" => self.new_thought = self.thought,
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
                    Some("thinking_delta") if delta["thinking"].as_str().is_some_and(|t| !t.is_empty()) => {
                        let thought = delta["thinking"].as_str().unwrap_or("");
                        // A later thinking block (between tool calls) starts on its own line, as when not streamed.
                        let text = if std::mem::take(&mut self.new_thought) { format!("\n{thought}") } else { thought.to_string() };
                        self.thought = true;
                        out.push(self.chunk(json!({ "reasoning_content": text }), None));
                    }
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
        let (out, dropped) = to_anthropic(&request, &ThinkingSupport::default());
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

    /// An agent that sets no output limit gets room to write a whole file (the cache benchmark, 2026-11-14), never more
    /// than the model takes; a limit the client set is kept.
    #[test]
    fn a_request_without_an_output_limit_gets_the_models_room() {
        let streamed = json!({ "model": "m", "stream": true, "messages": [{ "role": "user", "content": "hi" }] });
        assert_eq!(to_anthropic(&streamed, &ThinkingSupport::default()).0["max_tokens"], DEFAULT_STREAMED_MAX_TOKENS);
        let small = ThinkingSupport { max_output: Some(8_192), ..ThinkingSupport::default() };
        assert_eq!(to_anthropic(&streamed, &small).0["max_tokens"], 8_192);
        let mut limited = streamed.clone();
        limited["max_tokens"] = json!(500);
        assert_eq!(to_anthropic(&limited, &small).0["max_tokens"], 500);
    }

    fn support(types: &[&str], efforts: Option<&[&str]>) -> ThinkingSupport {
        ThinkingSupport { types: Some(types.iter().map(|t| t.to_string()).collect()),
            efforts: efforts.map(|e| e.iter().map(|l| l.to_string()).collect()), max_output: None }
    }

    /// ROADMAP B5: a model that takes only `budget_tokens` (Haiku 4.5) gets a budget for the level, inside
    /// `max_tokens`, which Anthropic counts the thinking in as OpenAI counts reasoning in `max_completion_tokens`.
    #[test]
    fn reasoning_effort_becomes_a_budget_where_the_model_takes_no_adaptive_thinking() {
        let old = support(&["enabled"], Some(&[]));
        let request = json!({ "model": "m", "reasoning_effort": "high", "top_p": 0.5, "messages": [{ "role": "user", "content": "hi" }] });
        let (out, dropped) = to_anthropic(&request, &old);
        assert_eq!(out["thinking"], json!({ "type": "enabled", "budget_tokens": 16_384 }));
        assert_eq!(out["max_tokens"], DEFAULT_MAX_TOKENS + 16_384, "no client limit: the answer keeps its default room");
        assert!(out.get("output_config").is_none() && out.get("top_p").is_none());
        assert_eq!(dropped, ["top_p (with thinking)"]);

        let limited = json!({ "model": "m", "reasoning_effort": "high", "max_completion_tokens": 3000, "messages": [] });
        let (out, _) = to_anthropic(&limited, &old);
        assert_eq!((out["max_tokens"].as_u64(), out["thinking"]["budget_tokens"].as_u64()), (Some(3000), Some(1976)));
        let tiny = json!({ "model": "m", "reasoning_effort": "low", "max_tokens": 1500, "messages": [] });
        let (out, dropped) = to_anthropic(&tiny, &old);
        assert!(out["thinking"].is_null());
        assert_eq!(dropped, ["reasoning_effort (max_tokens too small for thinking)"]);

        let none = support(&[], Some(&[]));
        let (out, dropped) = to_anthropic(&request, &none);
        assert!(out["thinking"].is_null());
        assert_eq!(out["top_p"], 0.5, "kept when no thinking is asked for");
        assert_eq!(dropped, ["reasoning_effort (the model does not think)"]);

        let off = json!({ "model": "m", "reasoning_effort": "high", "thinking": { "type": "disabled" }, "messages": [] });
        let (out, dropped) = to_anthropic(&off, &support(&["adaptive"], Some(&["high"])));
        assert!(out["thinking"].is_null());
        assert_eq!(dropped, ["thinking (disabled)", "reasoning_effort (thinking disabled)"]);
    }

    #[test]
    fn thinking_comes_back_as_reasoning_content_and_redacted_thinking_does_not() {
        let response = json!({ "content": [{ "type": "thinking", "thinking": "First.", "signature": "s1" },
            { "type": "redacted_thinking", "data": "xyz" }, { "type": "thinking", "thinking": "", "signature": "s2" },
            { "type": "thinking", "thinking": "Second.", "signature": "s3" }, { "type": "text", "text": "Done." }],
            "stop_reason": "end_turn", "usage": { "input_tokens": 1, "output_tokens": 2 } });
        let (out, _) = from_anthropic(&response, "m");
        assert_eq!(out["choices"][0]["message"], json!({ "role": "assistant", "content": "Done.", "reasoning_content": "First.\nSecond." }));

        let mut t = ChunkTranslator::new("m", false);
        let events = [
            json!({ "type": "message_start", "message": { "usage": { "input_tokens": 1 } } }),
            json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "thinking", "thinking": "", "signature": "" } }),
            json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "thinking_delta", "thinking": "First." } }),
            json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "signature_delta", "signature": "s1" } }),
            json!({ "type": "content_block_start", "index": 1, "content_block": { "type": "tool_use", "id": "t1", "name": "read", "input": {} } }),
            json!({ "type": "content_block_start", "index": 2, "content_block": { "type": "thinking", "thinking": "", "signature": "" } }),
            json!({ "type": "content_block_delta", "index": 2, "delta": { "type": "thinking_delta", "thinking": "Second." } }),
        ];
        let reasoning: Vec<String> = events.iter().flat_map(|e| t.push(e))
            .filter_map(|c| serde_json::from_str::<Value>(c.strip_prefix("data: ")?.trim()).ok())
            .filter_map(|d| d["choices"][0]["delta"]["reasoning_content"].as_str().map(String::from)).collect();
        assert_eq!(reasoning, ["First.", "\nSecond."]);
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
