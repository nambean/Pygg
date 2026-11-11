//! Rule-based Auto routing (ROADMAP P0-5, ADR-006). `auto` and `auto/cheap` are model names a client can
//! ask for; the gateway builds the route from the models the user connected, at request time.
//!
//! 1. **Candidates** are the enabled models of enabled connections.
//! 2. **Hard filters**, each with a reason the user can read. The client may use the connection, and its API
//!    format is reachable from the request's. The model is not cooling down, and its quota group is not held.
//!    What the request needs (tools, images, its prompt plus output limit) is *known* to fit: unknown counts
//!    as missing. Its cost can be stated (a price, or free or local billing). For `auto`, its tier is set and
//!    is at or above the client's minimum.
//! 3. **Ranking**, with no classifier and no invented quality scores:
//!    - `auto`: the lowest eligible tier at or above the client's minimum, then Local, Free, Subscription and
//!      Paid in that order, then the lower estimated cost. This is "the lowest eligible tier under your
//!      policy", not "the cheapest" (ADR-006).
//!    - `auto/cheap`: the lowest estimated cost of this request among the eligible models, free and local
//!      first. Tier breaks ties upwards. Tier is not required.
//! 4. The first few ranked models become the route, tried in order with the usual error-scoped fallback,
//!    quota groups, budget holds and retry policy. An explicit session stays on the model that answered it.
//!
//! The reason for the choice is returned in `x-smarttaskrouter-auto` and kept in the ledger.

use super::store::{self, Client, Model, Provider};
use super::{estimate_input_tokens, Surface, DEFAULT_OUTPUT_ESTIMATE};
use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;

/// The aliases a client may ask for.
pub const ALIASES: [&str; 2] = ["auto", "auto/cheap"];
/// At most this many ranked models make up the route.
pub const ROUTE_LENGTH: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Strategy {
    /// Lowest eligible tier at or above the client's minimum.
    Tier,
    /// Lowest estimated cost.
    Cheap,
}

/// Whether `requested` is an auto alias, and which.
pub fn strategy(requested: &str) -> Option<Result<Strategy, String>> {
    if requested != store::AUTO && !requested.starts_with("auto/") {
        return None;
    }
    Some(match requested {
        "auto" => Ok(Strategy::Tier),
        "auto/cheap" => Ok(Strategy::Cheap),
        other => Err(format!("\"{other}\" is not an automatic route. Use {}.", ALIASES.join(" or "))),
    })
}

/// What a request needs from a model.
#[derive(Clone, Debug, Default, PartialEq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Needs {
    pub surface_anthropic: bool,
    pub tools: bool,
    pub vision: bool,
    /// Estimated prompt tokens.
    pub input_tokens: u64,
    /// The output limit the request sets, if any.
    pub max_tokens: Option<u64>,
}

impl Needs {
    pub fn of(surface: Surface, body: &Value) -> Needs {
        let tools = body["tools"].as_array().is_some_and(|t| !t.is_empty());
        let image_part = |part: &Value| matches!(part["type"].as_str(), Some("image_url" | "image" | "input_image"));
        let vision = body["messages"].as_array().is_some_and(|messages| messages.iter().any(|m| {
            m["content"].as_array().is_some_and(|parts| parts.iter().any(image_part))
        }));
        let max_tokens = ["max_tokens", "max_completion_tokens", "max_output_tokens"].iter().find_map(|k| body[*k].as_u64());
        Needs { surface_anthropic: surface == Surface::Anthropic, tools, vision, input_tokens: estimate_input_tokens(body),
            max_tokens }
    }

    fn output_tokens(&self) -> u64 {
        self.max_tokens.unwrap_or(DEFAULT_OUTPUT_ESTIMATE)
    }

    /// The estimated cost of this request at `price`: the prompt at the higher of the input and cache-write
    /// prices, plus the output limit (or 4096 tokens).
    fn estimate_micro(&self, price: &crate::gateway::pricing::Price) -> u64 {
        let input_price = price.input_micro_per_mtok.max(price.cache_write_micro_per_mtok.unwrap_or(0));
        let per = |tokens: u64, micro: u64| ((tokens as u128 * micro as u128).div_ceil(1_000_000)) as u64;
        per(self.input_tokens, input_price) + per(self.output_tokens(), price.output_micro_per_mtok)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ranked {
    pub provider_id: String,
    pub model_id: String,
    pub tier: Option<String>,
    pub billing: String,
    /// Estimated cost of this request; 0 for free or local.
    pub estimate_micro_usd: u64,
    /// Why it is here, in ASCII so it fits in a header.
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Excluded {
    pub provider_id: String,
    pub model_id: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub alias: String,
    pub strategy: Strategy,
    pub min_tier: Option<String>,
    /// Ranked, best first. The route is the first `ROUTE_LENGTH`.
    pub ranked: Vec<Ranked>,
    pub excluded: Vec<Excluded>,
}

impl Plan {
    /// A one-line summary for the response header: which model answered, its place in the order, and why.
    pub fn header(&self, chosen: &Ranked) -> String {
        let rank = self.ranked.iter().position(|r| r.provider_id == chosen.provider_id && r.model_id == chosen.model_id)
            .map(|i| i + 1).unwrap_or(0);
        let order = match self.strategy {
            Strategy::Tier => "lowest eligible tier first",
            Strategy::Cheap => "lowest estimated cost first",
        };
        format!("{}; {}/{} ranked {rank} of {} ({order}): {}; {} excluded", self.alias, chosen.provider_id, chosen.model_id,
            self.ranked.len(), chosen.reason, self.excluded.len())
    }
}

fn billing_rank(billing: &str) -> u8 {
    match billing {
        "local" => 0,
        "free" => 1,
        "subscription" => 2,
        _ => 3,
    }
}

fn tier_rank(tier: &str) -> u8 {
    store::TIERS.iter().position(|t| *t == tier).unwrap_or(0) as u8
}

fn money(micro: u64) -> String {
    // Under a hundredth of a cent, four decimals would read as $0.0000, which is not what it costs.
    if micro > 0 && micro < 100 { "under $0.0001".into() } else { format!("${:.4}", micro as f64 / 1e6) }
}

/// Builds the plan for a request. Nothing is sent; the same function serves the simulator in the app.
pub fn plan(conn: &Connection, client: &Client, alias: &str, strategy: Strategy, needs: &Needs) -> store::Result<Plan> {
    let providers = store::list_providers(conn)?;
    let min_tier = (strategy == Strategy::Tier).then(|| client.auto_min_tier.clone());
    let mut ranked: Vec<(Ranked, (u64, u8, u8, u8))> = Vec::new();
    let mut excluded = Vec::new();
    for model in store::list_models(conn, None)? {
        let Some(provider) = providers.iter().find(|p| p.id == model.provider_id) else { continue };
        if !provider.enabled || !model.enabled {
            continue;
        }
        match judge(conn, client, provider, &model, strategy, min_tier.as_deref(), needs)? {
            Ok((estimate, reason_cost)) => {
                let tier = model.capabilities.tier.clone();
                let t = tier.as_deref().map(tier_rank).unwrap_or(0);
                let b = billing_rank(&model.billing);
                // The facts it was ranked on; its place in the order is in the header.
                let reason = match strategy {
                    Strategy::Tier => format!("tier {} (minimum {}); {}; {reason_cost}",
                        tier.as_deref().unwrap_or("?"), min_tier.as_deref().unwrap_or("?"), model.billing),
                    Strategy::Cheap => format!("{}; {reason_cost}", model.billing),
                };
                // Tier: lowest tier, then billing (local, free, subscription, paid), then estimated cost.
                // Cheap: estimated cost, then billing, then the higher tier.
                let key = match strategy {
                    Strategy::Tier => (estimate, t, b, 0),
                    Strategy::Cheap => (estimate, 0, b, 4 - t),
                };
                ranked.push((Ranked { provider_id: model.provider_id.clone(), model_id: model.model_id.clone(), tier,
                    billing: model.billing.clone(), estimate_micro_usd: estimate, reason }, key));
            }
            Err(reason) => excluded.push(Excluded { provider_id: model.provider_id.clone(), model_id: model.model_id.clone(), reason }),
        }
    }
    ranked.sort_by(|(a, ka), (b, kb)| {
        let order = match strategy {
            Strategy::Tier => (ka.1, ka.2, ka.0).cmp(&(kb.1, kb.2, kb.0)),
            Strategy::Cheap => (ka.0, ka.2, ka.3).cmp(&(kb.0, kb.2, kb.3)),
        };
        order.then_with(|| (&a.provider_id, &a.model_id).cmp(&(&b.provider_id, &b.model_id)))
    });
    Ok(Plan { alias: alias.to_string(), strategy, min_tier, ranked: ranked.into_iter().map(|(r, _)| r).collect(), excluded })
}

/// Whether one model may serve the request: its estimated cost and how it was stated, or why not.
fn judge(conn: &Connection, client: &Client, provider: &Provider, model: &Model, strategy: Strategy,
    min_tier: Option<&str>, needs: &Needs) -> store::Result<Result<(u64, String), String>> {
    let caps = &model.capabilities;
    let no = |reason: String| Ok(Err(reason));
    if !client.may_use(&provider.id) {
        return no(format!("this client may not use {}", provider.label));
    }
    if !needs.surface_anthropic && provider.wire == "anthropic" {
        return no("speaks the Anthropic API, which an OpenAI-format request cannot reach yet".into());
    }
    if let Some(health) = store::blocking_health(conn, &provider.id, &model.model_id)? {
        return no(format!("{} ({})", health.state.replace('_', " "), health.reason));
    }
    if let Some((holder, hold)) = store::group_hold(conn, provider, &model.model_id)? {
        return no(format!("shares limits with {}, which is {}", holder.label, hold.state.replace('_', " ")));
    }
    if strategy == Strategy::Tier {
        let Some(tier) = caps.tier.as_deref() else { return no("no tier set".into()) };
        let floor = min_tier.unwrap_or("T0");
        if tier_rank(tier) < tier_rank(floor) {
            return no(format!("tier {tier} is below this client's minimum {floor}"));
        }
    }
    if needs.tools {
        match caps.supports_tools {
            Some(true) => {}
            Some(false) => return no("does not take tools".into()),
            None => return no("tool support unknown".into()),
        }
    }
    if needs.vision {
        match caps.supports_vision {
            Some(true) => {}
            Some(false) => return no("does not take images".into()),
            None => return no("image support unknown".into()),
        }
    }
    let Some(context) = caps.context_tokens else { return no("context size unknown".into()) };
    let wanted = needs.input_tokens + needs.output_tokens();
    if wanted > context {
        return no(format!("about {} prompt and {} output tokens do not fit its {context}-token context",
            needs.input_tokens, needs.output_tokens()));
    }
    if let Some(limit) = needs.max_tokens {
        match caps.max_output_tokens {
            Some(max) if limit > max => return no(format!("the request allows {limit} output tokens; it gives at most {max}")),
            Some(_) => {}
            None => return no("maximum output unknown, and the request sets one".into()),
        }
    }
    match model.billing.as_str() {
        "free" | "local" => Ok(Ok((0, format!("{} (as set)", model.billing)))),
        "subscription" => no("subscription quota is not tracked yet, so its cost cannot be stated".into()),
        _ => match &model.price {
            Some(price) => {
                let estimate = needs.estimate_micro(price);
                let which = if price.source == "user" { "your price" } else { "the provider's list price" };
                Ok(Ok((estimate, format!("about {} at {which}", money(estimate)))))
            }
            None => no("price unknown".into()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::pricing::Price;
    use crate::gateway::store::{Capabilities, NewProvider};

    fn price(input: u64, output: u64) -> Price {
        Price { input_micro_per_mtok: input, output_micro_per_mtok: output, cached_input_micro_per_mtok: None,
            cache_write_micro_per_mtok: None, source: "user".into(), observed_at: 0 }
    }

    fn caps(tier: Option<&str>, context: u64, tools: Option<bool>) -> Capabilities {
        Capabilities { tier: tier.map(String::from), context_tokens: Some(context), max_output_tokens: Some(8192),
            supports_tools: tools, supports_vision: Some(false), ..Capabilities::default() }
    }

    struct World {
        conn: Connection,
        client: Client,
    }

    fn world() -> World {
        let conn = crate::db::open_in_memory().unwrap();
        let add = |label: &str, billing: &str| store::add_provider(&conn, &NewProvider { kind: "custom", label,
            endpoint: "http://x.test", credential_ref: None, default_billing: billing, wire: "openai" }).unwrap();
        let paid = add("Paid", "paid");
        let free = add("Free", "free");
        store::update_provider(&conn, &free.id, None, None, Some(true)).unwrap();
        let model = |p: &str, m: &str, c: Capabilities, pr: Option<Price>| {
            store::add_model(&conn, p, m).unwrap();
            store::update_model(&conn, p, m, None, None, Some(pr)).unwrap();
            store::set_capabilities(&conn, p, m, &c).unwrap();
        };
        model(&paid.id, "small", caps(Some("T1"), 32_000, Some(true)), Some(price(50_000, 200_000)));
        model(&paid.id, "mid", caps(Some("T2"), 128_000, Some(true)), Some(price(300_000, 1_200_000)));
        model(&paid.id, "big", caps(Some("T3"), 200_000, Some(true)), Some(price(3_000_000, 15_000_000)));
        model(&paid.id, "notools", caps(Some("T2"), 128_000, Some(false)), Some(price(100_000, 400_000)));
        model(&paid.id, "unpriced", caps(Some("T2"), 128_000, Some(true)), None);
        model(&paid.id, "untiered", caps(None, 128_000, Some(true)), Some(price(10_000, 40_000)));
        model(&free.id, "gift", caps(Some("T2"), 64_000, None), None);
        let client = store::create_client(&conn, "Tests", None).unwrap().0;
        World { conn, client }
    }

    fn names(plan: &Plan) -> Vec<String> {
        plan.ranked.iter().map(|r| format!("{}/{}", r.provider_id, r.model_id)).collect()
    }

    fn reason<'a>(plan: &'a Plan, model: &str) -> &'a str {
        &plan.excluded.iter().find(|e| e.model_id == model).unwrap().reason
    }

    #[test]
    fn auto_takes_the_lowest_eligible_tier_and_prefers_free_within_it() {
        let w = world();
        let needs = Needs { input_tokens: 1000, ..Needs::default() };
        let plan = plan(&w.conn, &w.client, "auto", Strategy::Tier, &needs).unwrap();
        // T2 minimum: free gift first (T2, free), then paid T2s by cost, then T3. T1 and untiered are out.
        assert_eq!(names(&plan), ["free/gift", "paid/notools", "paid/mid", "paid/big"]);
        assert_eq!(reason(&plan, "small"), "tier T1 is below this client's minimum T2");
        assert_eq!(reason(&plan, "untiered"), "no tier set");
        assert_eq!(reason(&plan, "unpriced"), "price unknown");
        assert!(plan.ranked[0].reason.starts_with("tier T2 (minimum T2); free"));
        assert!(plan.header(&plan.ranked[0]).starts_with("auto; free/gift ranked 1 of 4 (lowest eligible tier first): tier T2"));
    }

    #[test]
    fn unknown_counts_as_missing_for_what_the_request_needs() {
        let w = world();
        let needs = Needs { tools: true, input_tokens: 1000, ..Needs::default() };
        let plan = plan(&w.conn, &w.client, "auto", Strategy::Tier, &needs).unwrap();
        assert_eq!(names(&plan), ["paid/mid", "paid/big"]);
        assert_eq!(reason(&plan, "gift"), "tool support unknown");
        assert_eq!(reason(&plan, "notools"), "does not take tools");
        // A prompt that only the largest context fits.
        let long = Needs { input_tokens: 150_000, ..Needs::default() };
        let plan = super::plan(&w.conn, &w.client, "auto", Strategy::Tier, &long).unwrap();
        assert_eq!(names(&plan), ["paid/big"]);
        assert!(reason(&plan, "mid").contains("do not fit its 128000-token context"));
        // An output limit above what a model gives.
        let wide = Needs { input_tokens: 10, max_tokens: Some(16_000), ..Needs::default() };
        assert!(super::plan(&w.conn, &w.client, "auto", Strategy::Tier, &wide).unwrap().ranked.is_empty());
    }

    #[test]
    fn auto_cheap_ranks_by_estimated_cost_and_needs_no_tier() {
        let w = world();
        let needs = Needs { input_tokens: 2000, max_tokens: Some(500), ..Needs::default() };
        let plan = plan(&w.conn, &w.client, "auto/cheap", Strategy::Cheap, &needs).unwrap();
        assert_eq!(names(&plan), ["free/gift", "paid/untiered", "paid/small", "paid/notools", "paid/mid", "paid/big"]);
        assert_eq!(plan.ranked[0].estimate_micro_usd, 0);
        assert!(plan.ranked[1].estimate_micro_usd < plan.ranked[2].estimate_micro_usd);
    }

    #[test]
    fn the_client_minimum_and_allowlist_apply() {
        let w = world();
        store::set_client_auto_min_tier(&w.conn, &w.client.id, "T3").unwrap();
        store::set_client_connections(&w.conn, &w.client.id, Some(&["paid".to_string()])).unwrap();
        let client = store::list_clients(&w.conn).unwrap().remove(0);
        let plan = plan(&w.conn, &client, "auto", Strategy::Tier, &Needs::default()).unwrap();
        assert_eq!(names(&plan), ["paid/big"]);
        assert_eq!(reason(&plan, "gift"), "this client may not use Free");
    }

    #[test]
    fn aliases_are_recognized_and_unknown_ones_refused() {
        assert_eq!(strategy("auto"), Some(Ok(Strategy::Tier)));
        assert_eq!(strategy("auto/cheap"), Some(Ok(Strategy::Cheap)));
        assert!(matches!(strategy("auto/fast"), Some(Err(_))));
        assert_eq!(strategy("main/auto"), None);
        assert_eq!(strategy("automatic"), None);
    }
}
