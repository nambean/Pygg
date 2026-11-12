//! The Anthropic Messages surface (`POST /v1/messages`), so Claude Code and other Anthropic clients can use
//! any connection, through the same routing, fallback, budgets and ledger as `/v1/chat/completions`.
//!
//! - **To an Anthropic connection** it is a native-format semantic passthrough (ROADMAP P0-1, counter-review
//!   CR-08): the JSON body is parsed and written again in the same key order with only `model` replaced, so
//!   cache markers, thinking, citations and every other field reach the provider. `anthropic-version` and
//!   `anthropic-beta` are forwarded; other client headers are not. The provider's answer body comes back
//!   unchanged, with its rate-limit and request-id headers.
//! - **To an OpenAI-compatible connection** it is translated to Chat Completions and back, streamed or not.
//!   Text, images, tool definitions, tool calls and tool results cross. Cache markers cross only to providers that
//!   document accepting them on content parts (OpenRouter); otherwise they are dropped, and the response says so
//!   in `x-smarttaskrouter-dropped`.
//! - **Reasoning** (ROADMAP B5): a provider's `reasoning_content` (Qwen, DeepSeek, Kimi, GLM) or `reasoning`
//!   (OpenRouter) comes back as a `thinking` block signed `REASONING_SIGNATURE`, streamed or not, and earlier
//!   thinking goes back as `reasoning_content` to providers that document taking it (`Preset::takes_reasoning`;
//!   DeepSeek answers HTTP 400 in thinking mode with tools without it). The `thinking` setting itself is not
//!   translated. A block Pygg signed is never sent to Anthropic, which checks signatures.

use super::pricing::Usage;
use super::{authenticate, forward, parse_body, provider_headers, pump, record_success, route_headers, session_key,
    status_only, stream_response, Incoming, Mode, Refusal, Shared, Surface};
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::StreamExt;
use serde_json::{json, Map, Value};
use tokio::sync::mpsc;

pub fn anthropic_error(status: StatusCode, kind: &str, message: &str) -> Response {
    (status, Json(json!({ "type": "error", "error": { "type": kind, "message": message } }))).into_response()
}

fn error_kind(status: StatusCode) -> &'static str {
    match status.as_u16() {
        400 | 413 | 422 => "invalid_request_error",
        401 => "authentication_error",
        403 => "permission_error",
        404 => "not_found_error",
        429 => "rate_limit_error",
        529 | 503 => "overloaded_error",
        _ => "api_error",
    }
}

fn refusal_response(refusal: Refusal) -> Response {
    // An Anthropic provider's own error body is already in this format and goes back as it is.
    if let Some((status, content_type, body)) = refusal.upstream {
        let mut response = Response::new(Body::from(body));
        *response.status_mut() = status;
        if let Some(content_type) = content_type {
            response.headers_mut().insert(header::CONTENT_TYPE, content_type);
        }
        if let Some(seconds) = refusal.retry_after {
            response.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from(seconds));
        }
        return response;
    }
    let kind = match refusal.kind {
        "budget_exceeded" | "rate_limit_error" => "rate_limit_error",
        "model_not_found" => "not_found_error",
        _ => error_kind(refusal.status),
    };
    let mut response = anthropic_error(refusal.status, kind, &refusal.message);
    if let Some(seconds) = refusal.retry_after {
        response.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from(seconds));
    }
    response
}

/// The signature on a `thinking` block Pygg made from a provider's reasoning. Not Anthropic's: a request to
/// Anthropic goes without such blocks (`without_pygg_reasoning`).
pub const REASONING_SIGNATURE: &str = "pygg-reasoning-v1";

fn blocks_of<'a>(request: &'a Value, kind: &'a str) -> impl Iterator<Item = &'a Value> + 'a {
    request["messages"].as_array().into_iter().flatten()
        .flat_map(|m| m["content"].as_array().into_iter().flatten())
        .filter(move |b| b["type"] == kind)
}

/// The request without the thinking blocks Pygg signed, for Anthropic's own API, which rejects a signature it did
/// not make; `None` when there are none.
pub fn without_pygg_reasoning(request: &Value) -> Option<Value> {
    if !blocks_of(request, "thinking").any(|b| b["signature"] == REASONING_SIGNATURE) {
        return None;
    }
    let mut out = request.clone();
    for message in out["messages"].as_array_mut().into_iter().flatten() {
        if let Some(blocks) = message["content"].as_array_mut() {
            blocks.retain(|b| !(b["type"] == "thinking" && b["signature"] == REASONING_SIGNATURE));
        }
    }
    Some(out)
}

/// Everything in a request that `to_openai` does not carry, by name, apart from cache markers (which depend
/// on the target): thinking, block types other than text, image, tool use and tool result, server tools,
/// and top-level fields with no Chat Completions counterpart. Reported in `x-smarttaskrouter-dropped`, so
/// no field is lost without saying so (counter-review CR-08).
pub fn untranslated(request: &Value, takes_reasoning: bool) -> Vec<String> {
    let mut dropped: Vec<String> = Vec::new();
    let mut add = |name: String| {
        if !dropped.contains(&name) {
            dropped.push(name);
        }
    };
    // The setting is not translated; earlier thinking crosses only to a provider that takes it back.
    if !request["thinking"].is_null() || (!takes_reasoning && blocks_of(request, "thinking").next().is_some()) {
        add("thinking".into());
    }
    if blocks_of(request, "redacted_thinking").next().is_some() {
        add("redacted_thinking".into());
    }
    let carried = ["text", "image", "tool_use", "tool_result", "thinking", "redacted_thinking"];
    let blocks = request["system"].as_array().into_iter().flatten()
        .chain(request["messages"].as_array().into_iter().flatten()
            .flat_map(|m| m["content"].as_array().into_iter().flatten()));
    for block in blocks {
        if let Some(kind) = block["type"].as_str().filter(|k| !carried.contains(k)) {
            add(kind.to_string());
        }
        if block.get("citations").is_some_and(|c| !c.is_null()) {
            add("citations".into());
        }
    }
    if request["tools"].as_array().is_some_and(|t| t.iter().any(|t| t.get("input_schema").is_none())) {
        add("server_tools".into());
    }
    for field in ["top_k", "metadata", "service_tier", "container", "mcp_servers"] {
        if request.get(field).is_some_and(|v| !v.is_null()) {
            add(field.into());
        }
    }
    dropped
}

/// Whether any system, message or tool block carries a prompt-cache marker.
pub fn has_cache_control(request: &Value) -> bool {
    let blocks = |v: &Value| v.as_array().is_some_and(|a| a.iter().any(|b| b.get("cache_control").is_some()));
    blocks(&request["system"])
        || request["messages"].as_array().is_some_and(|m| m.iter().any(|m| blocks(&m["content"])))
        || blocks(&request["tools"])
}

// ---------- Request: Anthropic → OpenAI ----------

fn text_of(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|b| match b["type"].as_str() {
                Some("text") => b["text"].as_str().map(String::from),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn image_part(block: &Value) -> Option<Value> {
    let source = &block["source"];
    let url = match source["type"].as_str()? {
        "base64" => format!("data:{};base64,{}", source["media_type"].as_str()?, source["data"].as_str()?),
        "url" => source["url"].as_str()?.to_string(),
        _ => return None,
    };
    Some(json!({ "type": "image_url", "image_url": { "url": url } }))
}

/// Removes JSON Schema keys that some OpenAI-compatible providers reject.
pub(crate) fn clean_schema(schema: &Value) -> Value {
    match schema {
        Value::Object(map) => {
            let mut out = Map::new();
            for (k, v) in map {
                if k == "$schema" {
                    continue;
                }
                out.insert(k.clone(), clean_schema(v));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(clean_schema).collect()),
        other => other.clone(),
    }
}

/// A text part, keeping its cache marker when the target accepts one.
fn text_part(block: &Value, keep_cache: bool) -> Value {
    let mut part = json!({ "type": "text", "text": block["text"] });
    if keep_cache {
        if let Some(marker) = block.get("cache_control") {
            part["cache_control"] = marker.clone();
        }
    }
    part
}

/// Translates a Messages request to Chat Completions. With `keep_cache`, prompt-cache markers stay on the
/// system and user text parts (for providers that accept them there), and those parts are not merged into
/// one string, which would lose the markers.
pub fn to_openai(request: &Value, keep_cache: bool, takes_reasoning: bool) -> Value {
    let mut messages = Vec::new();
    let system_marked = keep_cache
        && request["system"].as_array().is_some_and(|a| a.iter().any(|b| b.get("cache_control").is_some()));
    if system_marked {
        let parts: Vec<Value> = request["system"].as_array().into_iter().flatten()
            .filter(|b| b["type"] == "text").map(|b| text_part(b, true)).collect();
        messages.push(json!({ "role": "system", "content": parts }));
    } else {
        let system = text_of(&request["system"]);
        if !system.is_empty() {
            messages.push(json!({ "role": "system", "content": system }));
        }
    }
    for message in request["messages"].as_array().into_iter().flatten() {
        let role = message["role"].as_str().unwrap_or("user");
        let content = &message["content"];
        if let Value::String(text) = content {
            messages.push(json!({ "role": role, "content": text }));
            continue;
        }
        let blocks = content.as_array().cloned().unwrap_or_default();
        if role == "assistant" {
            let text = text_of(content);
            let calls: Vec<Value> = blocks
                .iter()
                .filter(|b| b["type"] == "tool_use")
                .map(|b| json!({ "id": b["id"], "type": "function", "function": {
                    "name": b["name"], "arguments": serde_json::to_string(&b["input"]).unwrap_or_else(|_| "{}".into()) } }))
                .collect();
            let mut out = json!({ "role": "assistant", "content": if text.is_empty() { Value::Null } else { json!(text) } });
            if !calls.is_empty() {
                out["tool_calls"] = json!(calls);
            }
            let reasoning: String = blocks.iter().filter(|b| b["type"] == "thinking")
                .filter_map(|b| b["thinking"].as_str()).collect();
            if takes_reasoning && !reasoning.is_empty() {
                out["reasoning_content"] = json!(reasoning);
            }
            messages.push(out);
            continue;
        }
        // User turn: tool results become `tool` messages, which must directly follow the assistant's calls.
        let mut parts = Vec::new();
        for block in &blocks {
            match block["type"].as_str() {
                Some("tool_result") => {
                    let mut text = text_of(&block["content"]);
                    if block["is_error"] == true {
                        text = format!("Error: {text}");
                    }
                    messages.push(json!({ "role": "tool", "tool_call_id": block["tool_use_id"], "content": text }));
                }
                Some("text") => parts.push(text_part(block, keep_cache)),
                Some("image") => parts.extend(image_part(block)),
                _ => {}
            }
        }
        if !parts.is_empty() {
            let only_text = parts.iter().all(|p| p["type"] == "text" && p.get("cache_control").is_none());
            let content = if only_text {
                json!(parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("\n"))
            } else {
                json!(parts)
            };
            messages.push(json!({ "role": "user", "content": content }));
        }
    }

    let mut out = json!({ "model": request["model"], "messages": messages });
    for (from, to) in [("max_tokens", "max_tokens"), ("temperature", "temperature"), ("top_p", "top_p"),
        ("stop_sequences", "stop"), ("stream", "stream")] {
        if !request[from].is_null() {
            out[to] = request[from].clone();
        }
    }
    if let Some(tools) = request["tools"].as_array() {
        let tools: Vec<Value> = tools
            .iter()
            // Server tools (web search and the like) have a `type` and no schema; no provider here runs them.
            .filter(|t| t.get("input_schema").is_some())
            .map(|t| json!({ "type": "function", "function": { "name": t["name"],
                "description": t["description"].as_str().unwrap_or(""), "parameters": clean_schema(&t["input_schema"]) } }))
            .collect();
        if !tools.is_empty() {
            out["tools"] = json!(tools);
            if let Some(choice) = request["tool_choice"].as_object() {
                out["tool_choice"] = match choice.get("type").and_then(Value::as_str) {
                    Some("any") => json!("required"),
                    Some("none") => json!("none"),
                    Some("tool") => json!({ "type": "function", "function": { "name": choice.get("name") } }),
                    _ => json!("auto"),
                };
            }
        }
    }
    out
}

// ---------- Response: OpenAI → Anthropic ----------

fn stop_reason(finish: Option<&str>) -> &'static str {
    match finish {
        Some("length") => "max_tokens",
        Some("tool_calls") | Some("function_call") => "tool_use",
        _ => "end_turn",
    }
}

fn message_id() -> String {
    format!("msg_{}", uuid::Uuid::new_v4().simple())
}

fn usage_json(usage: Option<&Usage>) -> Value {
    let usage = usage.cloned().unwrap_or_default();
    json!({ "input_tokens": usage.prompt.saturating_sub(usage.cached), "output_tokens": usage.completion,
        "cache_read_input_tokens": usage.cached, "cache_creation_input_tokens": 0 })
}

fn parse_arguments(arguments: &Value) -> Value {
    match arguments {
        Value::String(s) if s.trim().is_empty() => json!({}),
        Value::String(s) => serde_json::from_str(s).unwrap_or_else(|_| json!({ "_raw": s })),
        Value::Object(_) => arguments.clone(),
        _ => json!({}),
    }
}

/// A provider's reasoning in a message or stream delta: `reasoning_content` (Qwen, DeepSeek, Kimi, GLM) or
/// `reasoning` (OpenRouter).
fn reasoning_of(message: &Value) -> Option<&str> {
    message["reasoning_content"].as_str().or_else(|| message["reasoning"].as_str()).filter(|t| !t.is_empty())
}

pub fn from_openai(response: &Value, model: &str) -> Value {
    let choice = &response["choices"][0];
    let message = &choice["message"];
    let mut content = Vec::new();
    if let Some(reasoning) = reasoning_of(message) {
        content.push(json!({ "type": "thinking", "thinking": reasoning, "signature": REASONING_SIGNATURE }));
    }
    if let Some(text) = message["content"].as_str().filter(|t| !t.is_empty()) {
        content.push(json!({ "type": "text", "text": text }));
    }
    for call in message["tool_calls"].as_array().into_iter().flatten() {
        content.push(json!({ "type": "tool_use",
            "id": call["id"].as_str().map(String::from).unwrap_or_else(|| format!("toolu_{}", uuid::Uuid::new_v4().simple())),
            "name": call["function"]["name"], "input": parse_arguments(&call["function"]["arguments"]) }));
    }
    let usage = Usage::from_json(&response["usage"]);
    json!({ "id": message_id(), "type": "message", "role": "assistant", "model": model, "content": content,
        "stop_reason": stop_reason(choice["finish_reason"].as_str()), "stop_sequence": null,
        "usage": usage_json(usage.as_ref()) })
}

/// Turns OpenAI stream chunks into Anthropic stream events, one chunk at a time.
pub struct StreamTranslator {
    model: String,
    started: bool,
    /// The open content block: its index and kind.
    open: Option<(usize, Block)>,
    next_index: usize,
    finish: Option<String>,
    pub usage: Option<Usage>,
    any_tool_call: bool,
    /// The tool call being received. Its block starts only once its name is known (found by the benchmark,
    /// ROADMAP C3: Qwen sends `id: ""` and no name in a call's first chunk, and an Anthropic client rejects a
    /// `tool_use` block without a name); arguments that arrive before then are held and sent after the start.
    tool: Option<ToolCall>,
}

#[derive(PartialEq)]
enum Block {
    Text,
    Thinking,
    /// A tool call, with the OpenAI tool-call key it belongs to.
    Tool(String),
}

struct ToolCall {
    /// The OpenAI tool-call index, or its id when the provider sends no index.
    key: String,
    id: Option<String>,
    name: Option<String>,
    /// Arguments received but not yet sent.
    held: String,
    /// The block's index once it has started.
    index: Option<usize>,
}

fn event(kind: &str, data: Value) -> String {
    format!("event: {kind}\ndata: {data}\n\n")
}

impl StreamTranslator {
    pub fn new(model: &str) -> StreamTranslator {
        StreamTranslator { model: model.into(), started: false, open: None, next_index: 0, finish: None, usage: None,
            any_tool_call: false, tool: None }
    }

    fn start(&mut self, out: &mut Vec<String>) {
        if !self.started {
            self.started = true;
            out.push(event("message_start", json!({ "type": "message_start", "message": { "id": message_id(),
                "type": "message", "role": "assistant", "model": self.model, "content": [], "stop_reason": null,
                "stop_sequence": null, "usage": { "input_tokens": 0, "output_tokens": 0 } } })));
        }
    }

    fn close(&mut self, out: &mut Vec<String>) {
        // A call that never got a name has no block to close; it is dropped, as no client could run it.
        self.tool = None;
        if let Some((index, kind)) = self.open.take() {
            if kind == Block::Thinking {
                out.push(event("content_block_delta", json!({ "type": "content_block_delta", "index": index,
                    "delta": { "type": "signature_delta", "signature": REASONING_SIGNATURE } })));
            }
            out.push(event("content_block_stop", json!({ "type": "content_block_stop", "index": index })));
        }
    }

    pub fn push(&mut self, chunk: &Value) -> Vec<String> {
        let mut out = Vec::new();
        self.start(&mut out);
        if let Some(usage) = Usage::from_json(&chunk["usage"]) {
            self.usage = Some(usage);
        }
        let Some(choice) = chunk["choices"].get(0) else { return out };
        let delta = &choice["delta"];
        if let Some(reasoning) = reasoning_of(delta) {
            if !matches!(self.open, Some((_, Block::Thinking))) {
                self.close(&mut out);
                let index = self.next_index;
                self.next_index += 1;
                self.open = Some((index, Block::Thinking));
                out.push(event("content_block_start", json!({ "type": "content_block_start", "index": index,
                    "content_block": { "type": "thinking", "thinking": "", "signature": "" } })));
            }
            let index = self.open.as_ref().map(|(i, _)| *i).unwrap_or(0);
            out.push(event("content_block_delta", json!({ "type": "content_block_delta", "index": index,
                "delta": { "type": "thinking_delta", "thinking": reasoning } })));
        }
        if let Some(text) = delta["content"].as_str().filter(|t| !t.is_empty()) {
            if !matches!(self.open, Some((_, Block::Text))) {
                self.close(&mut out);
                let index = self.next_index;
                self.next_index += 1;
                self.open = Some((index, Block::Text));
                out.push(event("content_block_start", json!({ "type": "content_block_start", "index": index,
                    "content_block": { "type": "text", "text": "" } })));
            }
            let index = self.open.as_ref().map(|(i, _)| *i).unwrap_or(0);
            out.push(event("content_block_delta", json!({ "type": "content_block_delta", "index": index,
                "delta": { "type": "text_delta", "text": text } })));
        }
        for (position, call) in delta["tool_calls"].as_array().into_iter().flatten().enumerate() {
            // Some providers omit `index`; then each call that carries an id is a new one.
            let key = call["index"].as_u64().map(|i| i.to_string())
                .or_else(|| call["id"].as_str().map(String::from))
                .unwrap_or_else(|| position.to_string());
            // An empty id or name is the same as none.
            let id = call["id"].as_str().filter(|s| !s.is_empty());
            let name = call["function"]["name"].as_str().filter(|s| !s.is_empty());
            // A fragment continues the current call when it has the same key, unless both carry ids that differ
            // (some providers send every whole call at index 0).
            let continues = match &self.tool {
                Some(current) if current.key == key => match (id, current.id.as_deref()) {
                    (Some(new), Some(old)) => new == old,
                    _ => true,
                },
                _ => false,
            };
            if !continues {
                self.close(&mut out);
                self.tool = Some(ToolCall { key: key.clone(), id: None, name: None, held: String::new(), index: None });
            }
            let fragment = match &call["function"]["arguments"] {
                Value::String(s) => s.clone(),
                Value::Object(_) => call["function"]["arguments"].to_string(),
                _ => String::new(),
            };
            let Some(current) = self.tool.as_mut() else { continue };
            if current.id.is_none() {
                current.id = id.map(String::from);
            }
            if current.name.is_none() {
                current.name = name.map(String::from);
            }
            current.held.push_str(&fragment);
            if current.index.is_none() {
                let Some(name) = current.name.clone() else { continue };
                let index = self.next_index;
                self.next_index += 1;
                current.index = Some(index);
                let id = current.id.clone().unwrap_or_else(|| format!("toolu_{}", uuid::Uuid::new_v4().simple()));
                current.id = Some(id.clone());
                self.open = Some((index, Block::Tool(key)));
                self.any_tool_call = true;
                out.push(event("content_block_start", json!({ "type": "content_block_start", "index": index,
                    "content_block": { "type": "tool_use", "id": id, "name": name, "input": {} } })));
            }
            if let (Some(index), false) = (current.index, current.held.is_empty()) {
                let partial = std::mem::take(&mut current.held);
                out.push(event("content_block_delta", json!({ "type": "content_block_delta", "index": index,
                    "delta": { "type": "input_json_delta", "partial_json": partial } })));
            }
        }
        if let Some(finish) = choice["finish_reason"].as_str() {
            self.finish = Some(finish.to_string());
        }
        out
    }

    pub fn finish(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        self.start(&mut out);
        self.close(&mut out);
        let mut reason = stop_reason(self.finish.as_deref());
        // Gemini reports "stop" even when it called a tool.
        if self.any_tool_call && reason == "end_turn" {
            reason = "tool_use";
        }
        let usage = usage_json(self.usage.as_ref());
        out.push(event("message_delta", json!({ "type": "message_delta",
            "delta": { "stop_reason": reason, "stop_sequence": null }, "usage": usage })));
        out.push(event("message_stop", json!({ "type": "message_stop" })));
        out
    }
}

// ---------- Handlers ----------

pub async fn messages(State(shared): State<Shared>, headers: HeaderMap, body: Bytes) -> Response {
    let client = match authenticate(&shared, &headers) {
        Ok(client) => client,
        Err(refusal) => return refusal_response(refusal),
    };
    let (request, requested) = match parse_body(&body) {
        Ok(parsed) => parsed,
        Err(refusal) => return refusal_response(refusal),
    };
    let stream = request["stream"] == true;
    let session = session_key(&headers);
    let incoming = Incoming { surface: Surface::Anthropic, body: &request, headers: &headers, requested: &requested,
        session: session.as_deref(), stream };
    let forwarded = match forward(&shared, &client, &incoming).await {
        Ok(forwarded) => forwarded,
        Err(refusal) => return refusal_response(refusal),
    };
    match forwarded.mode {
        Mode::AnthropicToOpenAi => translated(shared, forwarded, stream, requested).await,
        // An Anthropic request takes no other translated route; anything else is native.
        _ => native(shared, forwarded, stream).await,
    }
}

/// Same format: the provider's answer goes back unchanged; only its usage is read on the way.
async fn native(shared: Shared, mut forwarded: super::Forwarded, stream: bool) -> Response {
    let status = forwarded.response.status();
    let content_type = forwarded.response.headers().get(header::CONTENT_TYPE).cloned();
    let upstream_headers = forwarded.response.headers().clone();
    let upstream = std::mem::replace(&mut forwarded.response, status_only(status));
    if !stream {
        return match upstream.bytes().await {
            Ok(bytes) => {
                let parsed: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
                let usage = Usage::from_anthropic(&parsed["usage"]);
                let mut out = Response::new(Body::from(bytes));
                *out.status_mut() = status;
                if let Some(content_type) = content_type {
                    out.headers_mut().insert(header::CONTENT_TYPE, content_type);
                }
                provider_headers(&upstream_headers, out.headers_mut());
                route_headers(&mut out, &forwarded);
                record_success(&shared, forwarded, usage.as_ref(), "ok", None);
                out
            }
            Err(e) => {
                let message = crate::redact::redact(&e.to_string(), &[]);
                record_success(&shared, forwarded, None, "stream_error", Some(&message));
                anthropic_error(StatusCode::BAD_GATEWAY, "api_error", &format!("The provider's response broke off: {message}"))
            }
        };
    }
    let (tx, rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(64);
    let mut out = stream_response(rx, status, content_type);
    provider_headers(&upstream_headers, out.headers_mut());
    route_headers(&mut out, &forwarded);
    tokio::spawn(async move {
        let mut scanner = super::sse::UsageScanner::anthropic();
        let (mut outcome, error) = pump(upstream, &tx, |chunk| scanner.feed(chunk)).await;
        if outcome == "ok" && scanner.error.is_some() {
            outcome = "stream_error";
        }
        let error = error.or(scanner.error.clone());
        record_success(&shared, forwarded, scanner.usage.as_ref(), outcome, error.as_deref());
    });
    out
}

/// Translated to Chat Completions: the answer is translated back, streamed or not.
async fn translated(shared: Shared, mut forwarded: super::Forwarded, stream: bool, requested: String) -> Response {
    let status = forwarded.response.status();
    let upstream = std::mem::replace(&mut forwarded.response, status_only(status));

    if !stream {
        return match upstream.bytes().await {
            Ok(bytes) => {
                let parsed: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
                shared.capture_signatures(&parsed);
                let usage = Usage::from_json(&parsed["usage"]);
                let mut out = Json(from_openai(&parsed, &requested)).into_response();
                route_headers(&mut out, &forwarded);
                record_success(&shared, forwarded, usage.as_ref(), "ok", None);
                out
            }
            Err(e) => {
                let message = crate::redact::redact(&e.to_string(), &[]);
                record_success(&shared, forwarded, None, "stream_error", Some(&message));
                anthropic_error(StatusCode::BAD_GATEWAY, "api_error", &format!("The provider's response broke off: {message}"))
            }
        };
    }

    let (tx, rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(64);
    let mut out = stream_response(rx, StatusCode::OK, Some(HeaderValue::from_static("text/event-stream")));
    route_headers(&mut out, &forwarded);
    tokio::spawn(async move {
        use std::io::Write;
        let mut translator = StreamTranslator::new(&requested);
        let mut capture = capture_file(forwarded.session.as_deref(), &forwarded.model_id);
        let mut bytes = upstream.bytes_stream();
        let mut line = Vec::<u8>::new();
        let mut outcome = "ok";
        let mut error = None;
        'read: while let Some(chunk) = bytes.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(e) => {
                    outcome = "stream_error";
                    error = Some(crate::redact::redact(&e.to_string(), &[]));
                    break;
                }
            };
            for &b in chunk.iter() {
                if b != b'\n' {
                    if line.len() < 4 * 1024 * 1024 {
                        line.push(b);
                    }
                    continue;
                }
                let text = String::from_utf8_lossy(&line).trim_end_matches('\r').to_string();
                line.clear();
                if let Some(file) = capture.as_mut() {
                    let _ = writeln!(file, "IN  {text}");
                }
                let Some(data) = text.strip_prefix("data:").map(str::trim) else { continue };
                if data == "[DONE]" {
                    break 'read;
                }
                let Ok(value) = serde_json::from_str::<Value>(data) else { continue };
                shared.capture_signatures(&value);
                for event in translator.push(&value) {
                    if let Some(file) = capture.as_mut() {
                        let _ = write!(file, "OUT {event}");
                    }
                    if tx.send(Ok(Bytes::from(event))).await.is_err() {
                        outcome = "client_disconnected";
                        break 'read;
                    }
                }
            }
        }
        // A stream that ends without [DONE] is still finished cleanly if the provider closed it normally.
        if outcome != "client_disconnected" {
            if outcome == "stream_error" {
                let event = format!("event: error\ndata: {}\n\n",
                    json!({ "type": "error", "error": { "type": "api_error", "message": "The provider's stream broke off" } }));
                let _ = tx.send(Ok(Bytes::from(event))).await;
            } else {
                for event in translator.finish() {
                    if let Some(file) = capture.as_mut() {
                        let _ = write!(file, "OUT {event}");
                    }
                    let _ = tx.send(Ok(Bytes::from(event))).await;
                }
            }
        }
        if let Some(file) = capture.as_mut() {
            let _ = writeln!(file, "END {outcome}");
        }
        drop(capture);
        let usage = translator.usage.clone();
        record_success(&shared, forwarded, usage.as_ref(), outcome, error.as_deref());
    });
    out
}

/// A debugging aid (ROADMAP N4): with `SMARTTASKROUTER_CAPTURE_DIR` set, each translated stream is written there,
/// one file per request, as received from the provider (`IN`) and as sent to the client (`OUT`). The file holds the
/// model's whole answer, so the variable is for diagnosing a client on one's own machine; nothing is written unless
/// it is set.
fn capture_file(session: Option<&str>, model: &str) -> Option<std::io::BufWriter<std::fs::File>> {
    let dir = std::env::var_os("SMARTTASKROUTER_CAPTURE_DIR")?;
    let safe = |s: &str| s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect::<String>();
    let name = format!("{}-{}-{}-{}.txt", crate::db::now_ms(), safe(session.unwrap_or("none")), safe(model),
        &uuid::Uuid::new_v4().simple().to_string()[..6]);
    std::fs::create_dir_all(&dir).ok()?;
    std::fs::File::create(std::path::Path::new(&dir).join(name)).ok().map(std::io::BufWriter::new)
}

/// Token counting for Claude Code's context management. An Anthropic connection answers exactly; for
/// anything else the count is an estimate (about four characters a token), labeled in
/// `x-smarttaskrouter-estimate`.
pub async fn count_tokens(State(shared): State<Shared>, headers: HeaderMap, body: Bytes) -> Response {
    let client = match authenticate(&shared, &headers) {
        Ok(client) => client,
        Err(refusal) => return refusal_response(refusal),
    };
    let Ok(mut request) = serde_json::from_slice::<Value>(&body) else {
        return anthropic_error(StatusCode::BAD_REQUEST, "invalid_request_error", "The body is not JSON");
    };
    let requested = request["model"].as_str().unwrap_or("").to_string();
    // Counting sends the prompt to the provider too, so the client's connection list applies (review FR-01).
    let target = shared.with_db(|db| super::store::resolve(db, &requested)).ok().into_iter().flatten()
        .find(|(provider, _)| provider.wire == "anthropic" && client.allows(provider).is_ok());
    if let Some((provider, model_id)) = target {
        let key = provider.credential_ref.as_deref().and_then(|r| shared.secrets.read(r).ok().flatten());
        request["model"] = json!(model_id);
        let mut call = shared.http.post(format!("{}/messages/count_tokens", provider.endpoint)).json(&request)
            .header("anthropic-version", headers.get("anthropic-version").cloned()
                .unwrap_or_else(|| HeaderValue::from_static(super::ANTHROPIC_VERSION)));
        if let Some(key) = &key {
            call = call.header("x-api-key", key);
        }
        if let Ok(response) = call.send().await {
            if response.status().is_success() {
                if let Ok(bytes) = response.bytes().await {
                    let mut out = Response::new(Body::from(bytes));
                    out.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
                    return out;
                }
            }
        }
    }
    let chars = request["system"].to_string().len() + request["messages"].to_string().len() + request["tools"].to_string().len();
    let mut out = Json(json!({ "input_tokens": chars.div_ceil(4) })).into_response();
    out.headers_mut().insert("x-smarttaskrouter-estimate", HeaderValue::from_static("characters/4"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_a_tool_conversation_to_openai() {
        let request = json!({
            "model": "coding", "max_tokens": 1024, "stream": true,
            "system": [{ "type": "text", "text": "Be brief." }],
            "tools": [{ "name": "read", "description": "Read a file", "input_schema": {
                "$schema": "http://json-schema.org/draft-07/schema#", "type": "object",
                "properties": { "path": { "type": "string" } } } },
                { "type": "web_search_20250305", "name": "web_search" }],
            "tool_choice": { "type": "any" },
            "messages": [
                { "role": "user", "content": "Open a.txt" },
                { "role": "assistant", "content": [{ "type": "thinking", "thinking": "…", "signature": "x" },
                    { "type": "text", "text": "Reading." },
                    { "type": "tool_use", "id": "toolu_1", "name": "read", "input": { "path": "a.txt" } }] },
                { "role": "user", "content": [{ "type": "tool_result", "tool_use_id": "toolu_1", "content": "hello" },
                    { "type": "text", "text": "Now summarize." }] }
            ]
        });
        let out = to_openai(&request, false, false);
        let messages = out["messages"].as_array().unwrap();
        assert_eq!(messages[0], json!({ "role": "system", "content": "Be brief." }));
        assert_eq!(messages[2]["content"], "Reading.");
        assert_eq!(messages[2]["tool_calls"][0]["function"]["arguments"], "{\"path\":\"a.txt\"}");
        assert_eq!(messages[3], json!({ "role": "tool", "tool_call_id": "toolu_1", "content": "hello" }));
        assert_eq!(messages[4], json!({ "role": "user", "content": "Now summarize." }));
        assert_eq!(out["tools"].as_array().unwrap().len(), 1, "server tools are dropped");
        assert!(out["tools"][0]["function"]["parameters"].get("$schema").is_none());
        assert_eq!(out["tool_choice"], "required");
        assert_eq!((out["max_tokens"].as_u64(), out["stream"].as_bool()), (Some(1024), Some(true)));
    }

    #[test]
    fn names_everything_the_translation_drops() {
        let request = json!({ "model": "m", "top_k": 5, "metadata": { "user_id": "u" }, "thinking": { "type": "enabled" },
            "tools": [{ "type": "web_search_20250305", "name": "web_search" }],
            "messages": [{ "role": "user", "content": [
                { "type": "document", "source": { "type": "text", "data": "x" }, "citations": { "enabled": true } },
                { "type": "text", "text": "hi" }] }] });
        assert_eq!(untranslated(&request, false), ["thinking", "document", "citations", "server_tools", "top_k", "metadata"]);
        assert!(untranslated(&json!({ "messages": [{ "role": "user", "content": "plain" }] }), false).is_empty());
    }

    /// ROADMAP B5. DeepSeek's thinking mode with tools answers HTTP 400 unless the model's earlier reasoning comes
    /// back as `reasoning_content`; an Anthropic client sends it back as a thinking block.
    #[test]
    fn reasoning_crosses_both_ways_and_only_to_providers_that_take_it() {
        let response = json!({ "choices": [{ "finish_reason": "tool_calls", "message": { "reasoning_content": "Need the file.",
            "content": "", "tool_calls": [{ "id": "c1", "type": "function", "function": { "name": "read", "arguments": "{}" } }] } }] });
        let out = from_openai(&response, "m");
        assert_eq!(out["content"][0], json!({ "type": "thinking", "thinking": "Need the file.", "signature": REASONING_SIGNATURE }));
        assert_eq!(out["content"][1]["type"], "tool_use");
        let openrouter = from_openai(&json!({ "choices": [{ "message": { "reasoning": "Hmm.", "content": "Hi" } }] }), "m");
        assert_eq!((openrouter["content"][0]["thinking"].as_str(), openrouter["content"][1]["text"].as_str()), (Some("Hmm."), Some("Hi")));

        let request = json!({ "model": "m", "messages": [
            { "role": "user", "content": "Open a.txt" },
            { "role": "assistant", "content": [out["content"][0].clone(), out["content"][1].clone()] },
            { "role": "user", "content": [{ "type": "tool_result", "tool_use_id": "c1", "content": "hello" }] }] });
        let to_deepseek = to_openai(&request, false, true);
        assert_eq!(to_deepseek["messages"][1]["reasoning_content"], "Need the file.");
        assert!(untranslated(&request, true).is_empty(), "nothing lost on the way to DeepSeek");
        let elsewhere = to_openai(&request, false, false);
        assert!(elsewhere["messages"][1].get("reasoning_content").is_none());
        assert_eq!(untranslated(&request, false), ["thinking"]);

        // To Anthropic itself the block Pygg signed is left out; Anthropic's own blocks stay.
        let mut mixed = request.clone();
        mixed["messages"][1]["content"].as_array_mut().unwrap().insert(0,
            json!({ "type": "thinking", "thinking": "real", "signature": "EqQBCkYIBxgCKkA" }));
        let cleaned = without_pygg_reasoning(&mixed).unwrap();
        let kinds: Vec<&str> = cleaned["messages"][1]["content"].as_array().unwrap().iter().map(|b| b["signature"].as_str().unwrap_or("-")).collect();
        assert_eq!(kinds, ["EqQBCkYIBxgCKkA", "-"]);
        assert!(without_pygg_reasoning(&cleaned).is_none());
    }

    #[test]
    fn streamed_reasoning_is_a_signed_thinking_block_before_the_answer() {
        let mut t = StreamTranslator::new("m");
        let mut events = Vec::new();
        events.extend(t.push(&json!({ "choices": [{ "delta": { "reasoning_content": "Think" } }] })));
        events.extend(t.push(&json!({ "choices": [{ "delta": { "reasoning_content": "ing." } }] })));
        events.extend(t.push(&json!({ "choices": [{ "delta": { "content": "ok" }, "finish_reason": "stop" }] })));
        events.extend(t.finish());
        assert_eq!(kinds(&events), ["message_start", "content_block_start", "content_block_delta", "content_block_delta",
            "content_block_delta", "content_block_stop", "content_block_start", "content_block_delta", "content_block_stop",
            "message_delta", "message_stop"]);
        assert!(events[1].contains("\"type\":\"thinking\""));
        assert!(events[2].contains("thinking_delta") && events[3].contains("ing."));
        assert!(events[4].contains("signature_delta") && events[4].contains(REASONING_SIGNATURE));
        assert!(events[6].contains("\"index\":1") && events[6].contains("\"type\":\"text\""));
    }

    #[test]
    fn translates_a_response_with_a_tool_call() {
        let response = json!({ "choices": [{ "finish_reason": "tool_calls", "message": { "content": "Let me look.",
            "tool_calls": [{ "id": "call_9", "type": "function", "function": { "name": "read", "arguments": "{\"path\":\"b\"}" } }] } }],
            "usage": { "prompt_tokens": 30, "completion_tokens": 8, "prompt_tokens_details": { "cached_tokens": 10 } } });
        let out = from_openai(&response, "coding");
        assert_eq!(out["stop_reason"], "tool_use");
        assert_eq!(out["content"][0], json!({ "type": "text", "text": "Let me look." }));
        assert_eq!(out["content"][1]["input"], json!({ "path": "b" }));
        assert_eq!(out["usage"]["input_tokens"], 20);
        assert_eq!(out["usage"]["cache_read_input_tokens"], 10);
    }

    fn kinds(events: &[String]) -> Vec<String> {
        events.iter().map(|e| e.lines().next().unwrap().trim_start_matches("event: ").to_string()).collect()
    }

    #[test]
    fn streams_text_then_a_tool_call() {
        let mut t = StreamTranslator::new("coding");
        let mut events = Vec::new();
        events.extend(t.push(&json!({ "choices": [{ "delta": { "content": "Hi" } }] })));
        events.extend(t.push(&json!({ "choices": [{ "delta": { "content": " there" } }] })));
        events.extend(t.push(&json!({ "choices": [{ "delta": { "tool_calls": [{ "index": 0, "id": "c1",
            "function": { "name": "read", "arguments": "{\"pa" } }] } }] })));
        events.extend(t.push(&json!({ "choices": [{ "delta": { "tool_calls": [{ "index": 0,
            "function": { "arguments": "th\":1}" } }] }, "finish_reason": "tool_calls" }] })));
        events.extend(t.push(&json!({ "choices": [], "usage": { "prompt_tokens": 5, "completion_tokens": 2 } })));
        events.extend(t.finish());
        assert_eq!(kinds(&events), ["message_start", "content_block_start", "content_block_delta", "content_block_delta",
            "content_block_stop", "content_block_start", "content_block_delta", "content_block_delta",
            "content_block_stop", "message_delta", "message_stop"]);
        assert!(events[5].contains("\"index\":1") && events[5].contains("\"name\":\"read\""));
        assert!(events[9].contains("\"stop_reason\":\"tool_use\"") && events[9].contains("\"output_tokens\":2"));
    }

    #[test]
    fn gemini_style_whole_tool_calls_without_index() {
        let mut t = StreamTranslator::new("m");
        let mut events = t.push(&json!({ "choices": [{ "delta": { "tool_calls": [
            { "id": "a", "function": { "name": "x", "arguments": "{}" } },
            { "id": "b", "function": { "name": "y", "arguments": "{\"k\":1}" } }] }, "finish_reason": "stop" }] }));
        events.extend(t.finish());
        let starts = events.iter().filter(|e| e.starts_with("event: content_block_start")).count();
        assert_eq!(starts, 2);
        assert!(events.iter().any(|e| e.contains("\"stop_reason\":\"tool_use\"")));
    }

    /// Found by the benchmark (ROADMAP C3) with Qwen's token plan: a call's first chunk has `id: ""`, no name and
    /// part of the arguments; the id and name come in the next one. The block must start once, with the name.
    #[test]
    fn a_tool_call_whose_name_comes_later_starts_once_with_it() {
        let mut t = StreamTranslator::new("m");
        let mut events = t.push(&json!({ "choices": [{ "delta": { "tool_calls": [{ "index": 0, "id": "", "type": "function",
            "function": { "arguments": "{\"pa" } }] } }] }));
        assert!(!events.iter().any(|e| e.starts_with("event: content_block_start")), "no block before the name");
        events.extend(t.push(&json!({ "choices": [{ "delta": { "tool_calls": [{ "index": 0, "id": "call_7",
            "function": { "name": "read", "arguments": "th\":" } }] } }] })));
        events.extend(t.push(&json!({ "choices": [{ "delta": { "tool_calls": [{ "index": 0,
            "function": { "arguments": "1}" } }] }, "finish_reason": "tool_calls" }] })));
        events.extend(t.finish());
        let starts: Vec<&String> = events.iter().filter(|e| e.starts_with("event: content_block_start")).collect();
        assert_eq!(starts.len(), 1);
        assert!(starts[0].contains("\"id\":\"call_7\"") && starts[0].contains("\"name\":\"read\""), "{}", starts[0]);
        let partial: String = events.iter().filter(|e| e.contains("input_json_delta"))
            .map(|e| serde_json::from_str::<Value>(e.lines().nth(1).unwrap().trim_start_matches("data: ")).unwrap()
                ["delta"]["partial_json"].as_str().unwrap().to_string()).collect();
        assert_eq!(partial, "{\"path\":1}");
        // A call that never gets a name opens no block.
        let mut nameless = StreamTranslator::new("m");
        let mut events = nameless.push(&json!({ "choices": [{ "delta": { "tool_calls": [{ "index": 0,
            "function": { "arguments": "{}" } }] } }] }));
        events.extend(nameless.finish());
        assert!(!events.iter().any(|e| e.contains("tool_use\"") && e.starts_with("event: content_block_start")));
    }
}
