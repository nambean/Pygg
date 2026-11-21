//! What a running agent is doing, line by line, for the task's screen (the run view): each attempt's output read as
//! it arrives, turned into readable lines (a tool call, a command, what the model said), redacted, and kept in memory
//! for the screen to poll. Bounded: the last `MAX_LINES` lines of an attempt, the last `MAX_ATTEMPTS` attempts.
//! Nothing here is stored; after a restart the attempt's saved output tail is what remains.

use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};

pub const MAX_LINES: usize = 500;
pub const MAX_ATTEMPTS: usize = 40;
/// A line is cut to this many characters.
pub const MAX_LINE_CHARS: usize = 400;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    /// Counts from 1 within the attempt, so the screen asks for what came after the last one it has.
    pub seq: u64,
    pub at_ms: i64,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    /// Whether this attempt has a live log here at all (it ran since the app started).
    pub known: bool,
    pub lines: Vec<Line>,
    /// Lines dropped from the front to stay under `MAX_LINES`.
    pub dropped: u64,
    /// The agent has exited.
    pub done: bool,
}

#[derive(Default)]
struct Log {
    lines: VecDeque<Line>,
    next: u64,
    dropped: u64,
    done: bool,
    touched: i64,
}

fn logs() -> &'static Mutex<HashMap<String, Log>> {
    static LOGS: OnceLock<Mutex<HashMap<String, Log>>> = OnceLock::new();
    LOGS.get_or_init(Default::default)
}

/// Adds a line to an attempt's log. Empty text is ignored.
pub fn push(attempt: &str, text: &str) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    let text: String = if text.chars().count() > MAX_LINE_CHARS {
        text.chars().take(MAX_LINE_CHARS).chain("…".chars()).collect()
    } else {
        text.to_string()
    };
    let now = crate::db::now_ms();
    let mut all = logs().lock().unwrap_or_else(|p| p.into_inner());
    if !all.contains_key(attempt) && all.len() >= MAX_ATTEMPTS {
        // The least recently touched goes, finished ones first.
        if let Some(oldest) = all.iter().min_by_key(|(_, l)| (!l.done, l.touched)).map(|(id, _)| id.clone()) {
            all.remove(&oldest);
        }
    }
    let log = all.entry(attempt.to_string()).or_default();
    log.next += 1;
    log.lines.push_back(Line { seq: log.next, at_ms: now, text });
    if log.lines.len() > MAX_LINES {
        log.lines.pop_front();
        log.dropped += 1;
    }
    log.touched = now;
}

/// Marks an attempt's agent as exited.
pub fn finish(attempt: &str) {
    let mut all = logs().lock().unwrap_or_else(|p| p.into_inner());
    if let Some(log) = all.get_mut(attempt) {
        log.done = true;
        log.touched = crate::db::now_ms();
    }
}

/// An attempt's lines after `after` (0 for all kept).
pub fn since(attempt: &str, after: u64) -> Activity {
    let all = logs().lock().unwrap_or_else(|p| p.into_inner());
    match all.get(attempt) {
        None => Activity::default(),
        Some(log) => Activity {
            known: true,
            lines: log.lines.iter().filter(|l| l.seq > after).cloned().collect(),
            dropped: log.dropped,
            done: log.done,
        },
    }
}

/// Whether an agent writes its progress as it goes, one event per line (OpenCode and Codex do; Claude Code and Gemini
/// CLI, as Pygg starts them, write one document when they finish).
pub fn streams(agent: &str) -> bool {
    matches!(agent, "opencode" | "codex")
}

/// One line of an agent's output as a person would read it, or `None` for a line that says nothing to them (an
/// agent's bookkeeping events). Secrets are redacted.
pub fn readable(agent: &str, raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let text = match serde_json::from_str::<Value>(raw) {
        Ok(event) if event.is_object() => match agent {
            "opencode" => opencode(&event),
            "codex" => codex(&event),
            _ => None,
        }?,
        _ => raw.to_string(),
    };
    Some(crate::redact::redact(&text, &[]))
}

fn first_line(text: &str) -> String {
    text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").to_string()
}

/// `{"type":"tool_use","part":{"tool":"read","state":{"input":{"filePath":"…"},"status":"completed"}}}`, `text`
/// parts with what the model said, `error`, and step bookkeeping (skipped).
fn opencode(event: &Value) -> Option<String> {
    let part = &event["part"];
    match event["type"].as_str()? {
        "tool_use" => {
            let tool = part["tool"].as_str().unwrap_or("tool");
            let input = &part["state"]["input"];
            let failed = part["state"]["status"].as_str() == Some("error");
            let what = input["command"]
                .as_str()
                .map(|c| format!("$ {}", first_line(c)))
                .or_else(|| input["filePath"].as_str().map(|p| format!("{tool} {}", short_path(p))))
                .or_else(|| input["pattern"].as_str().map(|p| format!("{tool} {p}")))
                .unwrap_or_else(|| tool.to_string());
            Some(if failed { format!("{what} (failed)") } else { what })
        }
        "text" => part["text"].as_str().map(first_line).filter(|t| !t.is_empty()).map(|t| format!("» {t}")),
        "error" => Some(format!(
            "error: {}",
            event["error"]["data"]["message"].as_str().or(event["error"]["name"].as_str()).unwrap_or("unknown")
        )),
        _ => None,
    }
}

/// Codex `exec --json`: `item.completed` events with a command, a file change or a message.
fn codex(event: &Value) -> Option<String> {
    if event["type"].as_str()? != "item.completed" {
        return None;
    }
    let item = &event["item"];
    match item["type"].as_str()? {
        "command_execution" => item["command"].as_str().map(|c| format!("$ {}", first_line(c))),
        "file_change" => Some(format!(
            "edit {}",
            item["changes"]
                .as_array()
                .map(|c| c.iter().filter_map(|x| x["path"].as_str()).map(short_path).collect::<Vec<_>>().join(", "))
                .unwrap_or_default()
        )),
        "agent_message" => item["text"].as_str().map(first_line).map(|t| format!("» {t}")),
        _ => None,
    }
}

/// The last two parts of a path, which is what tells files apart in a worktree.
fn short_path(path: &str) -> String {
    let parts: Vec<&str> = path.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    parts[parts.len().saturating_sub(2)..].join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opencode_events_read_as_actions() {
        let read = r#"{"type":"tool_use","part":{"tool":"read","state":{"status":"completed","input":{"filePath":"C:\\w\\abc\\src\\app.js"}}}}"#;
        assert_eq!(readable("opencode", read).as_deref(), Some("read src/app.js"));
        let bash = r#"{"type":"tool_use","part":{"tool":"bash","state":{"status":"error","input":{"command":"npm test\nmore"}}}}"#;
        assert_eq!(readable("opencode", bash).as_deref(), Some("$ npm test (failed)"));
        let said = r#"{"type":"text","part":{"text":"\nI will add the route first.\nThen tests."}}"#;
        assert_eq!(readable("opencode", said).as_deref(), Some("» I will add the route first."));
        assert_eq!(readable("opencode", r#"{"type":"step_start","part":{}}"#), None, "bookkeeping says nothing");
        assert_eq!(readable("opencode", "plain text").as_deref(), Some("plain text"));
        let codex = r#"{"type":"item.completed","item":{"type":"command_execution","command":"cargo test"}}"#;
        assert_eq!(readable("codex", codex).as_deref(), Some("$ cargo test"));
        assert!(streams("opencode") && !streams("claude-code"));
    }

    #[test]
    fn an_attempts_log_is_bounded_and_read_after_a_sequence() {
        let id = format!("t-{}", uuid::Uuid::new_v4().simple());
        assert!(!since(&id, 0).known);
        for i in 0..(MAX_LINES + 5) {
            push(&id, &format!("line {i}"));
        }
        push(&id, "   ");
        let all = since(&id, 0);
        assert_eq!((all.lines.len(), all.dropped, all.done), (MAX_LINES, 5, false));
        assert_eq!(all.lines.last().unwrap().seq, (MAX_LINES + 5) as u64);
        let later = since(&id, (MAX_LINES + 3) as u64);
        assert_eq!(
            later.lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
            [format!("line {}", MAX_LINES + 3), format!("line {}", MAX_LINES + 4)]
        );
        finish(&id);
        assert!(since(&id, 0).done);
        finish("never-pushed");
        assert!(!since("never-pushed", 0).known, "finishing an unknown attempt adds nothing");
        push(&id, &"x".repeat(MAX_LINE_CHARS + 50));
        assert_eq!(since(&id, 0).lines.last().unwrap().text.chars().count(), MAX_LINE_CHARS + 1);
    }
}
