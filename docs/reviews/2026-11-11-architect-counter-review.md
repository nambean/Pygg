# Independent counter-review — Architect Decisions & P0 Gateway Implementation

**Date:** 2026-11-11  
**Status:** Answered in [the counter-review response](2026-11-11-counter-review-response.md); **not an accepted ADR** and **not a code fix**.  
**Repository baseline:** `main` after commits `c216b1e` (gateway P0-1…P0-4) and `08105f9` (gateway UI); source paths below refer to the GitHub main branch as inspected during this review.  
**Primary reviewed document:** [2026-11-11-architect-decisions.md](2026-11-11-architect-decisions.md). Also reviewed [architecture-review](2026-11-11-architecture-review.md), [review-response](2026-11-11-review-response.md), [ROADMAP](../ROADMAP.md), [IMPLEMENTATION-STATUS](../IMPLEMENTATION-STATUS.md), `gateway/policy.rs`, `gateway/mod.rs`, `gateway/anthropic.rs` and `gateway/store.rs`.

## Executive decision

**Accept the engineering direction of P0-1…P0-4**, including native Anthropic passthrough, explicit session IDs, atomic preflight budget reservations, conservative transport-error handling, quota grouping and detached request accounting. These are meaningful changes from the earlier gateway. However **"P0-1…P0-4 built" is not the same as "production-ready", "hard budget guaranteed", or "cache parity demonstrated"**. The parity test remains open. P0-5 should not hide unresolved safety contracts.

This response distinguishes **demonstrated source behavior** from **possible production risk** and from **provider-dependent facts that must be tested**. No paid provider requests or Rust tests were executed by the reviewer. Earlier illustrative cost multipliers are explicitly **not** measurements.

## Findings and decisions requested

### CR-01 · HIGH · Generic HTTP status codes cannot determine the quota/auth failure scope

**Source:** [architect decisions §Decisions #1](2026-11-11-architect-decisions.md); [policy.rs](../../apps/desktop/src-tauri/src/gateway/policy.rs) `decide`: 401 → `OtherGroups`, 402 → model-independent connection mark, 403/429 → model-scoped mark with `OtherGroups`. [mod.rs](../../apps/desktop/src-tauri/src/gateway/mod.rs) `forward` passes only HTTP status and retry-after to `policy::decide`.

**Observed:** An explicit HTTP 403 marks only the model and allows trying **another model on the same connection**. HTTP 402 marks connection and allows a different quota group. HTTP 429 marks one model, regardless of whether the actual limit is account, organization, project or model scoped. HTTP 401 allows a different quota group, even though the credential was invalid. These choices might be useful for some providers, but the HTTP code alone does not justify them.

**Risk:** Account-level authorization or suspension could be mistaken for model-level; request data may be routed into a different provider/organization when the user expected an explicit policy gate. Unrelated organizations may be independent, but terms and data-use consent still constrain fallbacks.

**Recommendation:** Define a normalized `ProviderError { http_status, provider_code?, retry_after?, scope: model|account|project|org|provider|unknown, retry_safety, source }`. Parse error codes with **provider-specific, versioned adapters**. For unresolved 401/402/403: **fail closed** until an approved, privacy-compatible fallback exists; unresolved 429: conservatively cool the narrowest *verified* quota group and stop account cycling. Do not automatically block all models on a provider just because they share its brand. Add a policy matrix that explains which fallback is permitted.

**Test gate:** 403 account suspended vs model not entitled; 402 budget exhausted at account scope; 429 model-specific vs project-wide; verify no cross-tenant/key fallback without entitlement, consent and quota scope.

### CR-02 · HIGH · "Own organization" is a user assertion, not verified independent quota capacity

**Source:** [architect decisions #1](2026-11-11-architect-decisions.md); `store.rs` `Provider.quota_group` and `separate_quota`; desktop Connections "Own organization" switch.

**Observed:** Default grouping by provider kind is conservative. Marking an account as a separate organization permits bypassing the default cross-connection skip rule.

**Risk:** The UI invites a non-technical user to interpret "separate credentials" as "separate rate limits", while billing entitlements may be shared at project/organization/tenant level. Distinct keys and accounts are not proof of independent quota, or proof that circumventing limits is permitted.

**Recommendation:** Represent separate fields `account_identity`, `organization_or_project_id`, `quota_group_id`, `quota_scope`, `confidence`, `source`, `checked_at`. Keep manual override labeled **"Independent provider quota confirmed"** with a warning and explicit user acknowledgement. The provider rules remain hard constraints regardless of override; do not use an arbitrary client switch to bypass an upstream shared cap. Make the UI show *why* a fallback is permitted.

**Test gate:** multiple models sharing one group, multiple projects under same organization, same provider across two permitted organizations; grouped vs explicitly independent behavior.

### CR-03 · HIGH · Reservation admission is not a guaranteed hard spending cap

**Source:** [architect decisions #3–4](2026-11-11-architect-decisions.md); [mod.rs](../../apps/desktop/src-tauri/src/gateway/mod.rs) `estimate_micro` uses `body.to_string().len() / 4` for input and `max_tokens` or 4096 as output fallback; [store.rs](../../apps/desktop/src-tauri/src/gateway/store.rs) `reserve` is transactional.

**Observed:** Atomic admission is implemented. But the input estimate is heuristic, cached read/write pricing can differ from normal input, a provider may apply a different tokenizer or output semantics, and upstream final usage may be absent. The comment "Pessimistic on purpose" is **not mathematically guaranteed** by dividing serialized bytes by four, especially on non-English text, code/JSON and multimodal content. A 4096-token default may under-reserve when provider/client output is uncapped or differently capped.

**Risk:** When the UI describes a strict limit, actual provider charges could exceed reservation even if concurrent admission is serialized correctly. Unknown-cost allowed attempts also cannot be bounded in dollars.

**Recommendation:** Rename behavior **estimated admission budget**, not "hard guaranteed cap". For a **strict budget** option, require provider/model-specific token bound and maximum generation length, supported cache pricing, verified unit prices, conservative safety margin and fail closed when any bound is unknown. Otherwise label `soft estimate` and warn users that final provider bills can exceed it. Include Anthropic cache-write/read and subscription-window units in the estimate separately. Never infer zero money cost from an unverified Free tier.

**Test gate:** Vietnamese/CJK/code/multimodal messages; 4k+ output; stale price; cache-create/read; unknown price allowed; concurrent requests whose **actual** settled cost exceeds reservation. Check displayed promises and overrun alerts.

### CR-04 · HIGH · Retry-After HTTP-date ignored; 6-hour cap may conflict with provider reset

**Source:** [policy.rs](../../apps/desktop/src-tauri/src/gateway/policy.rs) `retry_after` parses numeric seconds and `retry-after-ms` but not HTTP-date; `COOLDOWN_CAP` clips to six hours.

**Observed:** The implementation explicitly ignores HTTP-date `Retry-After`. A provider-supplied longer reset is replaced with a six-hour wait.

**Risk:** A retry at six hours may be premature when a verified longer limit is in effect. Numeric parsing alone misses a standards-compliant HTTP-date.

**Recommendation:** Implement RFC-compliant HTTP-date decoding, tolerate clock skew and persist verified reset-at. Distinguish **next permissible request time** from an optional **next health-check time**. A periodic probe is not license to send charged inference traffic before the reset. Do not truncate a verified reset for request eligibility.

**Test gate:** integer seconds, HTTP-date, `retry-after-ms`, malformed value, clock skew, 24-hour reset, daylight-saving transitions and process restart.

### CR-05 · MEDIUM–HIGH · HTTP 408/409/425 and 5xx fallback needs an idempotency/risk contract

**Source:** [policy.rs](../../apps/desktop/src-tauri/src/gateway/policy.rs): `408|409|425 => Next::Any`, `500..=599 => Next::Any`; [mod.rs](../../apps/desktop/src-tauri/src/gateway/mod.rs) `forward` may retry across compatible targets.

**Observed:** Ambiguous **transport** timeouts correctly stop. An explicit HTTP response is assumed safe enough to retry. But a server can emit an error *after* partially processing a task and charging, and some external tools or side effects are not replay-safe.

**Risk:** Duplicate API cost and divergent agent/tool results, especially for non-idempotent requests. HTTP 409 (conflict) is often application-state-specific; 425 is not proof an LLM request is idempotent.

**Recommendation:** Default stop on ambiguous 408/409/425 unless provider-specific semantics explicitly prove retry safe. Treat 5xx retry as **bounded paid risk**, not guaranteed free; require request-type and client-policy eligibility, preserve idempotency keys where supported, and charge every attempted request when usage is known. Do not splice response streams.

**Test gate:** 408 after server accepted work; 409 tool-related conflict; 425; 503 with and without provider-reported usage; billing across fallback.

### CR-06 · MEDIUM · Ledger settlement failure is swallowed; durable reservation/attempt reconciliation needs testing

**Source:** [mod.rs](../../apps/desktop/src-tauri/src/gateway/mod.rs) `record_success`: `let _ = shared.with_db(... store::record_usage; store::pin ...)`, then `drop(forwarded)` releases reservation via `Drop`. Failure logging similarly ignores DB errors.

**Observed:** The budget reservation transaction is atomic. The **recording of actual spend and releasing the reservation are separate operations**, and a database failure on `record_usage` is ignored while dropping the reservation can remove the held amount.

**Risk:** Provider may have billed the request, but the local usage ledger can lose the event; future requests may be admitted under the budget. Crashes or forced shutdown between ledger write and reservation release also require reconciliation semantics.

**Recommendation:** Record attempt state before upstream call. Finish with **single atomic settle transaction** (append usage + update reservation state), never discard a billed event simply because its DB insert failed; retry durable reconciliation with bounded recovery and clear status `accounting_degraded`. If accounting fails, fail closed on further paid admissions until repaired. Instrument DB errors without writing credentials.

**Test gate:** SQLite busy/locked/disk-full when settling; crash after upstream response; restart; simultaneous CLI and desktop writers; ensure no hidden spend.

### CR-07 · MEDIUM · Explicit session key and client/token scope still need operational abuse tests

**Source:** `gateway/mod.rs` `session_key`, `gateway/store.rs` `pinned_target`, `gateway/anthropic.rs` forwarded surface; [architect decisions](2026-11-11-architect-decisions.md) accepts explicit header and zero hashing.

**Agreement:** Explicit IDs instead of heuristic prompt hashes are correct. **Remaining contract:** validate user-supplied IDs for bounded length/character set and tenant scope; ensure an API client cannot pin a route to a provider/model/account outside its allowed capability, privacy and budget rules. Affinity keys should include route format or reject mismatches and expire on policy/account change. A pinned session cannot guarantee user-visible context continuity for external agent CLIs: the client still owns that context.

**Test gate:** identical ID across different clients/workspaces, huge or non-ASCII ID, disabled account, alias modified, mode changed, concurrent requests to the same session, revoked tokens, sensitive workspace.

### CR-08 · MEDIUM · "Native passthrough" promises must be measured by field-level parity

**Source:** [architect decisions #5–6](2026-11-11-architect-decisions.md) and [mod.rs](../../apps/desktop/src-tauri/src/gateway/mod.rs) `upstream_request` (native request retains JSON body with rewritten `model`; forwards Anthropic version/beta, not necessarily every custom header); `anthropic.rs` native response forwards body and content-type.

**Observed:** Native path preserves more content than the previous translated one. It is not an opaque byte-for-byte HTTP tunnel: JSON is parsed and serialized again, `model` rewritten, and a subset of headers forwarded. That is probably sufficient for supported Messages fields but must be described accurately.

**Recommendation:** Say **native-format semantic passthrough**. Maintain a matrix for fields and headers such as `cache_control`, `thinking`, `anthropic-beta`, supported citations, token counting, usage metadata, multipart/attachments, and error payloads. Preserve or explicitly reject unsupported features. Don't claim cache parity from a fake echo server alone.

**Test gate:** same Anthropic endpoint direct vs proxy, two-turn cache read/write and 1-hour/5-minute cases where supported; streaming, tool calls, thinking, citation; compare billed cost and loss of fields.

### CR-09 · LOW–MEDIUM · Documentation and UI should not imply all P0 is production-ready

**Source:** [ROADMAP](../ROADMAP.md) marks P0-1…P0-4 built; [IMPLEMENTATION-STATUS](../IMPLEMENTATION-STATUS.md) lists fake and some Gemini live tests; architect decisions explicitly says native Anthropic parity, terms, tray and real CLI not yet checked.

**Recommendation:** Keep **Built / Fake-tested / Live-tested / Provider-parity-tested / Release-ready** as separate states. P0-5 may begin in parallel with benchmark harness design, but publishing ROI claims is prohibited until P0 cost-parity and quality gates pass. Keep the global ROADMAP as the source of priorities, not duplicate P0 tracks.

## Accepted decisions — no need to reopen without evidence

1. Gateway first (ADR-016), not orchestrator-first implementation sequence.
2. The product north star is general-purpose AI Cost Optimizer; API-heavy developer workflows are the initial measurable market wedge, not a permanent narrowing of scope.
3. External CLI/IDE conversations are opaque: no silent Shared Memory injection or historical prompt rewriting.
4. No prompt hash for fake session identity; only explicit `x-smarttaskrouter-session` where client supports it.
5. No automatic replay on an ambiguous transport timeout.
6. No buffered stream-start fallback in MVP; changing the boundary requires a separate benchmark and semantics test.
7. Provider-specific tool/signature memory remains an acknowledged limitation until safely scoped persistence is justified.
8. Do not rename `gateway serve` solely for the proposed npm launcher; `str start` stays a future distribution contract.
9. No re-use of upstream open-source code/data without license and attribution review.

## Proposed acceptance order for Claude

**First (cost and safety):** CR-01 and CR-02 classify error/entitlement scopes → CR-03 guarantees or truthfully labels budgets → CR-06 makes accounting durable. CR-04 and CR-05 then harden cooldown and retry semantics. **Next (compatibility):** CR-07 scopes explicit sessions → CR-08 validates native semantics and cost parity. **Finally:** CR-09 adjusts status labels and P0 release checklist.

Before changes, Claude should add a **review response table** with `Accept / Partial / Reject / Defer`, code references, planned tests and any deviation from this proposal. A **rejected** finding must include a concrete counterexample, not just a preference for simpler code.

## Questions Claude must answer in the next review

1. For 402/403/429, what exact provider error fields prove account-level versus model-level scope, and what is the fallback policy when unknown?
2. Can user-confirmed independent quota groups still violate a shared upstream user/org limit, and how do we prevent it?
3. Which input/output/cache token bounds make the monthly cap enforceable for **every** supported provider; if not enforceable, how does UI state the limitation?
4. What happens if SQLite commits the usage event but fails to delete a reservation, or fails before writing the usage event?
5. Does native Anthropic path preserve all required semantic headers (beta/version, tool options) and produce real cache read/write parity? Show direct vs proxy measurements, not synthetic estimates.
6. Does Claude Code really emit the configured explicit session header consistently over long agent sessions and tool requests? What is the verification matrix?
7. What is the tested behavior on provider HTTP-date Retry-After and 24-hour+ resets?
8. Is the P0-5 Auto router hard-filtering on verified capability, budget and privacy data, or is it simply a static cheapest-first alias with a new name?

## Proof/limitations

This note is based on GitHub source inspection and the reported test status at the cited commits. **I did not run Rust tests or external provider paid requests**. All proposed vulnerabilities are engineering risk statements or observed contract gaps; none imply a demonstrated exploit or actual loss of user funds. All direct-provider cost multipliers from earlier discussion remain illustrative, not verified outcomes.
