//! `pygg ask` and `pygg chat` (owner's request, 2026-11-18): a question, or a conversation, with any model Pygg's
//! gateway serves, from the terminal. A web session such as Muse answers like any connection, with the same routing,
//! fallbacks, guard and ledger as any client.
//!
//! Nothing of a conversation is kept: `chat` holds its messages in memory and forgets them when it ends, and the app
//! has no conversation store (ADR-028). Each request goes to the ledger like any other. Both commands use a Pygg API
//! key of their own, "Pygg CLI (ask)", made at first use and kept in the OS credential store like the Claude Code
//! launcher's. A chat is one routing session, so it stays on the model that first answered (and on its prompt cache)
//! until `/model` changes it.

use std::io::{BufRead, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::gateway::store;

pub const ASK_HELP: &str = "usage: pygg ask [--model M] [--db PATH] QUESTION…
       echo QUESTION | pygg ask [--model M]

One question to a model through Pygg's gateway; the answer is printed. Any connection Pygg serves can answer, a web
session such as Muse included (its guard still paces it: short questions, a few at a time).

  --model M   connection/model (pygg models lists them), a combo, auto or auto/cheap; default auto

The request goes to the ledger like any other (pygg logs, pygg usage); the question and answer are not kept. The first
time, Pygg makes an API key of its own for this command, \"Pygg CLI (ask)\", kept in the system's credential store.

Examples:
  pygg ask --model muse/muse \"What is a prompt cache, in two sentences?\"
  pygg ask --model auto/cheap \"Name three uses of git worktree\"";

pub const CHAT_HELP: &str = "usage: pygg chat [--model M] [--db PATH]

A conversation with a model through Pygg's gateway, in the terminal. Each answer is printed as it arrives, then a line
with the model that answered, the time and the cost the ledger knows.

  --model M   connection/model (pygg models lists them), a combo, auto or auto/cheap; default auto

In the chat:
  /model M    change the model for the next messages (the conversation goes on; the new model reads it uncached)
  /new        start a new conversation
  /cost       what this chat has cost so far
  /exit       leave (also Ctrl+C, or Ctrl+Z then Enter on Windows, Ctrl+D elsewhere)

The conversation is held in memory only and forgotten when the chat ends; every request goes to the ledger like any
other. The whole conversation is sent again with each message, so long chats cost more per message, and a web session
such as Muse refuses prompts over its size limit.";

const SETTING: &str = "cli.ask";
const CLIENT_NAME: &str = "Pygg CLI (ask)";

#[derive(Serialize, Deserialize)]
struct Record {
    client_id: String,
    vault_ref: String,
}

/// Pygg's options before the question: `--model`, `--db`; the rest are words.
struct Options {
    model: String,
    db: Option<PathBuf>,
    words: Vec<String>,
}

fn parse(command: &str, args: Vec<String>, help: &str) -> Result<Options, i32> {
    let mut o = Options { model: "auto".into(), db: None, words: Vec::new() };
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" if o.words.is_empty() => {
                println!("{help}");
                return Err(0);
            }
            "--model" | "--db" if o.words.is_empty() => {
                let Some(v) = it.next() else {
                    eprintln!("pygg: {a} needs a value.\nUse: pygg {command} --help");
                    return Err(2);
                };
                if a == "--model" {
                    o.model = v;
                } else {
                    o.db = Some(PathBuf::from(v));
                }
            }
            _ if a.starts_with("--") && o.words.is_empty() => {
                eprintln!(
                    "pygg: unknown option {a} for pygg {command} (its options are --model and --db).\nUse: pygg {command} --help"
                );
                return Err(2);
            }
            _ => o.words.push(a),
        }
    }
    Ok(o)
}

/// What both commands need: the store, the key, and a gateway that answers.
struct Gateway {
    store: PathBuf,
    token: String,
    port: u16,
    http: reqwest::Client,
}

async fn connect(db: Option<PathBuf>) -> Result<Gateway, String> {
    let store = db
        .or_else(super::default_store)
        .filter(|p| p.is_file())
        .ok_or("Pygg's store was not found: open Pygg once, or pass --db PATH")?;
    let (token, port) = prepare(&store)?;
    super::health(port)
        .await
        .map_err(|e| format!("Pygg's gateway on http://127.0.0.1:{port}: {e}. Next: {}.", super::START_GATEWAY))?;
    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(15 * 60))
        .build()
        .map_err(|e| e.to_string())?;
    Ok(Gateway { store, token, port, http })
}

/// An answer as it came back.
struct Answer {
    text: String,
    route: Option<String>,
    attempts: Option<u32>,
}

/// Sends the conversation and streams the answer's text to `out` as it arrives.
async fn send(
    gw: &Gateway, session: &str, model: &str, messages: &[Value], out: &mut dyn Write,
) -> Result<Answer, String> {
    let body = json!({ "model": model, "stream": true, "messages": messages });
    let response = gw
        .http
        .post(format!("http://127.0.0.1:{}/v1/chat/completions", gw.port))
        .bearer_auth(&gw.token)
        .header(crate::gateway::SESSION_HEADER, session)
        .json(&body)
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
        let _ = write!(out, "{text}");
        return Ok(Answer { text, route, attempts });
    }
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
                    text.push_str(&t);
                    let _ = write!(out, "{t}");
                    let _ = out.flush();
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
    let o = match parse("ask", args, ASK_HELP) {
        Ok(o) => o,
        Err(code) => return code,
    };
    let mut question = o.words.join(" ");
    if question.trim().is_empty() && !std::io::stdin().is_terminal() {
        let _ = std::io::stdin().read_to_string(&mut question);
    }
    let question = question.trim().to_string();
    if question.is_empty() {
        eprintln!("pygg: ask a question.\nUse: pygg ask --model muse/muse \"your question\", or pygg chat");
        return 2;
    }
    let gw = match connect(o.db).await {
        Ok(g) => g,
        Err(e) => {
            eprintln!("pygg: {e}");
            return 1;
        }
    };
    let session = new_session("ask");
    let started = Instant::now();
    let messages = [json!({ "role": "user", "content": question })];
    let mut stdout = std::io::stdout();
    match send(&gw, &session, &o.model, &messages, &mut stdout).await {
        Ok(answer) => {
            println!();
            tokio::time::sleep(Duration::from_millis(300)).await;
            let cost = session_cost(&gw.store, std::slice::from_ref(&session));
            let route = answer.route.as_deref().unwrap_or(&o.model);
            eprintln!("\n{}", footer(route, answer.attempts, started.elapsed(), cost.as_ref()));
            if answer.text.is_empty() {
                eprintln!("pygg: the answer was empty.");
                return 1;
            }
            0
        }
        Err(e) => {
            eprintln!("pygg: {e}");
            1
        }
    }
}

/// `pygg chat …`
pub async fn chat(args: Vec<String>) -> i32 {
    let o = match parse("chat", args, CHAT_HELP) {
        Ok(o) => o,
        Err(code) => return code,
    };
    if !o.words.is_empty() {
        eprintln!("pygg chat takes options only; type your messages once it starts.\nUse: pygg chat --model muse/muse");
        return 2;
    }
    let gw = match connect(o.db).await {
        Ok(g) => g,
        Err(e) => {
            eprintln!("pygg: {e}");
            return 1;
        }
    };
    let mut model = o.model;
    let mut session = new_session("chat");
    let mut sessions = vec![session.clone()];
    let mut messages: Vec<Value> = Vec::new();
    eprintln!("Pygg chat with {model}. /model M changes it, /new starts over, /cost, /exit. Nothing is kept.\n");
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
            Some(Command::Unknown(c)) => {
                eprintln!("{c} is not a chat command: /model M, /new, /cost or /exit.\n");
                continue;
            }
            None => {}
        }
        messages.push(json!({ "role": "user", "content": line }));
        let started = Instant::now();
        println!();
        match send(&gw, &session, &model, &messages, &mut stdout).await {
            Ok(answer) => {
                println!();
                tokio::time::sleep(Duration::from_millis(200)).await;
                let last = last_cost(&gw.store, &session);
                let route = answer.route.as_deref().unwrap_or(&model);
                eprintln!("{}\n", footer(route, answer.attempts, started.elapsed(), last.as_ref()));
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
    New,
    Cost,
    Model(String),
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
        "/new" | "/clear" => Command::New,
        "/cost" => Command::Cost,
        "/model" if !rest.is_empty() => Command::Model(rest.to_string()),
        other => Command::Unknown(other.to_string()),
    })
}

/// The command's key (made at first use) and the gateway's port.
fn prepare(path: &Path) -> Result<(String, u16), String> {
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
    Ok((token(&conn)?, port))
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
        assert_eq!(command("/new"), Some(Command::New));
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
