//! What routing saved, as an estimate (SCREENS §2.2, Home and Usage): the requests the gateway answered, priced
//! again at one model the user chose to compare with, with the same tokens.
//!
//! It is an estimate and says so. Another model would have written a different number of tokens; the cache split is
//! assumed to be the same; and a request whose cost or tokens are not known is left out and counted, never taken as
//! $0. Failed attempts with a known cost are added to what routing cost, not hidden; those with an unknown cost are
//! counted. Subscription requests are counted apart: their money is a plan, not a price per token.

use std::collections::HashMap;

use rusqlite::{params, Connection};
use serde::Serialize;

use super::pricing::{self, Price, Usage};
use super::store::{self, Result};

/// The setting holding the model to compare with, as `connection/model`.
pub const BASELINE_SETTING: &str = "savings.baseline";

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingCounts {
    pub free: u64,
    pub local: u64,
    pub subscription: u64,
    pub paid: u64,
    pub unknown: u64,
}

/// One slice of the estimate: the answered requests of one billing, one client or one way of asking, priced both ways.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavingsPart {
    /// A billing (`free`, `local`, `subscription`, `paid`, `unknown`), a client's name (empty for none), or how the
    /// model was asked for (`auto`, `auto/cheap`, `combo`, `named`).
    pub key: String,
    pub answered: u64,
    pub compared: u64,
    pub routed_micro_usd: u64,
    pub baseline_micro_usd: u64,
}

/// The compared requests of one period (an hour or a day on the user's clock), for the trend.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavingsPeriod {
    pub start: i64,
    pub compared: u64,
    pub routed_micro_usd: u64,
    pub baseline_micro_usd: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Savings {
    /// `connection/model`, or `None` while the user has not chosen one.
    pub baseline: Option<String>,
    /// Whether the chosen model has a price; without one nothing can be compared.
    pub baseline_priced: bool,
    /// Requests the gateway answered in the period, by how their model is billed.
    pub answered: BillingCounts,
    /// Answered requests priced both ways.
    pub compared: u64,
    /// What those requests cost as routed (known costs only), plus failed attempts with a known cost.
    pub routed_micro_usd: u64,
    /// The same tokens at the baseline's price.
    pub baseline_micro_usd: u64,
    /// Part of `routed_micro_usd` from attempts that failed or fell back.
    pub failed_attempts_micro_usd: u64,
    /// Answered requests left out because their cost was unknown (subscription ones are counted in `answered`).
    pub left_out_unknown_cost: u64,
    /// Answered requests left out because the provider reported no tokens.
    pub left_out_no_tokens: u64,
    /// Failed attempts whose cost is unknown: they may have been billed, so the estimate may be too kind.
    pub failed_unknown_cost: u64,
    /// A model to propose as the baseline, from evidence: the paid, priced model clients asked for by name most often
    /// in the period (auto and combo names left out). Never the most expensive one, which would only inflate the
    /// figure. The user must accept it.
    pub suggested: Option<String>,
    /// How the saving came about, each answered request in one slice of each: by how its model is billed (free and
    /// local answers cost nothing; a subscription is a plan; paid ones may still be cheaper than the baseline), by the
    /// client that sent it, and by how the model was asked for (`auto` and a combo are Pygg's choice; a model asked for
    /// by name is the user's, so its difference is not Pygg's doing).
    pub by_billing: Vec<SavingsPart>,
    pub by_client: Vec<SavingsPart>,
    pub by_route: Vec<SavingsPart>,
    /// The compared requests per period, oldest first, for the trend; empty without a priced baseline.
    pub periods: Vec<SavingsPeriod>,
    /// Attempts that failed and were followed by the next model of a combo or of auto: requests kept alive.
    pub fell_back: u64,
}

/// How a request asked for its model: Pygg's choice (`auto`, `auto/cheap`, a combo) or the user's (`named`).
pub fn route_kind(requested_model: &str) -> &'static str {
    if requested_model == "auto" {
        "auto"
    } else if requested_model == "auto/cheap" {
        "auto/cheap"
    } else if requested_model.starts_with("auto/") {
        "auto"
    } else if requested_model.contains('/') {
        "named"
    } else {
        "combo"
    }
}

/// The ledger's route kind (migration 0030) as the estimate groups it: a pin or a combo's order does not change who chose.
fn recorded_route(kind: &str) -> &'static str {
    let kind = kind.split('+').next().unwrap_or(kind);
    match kind {
        "auto" => "auto",
        "auto/cheap" => "auto/cheap",
        // A role alias (`auto/code`, ADR-041) is Pygg's choice too; the estimate groups it with `auto`.
        "auto/plan" | "auto/code" | "auto/test" | "auto/review" => "auto",
        "named" => "named",
        _ => "combo",
    }
}

fn part<'a>(parts: &'a mut Vec<SavingsPart>, key: &str) -> &'a mut SavingsPart {
    if let Some(i) = parts.iter().position(|p| p.key == key) {
        return &mut parts[i];
    }
    parts.push(SavingsPart { key: key.to_string(), ..Default::default() });
    parts.last_mut().unwrap()
}

/// The paid model with a price that clients named directly most often since `since`.
fn suggestion(conn: &Connection, since: i64) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT requested_model, COUNT(*) AS n FROM usage_events WHERE created_at >= ?1 \
        AND requested_model LIKE '%/%' AND requested_model NOT LIKE 'auto/%' GROUP BY requested_model ORDER BY n DESC LIMIT 20")
        .map_err(|e| e.to_string())?;
    let names: Vec<String> = stmt
        .query_map(params![since], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<_>>()
        .map_err(|e| e.to_string())?;
    for name in names {
        let Some((provider_id, model_id)) = name.split_once('/') else { continue };
        let Some(provider) = store::get_provider(conn, provider_id)? else { continue };
        let (billing, price) = store::pricing_for(conn, &provider, model_id)?;
        if billing == "paid" && price.is_some_and(|p| p.input_micro_per_mtok > 0 || p.output_micro_per_mtok > 0) {
            return Ok(Some(name));
        }
    }
    Ok(None)
}

fn baseline_price(conn: &Connection, baseline: &str) -> Result<Option<Price>> {
    let Some((provider_id, model_id)) = baseline.split_once('/') else { return Ok(None) };
    let Some(provider) = store::get_provider(conn, provider_id)? else { return Ok(None) };
    let (_, price) = store::pricing_for(conn, &provider, model_id)?;
    Ok(price.filter(|p| p.input_micro_per_mtok > 0 || p.output_micro_per_mtok > 0))
}

type Row = (
    String,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    i64,
    Option<String>,
    Option<String>,
);

/// The estimate since `since`; the trend in periods of `bucket_ms` aligned to the user's clock (`offset_ms` east of UTC),
/// as `store::usage_over_time` aligns the usage chart.
pub fn savings(conn: &Connection, since: i64, bucket_ms: i64, offset_ms: i64) -> Result<Savings> {
    savings_in(conn, since, None, bucket_ms, offset_ms)
}

/// The same estimate for one routing session's requests (`pygg run`'s receipt, launches.rs).
pub fn session_savings(conn: &Connection, session: &str) -> Result<Savings> {
    savings_in(conn, 0, Some(session), 86_400_000, 0)
}

fn savings_in(conn: &Connection, since: i64, session: Option<&str>, bucket_ms: i64, offset_ms: i64) -> Result<Savings> {
    let baseline = store::setting(conn, BASELINE_SETTING)?.filter(|s| s.contains('/'));
    let price = match &baseline {
        Some(b) => baseline_price(conn, b)?,
        None => None,
    };
    let mut out = Savings { baseline: baseline.clone(), baseline_priced: price.is_some(), ..Default::default() };
    if baseline.is_none() {
        out.suggested = suggestion(conn, since)?;
    }
    // Only requests to the user's connections: an agent's own run (`agent:…`) has no connection here.
    // One session's rows by the session index (migration 0038), else the period's.
    let filter = if session.is_some() { "u.session_key = ?2 AND u.created_at >= ?1" } else { "u.created_at >= ?1" };
    let mut stmt = conn
        .prepare(&format!(
        "SELECT u.outcome, u.input_tokens, u.output_tokens, u.cache_tokens, u.cache_write_tokens, u.cost_micro_usd, \
         u.cost_status, u.provider, u.model, c.name, u.requested_model, u.created_at, u.billing, u.route_kind \
         FROM usage_events u JOIN providers p ON p.id = u.provider LEFT JOIN gateway_clients c ON c.id = u.client_id \
         WHERE {filter} ORDER BY u.created_at"))
        .map_err(|e| e.to_string())?;
    let map = |r: &rusqlite::Row| {
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
            r.get(12)?,
            r.get(13)?,
        ))
    };
    let rows: Vec<Row> = match session {
        Some(s) => stmt.query_map(params![since, s], map),
        None => stmt.query_map(params![since], map),
    }
    .map_err(|e| e.to_string())?
    .collect::<rusqlite::Result<_>>()
    .map_err(|e| e.to_string())?;
    // Billing by the same rule as the model list and the ledger (store::pricing_for), once per model.
    let mut billing_of: HashMap<(String, String), String> = HashMap::new();
    let bucket_ms = bucket_ms.max(60_000);
    for (
        outcome,
        input,
        output,
        cached,
        written,
        cost,
        status,
        provider_id,
        model_id,
        client,
        requested,
        at,
        sent_billing,
        sent_route,
    ) in rows
    {
        let known = status != "unknown";
        if outcome != "ok" {
            // Every attempt that gave no answer may have been billed, a client that left included: a known charge counts,
            // an unknown one is counted apart.
            if outcome == "fallback" {
                out.fell_back += 1;
            }
            if known {
                out.failed_attempts_micro_usd += cost.unwrap_or(0).max(0) as u64;
            } else {
                out.failed_unknown_cost += 1;
            }
            continue;
        }
        // The billing it was sent under (migration 0030); an older row has none, and follows today's settings.
        let key = (provider_id, model_id);
        if let Some(b) = sent_billing {
            billing_of.insert(key.clone(), b);
        } else if !billing_of.contains_key(&key) {
            let b = match store::get_provider(conn, &key.0)? {
                Some(p) => store::pricing_for(conn, &p, &key.1)?.0,
                None => "unknown".to_string(),
            };
            billing_of.insert(key.clone(), b);
        }
        let billing = match billing_of[&key].as_str() {
            "free" => {
                out.answered.free += 1;
                "free"
            }
            "local" => {
                out.answered.local += 1;
                "local"
            }
            "subscription" => {
                out.answered.subscription += 1;
                "subscription"
            }
            "paid" => {
                out.answered.paid += 1;
                "paid"
            }
            _ => {
                out.answered.unknown += 1;
                "unknown"
            }
        };
        let client = client.unwrap_or_default();
        let route = match sent_route.as_deref() {
            Some(kind) => recorded_route(kind),
            None => route_kind(requested.as_deref().unwrap_or("auto")),
        };
        part(&mut out.by_billing, billing).answered += 1;
        part(&mut out.by_client, &client).answered += 1;
        part(&mut out.by_route, route).answered += 1;
        // A subscription is a plan, not a price per token: counted, never compared.
        if billing == "subscription" {
            continue;
        }
        let Some(price) = &price else { continue };
        if !known {
            out.left_out_unknown_cost += 1;
            continue;
        }
        let (Some(input), Some(output)) = (input, output) else {
            out.left_out_no_tokens += 1;
            continue;
        };
        let usage = Usage {
            prompt: input.max(0) as u64,
            completion: output.max(0) as u64,
            cached: cached.unwrap_or(0).max(0) as u64,
            cache_write: written.unwrap_or(0).max(0) as u64,
            cache_reported: cached.is_some(),
            provider_cost_usd: None,
        };
        let routed = cost.unwrap_or(0).max(0) as u64;
        let at_baseline = pricing::resolve("paid", Some(price), Some(&usage)).micro_usd().unwrap_or(0);
        out.compared += 1;
        out.routed_micro_usd += routed;
        out.baseline_micro_usd += at_baseline;
        for p in [part(&mut out.by_billing, billing), part(&mut out.by_client, &client), part(&mut out.by_route, route)]
        {
            p.compared += 1;
            p.routed_micro_usd += routed;
            p.baseline_micro_usd += at_baseline;
        }
        let start = ((at + offset_ms).div_euclid(bucket_ms)) * bucket_ms - offset_ms;
        let period = match out.periods.last_mut() {
            Some(last) if last.start == start => last,
            _ => {
                out.periods.push(SavingsPeriod { start, ..Default::default() });
                out.periods.last_mut().unwrap()
            }
        };
        period.compared += 1;
        period.routed_micro_usd += routed;
        period.baseline_micro_usd += at_baseline;
    }
    out.routed_micro_usd += out.failed_attempts_micro_usd;
    // Billings in their fixed order; the other slices biggest first.
    const BILLINGS: [&str; 5] = ["free", "local", "subscription", "paid", "unknown"];
    out.by_billing.sort_by_key(|p| BILLINGS.iter().position(|b| *b == p.key).unwrap_or(BILLINGS.len()));
    out.by_client.sort_by(|a, b| b.answered.cmp(&a.answered).then_with(|| a.key.cmp(&b.key)));
    out.by_route.sort_by(|a, b| b.answered.cmp(&a.answered).then_with(|| a.key.cmp(&b.key)));
    Ok(out)
}

pub fn set_baseline(conn: &Connection, baseline: Option<&str>) -> Result<()> {
    store::set_setting(conn, BASELINE_SETTING, baseline.unwrap_or(""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::gateway::pricing::Cost;
    use crate::gateway::store::UsageRecord;

    fn record(conn: &Connection, provider: &str, model: &str, outcome: &str, tokens: Option<(u64, u64)>, cost: &Cost) {
        store::record_usage(
            conn,
            &UsageRecord {
                client_id: None,
                provider_id: provider,
                model_id: model,
                requested_model: "auto",
                input_tokens: tokens.map(|t| t.0),
                output_tokens: tokens.map(|t| t.1),
                cache_tokens: None,
                cost,
                outcome,
                http_status: Some(200),
                latency_ms: 1,
                error: None,
                cache_write_tokens: None,
                session_key: None,
                route_mode: None,
                route_reason: None,
                facts: None,
                repeated_tokens: None,
            },
        )
        .unwrap();
    }

    #[test]
    fn the_estimate_counts_what_it_leaves_out() {
        let conn = db::open_in_memory().unwrap();
        let p = store::add_provider(
            &conn,
            &store::NewProvider {
                kind: "gemini",
                label: "Gemini",
                endpoint: "https://example.test/v1/",
                credential_ref: Some("provider-x"),
                default_billing: "paid",
                wire: "openai",
            },
        )
        .unwrap();
        store::add_model(&conn, &p.id, "cheap").unwrap();
        store::add_model(&conn, &p.id, "strong").unwrap();
        let price = Price {
            input_micro_per_mtok: 2_000_000,
            output_micro_per_mtok: 10_000_000,
            cached_input_micro_per_mtok: None,
            cache_write_micro_per_mtok: None,
            source: "user".into(),
            observed_at: 0,
        };
        store::update_model(&conn, &p.id, "strong", None, None, Some(Some(price))).unwrap();
        let est = |micro| Cost::Estimated { micro_usd: micro, source: "x".into() };
        record(&conn, &p.id, "cheap", "ok", Some((1_000_000, 100_000)), &est(150_000));
        record(&conn, &p.id, "cheap", "ok", Some((1_000, 10)), &Cost::Unknown { source: "no price".into() });
        record(&conn, &p.id, "cheap", "ok", None, &est(0));
        record(&conn, &p.id, "cheap", "fallback", None, &est(20_000));
        record(&conn, &p.id, "cheap", "error", None, &Cost::Unknown { source: "503".into() });

        let none = savings(&conn, 0, 86_400_000, 0).unwrap();
        assert_eq!((none.baseline.as_deref(), none.compared), (None, 0), "nothing compared without a chosen model");
        assert_eq!(none.suggested, None, "only a model clients named and that has a price is proposed");

        set_baseline(&conn, Some(&format!("{}/strong", p.id))).unwrap();
        let s = savings(&conn, 0, 86_400_000, 0).unwrap();
        assert!(s.baseline_priced);
        assert_eq!(s.answered.paid, 3);
        assert_eq!(s.compared, 1);
        // 1M in at $2 + 0.1M out at $10 = $3.00.
        assert_eq!(s.baseline_micro_usd, 3_000_000);
        // $0.15 for the answer plus $0.02 for the failed attempt.
        assert_eq!((s.routed_micro_usd, s.failed_attempts_micro_usd), (170_000, 20_000));
        assert_eq!((s.left_out_unknown_cost, s.left_out_no_tokens, s.failed_unknown_cost), (1, 1, 1));
        assert_eq!(s.fell_back, 1, "the fallback attempt counts as a request kept alive");
        // The slices: every answer is paid (3), asked for as auto (3), from no client (3); one compared in each.
        let paid = s.by_billing.iter().find(|p| p.key == "paid").unwrap();
        assert_eq!(
            (paid.answered, paid.compared, paid.routed_micro_usd, paid.baseline_micro_usd),
            (3, 1, 150_000, 3_000_000)
        );
        assert_eq!(
            s.by_route,
            vec![SavingsPart {
                key: "auto".into(),
                answered: 3,
                compared: 1,
                routed_micro_usd: 150_000,
                baseline_micro_usd: 3_000_000
            }]
        );
        assert_eq!((s.by_client.len(), s.by_client[0].key.as_str()), (1, ""));
        assert_eq!(s.periods.len(), 1);
        assert_eq!(
            (s.periods[0].compared, s.periods[0].routed_micro_usd, s.periods[0].baseline_micro_usd),
            (1, 150_000, 3_000_000)
        );
        assert_eq!(
            (route_kind("coding"), route_kind("gemini/gemini-2.5-flash"), route_kind("auto/cheap")),
            ("combo", "named", "auto/cheap")
        );

        // Asked for by name, paid and priced: proposed while no baseline is chosen.
        set_baseline(&conn, None).unwrap();
        store::record_usage(
            &conn,
            &UsageRecord {
                client_id: None,
                provider_id: &p.id,
                model_id: "strong",
                requested_model: &format!("{}/strong", p.id),
                input_tokens: Some(10),
                output_tokens: Some(1),
                cache_tokens: None,
                cost: &est(1),
                outcome: "ok",
                http_status: Some(200),
                latency_ms: 1,
                error: None,
                cache_write_tokens: None,
                session_key: None,
                route_mode: None,
                route_reason: None,
                facts: None,
                repeated_tokens: None,
            },
        )
        .unwrap();
        assert_eq!(savings(&conn, 0, 86_400_000, 0).unwrap().suggested, Some(format!("{}/strong", p.id)));
    }
}
