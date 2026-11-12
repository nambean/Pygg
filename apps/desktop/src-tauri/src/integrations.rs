//! The integrations hub (ROADMAP D2, ADR-022): registers Pygg's MCP server with a coding agent the user already
//! has, so the agent can hand tasks to Pygg (ADR-027). Every change is the user's click, shown before it happens,
//! preceded by a backup of the agent's configuration file, checked afterwards, and removable.
//!
//! - **Claude Code and Codex CLI** edit their own configuration (`claude mcp add|remove`, `codex mcp add|remove`),
//!   so its format and comments stay theirs. Pygg only runs those commands.
//! - **OpenCode** has no command for this that runs without asking, so Pygg edits its JSON configuration itself,
//!   keeping every other key in its order, and refuses a file it cannot read exactly (comments, JSONC).
//! - The entry goes into the agent's **user** configuration, which serves every project: the MCP server takes the
//!   project from the folder the agent runs it in. A project file would be committed with the token in it.
//! - Each integration gets its **own client token**, which may start tasks and reaches no model connection.
//!   Removing the integration revokes it. The agent's own sign-in is never read or touched (ADR-026).
//! - **OpenCode's models through Pygg** (`opencode-gateway`, D2c) is the one integration that is not the MCP server:
//!   a `pygg` provider in OpenCode's configuration, pointed at the gateway with its own client token (a client like
//!   any other, under its limits in Clients) and the attempt's session header, so a task step's requests are costed
//!   by Pygg's ledger (tasks.rs). It lists `auto`, `auto/cheap` and the enabled models of enabled connections as
//!   they are when it is set up.
use crate::agents::{self, Launch};
use crate::gateway::store;
use crate::process::{self, Spec};
use rusqlite::Connection;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The name Pygg's server is registered under in every agent.
pub const SERVER: &str = "pygg";
const TIMEOUT: Duration = Duration::from_secs(60);
/// Where the new token goes in what is shown; the token itself is never shown.
const TOKEN_PLACEHOLDER: &str = "<new-token>";

/// The agents Pygg can register itself with: id (as in agents.rs), name, program.
pub const TARGETS: [(&str, &str, &str); 3] =
    [("claude-code", "Claude Code", "claude"), ("codex", "Codex CLI", "codex"), ("opencode", "OpenCode", "opencode")];

/// OpenCode's models through Pygg's gateway: a provider entry, not the MCP server.
pub const OPENCODE_GATEWAY: &str = "opencode-gateway";
const OPENCODE_GATEWAY_TARGET: (&str, &str, &str) = (OPENCODE_GATEWAY, "OpenCode: models through Pygg", "opencode");
/// The provider name in OpenCode's configuration, so its models are `pygg/<connection>/<model>`.
pub const PROVIDER: &str = "pygg";

fn target(id: &str) -> Result<(&'static str, &'static str, &'static str), String> {
    if id == OPENCODE_GATEWAY {
        return Ok(OPENCODE_GATEWAY_TARGET);
    }
    TARGETS.iter().copied().find(|t| t.0 == id).ok_or_else(|| format!("Pygg cannot set up {id}"))
}

/// Every integration id, in the order shown.
pub fn ids() -> Vec<&'static str> {
    TARGETS.iter().map(|t| t.0).chain(std::iter::once(OPENCODE_GATEWAY)).collect()
}

fn kind_of(id: &str) -> &'static str {
    if id == OPENCODE_GATEWAY { "gateway" } else { "mcp" }
}

/// Where each agent keeps the configuration its command edits, following the agents' own variables.
#[derive(Clone, Debug)]
pub struct Homes {
    pub claude: PathBuf,
    pub codex: PathBuf,
    pub opencode: PathBuf,
}

impl Homes {
    pub fn from_env() -> Homes {
        let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
        let home = var(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap_or_default();
        Homes {
            claude: var("CLAUDE_CONFIG_DIR").unwrap_or_else(|| home.clone()).join(".claude.json"),
            codex: var("CODEX_HOME").unwrap_or_else(|| home.join(".codex")).join("config.toml"),
            opencode: var("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config")).join("opencode").join("opencode.json"),
        }
    }

    fn config(&self, id: &str) -> &Path {
        match id {
            "claude-code" => &self.claude,
            "codex" => &self.codex,
            _ => &self.opencode,
        }
    }
}

/// What the agent is told to start: the app's own program with `mcp` (src/mcp.rs), and the gateway's address when
/// it is not the default. For OpenCode's models through Pygg: the gateway's address and the models it lists.
#[derive(Clone, Debug, Default)]
pub struct Server {
    pub program: String,
    pub args: Vec<String>,
    pub url: Option<String>,
    /// `http://127.0.0.1:<port>`, always.
    pub gateway_url: String,
    pub models: Vec<GatewayModel>,
}

/// A model OpenCode is told about, as Pygg knows it: unknown facts are left out, never guessed.
#[derive(Clone, Debug, PartialEq)]
pub struct GatewayModel {
    /// `auto`, `auto/cheap` or `<connection>/<model>`.
    pub id: String,
    pub context_tokens: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub supports_tools: Option<bool>,
}

/// `auto`, `auto/cheap`, then the enabled models of enabled connections.
pub fn gateway_models(conn: &Connection) -> Vec<GatewayModel> {
    let enabled: std::collections::HashSet<String> = store::list_providers(conn).unwrap_or_default().into_iter()
        .filter(|p| p.enabled).map(|p| p.id).collect();
    let aliases = crate::gateway::auto::ALIASES.iter().map(|a| GatewayModel { id: a.to_string(), context_tokens: None,
        max_output_tokens: None, supports_tools: Some(true) });
    let models = store::list_models(conn, None).unwrap_or_default().into_iter()
        .filter(|m| m.enabled && enabled.contains(&m.provider_id))
        .map(|m| GatewayModel { id: format!("{}/{}", m.provider_id, m.model_id), context_tokens: m.capabilities.context_tokens,
            max_output_tokens: m.capabilities.max_output_tokens, supports_tools: m.capabilities.supports_tools });
    aliases.chain(models).collect()
}

/// What Pygg remembers about an integration it set up (setting `integration.<id>`).
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    client_id: String,
    client_name: String,
    set_up_at: i64,
    backup: Option<String>,
}

fn record_key(id: &str) -> String {
    format!("integration.{id}")
}

fn record(conn: &Connection, id: &str) -> Option<Record> {
    store::setting(conn, &record_key(id)).ok().flatten().filter(|s| !s.is_empty()).and_then(|s| serde_json::from_str(&s).ok())
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Integration {
    pub id: String,
    pub name: String,
    /// `mcp` (Pygg's MCP server in the agent) or `gateway` (the agent's models through Pygg's gateway).
    pub kind: String,
    pub installed: bool,
    /// `not_installed`, `not_set_up`, `set_up` (by Pygg), `set_up_elsewhere` (an entry named pygg Pygg did not
    /// make) or `removed_elsewhere` (Pygg made it, and it is gone).
    pub state: String,
    /// The configuration file the agent's entry lives in.
    pub config_path: String,
    /// What the agent says about the entry, one redacted line, where it says something.
    pub detail: Option<String>,
    pub client_name: Option<String>,
    pub set_up_at: Option<i64>,
    pub backup: Option<String>,
}

/// What setting up would do, shown before the user agrees.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub config_path: String,
    pub config_exists: bool,
    /// The command Pygg runs, or the entry it adds, with the token shown as a placeholder.
    pub change: String,
    /// `command` (the agent edits its own file) or `file` (Pygg edits it).
    pub method: String,
    pub client_name: String,
    pub backup_dir: String,
}

fn client_name(name: &str) -> String {
    format!("{name} (MCP)")
}

fn client_name_for(id: &str, name: &str) -> String {
    if id == OPENCODE_GATEWAY { "OpenCode (models)".into() } else { client_name(name) }
}

fn launch(program: &str) -> Option<Launch> {
    agents::locate(program)
}

async fn run(launch: &Launch, args: Vec<String>) -> process::Outcome {
    let args = launch.prefix.iter().cloned().chain(args).collect();
    let spec = Spec { program: launch.program.clone(), args, raw_tail: None, cwd: std::env::temp_dir(), env: Vec::new(),
        timeout: TIMEOUT, output_limit: 256 * 1024 };
    process::run(&spec, std::future::pending()).await
}

fn one_line(out: &process::Outcome, secrets: &[&str]) -> Option<String> {
    let text = format!("{}\n{}", out.stdout, out.stderr);
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).take(4).collect();
    (!lines.is_empty()).then(|| crate::redact::redact(&lines.join(" · "), secrets).chars().take(240).collect())
}

/// The arguments that register the server with Claude Code or Codex, `token` included.
pub fn add_args(id: &str, server: &Server, token: &str) -> Vec<String> {
    let mut args: Vec<String> = match id {
        "claude-code" => vec!["mcp".into(), "add".into(), "--scope".into(), "user".into(), SERVER.into()],
        _ => vec!["mcp".into(), "add".into(), SERVER.into()],
    };
    let flag = if id == "claude-code" { "-e" } else { "--env" };
    let mut env = vec![format!("PYGG_TOKEN={token}")];
    if let Some(url) = &server.url {
        env.push(format!("PYGG_URL={url}"));
    }
    for e in env {
        args.push(flag.into());
        args.push(e);
    }
    args.push("--".into());
    args.push(server.program.clone());
    args.extend(server.args.iter().cloned());
    args
}

fn remove_args(id: &str) -> Vec<String> {
    match id {
        "claude-code" => vec!["mcp".into(), "remove".into(), SERVER.into(), "--scope".into(), "user".into()],
        _ => vec!["mcp".into(), "remove".into(), SERVER.into()],
    }
}

fn shown(program: &str, args: &[String]) -> String {
    let quote = |a: &String| if a.contains(' ') || a.is_empty() { format!("\"{a}\"") } else { a.clone() };
    std::iter::once(program.to_string()).chain(args.iter().map(quote)).collect::<Vec<_>>().join(" ")
}

/// OpenCode's entry for the server.
pub fn opencode_entry(server: &Server, token: &str) -> Value {
    let mut env = serde_json::Map::new();
    env.insert("PYGG_TOKEN".into(), json!(token));
    if let Some(url) = &server.url {
        env.insert("PYGG_URL".into(), json!(url));
    }
    let command: Vec<String> = std::iter::once(server.program.clone()).chain(server.args.iter().cloned()).collect();
    json!({"type": "local", "command": command, "environment": env, "enabled": true})
}

/// Reads OpenCode's configuration. A missing file is an empty configuration; a file that is not plain JSON (its
/// comments would be lost) is refused.
pub fn read_opencode(path: &Path) -> Result<Option<serde_json::Map<String, Value>>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("Could not read {}: {e}", path.display())),
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(map)) => Ok(Some(map)),
        _ => Err(format!("{} is not plain JSON (it may have comments), so Pygg will not rewrite it. Add the entry by \
            hand, as shown under Set It Up by Hand.", path.display())),
    }
}

/// The configuration with Pygg's entry added; refused when an entry of that name is already there.
pub fn with_opencode_entry(config: Option<serde_json::Map<String, Value>>, entry: Value)
    -> Result<serde_json::Map<String, Value>, String> {
    let mut config = config.unwrap_or_else(|| {
        let mut m = serde_json::Map::new();
        m.insert("$schema".into(), json!("https://opencode.ai/config.json"));
        m
    });
    let mcp = config.entry("mcp").or_insert_with(|| json!({}));
    let Value::Object(mcp) = mcp else { return Err("OpenCode's \"mcp\" setting is not an object".into()) };
    if mcp.contains_key(SERVER) {
        return Err(format!("OpenCode already has an MCP server named {SERVER}"));
    }
    mcp.insert(SERVER.into(), entry);
    Ok(config)
}

/// The configuration without Pygg's entry (and without `mcp` when nothing else was in it).
pub fn without_opencode_entry(mut config: serde_json::Map<String, Value>) -> serde_json::Map<String, Value> {
    let empty = match config.get_mut("mcp") {
        Some(Value::Object(mcp)) => {
            mcp.remove(SERVER);
            mcp.is_empty()
        }
        _ => false,
    };
    if empty {
        config.remove("mcp");
    }
    config
}

fn opencode_has_entry(config: &Option<serde_json::Map<String, Value>>) -> bool {
    config.as_ref().and_then(|c| c.get("mcp")).and_then(|m| m.get(SERVER)).is_some()
}

/// OpenCode's provider entry for Pygg's gateway. It speaks the Anthropic format, which the gateway serves for every
/// connection (translating where the connection speaks OpenAI's), and sends the attempt id Pygg gives a task step as
/// its session header; outside a task the variable is empty and the gateway ignores the header.
pub fn opencode_provider(server: &Server, token: &str) -> Value {
    let mut models = serde_json::Map::new();
    for m in &server.models {
        let mut entry = serde_json::Map::new();
        entry.insert("name".into(), json!(m.id));
        // OpenCode offers no tools to a model marked without them; only a model known not to take them is.
        entry.insert("tool_call".into(), json!(m.supports_tools != Some(false)));
        if let (Some(context), Some(output)) = (m.context_tokens, m.max_output_tokens) {
            entry.insert("limit".into(), json!({"context": context, "output": output}));
        }
        models.insert(m.id.clone(), Value::Object(entry));
    }
    json!({
        "npm": "@ai-sdk/anthropic",
        "name": "Pygg",
        "options": {
            "baseURL": format!("{}/v1", server.gateway_url.trim_end_matches('/')),
            "apiKey": token,
            "headers": {crate::gateway::SESSION_HEADER: "{env:PYGG_ATTEMPT_ID}"},
        },
        "models": models,
    })
}

/// The configuration with Pygg's provider added; refused when a provider of that name is already there.
pub fn with_opencode_provider(config: Option<serde_json::Map<String, Value>>, entry: Value)
    -> Result<serde_json::Map<String, Value>, String> {
    let mut config = config.unwrap_or_else(|| {
        let mut m = serde_json::Map::new();
        m.insert("$schema".into(), json!("https://opencode.ai/config.json"));
        m
    });
    let providers = config.entry("provider").or_insert_with(|| json!({}));
    let Value::Object(providers) = providers else { return Err("OpenCode's \"provider\" setting is not an object".into()) };
    if providers.contains_key(PROVIDER) {
        return Err(format!("OpenCode already has a provider named {PROVIDER}"));
    }
    providers.insert(PROVIDER.into(), entry);
    Ok(config)
}

/// The configuration without Pygg's provider (and without `provider` when nothing else was in it).
pub fn without_opencode_provider(mut config: serde_json::Map<String, Value>) -> serde_json::Map<String, Value> {
    let empty = match config.get_mut("provider") {
        Some(Value::Object(providers)) => {
            providers.remove(PROVIDER);
            providers.is_empty()
        }
        _ => false,
    };
    if empty {
        config.remove("provider");
    }
    config
}

fn opencode_has_provider(config: &Option<serde_json::Map<String, Value>>) -> bool {
    config.as_ref().and_then(|c| c.get("provider")).and_then(|m| m.get(PROVIDER)).is_some()
}

/// Writes a file whole: to a temporary file next to it, then renamed over it.
fn write_atomically(path: &Path, text: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("json.pygg-tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("Could not write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Could not replace {}: {e}", path.display())
    })
}

fn write_opencode(path: &Path, config: &serde_json::Map<String, Value>) -> Result<(), String> {
    let text = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    write_atomically(path, &format!("{text}\n"))
}

/// Copies the configuration file into `backup_dir` before Pygg changes it; `None` when there is no file yet.
pub fn back_up(config: &Path, backup_dir: &Path, id: &str, now_ms: i64) -> Result<Option<PathBuf>, String> {
    if !config.is_file() {
        return Ok(None);
    }
    std::fs::create_dir_all(backup_dir).map_err(|e| format!("Could not create {}: {e}", backup_dir.display()))?;
    let name = config.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "config".into());
    let target = backup_dir.join(format!("{id}-{now_ms}-{name}"));
    std::fs::copy(config, &target).map_err(|e| format!("Could not back up {}: {e}", config.display()))?;
    Ok(Some(target))
}

/// Puts the backup back, but only if the file changed since it was taken: used when a change failed half-way.
fn restore_if_changed(config: &Path, backup: Option<&Path>, before: &Option<Vec<u8>>) {
    let now = std::fs::read(config).ok();
    if &now == before {
        return;
    }
    match backup {
        Some(b) => { let _ = std::fs::copy(b, config); }
        None => { let _ = std::fs::remove_file(config); }
    }
}

/// Whether the agent has an entry named pygg, and what it says about it.
async fn registered(id: &str, homes: &Homes) -> (Option<bool>, Option<String>) {
    let (_, _, program) = match target(id) { Ok(t) => t, Err(_) => return (None, None) };
    if id == OPENCODE_GATEWAY {
        return match read_opencode(&homes.opencode) {
            Ok(config) => (Some(opencode_has_provider(&config)), None),
            Err(e) => (None, Some(e)),
        };
    }
    if id == "opencode" {
        return match read_opencode(&homes.opencode) {
            Ok(config) => (Some(opencode_has_entry(&config)), None),
            Err(e) => (None, Some(e)),
        };
    }
    let Some(launch) = launch(program) else { return (None, None) };
    let out = run(&launch, vec!["mcp".into(), "get".into(), SERVER.into()]).await;
    match out.exit {
        process::Exit::Code(0) => (Some(true), one_line(&out, &[])),
        process::Exit::Code(_) => (Some(false), None),
        _ => (None, one_line(&out, &[]).or_else(|| Some("The agent did not answer".into()))),
    }
}

/// The state of one integration. `rec` is what Pygg remembers about it.
async fn describe(id: &str, homes: &Homes, rec: Option<Record>) -> Result<Integration, String> {
    let (id, name, program) = target(id)?;
    let installed = launch(program).is_some();
    let config_path = homes.config(id).to_string_lossy().into_owned();
    let (present, detail) = if installed { registered(id, homes).await } else { (None, None) };
    let state = match (installed, present, &rec) {
        (_, Some(true), Some(_)) => "set_up",
        (_, Some(true), None) => "set_up_elsewhere",
        (_, Some(false), Some(_)) => "removed_elsewhere",
        (false, _, _) => "not_installed",
        _ => "not_set_up",
    };
    Ok(Integration { id: id.into(), name: name.into(), kind: kind_of(id).into(), installed, state: state.into(), config_path, detail,
        client_name: rec.as_ref().map(|r| r.client_name.clone()), set_up_at: rec.as_ref().map(|r| r.set_up_at),
        backup: rec.and_then(|r| r.backup) })
}

/// The records Pygg keeps, read from the store before the slow checks run.
pub fn records(conn: &Connection) -> Vec<(String, Option<String>)> {
    ids().into_iter().map(|id| (id.to_string(), store::setting(conn, &record_key(id)).ok().flatten())).collect()
}

pub async fn list_with(records: Vec<(String, Option<String>)>, homes: &Homes) -> Vec<Integration> {
    let mut out = Vec::new();
    for (id, rec) in records {
        let rec = rec.filter(|s| !s.is_empty()).and_then(|s| serde_json::from_str(&s).ok());
        if let Ok(i) = describe(&id, homes, rec).await {
            out.push(i);
        }
    }
    out
}

pub fn plan(id: &str, server: &Server, homes: &Homes, backup_dir: &Path) -> Result<Plan, String> {
    let (id, name, program) = target(id)?;
    let config = homes.config(id);
    let (change, method) = if id == OPENCODE_GATEWAY {
        let entry = json!({"provider": {PROVIDER: opencode_provider(server, TOKEN_PLACEHOLDER)}});
        (serde_json::to_string_pretty(&entry).unwrap_or_default(), "file")
    } else if id == "opencode" {
        let entry = json!({"mcp": {SERVER: opencode_entry(server, TOKEN_PLACEHOLDER)}});
        (serde_json::to_string_pretty(&entry).unwrap_or_default(), "file")
    } else {
        (shown(program, &add_args(id, server, TOKEN_PLACEHOLDER)), "command")
    };
    Ok(Plan { id: id.into(), name: name.into(), kind: kind_of(id).into(), config_path: config.to_string_lossy().into_owned(),
        config_exists: config.is_file(), change, method: method.into(), client_name: client_name_for(id, name),
        backup_dir: backup_dir.to_string_lossy().into_owned() })
}

/// The store, reached briefly between the slow steps, never across an await (`gateway::Shared` in the app).
pub trait Store {
    fn with<T>(&self, f: impl FnOnce(&Connection) -> T) -> T;
}

impl Store for crate::gateway::Shared {
    fn with<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
        self.with_db(f)
    }
}

fn saved(db: &impl Store, id: &str) -> Option<Record> {
    db.with(|c| record(c, id))
}

fn save(db: &impl Store, id: &str, rec: Option<&Record>) -> Result<(), String> {
    let text = rec.map(|r| serde_json::to_string(r).unwrap_or_default()).unwrap_or_default();
    db.with(|c| store::set_setting(c, &record_key(id), &text))
}

/// A token that may start tasks and reaches no model connection: the MCP server only uses the task API. For
/// OpenCode's models through Pygg, a client like any other: it reaches the connections, may not start tasks, and
/// its limits are set in Clients.
fn create_client(db: &impl Store, id: &str, name: &str) -> Result<(String, String), String> {
    db.with(|c| {
        let (client, token) = store::create_client(c, name, None)?;
        if id != OPENCODE_GATEWAY {
            store::set_client_may_run_tasks(c, &client.id, true)?;
            store::set_client_connections(c, &client.id, Some(&[]))?;
        }
        Ok((client.id, token))
    })
}

fn revoke(db: &impl Store, client_id: &str) {
    let _ = db.with(|c| store::revoke_client(c, client_id));
}

/// Sets the integration up: backup, a new token, the agent's own command (or Pygg's edit of OpenCode's file), and
/// a check that the agent now lists the server. Any failure undoes what was done.
pub async fn apply(id: &str, server: &Server, homes: &Homes, backup_dir: &Path, db: &impl Store, now_ms: i64)
    -> Result<Integration, String> {
    let (id, name, program) = target(id)?;
    let launch = launch(program).ok_or_else(|| format!("{name} is not installed on this computer"))?;
    let existing = saved(db, id);
    match registered(id, homes).await {
        (Some(true), _) if existing.is_some() => return Err(format!("Pygg is already set up in {name}")),
        (Some(true), _) => return Err(format!("{name} already has an MCP server named {SERVER}, set up outside \
            Pygg. Remove it here first, or keep using it.")),
        (None, Some(why)) => return Err(why),
        (None, None) => return Err(format!("{name} did not say whether it has an MCP server named {SERVER}")),
        _ => {}
    }
    if let Some(old) = &existing {
        revoke(db, &old.client_id); // removed outside Pygg: its token is no longer used
    }
    let config = homes.config(id).to_path_buf();
    // OpenCode's file is checked before anything is created.
    let opencode = if id == "opencode" || id == OPENCODE_GATEWAY { Some(read_opencode(&config)?) } else { None };
    if id == OPENCODE_GATEWAY && server.models.len() <= crate::gateway::auto::ALIASES.len() {
        return Err("No model is enabled in Connections yet, so OpenCode would only get auto. Add a connection first.".into());
    }
    let before = std::fs::read(&config).ok();
    let backup = back_up(&config, backup_dir, id, now_ms)?;
    let cname = client_name_for(id, name);
    let (client_id, token) = create_client(db, id, &cname)?;
    let undo = |why: String| {
        restore_if_changed(&config, backup.as_deref(), &before);
        revoke(db, &client_id);
        why
    };
    match opencode {
        Some(current) if id == OPENCODE_GATEWAY => {
            let updated = with_opencode_provider(current, opencode_provider(server, &token)).map_err(&undo)?;
            write_opencode(&config, &updated).map_err(&undo)?;
        }
        Some(current) => {
            let updated = with_opencode_entry(current, opencode_entry(server, &token)).map_err(&undo)?;
            write_opencode(&config, &updated).map_err(&undo)?;
        }
        None => {
            let out = run(&launch, add_args(id, server, &token)).await;
            if !out.succeeded() {
                let why = one_line(&out, &[&token]).unwrap_or_else(|| "no output".into());
                return Err(undo(format!("{name} refused the setup: {why}")));
            }
        }
    }
    if registered(id, homes).await.0 != Some(true) {
        if id != "opencode" && id != OPENCODE_GATEWAY {
            let _ = run(&launch, remove_args(id)).await;
        }
        return Err(undo(format!("{name} does not list the {SERVER} server after the setup, so it was undone")));
    }
    let rec = Record { client_id, client_name: cname, set_up_at: now_ms,
        backup: backup.map(|b| b.to_string_lossy().into_owned()) };
    save(db, id, Some(&rec))?;
    describe(id, homes, Some(rec)).await
}

/// Removes the entry (with the agent's own command, or from OpenCode's file after a backup) and revokes the token
/// Pygg made for it.
pub async fn remove(id: &str, homes: &Homes, backup_dir: &Path, db: &impl Store, now_ms: i64) -> Result<Integration, String> {
    let (id, name, program) = target(id)?;
    let existing = saved(db, id);
    let present = registered(id, homes).await.0;
    if present == Some(true) {
        let config = homes.config(id).to_path_buf();
        back_up(&config, backup_dir, id, now_ms)?;
        if id == OPENCODE_GATEWAY {
            let current = read_opencode(&config)?.unwrap_or_default();
            write_opencode(&config, &without_opencode_provider(current))?;
        } else if id == "opencode" {
            let current = read_opencode(&config)?.unwrap_or_default();
            write_opencode(&config, &without_opencode_entry(current))?;
        } else {
            let launch = launch(program).ok_or_else(|| format!("{name} is not installed on this computer"))?;
            let out = run(&launch, remove_args(id)).await;
            if !out.succeeded() {
                return Err(format!("{name} refused to remove it: {}", one_line(&out, &[]).unwrap_or_default()));
            }
        }
    }
    if let Some(rec) = &existing {
        revoke(db, &rec.client_id);
    }
    save(db, id, None)?;
    describe(id, homes, None).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(url: Option<&str>) -> Server {
        Server { program: r"C:\Users\me\AppData\Local\Pygg\Pygg.exe".into(), args: vec!["mcp".into()],
            url: url.map(String::from), gateway_url: "http://127.0.0.1:8741".into(), models: vec![
                GatewayModel { id: "auto".into(), context_tokens: None, max_output_tokens: None, supports_tools: Some(true) },
                GatewayModel { id: "anthropic/claude-haiku-5-5".into(), context_tokens: Some(200_000),
                    max_output_tokens: Some(64_000), supports_tools: Some(true) },
                GatewayModel { id: "ollama/qwen3:8b".into(), context_tokens: None, max_output_tokens: None, supports_tools: None },
                GatewayModel { id: "x/embed".into(), context_tokens: None, max_output_tokens: None, supports_tools: Some(false) },
            ] }
    }

    #[test]
    fn claude_code_is_registered_for_the_user_with_its_token_before_the_command() {
        let args = add_args("claude-code", &server(None), "str-x");
        assert_eq!(args, ["mcp", "add", "--scope", "user", "pygg", "-e", "PYGG_TOKEN=str-x", "--",
            r"C:\Users\me\AppData\Local\Pygg\Pygg.exe", "mcp"]);
        assert_eq!(remove_args("claude-code"), ["mcp", "remove", "pygg", "--scope", "user"]);
    }

    #[test]
    fn codex_gets_the_address_only_when_it_is_not_the_default() {
        let args = add_args("codex", &server(Some("http://127.0.0.1:8800")), "str-x");
        assert_eq!(args, ["mcp", "add", "pygg", "--env", "PYGG_TOKEN=str-x", "--env", "PYGG_URL=http://127.0.0.1:8800",
            "--", r"C:\Users\me\AppData\Local\Pygg\Pygg.exe", "mcp"]);
    }

    #[test]
    fn the_plan_shows_a_placeholder_never_a_token() {
        let homes = Homes { claude: "a".into(), codex: "b".into(), opencode: "c".into() };
        for id in ids() {
            let p = plan(id, &server(None), &homes, Path::new("backups")).unwrap();
            assert!(p.change.contains(TOKEN_PLACEHOLDER) && !p.change.contains("str-"), "{}", p.change);
        }
        let p = plan("claude-code", &server(None), &homes, Path::new("backups")).unwrap();
        assert!(p.change.starts_with("claude mcp add --scope user pygg -e"), "{}", p.change);
        assert!(p.change.ends_with(r"-e PYGG_TOKEN=<new-token> -- C:\Users\me\AppData\Local\Pygg\Pygg.exe mcp"), "{}", p.change);
        let spaced = Server { program: r"C:\Program Files\Pygg\Pygg.exe".into(), ..server(None) };
        let p = plan("codex", &spaced, &homes, Path::new("b")).unwrap();
        assert!(p.change.contains(r#""C:\Program Files\Pygg\Pygg.exe""#), "{}", p.change);
        assert!(plan("cursor", &server(None), &homes, Path::new("b")).is_err());
    }

    #[test]
    fn opencode_keeps_every_other_key_in_its_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opencode.json");
        std::fs::write(&path, r#"{"$schema":"https://opencode.ai/config.json","provider":{"x":{"options":{"apiKey":"k"}}},
            "mcp":{"other":{"type":"remote","url":"https://e"}},"model":"x/y"}"#).unwrap();
        let config = read_opencode(&path).unwrap();
        let added = with_opencode_entry(config, opencode_entry(&server(None), "str-x")).unwrap();
        write_opencode(&path, &added).unwrap();
        let back = read_opencode(&path).unwrap().unwrap();
        assert_eq!(back.keys().collect::<Vec<_>>(), ["$schema", "provider", "mcp", "model"]);
        assert_eq!(back["mcp"]["pygg"]["command"], json!([r"C:\Users\me\AppData\Local\Pygg\Pygg.exe", "mcp"]));
        assert_eq!(back["mcp"]["pygg"]["environment"]["PYGG_TOKEN"], "str-x");
        assert_eq!(back["provider"]["x"]["options"]["apiKey"], "k");
        // A second entry of that name is refused; removing it leaves the other server.
        assert!(with_opencode_entry(Some(back.clone()), json!({})).is_err());
        let removed = without_opencode_entry(back);
        assert_eq!(removed["mcp"], json!({"other": {"type": "remote", "url": "https://e"}}));
    }

    #[test]
    fn opencode_without_a_file_gets_one_and_loses_mcp_when_pygg_was_its_only_server() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("opencode.json");
        assert_eq!(read_opencode(&path).unwrap(), None);
        let added = with_opencode_entry(None, opencode_entry(&server(None), "t")).unwrap();
        write_opencode(&path, &added).unwrap();
        let back = read_opencode(&path).unwrap();
        assert!(opencode_has_entry(&back));
        let removed = without_opencode_entry(back.unwrap());
        assert!(!removed.contains_key("mcp") && removed.contains_key("$schema"));
    }

    #[test]
    fn opencodes_models_through_pygg_are_a_provider_with_the_session_header_and_only_known_facts() {
        let entry = opencode_provider(&server(None), "str-m");
        assert_eq!(entry["npm"], "@ai-sdk/anthropic");
        assert_eq!(entry["options"]["baseURL"], "http://127.0.0.1:8741/v1");
        assert_eq!(entry["options"]["apiKey"], "str-m");
        assert_eq!(entry["options"]["headers"]["x-smarttaskrouter-session"], "{env:PYGG_ATTEMPT_ID}");
        let models = entry["models"].as_object().unwrap();
        assert_eq!(models.keys().collect::<Vec<_>>(), ["auto", "anthropic/claude-haiku-5-5", "ollama/qwen3:8b", "x/embed"]);
        assert_eq!(models["anthropic/claude-haiku-5-5"]["limit"], json!({"context": 200000, "output": 64000}));
        assert!(models["ollama/qwen3:8b"].get("limit").is_none(), "unknown limits are left out");
        assert_eq!(models["ollama/qwen3:8b"]["tool_call"], true, "unknown tool support is not taken as none");
        assert_eq!(models["x/embed"]["tool_call"], false);

        // Added next to the user's own providers, refused twice, removed cleanly.
        let mut mine = serde_json::Map::new();
        mine.insert("provider".into(), json!({"x": {"options": {"apiKey": "k"}}}));
        mine.insert("model".into(), json!("x/y"));
        let added = with_opencode_provider(Some(mine), entry.clone()).unwrap();
        assert_eq!(added["provider"].as_object().unwrap().keys().collect::<Vec<_>>(), ["x", "pygg"]);
        assert!(opencode_has_provider(&Some(added.clone())));
        assert!(with_opencode_provider(Some(added.clone()), entry.clone()).unwrap_err().contains("already has a provider"));
        let removed = without_opencode_provider(added);
        assert_eq!(removed["provider"], json!({"x": {"options": {"apiKey": "k"}}}));
        let alone = without_opencode_provider(with_opencode_provider(None, entry).unwrap());
        assert!(!alone.contains_key("provider") && alone.contains_key("$schema"));

        let homes = Homes { claude: "a".into(), codex: "b".into(), opencode: "c".into() };
        let p = plan(OPENCODE_GATEWAY, &server(None), &homes, Path::new("b")).unwrap();
        assert_eq!((p.kind.as_str(), p.method.as_str(), p.client_name.as_str()), ("gateway", "file", "OpenCode (models)"));
        assert!(p.change.contains("<new-token>") && p.change.contains("\"pygg\""), "{}", p.change);
    }

    #[tokio::test]
    async fn opencodes_models_through_pygg_set_up_and_removed_in_a_scratch_file() {
        let dir = tempfile::tempdir().unwrap();
        let homes = Homes { claude: dir.path().join("c.json"), codex: dir.path().join("x.toml"),
            opencode: dir.path().join("opencode").join("opencode.json") };
        std::fs::create_dir_all(dir.path().join("opencode")).unwrap();
        std::fs::write(&homes.opencode, r#"{"model":"x/y"}"#).unwrap();
        let db = Db(std::sync::Mutex::new(crate::db::open_in_memory().unwrap()));
        let backups = dir.path().join("backups");
        if launch("opencode").is_none() {
            return; // apply needs OpenCode installed, as a user's would be
        }
        let only_auto = Server { models: server(None).models[..1].to_vec(), ..server(None) };
        assert!(apply(OPENCODE_GATEWAY, &only_auto, &homes, &backups, &db, 1).await.unwrap_err().contains("No model"));
        let set = apply(OPENCODE_GATEWAY, &server(None), &homes, &backups, &db, 2).await.unwrap();
        assert_eq!((set.state.as_str(), set.kind.as_str()), ("set_up", "gateway"));
        let config = read_opencode(&homes.opencode).unwrap().unwrap();
        assert_eq!(config["model"], "x/y");
        let token = config["provider"]["pygg"]["options"]["apiKey"].as_str().unwrap().to_string();
        let (tasks, limited): (bool, Option<String>) = db.with(|c| c.query_row(
            "SELECT may_run_tasks, allowed_connections FROM gateway_clients WHERE name = 'OpenCode (models)' AND revoked_at IS NULL",
            [], |r| Ok((r.get(0)?, r.get(1)?)))).unwrap();
        assert!(!tasks && limited.is_none(), "a client like any other: no tasks, every connection");
        assert!(token.starts_with("str-"));
        let gone = remove(OPENCODE_GATEWAY, &homes, &backups, &db, 3).await.unwrap();
        assert_eq!(gone.state, "not_set_up");
        assert_eq!(read_opencode(&homes.opencode).unwrap().unwrap(), serde_json::from_str::<serde_json::Map<String, Value>>(r#"{"model":"x/y"}"#).unwrap());
        let live: i64 = db.with(|c| c.query_row("SELECT COUNT(*) FROM gateway_clients WHERE revoked_at IS NULL", [], |r| r.get(0))).unwrap();
        assert_eq!(live, 0);
    }

    #[test]
    fn opencode_with_comments_is_refused_and_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opencode.json");
        let text = "{\n  // my model\n  \"model\": \"x/y\"\n}\n";
        std::fs::write(&path, text).unwrap();
        assert!(read_opencode(&path).unwrap_err().contains("not plain JSON"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    }

    struct Db(std::sync::Mutex<Connection>);

    impl Store for Db {
        fn with<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
            f(&self.0.lock().unwrap())
        }
    }

    /// Runs the real agents installed here, each pointed at a scratch configuration folder through its own variable
    /// (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `XDG_CONFIG_HOME`), so the user's own configuration is not touched.
    /// `cargo test --lib integrations::tests::live -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn live_set_up_check_and_remove_in_scratch_configurations() {
        let dir = tempfile::tempdir().unwrap();
        for (var, sub) in [("CLAUDE_CONFIG_DIR", "claude"), ("CODEX_HOME", "codex"), ("XDG_CONFIG_HOME", "xdg")] {
            std::fs::create_dir_all(dir.path().join(sub)).unwrap();
            std::env::set_var(var, dir.path().join(sub));
        }
        std::fs::write(dir.path().join("codex").join("config.toml"), "# mine\nmodel = \"gpt-5\"\n").unwrap();
        let homes = Homes::from_env();
        assert!(homes.codex.starts_with(dir.path()) && homes.claude.starts_with(dir.path()));
        let db = Db(std::sync::Mutex::new(crate::db::open_in_memory().unwrap()));
        let exe = std::env::current_exe().unwrap().to_string_lossy().into_owned();
        let server = Server { program: exe, args: vec!["mcp".into()], ..Server::default() };
        let backups = dir.path().join("backups");
        for (id, name, program) in TARGETS {
            if launch(program).is_none() {
                println!("{name}: not installed, skipped");
                continue;
            }
            let set = apply(id, &server, &homes, &backups, &db, 1).await;
            println!("{name}: apply -> {set:?}");
            let set = set.unwrap();
            assert_eq!(set.state, "set_up");
            let token_count: i64 = db.with(|c| c.query_row(
                "SELECT COUNT(*) FROM gateway_clients WHERE name = ?1 AND revoked_at IS NULL AND may_run_tasks = 1",
                [client_name(name)], |r| r.get(0))).unwrap();
            assert_eq!(token_count, 1);
            assert!(apply(id, &server, &homes, &backups, &db, 2).await.unwrap_err().contains("already set up"));
            let config = std::fs::read_to_string(homes.config(id)).unwrap();
            println!("{name}: {} now has the entry: {}", homes.config(id).display(), config.contains(SERVER));
            if id == "codex" {
                assert!(config.contains("# mine"), "Codex's own comment kept: {config}");
            }
            let gone = remove(id, &homes, &backups, &db, 3).await.unwrap();
            println!("{name}: remove -> {}", gone.state);
            assert_eq!(gone.state, "not_set_up");
            let live: i64 = db.with(|c| c.query_row("SELECT COUNT(*) FROM gateway_clients WHERE revoked_at IS NULL",
                [], |r| r.get(0))).unwrap();
            assert_eq!(live, 0, "the token is revoked");
        }
        println!("backups: {:?}", std::fs::read_dir(&backups).map(|d| d.count()).unwrap_or(0));
    }

    #[test]
    fn a_backup_is_a_copy_and_a_failed_change_is_put_back() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.toml");
        assert_eq!(back_up(&config, &dir.path().join("b"), "codex", 1).unwrap(), None);
        std::fs::write(&config, "model = \"x\"\n").unwrap();
        let before = std::fs::read(&config).ok();
        let backup = back_up(&config, &dir.path().join("b"), "codex", 2).unwrap().unwrap();
        assert!(backup.ends_with("codex-2-config.toml"));
        std::fs::write(&config, "broken").unwrap();
        restore_if_changed(&config, Some(&backup), &before);
        assert_eq!(std::fs::read_to_string(&config).unwrap(), "model = \"x\"\n");
        // A file that did not exist before goes away again.
        let fresh = dir.path().join("new.json");
        std::fs::write(&fresh, "{}").unwrap();
        restore_if_changed(&fresh, None, &None);
        assert!(!fresh.exists());
    }
}
