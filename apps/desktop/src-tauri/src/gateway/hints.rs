//! Claude Code's gateway hint headers (cost plan A2): what kind of request each one is (`main`, `subagent`, `workflow`,
//! `compaction`, `auxiliary`), which subagent sent it and which user prompt it serves. Claude Code sends them to a
//! custom base URL only with `CLAUDE_CODE_GATEWAY_HINT_HEADERS=1`, which `pygg run claude` sets (launcher.rs). They are
//! read here and kept in the ledger (migration 0039), never forwarded to a provider.
//!
//! The headers carry fixed vocabularies and random ids, never prompt text. Values are still checked: a request class
//! outside the documented list is kept as `other`, and an id that is not short printable ASCII is dropped.
//! Reference: https://code.claude.com/docs/en/llm-gateway-protocol ("Gateway hint headers").

use std::collections::HashMap;

use axum::http::HeaderMap;
use rusqlite::{params, Connection};
use serde::Serialize;

use super::store::Result;

pub const CLASS_HEADER: &str = "x-claude-code-request-class";
pub const AGENT_TYPE_HEADER: &str = "x-claude-code-agent-type";
pub const AGENT_ID_HEADER: &str = "x-claude-code-agent-id";
pub const PROMPT_ID_HEADER: &str = "x-claude-code-prompt-id";

/// The classes Claude Code documents, in the order Usage shows them.
pub const CLASSES: [&str; 5] = ["main", "subagent", "workflow", "compaction", "auxiliary"];

/// What a request said about itself.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RequestHint {
    pub class: Option<String>,
    pub agent_type: Option<String>,
    pub agent_id: Option<String>,
    pub prompt_id: Option<String>,
}

fn value(headers: &HeaderMap, name: &str, max: usize) -> Option<String> {
    let v = headers.get(name)?.to_str().ok()?.trim();
    (!v.is_empty() && v.len() <= max && v.chars().all(|c| c.is_ascii_graphic())).then(|| v.to_string())
}

/// The hint headers of a request, or `None` when it carried none.
pub fn from_headers(headers: &HeaderMap) -> Option<RequestHint> {
    let class = headers.get(CLASS_HEADER).and_then(|v| v.to_str().ok()).map(|v| {
        let v = v.trim().to_ascii_lowercase();
        if CLASSES.contains(&v.as_str()) {
            v
        } else {
            "other".to_string()
        }
    });
    let hint = RequestHint {
        class,
        agent_type: value(headers, AGENT_TYPE_HEADER, 40),
        agent_id: value(headers, AGENT_ID_HEADER, 96),
        prompt_id: value(headers, PROMPT_ID_HEADER, 64),
    };
    (hint != RequestHint::default()).then_some(hint)
}

/// One request class, or one subagent type, over a period or a session.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassGroup {
    pub key: String,
    /// Answered requests.
    pub requests: u64,
    /// Known money, failed attempts with a known cost included.
    pub known_micro_usd: u64,
    /// Entries whose cost is unknown (subscription ones included: a plan has no price per request).
    pub unknown_cost: u64,
    pub prompt_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    /// Distinct subagents that sent these requests.
    pub agents: u64,
}

/// Where requests went by kind, and how far subagents fanned out (Usage → Request Classes, the session receipt).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestClasses {
    /// Ledger entries that carried a request class.
    pub requests: u64,
    pub sessions: u64,
    /// By class, in `CLASSES` order, `other` last.
    pub groups: Vec<ClassGroup>,
    /// Subagents' own turns by agent type (`Explore`, `general-purpose`, `custom`…), most money first.
    pub agent_types: Vec<ClassGroup>,
    /// Distinct subagents.
    pub agents: u64,
    /// The most subagents one user prompt started.
    pub most_agents_one_prompt: u64,
    /// The most subagents of one session working at the same moment: each agent's span runs from the start of its
    /// first request to the end of its last.
    pub peak_parallel_agents: u64,
}

type Row = (
    Option<String>,
    String,
    String,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<String>,
    Option<String>,
    Option<String>,
    i64,
    Option<i64>,
);

/// Request classes since `since`, or of one session.
pub fn request_classes(conn: &Connection, since: i64, session: Option<&str>) -> Result<RequestClasses> {
    let filter = if session.is_some() { "session_key = ?2 AND created_at >= ?1" } else { "created_at >= ?1" };
    let sql = format!(
        "SELECT session_key, outcome, cost_status, cost_micro_usd, input_tokens, cache_tokens, \
         cache_write_tokens, request_class, agent_type, agent_id, created_at, latency_ms FROM usage_events \
         WHERE {filter} AND request_class IS NOT NULL ORDER BY created_at"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let map = |r: &rusqlite::Row| -> rusqlite::Result<Row> {
        Ok((
            r.get(0)?,
            r.get(1)?,
            r.get(2)?,
            r.get(3)?,
            r.get(4)?,
            r.get(5)?,
            r.get(6)?,
            r.get(7)?,
            r.get(8)?,
            r.get(9)?,
            r.get(10)?,
            r.get(11)?,
        ))
    };
    let rows: Vec<Row> = match session {
        Some(s) => stmt.query_map(params![since, s], map),
        None => stmt.query_map(params![since], map),
    }
    .map_err(|e| e.to_string())?
    .collect::<rusqlite::Result<_>>()
    .map_err(|e| e.to_string())?;
    let prompts: HashMap<String, Option<String>> = if rows.iter().any(|r| r.9.is_some()) {
        let sql = format!(
            "SELECT agent_id, MIN(prompt_id) FROM usage_events WHERE {filter} AND agent_id IS NOT NULL GROUP BY agent_id"
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let map = |r: &rusqlite::Row| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?));
        let found: HashMap<String, Option<String>> = match session {
            Some(s) => stmt.query_map(params![since, s], map),
            None => stmt.query_map(params![since], map),
        }
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<_>>()
        .map_err(|e| e.to_string())?;
        found
    } else {
        HashMap::new()
    };

    let n = |v: Option<i64>| v.unwrap_or(0).max(0) as u64;
    let mut out = RequestClasses::default();
    let mut groups: Vec<ClassGroup> = Vec::new();
    let mut types: Vec<ClassGroup> = Vec::new();
    let mut agents_of: HashMap<String, Vec<String>> = HashMap::new(); // group key -> agent ids
    let mut types_agents: HashMap<String, Vec<String>> = HashMap::new();
    let mut spans: HashMap<(String, String), (i64, i64)> = HashMap::new(); // (session, agent) -> (start, end)
    let mut sessions: Vec<String> = Vec::new();
    for (session_key, outcome, status, cost, input, cached, written, class, agent_type, agent_id, at, latency) in rows {
        out.requests += 1;
        let session_key = session_key.unwrap_or_default();
        if !sessions.contains(&session_key) {
            sessions.push(session_key.clone());
        }
        let class = class.unwrap_or_else(|| "other".into());
        let add = |list: &mut Vec<ClassGroup>, key: &str| {
            let i = match list.iter().position(|g| g.key == key) {
                Some(i) => i,
                None => {
                    list.push(ClassGroup { key: key.to_string(), ..Default::default() });
                    list.len() - 1
                }
            };
            let g = &mut list[i];
            if status == "unknown" {
                g.unknown_cost += 1;
            } else {
                g.known_micro_usd += n(cost);
            }
            if outcome == "ok" {
                g.requests += 1;
                g.prompt_tokens += n(input);
                g.cache_read_tokens += n(cached);
                g.cache_write_tokens += n(written);
            }
        };
        add(&mut groups, &class);
        if let Some(t) = agent_type.as_deref() {
            add(&mut types, t);
        }
        if let Some(id) = agent_id {
            let ids = agents_of.entry(class.clone()).or_default();
            if !ids.contains(&id) {
                ids.push(id.clone());
            }
            if let Some(t) = agent_type {
                let ids = types_agents.entry(t).or_default();
                if !ids.contains(&id) {
                    ids.push(id.clone());
                }
            }
            let start = at - latency.unwrap_or(0).max(0);
            let span = spans.entry((session_key, id)).or_insert((start, at));
            span.0 = span.0.min(start);
            span.1 = span.1.max(at);
        }
    }
    out.sessions = sessions.len() as u64;
    for g in groups.iter_mut() {
        g.agents = agents_of.get(&g.key).map_or(0, |v| v.len() as u64);
    }
    for g in types.iter_mut() {
        g.agents = types_agents.get(&g.key).map_or(0, |v| v.len() as u64);
    }
    groups.sort_by_key(|g| CLASSES.iter().position(|c| *c == g.key).unwrap_or(CLASSES.len()));
    types.sort_by(|a, b| b.known_micro_usd.cmp(&a.known_micro_usd).then_with(|| b.requests.cmp(&a.requests)));
    out.groups = groups;
    out.agent_types = types;

    let mut agents: Vec<&String> = spans.keys().map(|(_, a)| a).collect();
    agents.sort();
    agents.dedup();
    out.agents = agents.len() as u64;
    let mut per_prompt: HashMap<&str, u64> = HashMap::new();
    for a in &agents {
        if let Some(Some(p)) = prompts.get(a.as_str()) {
            *per_prompt.entry(p.as_str()).or_default() += 1;
        }
    }
    out.most_agents_one_prompt = per_prompt.values().copied().max().unwrap_or(0);
    // Per session, a sweep over the agents' spans: +1 at a start, −1 at an end, ends first at the same moment.
    let mut by_session: HashMap<&str, Vec<(i64, i32)>> = HashMap::new();
    for ((s, _), (start, end)) in &spans {
        let events = by_session.entry(s.as_str()).or_default();
        events.push((*start, 1));
        events.push((*end, -1));
    }
    for events in by_session.values_mut() {
        events.sort();
        let (mut now, mut peak) = (0i64, 0i64);
        for (_, d) in events.iter() {
            now += *d as i64;
            peak = peak.max(now);
        }
        out.peak_parallel_agents = out.peak_parallel_agents.max(peak as u64);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn hints_are_read_and_checked() {
        let mut h = HeaderMap::new();
        assert_eq!(from_headers(&h), None);
        h.insert(CLASS_HEADER, HeaderValue::from_static("Subagent"));
        h.insert(AGENT_TYPE_HEADER, HeaderValue::from_static("Explore"));
        h.insert(AGENT_ID_HEADER, HeaderValue::from_static("agent-1"));
        h.insert(PROMPT_ID_HEADER, HeaderValue::from_static("p-1"));
        let hint = from_headers(&h).unwrap();
        assert_eq!(hint.class.as_deref(), Some("subagent"));
        assert_eq!((hint.agent_type.as_deref(), hint.agent_id.as_deref()), (Some("Explore"), Some("agent-1")));
        h.insert(CLASS_HEADER, HeaderValue::from_static("brand-new"));
        h.insert(AGENT_ID_HEADER, HeaderValue::from_static("has space"));
        let hint = from_headers(&h).unwrap();
        assert_eq!(hint.class.as_deref(), Some("other"));
        assert_eq!(hint.agent_id, None);
    }

    #[allow(clippy::too_many_arguments)]
    fn row(
        conn: &Connection, id: &str, session: &str, at: i64, latency: i64, cost: Option<i64>, class: &str,
        agent: Option<(&str, &str)>, prompt: &str,
    ) {
        let status = if cost.is_some() { "estimated" } else { "unknown" };
        conn.execute(
            "INSERT INTO usage_events (id, client_id, provider, model, input_tokens, output_tokens, cache_tokens, \
             cache_write_tokens, cost_micro_usd, cost_status, source, created_at, latency_ms, outcome, session_key, \
             billing, request_class, agent_type, agent_id, prompt_id) VALUES (?1, 'c', 'p', 'm', 1000, 10, 600, 300, \
             ?2, ?3, 'price: test', ?4, ?5, 'ok', ?6, 'paid', ?7, ?8, ?9, ?10)",
            params![id, cost, status, at, latency, session, class, agent.map(|a| a.0), agent.map(|a| a.1), prompt],
        )
        .unwrap();
    }

    #[test]
    fn classes_agents_and_their_overlap() {
        let conn = crate::db::open_in_memory().unwrap();
        row(&conn, "m1", "s", 1_000, 500, Some(100), "main", None, "p1");
        // Three Explore agents from prompt p1: a and b overlap, c starts after both ended.
        row(&conn, "a1", "s", 2_000, 900, Some(40), "subagent", Some(("Explore", "a")), "p1");
        row(&conn, "a2", "s", 4_000, 900, Some(40), "subagent", Some(("Explore", "a")), "p1");
        row(&conn, "b1", "s", 3_000, 900, Some(50), "subagent", Some(("Explore", "b")), "p1");
        row(&conn, "c1", "s", 9_000, 500, None, "subagent", Some(("general-purpose", "c")), "p1");
        // A subagent's compaction keeps its id but has no type.
        conn.execute(
            "INSERT INTO usage_events (id, client_id, provider, model, cost_micro_usd, cost_status, source, created_at, \
             latency_ms, outcome, session_key, request_class, agent_id) \
             VALUES ('k1', 'c', 'p', 'm', 7, 'estimated', 'x', 9500, 100, 'ok', 's', 'compaction', 'c')",
            [],
        )
        .unwrap();
        row(&conn, "x1", "s", 10_000, 10, Some(1), "auxiliary", None, "p2");
        row(&conn, "o1", "other-session", 10_000, 10, Some(1000), "main", None, "p9");
        let all = request_classes(&conn, 0, None).unwrap();
        assert_eq!((all.requests, all.sessions), (8, 2));
        let one = request_classes(&conn, 0, Some("s")).unwrap();
        assert_eq!(one.requests, 7);
        let keys: Vec<&str> = one.groups.iter().map(|g| g.key.as_str()).collect();
        assert_eq!(keys, ["main", "subagent", "compaction", "auxiliary"]);
        let sub = &one.groups[1];
        assert_eq!((sub.requests, sub.known_micro_usd, sub.unknown_cost, sub.agents), (4, 130, 1, 3));
        assert_eq!(one.groups[2].agents, 1, "the compaction is counted to the agent that sent it");
        assert_eq!(one.agent_types[0].key, "Explore");
        assert_eq!(one.agent_types[0].agents, 2);
        assert_eq!(one.agents, 3);
        assert_eq!(one.most_agents_one_prompt, 3);
        // a: 1100..4000, b: 2100..3000, c: 8500..9500 → two at once at most.
        assert_eq!(one.peak_parallel_agents, 2);
    }
}
