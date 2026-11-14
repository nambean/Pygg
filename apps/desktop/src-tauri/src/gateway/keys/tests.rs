//! Key rings (ADR-038): the store's rules and each strategy's order, with an in-memory store.

use super::*;
use crate::gateway::store::NewProvider;

const NOW: i64 = 1_794_560_000_000;

fn store_with(kind: &str) -> (Connection, Provider) {
    let conn = crate::db::open_in_memory().unwrap();
    let provider = store::add_provider(&conn, &NewProvider { kind, label: "Router", endpoint: "https://r.test/v1",
        credential_ref: Some("provider-first"), default_billing: "paid", wire: "openai" }).unwrap();
    (conn, provider)
}

fn ring(conn: &Connection, provider: &Provider, labels: &[&str]) -> Vec<Key> {
    for (i, label) in labels.iter().enumerate() {
        add(conn, &provider.id, label, &format!("provider-k{i}"), true, NOW).unwrap();
    }
    list(conn, &provider.id).unwrap()
}

fn ids(keys: &[Key]) -> Vec<&str> {
    keys.iter().map(|k| k.label.as_str()).collect()
}

fn used(conn: &Connection, key: &Key, times: usize) {
    for _ in 0..times {
        conn.execute("INSERT INTO usage_events (id, provider, model, cost_status, source, created_at, outcome, key_id) \
            VALUES (lower(hex(randomblob(8))), ?1, 'm', 'unknown', 'test', ?2, 'ok', ?3)", params![key.provider_id, crate::db::now_ms(), key.id]).unwrap();
    }
}

#[test]
fn the_schema_has_key_rings() {
    let conn = crate::db::open_in_memory().unwrap();
    for table in ["provider_keys", "key_health", "key_limits"] {
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1", [table], |r| r.get(0)).unwrap();
        assert_eq!(n, 1, "{table}");
    }
    let (conn, provider) = store_with("openrouter");
    assert_eq!(strategy(&conn, &provider.id).unwrap(), "in_order", "the default for every connection");
    conn.execute("UPDATE usage_events SET key_id = NULL", []).unwrap();
}

#[test]
fn a_second_key_needs_another_account_and_the_first_joins_the_ring() {
    let (conn, provider) = store_with("openrouter");
    assert!(list(&conn, &provider.id).unwrap().is_empty());
    assert_eq!(add(&conn, &provider.id, "Work", "provider-w", false, NOW).unwrap_err(), SEPARATE_ACCOUNT);
    assert!(list(&conn, &provider.id).unwrap().is_empty(), "nothing kept");
    let work = add(&conn, &provider.id, "Work", "provider-w", true, NOW).unwrap();
    let keys = list(&conn, &provider.id).unwrap();
    assert_eq!(ids(&keys), ["First key", "Work"]);
    assert_eq!(keys[0].credential_ref, "provider-first");
    assert_eq!(keys[0].separate_account_at, None);
    assert_eq!(work.separate_account_at, Some(NOW));
    assert!(add(&conn, &provider.id, "work", "provider-x", true, NOW).unwrap_err().contains("already has a key named"));
    assert!(add(&conn, &provider.id, "  ", "provider-x", true, NOW).is_err());
    // The secret is never part of what is shown.
    assert!(!serde_json::to_string(&work).unwrap().contains("provider-w"));
}

#[test]
fn sign_ins_and_keyless_connections_have_no_ring() {
    let conn = crate::db::open_in_memory().unwrap();
    let web = store::add_web_provider(&conn, "chatgpt-web", "ChatGPT", crate::websession::WARNING_VERSION).unwrap();
    assert_eq!(add(&conn, &web.id, "Second", "provider-s", true, NOW).unwrap_err(), NO_SIGN_INS);
    let local = store::add_provider(&conn, &NewProvider { kind: "ollama", label: "Local", endpoint: "http://127.0.0.1:11434/v1",
        credential_ref: None, default_billing: "local", wire: "openai" }).unwrap();
    assert_eq!(add(&conn, &local.id, "Second", "provider-s", true, NOW).unwrap_err(), NO_SIGN_INS);
    let keyless = store::add_provider(&conn, &NewProvider { kind: "custom", label: "Custom", endpoint: "https://c.test/v1",
        credential_ref: None, default_billing: "paid", wire: "openai" }).unwrap();
    assert!(add(&conn, &keyless.id, "Second", "provider-s", true, NOW).unwrap_err().contains("no key yet"));
}

#[test]
fn removing_the_first_key_makes_the_next_the_connections_and_the_last_stays() {
    let (conn, provider) = store_with("openrouter");
    let keys = ring(&conn, &provider, &["B", "C"]);
    assert_eq!(remove(&conn, &keys[0].id).unwrap(), "provider-first");
    let own: Option<String> = conn.query_row("SELECT credential_ref FROM providers WHERE id = ?1", [&provider.id], |r| r.get(0)).unwrap();
    assert_eq!(own.as_deref(), Some("provider-k0"), "B is now the connection's key, for model lists and checks");
    let left = list(&conn, &provider.id).unwrap();
    assert_eq!(ids(&left), ["B", "C"]);
    assert_eq!(left.iter().map(|k| k.rank).collect::<Vec<_>>(), [0, 1]);
    remove(&conn, &left[1].id).unwrap();
    assert!(remove(&conn, &left[0].id).unwrap_err().contains("only key"));
    // Removing the connection hands back every secret's reference, to delete.
    let (conn, provider) = store_with("openrouter");
    ring(&conn, &provider, &["B"]);
    let mut refs = store::remove_provider(&conn, &provider.id).unwrap();
    refs.sort();
    assert_eq!(refs, ["provider-first", "provider-k0"]);
    assert_eq!(conn.query_row("SELECT COUNT(*) FROM provider_keys", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
}

#[test]
fn the_last_key_on_stays_on_and_the_order_can_change() {
    let (conn, provider) = store_with("openrouter");
    let keys = ring(&conn, &provider, &["B"]);
    set_enabled(&conn, &keys[0].id, false).unwrap();
    assert!(set_enabled(&conn, &keys[1].id, false).unwrap_err().contains("only key on"));
    set_enabled(&conn, &keys[0].id, true).unwrap();
    move_key(&conn, &keys[1].id, true).unwrap();
    assert_eq!(ids(&list(&conn, &provider.id).unwrap()), ["B", "First key"]);
    move_key(&conn, &keys[1].id, true).unwrap();
    assert_eq!(ids(&list(&conn, &provider.id).unwrap()), ["B", "First key"], "the first stays first");
    assert!(set_strategy(&conn, &provider.id, "random").is_err());
}

#[test]
fn each_strategy_orders_the_keys_its_own_way() {
    let (conn, provider) = store_with("openrouter");
    let keys = ring(&conn, &provider, &["B", "C"]);
    assert_eq!(ids(&order(&conn, &provider, "m", None, NOW).unwrap()), ["First key", "B", "C"]);

    set_strategy(&conn, &provider.id, "round_robin").unwrap();
    let starts: Vec<String> = (0..4).map(|_| order(&conn, &provider, "m", None, NOW).unwrap()[0].label.clone()).collect();
    assert_eq!(starts, ["First key", "B", "C", "First key"], "one request each, in turn");

    set_strategy(&conn, &provider.id, "least_used").unwrap();
    used(&conn, &keys[0], 3);
    used(&conn, &keys[1], 1);
    assert_eq!(ids(&order(&conn, &provider, "m", None, crate::db::now_ms()).unwrap()), ["C", "B", "First key"]);

    set_strategy(&conn, &provider.id, "quota_aware").unwrap();
    let limits = |remaining: &str, reset: &str| vec![("x-ratelimit-limit-requests".to_string(), "100".to_string()),
        ("x-ratelimit-remaining-requests".to_string(), remaining.to_string()), ("x-ratelimit-reset-requests".to_string(), reset.to_string())];
    record_limits(&conn, &keys[0].id, "m", 200, &limits("90", "1m")).unwrap();
    record_limits(&conn, &keys[1].id, "m", 200, &limits("20", "1m")).unwrap();
    // C's account says its requests are used up for the next hour: it goes last, after B with 20 % left.
    record_limits(&conn, &keys[2].id, "m", 429, &limits("0", "1h")).unwrap();
    let now = crate::db::now_ms();
    assert_eq!(ids(&order(&conn, &provider, "m", None, now).unwrap()), ["First key", "B", "C"]);
    // Another model: nothing stated, so the use today decides.
    assert_eq!(ids(&order(&conn, &provider, "other", None, now).unwrap()), ["C", "B", "First key"]);

    // A session stays on the key that answered it, whatever the strategy.
    assert_eq!(order(&conn, &provider, "m", Some(&keys[2].id), now).unwrap()[0].label, "C");
    // A key turned off is not tried.
    set_enabled(&conn, &keys[0].id, false).unwrap();
    assert_eq!(ids(&order(&conn, &provider, "m", None, now).unwrap()), ["B", "C"]);
}

#[test]
fn one_key_is_no_ring() {
    let (conn, provider) = store_with("openrouter");
    assert!(order(&conn, &provider, "m", None, NOW).unwrap().is_empty(), "its own key, as before");
    let keys = ring(&conn, &provider, &["B"]);
    remove(&conn, &keys[1].id).unwrap();
    assert!(order(&conn, &provider, "m", None, NOW).unwrap().is_empty());
}

#[test]
fn a_keys_marks_are_its_own_and_an_answer_clears_them() {
    let (conn, provider) = store_with("openrouter");
    let keys = ring(&conn, &provider, &["B"]);
    let now = crate::db::now_ms();
    mark(&conn, &keys[0].id, "", "invalid_credentials", "rejected", Some(401), None).unwrap();
    mark(&conn, &keys[1].id, "m", "rate_limited", "429", Some(429), Some(now + 60_000)).unwrap();
    assert_eq!(blocking(&conn, &keys[0], "any", now).unwrap().unwrap().state, "invalid_credentials", "a rejected key, for every model");
    assert!(blocking(&conn, &keys[1], "m", now).unwrap().is_some());
    assert!(blocking(&conn, &keys[1], "other", now).unwrap().is_none(), "a rate limit on one model only");
    assert!(blocking(&conn, &keys[1], "m", now + 61_000).unwrap().is_none(), "a cooldown ends");
    assert!(store::list_health(&conn, Some(&provider.id)).unwrap().is_empty(), "the connection itself is not marked");
    healthy(&conn, &keys[1].id, "m").unwrap();
    assert!(blocking(&conn, &keys[1], "m", now).unwrap().is_none());
    store::clear_health(&conn, &provider.id).unwrap();
    assert!(health(&conn, &provider.id, now).unwrap().is_empty(), "Connections → Clear clears every key");
}
