//! `pygg ask` and `pygg chat` (owner's request, 2026-11-18): a question, or a conversation, with any model Pygg's
//! gateway serves, from the terminal. A web session such as Muse answers like any connection, with the same routing,
//! fallbacks, guard and ledger as any client. The options follow the AI command-line tools people know (`llm`,
//! `aichat`, `claude -p`): `-m`, `-s`, `-t`, `--max-tokens`, `-f`, `--no-stream`, `-r`, `--json`, `-x`.
//!
//! Nothing of a conversation is kept: `chat` holds its messages in memory and forgets them when it ends, and the app
//! has no conversation store (ADR-028). Each request goes to the ledger like any other. Both commands use a Pygg API
//! key of their own, "Pygg CLI (ask)", made at first use and kept in the OS credential store like the Claude Code
//! launcher's. A chat is one routing session, so it stays on the model that first answered (and on its prompt cache)
//! until `/model` changes it. With no `--model`, the model set by `pygg default` is used, else `auto`.

use std::io::{BufRead, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::gateway::store;

const OPTIONS_HELP: &str =
    "  -m, --model M        connection/model (pygg models lists them), a combo, auto or auto/cheap;
                       default: the model set with pygg default, else auto
  -s, --system TEXT    a system prompt (instructions the model reads first)
  -t, --temperature T  0 to 2: lower is steadier, higher more varied
  --max-tokens N       the longest answer, in tokens
  -f, --file PATH      add a text file after the question (repeat for more; up to 200 KB each)
  --no-stream          print the answer once it is complete
  --no-input           never read stdin (for scripts whose stdin stays open)
  --save-dir DIR       where pictures, videos and files in an answer are saved (the current folder by default)
  --no-save            print their addresses instead of saving them
  -r, --raw            no line about the model and cost after the answer";

pub fn ask_help() -> String {
    format!(
        "usage: pygg ask [options] QUESTION…
       echo QUESTION | pygg ask [options]

One question to a model through Pygg's gateway; the answer is printed. Any connection Pygg serves can answer, a web
session such as Muse included (its guard still paces it: short questions, a few at a time).

{OPTIONS_HELP}
  --json               one JSON document: the answer, the model that answered, the time and the cost
  -x, --extract        print only the answer's first code block

The request goes to the ledger like any other (pygg logs, pygg usage); the question and answer are not kept. The first
time, Pygg makes an API key of its own for this command, \"Pygg CLI (ask)\", kept in the system's credential store.

Examples:
  pygg ask -m muse/muse \"What is a prompt cache, in two sentences?\"
  pygg ask -m auto/cheap -f src/main.rs \"What does this file do?\"
  git diff | pygg ask -s \"You review code briefly\" \"Anything risky here?\""
    )
}

pub fn chat_help() -> String {
    format!(
        "usage: pygg chat [options]

A conversation with a model through Pygg's gateway, in the terminal. Each answer is printed as it arrives, then a line
with the model that answered, the time and the cost the ledger knows.

{OPTIONS_HELP}

In the chat:
  /model M                change the model for the next messages (the conversation goes on; the new model reads it
                          without its cache)
  /system TEXT            set the system prompt (/system alone removes it)
  /set temperature T      also: /set max-tokens N
  /file PATH              add a text file to your next message
  /new                    start a new conversation
  /cost                   what this chat has cost so far
  /help                   these commands
  /exit                   leave (also Ctrl+C, or Ctrl+Z then Enter on Windows, Ctrl+D elsewhere)

The conversation is held in memory only and forgotten when the chat ends; every request goes to the ledger like any
other. The whole conversation is sent again with each message, so long chats cost more per message, and a web session
such as Muse refuses prompts over its size limit."
    )
}

const SETTING: &str = "cli.ask";
const CLIENT_NAME: &str = "Pygg CLI (ask)";
const FILE_LIMIT: u64 = 200 * 1024;

#[derive(Serialize, Deserialize)]
struct Record {
    client_id: String,
    vault_ref: String,
}

/// What a request carries besides the conversation.
#[derive(Clone, Debug, Default, PartialEq)]
struct Settings {
    system: Option<String>,
    temperature: Option<f64>,
    max_tokens: Option<u64>,
    stream: bool,
}

/// Pygg's options before the question; the rest are words.
#[derive(Debug, Default)]
struct Options {
    model: Option<String>,
    db: Option<PathBuf>,
    words: Vec<String>,
    settings: Settings,
    files: Vec<PathBuf>,
    raw: bool,
    json: bool,
    extract: bool,
    no_input: bool,
    no_save: bool,
    save_dir: Option<PathBuf>,
}

/// Parses the options; `Err(code)` when the command should stop with that exit code (help, a usage error).
fn parse(command: &str, args: Vec<String>, help: &str, ask: bool) -> Result<Options, i32> {
    let mut o = Options { settings: Settings { stream: true, ..Default::default() }, ..Default::default() };
    let usage = |message: String| -> i32 {
        eprintln!("pygg: {message}\nUse: pygg {command} --help");
        2
    };
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        // Options come before the question; anything after its first word is the question's.
        if !o.words.is_empty() || !a.starts_with('-') || a == "-" {
            o.words.push(a);
            continue;
        }
        let mut value = |name: &str| it.next().ok_or_else(|| usage(format!("{name} needs a value.")));
        match a.as_str() {
            "-h" | "--help" => {
                println!("{help}");
                return Err(0);
            }
            "-m" | "--model" => o.model = Some(value(&a)?),
            "--db" => o.db = Some(PathBuf::from(value(&a)?)),
            "-s" | "--system" => o.settings.system = Some(value(&a)?),
            "-t" | "--temperature" => {
                let v = value(&a)?;
                o.settings.temperature = Some(
                    v.parse::<f64>()
                        .ok()
                        .filter(|t| (0.0..=2.0).contains(t))
                        .ok_or_else(|| usage(format!("{a} takes a number from 0 to 2, not {v}.")))?,
                );
            }
            "--max-tokens" => {
                let v = value(&a)?;
                o.settings.max_tokens = Some(
                    v.parse::<u64>()
                        .ok()
                        .filter(|n| *n > 0)
                        .ok_or_else(|| usage(format!("--max-tokens takes a whole number above 0, not {v}.")))?,
                );
            }
            "-f" | "--file" => o.files.push(PathBuf::from(value(&a)?)),
            "--no-stream" => o.settings.stream = false,
            "--no-input" => o.no_input = true,
            "-r" | "--raw" => o.raw = true,
            "--no-save" => o.no_save = true,
            "--save-dir" => o.save_dir = Some(PathBuf::from(value(&a)?)),
            "--json" if ask => o.json = true,
            "-x" | "--extract" if ask => o.extract = true,
            _ => return Err(usage(format!("unknown option {a} for pygg {command}."))),
        }
    }
    Ok(o)
}

/// A text file's contents, as it is added to a message.
fn file_block(path: &Path) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.len() > FILE_LIMIT {
        return Err(format!("{} is {} KB; pygg adds files of up to 200 KB", path.display(), meta.len() / 1024));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = String::from_utf8(bytes).map_err(|_| format!("{} is not a text file", path.display()))?;
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.display().to_string());
    Ok(format!("<file name=\"{name}\">\n{}\n</file>", text.trim_end()))
}

/// The user's message: the text, then each file.
fn with_files(text: &str, files: &[PathBuf]) -> Result<String, String> {
    let mut message = text.trim().to_string();
    for f in files {
        message.push_str("\n\n");
        message.push_str(&file_block(f)?);
    }
    Ok(message)
}

/// The first fenced code block of an answer, without its fences; `None` when it has none.
fn first_code_block(text: &str) -> Option<String> {
    let start = text.find("```")?;
    let after = &text[start + 3..];
    let body_start = after.find('\n')? + 1;
    let body = &after[body_start..];
    let end = body.find("```")?;
    Some(body[..end].trim_end_matches(['\n', '\r']).to_string())
}

/// What both commands need: the store, the key, and a gateway that answers.
struct Gateway {
    store: PathBuf,
    token: String,
    port: u16,
    default_model: String,
    http: reqwest::Client,
}

async fn connect(db: Option<PathBuf>) -> Result<Gateway, String> {
    let store = db
        .or_else(super::default_store)
        .filter(|p| p.is_file())
        .ok_or("Pygg's store was not found: open Pygg once, or pass --db PATH")?;
    let (token, port, default_model) = prepare(&store)?;
    super::health(port)
        .await
        .map_err(|e| format!("Pygg's gateway on http://127.0.0.1:{port}: {e}. Next: {}.", super::START_GATEWAY))?;
    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(15 * 60))
        .build()
        .map_err(|e| e.to_string())?;
    Ok(Gateway { store, token, port, default_model, http })
}

/// An answer as it came back.
struct Answer {
    text: String,
    route: Option<String>,
    attempts: Option<u32>,
}

/// The request body: the conversation after the system prompt, and the settings that are set.
fn body(model: &str, messages: &[Value], settings: &Settings) -> Value {
    let mut all = Vec::with_capacity(messages.len() + 1);
    if let Some(system) = settings.system.as_deref().filter(|s| !s.trim().is_empty()) {
        all.push(json!({ "role": "system", "content": system }));
    }
    all.extend(messages.iter().cloned());
    let mut body = json!({ "model": model, "stream": settings.stream, "messages": all });
    if let Some(t) = settings.temperature {
        body["temperature"] = json!(t);
    }
    if let Some(n) = settings.max_tokens {
        body["max_tokens"] = json!(n);
    }
    body
}

/// Sends the conversation and writes the answer's text to `out` as it arrives.
async fn send(
    gw: &Gateway, session: &str, model: &str, messages: &[Value], settings: &Settings, out: &mut dyn Write,
) -> Result<Answer, String> {
    let response = gw
        .http
        .post(format!("http://127.0.0.1:{}/v1/chat/completions", gw.port))
        .bearer_auth(&gw.token)
        .header(crate::gateway::SESSION_HEADER, session)
        .json(&body(model, messages, settings))
        .send()
        .await
        .map_err(|e| format!("the gateway did not answer: {e}"))?;
    let status = response.status();
    let header = |name: &str| response.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string);
    let route = header("x-pygg-route");
    let attempts = header("x-pygg-attempts").and_then(|a| a.parse().ok());
    let streamed = header("content-type").is_some_and(|c| c.starts_with("text/event-stream"));
    if !status.is_success() {
        let value: Value = response.json().await.unwrap_or(Value::Null);
        let message = value["error"]["message"].as_str().unwrap_or("no message");
        return Err(format!("{model} did not answer (HTTP {}): {message}", status.as_u16()));
    }
    let mut text = String::new();
    if !streamed {
        let value: Value = response.json().await.map_err(|e| e.to_string())?;
        text = answer_text(&value);
        let _ = write!(out, "{}", &text[..media_start(&text).unwrap_or(text.len())]);
        return Ok(Answer { text, route, attempts });
    }
    // The pictures, videos and files after the text are saved, not printed (a picture's data URL is long).
    let mut hiding = false;
    let mut buffer = String::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("the answer was cut off: {e}"))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(end) = buffer.find('\n') {
            let line = buffer[..end].trim_end_matches('\r').to_string();
            buffer.drain(..=end);
            match sse_delta(&line) {
                Delta::Text(t) => {
                    let before = text.len();
                    text.push_str(&t);
                    if !hiding {
                        // Where the links start may fall inside this piece, or across the last two.
                        let mut from = before.saturating_sub(16);
                        while !text.is_char_boundary(from) {
                            from -= 1;
                        }
                        let shown = match media_start(&text[from..]) {
                            Some(i) => {
                                hiding = true;
                                &text[before..(from + i).max(before)]
                            }
                            None => &text[before..],
                        };
                        let _ = write!(out, "{shown}");
                        let _ = out.flush();
                    }
                }
                Delta::Error(e) => return Err(e),
                Delta::None => {}
            }
        }
    }
    Ok(Answer { text, route, attempts })
}

enum Delta {
    Text(String),
    Error(String),
    None,
}

/// One line of a Chat Completions stream: a piece of the answer, an error event, or nothing to show.
fn sse_delta(line: &str) -> Delta {
    let Some(data) = line.strip_prefix("data:").map(str::trim) else { return Delta::None };
    if data.is_empty() || data == "[DONE]" {
        return Delta::None;
    }
    let Ok(value) = serde_json::from_str::<Value>(data) else { return Delta::None };
    if let Some(message) = value["error"]["message"].as_str() {
        return Delta::Error(message.to_string());
    }
    match value["choices"][0]["delta"]["content"].as_str() {
        Some(t) if !t.is_empty() => Delta::Text(t.to_string()),
        _ => Delta::None,
    }
}

/// The answer's text: a Chat Completions message, its parts joined when it came as a list.
fn answer_text(value: &Value) -> String {
    let content = &value["choices"][0]["message"]["content"];
    match content {
        Value::String(s) => s.trim().to_string(),
        Value::Array(parts) => {
            parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("").trim().to_string()
        }
        _ => String::new(),
    }
}

fn session_cost(store: &Path, sessions: &[String]) -> Option<crate::launches::SessionCost> {
    let conn = Connection::open_with_flags(store, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    conn.busy_timeout(Duration::from_secs(2)).ok()?;
    let mut total = crate::launches::SessionCost::default();
    for s in sessions {
        let c = crate::launches::session_cost(&conn, s).ok()?;
        total.answered += c.answered;
        total.failed += c.failed;
        total.known_micro_usd += c.known_micro_usd;
        total.unknown_cost += c.unknown_cost;
        total.subscription += c.subscription;
        total.prompt_tokens += c.prompt_tokens;
        total.output_tokens += c.output_tokens;
    }
    Some(total)
}

fn cost_text(cost: Option<&crate::launches::SessionCost>) -> String {
    match cost {
        Some(c) if c.subscription > 0 => "on a plan, no price per request".into(),
        Some(c) if c.unknown_cost > 0 => "cost unknown (no price for this model)".into(),
        Some(c) => super::read::money(c.known_micro_usd),
        None => "cost not read".into(),
    }
}

fn footer(route: &str, attempts: Option<u32>, took: Duration, cost: Option<&crate::launches::SessionCost>) -> String {
    let mut parts = vec![format!("{route} via Pygg"), format!("{:.1} s", took.as_secs_f64())];
    if let Some(n) = attempts.filter(|n| *n > 1) {
        parts.push(format!("answered on attempt {n}"));
    }
    parts.push(cost_text(cost));
    format!("— {}", parts.join(" · "))
}

fn new_session(kind: &str) -> String {
    format!("{kind}-{}", uuid::Uuid::new_v4().simple())
}

/// `pygg ask …`
pub async fn ask(args: Vec<String>) -> i32 {
    let o = match parse("ask", args, &ask_help(), true) {
        Ok(o) => o,
        Err(code) => return code,
    };
    let mut question = o.words.join(" ");
    let piped = !o.no_input && !std::io::stdin().is_terminal();
    if (question.trim().is_empty() || question.trim() == "-") && piped {
        question.clear();
        let _ = std::io::stdin().read_to_string(&mut question);
    } else if piped && o.words.first().is_some_and(|w| w != "-") {
        // A question and a pipe: the piped text follows the question (git diff | pygg ask "anything risky?").
        let mut piped = String::new();
        let _ = std::io::stdin().read_to_string(&mut piped);
        if !piped.trim().is_empty() {
            question = format!("{}\n\n{}", question.trim(), piped.trim_end());
        }
    }
    if question.trim().is_empty() && o.files.is_empty() {
        eprintln!("pygg: ask a question.\nUse: pygg ask -m muse/muse \"your question\", or pygg chat");
        return 2;
    }
    let message = match with_files(&question, &o.files) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("pygg: {e}");
            return 2;
        }
    };
    let gw = match connect(o.db).await {
        Ok(g) => g,
        Err(e) => {
            eprintln!("pygg: {e}");
            return 1;
        }
    };
    let model = o.model.unwrap_or_else(|| gw.default_model.clone());
    let session = new_session("ask");
    let started = Instant::now();
    let messages = [json!({ "role": "user", "content": message })];
    // --json and --extract print once the answer is whole.
    let quiet = o.json || o.extract;
    let mut sink = std::io::sink();
    let mut stdout = std::io::stdout();
    let out: &mut dyn Write = if quiet { &mut sink } else { &mut stdout };
    let result = send(&gw, &session, &model, &messages, &o.settings, out).await;
    let answer = match result {
        Ok(a) => a,
        Err(e) => {
            if o.json {
                println!(
                    "{}",
                    json!({ "schemaVersion": super::read::JSON_SCHEMA, "ok": false, "model": model, "error": e })
                );
            } else {
                eprintln!("pygg: {e}");
            }
            return 1;
        }
    };
    let took = started.elapsed();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let cost = session_cost(&gw.store, std::slice::from_ref(&session));
    let route = answer.route.clone().unwrap_or_else(|| model.clone());
    if o.json {
        let c = cost.clone().unwrap_or_default();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schemaVersion": super::read::JSON_SCHEMA, "ok": true, "model": route, "asked": model,
                "answer": answer.text, "attempts": answer.attempts, "seconds": took.as_secs_f64(),
                "inputTokens": c.prompt_tokens, "outputTokens": c.output_tokens,
                "knownMicroUsd": c.known_micro_usd, "costKnown": c.unknown_cost == 0 && c.subscription == 0,
            }))
            .unwrap_or_default()
        );
        return 0;
    }
    if o.extract {
        match first_code_block(&answer.text) {
            Some(code) => println!("{code}"),
            None => {
                eprintln!("pygg: the answer has no code block; here it is whole.");
                println!("{}", answer.text);
            }
        }
    } else {
        println!();
    }
    if !o.raw {
        eprintln!("\n{}", footer(&route, answer.attempts, took, cost.as_ref()));
    }
    keep_media(&gw.http, &answer.text, o.no_save, o.save_dir.as_deref()).await;
    if answer.text.is_empty() {
        eprintln!("pygg: the answer was empty.");
        return 1;
    }
    0
}

/// `pygg chat …`
pub async fn chat(args: Vec<String>) -> i32 {
    let o = match parse("chat", args, &chat_help(), false) {
        Ok(o) => o,
        Err(code) => return code,
    };
    if !o.words.is_empty() {
        eprintln!("pygg chat takes options only; type your messages once it starts.\nUse: pygg chat -m muse/muse");
        return 2;
    }
    let gw = match connect(o.db).await {
        Ok(g) => g,
        Err(e) => {
            eprintln!("pygg: {e}");
            return 1;
        }
    };
    let mut model = o.model.unwrap_or_else(|| gw.default_model.clone());
    let mut settings = o.settings;
    let mut files = o.files;
    let mut session = new_session("chat");
    let mut sessions = vec![session.clone()];
    let mut messages: Vec<Value> = Vec::new();
    eprintln!("Pygg chat with {model}. /help lists the commands; /exit leaves. Nothing is kept.\n");
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    loop {
        eprint!("you › ");
        let _ = std::io::stderr().flush();
        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match command(line) {
            Some(Command::Exit) => break,
            Some(Command::Help) => {
                eprintln!("/model M · /system TEXT · /set temperature T · /set max-tokens N · /file PATH · /new · /cost · /exit\n");
                continue;
            }
            Some(Command::New) => {
                messages.clear();
                session = new_session("chat");
                sessions.push(session.clone());
                eprintln!("A new conversation with {model}.\n");
                continue;
            }
            Some(Command::Cost) => {
                tokio::time::sleep(Duration::from_millis(200)).await;
                let cost = session_cost(&gw.store, &sessions);
                let answered = cost.as_ref().map_or(0, |c| c.answered);
                eprintln!("This chat: {answered} answers, {}.\n", cost_text(cost.as_ref()));
                continue;
            }
            Some(Command::Model(m)) => {
                model = m;
                // A new routing session, so the next answer is not held to the model the last one pinned.
                session = new_session("chat");
                sessions.push(session.clone());
                eprintln!("Next messages go to {model}; the conversation so far goes with them.\n");
                continue;
            }
            Some(Command::System(s)) => {
                eprintln!("{}\n", if s.is_some() { "System prompt set." } else { "System prompt removed." });
                settings.system = s;
                continue;
            }
            Some(Command::Set(name, value)) => {
                match set(&mut settings, &name, &value) {
                    Ok(text) => eprintln!("{text}\n"),
                    Err(e) => eprintln!("{e}\n"),
                }
                continue;
            }
            Some(Command::File(path)) => {
                match file_block(Path::new(&path)) {
                    Ok(_) => {
                        files.push(PathBuf::from(&path));
                        eprintln!("{path} goes with your next message.\n");
                    }
                    Err(e) => eprintln!("{e}\n"),
                }
                continue;
            }
            Some(Command::Unknown(c)) => {
                eprintln!("{c} is not a chat command; /help lists them.\n");
                continue;
            }
            None => {}
        }
        let message = match with_files(line, &files) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("{e}\n");
                continue;
            }
        };
        files.clear();
        messages.push(json!({ "role": "user", "content": message }));
        let started = Instant::now();
        println!();
        match send(&gw, &session, &model, &messages, &settings, &mut stdout).await {
            Ok(answer) => {
                println!();
                if !o.raw {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    let last = last_cost(&gw.store, &session);
                    let route = answer.route.as_deref().unwrap_or(&model);
                    eprintln!("{}", footer(route, answer.attempts, started.elapsed(), last.as_ref()));
                }
                keep_media(&gw.http, &answer.text, o.no_save, o.save_dir.as_deref()).await;
                eprintln!();
                messages.push(json!({ "role": "assistant", "content": answer.text }));
            }
            Err(e) => {
                // The message stays out of the conversation, so it can be sent again or changed.
                messages.pop();
                eprintln!("\npygg: {e}\n");
            }
        }
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    let cost = session_cost(&gw.store, &sessions);
    let answered = cost.as_ref().map_or(0, |c| c.answered);
    eprintln!("\nChat ended: {answered} answers, {}. Nothing of it was kept.", cost_text(cost.as_ref()));
    0
}

// ---------- Pictures, videos and files in an answer ----------

/// Where the pictures, videos and files a web session adds after its text begin (`websession::media_note`).
fn media_start(text: &str) -> Option<usize> {
    ["\n\n![image ", "\n\n[video ", "\n\n[file "].iter().filter_map(|m| text.find(m)).min()
}

/// The pictures, videos and files a web session put in an answer (`websession::media_note`): `![image 1](URL)`,
/// `[video 1](URL)`, `[file 1](URL)`, as (label, URL).
fn media_links(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find("](") else { break };
        let label = &rest[..close];
        let after = &rest[close + 2..];
        let Some(end) = after.find(')') else { break };
        let url = &after[..end];
        let kind = label.split(' ').next().unwrap_or("");
        let numbered =
            label.split(' ').nth(1).is_some_and(|n| n.parse::<u32>().is_ok()) && label.split(' ').count() == 2;
        if numbered
            && matches!(kind, "image" | "video" | "file")
            // A picture or video copied out of the page comes as a data URL (Muse's videos: `data:video/mp4`).
            && (url.starts_with("https://") || url.starts_with("data:"))
        {
            out.push((label.to_string(), url.to_string()));
            rest = &after[end + 1..];
        }
    }
    out
}

/// A file name's extension from the address, else from the type the server gave, else `bin`.
fn extension(url: &str, content_type: Option<&str>) -> String {
    let path = url.split(['?', '#']).next().unwrap_or("");
    if let Some(ext) =
        path.rsplit('/').next().and_then(|name| name.rsplit_once('.')).map(|(_, e)| e.to_ascii_lowercase())
    {
        if (1..=5).contains(&ext.len()) && ext.chars().all(|c| c.is_ascii_alphanumeric()) {
            return ext;
        }
    }
    let mime = content_type.unwrap_or("").split(';').next().unwrap_or("").trim();
    match mime {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/quicktime" => "mov",
        "audio/mpeg" => "mp3",
        "application/pdf" => "pdf",
        "application/zip" => "zip",
        "text/plain" => "txt",
        _ => "bin",
    }
    .to_string()
}

/// Standard base64 (a picture's data URL).
fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    };
    let clean: Vec<u8> = text.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=').collect();
    let mut out = Vec::with_capacity(clean.len() * 3 / 4);
    for chunk in clean.chunks(4) {
        let mut n = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            n |= value(*c)? << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Some(out)
}

/// The most one picture, video or file may be, so a strange answer cannot fill the disk.
const MEDIA_LIMIT: u64 = 500 * 1024 * 1024;

/// Saves one picture, video or file into `dir`; the path it went to.
async fn save_one(http: &reqwest::Client, label: &str, url: &str, dir: &Path) -> Result<PathBuf, String> {
    let stem = format!("pygg-{}-{}", label.replace(' ', ""), &uuid::Uuid::new_v4().simple().to_string()[..6]);
    if let Some(data) = url.strip_prefix("data:") {
        let (head, body) = data.split_once(',').ok_or("a data address with no data")?;
        let bytes = if head.ends_with(";base64") {
            base64_decode(body).ok_or("data that is not base64")?
        } else {
            body.as_bytes().to_vec()
        };
        let mime = head.split(';').next();
        // A video copied from the page may carry no type: it is saved as MP4, a picture as PNG.
        let ext = match extension("", mime).as_str() {
            "bin" if label.starts_with("video") => "mp4".to_string(),
            "bin" if label.starts_with("image") => "png".to_string(),
            ext => ext.to_string(),
        };
        let path = dir.join(format!("{stem}.{ext}"));
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        return Ok(path);
    }
    let response = http.get(url).timeout(Duration::from_secs(300)).send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("the site answered HTTP {}", response.status().as_u16()));
    }
    if response.content_length().is_some_and(|n| n > MEDIA_LIMIT) {
        return Err("it is larger than 500 MB".into());
    }
    let content_type = response.headers().get("content-type").and_then(|v| v.to_str().ok()).map(str::to_string);
    let path = dir.join(format!("{stem}.{}", extension(url, content_type.as_deref())));
    let mut file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    let mut stream = response.bytes_stream();
    let mut written = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        written += chunk.len() as u64;
        if written > MEDIA_LIMIT {
            drop(file);
            let _ = std::fs::remove_file(&path);
            return Err("it is larger than 500 MB".into());
        }
        file.write_all(&chunk).map_err(|e| e.to_string())?;
    }
    Ok(path)
}

/// Saves the pictures, videos and files in an answer (or lists them with --no-save), saying where each went.
async fn keep_media(http: &reqwest::Client, text: &str, no_save: bool, save_dir: Option<&Path>) {
    let links = media_links(text);
    if links.is_empty() {
        return;
    }
    if no_save {
        for (label, url) in &links {
            if !url.starts_with("data:") {
                eprintln!("{label}: {url}");
            }
        }
        return;
    }
    let dir = save_dir.map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("pygg: could not use {}: {e}", dir.display());
        return;
    }
    for (label, url) in &links {
        match save_one(http, label, url, &dir).await {
            Ok(path) => {
                let shown = std::fs::canonicalize(&path).unwrap_or(path);
                let shown = shown.display().to_string();
                eprintln!("Saved {label}: {}", shown.trim_start_matches(r"\\?\"));
            }
            Err(e) => eprintln!(
                "{label} not saved ({e}): {}",
                if url.starts_with("data:") { "(a data address)" } else { url }
            ),
        }
    }
}

/// `/set NAME VALUE` in a chat.
fn set(settings: &mut Settings, name: &str, value: &str) -> Result<String, String> {
    match name {
        "temperature" => {
            let t = value.parse::<f64>().ok().filter(|t| (0.0..=2.0).contains(t)).ok_or("temperature takes 0 to 2")?;
            settings.temperature = Some(t);
            Ok(format!("Temperature {t}."))
        }
        "max-tokens" | "max_tokens" => {
            let n = value.parse::<u64>().ok().filter(|n| *n > 0).ok_or("max-tokens takes a whole number above 0")?;
            settings.max_tokens = Some(n);
            Ok(format!("Answers of at most {n} tokens."))
        }
        _ => Err(format!("{name} cannot be set here: /set temperature T or /set max-tokens N")),
    }
}

/// The cost of a session's latest answer.
fn last_cost(store: &Path, session: &str) -> Option<crate::launches::SessionCost> {
    let conn = Connection::open_with_flags(store, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    conn.busy_timeout(Duration::from_secs(2)).ok()?;
    let (cost, status, billing): (Option<i64>, String, Option<String>) = conn
        .query_row(
            "SELECT cost_micro_usd, cost_status, billing FROM usage_events WHERE session_key = ?1 AND created_at >= 0 \
             AND outcome = 'ok' ORDER BY created_at DESC LIMIT 1",
            [session],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok()?;
    let subscription = billing.as_deref() == Some("subscription");
    Some(crate::launches::SessionCost {
        answered: 1,
        subscription: u64::from(subscription),
        unknown_cost: u64::from(status == "unknown" && !subscription),
        known_micro_usd: if status == "unknown" { 0 } else { cost.unwrap_or(0).max(0) as u64 },
        ..Default::default()
    })
}

#[derive(Debug, PartialEq)]
enum Command {
    Exit,
    Help,
    New,
    Cost,
    Model(String),
    System(Option<String>),
    Set(String, String),
    File(String),
    Unknown(String),
}

/// A chat command, or `None` for a message (a line that does not start with `/`).
fn command(line: &str) -> Option<Command> {
    if !line.starts_with('/') {
        return None;
    }
    let mut parts = line.splitn(2, char::is_whitespace);
    let name = parts.next().unwrap_or("");
    let rest = parts.next().map(str::trim).unwrap_or("");
    Some(match name {
        "/exit" | "/quit" => Command::Exit,
        "/help" | "/?" => Command::Help,
        "/new" | "/clear" => Command::New,
        "/cost" => Command::Cost,
        "/model" if !rest.is_empty() => Command::Model(rest.to_string()),
        "/system" => Command::System((!rest.is_empty()).then(|| rest.to_string())),
        "/set" => {
            let mut kv = rest.splitn(2, char::is_whitespace);
            match (kv.next(), kv.next().map(str::trim)) {
                (Some(k), Some(v)) if !k.is_empty() && !v.is_empty() => Command::Set(k.to_string(), v.to_string()),
                _ => Command::Unknown("/set".into()),
            }
        }
        "/file" if !rest.is_empty() => Command::File(rest.trim_matches('"').to_string()),
        other => Command::Unknown(other.to_string()),
    })
}

/// The command's key (made at first use), the gateway's port and the default model.
fn prepare(path: &Path) -> Result<(String, u16, String), String> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE).map_err(|e| e.to_string())?;
    conn.busy_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    let schema: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    if schema < crate::db::SCHEMA_VERSION {
        return Err(format!(
            "the store is at schema v{schema} and this program needs v{}: open the updated Pygg once",
            crate::db::SCHEMA_VERSION
        ));
    }
    let port = store::setting(&conn, crate::gateway::PORT_KEY)
        .ok()
        .flatten()
        .and_then(|p| p.parse().ok())
        .unwrap_or(crate::gateway::DEFAULT_PORT);
    Ok((token(&conn)?, port, super::manage::default_model(&conn)))
}

fn token(conn: &Connection) -> Result<String, String> {
    let record: Option<Record> = store::setting(conn, SETTING)
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
        .and_then(|s| serde_json::from_str(&s).ok());
    if let Some(r) = &record {
        if let Ok(Some(t)) = crate::vault::read(&r.vault_ref) {
            return Ok(t);
        }
        // The key went missing from the credential store: revoke the old one and make another.
        let _ = store::revoke_client(conn, &r.client_id);
    }
    let (client, token) = store::create_client(conn, CLIENT_NAME, None)?;
    let vault_ref = crate::vault::new_client_reference();
    if let Err(e) = crate::vault::store(&vault_ref, &token) {
        let _ = store::revoke_client(conn, &client.id);
        return Err(format!("the key could not be kept in the credential store: {e}"));
    }
    store::set_client_token_ref(conn, &client.id, Some(&vault_ref))?;
    let record = Record { client_id: client.id, vault_ref };
    store::set_setting(conn, SETTING, &serde_json::to_string(&record).map_err(|e| e.to_string())?)?;
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn options_come_before_the_question() {
        let o = parse(
            "ask",
            args(&[
                "-m",
                "muse/muse",
                "-s",
                "be brief",
                "-t",
                "0.2",
                "--max-tokens",
                "100",
                "-f",
                "a.rs",
                "--json",
                "what",
                "-x",
            ]),
            "",
            true,
        )
        .unwrap();
        assert_eq!(o.model.as_deref(), Some("muse/muse"));
        assert_eq!(o.settings.system.as_deref(), Some("be brief"));
        assert_eq!((o.settings.temperature, o.settings.max_tokens), (Some(0.2), Some(100)));
        assert_eq!(o.files, vec![PathBuf::from("a.rs")]);
        assert!(o.json && !o.extract, "-x after the question's first word is part of the question");
        assert_eq!(o.words, ["what", "-x"]);
        assert_eq!(parse("ask", args(&["-t", "3"]), "", true).unwrap_err(), 2);
        assert_eq!(parse("chat", args(&["--json"]), "", false).unwrap_err(), 2, "chat has no --json");
    }

    #[test]
    fn the_body_carries_only_what_is_set() {
        let m = [json!({"role": "user", "content": "hi"})];
        let b = body("auto", &m, &Settings { stream: true, ..Default::default() });
        assert_eq!(b["messages"].as_array().unwrap().len(), 1);
        assert!(b.get("temperature").is_none() && b.get("max_tokens").is_none());
        let s =
            Settings { system: Some("be brief".into()), temperature: Some(0.0), max_tokens: Some(9), stream: false };
        let b = body("auto", &m, &s);
        assert_eq!(b["messages"][0]["role"], "system");
        assert_eq!(
            (b["temperature"].as_f64(), b["max_tokens"].as_u64(), b["stream"].as_bool()),
            (Some(0.0), Some(9), Some(false))
        );
    }

    #[test]
    fn media_links_are_read_from_an_answer() {
        let text = "Here you go.\n\n![image 1](https://cdn.x/a.jpg?oh=1)\n\n[video 1](https://cdn.x/v.mp4)\n\n\
                    [file 1](https://cdn.x/r.pdf) and [a link](https://x.y) and [image 2](http://plain)";
        let links = media_links(text);
        let urls: Vec<&str> = links.iter().map(|(_, u)| u.as_str()).collect();
        assert_eq!(urls, ["https://cdn.x/a.jpg?oh=1", "https://cdn.x/v.mp4", "https://cdn.x/r.pdf"]);
        assert_eq!(links[1].0, "video 1");
        // Muse's video, copied out of the page (2026-11-18: it was taken, then skipped here as not a picture).
        let muse = media_links("I'll make it.\n\n[video 1](data:video/mp4;base64,AAAAIGZ0eXA=)");
        assert_eq!(muse, [("video 1".to_string(), "data:video/mp4;base64,AAAAIGZ0eXA=".to_string())]);
        assert_eq!(extension("https://cdn.x/a.jpg?oh=1", None), "jpg");
        assert_eq!(extension("https://cdn.x/render/abc", Some("video/mp4")), "mp4");
        assert_eq!(extension("https://cdn.x/x", None), "bin");
    }

    #[test]
    fn the_links_after_the_text_are_not_printed() {
        assert_eq!(media_start("Here you go.\n\n![image 1](data:image/png;base64,AAA)"), Some(12));
        assert_eq!(media_start("Here.\n\n[video 1](https://x/v.mp4)\n\n![image 1](https://x/a.jpg)"), Some(5));
        assert_eq!(media_start("plain [link](https://x)"), None);
    }

    #[test]
    fn a_data_video_is_saved_as_mp4() {
        let dir = std::env::temp_dir().join(format!("pygg-media-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let http = reqwest::Client::new();
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let typed = rt.block_on(save_one(&http, "video 1", "data:video/mp4;base64,aGVsbG8=", &dir)).unwrap();
        assert_eq!(typed.extension().unwrap(), "mp4");
        assert_eq!(std::fs::read(&typed).unwrap(), b"hello");
        let untyped = rt.block_on(save_one(&http, "video 2", "data:;base64,aGk=", &dir)).unwrap();
        assert_eq!(untyped.extension().unwrap(), "mp4");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn data_pictures_decode() {
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(base64_decode("aGk").unwrap(), b"hi");
        assert!(base64_decode("a$b").is_none());
    }

    #[test]
    fn the_first_code_block_is_found() {
        assert_eq!(first_code_block("Here:\n```rust\nfn a() {}\n```\nand ```x\ny\n```").as_deref(), Some("fn a() {}"));
        assert_eq!(first_code_block("no code"), None);
    }

    #[test]
    fn files_go_after_the_question() {
        let dir = std::env::temp_dir().join(format!("pygg-ask-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("note.txt");
        std::fs::write(&f, "xin chào\n").unwrap();
        let m = with_files("What is this?", std::slice::from_ref(&f)).unwrap();
        assert_eq!(m, "What is this?\n\n<file name=\"note.txt\">\nxin chào\n</file>");
        std::fs::write(&f, [0xff, 0xfe, 0x00]).unwrap();
        assert!(with_files("x", &[f]).unwrap_err().contains("not a text file"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_answer_is_read_from_text_or_parts() {
        assert_eq!(answer_text(&json!({"choices": [{"message": {"content": " 4 "}}]})), "4");
        assert_eq!(
            answer_text(
                &json!({"choices": [{"message": {"content": [{"type": "text", "text": "a"}, {"text": "b"}]}}]})
            ),
            "ab"
        );
        assert_eq!(answer_text(&json!({})), "");
    }

    #[test]
    fn stream_lines_give_text_errors_or_nothing() {
        assert!(
            matches!(sse_delta(r#"data: {"choices":[{"delta":{"content":"Xin"}}]}"#), Delta::Text(t) if t == "Xin")
        );
        assert!(matches!(sse_delta("data: [DONE]"), Delta::None));
        assert!(matches!(sse_delta(": keep-alive"), Delta::None));
        assert!(matches!(sse_delta(r#"data: {"choices":[{"delta":{"role":"assistant"}}]}"#), Delta::None));
        assert!(matches!(sse_delta(r#"data: {"error":{"message":"guard"}}"#), Delta::Error(m) if m == "guard"));
    }

    #[test]
    fn chat_commands() {
        assert_eq!(command("hello"), None);
        assert_eq!(command("/exit"), Some(Command::Exit));
        assert_eq!(command("/model  muse/muse "), Some(Command::Model("muse/muse".into())));
        assert_eq!(command("/model"), Some(Command::Unknown("/model".into())));
        assert_eq!(command("/system be brief"), Some(Command::System(Some("be brief".into()))));
        assert_eq!(command("/system"), Some(Command::System(None)));
        assert_eq!(command("/set temperature 0.3"), Some(Command::Set("temperature".into(), "0.3".into())));
        assert_eq!(command("/set"), Some(Command::Unknown("/set".into())));
        assert_eq!(command("/file \"a b.txt\""), Some(Command::File("a b.txt".into())));
        let mut s = Settings::default();
        assert!(set(&mut s, "temperature", "0.3").is_ok() && s.temperature == Some(0.3));
        assert!(set(&mut s, "temperature", "9").is_err());
        assert!(set(&mut s, "top_p", "1").is_err());
    }

    #[test]
    fn the_footer_says_what_is_known_of_the_cost() {
        let unknown = crate::launches::SessionCost { answered: 1, unknown_cost: 1, ..Default::default() };
        let line = footer("muse/muse", Some(1), Duration::from_millis(18_200), Some(&unknown));
        assert_eq!(line, "— muse/muse via Pygg · 18.2 s · cost unknown (no price for this model)");
        let paid = crate::launches::SessionCost { answered: 1, known_micro_usd: 1200, ..Default::default() };
        let line = footer("openrouter/x", Some(2), Duration::from_secs(2), Some(&paid));
        assert_eq!(line, "— openrouter/x via Pygg · 2.0 s · answered on attempt 2 · $0.0012");
    }
}
