//! Re-checks connections while the gateway runs (owner, 2026-11-13: "we have no way to check the providers' state
//! again"). An API connection is checked by reading its provider's model list, which costs nothing and sends no
//! prompt, every `CHECK_EVERY_MS`; the result is kept (`connection_checks`) and counts in the health strip beside real
//! requests. Web sessions are not checked on a timer or by Check All: a check loads the site's page, and pages loaded by
//! a program are the pattern the web guard exists to avoid (ADR-035). Each is checked when the user asks (Check
//! Sign-In), at the guard's pace.
//!
//! One check runs at a time per connection, whoever asked for it (the timer, Check Now, Check All, Refresh Models), and
//! each reads the connection afresh once its turn comes, so a check started with an old key never lands after one
//! with the new key (Codex's fourth review, A12).

use super::{catalog, presets, store, Shared};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;
use tokio::sync::watch;

/// How often an API connection is checked.
pub const CHECK_EVERY_MS: i64 = 30 * 60_000;
/// How often the loop looks for a connection due.
pub const LOOP_EVERY: Duration = Duration::from_secs(60);
/// How many providers are checked at once: they are independent, and one slow provider must not hold up the rest.
pub const AT_ONCE: usize = 4;
/// The detail of a check whose provider answered but keeps no model list (HTTP 404 or 405 on `/models`).
pub const NO_LIST: &str = "Reachable · the provider keeps no model list, so the key was not checked";

type Turns = Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>;
static TURNS: LazyLock<Turns> = LazyLock::new(Turns::default);

fn turn(provider_id: &str) -> Arc<tokio::sync::Mutex<()>> {
    TURNS.lock().unwrap_or_else(|p| p.into_inner()).entry(provider_id.to_string()).or_default().clone()
}

/// Reads the connection's model list and keeps the result as its check; returns the check and, when the list was read,
/// the list. Called with the connection's turn held.
async fn check_now(shared: &Shared, provider_id: &str) -> Option<(store::Check, Result<Vec<store::Listed>, String>)> {
    // The connection as it is now: a new key, or a connection deleted while this waited.
    let provider = shared.with_db(|db| store::get_provider(db, provider_id)).ok().flatten()?;
    let fetched = catalog::fetch_list(shared, &provider).await;
    let (ok, detail) = match &fetched {
        Ok(listed) => (true, format!("Reachable · {} models listed", listed.len())),
        // The provider answered; it just has no list to give. That is not a failure of the connection.
        Err(e) if matches!(e.status, Some(404 | 405)) => (true, NO_LIST.to_string()),
        Err(e) => (false, e.message.chars().take(300).collect()),
    };
    if ok {
        let _ = shared.with_db(|db| store::probe_succeeded(db, &provider.id));
    }
    let _ = shared.with_db(|db| store::record_check(db, &provider.id, ok, "models", &detail));
    let check = store::Check { at: crate::db::now_ms(), ok, kind: "models".into(), detail };
    Some((check, fetched.map_err(|e| e.message)))
}

/// Checks one API connection by reading its model list, waiting for a check already running on it, and keeps the
/// result. A connection marked unreachable that answers is cleared at once, as the probe does.
pub async fn check_api(shared: &Shared, provider: &store::Provider) -> store::Check {
    let turn = turn(&provider.id);
    let _held = turn.lock().await;
    match check_now(shared, &provider.id).await {
        Some((check, _)) => check,
        None => store::Check {
            at: crate::db::now_ms(),
            ok: false,
            kind: "models".into(),
            detail: "Connection not found".into(),
        },
    }
}

/// Reads the model list for Refresh Models, as a check of the connection too: the list, or why there is none.
pub async fn list_and_check(shared: &Shared, provider: &store::Provider) -> Result<Vec<store::Listed>, String> {
    let turn = turn(&provider.id);
    let _held = turn.lock().await;
    match check_now(shared, &provider.id).await {
        Some((_, listed)) => listed,
        None => Err("Connection not found".into()),
    }
}

/// Checks these API connections, `AT_ONCE` at a time; returns their checks.
pub async fn check_many(shared: &Shared, providers: Vec<store::Provider>) -> Vec<store::Check> {
    futures_util::stream::iter(providers.into_iter().filter(|p| !presets::is_web(&p.kind)))
        .map(|p| async move { check_api(shared, &p).await })
        .buffer_unordered(AT_ONCE)
        .collect()
        .await
}

/// One round: every enabled API connection whose last check is older than `CHECK_EVERY_MS`, a few at a time. A
/// connection being checked already (Check Now) is left to that check.
pub async fn check_due(shared: &Shared) -> usize {
    let due = shared.with_db(|db| store::checks_due(db, CHECK_EVERY_MS, crate::db::now_ms())).unwrap_or_default();
    let providers = shared.with_db(store::list_providers).unwrap_or_default();
    let due: Vec<String> =
        due.into_iter().filter(|id| providers.iter().any(|p| p.id == *id && !presets::is_web(&p.kind))).collect();
    futures_util::stream::iter(due)
        .map(|id| async move {
            let turn = turn(&id);
            let Ok(_held) = turn.try_lock() else { return 0 };
            usize::from(check_now(shared, &id).await.is_some())
        })
        .buffer_unordered(AT_ONCE)
        .fold(0, |n, one| async move { n + one })
        .await
}

/// Checks until `stop` turns true (the server stopping), also in the middle of a round.
pub fn spawn(shared: Shared, mut stop: watch::Receiver<bool>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(LOOP_EVERY) => {}
                changed = stop.changed() => { if changed.is_err() || *stop.borrow() { break; } continue; }
            }
            tokio::select! {
                _ = check_due(&shared) => {}
                changed = stop.changed() => { if changed.is_err() || *stop.borrow() { break; } }
            }
        }
    })
}
