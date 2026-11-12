//! Rule-based Auto routing (ROADMAP P0-5, ADR-006). `auto` and `auto/cheap` are model names a client can
//! ask for; the gateway builds the route from the models the user connected, at request time.
//!
//! 1. **Candidates** are the enabled models of enabled connections.
//! 2. **Hard filters**, each with a reason the user can read. The client may use the connection, and its API
//!    format is reachable from the request's. The model is not cooling down, and its quota group is not held.
//!    What the request needs (tools, images, its prompt plus output limit) is *known* to fit: unknown counts
//!    as missing. Its cost can be stated (a price, or free, local or subscription billing; a subscription uses
//!    a plan's quota, not money). For `auto`, its tier is set and is at or above the client's minimum.
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
    /// An Anthropic request's `thinking.type` (`adaptive`, `enabled`, `disabled`, `between_tools`). Claude Code sends
    /// `adaptive` to a model it does not know, and models that do not take a type answer 400.
    #[serde(default)]
    pub thinking: Option<String>,
    /// An Anthropic request's `output_config.effort` (`low` to `max`).
    #[serde(default)]
    pub effort: Option<String>,
}

impl Needs {
    pub fn of(surface: Surface, body: &Value) -> Needs {
        let tools = body["tools"].as_array().is_some_and(|t| !t.is_empty());
        let image_part = |part: &Value| matches!(part["type"].as_str(), Some("image_url" | "image" | "input_image"));
        // Chat Completions and Messages carry `messages`; the Responses API carries `input` items.
        let turns = body["messages"].as_array().or_else(|| body["input"].as_array());
        let vision = turns.is_some_and(|messages| messages.iter().any(|m| {
            m["content"].as_array().is_some_and(|parts| parts.iter().any(image_part))
        }));
        let max_tokens = ["max_tokens", "max_completion_tokens", "max_output_tokens"].iter().find_map(|k| body[*k].as_u64());
        let anthropic = surface == Surface::Anthropic;
        // Any type the request names: Anthropic's list has adaptive, enabled, disabled and between_tools, and a model
        // answers 400 to one it does not list (Claude Opus 5.5 refuses disabled).
        let thinking = body["thinking"]["type"].as_str().filter(|t| anthropic && !t.is_empty()).map(String::from);
        let effort = body["output_config"]["effort"].as_str().filter(|_| anthropic).map(String::from);
        Needs { surface_anthropic: anthropic, tools, vision, input_tokens: estimate_input_tokens(body), max_tokens, thinking, effort }
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
                    Strategy::Tier => format!("tier {}{} (minimum {}); {}; {reason_cost}",
                        tier.as_deref().unwrap_or("?"),
                        match model.capabilities.tier_source.as_deref() {
                            Some(store::CURATED) => " from Pygg's table",
                            Some(super::reference::TIER_SOURCE) => " from its list price",
                            _ => "",
                        },
                        min_tier.as_deref().unwrap_or("?"), model.billing),
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
    // A web session (ADR-035) is slow, unofficial and, for an agent like Muse, acts as the signed-in user: it serves
    // only a request that names it or a combo the user built, never one auto chose.
    if super::presets::is_web(&provider.kind) {
        return no(crate::websession::NOT_FOR_AUTO.into());
    }
    if let Err(why) = client.allows(provider) {
        return no(why);
    }
    // Both formats reach both kinds of target since ROADMAP B5: OpenAI → Anthropic is translated too.
    if let Some(health) = store::blocking_health(conn, &provider.id, &model.model_id)? {
        return no(format!("{} ({})", health.state.replace('_', " "), health.reason));
    }
    if let Some((holder, hold)) = store::group_hold(conn, provider, &model.model_id)? {
        return no(format!("shares limits with {}, which is {}", holder.label, hold.state.replace('_', " ")));
    }
    if let Some(why) = super::quota::exhausted(conn, provider, model, crate::db::now_ms())? {
        return no(why);
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
    // Thinking and effort reach only a native Anthropic target; a translated route drops them and says so.
    if provider.wire == "anthropic" {
        if let Some(kind) = &needs.thinking {
            match &caps.thinking_types {
                Some(types) if types.contains(kind) => {}
                Some(_) => return no(format!("does not take thinking: {kind}")),
                None => return no(format!("unknown whether it takes thinking: {kind} (refresh its model list, or set it)")),
            }
        }
        if let Some(level) = &needs.effort {
            match &caps.effort_levels {
                Some(levels) if levels.contains(level) => {}
                Some(_) => return no(format!("does not take effort: {level}")),
                None => return no(format!("unknown whether it takes effort: {level} (refresh its model list, or set it)")),
            }
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
        // A plan with a quota (a token plan, a subscription): no money per request, so its estimate is $0, and it
        // ranks after free and before paid. Its quota is not tracked yet; when it runs out, the provider's 429
        // cools it down and the route moves on.
        "subscription" => Ok(Ok((0, "the plan's quota, not money; quota left not tracked".into()))),
        _ => match &model.price {
            Some(price) => {
                // A strict limit holds the most the request could cost, as the gateway's admission does.
                let strict = client.strict_budget && client.monthly_budget_micro_usd.is_some();
                let estimate = match (strict, needs.max_tokens, caps.max_output_tokens) {
                    (true, None, Some(most)) => Needs { max_tokens: Some(most), ..needs.clone() }.estimate_micro(price),
                    (true, None, None) => return no("this client's limit is strict, and neither the request nor the model states a maximum output".into()),
                    _ => needs.estimate_micro(price),
                };
                let which = if price.source == "user" { "your price" } else { "the provider's list price" };
                Ok(Ok((estimate, format!("about {} at {which}", money(estimate)))))
            }
            None => no("price unknown".into()),
        },
    }
}

/// Why nothing was eligible, grouped so that a store with hundreds of models still says what to fix: one group per
/// connection and reason (numbers aside), the largest first, each with an example model and how many more share it.
/// Found when an owner's 584 models were refused with the first 12 reasons, all from one connection.
pub fn summarize(excluded: &[Excluded], groups: usize) -> String {
    // (connection, reason with digits masked, example, count), in first-seen order.
    let mut found: Vec<(String, String, String, usize)> = Vec::new();
    for e in excluded {
        let key: String = e.reason.chars().map(|c| if c.is_ascii_digit() { '#' } else { c }).collect();
        match found.iter_mut().find(|g| g.0 == e.provider_id && g.1 == key) {
            Some(g) => g.3 += 1,
            None => {
                let reason: String = e.reason.chars().take(200).collect();
                found.push((e.provider_id.clone(), key, format!("{}/{}: {reason}", e.provider_id, e.model_id), 1));
            }
        }
    }
    found.sort_by(|a, b| b.3.cmp(&a.3));
    let mut parts: Vec<String> = found.iter().take(groups).map(|(connection, _, example, count)| match count {
        1 => example.clone(),
        n => format!("{example} (and {} more on {connection} for the same reason)", n - 1),
    }).collect();
    let rest: usize = found.iter().skip(groups).map(|g| g.3).sum();
    if rest > 0 {
        parts.push(format!("{rest} more for other reasons"));
    }
    parts.join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::pricing::Price;
    use crate::gateway::store::{Capabilities, NewProvider};

    #[test]
    fn a_refusal_groups_its_reasons_by_connection_largest_first() {
        let ex = |p: &str, m: &str, r: &str| Excluded { provider_id: p.into(), model_id: m.into(), reason: r.into() };
        let mut excluded = vec![ex("claude", "opus", "tool support unknown"), ex("claude", "haiku", "tier T1 is below this client's minimum T2")];
        for i in 0..5 {
            excluded.push(ex("openrouter", &format!("m{i}"), &format!("payment required (you can only afford {} tokens)", 100 + i)));
        }
        excluded.push(ex("cohere", "embed", "no tier set"));
        let text = summarize(&excluded, 3);
        assert!(text.starts_with("openrouter/m0: payment required (you can only afford 100 tokens) (and 4 more on openrouter for the same reason)"), "{text}");
        assert!(text.contains("; claude/opus: tool support unknown; claude/haiku: tier T1"), "{text}");
        assert!(text.ends_with("; 1 more for other reasons"), "{text}");
        assert_eq!(summarize(&excluded[..1], 3), "claude/opus: tool support unknown");
    }

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
    fn thinking_and_effort_go_only_to_anthropic_models_known_to_take_them() {
        let w = world();
        let claude = store::add_provider(&w.conn, &NewProvider { kind: "anthropic", label: "Claude", endpoint: "http://a.test",
            credential_ref: None, default_billing: "paid", wire: "anthropic" }).unwrap();
        let model = |m: &str, thinking: Option<Vec<&str>>, effort: Option<Vec<&str>>| {
            store::add_model(&w.conn, &claude.id, m).unwrap();
            store::update_model(&w.conn, &claude.id, m, None, None, Some(Some(price(100_000, 500_000)))).unwrap();
            store::set_capabilities(&w.conn, &claude.id, m, &Capabilities {
                thinking_types: thinking.map(|t| t.into_iter().map(String::from).collect()),
                effort_levels: effort.map(|e| e.into_iter().map(String::from).collect()),
                ..caps(Some("T3"), 200_000, Some(true)) }).unwrap();
        };
        model("new", Some(vec!["adaptive"]), Some(vec!["low", "high"]));
        model("old", Some(vec!["enabled"]), Some(vec![]));
        model("bare", None, None);
        // What Claude Code sends to a model it does not know.
        let body = serde_json::json!({ "model": "auto", "max_tokens": 1000, "thinking": { "type": "adaptive" },
            "output_config": { "effort": "high" }, "messages": [{ "role": "user", "content": "hi" }] });
        let needs = Needs::of(Surface::Anthropic, &body);
        assert_eq!((needs.thinking.as_deref(), needs.effort.as_deref()), (Some("adaptive"), Some("high")));
        let first = plan(&w.conn, &w.client, "auto", Strategy::Tier, &needs).unwrap();
        assert!(names(&first).contains(&"claude/new".to_string()), "{:?}", names(&first));
        assert_eq!(reason(&first, "old"), "does not take thinking: adaptive");
        assert!(reason(&first, "bare").starts_with("unknown whether it takes thinking: adaptive"));
        // OpenAI-format targets get a translation that drops both, so they stay eligible.
        assert!(names(&first).contains(&"paid/mid".to_string()));

        let effort_only = Needs { surface_anthropic: true, effort: Some("max".into()), input_tokens: 1000, ..Needs::default() };
        let second = plan(&w.conn, &w.client, "auto", Strategy::Tier, &effort_only).unwrap();
        assert_eq!(reason(&second, "new"), "does not take effort: max");
        assert_eq!(reason(&second, "old"), "does not take effort: max");
        // An OpenAI-format request carries neither.
        assert_eq!(Needs::of(Surface::OpenAi, &body).thinking, None);
        let off = serde_json::json!({ "thinking": { "type": "disabled" }, "messages": [] });
        assert_eq!(Needs::of(Surface::Anthropic, &off).thinking.as_deref(), Some("disabled"));
    }

    #[test]
    fn a_free_connection_does_not_make_a_priced_model_free() {
        let conn = crate::db::open_in_memory().unwrap();
        let p = store::add_provider(&conn, &NewProvider { kind: "openrouter", label: "OpenRouter", endpoint: "http://o.test",
            credential_ref: None, default_billing: "free", wire: "openai" }).unwrap();
        let price = |input: u64, output: u64| Price { input_micro_per_mtok: input, output_micro_per_mtok: output,
            cached_input_micro_per_mtok: None, cache_write_micro_per_mtok: None, source: "provider: openrouter /models".into(), observed_at: 1 };
        let listed = |id: &str, p: Option<Price>| store::Listed { id: id.into(), price: p, ..store::Listed::default() };
        store::import_listed(&conn, &p.id, &[listed("deepseek/deepseek-chat", Some(price(270_000, 1_100_000))),
            listed("x/model:free", Some(price(0, 0))), listed("y/unpriced", None)], Some("provider: openrouter /models")).unwrap();
        let billing = |m: &str| store::list_models(&conn, None).unwrap().into_iter().find(|x| x.model_id == m).unwrap().billing;
        assert_eq!(billing("deepseek/deepseek-chat"), "paid", "listed at a price, so charged");
        assert_eq!(billing("x/model:free"), "free");
        assert_eq!(billing("y/unpriced"), "unknown", "only a $0 listing makes a model free here");
        // The user's own word on a model stands.
        store::update_model(&conn, &p.id, "deepseek/deepseek-chat", None, Some("free"), None).unwrap();
        assert_eq!(billing("deepseek/deepseek-chat"), "free");
        // And the ledger follows: a paid model's success is priced, not "connection marked free".
        let model = store::list_models(&conn, None).unwrap().into_iter().find(|x| x.model_id == "x/model:free").unwrap();
        assert_eq!(model.billing, "free");
        let provider = store::get_provider(&conn, &p.id).unwrap().unwrap();
        store::update_model(&conn, &p.id, "deepseek/deepseek-chat", None, Some("unknown"), None).unwrap();
        assert_eq!(store::pricing_for(&conn, &provider, "deepseek/deepseek-chat").unwrap().0, "paid");
        assert_eq!(store::pricing_for(&conn, &provider, "x/model:free").unwrap().0, "free");

        // Where the account itself is free (a trial key), a list price does not make a call paid.
        let trial = store::add_provider(&conn, &NewProvider { kind: "cohere", label: "Cohere", endpoint: "http://c.test",
            credential_ref: None, default_billing: "free", wire: "openai" }).unwrap();
        store::import_listed(&conn, &trial.id, &[listed("command-a", Some(price(2_500_000, 10_000_000)))], Some("models.dev")).unwrap();
        let command = store::list_models(&conn, Some(&trial.id)).unwrap().remove(0);
        assert_eq!((command.billing.as_str(), command.enabled), ("free", true));
    }

    #[test]
    fn a_connection_used_for_its_free_models_starts_with_its_paid_models_off() {
        let conn = crate::db::open_in_memory().unwrap();
        let p = store::add_provider(&conn, &NewProvider { kind: "openrouter", label: "OpenRouter", endpoint: "http://o.test",
            credential_ref: None, default_billing: "free", wire: "openai" }).unwrap();
        let price = |output: u64| Price { input_micro_per_mtok: output / 4, output_micro_per_mtok: output,
            cached_input_micro_per_mtok: None, cache_write_micro_per_mtok: None, source: "provider: openrouter /models".into(), observed_at: 1 };
        let listed = |id: &str, p: Option<Price>| store::Listed { id: id.into(), price: p, ..store::Listed::default() };
        store::import_listed(&conn, &p.id, &[listed("a/paid", Some(price(1_000_000))), listed("b/chosen", Some(price(2_000_000))),
            listed("c/model:free", Some(price(0))), listed("d/unpriced", None)], Some("provider: openrouter /models")).unwrap();
        let state = |m: &str| store::list_models(&conn, None).unwrap().into_iter().find(|x| x.model_id == m)
            .map(|x| (x.enabled, x.enabled_source)).unwrap();
        let off = (false, Some(store::OFF_ON_FREE_CONNECTION.to_string()));
        assert_eq!(state("a/paid"), off);
        assert_eq!(state("c/model:free"), (true, None));
        assert_eq!(state("d/unpriced"), off, "not listed at $0: OpenRouter's own routers list no price and may pick a paid model");

        // The user turns a paid one on: it stays on, and is billed as paid.
        store::update_model(&conn, &p.id, "b/chosen", Some(true), None, None).unwrap();
        assert_eq!(state("b/chosen"), (true, Some("user".into())));
        store::apply_free_connection_rule(&conn).unwrap();
        assert_eq!(state("b/chosen"), (true, Some("user".into())));

        // The connection is now used as paid: what Pygg turned off comes back; the user's choice stays.
        store::update_provider(&conn, &p.id, None, Some("paid"), None).unwrap();
        assert_eq!(state("a/paid"), (true, None));
        store::update_provider(&conn, &p.id, None, Some("free"), None).unwrap();
        assert_eq!(state("a/paid"), off);

        // A $0 price learned later turns an untouched model on; a price above zero off again.
        store::update_model(&conn, &p.id, "d/unpriced", None, None, Some(Some(price(0)))).unwrap();
        assert_eq!(state("d/unpriced"), (true, None));
        store::update_model(&conn, &p.id, "d/unpriced", None, None, Some(Some(price(500_000)))).unwrap();
        assert_eq!(state("d/unpriced"), off);

    }

    #[test]
    fn the_free_model_kinds_in_the_billing_rule_are_the_presets_that_say_so() {
        let kinds: Vec<&str> = crate::gateway::presets::PRESETS.iter().filter(|p| p.free_models).map(|p| p.id).collect();
        assert_eq!(kinds, ["openrouter", "zai", "zai-anthropic"]);
        let listed = store::FREE_MODELS_CONNECTION.split("IN (").nth(1).unwrap().split(')').next().unwrap();
        assert_eq!(listed.split(", ").map(|k| k.trim_matches('\'')).collect::<Vec<_>>(), kinds);
        for p in crate::gateway::presets::PRESETS.iter().filter(|p| p.free_models) {
            assert_eq!(p.default_billing, "free", "{} starts as free", p.id);
        }
    }

    #[test]
    fn the_tier_table_fills_only_what_nobody_set_and_auto_says_so() {
        let conn = crate::db::open_in_memory().unwrap();
        let p = store::add_provider(&conn, &NewProvider { kind: "gemini", label: "Gemini", endpoint: "http://g.test",
            credential_ref: None, default_billing: "free", wire: "openai" }).unwrap();
        for m in ["gemini-2.5-flash", "gemini-2.5-pro", "mystery-1"] {
            store::add_model(&conn, &p.id, m).unwrap();
        }
        let tier = |m: &str| store::list_models(&conn, None).unwrap().into_iter().find(|x| x.model_id == m).unwrap().capabilities;
        let flash = tier("gemini-2.5-flash");
        assert_eq!((flash.tier.as_deref(), flash.tier_source.as_deref()), (Some("T2"), Some(store::CURATED)));
        assert!(flash.tier_note.unwrap().contains("Flash"));
        assert_eq!(tier("mystery-1").tier, None, "a line the table does not know stays unset");

        // The user's tier, and a tier the user cleared, are theirs.
        store::set_capabilities(&conn, &p.id, "gemini-2.5-pro", &Capabilities { tier: Some("T4".into()), ..tier("gemini-2.5-pro") }).unwrap();
        store::set_capabilities(&conn, &p.id, "gemini-2.5-flash", &Capabilities { tier: None, ..tier("gemini-2.5-flash") }).unwrap();
        store::fill_curated_tiers(&conn).unwrap();
        assert_eq!(tier("gemini-2.5-pro").tier.as_deref(), Some("T4"));
        assert_eq!((tier("gemini-2.5-flash").tier, tier("gemini-2.5-flash").tier_source.as_deref()), (None, Some("user")));

        // auto names where a tier came from.
        store::add_model(&conn, &p.id, "gemini-3.1-flash-lite").unwrap();
        let lite = tier("gemini-3.1-flash-lite");
        store::set_capabilities(&conn, &p.id, "gemini-3.1-flash-lite", &Capabilities { context_tokens: Some(1_000_000), ..lite.clone() }).unwrap();
        // That save made it the user's; give the table a fresh model to show the reason on.
        store::add_model(&conn, &p.id, "gemini-3.5-flash").unwrap();
        conn.execute("UPDATE models SET context_tokens = 1000000 WHERE model_id = 'gemini-3.5-flash'", []).unwrap();
        let client = store::create_client(&conn, "T", None).unwrap().0;
        let plan = plan(&conn, &client, "auto", Strategy::Tier, &Needs { input_tokens: 100, ..Needs::default() }).unwrap();
        let ranked = plan.ranked.iter().find(|r| r.model_id == "gemini-3.5-flash").unwrap();
        assert!(ranked.reason.starts_with("tier T2 from Pygg's table (minimum T2)"), "{}", ranked.reason);
    }

    #[test]
    fn claude_models_take_tools_from_pyggs_table_unless_the_user_said_otherwise() {
        let conn = crate::db::open_in_memory().unwrap();
        let p = store::add_provider(&conn, &NewProvider { kind: "anthropic", label: "Claude", endpoint: "https://api.anthropic.com/v1",
            credential_ref: None, default_billing: "paid", wire: "anthropic" }).unwrap();
        let other = store::add_provider(&conn, &NewProvider { kind: "gemini", label: "Gemini", endpoint: "https://g",
            credential_ref: None, default_billing: "paid", wire: "openai" }).unwrap();
        let listed = |id: &str| store::Listed { id: id.into(), context_tokens: Some(200_000), max_output_tokens: Some(64_000),
            ..store::Listed::default() };
        store::import_listed(&conn, &p.id, &[listed("claude-sonnet-5-5"), listed("claude-haiku-5-5")], Some("provider: anthropic /models")).unwrap();
        store::import_listed(&conn, &other.id, &[listed("gemini-x")], Some("provider: gemini /models")).unwrap();
        // The user says one does not take tools; that stays.
        let caps = |pid: &str, m: &str| store::list_models(&conn, Some(pid)).unwrap().into_iter().find(|x| x.model_id == m).unwrap().capabilities;
        store::set_capabilities(&conn, &p.id, "claude-haiku-5-5", &Capabilities { supports_tools: Some(false), ..caps(&p.id, "claude-haiku-5-5") }).unwrap();
        store::import_listed(&conn, &p.id, &[listed("claude-sonnet-5-5"), listed("claude-haiku-5-5")], Some("provider: anthropic /models")).unwrap();
        let sonnet = caps(&p.id, "claude-sonnet-5-5");
        assert_eq!((sonnet.supports_tools, sonnet.context_tokens), (Some(true), Some(200_000)));
        assert_eq!(sonnet.source.as_deref(), Some(format!("provider: anthropic /models; tools: {}", store::CURATED_TOOLS).as_str()));
        assert_eq!(caps(&p.id, "claude-haiku-5-5").supports_tools, Some(false), "the user's word stands");
        assert_eq!(caps(&other.id, "gemini-x").supports_tools, None, "only Anthropic's are filled");
    }

    #[test]
    fn a_strict_limit_prices_the_most_a_request_could_cost() {
        let w = world();
        store::set_client_budget(&w.conn, &w.client.id, Some(5_000_000)).unwrap();
        store::set_client_strict_budget(&w.conn, &w.client.id, true).unwrap();
        let client = store::list_clients(&w.conn).unwrap().into_iter().find(|c| c.id == w.client.id).unwrap();
        let needs = Needs { input_tokens: 1000, ..Needs::default() };
        let strict = plan(&w.conn, &client, "auto/cheap", Strategy::Cheap, &needs).unwrap();
        let loose = plan(&w.conn, &w.client, "auto/cheap", Strategy::Cheap, &needs).unwrap();
        let cost = |p: &Plan, m: &str| p.ranked.iter().find(|r| r.model_id == m).unwrap().estimate_micro_usd;
        // 8192 output tokens (the model's maximum) instead of the 4096 a request without a limit is estimated at.
        assert!(cost(&strict, "mid") > cost(&loose, "mid"), "{} vs {}", cost(&strict, "mid"), cost(&loose, "mid"));
        assert_eq!(reason(&strict, "unpriced"), "price unknown");
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
    fn a_plan_with_a_quota_ranks_after_free_and_before_paid() {
        let w = world();
        store::update_model(&w.conn, "paid", "unpriced", None, Some("subscription"), None).unwrap();
        let plan = plan(&w.conn, &w.client, "auto", Strategy::Tier, &Needs { input_tokens: 1000, ..Needs::default() }).unwrap();
        assert_eq!(names(&plan), ["free/gift", "paid/unpriced", "paid/notools", "paid/mid", "paid/big"]);
        assert!(plan.ranked[1].reason.contains("the plan's quota, not money"));
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
