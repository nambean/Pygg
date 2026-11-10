# OmniRoute Feature Research — SmartTaskRouter Adoption Decision

> Decision record v1.0 · 2026-11-11 · Research and implementation plan only. No OmniRoute runtime or new SmartTaskRouter feature was implemented by this commit.
>
> Primary project objective: **AI Cost Optimizer** for everyday conversations and developer workflows. Default UX: user asks normally and the system selects an authorized, capable model with the best expected total cost, preserving continuity across providers.

## 1. Source provenance and verification level

Primary source: [cheaper-inference/OmniRoute](https://github.com/cheaper-inference/OmniRoute), referenced on branch [release/v3.8.54](https://github.com/cheaper-inference/OmniRoute/tree/release/v3.8.54), reviewed on 2026-11-11.

Reference documentation:
- [Auto-Combo Engine](https://github.com/cheaper-inference/OmniRoute/blob/release/v3.8.54/docs/routing/AUTO-COMBO.md)
- [Compression Engines](https://github.com/cheaper-inference/OmniRoute/blob/release/v3.8.54/docs/compression/COMPRESSION_ENGINES.md)
- [Free Tiers](https://github.com/cheaper-inference/OmniRoute/blob/release/v3.8.54/docs/reference/FREE_TIERS.md)
- [Repository README and feature catalog](https://github.com/cheaper-inference/OmniRoute/blob/release/v3.8.54/README.md)
- [autoCombo services directory](https://github.com/cheaper-inference/OmniRoute/tree/release/v3.8.54/open-sse/services/autoCombo)

**Evidence distinction:** documentation explicitly describes auto routes, composition, quota strategies, compression modes and resilience. Individual referenced source modules are **code review targets**, not independently executed/verified here; no local OmniRoute test suite was run. Marketing statements such as high token-saving percentages, provider/model counts or total free token capacity are upstream claims, not performance guarantees for SmartTaskRouter. Open issues, unmerged PRs and docs-only proposals must never be equated with released, tested features. Validate license and dependencies before reuse; prefer reimplementing small contracts in Rust to wholesale code transfer.

### Feature groups observed upstream

1. Auto aliases: `auto`, `auto/coding`, `auto/cheap`, `auto/fast`, `auto/offline`, `auto/smart`, plus capability × preference combinations such as `auto/coding:cheap`.
2. Auto-Combo candidate ranking: multi-factor health/quota/cost/latency/task-fitness signals; configurable policies and exploration modes.
3. Quota-aware strategies: `headroom`, `quota-weighted`, `reset-aware`, `least-used`, and account/session affinity.
4. Compression engines: `off`, `lite`, `standard`, `aggressive`, `ultra`, `rtk`, `stacked`, experimental special-format modes. Documentation says default stacked path includes session deduplication and lite cleanup.
5. Circuit breaker / connection cooldown / model lockout patterns.
6. Free tier catalog with shared quota pool deduplication, provenance and recurring vs promotional distinction.
7. More ambitious routes (fusion/pipeline/chaos), proxy/web session features, broad MCP management.

## 2. Final decisions: Adopt / Adapt / Defer / Reject

| ID | Upstream pattern | Decision | Priority | Reason and product guardrail |
| --- | --- | --- | --- | --- |
| OR-01 | Auto aliases and zero-configuration model choice | **ADAPT** | P0 | Expose `auto`, `auto/cheap`, `auto/fast`, `auto/coding`, but map each to policy + verified eligibility; everyday Auto Chat needs no alias entry |
| OR-02 | Dynamic candidate factory from connected accounts | **ADOPT** | P0 | Builds candidates from actual user entitlements, capability and available quota; no hard-coded best model |
| OR-03 | Free Tier catalog / shared quota pool accounting | **ADOPT** | P0 | Accurate account-model tier, pooled limits, resets, expiry, source/date and terms; source data must be refreshed and never fabricate allowance |
| OR-04 | Quota-aware account routing | **ADAPT** | P0 | Headroom / fill-first / RR / sticky; only across authorized accounts and respecting shared organization-level limits |
| OR-05 | Circuit breaker + cooldown + hard-error failover | **ADOPT** | P0 | Bounded retry, Retry-After, account/model-specific failures; pre-stream failover only |
| OR-06 | Tool-output compression (RTK-style) | **ADAPT** | P0 (opt-in trials); P1 default after evidence | Begin deterministic dedup/truncation for noisy logs with lossless original; only enable as default on validated safe classes |
| OR-07 | Prompt/context deduplication and lite cleanup | **ADOPT** | P0 | Canonical event IDs, dedup exact repeated chunks, don't delete system/tool schema; benchmark actual token savings |
| OR-08 | Cache-affinity routing | **ADAPT** | P1 | Combined with Shared Memory + account/model affinity and cache observations; no cross-provider cache reuse claims |
| OR-09 | Auto-Combo 16-factor weighted scoring | **ADAPT** | P1 | Start with deterministic hard filters + small explainable rule set; introduce empirically calibrated weights only after sufficient observations |
| OR-10 | Context relay | **ADAPT** | P1 | Our Universal Shared Memory is canonical history + versioned summary + relevant facts + context package, not a proprietary tool state bridge |
| OR-11 | Semantic caching | **DEFER** | P2 | Risk of incorrect near-match reuse, privacy leaks and stale facts; require scope isolation, confidence tests and user controls |
| OR-12 | Advanced natural-language compaction | **DEFER** | P2 | Needs task-specific quality/continuity benchmarks and rollback; no aggressive global default |
| OR-13 | Broad 20-strategy selector UI | **REJECT** | — | Product serves general users; present Auto/Save More/Balanced/Quality, and advanced strategy controls only for power users |
| OR-14 | Fusion / multi-model fan-out + judge | **REJECT from default** | — | Multiplies inference cost, contrary to primary value; possible explicit premium research tool later |
| OR-15 | Chaos routing / speculative parallel requests | **REJECT** | — | Unpredictable spend and low reproducibility |
| OR-16 | TLS impersonation, cookie/session bridge, limit evasion | **REJECT** | — | Unneeded risk to security, terms and privacy |
| OR-17 | Full proxy marketplace / broad platform emulation | **REJECT** | — | Not required for authorized local-first AI cost optimization |
| OR-18 | 100+ MCP admin tools / unrestricted agent policy edits | **DEFER** | P2 | Prefer narrowly scoped, consented read-only diagnostics; critical configuration mutations require explicit human approval |
| OR-19 | Pipeline (multi-stage model outputs) | **ADAPT** | P1/P2 | Only task orchestrator stages with measurable benefit and bounded budget; never automatic on ordinary chat |
| OR-20 | "Offline" model-route aliases | **ADAPT naming** | P1 | Distinguish local/offline inference vs provider quota availability; use `auto/local` for strictly local and `auto/available` for quota-aware candidates |

**Important distinction:** Free/Subscription/API Paid/Local/Unknown = billing/entitlement; T0–T4 = approximate capability; client auth type and quota group are separate dimensions.

## 3. Committed core product UX

### Consumer Auto Chat

- No prompt asking user to choose model, complexity, tier or agent.
- Default **Auto**, configurable **Save More / Balanced / Quality** from settings; all fallbacks respect consent, budget and capability.
- Keep conversation with single canonical ID; a model/account switch occurs only at a safe turn boundary and after affinity/cost analysis.
- "Why this model?" expands to source, billing, tier, switching rationale, input/output/cache token use, actual spend and estimate uncertainty.
- Avoid fabricated savings. Show `Unknown` where cost/quota is not measurable; show free API cost separately from local operating cost.

### Developer Integrations

- One base URL and stable aliases for compatible clients, only after client-specific streaming/tool contract tests.
- `auto` aliases in gateway-only mode must remain pinned per conversation and cannot silently change the model during an agent session.
- Multi-account routing is separate from model selection: the pool chooses an eligible connection to an already-compatible candidate.
- Per-client scoped tokens, allowlists, reservations and event ledger.

### Provider Catalog and Strategy UX

- `Connections > Providers / Models / Accounts / Integrations`; account-model entitlements and quota pool aggregation.
- `Policies > Auto Routing / Account Pools / Cost & Cache / Compression / Budgets`; power-user only, UI initially uses four top-level strategies.
- `Usage > Savings / Token Breakdown / Cache / Account Utilization / Verification Outcomes`.
- `Settings > Memory & Context` continues the universal memory design; no separate provider-specific user memory.

## 4. Reference algorithm (hard filters before ranking)

```text
Request + conversation + workspace + connected accounts
  -> preserve canonical conversation event
  -> detect intent, risk and required capabilities at minimal classifier cost
  -> build eligible (provider, account, exact model, wire-format) candidates
     hard constraints: consent, privacy, provider terms, availability, quota-group,
     compatible modality/tools/context, allowed client and spending cap
  -> assess STAY candidate (cache benefit + conversation/account affinity)
  -> estimate other candidates (inference + context rebuild + compaction + retry)
  -> rank with explainable conservative cost / quality policy
  -> pin chosen account/model for turn, active tools and agent attempt
  -> optionally compress safe redundant content, preserving source
  -> execute with pre-stream bounded failover and circuit breakers
  -> persist provider-reported usage and real outcome
  -> refine measurements asynchronously with no hidden paid calls
```

**Do not copy OmniRoute's weights as authoritative quality scores.** Weights, latency and quality are environment/model/account-specific; the first production release uses policy rules and measured telemetry only.

### Effective cost objective

`C_effective = C_input + C_output + C_cache + C_context_rehydrate + C_compress + C_retry + C_validation + C_local_operating + C_expected_failure`

All costs carry provenance and currency/unit. A subscription allowance is not equivalent to free unlimited requests; local inference with `$0 API` still consumes resources. Optimize quality-adjusted cost per successful answer/task, not token count alone.

## 5. Safe token compression specification

**First release compression rules (lossless or reversible):**
- Exact duplicate line/block detection in noisy tool outputs with preserved counts and positions.
- Hide repetitive progress bars and ANSI escape noise (restore raw on request).
- Keep first and last relevant output windows and all error lines, stack trace frames and command exit codes; if middle is omitted, preserve external raw reference and display a clear truncation marker.
- Protect system/developer instructions, structured tool schemas, JSON key/value data, code blocks containing active edits, retrieved factual citations, the most recent turns and unclosed tool calls.
- Dedup repeated history by stable message/event ID, **not semantic near-duplicate deletion**.
- Per-client/per-task opt-out `compression=off`; record compression ruleset/version and pre/post input-token estimates.

**Rollout:** shadow-only measurements → feature-flagged opt-in → allow default on verified classes if quality does not regress. LLM-based summaries/semantic compaction remain separate from deterministic compression and count their own token cost.

**Do not claim:** "60–90% overall cost savings" based on a benchmark for selected shell output; distinguish tokens removed from transmitted input tokens, cache-billed tokens and actual total spend.

## 6. Free Tier Intelligence & Quota Group specification

- Store account-model entitlement `Free | Subscription | API Paid | Local | Unknown`, provider account, exact model, price/unit, rate-limit group, quota group, recurring/promotional/one-time type, expiry, reset window, supported capabilities, data-use terms, observed source and time.
- Ingest through provider APIs where possible, plus signed/versioned reviewed metadata packs for providers without quota API; never treat a community-maintained catalog as live entitlement proof.
- Avoid multiplying one shared pool's allowance by the number of models drawing from it.
- Hide accounts whose entitlements are disabled, exhausted or privacy-incompatible from eligible routing.
- Reconcile provider responses to local reservation ledger; if backend cannot report quota, show `Unknown` and rely on hard request limits rather than guessing.
- Separate two levels: static *potentially free model catalog* versus *currently usable free entitlement on this connected account*.
- Show provenance and confidence in UI, including the ability to inspect per-account token and quota consumption.

## 7. Resilience and account routing

- Account pool algorithms: Fill First / RR / Sticky RR available as explicit options; advanced quota-weighted and cost-aware use only validated comparable data.
- Circuit breaker hierarchy: provider-level outage vs account auth/limit vs model outage. Avoid spreading one account's 429 to every unrelated account/model.
- Respect Retry-After/reset; bounded exponential backoff with jitter; no 401/403 cycling to bypass restrictions.
- Pre-stream failover only; no output splicing or replay with side effects after partial stream without a new user-initiated attempt.
- Transactionally reserve estimated capacity, settle actual usage and recover on restart.
- Session affinity persists across normal conversation turns and is invalidated when privilege/privacy/health/budget requires.
- Provider quota sharing under one user/org is never treated as independent free capacity.

## 8. Architecture additions (Rust, Tauri, SQLite)

Proposed modules in existing Rust core, **not implementation claims**:

| Module | Responsibility |
| --- | --- |
| `catalog.rs` extensions | model metadata, free tiers, pricing, account entitlements and quota groups |
| `candidate_factory.rs` | build candidate routes from connected account-model entitlements |
| `auto_router.rs` | hard filters, explainable ranking, alias policy and model affinity |
| `health.rs` | circuit breaker, cooldown, quota and availability |
| `account_rotation.rs` | safe account pool selection and atomic reservations |
| `compression.rs` | deterministic tool-output reduction, reversible compaction metadata |
| `cache_telemetry.rs` | per-provider/account prompt-cache usage and switching penalties |
| `routing_metrics.rs` | task/turn-level quality, latency, cost-per-success and explanation |
| `metadata_updates.rs` | source-verified catalog pack update and rollback |

Suggested logical records:

```text
model_catalog(model_id, provider_id, capabilities, pricing_source, observed_at, pack_version)
model_entitlements(account_id, model_id, billing_type, quota_group_id, expires_at,
                   price_json, terms_ref, observed_at, supports_route)
quota_groups(id, provider_tenant_ref, limit_unit, limit_value, reset_at, source)
auto_route_decisions(id, conversation_id, client_id, model_id, account_id,
                     alias, strategy, excluded_reasons_json, chosen_reason, cache_cost_estimate,
                     estimated_total_cost, actual_total_cost, policy_snapshot)
compression_events(id, request_id, ruleset_hash, source_tokens_est, sent_tokens_actual,
                   raw_ref, compressed_ref, protected_spans_json, quality_flag)
circuit_breaker_events(id, scope, scope_id, failure_class, opened_at, retry_at, closed_at)
catalog_updates(id, source, version, signature_status, applied_at, rollback_ref)
```

These are migration candidates; reconcile with existing [Account Rotation](ACCOUNT_API_ROTATION.md) and [Universal Shared Memory](UNIVERSAL_SHARED_MEMORY.md) models instead of duplicating tables.

## 9. API/CLI/UX contracts

```text
GET  /api/routing/aliases
POST /api/routing/simulate
GET  /api/routing/decisions/:requestId
GET  /api/catalog/free
GET  /api/catalog/entitlements
POST /api/catalog/refresh
GET  /api/quota/groups
GET  /api/compression/rules
POST /api/compression/preview
GET  /api/compression/metrics
GET  /api/health/circuits
POST /api/health/recheck       # authorized control only
```

Gateway aliases:
- `auto`: balanced quality-adjusted selection under hard constraints
- `auto/cheap`: prioritize total cost only among capable models
- `auto/fast`: latency emphasis after eligibility filters
- `auto/coding`: coding-capable verified candidates
- `auto/local`: only explicitly allowed local endpoints, no cloud fallback
- `auto/available`: quota/health emphasis (not synonymous with offline)
- `auto/quality`: quality emphasis within spend policy

Aliases are conceptual; no endpoint or package is published just by these docs. Unsupported clients and unverified format translations must be excluded and clearly labeled.

## 10. Validation plan and baseline

**Baseline variants:**
- B0: user-pinned premium model, full-history context.
- B1: static cheapest capable model, no context optimization.
- B2: Auto routing with hard filters and no compression.
- B3: Auto routing + safe deterministic compression.
- B4: Auto routing + Shared Memory and cache-aware affinity.

**Test categories:** short factual questions, rewriting/translation, long multi-turn references, contradictory updated facts, JSON/structured tool outputs, code edits with deterministic checks, image+text (if supported), high-risk requests requiring quality escalation and sensitive-workspace restrictions.

**Measured KPIs:** verified task/answer quality, cost per successful task or acceptable answer, input/output/cache token counts, compression overhead, ratio of unknown costs, routing time overhead, p50/p95 latency, provider quota exhaustion, circuit breaker accuracy, switch frequency, missing-context rate, continuity and privacy failures.

**Adoption gates (do not preclaim results):**
- No known unauthorized account/data disclosure, paid-spend over budget or forbidden cross-provider routing.
- Deterministic compression roundtrips raw output; no protected spans dropped.
- Failed streamed requests do not invoke automatic cross-model replay.
- Contract-tested adapters preserve tool and streaming semantics.
- No statistically meaningful regression of answer quality/continuity against baseline under fixed evaluated cases.
- Measured lower *total cost per accepted result*, not just lower model token price.
- Feature flags and immediate rollback to previous safe policy.

## 11. Prioritized, dependency-aware delivery plan

| Sprint track | Scope | Phase | Acceptance |
| --- | --- | --- | --- |
| OR-P0-1 | Provider catalog + account entitlement + quota group provenance | P0 gateway | Correct per-account model eligibility and shared quota accounting |
| OR-P0-2 | Dynamic candidate factory + safe aliases + explainable hard filters | P0 gateway | Simulate and execute `auto` with verified eligible models |
| OR-P0-3 | Account health/cooldown + pre-stream failover + reservation ledger | P0 gateway | Safe retries under 429/5xx/timeout with concurrency tests |
| OR-P0-4 | Tool output dedup + conservative compression in shadow/opt-in | P0 experiment | Zero protected content lost, pre/post tokens accounted |
| OR-P1-1 | Model affinity + cache telemetry + universal context handoff | P1 memory | Continuity benchmark and verified cost of switching |
| OR-P1-2 | Context-aware task classifier + calibrated score profiles | P1 routing | Better or equivalent quality with lower measured cost |
| OR-P1-3 | Auto Chat integration, route explanations, account/cost inspector | P1 UX | User sends normal messages with no model-selection prompt |
| OR-P2-1 | Optional semantic caching, advanced compaction, MCP read-only diagnostics | P2 optional | Explicit opt-in, independent quality and security gates |

**Interaction with existing ADR-016:** Gateway remains first. Integration is incremental; no rewrite of desktop shell, SQLite base, keychain, worktree runner or current task list. Existing CLI/IDE plan, account rotation and memory specs stay authoritative for their domains.

## 12. Explicitly deferred / rejected scope

**Deferred:** hybrid semantic caching, vector-heavy reranking, aggressive natural-language rewriting, complicated SLA/bandit strategies, unrestricted MCP gateway automation, pipeline orchestration for general consumer chat.

**Rejected for core product:** multi-model fusion as default, chaos routing, stealth credential/session collection, TLS/browser impersonation, quota evasion, proxy marketplace, rotating across accounts that share a single rate-limit identity to circumvent restrictions, unvalidated claim of `free` or `60–90% cost savings`.

## 13. References to update after implementation

- [Product Plan](SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md)
- [Account/API Rotation](ACCOUNT_API_ROTATION.md)
- [Universal Shared Memory](UNIVERSAL_SHARED_MEMORY.md)
- [CLI / IDE / npm / tiers](CLI_IDE_NPM_PROVIDER_TIERS.md)
- [Screen Design](design/SCREENS.md)
- [Architecture Decisions](DECISIONS.md)
- [Implementation Status](IMPLEMENTATION-STATUS.md)

Only update Implementation Status when actual code and tests land.
