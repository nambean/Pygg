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
//! - **Codex's models through Pygg** (`codex-gateway`, ADR-033's follow-up) is its twin for Codex CLI: a `pygg`
//!   model provider in Codex's `config.toml` speaking the Responses API, with its own client token and the session
//!   header, plus the profile file `pygg.config.toml` (Codex 0.134+) using it with `auto`. Codex's default `model`
//!   and `model_provider` are never changed: the user picks Pygg per run (`codex --profile pygg`). Codex has no command for this, so Pygg edits the
//!   TOML itself with `toml_edit`, which keeps every other key, table and comment where it was, and refuses a file it
//!   cannot parse or a provider table written inline.
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

/// Codex's models through Pygg's gateway: a model provider and a profile in `config.toml`, not the MCP server.
pub const CODEX_GATEWAY: &str = "codex-gateway";
const CODEX_GATEWAY_TARGET: (&str, &str, &str) = (CODEX_GATEWAY, "Codex: models through Pygg", "codex");
/// The profile Pygg adds next to its provider, so `codex --profile pygg` uses it with `auto`.
pub const CODEX_PROFILE: &str = "pygg";

fn target(id: &str) -> Result<(&'static str, &'static str, &'static str), String> {
    match id {
        OPENCODE_GATEWAY => Ok(OPENCODE_GATEWAY_TARGET),
        CODEX_GATEWAY => Ok(CODEX_GATEWAY_TARGET),
        _ => TARGETS.iter().copied().find(|t| t.0 == id).ok_or_else(|| format!("Pygg cannot set up {id}")),
    }
}

/// Every integration id, in the order shown.
pub fn ids() -> Vec<&'static str> {
    TARGETS.iter().map(|t| t.0).chain([OPENCODE_GATEWAY, CODEX_GATEWAY]).collect()
}

fn kind_of(id: &str) -> &'static str {
    if id == OPENCODE_GATEWAY || id == CODEX_GATEWAY { "gateway" } else { "mcp" }
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
            "codex" | CODEX_GATEWAY => &self.codex,
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

/// `auto`, `auto/cheap`, then the enabled models of enabled connections, leaving out those known not to take tools:
/// a coding agent cannot work with them (the owner's OpenRouter connection listed 66 of 431). A model whose tool
/// support is unknown stays: Pygg does not guess.
pub fn gateway_models(conn: &Connection) -> Vec<GatewayModel> {
    let enabled: std::collections::HashSet<String> = store::list_providers(conn).unwrap_or_default().into_iter()
        .filter(|p| p.enabled).map(|p| p.id).collect();
    let aliases = crate::gateway::auto::ALIASES.iter().map(|a| GatewayModel { id: a.to_string(), context_tokens: None,
        max_output_tokens: None, supports_tools: Some(true) });
    let models = store::list_models(conn, None).unwrap_or_default().into_iter()
        .filter(|m| m.enabled && enabled.contains(&m.provider_id) && m.capabilities.supports_tools != Some(false))
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
    match id {
        OPENCODE_GATEWAY => "OpenCode (models)".into(),
        CODEX_GATEWAY => "Codex (models)".into(),
        _ => client_name(name),
    }
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
/// connection (translating where the connection speaks OpenAI's), and sends `PYGG_ATTEMPT_ID` as its session header:
/// the id Pygg gives a task step, or a launch session from `pygg run opencode`. Started any other way, the variable is
/// empty and the gateway ignores the header.
pub fn opencode_provider(server: &Server, token: &str) -> Value {
    let models = opencode_models(&server.models);
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

/// The `models` of Pygg's provider in OpenCode.
fn opencode_models(list: &[GatewayModel]) -> Value {
    let mut models = serde_json::Map::new();
    for m in list {
        let mut entry = serde_json::Map::new();
        entry.insert("name".into(), json!(m.id));
        // OpenCode offers no tools to a model marked without them; only a model known not to take them is.
        entry.insert("tool_call".into(), json!(m.supports_tools != Some(false)));
        if let (Some(context), Some(output)) = (m.context_tokens, m.max_output_tokens) {
            entry.insert("limit".into(), json!({"context": context, "output": output}));
        }
        models.insert(m.id.clone(), Value::Object(entry));
    }
    Value::Object(models)
}

/// Keeps the model list of OpenCode's Pygg provider current, so the user never sets it up again after a change in
/// Connections or after the reference catalog filled what a provider leaves out (ADR-031). Only an entry Pygg set
/// up is touched, and only its `models`: its address, token and everything else in the file stay as they are. No
/// write when nothing changed. Returns whether the file was written.
pub fn sync_opencode_models(db: &impl Store, homes: &Homes) -> Result<bool, String> {
    if saved(db, OPENCODE_GATEWAY).is_none() {
        return Ok(false);
    }
    let wanted = opencode_models(&db.with(gateway_models));
    let Some(mut config) = read_opencode(&homes.opencode)? else { return Ok(false) };
    let Some(Value::Object(entry)) = config.get_mut("provider").and_then(|p| p.get_mut(PROVIDER)) else { return Ok(false) };
    if entry.get("models") == Some(&wanted) {
        return Ok(false);
    }
    entry.insert("models".into(), wanted);
    write_opencode(&homes.opencode, &config)?;
    Ok(true)
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

/// Whether `pygg run <tool>` can start OpenCode or Codex through Pygg: their Pygg provider is in their configuration
/// (and, for Codex, Pygg's profile file is there), as CLI Tools → Set Up leaves them. Says how to set it up if not.
pub fn launch_ready(tool: &str, homes: &Homes) -> Result<(), String> {
    match tool {
        "opencode" => {
            let config = read_opencode(&homes.opencode)?;
            if opencode_has_provider(&config) { Ok(()) } else {
                Err("OpenCode's Pygg provider is not set up: open Pygg → CLI Tools → OpenCode → Set Up, or run `pygg setup opencode`.".into())
            }
        }
        "codex" => {
            let doc = read_codex(&homes.codex)?;
            if codex_has_provider(&doc) && codex_profile_path(&homes.codex).is_file() { Ok(()) } else {
                Err("Codex's Pygg provider is not set up: open Pygg → CLI Tools → Codex → Set Up, or run `pygg setup codex`.".into())
            }
        }
        other => Err(format!("pygg run does not start {other} this way")),
    }
}

/// Reads Codex's `config.toml`. A missing file is an empty configuration; a file that is not valid TOML is refused,
/// naming only the line (the file may hold other secrets, so none of it is quoted).
pub fn read_codex(path: &Path) -> Result<Option<toml_edit::DocumentMut>, String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("Could not read {}: {e}", path.display())),
    };
    let refused = |why: String| format!("{} {why}, so Pygg will not rewrite it. Add the provider by hand, as shown in \
        docs/AGENTS.md (Codex through Pygg's gateway).", path.display());
    let text = String::from_utf8(bytes).map_err(|_| refused("is not UTF-8 text".into()))?;
    text.parse::<toml_edit::DocumentMut>().map(Some).map_err(|e| {
        let line = e.span().map(|s| text[..s.start.min(text.len())].matches('\n').count() + 1);
        refused(match line { Some(l) => format!("is not valid TOML (line {l})"), None => "is not valid TOML".into() })
    })
}

/// Codex's `[model_providers.pygg]`: Pygg's gateway over the Responses API (ADR-033), with the client token as the
/// bearer and `PYGG_ATTEMPT_ID` as its session header: the id Pygg gives a task step, or a launch session from
/// `pygg run codex`. Codex sends no header when the variable is unset.
fn codex_provider(server: &Server, token: &str) -> toml_edit::Table {
    let mut t = toml_edit::Table::new();
    t["name"] = toml_edit::value("Pygg");
    t["base_url"] = toml_edit::value(format!("{}/v1", server.gateway_url.trim_end_matches('/')));
    t["wire_api"] = toml_edit::value("responses");
    t["experimental_bearer_token"] = toml_edit::value(token);
    let mut headers = toml_edit::InlineTable::new();
    headers.insert(crate::gateway::SESSION_HEADER, agents::ATTEMPT_ENV.into());
    t["env_http_headers"] = toml_edit::value(headers);
    t
}

/// Codex's profile file for Pygg, `pygg.config.toml` next to `config.toml`: Codex 0.134 and later read a profile
/// from `<name>.config.toml` and refuse `--profile <name>` while `config.toml` still has a `[profiles.<name>]` table.
/// It selects Pygg's provider with `auto`, so the user's own default stays as it is.
pub fn codex_profile_path(config: &Path) -> PathBuf {
    config.with_file_name(format!("{CODEX_PROFILE}.config.toml"))
}

fn codex_profile_text() -> String {
    format!("# Codex's models through Pygg's gateway, added by Pygg (Agents > From Your IDE): codex --profile \
        {CODEX_PROFILE}\nmodel_provider = \"{PROVIDER}\"\nmodel = \"auto\"\n")
}

/// Whether a profile file is still the one Pygg wrote: it selects Pygg's provider. One the user pointed elsewhere
/// is theirs.
fn codex_profile_is_ours(path: &Path) -> bool {
    std::fs::read_to_string(path).ok().and_then(|t| t.parse::<toml_edit::DocumentMut>().ok())
        .is_some_and(|d| d.get("model_provider").and_then(|v| v.as_str()) == Some(PROVIDER))
}

/// The table under `key` that Pygg may add to: created (without a header of its own) when missing, refused when it
/// is written inline or is not a table.
fn codex_table<'a>(doc: &'a mut toml_edit::DocumentMut, key: &str) -> Result<&'a mut toml_edit::Table, String> {
    if doc.get(key).is_none() {
        let mut t = toml_edit::Table::new();
        t.set_implicit(true);
        doc.insert(key, toml_edit::Item::Table(t));
    }
    doc.get_mut(key).and_then(|i| i.as_table_mut()).ok_or_else(|| format!("Codex's \"{key}\" is written inline or is \
        not a table, so Pygg will not add to it. Add the provider by hand."))
}

/// The configuration with Pygg's provider added at its end, everything else left as it was. Refused when Codex
/// already has a provider of that name, or a legacy `[profiles.pygg]` table (Codex would refuse the profile).
pub fn with_codex_provider(doc: Option<toml_edit::DocumentMut>, server: &Server, token: &str)
    -> Result<toml_edit::DocumentMut, String> {
    let mut doc = doc.unwrap_or_default();
    if doc.get("profiles").and_then(|p| p.get(CODEX_PROFILE)).is_some() {
        return Err(format!("Codex's config.toml has a [profiles.{CODEX_PROFILE}] table, which Codex no longer reads and \
            which stops codex --profile {CODEX_PROFILE} from working. Remove or rename it, then set up again."));
    }
    // Nothing but comments (which toml_edit keeps after the last table): no blank line above Pygg's block.
    let fresh = doc.as_table().is_empty();
    let last = doc.as_table().iter().filter_map(|(_, i)| i.as_table()).flat_map(table_positions).max().unwrap_or(0);
    let mut provider = codex_provider(server, token);
    provider.set_position(Some(last + 1));
    let gap = if fresh { "" } else { "\n" };
    provider.decor_mut().set_prefix(format!("{gap}# Pygg's gateway, added by Pygg (Agents > From Your IDE). Use it with: \
        codex --profile {CODEX_PROFILE}\n"));
    let providers = codex_table(&mut doc, "model_providers")?;
    if providers.contains_key(PROVIDER) {
        return Err(format!("Codex already has a model provider named {PROVIDER}"));
    }
    providers.insert(PROVIDER, toml_edit::Item::Table(provider));
    Ok(doc)
}

/// The positions of a table and of every table under it, as the parser numbered them.
fn table_positions(t: &toml_edit::Table) -> Vec<isize> {
    let mut out: Vec<isize> = t.position().into_iter().collect();
    for (_, item) in t.iter() {
        match item {
            toml_edit::Item::Table(child) => out.extend(table_positions(child)),
            toml_edit::Item::ArrayOfTables(a) => a.iter().for_each(|child| out.extend(table_positions(child))),
            _ => {}
        }
    }
    out
}

/// The configuration without Pygg's provider (and without `model_providers` when Pygg's was all it held and it had
/// no header of its own). Refused while Codex's default provider is Pygg's: Codex would not start without it.
pub fn without_codex_provider(mut doc: toml_edit::DocumentMut) -> Result<toml_edit::DocumentMut, String> {
    if doc.get("model_provider").and_then(|v| v.as_str()) == Some(PROVIDER) {
        return Err(format!("Codex's default model_provider is {PROVIDER}. Set it back to another provider in config.toml \
            first, or Codex would not start once Pygg's provider is gone."));
    }
    let Some(providers) = doc.get_mut("model_providers").and_then(|i| i.as_table_mut()) else { return Ok(doc) };
    providers.remove(PROVIDER);
    if providers.is_empty() && (providers.is_implicit() || providers.is_dotted()) {
        doc.remove("model_providers");
    }
    Ok(doc)
}

fn codex_has_provider(doc: &Option<toml_edit::DocumentMut>) -> bool {
    doc.as_ref().and_then(|d| d.get("model_providers")).and_then(|p| p.get(PROVIDER)).is_some()
}

/// What Pygg adds: the provider at the end of `config.toml`, and the profile file next to it.
fn codex_change(server: &Server, token: &str) -> Result<String, String> {
    let provider = with_codex_provider(None, server, token)?.to_string();
    Ok(format!("{provider}\n# A new file next to it: {CODEX_PROFILE}.config.toml\n{}", codex_profile_text()))
}

/// Writes Codex's profile file, refusing to replace one that is there.
fn write_codex_profile(path: &Path) -> Result<(), String> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|e| format!("Could not create {}: {e}", path.display()))?;
    file.write_all(codex_profile_text().as_bytes()).map_err(|e| format!("Could not write {}: {e}", path.display()))
}

/// Writes a file whole: to a temporary file next to it, then renamed over it.
fn write_atomically(path: &Path, text: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "config".into());
    let tmp = path.with_file_name(format!("{name}.pygg-tmp"));
    std::fs::write(&tmp, text).map_err(|e| format!("Could not write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Could not replace {}: {e}", path.display())
    })
}

/// Writes Codex's configuration as `toml_edit` renders it, which keeps the bytes of what Pygg did not touch except
/// line endings: it writes LF, so a file that had CRLF (Notepad's) gets CRLF back.
fn write_codex(path: &Path, doc: &toml_edit::DocumentMut) -> Result<(), String> {
    let crlf = std::fs::read(path).is_ok_and(|b| b.windows(2).any(|w| w == b"\r\n"));
    let text = doc.to_string();
    let text = if crlf { text.replace("\r\n", "\n").replace('\n', "\r\n") } else { text };
    write_atomically(path, &text)
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
    if id == CODEX_GATEWAY {
        return match read_codex(&homes.codex) {
            Ok(config) => (Some(codex_has_provider(&config)), None),
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
    // All at once, in the original order: `claude mcp get` alone takes seconds, as it checks the server.
    futures_util::future::join_all(records.into_iter().map(|(id, rec)| async move {
        let rec = rec.filter(|s| !s.is_empty()).and_then(|s| serde_json::from_str(&s).ok());
        describe(&id, homes, rec).await.ok()
    })).await.into_iter().flatten().collect()
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
    } else if id == CODEX_GATEWAY {
        (codex_change(server, TOKEN_PLACEHOLDER)?, "file")
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

/// A token that may start tasks and reaches no model connection: the MCP server only uses the task API. For an
/// agent's models through Pygg (OpenCode's, Codex's), a client like any other: it reaches the connections, may not
/// start tasks, and its limits are set in Clients.
fn create_client(db: &impl Store, id: &str, name: &str) -> Result<(String, String), String> {
    db.with(|c| {
        let (client, token) = store::create_client(c, name, None)?;
        if kind_of(id) == "mcp" {
            store::set_client_may_run_tasks(c, &client.id, true)?;
            store::set_client_connections(c, &client.id, Some(&[]))?;
        }
        Ok((client.id, token))
    })
}

fn revoke(db: &impl Store, client_id: &str) {
    let _ = db.with(|c| store::revoke_client(c, client_id));
}

/// Removes Pygg's profile file after backing it up, unless the user pointed it at another provider.
fn remove_codex_profile(config: &Path, backup_dir: &Path, now_ms: i64) -> Result<(), String> {
    let profile = codex_profile_path(config);
    if codex_profile_is_ours(&profile) {
        back_up(&profile, backup_dir, CODEX_GATEWAY, now_ms)?;
        std::fs::remove_file(&profile).map_err(|e| format!("Could not remove {}: {e}", profile.display()))?;
    }
    Ok(())
}

/// How an integration changes the agent's configuration.
enum Edit {
    /// The agent's own command.
    Command,
    /// Pygg edits `opencode.json` (`None`: no file yet).
    OpenCode(Option<serde_json::Map<String, Value>>),
    /// Pygg edits Codex's `config.toml` (`None`: no file yet).
    Codex(Option<toml_edit::DocumentMut>),
}

/// Sets the integration up: backup, a new token, the agent's own command (or Pygg's edit of OpenCode's file), and
/// a check that the agent now lists the server. Any failure undoes what was done.
pub async fn apply(id: &str, server: &Server, homes: &Homes, backup_dir: &Path, db: &impl Store, now_ms: i64)
    -> Result<Integration, String> {
    let (id, name, program) = target(id)?;
    let launch = launch(program).ok_or_else(|| format!("{name} is not installed on this computer"))?;
    let existing = saved(db, id);
    let what = if kind_of(id) == "gateway" { "a provider" } else { "an MCP server" };
    match registered(id, homes).await {
        (Some(true), _) if existing.is_some() => return Err(format!("Pygg is already set up in {name}")),
        (Some(true), _) => return Err(format!("{name} already has {what} named {SERVER}, set up outside \
            Pygg. Remove it here first, or keep using it.")),
        (None, Some(why)) => return Err(why),
        (None, None) => return Err(format!("{name} did not say whether it has {what} named {SERVER}")),
        _ => {}
    }
    if let Some(old) = &existing {
        revoke(db, &old.client_id); // removed outside Pygg: its token is no longer used
    }
    let config = homes.config(id).to_path_buf();
    // A file Pygg edits itself is checked before anything is created.
    let edit = match id {
        "opencode" | OPENCODE_GATEWAY => Edit::OpenCode(read_opencode(&config)?),
        CODEX_GATEWAY => {
            let profile = codex_profile_path(&config);
            if profile.exists() {
                return Err(format!("Codex already has a profile file {}. Rename or remove it, then set up again.",
                    profile.display()));
            }
            Edit::Codex(read_codex(&config)?)
        }
        _ => Edit::Command,
    };
    if kind_of(id) == "gateway" && server.models.len() <= crate::gateway::auto::ALIASES.len() {
        return Err("No model is enabled in Connections yet, so auto would have nothing to route to. Add a connection \
            first.".into());
    }
    let before = std::fs::read(&config).ok();
    let backup = back_up(&config, backup_dir, id, now_ms)?;
    let cname = client_name_for(id, name);
    let (client_id, token) = create_client(db, id, &cname)?;
    let profile = (id == CODEX_GATEWAY).then(|| codex_profile_path(&config));
    let undo = |why: String| {
        restore_if_changed(&config, backup.as_deref(), &before);
        if let Some(p) = profile.as_deref().filter(|p| codex_profile_is_ours(p)) {
            let _ = std::fs::remove_file(p);
        }
        revoke(db, &client_id);
        why
    };
    match edit {
        Edit::OpenCode(current) if id == OPENCODE_GATEWAY => {
            let updated = with_opencode_provider(current, opencode_provider(server, &token)).map_err(&undo)?;
            write_opencode(&config, &updated).map_err(&undo)?;
        }
        Edit::OpenCode(current) => {
            let updated = with_opencode_entry(current, opencode_entry(server, &token)).map_err(&undo)?;
            write_opencode(&config, &updated).map_err(&undo)?;
        }
        Edit::Codex(current) => {
            let updated = with_codex_provider(current, server, &token).map_err(&undo)?;
            write_codex(&config, &updated).map_err(&undo)?;
            write_codex_profile(&codex_profile_path(&config)).map_err(&undo)?;
        }
        Edit::Command => {
            let out = run(&launch, add_args(id, server, &token)).await;
            if !out.succeeded() {
                let why = one_line(&out, &[&token]).unwrap_or_else(|| "no output".into());
                return Err(undo(format!("{name} refused the setup: {why}")));
            }
        }
    }
    if registered(id, homes).await.0 != Some(true) {
        if kind_of(id) == "mcp" && id != "opencode" {
            let _ = run(&launch, remove_args(id)).await;
        }
        return Err(undo(format!("{name} does not list the {SERVER} entry after the setup, so it was undone")));
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
        } else if id == CODEX_GATEWAY {
            let current = read_codex(&config)?.unwrap_or_default();
            write_codex(&config, &without_codex_provider(current)?)?;
            remove_codex_profile(&config, backup_dir, now_ms)?;
        } else {
            let launch = launch(program).ok_or_else(|| format!("{name} is not installed on this computer"))?;
            let out = run(&launch, remove_args(id)).await;
            if !out.succeeded() {
                return Err(format!("{name} refused to remove it: {}", one_line(&out, &[]).unwrap_or_default()));
            }
        }
    }
    if id == CODEX_GATEWAY && present == Some(false) {
        // The provider was taken out by hand: Pygg's profile file, if still there, would only fail.
        remove_codex_profile(homes.config(id), backup_dir, now_ms)?;
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

    #[test]
    fn opencode_is_told_about_enabled_models_except_those_known_not_to_take_tools() {
        let conn = crate::db::open_in_memory().unwrap();
        let p = store::add_provider(&conn, &store::NewProvider { kind: "openrouter", label: "OpenRouter",
            endpoint: "https://openrouter.ai/api/v1", credential_ref: None, default_billing: "paid", wire: "openai" }).unwrap();
        for (model, tools) in [("a", Some(true)), ("b", Some(false)), ("c", None), ("d", Some(true))] {
            store::add_model(&conn, &p.id, model).unwrap();
            store::set_capabilities(&conn, &p.id, model, &store::Capabilities { supports_tools: tools,
                context_tokens: Some(1000), ..store::Capabilities::default() }).unwrap();
        }
        store::update_model(&conn, &p.id, "d", Some(false), None, None).unwrap();
        let ids: Vec<String> = gateway_models(&conn).into_iter().map(|m| m.id).collect();
        assert_eq!(ids, ["auto".to_string(), "auto/cheap".into(), format!("{}/a", p.id), format!("{}/c", p.id)]);
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
    fn opencodes_model_list_follows_connections_without_touching_anything_else() {
        let dir = tempfile::tempdir().unwrap();
        let homes = Homes { claude: dir.path().join("c.json"), codex: dir.path().join("x.toml"),
            opencode: dir.path().join("opencode.json") };
        let db = Db(std::sync::Mutex::new(crate::db::open_in_memory().unwrap()));
        let p = db.with(|c| store::add_provider(c, &store::NewProvider { kind: "openrouter", label: "OpenRouter",
            endpoint: "https://openrouter.ai/api/v1", credential_ref: None, default_billing: "paid", wire: "openai" })).unwrap();
        for model in ["a", "b"] {
            db.with(|c| store::add_model(c, &p.id, model)).unwrap();
        }
        let old = json!({"$schema": "s", "model": "x/y", "provider": {"pygg": {"npm": "@ai-sdk/anthropic",
            "options": {"baseURL": "http://127.0.0.1:8741/v1", "apiKey": "str-kept"}, "models": {"auto": {"name": "auto"}}},
            "other": {"models": {"m": {}}}}});
        std::fs::write(&homes.opencode, serde_json::to_string_pretty(&old).unwrap()).unwrap();

        // Not set up by Pygg: an entry named pygg made elsewhere is not Pygg's to change.
        assert_eq!(sync_opencode_models(&db, &homes), Ok(false));
        save(&db, OPENCODE_GATEWAY, Some(&Record { client_id: "c".into(), client_name: "OpenCode (models)".into(),
            set_up_at: 1, backup: None })).unwrap();

        assert_eq!(sync_opencode_models(&db, &homes), Ok(true));
        let config = read_opencode(&homes.opencode).unwrap().unwrap();
        let ids: Vec<String> = config["provider"]["pygg"]["models"].as_object().unwrap().keys().cloned().collect();
        assert_eq!(ids, ["auto".to_string(), "auto/cheap".into(), format!("{}/a", p.id), format!("{}/b", p.id)]);
        assert_eq!(config["provider"]["pygg"]["options"], old["provider"]["pygg"]["options"], "the token and address stay");
        assert_eq!((&config["model"], &config["provider"]["other"]), (&old["model"], &old["provider"]["other"]));

        // Nothing changed: no write.
        let written = std::fs::metadata(&homes.opencode).unwrap().modified().unwrap();
        assert_eq!(sync_opencode_models(&db, &homes), Ok(false));
        assert_eq!(std::fs::metadata(&homes.opencode).unwrap().modified().unwrap(), written);

        // A model found not to take tools (the reference catalog, or the user) leaves the list.
        db.with(|c| store::set_capabilities(c, &p.id, "b", &store::Capabilities { supports_tools: Some(false),
            ..store::Capabilities::default() })).unwrap();
        assert_eq!(sync_opencode_models(&db, &homes), Ok(true));
        let config = read_opencode(&homes.opencode).unwrap().unwrap();
        assert!(config["provider"]["pygg"]["models"].get(format!("{}/b", p.id)).is_none());

        // Removed from the file outside Pygg: nothing is added back.
        std::fs::write(&homes.opencode, r#"{"model":"x/y"}"#).unwrap();
        assert_eq!(sync_opencode_models(&db, &homes), Ok(false));
        assert_eq!(std::fs::read_to_string(&homes.opencode).unwrap(), r#"{"model":"x/y"}"#);
    }

    #[test]
    fn pygg_run_starts_opencode_and_codex_only_once_they_are_set_up() {
        let dir = tempfile::tempdir().unwrap();
        let homes = Homes { claude: dir.path().join(".claude.json"), codex: dir.path().join("config.toml"),
            opencode: dir.path().join("opencode.json") };
        assert!(launch_ready("opencode", &homes).unwrap_err().contains("pygg setup opencode"));
        assert!(launch_ready("codex", &homes).unwrap_err().contains("pygg setup codex"));
        write_opencode(&homes.opencode, &with_opencode_provider(None, opencode_provider(&server(None), "str-o")).unwrap()).unwrap();
        assert_eq!(launch_ready("opencode", &homes), Ok(()));
        // Codex needs its provider and Pygg's profile file, which `--profile pygg` reads.
        write_codex(&homes.codex, &with_codex_provider(None, &server(None), "str-c").unwrap()).unwrap();
        assert!(launch_ready("codex", &homes).is_err());
        write_codex_profile(&codex_profile_path(&homes.codex)).unwrap();
        assert_eq!(launch_ready("codex", &homes), Ok(()));
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

    /// Codex configurations as users write them: comments everywhere, other providers and profiles, MCP servers,
    /// Windows paths in both string kinds, dotted keys, CRLF, no final newline.
    const CODEX_FIXTURES: [&str; 7] = [
        "# my Codex\nmodel = \"gpt-5.5\" # the default\nmodel_provider = \"openai\"\napproval_policy = \"on-request\"\n\n\
         [model_providers.ollama]\n# local\nname = \"Ollama\"\nbase_url = \"http://localhost:11434/v1\"\n\n\
         [profiles.fast]\nmodel = \"gpt-5.5-mini\"\nmodel_reasoning_effort = \"low\"\n\n\
         [mcp_servers.docs]\ncommand = 'C:\\Program Files\\Docs\\docs.exe'\nargs = [\"--root\", \"C:\\\\Users\\\\me\\\\docs\"]\n\
         env = { DOCS_KEY = \"k\" }\n\n[windows]\nsandbox = \"unelevated\"\n\n# the end\n",
        "model = \"o4\"\r\n\r\n# providers\r\n[model_providers.azure]\r\nname = \"Azure\"\r\nbase_url = \"https://x.openai.azure.com/openai\"\r\n\
         env_key = \"AZURE_KEY\"\r\n\r\n[projects.'C:\\Users\\me\\repo']\r\ntrust_level = \"trusted\"\r\n",
        "model = \"gpt-5.5\"\n[profiles.deep]\nmodel_reasoning_effort = \"high\"",
        "",
        "# only a comment\n",
        "model = \"gpt-5.5\"\nmodel_providers.lm.name = \"LM Studio\"\nmodel_providers.lm.base_url = \"http://localhost:1234/v1\"\n\
         profiles.x.model = \"y\"\n",
        "[model_providers]\n\n[profiles]\n\n[tui]\nnotifications = true\n",
    ];

    #[test]
    fn codex_gets_pyggs_provider_and_profile_and_loses_them_leaving_its_file_byte_identical() {
        for (n, before) in CODEX_FIXTURES.iter().enumerate() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("config.toml");
            std::fs::write(&path, before).unwrap();
            let doc = read_codex(&path).unwrap();
            assert!(!codex_has_provider(&doc), "fixture {n}");
            write_codex(&path, &with_codex_provider(doc, &server(None), "str-c").unwrap()).unwrap();
            let after = std::fs::read_to_string(&path).unwrap();
            // The user's part comes first, unchanged; a comment that ended the file still ends it.
            let trailing = ["# the end\n", "# only a comment\n"].into_iter().find(|t| before.ends_with(t)).unwrap_or("");
            assert!(after.starts_with(before[..before.len() - trailing.len()].trim_end()), "fixture {n}:\n{after}");
            assert!(after.ends_with(trailing), "fixture {n}:\n{after}");
            let doc = read_codex(&path).unwrap();
            assert!(codex_has_provider(&doc), "fixture {n}");
            let d = doc.clone().unwrap();
            let p = &d["model_providers"]["pygg"];
            assert_eq!(p["base_url"].as_str(), Some("http://127.0.0.1:8741/v1"));
            assert_eq!(p["wire_api"].as_str(), Some("responses"));
            assert_eq!(p["experimental_bearer_token"].as_str(), Some("str-c"));
            assert_eq!(p["env_http_headers"]["x-smarttaskrouter-session"].as_str(), Some("PYGG_ATTEMPT_ID"));
            // The user's own default stays theirs.
            assert_eq!(d.get("model_provider").and_then(|v| v.as_str()), if n == 0 { Some("openai") } else { None });
            assert!(with_codex_provider(doc.clone(), &server(None), "t").unwrap_err().contains("already has a model provider"));
            write_codex(&path, &without_codex_provider(doc.unwrap()).unwrap()).unwrap();
            let back = std::fs::read_to_string(&path).unwrap();
            if n == 2 {
                // No final newline: toml_edit ends the last line, the only byte that changes.
                assert_eq!(back, format!("{before}\n"), "fixture {n}");
            } else {
                assert_eq!(back, *before, "fixture {n}");
            }
        }
    }

    #[test]
    fn codex_files_pygg_cannot_edit_safely_are_refused_and_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let broken = "model = \"gpt-5.5\"\n[model_providers.x]\nexperimental_bearer_token = \"secret-1\nname = \"x\"\n";
        std::fs::write(&path, broken).unwrap();
        let why = read_codex(&path).unwrap_err();
        assert!(why.contains("not valid TOML (line 3)") && !why.contains("secret-1"), "{why}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);

        let inline: toml_edit::DocumentMut = "model_providers = { other = { name = \"o\" } }\n".parse().unwrap();
        assert!(with_codex_provider(Some(inline), &server(None), "t").unwrap_err().contains("inline"));
        // A legacy [profiles.pygg] would make Codex refuse --profile pygg.
        let legacy: toml_edit::DocumentMut = "[profiles.pygg]\nmodel = \"gpt-5.5\"\n".parse().unwrap();
        assert!(with_codex_provider(Some(legacy), &server(None), "t").unwrap_err().contains("[profiles.pygg]"));

        // Removing Pygg's provider while it is Codex's default would leave Codex unable to start.
        let default = with_codex_provider(Some("model_provider = \"pygg\"\n".parse().unwrap()), &server(None), "t").unwrap();
        assert!(without_codex_provider(default).unwrap_err().contains("default model_provider"));
        let alone = without_codex_provider(with_codex_provider(None, &server(None), "t").unwrap()).unwrap();
        assert!(alone.get("model_providers").is_none() && alone.to_string().is_empty());
    }

    #[test]
    fn codexs_plan_is_the_toml_block_with_a_placeholder() {
        let homes = Homes { claude: "a".into(), codex: "b".into(), opencode: "c".into() };
        let p = plan(CODEX_GATEWAY, &server(Some("http://127.0.0.1:8800")), &homes, Path::new("b")).unwrap();
        assert_eq!((p.kind.as_str(), p.method.as_str(), p.client_name.as_str()), ("gateway", "file", "Codex (models)"));
        assert!(p.change.starts_with("# Pygg's gateway"), "{}", p.change);
        assert!(p.change.contains("[model_providers.pygg]") && !p.change.contains("[profiles"), "{}", p.change);
        assert!(p.change.contains("pygg.config.toml\n") && p.change.ends_with("model_provider = \"pygg\"\nmodel = \"auto\"\n"),
            "{}", p.change);
        assert!(p.change.contains("experimental_bearer_token = \"<new-token>\""), "{}", p.change);
        assert_eq!(homes.config(CODEX_GATEWAY), Path::new("b"));
    }

    #[tokio::test]
    async fn codexs_models_through_pygg_set_up_and_removed_in_a_scratch_codex_home() {
        let dir = tempfile::tempdir().unwrap();
        let homes = Homes { claude: dir.path().join("c.json"), codex: dir.path().join("codex").join("config.toml"),
            opencode: dir.path().join("o.json") };
        std::fs::create_dir_all(dir.path().join("codex")).unwrap();
        std::fs::write(&homes.codex, CODEX_FIXTURES[0]).unwrap();
        let db = Db(std::sync::Mutex::new(crate::db::open_in_memory().unwrap()));
        let backups = dir.path().join("backups");
        if launch("codex").is_none() {
            return; // apply needs Codex installed, as a user's would be
        }
        let only_auto = Server { models: server(None).models[..1].to_vec(), ..server(None) };
        assert!(apply(CODEX_GATEWAY, &only_auto, &homes, &backups, &db, 1).await.unwrap_err().contains("No model"));
        let set = apply(CODEX_GATEWAY, &server(None), &homes, &backups, &db, 2).await.unwrap();
        assert_eq!((set.state.as_str(), set.kind.as_str(), set.client_name.as_deref()), ("set_up", "gateway", Some("Codex (models)")));
        let backup = set.backup.clone().unwrap();
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), CODEX_FIXTURES[0]);
        let doc = read_codex(&homes.codex).unwrap().unwrap();
        let token = doc["model_providers"]["pygg"]["experimental_bearer_token"].as_str().unwrap().to_string();
        assert!(token.starts_with("str-"));
        let profile = dir.path().join("codex").join("pygg.config.toml");
        assert_eq!(std::fs::read_to_string(&profile).unwrap(), codex_profile_text());
        let (tasks, limited): (bool, Option<String>) = db.with(|c| c.query_row(
            "SELECT may_run_tasks, allowed_connections FROM gateway_clients WHERE name = 'Codex (models)' AND revoked_at IS NULL",
            [], |r| Ok((r.get(0)?, r.get(1)?)))).unwrap();
        assert!(!tasks && limited.is_none(), "a client like any other: no tasks, every connection");
        assert!(apply(CODEX_GATEWAY, &server(None), &homes, &backups, &db, 3).await.unwrap_err().contains("already set up"));

        // Taken out by hand: Pygg says so.
        std::fs::write(&homes.codex, CODEX_FIXTURES[0]).unwrap();
        let listed = list_with(db.with(records), &homes).await;
        assert_eq!(listed.iter().find(|i| i.id == CODEX_GATEWAY).unwrap().state, "removed_elsewhere");
        write_codex(&homes.codex, &with_codex_provider(read_codex(&homes.codex).unwrap(), &server(None), &token).unwrap()).unwrap();

        // Remove: the provider and Pygg's profile file go, the token is revoked, the file is as it was.
        let gone = remove(CODEX_GATEWAY, &homes, &backups, &db, 4).await.unwrap();
        assert_eq!(gone.state, "not_set_up");
        assert_eq!(std::fs::read_to_string(&homes.codex).unwrap(), CODEX_FIXTURES[0]);
        assert!(!profile.exists());
        let live: i64 = db.with(|c| c.query_row("SELECT COUNT(*) FROM gateway_clients WHERE revoked_at IS NULL", [], |r| r.get(0))).unwrap();
        assert_eq!(live, 0);

        // A profile file of the user's own is never replaced; one the user pointed elsewhere is not removed.
        std::fs::write(&profile, "model = \"gpt-5.5\"\n").unwrap();
        assert!(apply(CODEX_GATEWAY, &server(None), &homes, &backups, &db, 5).await.unwrap_err().contains("profile file"));
        assert_eq!(std::fs::read_to_string(&homes.codex).unwrap(), CODEX_FIXTURES[0], "nothing changed");
        std::fs::remove_file(&profile).unwrap();
        apply(CODEX_GATEWAY, &server(None), &homes, &backups, &db, 6).await.unwrap();
        std::fs::write(&profile, "model_provider = \"openai\"\n").unwrap();
        remove(CODEX_GATEWAY, &homes, &backups, &db, 7).await.unwrap();
        assert_eq!(std::fs::read_to_string(&profile).unwrap(), "model_provider = \"openai\"\n");
        std::fs::remove_file(&profile).unwrap();

        // A provider named pygg that Pygg did not make is shown as such and not set up over.
        std::fs::write(&homes.codex, "[model_providers.pygg]\nname = \"mine\"\n").unwrap();
        let listed = list_with(db.with(records), &homes).await;
        assert_eq!(listed.iter().find(|i| i.id == CODEX_GATEWAY).unwrap().state, "set_up_elsewhere");
        assert!(apply(CODEX_GATEWAY, &server(None), &homes, &backups, &db, 8).await.unwrap_err().contains("a provider named pygg"));
    }

    /// Sets up Codex's models through Pygg for real, against a scratch store served by a scratch gateway and a
    /// scratch `CODEX_HOME`, runs `codex exec --profile pygg` once, then removes it. The caller starts the gateway
    /// (`pygg-cli gateway --db STORE serve --port PORT`) and sets `PYGG_LIVE_DB`, `PYGG_LIVE_PORT` and `CODEX_HOME`
    /// (and `PYGG_LIVE_MODEL` to name a model instead of the profile's `auto`).
    /// `cargo test --lib integrations::tests::live_codex -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn live_codex_models_through_a_scratch_gateway() {
        let store = PathBuf::from(std::env::var("PYGG_LIVE_DB").expect("PYGG_LIVE_DB: a scratch store"));
        let port = std::env::var("PYGG_LIVE_PORT").expect("PYGG_LIVE_PORT");
        let home = PathBuf::from(std::env::var("CODEX_HOME").expect("CODEX_HOME: a scratch folder"));
        let user = PathBuf::from(std::env::var("USERPROFILE").unwrap_or_default());
        assert!(!home.starts_with(user.join(".codex")) && store.file_name().is_some(), "scratch folders only");
        let homes = Homes::from_env();
        assert!(homes.codex.starts_with(&home));
        let before = std::fs::read_to_string(&homes.codex).unwrap();
        let db = Db(std::sync::Mutex::new(crate::db::open(&store).unwrap()));
        let server = Server { gateway_url: format!("http://127.0.0.1:{port}"), models: db.with(gateway_models), ..Server::default() };
        println!("models: {:?}", server.models.iter().map(|m| &m.id).collect::<Vec<_>>());
        let backups = home.join("backups");
        let set = apply(CODEX_GATEWAY, &server, &homes, &backups, &db, crate::db::now_ms()).await.unwrap();
        println!("apply -> {} ({:?}), backup {:?}", set.state, set.client_name, set.backup);
        assert_eq!(set.state, "set_up");

        let codex = launch("codex").expect("Codex installed");
        let mut cmd = std::process::Command::new(&codex.program);
        cmd.args(&codex.prefix).args(["exec", "--profile", "pygg", "--skip-git-repo-check", "--json"]);
        // `auto` refuses a model below the new client's minimum tier (T2); PYGG_LIVE_MODEL names one instead.
        if let Ok(model) = std::env::var("PYGG_LIVE_MODEL") {
            cmd.args(["-m", &model]);
        }
        cmd.arg("Reply with exactly: ok")
            .current_dir(&home).env("CODEX_HOME", &home).env(agents::ATTEMPT_ENV, "live-codex-gateway");
        for (name, _) in std::env::vars() {
            if name.starts_with("CLAUDE") || name.starts_with("ANTHROPIC_") || process::HOST_SESSION_VARS.contains(&name.as_str()) {
                cmd.env_remove(name);
            }
        }
        let out = cmd.output().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        println!("codex exit {:?}\n{stdout}\n{}", out.status.code(), String::from_utf8_lossy(&out.stderr).lines()
            .filter(|l| !l.trim().is_empty()).take(20).collect::<Vec<_>>().join("\n"));

        let gone = remove(CODEX_GATEWAY, &homes, &backups, &db, crate::db::now_ms()).await.unwrap();
        println!("remove -> {}", gone.state);
        assert_eq!(std::fs::read_to_string(&homes.codex).unwrap(), before, "byte-identical after removal");
        let live: i64 = db.with(|c| c.query_row("SELECT COUNT(*) FROM gateway_clients WHERE name = 'Codex (models)' AND \
            revoked_at IS NULL", [], |r| r.get(0))).unwrap();
        assert_eq!(live, 0, "the token is revoked");
        assert!(out.status.success() && stdout.contains("\"text\":\"ok\""), "Codex answered through Pygg");
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
