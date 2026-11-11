//! The Anthropic Messages surface (`POST /v1/messages`), so Claude Code and other Anthropic clients can use
//! any connection, through the same routing, fallback, budgets and ledger as `/v1/chat/completions`.
//!
//! - **To an Anthropic connection** it is a native-format semantic passthrough (ROADMAP P0-1, counter-review
//!   CR-08): the JSON body is parsed and written again in the same key order with only `model` replaced, so
//!   cache markers, thinking, citations and every other field reach the provider. `anthropic-version` and
//!   `anthropic-beta` are forwarded; other client headers are not. The provider's answer body comes back
//!   unchanged, with its rate-limit and request-id headers.
//! - **To an OpenAI-compatible connection** it is translated to Chat Completions and back, streamed or not.
//!   Text, images, tool definitions, tool calls and tool results cross. Thinking does not. Cache markers cross
//!   only to providers that document accepting them on content parts (OpenRouter); otherwise they are dropped,
//!   and the response says so in `x-smarttaskrouter-dropped`.

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

/// Whether the request asks for extended thinking or carries earlier thinking blocks.
pub fn has_thinking(request: &Value) -> bool {
    !request["thinking"].is_null()
        || request["messages"].as_array().is_some_and(|m| m.iter().any(|m| m["content"].as_array()
            .is_some_and(|blocks| blocks.iter().any(|b| matches!(b["type"].as_str(), Some("thinking" | "redacted_thinking"))))))
}

/// Everything in a request that `to_openai` does not carry, by name, apart from cache markers (which depend
/// on the target): thinking, block types other than text, image, tool use and tool result, server tools,
/// and top-level fields with no Chat Completions counterpart. Reported in `x-smarttaskrouter-dropped`, so
/// no field is lost without saying so (counter-review CR-08).
pub fn untranslated(request: &Value) -> Vec<String> {
    let mut dropped: Vec<String> = Vec::new();
    let mut add = |name: String| {
        if !dropped.contains(&name) {
            dropped.push(name);
        }
    };
    if has_thinking(request) {
        add("thinking".into());
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
fn clean_schema(schema: &Value) -> Value {
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
pub fn to_openai(request: &Value, keep_cache: bool) -> Value {
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

pub fn from_openai(response: &Value, model: &str) -> Value {
    let choice = &response["choices"][0];
    let message = &choice["message"];
    let mut content = Vec::new();
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
    /// The open content block: its index, and for a tool call the OpenAI tool-call index it belongs to.
    open: Option<(usize, Option<String>)>,
    next_index: usize,
    finish: Option<String>,
    pub usage: Option<Usage>,
    any_tool_call: bool,
    open_tool_id: Option<String>,
}

fn event(kind: &str, data: Value) -> String {
    format!("event: {kind}\ndata: {data}\n\n")
}

impl StreamTranslator {
    pub fn new(model: &str) -> StreamTranslator {
        StreamTranslator { model: model.into(), started: false, open: None, next_index: 0, finish: None, usage: None,
            any_tool_call: false, open_tool_id: None }
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
        self.open_tool_id = None;
        if let Some((index, _)) = self.open.take() {
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
        if let Some(text) = delta["content"].as_str().filter(|t| !t.is_empty()) {
            if !matches!(self.open, Some((_, None))) {
                self.close(&mut out);
                let index = self.next_index;
                self.next_index += 1;
                self.open = Some((index, None));
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
            // A fragment continues the open call when it has the same key and no id, or repeats the same id.
            let same_key = matches!(&self.open, Some((_, Some(open))) if *open == key);
            let same_id = match call["id"].as_str() {
                None => true,
                Some(id) => self.open_tool_id.as_deref() == Some(id),
            };
            if !(same_key && same_id) {
                self.close(&mut out);
                let index = self.next_index;
                self.next_index += 1;
                self.open = Some((index, Some(key)));
                self.any_tool_call = true;
                let id = call["id"].as_str().map(String::from)
                    .unwrap_or_else(|| format!("toolu_{}", uuid::Uuid::new_v4().simple()));
                self.open_tool_id = Some(id.clone());
                out.push(event("content_block_start", json!({ "type": "content_block_start", "index": index,
                    "content_block": { "type": "tool_use", "id": id, "name": call["function"]["name"], "input": {} } })));
            }
            let fragment = match &call["function"]["arguments"] {
                Value::String(s) => s.clone(),
                Value::Object(_) => call["function"]["arguments"].to_string(),
                _ => String::new(),
            };
            if !fragment.is_empty() {
                let index = self.open.as_ref().map(|(i, _)| *i).unwrap_or(0);
                out.push(event("content_block_delta", json!({ "type": "content_block_delta", "index": index,
                    "delta": { "type": "input_json_delta", "partial_json": fragment } })));
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
        Mode::Native => native(shared, forwarded, stream).await,
        Mode::AnthropicToOpenAi => translated(shared, forwarded, stream, requested).await,
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
        let mut translator = StreamTranslator::new(&requested);
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
                let Some(data) = text.strip_prefix("data:").map(str::trim) else { continue };
                if data == "[DONE]" {
                    break 'read;
                }
                let Ok(value) = serde_json::from_str::<Value>(data) else { continue };
                shared.capture_signatures(&value);
                for event in translator.push(&value) {
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
                    let _ = tx.send(Ok(Bytes::from(event))).await;
                }
            }
        }
        let usage = translator.usage.clone();
        record_success(&shared, forwarded, usage.as_ref(), outcome, error.as_deref());
    });
    out
}

/// Token counting for Claude Code's context management. An Anthropic connection answers exactly; for
/// anything else the count is an estimate (about four characters a token), labeled in
/// `x-smarttaskrouter-estimate`.
pub async fn count_tokens(State(shared): State<Shared>, headers: HeaderMap, body: Bytes) -> Response {
    if let Err(refusal) = authenticate(&shared, &headers) {
        return refusal_response(refusal);
    }
    let Ok(mut request) = serde_json::from_slice::<Value>(&body) else {
        return anthropic_error(StatusCode::BAD_REQUEST, "invalid_request_error", "The body is not JSON");
    };
    let requested = request["model"].as_str().unwrap_or("").to_string();
    let target = shared.with_db(|db| super::store::resolve(db, &requested)).ok().into_iter().flatten()
        .find(|(provider, _)| provider.wire == "anthropic");
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
        let out = to_openai(&request, false);
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
        assert_eq!(untranslated(&request), ["thinking", "document", "citations", "server_tools", "top_k", "metadata"]);
        assert!(untranslated(&json!({ "messages": [{ "role": "user", "content": "plain" }] })).is_empty());
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
}
