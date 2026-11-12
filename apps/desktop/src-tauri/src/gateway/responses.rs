//! The OpenAI Responses surface (`POST /v1/responses`, ADR-033), so Codex CLI, which speaks nothing else since
//! it dropped Chat Completions, and other Responses clients can use any connection, through the same routing,
//! `auto`, combos, client limits, holds, session pins and ledger as the other two surfaces.
//!
//! - **To a connection whose provider serves `/responses` itself** (`Preset::serves_responses`: OpenAI) it is a
//!   native passthrough: only `model` is replaced, and the provider's answer comes back unchanged.
//! - **To any other OpenAI-format connection** it is translated to Chat Completions and back, streamed or not.
//! - **To an Anthropic-format connection** it is translated to Chat Completions and then, by the tested
//!   `openai_to_anthropic` translator, to Messages; the answer comes back the same two steps.
//!
//! What crosses: `instructions` and system or developer messages, user and assistant messages with text and
//! images, `function_call` / `function_call_output`, Codex's free-form `custom` tools (a function with one string
//! parameter, `input`, whose description carries the tool's grammar; the call comes back as a
//! `custom_tool_call`), `namespace` tools (flattened as `<namespace>__<name>` and given back their namespace),
//! tools in an `additional_tools` input item, `tool_choice`, `max_output_tokens`, `temperature`, `top_p` and
//! `text.format`. A provider's reasoning (`reasoning_content` or `reasoning`) comes back as a `reasoning` item:
//! as summary text when the client asked for a summary (`reasoning.summary`), otherwise as reasoning text. Reasoning
//! sent back goes to providers that take it (`Preset::takes_reasoning`) as `reasoning_content`. Everything else,
//! including another provider's encrypted reasoning and hosted tools such as `web_search`, is named in
//! `x-smarttaskrouter-dropped`.
//!
//! Pygg stores no responses: `previous_response_id`, `conversation` and `background` are refused, and a client
//! sends the whole conversation each time (Codex does, with `store = false`).

use super::openai_to_anthropic;
use super::pricing::Usage;
use super::{authenticate, forward, openai_error, parse_body, provider_headers, pump, record_success, route_headers,
    session_key, status_only, stream_response, Forwarded, Incoming, Mode, Shared, Surface};
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio::sync::mpsc;

/// The one parameter of the function a free-form custom tool becomes.
pub const CUSTOM_INPUT: &str = "input";
/// Joins a namespace and a tool name into one function name, which Chat Completions providers limit to
/// `[a-zA-Z0-9_-]`.
const NAMESPACE_SEPARATOR: &str = "__";

// ---------- Tools ----------

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ToolKind {
    Function,
    Custom,
}

/// What a function name sent upstream stands for in the client's request.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolName {
    pub kind: ToolKind,
    pub name: String,
    pub namespace: Option<String>,
}

/// The client's tools by the function name each one is given upstream.
#[derive(Clone, Debug, Default)]
pub struct Tools {
    by_upstream_name: HashMap<String, ToolName>,
}

fn upstream_name(namespace: Option<&str>, name: &str) -> String {
    match namespace.filter(|n| !n.is_empty()) {
        Some(namespace) => format!("{namespace}{NAMESPACE_SEPARATOR}{name}"),
        None => name.to_string(),
    }
}

/// Every tool definition in a request: `tools`, and the tools of `additional_tools` input items (Codex sends its
/// tools that way to OpenAI's own models).
fn tool_definitions(request: &Value) -> Vec<&Value> {
    let mut out: Vec<&Value> = request["tools"].as_array().into_iter().flatten().collect();
    for item in request["input"].as_array().into_iter().flatten().filter(|i| i["type"] == "additional_tools") {
        out.extend(item["tools"].as_array().into_iter().flatten());
    }
    out
}

impl Tools {
    pub fn of(request: &Value) -> Tools {
        let mut tools = Tools::default();
        for tool in tool_definitions(request) {
            tools.add(tool, None);
        }
        tools
    }

    fn add(&mut self, tool: &Value, namespace: Option<&str>) {
        let kind = match tool["type"].as_str() {
            Some("function") => ToolKind::Function,
            Some("custom") => ToolKind::Custom,
            Some("namespace") if namespace.is_none() => {
                let inner = tool["name"].as_str().unwrap_or("");
                for nested in tool["tools"].as_array().into_iter().flatten() {
                    self.add(nested, Some(inner));
                }
                return;
            }
            _ => return,
        };
        let Some(name) = tool["name"].as_str() else { return };
        self.by_upstream_name.insert(upstream_name(namespace, name),
            ToolName { kind, name: name.to_string(), namespace: namespace.filter(|n| !n.is_empty()).map(String::from) });
    }

    /// The client's tool for a function name the provider called; a name the request did not define is taken
    /// as a plain function of that name.
    pub fn lookup(&self, upstream: &str) -> ToolName {
        self.by_upstream_name.get(upstream).cloned()
            .unwrap_or(ToolName { kind: ToolKind::Function, name: upstream.to_string(), namespace: None })
    }
}

// ---------- Request: Responses → Chat Completions ----------

/// The fields `to_chat` carries or deliberately leaves out without loss; any other top-level field is named.
const HANDLED: &[&str] = &["model", "input", "instructions", "tools", "tool_choice", "stream", "store", "include",
    "reasoning", "text", "parallel_tool_calls", "max_output_tokens", "temperature", "top_p", "stream_options"];

struct Dropped(Vec<String>);

impl Dropped {
    fn add(&mut self, name: impl Into<String>) {
        let name = name.into();
        if !self.0.contains(&name) {
            self.0.push(name);
        }
    }
}

fn part_text(part: &Value) -> Option<&str> {
    match part["type"].as_str() {
        Some("input_text" | "output_text" | "text" | "summary_text" | "reasoning_text") => part["text"].as_str(),
        Some("refusal") => part["refusal"].as_str(),
        _ => None,
    }
}

/// The text of a message's content, naming any part that is not text.
fn content_text(content: &Value, dropped: &mut Dropped) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => {
            let mut texts = Vec::new();
            for part in parts {
                match part_text(part) {
                    Some(text) => texts.push(text.to_string()),
                    None => dropped.add(part["type"].as_str().unwrap_or("content part").to_string()),
                }
            }
            texts.join("\n")
        }
        _ => String::new(),
    }
}

/// A user message's content: a string when it is only text, otherwise Chat Completions parts with images.
fn user_content(content: &Value, dropped: &mut Dropped) -> Option<Value> {
    let Value::Array(parts) = content else {
        return content.as_str().filter(|s| !s.is_empty()).map(|s| json!(s));
    };
    let mut out = Vec::new();
    for part in parts {
        if let Some(text) = part_text(part) {
            if !text.is_empty() {
                out.push(json!({ "type": "text", "text": text }));
            }
            continue;
        }
        match (part["type"].as_str(), part["image_url"].as_str()) {
            (Some("input_image"), Some(url)) => {
                let mut image = json!({ "url": url });
                if let Some(detail) = part["detail"].as_str() {
                    image["detail"] = json!(detail);
                }
                out.push(json!({ "type": "image_url", "image_url": image }));
            }
            // An uploaded file id means nothing to another provider.
            (kind, _) => dropped.add(kind.unwrap_or("content part").to_string()),
        }
    }
    if out.is_empty() {
        return None;
    }
    if out.iter().all(|p| p["type"] == "text") {
        return Some(json!(out.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("\n")));
    }
    Some(json!(out))
}

/// A tool's output: a string, or content items whose text is kept.
fn output_text(output: &Value, dropped: &mut Dropped) -> String {
    match output {
        Value::String(s) => s.clone(),
        Value::Array(_) => content_text(output, dropped),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// A reasoning item's readable text: its reasoning text, else its summary.
fn reasoning_text(item: &Value) -> String {
    let join = |key: &str| item[key].as_array().into_iter().flatten().filter_map(part_text).collect::<Vec<_>>().join("\n");
    let content = join("content");
    if content.is_empty() { join("summary") } else { content }
}

/// The assistant message new assistant content goes into: the last message when it is an assistant's, else a new
/// one. Reasoning that came before it goes on it, for a provider that takes it back.
fn assistant<'a>(messages: &'a mut Vec<Value>, reasoning: &mut Option<String>) -> &'a mut Value {
    if messages.last().is_none_or(|m| m["role"] != "assistant") {
        messages.push(json!({ "role": "assistant", "content": null }));
    }
    let message = messages.last_mut().expect("just pushed");
    if let Some(text) = reasoning.take() {
        let earlier = message["reasoning_content"].as_str().unwrap_or("").to_string();
        message["reasoning_content"] = json!(if earlier.is_empty() { text } else { format!("{earlier}\n{text}") });
    }
    message
}

fn custom_description(tool: &Value) -> String {
    let mut text = tool["description"].as_str().unwrap_or("").trim_end().to_string();
    if !text.is_empty() {
        text.push_str("\n\n");
    }
    text.push_str("Give the whole input as the string `input`. It is free-form text, not JSON.");
    let format = &tool["format"];
    if format["type"] == "grammar" {
        if let Some(definition) = format["definition"].as_str() {
            text.push_str(&format!(" It must match this {} grammar:\n{}", format["syntax"].as_str().unwrap_or(""), definition.trim()));
        }
    }
    text
}

/// The Chat Completions functions for a request's tools, naming the hosted tools no other provider runs.
fn chat_tools(request: &Value, dropped: &mut Dropped) -> Vec<Value> {
    fn one(tool: &Value, namespace: Option<&str>, out: &mut Vec<Value>, dropped: &mut Dropped) {
        let name = tool["name"].as_str().unwrap_or("");
        match tool["type"].as_str() {
            Some("function") => {
                let parameters = if tool["parameters"].is_object() {
                    super::anthropic::clean_schema(&tool["parameters"])
                } else {
                    json!({ "type": "object", "properties": {} })
                };
                out.push(json!({ "type": "function", "function": { "name": upstream_name(namespace, name),
                    "description": tool["description"].as_str().unwrap_or(""), "parameters": parameters } }));
            }
            Some("custom") => out.push(json!({ "type": "function", "function": { "name": upstream_name(namespace, name),
                "description": custom_description(tool), "parameters": { "type": "object",
                    "properties": { CUSTOM_INPUT: { "type": "string", "description": "The tool's free-form input." } },
                    "required": [CUSTOM_INPUT] } } })),
            Some("namespace") if namespace.is_none() => {
                for nested in tool["tools"].as_array().into_iter().flatten() {
                    one(nested, Some(name), out, dropped);
                }
            }
            other => dropped.add(format!("{} tool", other.unwrap_or("unknown"))),
        }
    }
    let mut out = Vec::new();
    for tool in tool_definitions(request) {
        one(tool, None, &mut out, dropped);
    }
    out
}

fn chat_tool_choice(choice: &Value, dropped: &mut Dropped) -> Option<Value> {
    match choice {
        Value::String(s) if matches!(s.as_str(), "auto" | "none" | "required") => Some(json!(s)),
        Value::Object(o) => match o.get("type").and_then(Value::as_str) {
            Some("function" | "custom") => Some(json!({ "type": "function", "function": {
                "name": upstream_name(o.get("namespace").and_then(Value::as_str), o.get("name").and_then(Value::as_str).unwrap_or("")) } })),
            Some("allowed_tools") => {
                dropped.add("tool_choice.allowed_tools");
                o.get("mode").and_then(Value::as_str).filter(|m| *m == "required").map(|m| json!(m))
            }
            other => {
                dropped.add(format!("tool_choice.{}", other.unwrap_or("unknown")));
                None
            }
        },
        _ => None,
    }
}

/// The Chat Completions request for a Responses request, and the names of what it could not carry. With
/// `takes_reasoning`, earlier reasoning goes back as `reasoning_content` on the assistant message after it.
pub fn to_chat(request: &Value, takes_reasoning: bool) -> (Value, Vec<String>) {
    let mut dropped = Dropped(Vec::new());
    let mut messages: Vec<Value> = Vec::new();
    if let Some(instructions) = request["instructions"].as_str().filter(|s| !s.is_empty()) {
        messages.push(json!({ "role": "system", "content": instructions }));
    }
    let items = match &request["input"] {
        Value::String(s) => vec![json!({ "type": "message", "role": "user", "content": s })],
        Value::Array(items) => items.clone(),
        _ => Vec::new(),
    };
    let mut reasoning: Option<String> = None;
    for item in &items {
        let kind = item["type"].as_str().unwrap_or(if item.get("role").is_some() { "message" } else { "" });
        match kind {
            "message" => match item["role"].as_str().unwrap_or("user") {
                "system" | "developer" => {
                    let text = content_text(&item["content"], &mut dropped);
                    if !text.is_empty() {
                        messages.push(json!({ "role": "system", "content": text }));
                    }
                }
                "assistant" => {
                    let text = content_text(&item["content"], &mut dropped);
                    if !text.is_empty() {
                        let message = assistant(&mut messages, &mut reasoning);
                        let earlier = message["content"].as_str().unwrap_or("").to_string();
                        message["content"] = json!(if earlier.is_empty() { text } else { format!("{earlier}\n{text}") });
                    }
                }
                _ => {
                    if let Some(content) = user_content(&item["content"], &mut dropped) {
                        messages.push(json!({ "role": "user", "content": content }));
                    }
                }
            },
            "function_call" | "custom_tool_call" => {
                let name = upstream_name(item["namespace"].as_str(), item["name"].as_str().unwrap_or(""));
                let arguments = if kind == "custom_tool_call" {
                    json!({ CUSTOM_INPUT: item["input"].as_str().unwrap_or("") }).to_string()
                } else {
                    item["arguments"].as_str().map(String::from).unwrap_or_else(|| "{}".into())
                };
                let call = json!({ "id": item["call_id"], "type": "function", "function": { "name": name, "arguments": arguments } });
                let message = assistant(&mut messages, &mut reasoning);
                match message["tool_calls"].as_array_mut() {
                    Some(calls) => calls.push(call),
                    None => message["tool_calls"] = json!([call]),
                }
            }
            "function_call_output" | "custom_tool_call_output" => {
                messages.push(json!({ "role": "tool", "tool_call_id": item["call_id"],
                    "content": output_text(&item["output"], &mut dropped) }));
            }
            "reasoning" => {
                let text = reasoning_text(item);
                if takes_reasoning && !text.is_empty() {
                    reasoning = Some(match reasoning.take() { Some(r) => format!("{r}\n{text}"), None => text });
                } else if !text.is_empty() || item["encrypted_content"].is_string() {
                    // Another provider's encrypted reasoning, or reasoning this provider does not take back.
                    dropped.add("reasoning");
                }
            }
            // Its tools are read by `chat_tools`.
            "additional_tools" => {}
            "" => dropped.add("input item"),
            other => dropped.add(other.to_string()),
        }
    }
    // Leading system and developer text is one system message: some providers take only one, and only first.
    let leading = messages.iter().take_while(|m| m["role"] == "system").count();
    if leading > 1 {
        let joined = messages[..leading].iter().filter_map(|m| m["content"].as_str()).collect::<Vec<_>>().join("\n\n");
        messages.splice(..leading, [json!({ "role": "system", "content": joined })]);
    }

    let mut out = json!({ "model": request["model"], "messages": messages });
    for field in ["temperature", "top_p", "stream"] {
        if !request[field].is_null() {
            out[field] = request[field].clone();
        }
    }
    if let Some(max) = request["max_output_tokens"].as_u64() {
        out["max_tokens"] = json!(max);
    }
    let tools = chat_tools(request, &mut dropped);
    if !tools.is_empty() {
        out["tools"] = json!(tools);
        if let Some(choice) = chat_tool_choice(&request["tool_choice"], &mut dropped) {
            out["tool_choice"] = choice;
        }
    }
    if request["parallel_tool_calls"] == false {
        dropped.add("parallel_tool_calls");
    }
    let format = &request["text"]["format"];
    match format["type"].as_str() {
        Some("json_schema") => {
            let mut schema = json!({ "name": format["name"].as_str().unwrap_or("response"), "schema": format["schema"] });
            if let Some(strict) = format["strict"].as_bool() {
                schema["strict"] = json!(strict);
            }
            out["response_format"] = json!({ "type": "json_schema", "json_schema": schema });
        }
        Some("json_object") => out["response_format"] = json!({ "type": "json_object" }),
        Some("text") | None => {}
        Some(other) => dropped.add(format!("text.format.{other}")),
    }
    if request["text"].get("verbosity").is_some_and(|v| !v.is_null()) {
        dropped.add("text.verbosity");
    }
    if request["reasoning"].get("effort").is_some_and(|v| !v.is_null()) {
        dropped.add("reasoning.effort");
    }
    if let Some(fields) = request.as_object() {
        for (key, value) in fields {
            if !value.is_null() && !HANDLED.contains(&key.as_str()) {
                dropped.add(key.clone());
            }
        }
    }
    (out, dropped.0)
}

/// What only Pygg's own storage could answer, which it does not keep: the refusal for it, if the request needs it.
pub fn stateful(request: &Value) -> Option<String> {
    for field in ["previous_response_id", "conversation"] {
        if request.get(field).is_some_and(|v| !v.is_null()) {
            return Some(format!("Pygg does not store responses, so \"{field}\" cannot be used. Send the whole \
                conversation in \"input\" with \"store\": false (Codex does)."));
        }
    }
    (request["background"] == true).then(|| "Pygg does not run responses in the background; send the request \
        without \"background\".".to_string())
}

// ---------- Response: Chat Completions → Responses ----------

/// What a translated answer needs from its request.
#[derive(Clone)]
pub struct Context {
    pub id: String,
    pub model: String,
    pub created: u64,
    pub tools: Tools,
    /// The client asked for reasoning summaries (`reasoning.summary`, other than `none`).
    pub summary: bool,
    tool_choice: Value,
    parallel_tool_calls: bool,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}

impl Context {
    pub fn new(request: &Value, model: &str) -> Context {
        Context { id: new_id("resp"), model: model.to_string(), created: now_secs(), tools: Tools::of(request),
            summary: request["reasoning"]["summary"].as_str().is_some_and(|s| s != "none"),
            tool_choice: if request["tool_choice"].is_null() { json!("auto") } else { request["tool_choice"].clone() },
            parallel_tool_calls: request["parallel_tool_calls"] != false }
    }

    /// The response object, as `response.created` and the final event carry it and a non-streamed answer is.
    fn response(&self, status: &str, output: Vec<Value>, usage: Option<&Usage>, finish: Option<&str>) -> Value {
        let mut out = json!({ "id": self.id, "object": "response", "created_at": self.created, "status": status,
            "model": self.model, "output": output, "error": null, "incomplete_details": null,
            "parallel_tool_calls": self.parallel_tool_calls, "tool_choice": self.tool_choice, "store": false });
        if status != "in_progress" {
            out["usage"] = usage_json(usage);
        }
        if status == "incomplete" {
            let reason = if finish == Some("content_filter") { "content_filter" } else { "max_output_tokens" };
            out["incomplete_details"] = json!({ "reason": reason });
        }
        out
    }

    fn call_item(&self, call_id: &str, upstream: &str, arguments: &str, status: &str) -> Value {
        let tool = self.tools.lookup(upstream);
        let mut item = match tool.kind {
            ToolKind::Function => json!({ "id": new_id("fc"), "type": "function_call", "status": status,
                "call_id": call_id, "name": tool.name, "arguments": arguments }),
            ToolKind::Custom => json!({ "id": new_id("ctc"), "type": "custom_tool_call", "status": status,
                "call_id": call_id, "name": tool.name, "input": custom_input(arguments) }),
        };
        if let Some(namespace) = tool.namespace {
            item["namespace"] = json!(namespace);
        }
        item
    }

    fn reasoning_item(&self, text: &str) -> Value {
        if self.summary {
            json!({ "id": new_id("rs"), "type": "reasoning", "summary": [{ "type": "summary_text", "text": text }] })
        } else {
            json!({ "id": new_id("rs"), "type": "reasoning", "summary": [],
                "content": [{ "type": "reasoning_text", "text": text }] })
        }
    }
}

/// A custom tool's input from the arguments of the function it became: the `input` string, or the arguments as
/// they came when the model did not follow the schema.
fn custom_input(arguments: &str) -> String {
    match serde_json::from_str::<Value>(arguments) {
        Ok(Value::Object(map)) => match map.get(CUSTOM_INPUT) {
            Some(Value::String(s)) => s.clone(),
            _ => arguments.to_string(),
        },
        _ => arguments.to_string(),
    }
}

/// Responses usage: `input_tokens` is the whole prompt with the cached part in its details, as in Chat Completions.
pub fn usage_json(usage: Option<&Usage>) -> Value {
    let u = usage.cloned().unwrap_or_default();
    json!({ "input_tokens": u.prompt, "input_tokens_details": { "cached_tokens": u.cached },
        "output_tokens": u.completion, "output_tokens_details": { "reasoning_tokens": 0 },
        "total_tokens": u.prompt + u.completion })
}

fn status_of(finish: Option<&str>) -> &'static str {
    match finish {
        Some("length" | "content_filter") => "incomplete",
        _ => "completed",
    }
}

/// A provider's reasoning in a message or delta: `reasoning_content` (Qwen, DeepSeek, Kimi, GLM) or `reasoning`
/// (OpenRouter).
fn reasoning_of(message: &Value) -> Option<&str> {
    message["reasoning_content"].as_str().or_else(|| message["reasoning"].as_str()).filter(|t| !t.is_empty())
}

/// A Responses object for a chat completion, and its usage for the ledger.
pub fn from_chat(response: &Value, ctx: &Context) -> (Value, Option<Usage>) {
    let choice = &response["choices"][0];
    let message = &choice["message"];
    let mut output = Vec::new();
    if let Some(reasoning) = reasoning_of(message) {
        output.push(ctx.reasoning_item(reasoning));
    }
    if let Some(text) = message["content"].as_str().filter(|t| !t.is_empty()) {
        output.push(json!({ "id": new_id("msg"), "type": "message", "status": "completed", "role": "assistant",
            "content": [{ "type": "output_text", "text": text, "annotations": [] }] }));
    }
    for call in message["tool_calls"].as_array().into_iter().flatten() {
        let arguments = match &call["function"]["arguments"] {
            Value::String(s) => s.clone(),
            Value::Null => "{}".to_string(),
            other => other.to_string(),
        };
        let call_id = call["id"].as_str().filter(|s| !s.is_empty()).map(String::from).unwrap_or_else(|| new_id("call"));
        output.push(ctx.call_item(&call_id, call["function"]["name"].as_str().unwrap_or(""), &arguments, "completed"));
    }
    let usage = Usage::from_json(&response["usage"]);
    let finish = choice["finish_reason"].as_str();
    (ctx.response(status_of(finish), output, usage.as_ref(), finish), usage)
}

// ---------- Streaming: Chat Completions chunks → Responses events ----------

#[derive(PartialEq)]
enum Open {
    Reasoning,
    Text,
    /// A tool call, by its key (the chat tool-call index, or its id when the provider sends none).
    Call(String),
}

/// The item being streamed.
struct Item {
    kind: Open,
    /// Its output index once it has started; a tool call starts only once its name is known (Qwen sends the name
    /// after the first fragment of its arguments).
    index: Option<usize>,
    id: String,
    text: String,
    call_id: Option<String>,
    name: Option<String>,
    /// Arguments received and not yet sent.
    held: String,
}

/// Turns Chat Completions stream chunks into Responses stream events.
pub struct Stream {
    ctx: Context,
    sequence: u64,
    started: bool,
    output: Vec<Value>,
    open: Option<Item>,
    finish: Option<String>,
    any_call: bool,
    pub usage: Option<Usage>,
}

impl Stream {
    pub fn new(ctx: Context) -> Stream {
        Stream { ctx, sequence: 0, started: false, output: Vec::new(), open: None, finish: None, any_call: false, usage: None }
    }

    fn event(&mut self, kind: &str, mut data: Value) -> String {
        data["type"] = json!(kind);
        data["sequence_number"] = json!(self.sequence);
        self.sequence += 1;
        format!("event: {kind}\ndata: {data}\n\n")
    }

    fn start(&mut self, out: &mut Vec<String>) {
        if !self.started {
            self.started = true;
            let response = self.ctx.response("in_progress", Vec::new(), None, None);
            out.push(self.event("response.created", json!({ "response": response })));
            out.push(self.event("response.in_progress", json!({ "response": response })));
        }
    }

    /// Starts an item at the next output index, with its `added` events. The item is not in `open` meanwhile.
    fn begin(&mut self, item: &mut Item, out: &mut Vec<String>) {
        let index = self.output.len();
        item.index = Some(index);
        match &item.kind {
            Open::Reasoning => {
                let added = if self.ctx.summary {
                    json!({ "id": item.id, "type": "reasoning", "summary": [] })
                } else {
                    json!({ "id": item.id, "type": "reasoning", "summary": [], "content": [] })
                };
                out.push(self.event("response.output_item.added", json!({ "output_index": index, "item": added })));
                if self.ctx.summary {
                    out.push(self.event("response.reasoning_summary_part.added", json!({ "item_id": item.id,
                        "output_index": index, "summary_index": 0, "part": { "type": "summary_text", "text": "" } })));
                }
            }
            Open::Text => {
                out.push(self.event("response.output_item.added", json!({ "output_index": index, "item": {
                    "id": item.id, "type": "message", "status": "in_progress", "role": "assistant", "content": [] } })));
                out.push(self.event("response.content_part.added", json!({ "item_id": item.id, "output_index": index,
                    "content_index": 0, "part": { "type": "output_text", "text": "", "annotations": [] } })));
            }
            Open::Call(_) => {
                let call_id = item.call_id.clone().unwrap_or_else(|| new_id("call"));
                item.call_id = Some(call_id.clone());
                let mut added = self.ctx.call_item(&call_id, item.name.as_deref().unwrap_or(""), "", "in_progress");
                added["id"] = json!(item.id);
                out.push(self.event("response.output_item.added", json!({ "output_index": index, "item": added })));
            }
        }
    }

    fn delta(&mut self, item: &Item, delta: &str, out: &mut Vec<String>) {
        let Some(index) = item.index else { return };
        let (kind, extra) = match &item.kind {
            Open::Reasoning if self.ctx.summary => ("response.reasoning_summary_text.delta", json!({ "summary_index": 0 })),
            Open::Reasoning => ("response.reasoning_text.delta", json!({ "content_index": 0 })),
            Open::Text => ("response.output_text.delta", json!({ "content_index": 0, "logprobs": [] })),
            Open::Call(_) if self.ctx.tools.lookup(item.name.as_deref().unwrap_or("")).kind == ToolKind::Custom => return,
            Open::Call(_) => ("response.function_call_arguments.delta", json!({})),
        };
        let mut data = json!({ "item_id": item.id, "output_index": index, "delta": delta });
        for (k, v) in extra.as_object().into_iter().flatten() {
            data[k] = v.clone();
        }
        out.push(self.event(kind, data));
    }

    /// Ends the open item with its `done` events. A tool call that never got a name has nothing to end; it is
    /// dropped, as no client could run it.
    fn close(&mut self, out: &mut Vec<String>) {
        let Some(item) = self.open.take() else { return };
        let Some(index) = item.index else { return };
        let id = item.id.clone();
        let done = match &item.kind {
            Open::Reasoning => {
                let mut done = self.ctx.reasoning_item(&item.text);
                done["id"] = json!(id);
                if self.ctx.summary {
                    out.push(self.event("response.reasoning_summary_text.done", json!({ "item_id": id, "output_index": index,
                        "summary_index": 0, "text": item.text })));
                    out.push(self.event("response.reasoning_summary_part.done", json!({ "item_id": id, "output_index": index,
                        "summary_index": 0, "part": { "type": "summary_text", "text": item.text } })));
                } else {
                    out.push(self.event("response.reasoning_text.done", json!({ "item_id": id, "output_index": index,
                        "content_index": 0, "text": item.text })));
                }
                done
            }
            Open::Text => {
                let part = json!({ "type": "output_text", "text": item.text, "annotations": [] });
                out.push(self.event("response.output_text.done", json!({ "item_id": id, "output_index": index,
                    "content_index": 0, "text": item.text, "logprobs": [] })));
                out.push(self.event("response.content_part.done", json!({ "item_id": id, "output_index": index,
                    "content_index": 0, "part": part })));
                json!({ "id": id, "type": "message", "status": "completed", "role": "assistant", "content": [part] })
            }
            Open::Call(_) => {
                let arguments = item.text.clone();
                let mut done = self.ctx.call_item(item.call_id.as_deref().unwrap_or(""), item.name.as_deref().unwrap_or(""),
                    &arguments, "completed");
                done["id"] = json!(id);
                if done["type"] == "custom_tool_call" {
                    let input = done["input"].clone();
                    out.push(self.event("response.custom_tool_call_input.delta", json!({ "item_id": id, "output_index": index,
                        "delta": input })));
                    out.push(self.event("response.custom_tool_call_input.done", json!({ "item_id": id, "output_index": index,
                        "input": input })));
                } else {
                    out.push(self.event("response.function_call_arguments.done", json!({ "item_id": id, "output_index": index,
                        "arguments": arguments })));
                }
                done
            }
        };
        out.push(self.event("response.output_item.done", json!({ "output_index": index, "item": done })));
        self.output.push(done);
    }

    /// Continues the open item of `kind`, or closes the open one and starts a new one.
    fn ensure(&mut self, kind: Open, out: &mut Vec<String>) {
        if self.open.as_ref().is_some_and(|o| o.kind == kind) {
            return;
        }
        self.close(out);
        let prefix = match kind { Open::Reasoning => "rs", Open::Text => "msg", Open::Call(_) => "fc" };
        let mut item = Item { kind, index: None, id: new_id(prefix), text: String::new(), call_id: None, name: None,
            held: String::new() };
        if !matches!(item.kind, Open::Call(_)) {
            self.begin(&mut item, out);
        }
        self.open = Some(item);
    }

    fn append(&mut self, text: &str, out: &mut Vec<String>) {
        let Some(mut item) = self.open.take() else { return };
        item.text.push_str(text);
        self.delta(&item, text, out);
        self.open = Some(item);
    }

    /// The events for one chunk (its `data` object).
    pub fn push(&mut self, chunk: &Value) -> Vec<String> {
        let mut out = Vec::new();
        self.start(&mut out);
        if let Some(usage) = Usage::from_json(&chunk["usage"]) {
            self.usage = Some(usage);
        }
        let Some(choice) = chunk["choices"].get(0) else { return out };
        let delta = &choice["delta"];
        if let Some(reasoning) = reasoning_of(delta) {
            self.ensure(Open::Reasoning, &mut out);
            self.append(reasoning, &mut out);
        }
        if let Some(text) = delta["content"].as_str().filter(|t| !t.is_empty()) {
            self.ensure(Open::Text, &mut out);
            self.append(text, &mut out);
        }
        for (position, call) in delta["tool_calls"].as_array().into_iter().flatten().enumerate() {
            let key = call["index"].as_u64().map(|i| i.to_string())
                .or_else(|| call["id"].as_str().filter(|s| !s.is_empty()).map(String::from))
                .unwrap_or_else(|| position.to_string());
            let id = call["id"].as_str().filter(|s| !s.is_empty());
            let name = call["function"]["name"].as_str().filter(|s| !s.is_empty());
            // A fragment continues the open call when it has the same key, unless both carry ids that differ
            // (some providers send every whole call at index 0).
            let continues = match &self.open {
                Some(Item { kind: Open::Call(k), call_id, .. }) if *k == key => match (id, call_id.as_deref()) {
                    (Some(new), Some(old)) => new == old,
                    _ => true,
                },
                _ => false,
            };
            if !continues {
                self.close(&mut out);
                self.open = Some(Item { kind: Open::Call(key), index: None, id: new_id("fc"), text: String::new(),
                    call_id: None, name: None, held: String::new() });
            }
            let fragment = match &call["function"]["arguments"] {
                Value::String(s) => s.clone(),
                Value::Object(_) => call["function"]["arguments"].to_string(),
                _ => String::new(),
            };
            let Some(mut item) = self.open.take() else { continue };
            if item.call_id.is_none() {
                item.call_id = id.map(String::from);
            }
            if item.name.is_none() {
                item.name = name.map(String::from);
            }
            item.held.push_str(&fragment);
            if item.index.is_none() && item.name.is_some() {
                self.begin(&mut item, &mut out);
                self.any_call = true;
            }
            if item.index.is_some() && !item.held.is_empty() {
                let held = std::mem::take(&mut item.held);
                item.text.push_str(&held);
                self.delta(&item, &held, &mut out);
            }
            self.open = Some(item);
        }
        if let Some(finish) = choice["finish_reason"].as_str() {
            self.finish = Some(finish.to_string());
        }
        out
    }

    /// The closing events: the open item's end, then `response.completed` (or `response.incomplete` when the
    /// output limit or a filter cut it short) with every item and the usage.
    pub fn finish(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        self.start(&mut out);
        self.close(&mut out);
        // Gemini reports "stop" even when it called a tool; that is not a cut-short answer either way.
        let finish = if self.any_call { None } else { self.finish.clone() };
        let status = status_of(finish.as_deref());
        let response = self.ctx.response(status, self.output.clone(), self.usage.as_ref(), finish.as_deref());
        let kind = if status == "completed" { "response.completed" } else { "response.incomplete" };
        out.push(self.event(kind, json!({ "response": response })));
        out
    }

    /// The events for a provider stream that broke off: `response.failed`, which a client reads as an error
    /// it may retry.
    pub fn fail(&mut self, message: &str) -> Vec<String> {
        let mut out = Vec::new();
        self.start(&mut out);
        self.open = None;
        let mut response = self.ctx.response("failed", self.output.clone(), self.usage.as_ref(), None);
        response["error"] = json!({ "code": "server_error", "message": message });
        out.push(self.event("response.failed", json!({ "response": response })));
        out
    }
}

// ---------- Handler ----------

pub async fn responses(State(shared): State<Shared>, headers: HeaderMap, body: Bytes) -> Response {
    let client = match authenticate(&shared, &headers) {
        Ok(client) => client,
        Err(refusal) => return refusal.openai(),
    };
    let (request, requested) = match parse_body(&body) {
        Ok(parsed) => parsed,
        Err(refusal) => return refusal.openai(),
    };
    if let Some(message) = stateful(&request) {
        return openai_error(StatusCode::BAD_REQUEST, "invalid_request_error", &message);
    }
    let stream = request["stream"] == true;
    let session = session_key(&headers);
    let incoming = Incoming { surface: Surface::Responses, body: &request, headers: &headers, requested: &requested,
        session: session.as_deref(), stream };
    let forwarded = match forward(&shared, &client, &incoming).await {
        Ok(forwarded) => forwarded,
        Err(refusal) => return refusal.openai(),
    };
    let ctx = Context::new(&request, &requested);
    match forwarded.mode {
        Mode::ResponsesToOpenAi => from_chat_upstream(shared, forwarded, stream, ctx).await,
        Mode::ResponsesToAnthropic => from_anthropic_upstream(shared, forwarded, stream, ctx).await,
        // A Responses request takes no other translated route; anything else is native.
        _ => native(shared, forwarded, stream).await,
    }
}

fn broke_off(shared: &Shared, forwarded: Forwarded, error: reqwest::Error) -> Response {
    let message = crate::redact::redact(&error.to_string(), &[]);
    record_success(shared, forwarded, None, "stream_error", Some(&message));
    openai_error(StatusCode::BAD_GATEWAY, "api_error", &format!("The provider's response broke off: {message}"))
}

/// Same format: the provider's answer goes back unchanged; only its usage is read on the way.
async fn native(shared: Shared, mut forwarded: Forwarded, stream: bool) -> Response {
    let status = forwarded.response.status();
    let content_type = forwarded.response.headers().get(header::CONTENT_TYPE).cloned();
    let upstream_headers = forwarded.response.headers().clone();
    let upstream = std::mem::replace(&mut forwarded.response, status_only(status));
    if !stream {
        return match upstream.bytes().await {
            Ok(bytes) => {
                let parsed: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
                let usage = Usage::from_responses(&parsed["usage"]);
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
            Err(e) => broke_off(&shared, forwarded, e),
        };
    }
    let (tx, rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(64);
    let mut out = stream_response(rx, status, content_type);
    provider_headers(&upstream_headers, out.headers_mut());
    route_headers(&mut out, &forwarded);
    tokio::spawn(async move {
        let mut scanner = super::sse::UsageScanner::responses();
        let (mut outcome, error) = pump(upstream, &tx, |chunk| scanner.feed(chunk)).await;
        if outcome == "ok" && scanner.error.is_some() {
            outcome = "stream_error";
        }
        let error = error.or(scanner.error.clone());
        record_success(&shared, forwarded, scanner.usage.as_ref(), outcome, error.as_deref());
    });
    out
}

/// Splits a server-sent event stream into the payloads of its `data:` lines.
#[derive(Default)]
struct DataLines {
    line: Vec<u8>,
}

impl DataLines {
    fn feed(&mut self, bytes: &[u8]) -> Vec<String> {
        let mut out = Vec::new();
        for &b in bytes {
            if b != b'\n' {
                if self.line.len() < 4 * 1024 * 1024 {
                    self.line.push(b);
                }
                continue;
            }
            let text = String::from_utf8_lossy(&self.line).trim_end_matches('\r').to_string();
            self.line.clear();
            if let Some(data) = text.strip_prefix("data:") {
                out.push(data.trim().to_string());
            }
        }
        out
    }
}

fn translated_response(forwarded: &Forwarded, body: Value) -> Response {
    let mut out = Json(body).into_response();
    route_headers(&mut out, forwarded);
    out
}

fn sse_response(forwarded: &Forwarded) -> (mpsc::Sender<Result<Bytes, std::io::Error>>, Response) {
    let (tx, rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(64);
    let mut out = stream_response(rx, StatusCode::OK, Some(HeaderValue::from_static("text/event-stream")));
    route_headers(&mut out, forwarded);
    (tx, out)
}

async fn send_all(tx: &mpsc::Sender<Result<Bytes, std::io::Error>>, events: Vec<String>) -> bool {
    for event in events {
        if tx.send(Ok(Bytes::from(event))).await.is_err() {
            return false;
        }
    }
    true
}

/// Translated to Chat Completions: the answer is translated back, streamed or not.
async fn from_chat_upstream(shared: Shared, mut forwarded: Forwarded, stream: bool, ctx: Context) -> Response {
    let upstream = std::mem::replace(&mut forwarded.response, status_only(StatusCode::OK));
    if !stream {
        return match upstream.bytes().await {
            Ok(bytes) => {
                let parsed: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
                shared.capture_signatures(&parsed);
                let (body, usage) = from_chat(&parsed, &ctx);
                let out = translated_response(&forwarded, body);
                record_success(&shared, forwarded, usage.as_ref(), "ok", None);
                out
            }
            Err(e) => broke_off(&shared, forwarded, e),
        };
    }
    let (tx, out) = sse_response(&forwarded);
    tokio::spawn(async move {
        let mut translator = Stream::new(ctx);
        let mut lines = DataLines::default();
        let mut bytes = upstream.bytes_stream();
        let (mut outcome, mut error) = ("ok", None);
        'read: while let Some(chunk) = bytes.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(e) => { outcome = "stream_error"; error = Some(crate::redact::redact(&e.to_string(), &[])); break; }
            };
            for data in lines.feed(&chunk) {
                if data == "[DONE]" {
                    break 'read;
                }
                let Ok(value) = serde_json::from_str::<Value>(&data) else { continue };
                shared.capture_signatures(&value);
                if !send_all(&tx, translator.push(&value)).await {
                    outcome = "client_disconnected";
                    break 'read;
                }
            }
        }
        match outcome {
            "ok" => { send_all(&tx, translator.finish()).await; }
            "stream_error" => { send_all(&tx, translator.fail("The provider's stream broke off")).await; }
            _ => {}
        }
        let usage = translator.usage.clone();
        record_success(&shared, forwarded, usage.as_ref(), outcome, error.as_deref());
    });
    out
}

/// The Chat Completions chunks a `ChunkTranslator` wrote, as values.
fn chunks(lines: Vec<String>) -> Vec<Value> {
    lines.iter().filter_map(|l| l.strip_prefix("data: ")).map(str::trim).filter(|d| *d != "[DONE]")
        .filter_map(|d| serde_json::from_str(d).ok()).collect()
}

/// Translated to Anthropic Messages through Chat Completions: the answer comes back the same two steps.
async fn from_anthropic_upstream(shared: Shared, mut forwarded: Forwarded, stream: bool, ctx: Context) -> Response {
    let upstream = std::mem::replace(&mut forwarded.response, status_only(StatusCode::OK));
    let model = ctx.model.clone();
    if !stream {
        return match upstream.bytes().await {
            Ok(bytes) => {
                let parsed: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
                let (chat, usage) = openai_to_anthropic::from_anthropic(&parsed, &model);
                let (body, _) = from_chat(&chat, &ctx);
                let out = translated_response(&forwarded, body);
                record_success(&shared, forwarded, usage.as_ref(), "ok", None);
                out
            }
            Err(e) => broke_off(&shared, forwarded, e),
        };
    }
    let (tx, out) = sse_response(&forwarded);
    tokio::spawn(async move {
        let mut to_chat = openai_to_anthropic::ChunkTranslator::new(&model, true);
        let mut translator = Stream::new(ctx);
        let mut lines = DataLines::default();
        let mut bytes = upstream.bytes_stream();
        let (mut outcome, mut error): (&str, Option<String>) = ("ok", None);
        'read: while let Some(chunk) = bytes.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(e) => { outcome = "stream_error"; error = Some(crate::redact::redact(&e.to_string(), &[])); break; }
            };
            for data in lines.feed(&chunk) {
                let Ok(event) = serde_json::from_str::<Value>(&data) else { continue };
                if event["type"] == "error" {
                    outcome = "stream_error";
                    error = Some(event["error"]["message"].as_str().unwrap_or("stream error").to_string());
                    break 'read;
                }
                let events: Vec<String> = chunks(to_chat.push(&event)).iter().flat_map(|c| translator.push(c)).collect();
                if !send_all(&tx, events).await {
                    outcome = "client_disconnected";
                    break 'read;
                }
            }
        }
        match outcome {
            "ok" => {
                let mut events: Vec<String> = chunks(to_chat.finish()).iter().flat_map(|c| translator.push(c)).collect();
                events.extend(translator.finish());
                send_all(&tx, events).await;
            }
            "stream_error" => {
                let message = error.clone().unwrap_or_else(|| "The provider's stream broke off".into());
                send_all(&tx, translator.fail(&message)).await;
            }
            _ => {}
        }
        // The Anthropic usage, with its cache writes, is the one the ledger keeps.
        let usage = to_chat.usage.clone();
        record_success(&shared, forwarded, usage.as_ref(), outcome, error.as_deref());
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A request shaped like Codex CLI 0.163.0's, captured on 2026-11-12 (trimmed).
    fn codex_request() -> Value {
        json!({ "model": "auto", "stream": true, "store": false, "include": ["reasoning.encrypted_content"],
            "tool_choice": "auto", "parallel_tool_calls": true, "reasoning": { "summary": "auto" },
            "prompt_cache_key": "01a1", "client_metadata": { "thread_id": "01a1" },
            "instructions": "Be precise.",
            "tools": [
                { "type": "function", "name": "exec_command", "description": "Runs a command", "strict": false,
                    "parameters": { "type": "object", "properties": { "cmd": { "type": "string" } }, "required": ["cmd"],
                        "additionalProperties": false } },
                { "type": "custom", "name": "apply_patch", "description": "Edit files.",
                    "format": { "type": "grammar", "syntax": "lark", "definition": "start: begin_patch hunk+ end_patch" } },
                { "type": "namespace", "name": "multi_agent_v1", "description": "Agents.", "tools": [
                    { "type": "function", "name": "close_agent", "parameters": { "type": "object", "properties": {} } }] },
                { "type": "web_search", "external_web_access": false }],
            "input": [
                { "type": "message", "role": "developer", "content": [{ "type": "input_text", "text": "You are Codex." }] },
                { "type": "message", "role": "user", "content": [{ "type": "input_text", "text": "<environment_context/>" }] },
                { "type": "message", "role": "user", "content": [{ "type": "input_text", "text": "Fix add()" },
                    { "type": "input_image", "image_url": "data:image/png;base64,QUJD" }] },
                { "type": "reasoning", "id": "rs_1", "summary": [{ "type": "summary_text", "text": "Need to look." }] },
                { "type": "function_call", "call_id": "call_1", "name": "exec_command", "arguments": "{\"cmd\":\"cat a.py\"}" },
                { "type": "function_call_output", "call_id": "call_1", "output": "def add(a, b): return a - b" },
                { "type": "custom_tool_call", "call_id": "call_2", "name": "apply_patch", "input": "*** Begin Patch\n" },
                { "type": "function_call", "call_id": "call_3", "name": "close_agent", "namespace": "multi_agent_v1", "arguments": "{}" },
                { "type": "custom_tool_call_output", "call_id": "call_2", "output": [{ "type": "input_text", "text": "Done" }] },
                { "type": "function_call_output", "call_id": "call_3", "output": "closed" },
                { "type": "reasoning", "id": "rs_2", "summary": [], "encrypted_content": "gAAAA" },
                { "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": "Fixed." }] }] })
    }

    #[test]
    fn translates_a_codex_request_to_chat_completions() {
        let (out, dropped) = to_chat(&codex_request(), false);
        let m = out["messages"].as_array().unwrap();
        assert_eq!(m[0], json!({ "role": "system", "content": "Be precise.\n\nYou are Codex." }), "leading system text is one message");
        assert_eq!(m[1], json!({ "role": "user", "content": "<environment_context/>" }));
        assert_eq!(m[2]["content"][1], json!({ "type": "image_url", "image_url": { "url": "data:image/png;base64,QUJD" } }));
        assert_eq!(m[3]["tool_calls"][0]["function"], json!({ "name": "exec_command", "arguments": "{\"cmd\":\"cat a.py\"}" }));
        assert!(m[3].get("reasoning_content").is_none(), "not to a provider that does not take it");
        assert_eq!(m[4], json!({ "role": "tool", "tool_call_id": "call_1", "content": "def add(a, b): return a - b" }));
        // Parallel calls are one assistant message; the custom call's input is the `input` argument.
        let calls = m[5]["tool_calls"].as_array().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0]["function"]["arguments"], "{\"input\":\"*** Begin Patch\\n\"}");
        assert_eq!(calls[1]["function"]["name"], "multi_agent_v1__close_agent");
        assert_eq!(m[6], json!({ "role": "tool", "tool_call_id": "call_2", "content": "Done" }));
        assert_eq!(m[8], json!({ "role": "assistant", "content": "Fixed." }));
        assert_eq!(m.len(), 9);

        let tools = out["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["function"]["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["exec_command", "apply_patch", "multi_agent_v1__close_agent"]);
        assert_eq!(tools[1]["function"]["parameters"]["required"], json!(["input"]));
        assert!(tools[1]["function"]["description"].as_str().unwrap().contains("start: begin_patch hunk+ end_patch"));
        assert_eq!(out["tool_choice"], "auto");
        assert_eq!(out["stream"], true);
        assert_eq!(dropped, ["reasoning", "web_search tool", "prompt_cache_key", "client_metadata"]);
    }

    #[test]
    fn reasoning_goes_back_to_a_provider_that_takes_it() {
        let (out, dropped) = to_chat(&codex_request(), true);
        assert_eq!(out["messages"][3]["reasoning_content"], "Need to look.");
        assert!(dropped.contains(&"reasoning".to_string()), "the encrypted one is still dropped: {dropped:?}");
        let plain = json!({ "model": "m", "input": [{ "type": "reasoning", "summary": [{ "type": "summary_text", "text": "Hm." }] },
            { "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": "Hi" }] }] });
        let (out, dropped) = to_chat(&plain, true);
        assert_eq!(out["messages"][0], json!({ "role": "assistant", "content": "Hi", "reasoning_content": "Hm." }));
        assert!(dropped.is_empty());
    }

    #[test]
    fn a_plain_request_loses_nothing_and_options_cross() {
        let request = json!({ "model": "m", "input": "Hi", "max_output_tokens": 50, "temperature": 0.2,
            "text": { "format": { "type": "json_schema", "name": "r", "schema": { "type": "object" }, "strict": true } } });
        let (out, dropped) = to_chat(&request, false);
        assert_eq!(out["messages"], json!([{ "role": "user", "content": "Hi" }]));
        assert_eq!((out["max_tokens"].as_u64(), out["temperature"].as_f64()), (Some(50), Some(0.2)));
        assert_eq!(out["response_format"]["json_schema"]["strict"], true);
        assert!(dropped.is_empty(), "{dropped:?}");
        assert!(stateful(&request).is_none());
        assert!(stateful(&json!({ "previous_response_id": "resp_1" })).unwrap().contains("previous_response_id"));
        assert!(stateful(&json!({ "background": true })).is_some());
    }

    #[test]
    fn a_tool_choice_names_the_flattened_function() {
        let request = json!({ "model": "m", "input": "x", "tools": codex_request()["tools"],
            "tool_choice": { "type": "custom", "name": "apply_patch" } });
        let (out, _) = to_chat(&request, false);
        assert_eq!(out["tool_choice"], json!({ "type": "function", "function": { "name": "apply_patch" } }));
    }

    #[test]
    fn translates_a_chat_completion_with_reasoning_and_every_kind_of_call() {
        let ctx = Context::new(&codex_request(), "auto");
        let response = json!({ "choices": [{ "finish_reason": "tool_calls", "message": { "reasoning_content": "Look first.",
            "content": "Patching.", "tool_calls": [
                { "id": "c1", "type": "function", "function": { "name": "apply_patch", "arguments": "{\"input\":\"*** Begin Patch\"}" } },
                { "id": "c2", "type": "function", "function": { "name": "multi_agent_v1__close_agent", "arguments": "{}" } },
                { "id": "c3", "type": "function", "function": { "name": "exec_command", "arguments": "{\"cmd\":\"ls\"}" } }] } }],
            "usage": { "prompt_tokens": 100, "completion_tokens": 20, "prompt_tokens_details": { "cached_tokens": 60 } } });
        let (out, usage) = from_chat(&response, &ctx);
        assert_eq!((out["object"].as_str(), out["status"].as_str(), out["model"].as_str()), (Some("response"), Some("completed"), Some("auto")));
        let output = out["output"].as_array().unwrap();
        assert_eq!(output[0]["type"], "reasoning");
        assert_eq!(output[0]["summary"][0], json!({ "type": "summary_text", "text": "Look first." }));
        assert_eq!(output[1]["content"][0]["text"], "Patching.");
        assert_eq!((output[2]["type"].as_str(), output[2]["input"].as_str(), output[2]["call_id"].as_str()),
            (Some("custom_tool_call"), Some("*** Begin Patch"), Some("c1")));
        assert_eq!((output[3]["name"].as_str(), output[3]["namespace"].as_str()), (Some("close_agent"), Some("multi_agent_v1")));
        assert_eq!(output[4]["arguments"], "{\"cmd\":\"ls\"}");
        assert_eq!(out["usage"], json!({ "input_tokens": 100, "input_tokens_details": { "cached_tokens": 60 },
            "output_tokens": 20, "output_tokens_details": { "reasoning_tokens": 0 }, "total_tokens": 120 }));
        assert_eq!(usage.unwrap().cached, 60);

        let cut = json!({ "choices": [{ "finish_reason": "length", "message": { "content": "Partial" } }] });
        let (out, _) = from_chat(&cut, &Context::new(&json!({}), "m"));
        assert_eq!((out["status"].as_str(), out["incomplete_details"]["reason"].as_str()), (Some("incomplete"), Some("max_output_tokens")));
    }

    fn parse(events: &[String]) -> Vec<Value> {
        events.iter().map(|e| {
            let mut lines = e.lines();
            let kind = lines.next().unwrap().trim_start_matches("event: ").to_string();
            let data: Value = serde_json::from_str(lines.next().unwrap().trim_start_matches("data: ")).unwrap();
            assert_eq!(data["type"], kind.as_str());
            data
        }).collect()
    }

    fn kinds(events: &[Value]) -> Vec<&str> {
        events.iter().map(|e| e["type"].as_str().unwrap()).collect()
    }

    #[test]
    fn streams_reasoning_text_and_a_function_call() {
        let mut t = Stream::new(Context::new(&codex_request(), "auto"));
        let mut events = Vec::new();
        events.extend(t.push(&json!({ "choices": [{ "delta": { "role": "assistant", "reasoning_content": "Think" } }] })));
        events.extend(t.push(&json!({ "choices": [{ "delta": { "reasoning_content": "ing." } }] })));
        events.extend(t.push(&json!({ "choices": [{ "delta": { "content": "Run" } }] })));
        events.extend(t.push(&json!({ "choices": [{ "delta": { "content": "ning." } }] })));
        // Qwen's way: the first fragment has no id and no name.
        events.extend(t.push(&json!({ "choices": [{ "delta": { "tool_calls": [{ "index": 0, "id": "", "function": { "arguments": "{\"cmd\":" } }] } }] })));
        events.extend(t.push(&json!({ "choices": [{ "delta": { "tool_calls": [{ "index": 0, "id": "call_7",
            "function": { "name": "exec_command", "arguments": "\"ls\"}" } }] }, "finish_reason": "tool_calls" }] })));
        events.extend(t.push(&json!({ "choices": [], "usage": { "prompt_tokens": 9, "completion_tokens": 4 } })));
        events.extend(t.finish());
        let events = parse(&events);
        assert_eq!(kinds(&events), ["response.created", "response.in_progress",
            "response.output_item.added", "response.reasoning_summary_part.added", "response.reasoning_summary_text.delta",
            "response.reasoning_summary_text.delta", "response.reasoning_summary_text.done", "response.reasoning_summary_part.done",
            "response.output_item.done",
            "response.output_item.added", "response.content_part.added", "response.output_text.delta", "response.output_text.delta",
            "response.output_text.done", "response.content_part.done", "response.output_item.done",
            "response.output_item.added", "response.function_call_arguments.delta", "response.function_call_arguments.done",
            "response.output_item.done", "response.completed"]);
        let sequence: Vec<u64> = events.iter().map(|e| e["sequence_number"].as_u64().unwrap()).collect();
        assert_eq!(sequence, (0..events.len() as u64).collect::<Vec<_>>());
        assert_eq!(events[8]["item"]["summary"][0]["text"], "Thinking.");
        assert_eq!(events[15]["item"]["content"][0]["text"], "Running.");
        assert_eq!((events[16]["output_index"].as_u64(), events[16]["item"]["call_id"].as_str(), events[16]["item"]["name"].as_str()),
            (Some(2), Some("call_7"), Some("exec_command")));
        assert_eq!(events[17]["delta"], "{\"cmd\":\"ls\"}", "held arguments are sent once the call starts");
        assert_eq!(events[19]["item"]["arguments"], "{\"cmd\":\"ls\"}");
        let done = &events[20]["response"];
        assert_eq!(done["status"], "completed");
        assert_eq!(done["output"].as_array().unwrap().len(), 3);
        assert_eq!(done["usage"]["input_tokens"], 9);
        assert_eq!(t.usage.unwrap().completion, 4);
    }

    #[test]
    fn streams_a_custom_tool_call_as_its_input() {
        let mut t = Stream::new(Context::new(&codex_request(), "auto"));
        let mut events = t.push(&json!({ "choices": [{ "delta": { "tool_calls": [{ "index": 0, "id": "c1",
            "function": { "name": "apply_patch", "arguments": "{\"input\":\"*** Begin" } }] } }] }));
        events.extend(t.push(&json!({ "choices": [{ "delta": { "tool_calls": [{ "index": 0, "function": { "arguments": " Patch\"}" } }] } }] })));
        events.extend(t.push(&json!({ "choices": [{ "delta": { "tool_calls": [{ "index": 1, "id": "c2",
            "function": { "name": "multi_agent_v1__close_agent", "arguments": "{}" } }] }, "finish_reason": "stop" }] })));
        events.extend(t.finish());
        let events = parse(&events);
        assert_eq!(kinds(&events), ["response.created", "response.in_progress", "response.output_item.added",
            "response.custom_tool_call_input.delta", "response.custom_tool_call_input.done", "response.output_item.done",
            "response.output_item.added", "response.function_call_arguments.delta", "response.function_call_arguments.done",
            "response.output_item.done", "response.completed"]);
        assert_eq!(events[2]["item"]["type"], "custom_tool_call");
        assert_eq!(events[5]["item"]["input"], "*** Begin Patch");
        assert_eq!((events[9]["item"]["name"].as_str(), events[9]["item"]["namespace"].as_str()), (Some("close_agent"), Some("multi_agent_v1")));
        assert_eq!(events[10]["response"]["status"], "completed", "a call that finished with stop is complete");
    }

    #[test]
    fn reasoning_without_a_summary_request_is_reasoning_text_and_a_break_is_a_failure() {
        let mut t = Stream::new(Context::new(&json!({ "model": "m" }), "m"));
        let mut events = t.push(&json!({ "choices": [{ "delta": { "reasoning": "Hm" } }] }));
        events.extend(t.push(&json!({ "choices": [{ "delta": { "content": "Hi" }, "finish_reason": "length" }] })));
        events.extend(t.finish());
        let parsed = parse(&events);
        assert!(kinds(&parsed).contains(&"response.reasoning_text.delta"));
        assert_eq!(parsed[2]["item"]["content"], json!([]));
        let last = parsed.last().unwrap();
        assert_eq!((last["type"].as_str(), last["response"]["incomplete_details"]["reason"].as_str()),
            (Some("response.incomplete"), Some("max_output_tokens")));

        let mut broken = Stream::new(Context::new(&json!({ "model": "m" }), "m"));
        let mut events = broken.push(&json!({ "choices": [{ "delta": { "content": "Hi" } }] }));
        events.extend(broken.fail("The provider's stream broke off"));
        let parsed = parse(&events);
        let last = parsed.last().unwrap();
        assert_eq!((last["type"].as_str(), last["response"]["status"].as_str()), (Some("response.failed"), Some("failed")));
        assert_eq!(last["response"]["error"]["message"], "The provider's stream broke off");
    }

    #[test]
    fn an_anthropic_stream_crosses_both_steps() {
        let mut to_chat = openai_to_anthropic::ChunkTranslator::new("m", true);
        let mut t = Stream::new(Context::new(&codex_request(), "m"));
        let anthropic = [
            json!({ "type": "message_start", "message": { "usage": { "input_tokens": 12, "output_tokens": 1 } } }),
            json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "tool_use", "id": "toolu_1", "name": "apply_patch", "input": {} } }),
            json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "input_json_delta", "partial_json": "{\"input\":\"x\"}" } }),
            json!({ "type": "message_delta", "delta": { "stop_reason": "tool_use" }, "usage": { "output_tokens": 7 } }),
        ];
        let mut events: Vec<String> = anthropic.iter().flat_map(|e| chunks(to_chat.push(e))).flat_map(|c| t.push(&c)).collect();
        events.extend(chunks(to_chat.finish()).iter().flat_map(|c| t.push(c)));
        events.extend(t.finish());
        let parsed = parse(&events);
        let done = &parsed.last().unwrap()["response"];
        assert_eq!(done["output"][0], json!({ "id": done["output"][0]["id"], "type": "custom_tool_call", "status": "completed",
            "call_id": "toolu_1", "name": "apply_patch", "input": "x" }));
        assert_eq!((done["usage"]["input_tokens"].as_u64(), done["usage"]["output_tokens"].as_u64()), (Some(12), Some(7)));
    }
}
