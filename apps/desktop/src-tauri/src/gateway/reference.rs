//! The reference catalog (ADR-031): what Pygg looks up so that the user only enters a key. Facts a provider's own
//! model list leaves out (context, maximum output, tool and image support, list price) are filled from
//! [models.dev](https://models.dev), the open catalog OpenCode uses, keyed by each provider's own model ids. A
//! catalog fact fills only an unknown: the provider's own list comes first, and what the user set is never touched.
//! Every fact names its source, so a figure from the catalog is never mistaken for the provider's or the user's.
//!
//! A tier is still a judgement: Pygg's naming table (tiers.rs) comes first, and a model it does not know gets one
//! from its list price band (`price-band:v1`), with the rule shown as the reason.
//!
//! The catalog is fetched at most once a day and kept next to the store, so it works offline and the request
//! carries nothing about the user (a GET of a public file).
use super::pricing::Price;
use super::store::{self, Provider, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const URL: &str = "https://models.dev/api.json";
/// Written into a model's facts source and a price's source.
pub const SOURCE: &str = "models.dev";
/// The tier source for a tier from the list price band.
pub const TIER_SOURCE: &str = "price-band:v1";
pub const TIER_NOTE: &str = "from its list price: Pygg's rule puts output under $1 per million tokens at T1, under $5 at \
    T2, under $20 at T3, and above that at T4";
const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Kinds that run on this computer: their model names are the user's own, and they cost nothing.
const LOCAL: &[&str] = &["ollama", "lmstudio", "llamacpp", "vllm", "docker-model-runner", "jan"];

/// Pygg's connection kinds whose catalog provider id differs from the kind, or that have no endpoint in it.
const KINDS: &[(&str, &str)] = &[("gemini", "google"), ("together", "togetherai"), ("fireworks", "fireworks-ai"),
    ("moonshot", "moonshotai"), ("moonshot-anthropic", "moonshotai"), ("qwen", "alibaba"), ("zai-anthropic", "zai"),
    ("minimax-anthropic", "minimax"), ("deepseek-anthropic", "deepseek")];

/// The cached catalog for a store: `<store's folder>/reference/models.dev.json`.
pub fn cache_path(store: &Path) -> PathBuf {
    store.parent().unwrap_or(Path::new(".")).join("reference").join("models.dev.json")
}

/// The cached catalog, if there is one, and whether it is older than a day.
pub fn load(path: &Path) -> Option<(Value, bool)> {
    let text = std::fs::read_to_string(path).ok()?;
    let catalog = serde_json::from_str::<Value>(&text).ok().filter(Value::is_object)?;
    let stale = std::fs::metadata(path).and_then(|m| m.modified()).ok()
        .and_then(|t| t.elapsed().ok()).map_or(true, |age| age > MAX_AGE);
    Some((catalog, stale))
}

/// Fetches the catalog and keeps it next to the store.
pub async fn fetch(http: &reqwest::Client, path: &Path) -> std::result::Result<Value, String> {
    let response = http.get(URL).timeout(Duration::from_secs(60)).send().await.map_err(|e| format!("models.dev: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("models.dev answered HTTP {}", response.status().as_u16()));
    }
    let text = response.text().await.map_err(|e| format!("models.dev: {e}"))?;
    let catalog: Value = serde_json::from_str(&text).map_err(|e| format!("models.dev sent something that is not its catalog: {e}"))?;
    if !catalog.is_object() {
        return Err("models.dev sent something that is not its catalog".into());
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, &text).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
    Ok(catalog)
}

/// The cached catalog when it is fresh, else a fresh one (falling back to the cached one offline).
pub async fn current(http: &reqwest::Client, path: &Path) -> Option<Value> {
    match load(path) {
        Some((catalog, false)) => Some(catalog),
        cached => match fetch(http, path).await {
            Ok(catalog) => Some(catalog),
            Err(e) => {
                eprintln!("reference catalog: {e}");
                cached.map(|(catalog, _)| catalog)
            }
        },
    }
}

fn normalize(url: &str) -> String {
    url.trim().trim_end_matches('/').to_ascii_lowercase()
}

/// The catalog's entry for a connection: the one whose API address is the connection's, else the one for its kind.
fn catalog_provider<'a>(catalog: &'a Value, provider: &Provider) -> Option<&'a serde_json::Map<String, Value>> {
    if LOCAL.contains(&provider.kind.as_str()) {
        return None;
    }
    let all = catalog.as_object()?;
    let endpoint = normalize(&provider.endpoint);
    let by_endpoint = all.values().find(|p| p["api"].as_str().map(normalize).as_deref() == Some(endpoint.as_str()));
    let kind = KINDS.iter().find(|(k, _)| *k == provider.kind).map(|(_, id)| *id).unwrap_or(provider.kind.as_str());
    by_endpoint.or_else(|| all.get(kind))?["models"].as_object()
}

fn micro(usd: &Value) -> Option<u64> {
    usd.as_f64().filter(|v| v.is_finite() && *v >= 0.0).map(|v| (v * 1e6).round() as u64)
}

/// What the catalog says about one model, as a price and facts.
fn price_of(entry: &Value, now: i64) -> Option<Price> {
    let cost = &entry["cost"];
    Some(Price { input_micro_per_mtok: micro(&cost["input"])?, output_micro_per_mtok: micro(&cost["output"])?,
        cached_input_micro_per_mtok: micro(&cost["cache_read"]), cache_write_micro_per_mtok: micro(&cost["cache_write"]),
        source: format!("{SOURCE} list price"), observed_at: now })
}

#[derive(Debug, Default, PartialEq)]
pub struct Applied {
    /// Models the catalog described.
    pub matched: usize,
    /// Models that got at least one fact or a price from it.
    pub filled: usize,
    /// Models that got a tier from their price band.
    pub tiered: usize,
}

/// Fills unknown facts and prices from the catalog, then tiers: the naming table first, the price band after.
pub fn apply(conn: &Connection, catalog: &Value) -> Result<Applied> {
    let now = crate::db::now_ms();
    let mut applied = Applied::default();
    for provider in store::list_providers(conn)? {
        let Some(models) = catalog_provider(catalog, &provider) else { continue };
        let mut statement = conn.prepare("SELECT model_id FROM models WHERE provider_id = ?1").map_err(|e| e.to_string())?;
        let ids: Vec<String> = statement.query_map([&provider.id], |r| r.get(0)).map_err(|e| e.to_string())?
            .collect::<rusqlite::Result<_>>().map_err(|e| e.to_string())?;
        for id in ids {
            let Some(entry) = models.get(&id) else { continue };
            applied.matched += 1;
            let vision = entry["modalities"]["input"].as_array().map(|m| m.iter().any(|v| v == "image"));
            let changed = conn.execute(
                "UPDATE models SET \
                   context_tokens = COALESCE(context_tokens, ?3), max_output_tokens = COALESCE(max_output_tokens, ?4), \
                   supports_tools = COALESCE(supports_tools, ?5), supports_vision = COALESCE(supports_vision, ?6), \
                   caps_source = CASE WHEN caps_source IS NULL THEN ?7 WHEN instr(caps_source, ?7) > 0 THEN caps_source \
                                      ELSE caps_source || '; gaps: ' || ?7 END, \
                   caps_observed_at = COALESCE(caps_observed_at, ?8) \
                 WHERE provider_id = ?1 AND model_id = ?2 AND (caps_source IS NULL OR caps_source <> 'user') \
                   AND ((context_tokens IS NULL AND ?3 IS NOT NULL) OR (max_output_tokens IS NULL AND ?4 IS NOT NULL) \
                     OR (supports_tools IS NULL AND ?5 IS NOT NULL) OR (supports_vision IS NULL AND ?6 IS NOT NULL))",
                params![provider.id, id, entry["limit"]["context"].as_u64().filter(|v| *v > 0).map(|v| v as i64),
                    entry["limit"]["output"].as_u64().filter(|v| *v > 0).map(|v| v as i64), entry["tool_call"].as_bool(),
                    vision, SOURCE, now]).map_err(|e| e.to_string())?;
            let priced = match price_of(entry, now) {
                Some(price) => conn.execute("UPDATE models SET price_json = ?3 WHERE provider_id = ?1 AND model_id = ?2 AND price_json IS NULL",
                    params![provider.id, id, serde_json::to_string(&price).unwrap_or_default()]).map_err(|e| e.to_string())?,
                None => 0,
            };
            if changed + priced > 0 {
                applied.filled += 1;
            }
        }
    }
    store::fill_curated_tiers(conn)?;
    applied.tiered = fill_price_band_tiers(conn)?;
    Ok(applied)
}

/// The tier a list price suggests, by output price per million tokens. A $0 price says nothing about a model (a
/// free variant of a large one), so it gives no tier.
pub fn price_band(price: &Price) -> Option<&'static str> {
    let output = price.output_micro_per_mtok;
    match output {
        0 => None,
        o if o < 1_000_000 => Some("T1"),
        o if o < 5_000_000 => Some("T2"),
        o if o < 20_000_000 => Some("T3"),
        _ => Some("T4"),
    }
}

/// Gives models that no one, and not the naming table, has tiered a tier from their price band.
pub fn fill_price_band_tiers(conn: &Connection) -> Result<usize> {
    let mut statement = conn.prepare("SELECT provider_id, model_id, price_json FROM models \
        WHERE tier IS NULL AND tier_source IS NULL AND price_json IS NOT NULL").map_err(|e| e.to_string())?;
    let rows: Vec<(String, String, String)> = statement.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(|e| e.to_string())?.collect::<rusqlite::Result<_>>().map_err(|e| e.to_string())?;
    let mut filled = 0;
    for (provider, model, json) in rows {
        let Some(tier) = serde_json::from_str::<Price>(&json).ok().as_ref().and_then(price_band) else { continue };
        let Some(n) = store::TIERS.iter().position(|t| *t == tier) else { continue };
        filled += conn.execute("UPDATE models SET tier = ?3, tier_source = ?4 WHERE provider_id = ?1 AND model_id = ?2 \
            AND tier IS NULL AND tier_source IS NULL", params![provider, model, n as i64, TIER_SOURCE]).map_err(|e| e.to_string())?;
    }
    Ok(filled)
}

/// When the catalog was last applied (setting `reference.applied_at`), for the Connections screen.
pub fn applied_at(conn: &Connection) -> Option<i64> {
    conn.query_row("SELECT value FROM settings WHERE key = 'reference.applied_at'", [], |r| r.get::<_, String>(0))
        .optional().ok().flatten().and_then(|v| v.parse().ok())
}

pub fn mark_applied(conn: &Connection) {
    let _ = store::set_setting(conn, "reference.applied_at", &crate::db::now_ms().to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::store::{Capabilities, Listed, NewProvider};
    use serde_json::json;

    fn catalog() -> Value {
        json!({
            "anthropic": {"id": "anthropic", "models": {
                "claude-sonnet-5-5": {"tool_call": true, "cost": {"input": 2, "output": 10, "cache_read": 0.2, "cache_write": 2.5},
                    "limit": {"context": 1000000, "output": 128000}, "modalities": {"input": ["text", "image"]}},
                "claude-haiku-5-5": {"tool_call": true, "cost": {"input": 0.1, "output": 0.5}, "limit": {"context": 200000, "output": 64000}}}},
            "alibaba-token-plan": {"api": "https://token-plan.example/compatible-mode/v1/", "models": {
                "qwen3.7-plus": {"tool_call": true, "cost": {"input": 0.4, "output": 2.4}, "limit": {"context": 131072, "output": 32768}}}},
            "alibaba": {"api": "https://dashscope-intl.aliyuncs.com/compatible-mode/v1", "models": {
                "qwen3.7-plus": {"tool_call": false, "cost": {"input": 9, "output": 99}}}},
            "zai": {"models": {"glm-5.3": {"tool_call": true, "cost": {"input": 1, "output": 3.2}, "limit": {"context": 200000, "output": 131072}},
                "glm-5.2": {"tool_call": true, "cost": {"input": 1, "output": 3.2}, "limit": {"context": 200000, "output": 131072}},
                "glm-image": {"type": "image", "tool_call": false, "cost": {"input": 0, "output": 0}}}},
            "lmstudio": {"models": {"qwen3:8b": {"tool_call": true}}}
        })
    }

    fn add(conn: &Connection, kind: &str, endpoint: &str, models: &[&str]) -> String {
        let p = store::add_provider(conn, &NewProvider { kind, label: kind, endpoint, credential_ref: None,
            default_billing: "paid", wire: "openai" }).unwrap();
        for m in models {
            store::add_model(conn, &p.id, m).unwrap();
        }
        p.id
    }

    fn model(conn: &Connection, provider: &str, id: &str) -> store::Model {
        store::list_models(conn, Some(provider)).unwrap().into_iter().find(|m| m.model_id == id).unwrap()
    }

    #[test]
    fn the_catalog_fills_what_is_unknown_and_names_itself() {
        let conn = crate::db::open_in_memory().unwrap();
        let claude = add(&conn, "anthropic", "https://api.anthropic.com/v1", &["claude-sonnet-5-5", "claude-haiku-5-5", "claude-unknown"]);
        // The provider's own list said the context; the catalog fills only the gaps.
        store::import_listed(&conn, &claude, &[Listed { id: "claude-haiku-5-5".into(), context_tokens: Some(150_000), ..Listed::default() }],
            Some("provider: anthropic /models")).unwrap();
        let applied = apply(&conn, &catalog()).unwrap();
        assert_eq!((applied.matched, applied.filled), (2, 2));
        let sonnet = model(&conn, &claude, "claude-sonnet-5-5");
        assert_eq!((sonnet.capabilities.context_tokens, sonnet.capabilities.max_output_tokens, sonnet.capabilities.supports_tools,
            sonnet.capabilities.supports_vision), (Some(1_000_000), Some(128_000), Some(true), Some(true)));
        let price = sonnet.price.unwrap();
        assert_eq!((price.input_micro_per_mtok, price.output_micro_per_mtok, price.cached_input_micro_per_mtok,
            price.source.as_str()), (2_000_000, 10_000_000, Some(200_000), "models.dev list price"));
        let haiku = model(&conn, &claude, "claude-haiku-5-5");
        assert_eq!(haiku.capabilities.context_tokens, Some(150_000), "the provider's own figure stays");
        assert_eq!(haiku.capabilities.max_output_tokens, Some(64_000));
        let source = haiku.capabilities.source.unwrap();
        assert!(source.starts_with("provider: anthropic /models; tools: curated:v1") && source.ends_with("; gaps: models.dev"), "{source}");
        // Tiers: the naming table still decides Claude's.
        assert_eq!((sonnet.capabilities.tier.as_deref(), sonnet.capabilities.tier_source.as_deref()), (Some("T3"), Some(store::CURATED)));
        assert_eq!(model(&conn, &claude, "claude-unknown").price, None, "not in the catalog: still unknown");
        // Applying again changes nothing.
        assert_eq!(apply(&conn, &catalog()).unwrap().filled, 0);
    }

    #[test]
    fn the_connection_is_matched_by_its_address_first_and_local_ones_never() {
        let conn = crate::db::open_in_memory().unwrap();
        let plan = add(&conn, "qwen", "https://token-plan.example/compatible-mode/v1", &["qwen3.7-plus"]);
        let intl = add(&conn, "qwen", "https://dashscope-intl.aliyuncs.com/compatible-mode/v1", &["qwen3.7-plus"]);
        let local = add(&conn, "lmstudio", "http://127.0.0.1:1234/v1", &["qwen3:8b"]);
        apply(&conn, &catalog()).unwrap();
        assert_eq!(model(&conn, &plan, "qwen3.7-plus").capabilities.supports_tools, Some(true), "the token plan's own entry");
        assert_eq!(model(&conn, &intl, "qwen3.7-plus").capabilities.supports_tools, Some(false));
        assert_eq!(model(&conn, &local, "qwen3:8b").capabilities.source, None);
    }

    #[test]
    fn the_users_facts_and_price_are_never_replaced_and_a_price_band_gives_a_tier() {
        let conn = crate::db::open_in_memory().unwrap();
        let zai = add(&conn, "zai", "https://api.z.ai/api/paas/v4", &["glm-5.3", "glm-5.2", "glm-image"]);
        store::set_capabilities(&conn, &zai, "glm-5.3", &Capabilities { supports_tools: Some(false), ..Capabilities::default() }).unwrap();
        let mine = Price { input_micro_per_mtok: 7, output_micro_per_mtok: 8, source: "user".into(), ..Price::default() };
        store::update_model(&conn, &zai, "glm-5.3", None, None, Some(Some(mine.clone()))).unwrap();
        apply(&conn, &catalog()).unwrap();
        let glm = model(&conn, &zai, "glm-5.3");
        assert_eq!((glm.capabilities.supports_tools, glm.capabilities.context_tokens), (Some(false), None), "the user's facts stand");
        assert_eq!(glm.price.as_ref().map(|p| p.output_micro_per_mtok), Some(8), "and the user's price");
        assert_eq!(glm.capabilities.tier, None, "the user's tier, cleared, stays cleared");
        // An unknown price is filled, and its band gives a tier: output $3.20 per million is T2.
        let other = model(&conn, &zai, "glm-5.2");
        assert_eq!(other.price.as_ref().map(|p| p.output_micro_per_mtok), Some(3_200_000));
        assert_eq!((other.capabilities.tier.as_deref(), other.capabilities.tier_source.as_deref()), (Some("T2"), Some(TIER_SOURCE)));
        assert_eq!(other.capabilities.tier_note.as_deref(), Some(TIER_NOTE));
        let image = model(&conn, &zai, "glm-image");
        assert_eq!((image.capabilities.supports_tools, image.capabilities.tier.as_deref()), (Some(false), None), "a $0 price gives no tier");
    }

    #[test]
    fn price_bands() {
        let p = |o: u64| Price { output_micro_per_mtok: o, ..Price::default() };
        assert_eq!([p(0), p(500_000), p(1_000_000), p(10_000_000), p(25_000_000)].iter().map(price_band).collect::<Vec<_>>(),
            [None, Some("T1"), Some("T2"), Some("T3"), Some("T4")]);
    }
}
