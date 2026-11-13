//! Re-checks connections while the gateway runs (owner, 2026-11-13: "we have no way to check the providers' state
//! again"). An API connection is checked by reading its provider's model list, which costs nothing and sends no
//! prompt, every `CHECK_EVERY_MS`; the result is kept (`connection_checks`) and counts in the health strip beside real
//! requests. Web sessions are not checked on a timer: a check loads the site's page, and a page loaded every half hour
//! by a program is the kind of pattern the web guard exists to avoid (ADR-035). They are checked when the user asks
//! (Check Sign-In, or Check All at most once an hour each).

use super::{catalog, presets, store, Shared};
use std::time::Duration;
use tokio::sync::watch;

/// How often an API connection is checked.
pub const CHECK_EVERY_MS: i64 = 30 * 60_000;
/// Check All checks a web session only if its last check is older than this.
pub const WEB_CHECK_GAP_MS: i64 = 60 * 60_000;
/// How often the loop looks for a connection due.
pub const LOOP_EVERY: Duration = Duration::from_secs(60);

/// Checks one API connection by reading its model list, and keeps the result. A connection marked unreachable that
/// answers is cleared at once, as the probe does.
pub async fn check_api(shared: &Shared, provider: &store::Provider) -> store::Check {
    let (ok, detail) = match catalog::fetch_models(shared, provider).await {
        Ok(listed) => {
            let _ = shared.with_db(|db| store::probe_succeeded(db, &provider.id));
            (true, format!("Reachable · {} models listed", listed.len()))
        }
        Err(e) => (false, e.chars().take(300).collect()),
    };
    let _ = shared.with_db(|db| store::record_check(db, &provider.id, ok, "models", &detail));
    store::Check { at: crate::db::now_ms(), ok, kind: "models".into(), detail }
}

/// One round: every enabled API connection whose last check is older than `CHECK_EVERY_MS`, one after another.
pub async fn check_due(shared: &Shared) -> usize {
    let due = shared.with_db(|db| store::checks_due(db, CHECK_EVERY_MS, crate::db::now_ms())).unwrap_or_default();
    let providers = shared.with_db(store::list_providers).unwrap_or_default();
    let mut checked = 0;
    for id in due {
        let Some(provider) = providers.iter().find(|p| p.id == id) else { continue };
        if presets::is_web(&provider.kind) {
            continue;
        }
        check_api(shared, provider).await;
        checked += 1;
    }
    checked
}

/// Checks until `stop` turns true (the server stopping).
pub fn spawn(shared: Shared, mut stop: watch::Receiver<bool>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(LOOP_EVERY) => { check_due(&shared).await; }
                changed = stop.changed() => { if changed.is_err() || *stop.borrow() { break; } }
            }
        }
    })
}
