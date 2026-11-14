//! The diagnostics bundle (SCREENS §5.8, ROADMAP D4): one zip a user who hits a problem can save and send. It is
//! built from an allowlist: every query names the columns it reads, and nothing else leaves the store. Keys live in
//! the OS credential store and are never read here; client tokens are kept only as hashes, and neither the hash nor
//! the prefix is copied. Prompts, task titles, agent output, check output, diffs, provider error text and file
//! contents are not read at all. Every text value, and then every file as a whole, still goes through
//! [`crate::redact`], so a key someone pasted into a label or an endpoint is masked too.

use rusqlite::types::ValueRef;
use rusqlite::Connection;
use serde::Serialize;
use serde_json::{json, Map, Value};

/// How many of the latest ledger rows the bundle holds.
pub const LEDGER_ROWS: usize = 200;
/// How many of the latest app log lines the bundle holds, when there is a log.
pub const LOG_LINES: usize = 200;
/// The longest text value copied; anything longer is cut.
const MAX_TEXT: usize = 200;

/// What the caller knows that the store does not: the process's own view of the gateway and its log.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    /// Who built the bundle: `app` or `cli`.
    pub made_by: &'static str,
    /// `running`, `stopped`, `failed`, or for the CLI what `/health` said.
    pub gateway_state: String,
    pub gateway_port: u16,
    pub gateway_error: Option<String>,
    /// Recent log lines, if the process keeps any; the app keeps no log file today.
    pub log_lines: Vec<String>,
}

fn clean(text: &str) -> String {
    let cut: String = text.chars().take(MAX_TEXT).collect();
    crate::redact::redact(&cut, &[])
}

/// An endpoint without its user name, password, query or fragment, which is where a key would hide in a URL.
pub fn clean_endpoint(endpoint: &str) -> String {
    let (scheme, rest) = match endpoint.split_once("://") {
        Some((s, r)) => (format!("{s}://"), r),
        None => (String::new(), endpoint),
    };
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    let rest = match rest.split_once('/') {
        Some((authority, path)) => format!("{}/{path}", authority.rsplit('@').next().unwrap_or("")),
        None => rest.rsplit('@').next().unwrap_or("").to_string(),
    };
    clean(&format!("{scheme}{rest}"))
}

/// Rows of an allowlisted query as JSON objects keyed by column name; text is cut and redacted.
fn rows(conn: &Connection, sql: &str) -> Result<Vec<Value>, String> {
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let mut out = Vec::new();
    let mut query = stmt.query([]).map_err(|e| e.to_string())?;
    while let Some(row) = query.next().map_err(|e| e.to_string())? {
        let mut object = Map::new();
        for (i, name) in names.iter().enumerate() {
            let value = match row.get_ref(i).map_err(|e| e.to_string())? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(n) => json!(n),
                ValueRef::Real(f) => json!(f),
                ValueRef::Text(t) => Value::String(clean(&String::from_utf8_lossy(t))),
                // No allowlisted column is a blob; say so rather than copy bytes.
                ValueRef::Blob(_) => Value::String("[blob]".into()),
            };
            object.insert(name.clone(), value);
        }
        out.push(Value::Object(object));
    }
    Ok(out)
}

fn windows_version() -> String {
    #[cfg(windows)]
    {
        let v = windows_version::OsVersion::current();
        let name = if v.major == 10 && v.build >= 22_000 { "Windows 11" } else { "Windows" };
        format!("{name} {}.{}.{}", v.major, v.minor, v.build)
    }
    #[cfg(not(windows))]
    {
        format!("{} (not Windows)", std::env::consts::OS)
    }
}

/// The bundle's files as (name, contents), before zipping.
pub fn files(conn: &Connection, context: &Context, now_ms: i64) -> Result<Vec<(String, String)>, String> {
    let store_schema: u32 = crate::db::version(conn).map_err(|e| e.to_string())?;
    let summary = json!({
        "generatedAt": crate::gateway::store::iso_utc(now_ms),
        "madeBy": context.made_by,
        "app": "Pygg",
        "appVersion": env!("CARGO_PKG_VERSION"),
        "schemaVersion": crate::db::SCHEMA_VERSION,
        "storeSchemaVersion": store_schema,
        "os": windows_version(),
        "arch": std::env::consts::ARCH,
        "gateway": {
            "state": clean(&context.gateway_state),
            "port": context.gateway_port,
            "error": context.gateway_error.as_deref().map(clean),
        },
    });

    // Connections: no label (people put e-mail addresses there), no credential reference, only whether one is set.
    let mut connections = rows(
        conn,
        "SELECT id, kind, wire, endpoint, enabled, default_billing, data_use_terms, quota_group, \
         credential_ref IS NOT NULL AS has_credential, created_at, \
         (SELECT count(*) FROM models m WHERE m.provider_id = p.id) AS models, \
         (SELECT count(*) FROM models m WHERE m.provider_id = p.id AND m.enabled = 1) AS models_enabled \
         FROM providers p ORDER BY created_at",
    )?;
    let health = rows(
        conn,
        "SELECT provider_id, model_id, state, reason, http_status, until_ms, updated_at, strikes \
         FROM connection_health ORDER BY updated_at DESC",
    )?;
    for connection in &mut connections {
        if let Some(Value::String(endpoint)) = connection.get("endpoint").cloned() {
            connection["endpoint"] = Value::String(clean_endpoint(&endpoint));
        }
        let id = connection["id"].clone();
        let mine: Vec<Value> = health.iter().filter(|h| h["provider_id"] == id).cloned().collect();
        connection["health"] = Value::Array(mine);
    }

    // Clients: the name and the limits; never the token's hash or prefix.
    let clients = rows(conn,
        "SELECT id, name, monthly_budget_micro_usd, max_concurrency, unknown_cost_policy, paid_retries, auto_min_tier, \
         may_run_tasks, require_no_training, strict_budget, allowed_connections, created_at, last_used_at, revoked_at \
         FROM gateway_clients ORDER BY created_at")?;

    let tables: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .map_err(|e| e.to_string())?;
        let names = stmt.query_map([], |r| r.get(0)).map_err(|e| e.to_string())?;
        names.collect::<rusqlite::Result<_>>().map_err(|e| e.to_string())?
    };
    let mut counts = Map::new();
    for table in tables {
        // Names come from sqlite_master; quoted all the same.
        let n: i64 = conn
            .query_row(&format!("SELECT count(*) FROM \"{}\"", table.replace('"', "\"\"")), [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        counts.insert(table, json!(n));
    }

    // The ledger: what was asked for, where it went, what it cost. Not the provider's error text, not the session key.
    let ledger = rows(conn, &format!(
        "SELECT id, created_at, client_id, attempt_id IS NOT NULL AS from_task, requested_model, provider, model, route_mode, \
         route_reason, outcome, http_status, latency_ms, input_tokens, output_tokens, cache_tokens, cache_write_tokens, \
         cost_micro_usd, cost_status, at_risk_micro_usd, source \
         FROM usage_events ORDER BY created_at DESC LIMIT {LEDGER_ROWS}"))?;

    // Tasks: states only. Titles, prompts and reasons (which quote agent and check output) stay out.
    let tasks = rows(
        conn,
        "SELECT t.id, t.workspace_id, t.strategy, t.state, t.phase, t.result, t.client_id IS NOT NULL AS from_client, \
         t.created_at, t.updated_at, \
         (SELECT count(*) FROM attempts a JOIN task_stages s ON s.id = a.stage_id WHERE s.task_id = t.id) AS attempts, \
         (SELECT group_concat(a.state || ':' || a.billing || ':T' || a.tier, ' ') FROM attempts a JOIN task_stages s \
            ON s.id = a.stage_id WHERE s.task_id = t.id) AS attempt_states \
         FROM tasks t ORDER BY t.updated_at DESC",
    )?;

    let log = context.log_lines.iter().rev().take(LOG_LINES).rev().map(|l| clean(l)).collect::<Vec<_>>();

    let pretty = |v: &Value| serde_json::to_string_pretty(v).unwrap_or_default();
    let mut out = vec![
        ("README.txt".to_string(), README.to_string()),
        ("summary.json".to_string(), pretty(&summary)),
        ("connections.json".to_string(), pretty(&Value::Array(connections))),
        ("clients.json".to_string(), pretty(&Value::Array(clients))),
        ("tables.json".to_string(), pretty(&Value::Object(counts))),
        ("ledger.json".to_string(), pretty(&Value::Array(ledger))),
        ("tasks.json".to_string(), pretty(&Value::Array(tasks))),
    ];
    if !log.is_empty() {
        out.push(("log.txt".to_string(), log.join("\n")));
    }
    // Once more over each whole file, in case a value was assembled from parts.
    Ok(out.into_iter().map(|(name, text)| (name, crate::redact::redact(&text, &[]))).collect())
}

/// The bundle as zip bytes.
pub fn bundle(conn: &Connection, context: &Context, now_ms: i64) -> Result<Vec<u8>, String> {
    let files = files(conn, context, now_ms)?;
    Ok(zip(&files.iter().map(|(n, t)| (n.as_str(), t.as_bytes())).collect::<Vec<_>>(), now_ms))
}

/// The file name the save dialog suggests.
pub fn file_name(now_ms: i64) -> String {
    let stamp = crate::gateway::store::iso_utc(now_ms);
    format!("pygg-diagnostics-{}-{}{}.zip", &stamp[..10], &stamp[11..13], &stamp[14..16])
}

const README: &str = "Pygg diagnostics bundle\r\n\
\r\n\
Made on request, to send to whoever helps with a problem. It holds:\r\n\
  summary.json      app and schema version, Windows version, gateway state and port\r\n\
  connections.json  each connection's kind, endpoint (without user name, query or fragment), enabled state,\r\n\
                    billing, data-use terms, whether a key is set, model counts and health\r\n\
  clients.json      each client's name and limits\r\n\
  tables.json       how many rows each table of the store holds\r\n\
  ledger.json       the latest 200 ledger rows: models, outcome, tokens, cost (no error text)\r\n\
  tasks.json        each task's state, strategy and attempt states\r\n\
  log.txt           recent app log lines, when there are any\r\n\
\r\n\
It never holds keys, tokens or their hashes, connection labels, prompts, task titles, agent or check output,\r\n\
diffs, provider error text or file contents. Text values are cut to 200 characters and passed through Pygg's\r\n\
redaction, which masks anything shaped like an API key.\r\n";

// ---------- A minimal zip writer: stored entries, no compression ----------

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// MS-DOS date and time of a Unix ms instant (UTC), as zip stores them.
fn dos_time(ms: i64) -> (u16, u16) {
    let iso = crate::gateway::store::iso_utc(ms);
    let n = |a: usize, b: usize| iso[a..b].parse::<u16>().unwrap_or(0);
    let (year, month, day, hour, minute, second) = (n(0, 4), n(5, 7), n(8, 10), n(11, 13), n(14, 16), n(17, 19));
    let date = (year.saturating_sub(1980) << 9) | (month << 5) | day;
    let time = (hour << 11) | (minute << 5) | (second / 2);
    (time, date)
}

/// A zip archive of the given files, each stored as-is (they are small text files).
pub fn zip(files: &[(&str, &[u8])], now_ms: i64) -> Vec<u8> {
    let (time, date) = dos_time(now_ms);
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let size = data.len() as u32;
        let name = name.as_bytes();
        // Local file header; bit 11 says the name is UTF-8.
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0x0800u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&time.to_le_bytes());
        out.extend_from_slice(&date.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(data);
        // Central directory entry.
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0x0800u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&time.to_le_bytes());
        central.extend_from_slice(&date.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0u8; 8]); // extra, comment, disk start, internal attributes
        central.extend_from_slice(&0u32.to_le_bytes()); // external attributes
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name);
    }
    let central_offset = out.len() as u32;
    let count = files.len() as u16;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0u8; 4]); // this disk, disk with the central directory
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&central_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAKE_KEY: &str = "sk-or-v1-FAKEKEYdiag0123456789abcdef";
    const FAKE_TOKEN: &str = "pygg_tok_FAKEtokenDiag0123456789";
    const PROMPT: &str = "Refactor the secret payroll module called Zanzibar";

    fn store_with_secrets() -> Connection {
        let conn = crate::db::open_in_memory().unwrap();
        // A key pasted into the label and the endpoint's query, and a credential reference naming it.
        conn.execute(&format!(
            "INSERT INTO providers (id, kind, account_label, endpoint, enabled, credential_ref, created_at) VALUES \
             ('p1', 'openrouter', 'me@example.com {FAKE_KEY}', 'https://user:{FAKE_KEY}@openrouter.ai/api/v1?key={FAKE_KEY}#x', 1, \
             'pygg:p1:{FAKE_KEY}', 1)"), []).unwrap();
        conn.execute(&format!(
            "INSERT INTO connection_health (provider_id, model_id, state, reason, http_status, until_ms, updated_at) VALUES \
             ('p1', '', 'invalid_credentials', 'rejected key {FAKE_KEY}', 401, NULL, 2)"), []).unwrap();
        // A client whose token prefix and hash are the token itself, to prove neither column is read.
        conn.execute(
            &format!(
                "INSERT INTO gateway_clients (id, name, token_hash, token_prefix, created_at) VALUES \
             ('c1', 'OpenCode', '{FAKE_TOKEN}', '{FAKE_TOKEN}', 3)"
            ),
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO workspaces (id, name, path, created_at) VALUES ('w1', 'repo', 'C:/src/zanzibar', 1)",
            [],
        )
        .unwrap();
        conn.execute(&format!(
            "INSERT INTO tasks (id, workspace_id, title, prompt, strategy, state, reason, created_at, updated_at) VALUES \
             ('t1', 'w1', '{PROMPT}', '{PROMPT} using {FAKE_KEY}', 'free_first', 'check_failed', 'agent said: {PROMPT}', 4, 5)"), []).unwrap();
        conn.execute(
            "INSERT INTO task_stages (id, task_id, position, name, state) VALUES ('s1', 't1', 0, 'main', 'done')",
            [],
        )
        .unwrap();
        conn.execute(&format!(
            "INSERT INTO attempts (id, stage_id, number, agent, model_ref, tier, billing, state, summary, log_tail) VALUES \
             ('a1', 's1', 1, 'codex', 'x', 1, 'free', 'failed', '{PROMPT}', 'diff --git a/x b/x {FAKE_TOKEN}')"), []).unwrap();
        conn.execute(&format!(
            "INSERT INTO usage_events (id, attempt_id, client_id, provider, model, cost_status, source, created_at, error, session_key) \
             VALUES ('u1', 'a1', 'c1', 'openrouter', 'm', 'unknown', 'gateway', 6, 'upstream echoed: {PROMPT} {FAKE_KEY}', '{FAKE_TOKEN}')"),
            []).unwrap();
        conn.execute(&format!("INSERT INTO settings (key, value) VALUES ('note', '{FAKE_TOKEN}')"), []).unwrap();
        conn
    }

    #[test]
    fn a_bundle_holds_no_key_token_or_prompt() {
        let conn = store_with_secrets();
        let context = Context {
            made_by: "app",
            gateway_state: "running".into(),
            gateway_port: 8741,
            gateway_error: Some(format!("upstream said {FAKE_KEY}")),
            log_lines: vec![format!("Bearer {FAKE_KEY}")],
        };
        let bytes = bundle(&conn, &context, 1_700_000_000_000).unwrap();
        // Entries are stored uncompressed, so a search of the bytes is a search of every file.
        let text = String::from_utf8_lossy(&bytes);
        for needle in [
            FAKE_KEY,
            "FAKEKEYdiag",
            FAKE_TOKEN,
            "FAKEtokenDiag",
            "Zanzibar",
            "payroll",
            "me@example.com",
            "diff --git",
        ] {
            assert!(!text.contains(needle), "the bundle holds {needle}");
        }
        // What it is for is still there.
        for expected in [
            "\"kind\": \"openrouter\"",
            "https://openrouter.ai/api/v1",
            "\"has_credential\": 1",
            "invalid_credentials",
            "\"name\": \"OpenCode\"",
            "\"state\": \"check_failed\"",
            "\"tasks\": 1",
            "8741",
            "\"schemaVersion\"",
            "log.txt",
        ] {
            assert!(text.contains(expected), "the bundle lacks {expected}");
        }
    }

    #[test]
    fn endpoints_lose_user_info_query_and_fragment() {
        assert_eq!(clean_endpoint("https://u:p@host:8080/v1?key=abc#f"), "https://host:8080/v1");
        assert_eq!(clean_endpoint("http://127.0.0.1:11434"), "http://127.0.0.1:11434");
        assert_eq!(clean_endpoint("host/v1?x"), "host/v1");
    }

    #[test]
    fn the_zip_is_well_formed() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        let z = zip(&[("a.txt", b"hello"), ("b.json", b"{}")], 1_700_000_000_000);
        assert_eq!(&z[..4], &[0x50, 0x4b, 0x03, 0x04]);
        let end = z.len() - 22;
        assert_eq!(&z[end..end + 4], &[0x50, 0x4b, 0x05, 0x06]);
        assert_eq!(u16::from_le_bytes([z[end + 10], z[end + 11]]), 2);
        // 2023-11-14 22:13:20 UTC.
        assert_eq!(dos_time(1_700_000_000_000), ((22 << 11) | (13 << 5) | 10, (43 << 9) | (11 << 5) | 14));
        assert_eq!(file_name(1_700_000_000_000), "pygg-diagnostics-2023-11-14-2213.zip");
    }
}
