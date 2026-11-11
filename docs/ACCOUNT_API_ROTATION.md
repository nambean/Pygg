# Multi-Account and API Key Rotation — Account Pool Router

> **Status:** Reference · account pool specification; partly built, pools are ROADMAP E · reviewed 2026-11-11 · order: [ROADMAP](ROADMAP.md) · built: [Implementation Status](IMPLEMENTATION-STATUS.md)

> **Update 2026-11-11 (ADR-026):** account pools apply to API-key and token-plan connections only. Subscriptions are used through the vendors' agent CLIs with their own sign-in, which Pygg never reads, so subscription sessions are not pooled or rotated. References to consumer Auto Chat, and to Universal Shared Memory context packages and handoff (§3, §6), no longer apply (ADR-028); pools would serve gateway clients, under the gateway's explicit session affinity (P0-4).


> Specification v1.0 · 2026-11-11 · **Partly built:** client allowlists, quota groups, error-scoped fallback, cooldowns, spend reservations and a circuit breaker with a probe; account pools and rotation strategies are not (ROADMAP E).
>
> Product: AI Cost Optimizer. Account rotation is an **availability, entitlement usage and cost-control mechanism** for authorized connections, not a way to evade provider quotas, bans, account restrictions, subscriptions or terms.

## Architecture review correction — failure scopes and current implementation (2026-11-11)

When this was written the gateway fell back on 401/403/429 and connection errors across any combo member. P0-2 replaced that with error-scoped fallback, quota-group holds and per-client allowlists, and B4 added the circuit breaker and probe; account pools are still not built (ROADMAP E).

- 401: authentication invalid on this credential; do not blindly retry through other identities.
- 403: classify model permission, organization policy or account suspension by provider response; block at the **correct scope** (not all accounts sharing provider name indiscriminately).
- 429: honor `Retry-After`, respect shared organization/quota group and terms; no cycling accounts to sidestep a shared cap.
- Ambiguous read timeout: provider may have run/billed; **no automatic fallback**. Connection-not-established failures may have a safe bounded fallback.
- No automatic switch after response start or an active tool call; no hidden context/model changes.
- Client and account budget reservations must be transactionally protected. A concurrent semaphore alone is insufficient.

Account affinity requires a **client-supplied, authenticated session key**. Do not guess persistent session identity from hash(system prompt + first message). Without a session ID, a gateway-only client owns its conversation context and the router cannot promise cross-turn continuity.

## 1. Sources and design inspiration

9Router documentation and source architecture describe multi-account per provider, account fallback and combo fallback:
- https://github.com/decolua/9router/blob/master/docs/ARCHITECTURE.md
- https://docs.9router.com/
- https://github.com/decolua/9router/pull/1 (round-robin, sticky round-robin and mutex/race-condition lessons)
- https://github.com/decolua/9router/issues/3893 (UI mentions Fill First vs Round Robin, sticky limits)
- https://github.com/decolua/9router/issues/1075 (feature request for combo account pinning)
- https://github.com/decolua/9router/discussions/4153 (feature proposal for client-key allowlists)
- https://github.com/decolua/9router/issues/5359 (proposed quota lockout/reset protection)
- https://github.com/decolua/9router/pull/5316 (session stickiness proposal; check whether merged before adopting).

**Evidence caveat:** issues/discussions and open PRs may be proposals, not shipped features. Use verified mainline source and tests for behavior claims; independently implement and test in Rust. Do not adopt traffic interception, credential extraction or proxy/IP tricks to defeat access restrictions.

## 2. Terminology

- **Provider**: Anthropic, OpenAI, OpenRouter, Gemini, custom-compatible, Ollama, etc.
- **Account / Connection**: one independently authorized endpoint/credential/official session, with its own health, limits, terms and data policy.
- **Model entitlement**: access to one exact model within a connection; can be Free, Subscription, API Paid, Local or Unknown.
- **Account pool**: explicitly permitted connections that support a compatible model/route. Membership and limits are scoped to client token, workspace and privacy policy.
- **Combo**: an ordered/weighted sequence of model+entitlement candidates; **model fallback** is separate from **account fallback**.
- **Session affinity**: durable mapping `client_id + conversation_id + provider_format + model_alias` → selected connection/model. Protects conversation continuity and provider-side prompt caching.
- **Cooldown**: temporary suspension after certain transient failures, ideally driven by `Retry-After` or provider reset metadata.

## 3. Account selection strategies

| Strategy | Behavior | Best for | Guardrails |
| --- | --- | --- | --- |
| `fill_first` | Prefer ordered account until healthy capacity is insufficient | Use near-expiring authorized allowance, predictable costs | Must honor explicit limits/quotas and health |
| `round_robin` | Rotate eligible accounts for **new independent sessions/tasks** | Distribute load fairly | Do not rotate every message of an active conversation |
| `sticky_round_robin` | Round-robin on session start; pin selected account for session or bounded turn count | Cache reuse and stable conversations | A turn-count cap cannot force a mid-tool call switch |
| `weighted` | Assign new sessions by configured weights, price and permitted capacity | Heterogeneous accounts and budgets | Weight is not entitlement; never use unavailable capacity |
| `least_used` | Prefer lowest measured share of permitted capacity in relevant window | Smooth legitimate throughput | Only calculate with comparable, verified quota units |
| `cost_aware` | Among eligible accounts optimize predicted verified cost, continuity, latency and reliability | Pygg default in Auto mode | Include cache miss and context handoff overhead |
| `pinned` | Always use designated account for explicit client/model mapping | Stable tools, strict enterprise policy | No silent fallback outside pin |
| `failover_only` | Use primary until genuine transient failure; fail over only when safe | Simplicity / stable cache | No fallback after stream has started |

**Default recommendation:** `cost_aware + session_affinity` in consumer Auto Chat; `sticky_round_robin` as an option for multiple equivalent accounts; `fill_first` available for predictable quota use. Existing agent session/attempt remains pinned throughout execution. Cross-provider switching only at a safe boundary and through the Universal Shared Memory/Context Handoff contract.

For subscription-backed CLI agents, account selection is only via official supported configuration/authentication and provider-permitted behavior; generic gateway key swapping is **not** assumed supported.

## 4. Eligibility before rotation

A connection is eligible only if:
1. User explicitly connected and enabled it, and routing is permitted by its provider terms.
2. Client API token, workspace, model alias and policy allow that exact provider/account/model.
3. Model and wire format support required tools, streaming, structured output, images and context length.
4. Privacy / provider training terms match the conversation or workspace.
5. Confirmed billing tier and cost fit policy; Unknown is never silently zero.
6. Health is suitable and the account is not disabled, revoked, cooling down or exhausted.
7. Rate-limit and remaining entitlement checks are respected; limited or unavailable accounts do not get retries to exceed provider limits.
8. Session affinity/pinning constraints are satisfied, unless a recorded, permitted safe failover is invoked.

Never pool accounts owned by unrelated users or borrowed credentials. Never rotate IPs/accounts to circumvent per-user, per-organization or per-plan restrictions; provider limits and ToS remain hard constraints.

## 5. Failure classification and backoff

| Event | Action | When fallback allowed |
| --- | --- | --- |
| HTTP 429 with `Retry-After` | Honor retry time; mark this account/model cooling down; no immediate retries | Only to another independently authorized eligible route if provider rules permit and no output emitted |
| Explicit quota exhausted | Quarantine until verified reset or manual revalidation | Same rules; never cycle through identities to sidestep a shared provider quota |
| 401/invalid credential | Disable connection and request user repair | A separate permitted account can serve a new request; don't silently rotate compromised credentials |
| 403/suspended or policy block | Block route; show cause; do not treat as ordinary transient error | **No evasion fallback** |
| 5xx/network timeout | Exponential backoff with bounded jitter and circuit breaker | Pre-stream only, with idempotency safeguards |
| Proxy/transport failure | Report route failure; strict network-route settings fail closed | Do not mislabel account as exhausted or silently leak traffic direct |
| Context/tool incompatibility | Exclude candidate as incapable | Other compatible candidates only; not a retryable account failure |
| Stream already emitted tokens/tool outputs | Preserve partial response and error state | No automatic replay unless client explicitly requests a new attempt; never splice different model outputs invisibly |

**Circuit breaker state:** `healthy | degraded | cooling_down | exhausted | invalid_credentials | policy_blocked | disabled`, reason/source/time, `next_check_at`; use health probing only when safe. No busy loops.

## 6. Session affinity and Universal Memory

- Canonical conversation ID belongs to Pygg, not an upstream provider.
- Pin account/model for an active turn, streaming response and ongoing tool call; for coding agents pin for the entire attempt.
- Persist affinity with account/model, last-used, TTL, prior-success metadata and cache observations.
- A fallback prior to first emitted byte may update affinity after a confirmed successful response. A failed/pre-stream attempt may be billed; record it.
- On later turns, prefer the pinned account if eligible and economically reasonable. Compare measured cache benefit vs switching cost before moving.
- On a permitted switch, create a scoped `Context Package` from raw history + recent turns + verified memory (see [Universal Shared Memory](UNIVERSAL_SHARED_MEMORY.md)). Never copy provider KV cache or hidden reasoning.
- Cache and quota should be partitioned **per account, model, endpoint and pricing window**, never aggregated as if one common pool.
- Key session affinity by authenticated client scope and session/conversation ID; avoid merging conversations merely because they share a model alias.

## 7. Concurrency, budget and fairness

**Atomic reservation algorithm:**
1. Read eligible accounts, current health, quotas, policy and existing affinity.
2. Under a SQLite transaction or equivalent scoped synchronization, reserve expected per-account budget/concurrency and choose an account.
3. Persist route decision with policy snapshot, account ID, model ID, strategy, excluded reasons and reservation ID.
4. Execute request with idempotency key where protocol allows.
5. Settle reservation using actual usage; mark unknown actual cost explicitly, never as $0.
6. Update health and affinity after result; retry/fallback only under policy and pre-stream rules.

- Use per-account semaphores and a transactionally consistent cursor for round-robin; multiple desktop/CLI clients must not select the same exhausted capacity.
- Guard against double reservation, cancellation, process restart and late usage reconciliation.
- Apply token and account-scoped daily/monthly budgets; optional per-client/API-key allowlists.
- If provider reports a **shared** quota for accounts belonging to the same organization/user, represent `quota_group_id` and avoid pretending they have independent capacity.
- Configurable fairness weighted by authorized capacity; prioritize correctness over maximizing every last request.

## 8. UI design

### Connections → Accounts (new subtab)

```text
Connections / Providers / Accounts
[All] [Free] [Subscription] [API Paid] [Local] [Unknown] [Add Account]
Provider  Account          Tier           Health         Quota / reset             Priority
OpenAI    Personal API     API Paid       Healthy        $ cap, verified           1
OpenAI    Work API         API Paid       Healthy        Unknown                   2
Gemini    Free Project     Free           Cooldown       Provider API · 14:30       1
Ollama    This Computer    Local          Available      Local runtime             1

Account Pool: [Coding Pool ▾]   Strategy [Cost Aware ▾]   Session sticky [On]
[Configure Allowed Accounts] [Test Route] [View Decision History]
```

**Account details:** provider/account label (not secret), authorization method, tier and entitlements, health, measured latency, quota and provenance, cooldown/reset, priority/weight, whether model/format/terms are supported, connection-specific budgets; `Enable`, `Pause`, `Test`, `Edit`, `Remove`.

**Pool editor:** allowed provider/accounts per client token and workspace; strategy, account weights/priority, sticky policy, failover behavior, paid approval, ceiling, health thresholds. Effective routing explanation and simulation.

**Active sessions:** session ID masked, pinned account/model, last used, cache observation, affinity TTL, manual `Release on next safe turn` control; never silently interrupt a running stream.

### Policies → Rotation & Failover

```text
Account Selection: [Cost Aware] [Fill First] [Round Robin] [Sticky RR] [Weighted]
[✓] Preserve account across conversation
[✓] Respect provider Retry-After and quota reset
[✓] Require approval before paid account/model escalation
Max request retries [1]  Max fallbacks [2]  Session TTL [2h — example]
[Simulate 10 requests] [Save] [Revert]
```

**Simulator outputs:** new conversation vs continuing conversation, selected account, alternative eligibility, price/cache impact, quota and retry reasons. Mock inputs clearly labeled.

### Usage → Account utilization

By provider/account/model/client token/combo: input/output/cache tokens, paid cost vs subscription window vs local operating cost, quota and reset sources, cooldowns, error rate, account changes, actual cost to user, cache hit/miss and incremental savings estimate against reproducible baseline.

## 9. Rust / SQLite extension (proposed)

```text
account_pools(id, name, enabled, strategy, session_sticky, sticky_ttl_seconds,
              max_fallbacks, client_policy_id, created_at, updated_at)
account_pool_members(id, pool_id, provider_account_id, enabled, priority, weight,
                     allowed_model_ids_json, quota_group_id?)
account_health(id, provider_account_id, model_id?, status, reason_code, failure_count,
               retry_after_at?, reset_at?, last_checked_at, source)
account_quota_groups(id, provider_id, provider_tenant_ref?, limit_unit, limit_value,
                     remaining_value?, reset_at?, source, checked_at)
account_affinity(id, client_token_id, conversation_id, model_alias, provider_account_id,
                 model_id, session_state, last_success_at?, expires_at, cache_ref?)
account_reservations(id, pool_id, provider_account_id, client_token_id, request_id,
                     reserved_budget_micro_usd?, reserved_tokens?, released_at?, settled_at?)
rotation_decisions(id, request_id, session_id?, strategy, selected_account_id,
                   eligible_json, excluded_json, policy_snapshot, reason, created_at)
```

Use SQLite migrations and foreign keys; secret material stays exclusively in OS keychain. Concurrency safe across processes and app restarts. Account labels can be exposed to user; credentials never appear in debug logs.

### APIs (typed Tauri commands and optional scoped localhost HTTP)

```text
GET/POST /api/account-pools
GET/PATCH/DELETE /api/account-pools/:id
POST /api/account-pools/:id/simulate
GET /api/accounts/:id/health
POST /api/accounts/:id/recheck
GET /api/sessions/affinity
POST /api/sessions/:id/release-affinity  # on next safe boundary
GET /api/rotation/decisions
GET /api/usage/accounts
```

Client permission boundary applies to every retry/fallback, including direct account pin headers; an API key never grants more accounts than its allowlist.

## 10. Integration order and QA

**P0 (as proposed; the order now follows ROADMAP, where pools are E)** — account registry, model entitlement and quota groups, health states, `fill_first`, `round_robin`, basic safe pre-stream failover, per-client allowlists, ledger with account ID.

**P1** — `sticky_round_robin`, durable affinity, cost-aware candidate scoring, Retry-After/backoff, reservation concurrency, Pools UI, simulation and quota provenance.

**P2** — weighted and least-used strategies, more provider-specific quota adapters, cross-provider affinity benchmarks and advanced utilization analytics.

**Critical tests:**
1. 2 authorized accounts in a provider; concurrent requests select transactionally without data races.
2. Round-robin distributes **new independent sessions**, sticky keeps same account during multi-turn chat.
3. Pin cannot be overridden by a combo, header, fallback or broad client key.
4. 429 respects actual reset; no repeated probe storm; limits shared by `quota_group_id` apply to all members.
5. 401 disables, 403 blocks, 5xx is transient; no account rotation to evade suspensions.
6. No automatic retry after first stream output; failed attempts and partial billable usage logged.
7. Retry to another eligible account occurs only if provider terms and client scope allow it.
8. Workspaces labeled sensitive never route to inappropriate training/data-use account.
9. A model switch with session context must pass memory/format compatibility and preserve provenance.
10. Reservation is released/settled on success, cancel, timeout and crash recovery.
11. Subscription-backed agents use only their permitted official auth; no token extraction/identity cycling.
12. UI shows quota source, cooldown, stable account selection and reasons, without exposing secrets.

## 11. Release criteria

A route is **reliable** when it respects limits and privacy, preserves conversation continuity, reports why an account was chosen, handles transient failures safely and records all actual costs. A higher request success count achieved by defeating quota policy is **not a valid optimization**.

This is a specification, not an implementation report. All strategies require version-specific integration tests before claiming 9Router-equivalent behavior.

## OmniRoute-informed integration (scope decision)

[OmniRoute research](OMNIROUTE_FEATURE_RESEARCH.md) adds verified shared quota-group accounting, dynamic candidate construction, circuit-breaker layering and conservative pre-stream failover to the account pool roadmap. Keep pool selection distinct from model selection. Do **not** extrapolate free quotas across distinct model entries using the same pool, or rotate credentials to bypass shared provider limits. Cost-aware affinity and quota-weighted selection remain subject to empirical eligibility and concurrency tests.
