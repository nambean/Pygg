//! Health probing (ROADMAP B4, Account Rotation §5 "half-open"): while the gateway runs, a connection marked
//! unreachable is checked again by reading the provider's model list, which costs nothing and sends no prompt. When
//! the list answers, the outage is lifted at once instead of waiting out a cooldown that may have grown with each
//! failure. Only outages are probed: a rejected key or an exhausted quota is not something a model list can show
//! is fixed (some lists answer without a key), so those wait for their cooldown or the user.

use super::{catalog, store, Shared};
use std::time::Duration;
use tokio::sync::watch;

/// How often the loop looks for something to probe.
pub const PROBE_EVERY: Duration = Duration::from_secs(30);
/// How long after the last failure or check a connection is probed again.
pub const PROBE_GAP_MS: i64 = 60_000;

/// One round: every unreachable connection not checked for `PROBE_GAP_MS`. Returns those that answered.
pub async fn probe_once(shared: &Shared) -> Vec<String> {
    let due = shared.with_db(|db| store::unreachable_to_probe(db, PROBE_GAP_MS)).unwrap_or_default();
    let mut recovered = Vec::new();
    for id in due {
        let Some(provider) = shared.with_db(store::list_providers).unwrap_or_default().into_iter().find(|p| p.id == id)
        else {
            continue;
        };
        match catalog::fetch_models(shared, &provider).await {
            Ok(_) => {
                let _ = shared.with_db(|db| store::probe_succeeded(db, &id));
                recovered.push(id);
            }
            Err(_) => {
                let _ = shared.with_db(|db| store::probe_failed(db, &id));
            }
        }
    }
    recovered
}

/// Probes until `stop` turns true (the server stopping).
pub fn spawn(shared: Shared, mut stop: watch::Receiver<bool>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(PROBE_EVERY) => { probe_once(&shared).await; }
                changed = stop.changed() => { if changed.is_err() || *stop.borrow() { break; } }
            }
        }
    })
}
