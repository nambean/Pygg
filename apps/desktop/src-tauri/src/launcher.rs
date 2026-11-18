//! Claude Code through Pygg, one launch at a time (CLI Tools, SCREENS §2.2).
//!
//! Pointing Claude Code's own settings at another endpoint would stop it using the user's Claude plan for every
//! session. The launcher leaves those settings alone: `pygg run claude [args…]` starts Claude Code with the gateway's
//! address, a Pygg API key of its own and the model `auto` in that one process's environment. A plain `claude` stays
//! as it was.
//!
//! Set-up makes the API key "Claude Code (launcher)" and keeps it in the OS credential store, never in a file, so it
//! can be handed to the process at launch; Remove revokes it and deletes it from the store.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::gateway::store;

/// Where the record of Pygg's set-up is kept, in the store's settings.
pub const SETTING: &str = "launcher.claude-code";
pub const CLIENT_NAME: &str = "Claude Code (launcher)";

#[derive(Serialize, Deserialize)]
struct Record {
    client_id: String,
    vault_ref: String,
    set_up_at: i64,
}

/// What CLI Tools shows for the launcher.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub set_up: bool,
    pub client_id: Option<String>,
    pub client_name: Option<String>,
    pub set_up_at: Option<i64>,
    /// Whether Claude Code was found on this computer.
    pub installed: bool,
}

fn record(conn: &Connection) -> Option<Record> {
    store::setting(conn, SETTING).ok().flatten().filter(|s| !s.is_empty()).and_then(|s| serde_json::from_str(&s).ok())
}

pub fn state(conn: &Connection) -> State {
    let installed = crate::agents::locate("claude").is_some();
    match record(conn) {
        Some(r) => State {
            set_up: true,
            client_id: Some(r.client_id),
            client_name: Some(CLIENT_NAME.into()),
            set_up_at: Some(r.set_up_at),
            installed,
        },
        None => State { set_up: false, client_id: None, client_name: None, set_up_at: None, installed },
    }
}

/// Makes the launcher's API key and keeps it in the credential store. Refused when it is already set up.
pub fn set_up(conn: &Connection, now_ms: i64) -> Result<State, String> {
    if record(conn).is_some() {
        return Err("The Claude Code launcher is already set up. Remove it first to make a new key.".into());
    }
    let (client, token) = store::create_client(conn, CLIENT_NAME, None)?;
    let vault_ref = crate::vault::new_client_reference();
    if let Err(e) = crate::vault::store(&vault_ref, &token) {
        let _ = store::revoke_client(conn, &client.id);
        return Err(format!("The key could not be kept in the credential store: {e}"));
    }
    store::set_client_token_ref(conn, &client.id, Some(&vault_ref))?;
    let rec = Record { client_id: client.id, vault_ref, set_up_at: now_ms };
    store::set_setting(conn, SETTING, &serde_json::to_string(&rec).map_err(|e| e.to_string())?)?;
    Ok(state(conn))
}

/// Revokes the launcher's key and deletes it from the credential store.
pub fn remove(conn: &Connection) -> Result<State, String> {
    if let Some(r) = record(conn) {
        let _ = store::revoke_client(conn, &r.client_id);
        crate::vault::remove(&r.vault_ref).map_err(|e| e.to_string())?;
    }
    store::set_setting(conn, SETTING, "")?;
    Ok(state(conn))
}

/// The launcher's key, read from the credential store for one launch.
pub fn token(conn: &Connection) -> Result<String, String> {
    let r = record(conn).ok_or("The Claude Code launcher is not set up: open Pygg → CLI Tools → Claude Code → Set Up, or run `pygg setup claude`.")?;
    crate::vault::read(&r.vault_ref).map_err(|e| e.to_string())?
        .ok_or_else(|| "The launcher's key is missing from the credential store. Remove the launcher in CLI Tools and set it up again.".into())
}

/// A launch session's id (`pygg run`): `launch-` and a fresh UUID, so the ledger tells a launch from a task attempt or
/// a session the tool named itself (Codex's fifth discussion, T3).
pub const LAUNCH_PREFIX: &str = "launch-";

pub fn launch_session() -> String {
    format!("{LAUNCH_PREFIX}{}", uuid::Uuid::new_v4().simple())
}

/// The variables Claude Code is started with: the gateway, the key, `model` for every model slot, and a fresh session
/// so the whole session stays on one model (and one prompt cache).
pub fn environment(base_url: &str, token: &str, model: &str, session: &str) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> =
        vec![("ANTHROPIC_BASE_URL".into(), base_url.into()), ("ANTHROPIC_AUTH_TOKEN".into(), token.into())];
    for slot in [
        "ANTHROPIC_MODEL",
        "ANTHROPIC_DEFAULT_OPUS_MODEL",
        "ANTHROPIC_DEFAULT_SONNET_MODEL",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL",
    ] {
        env.push((slot.into(), model.into()));
    }
    env.push(("ANTHROPIC_CUSTOM_HEADERS".into(), format!("x-pygg-session: {session}")));
    // Claude Code then says what each request is (main, subagent, compaction…), which subagent sent it and which
    // prompt it serves (gateway/hints.rs). Pygg reads them and forwards none to the provider.
    env.push(("CLAUDE_CODE_GATEWAY_HINT_HEADERS".into(), "1".into()));
    env
}

/// Variables removed from Claude Code's environment at launch: another key would be sent to the gateway instead
/// of Pygg's, and a host session's variables would make it act as that session's child.
pub fn removed_variables() -> impl Iterator<Item = &'static str> {
    ["ANTHROPIC_API_KEY", "CLAUDE_CODE_USE_BEDROCK", "CLAUDE_CODE_USE_VERTEX"]
        .into_iter()
        .chain(crate::process::HOST_SESSION_VARS.iter().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_environment_points_every_slot_at_the_model() {
        let env = environment("http://127.0.0.1:8741", "str-x", "auto", "s1");
        let get = |k: &str| env.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        assert_eq!(get("ANTHROPIC_BASE_URL"), Some("http://127.0.0.1:8741"));
        assert_eq!(get("ANTHROPIC_DEFAULT_HAIKU_MODEL"), Some("auto"));
        assert_eq!(get("ANTHROPIC_CUSTOM_HEADERS"), Some("x-pygg-session: s1"));
        assert_eq!(get("CLAUDE_CODE_GATEWAY_HINT_HEADERS"), Some("1"));
        assert!(removed_variables().any(|v| v == "ANTHROPIC_API_KEY"));
        assert!(removed_variables().any(|v| v == "CLAUDECODE"));
    }

    #[test]
    fn state_without_a_record_is_not_set_up() {
        let conn = crate::db::open_in_memory().unwrap();
        assert!(!state(&conn).set_up);
        assert!(token(&conn).unwrap_err().contains("not set up"));
        // Removing what is not there is not an error.
        assert!(!remove(&conn).unwrap().set_up);
    }
}
