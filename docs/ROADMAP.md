# SmartTaskRouter — Authoritative Product & Delivery Roadmap

> v1.0 · 2026-11-11 · This file is the **single source of truth for global implementation priority**, superseding phase numbering in design/RFC documents. Always consult [Implementation Status](IMPLEMENTATION-STATUS.md) for what actually runs. Nothing marked Planned here is a code-delivery claim.
>
> **North star:** AI Cost Optimizer for everyday AI users *and* developers: automatically choose an eligible provider/model/account, preserve user experience and context, and minimize **quality-adjusted total cost per successful answer/task**, not nominal token price. **Initial measured market wedge:** API-heavy developers using multiple authorized providers/agent CLIs, because pay-per-use ROI and task verification can be benchmarked. Consumer Auto Chat remains part of the product goal; its spend-saving value must be validated by audience.

## Current built foundation (as reported by Implementation Status, not independently rerun here)

A local Rust gateway at `127.0.0.1:8741`, OpenAI Chat Completions and Anthropic Messages *translation* surfaces, a few provider presets, static combos with fallback, client tokens, a cost-provenance ledger, a headless `gateway serve` binary and desktop Overview/Connections/Combos/Clients/Usage screens exist. Live verification: Gemini (routing, fallback, ledger), Anthropic (native passthrough, with cache parity measured direct versus gateway on claude-haiku-4-5) and a headless Claude Code session (session header on every request). No claim of parity for other models, the 1-hour cache TTL, OpenRouter's cache, or an interactive Claude Code session. Client tokens can be restricted to named connections; a server error does not move to another model unless the client allows it. Auto route does **not** yet exist. [Implementation Status](IMPLEMENTATION-STATUS.md) is authoritative, including the verification level of each P0 item.

> **Independent counter-review (2026-11-11):** [Architect counter-review](reviews/2026-11-11-architect-counter-review.md) challenges error/quota scope, budget guarantee, accounting durability, Retry-After HTTP-date, HTTP 408/409/425 retry, session isolation and native Anthropic parity. **Answered item by item** in the [counter-review response](reviews/2026-11-11-counter-review-response.md): six accepted, three in part, with the code and tests that implement them. Since then P0-1 has measured cache parity on one Anthropic model, and P0-4 a headless Claude Code run; see the status table below.

> **Follow-up review (2026-11-11):** [Remaining Gateway Risks](reviews/2026-11-11-remaining-gateway-risks.md) requests Claude's response on per-client privacy-safe fallback, paid retry exposure after 5xx, SQLite fault-injection/accounting-degraded behavior, and documentation updates following real Anthropic/Claude Code tests. **Answered** in the [follow-up response](reviews/2026-11-11-remaining-gateway-risks-response.md): all four accepted. Client tokens can be restricted to named connections, paid retries after a server error are off by default, and no paid request is admitted while a ledger entry waits to be written. Provider data-use terms (GW-10) and provider-side usage reconciliation are deferred. Check [Implementation Status](IMPLEMENTATION-STATUS.md) for the latest parity evidence.

> **Post-`d518438` engineering review (2026-11-11):** [Cross-process accounting and Free/Local replay safety](reviews/2026-11-11-post-d518438-counter-review.md) challenges process-local degraded-ledger admission and the assumption that Free/Local 5xx requests are automatically replay-safe. Requests a documented singleton-or-multi-process contract and tests. **Review only, not an implemented fix.**

## P0: cost-safe gateway parity — five items, in dependency order

| Order | Deliverable | Why mandatory | Acceptance gate |
| --- | --- | --- | --- |
| **P0-1** | **Same-format passthrough + cache preservation** | Converting Anthropic to OpenAI currently drops cache markers/thinking, making proxy potentially more expensive | Compare direct vs gateway on same official native endpoint, cache read/write/input, tool/citation/thinking compatibility; no undisclosed dropped fields |
| **P0-2** | **Safe fallback and circuit breaker** | 401/403/429/ambiguous read timeout should not cause unbounded or policy-violating account/model retries | Error-specific routing, observed Retry-After/reset, shared quota scopes, pre-stream idempotency, no replay after ambiguous timeout/partial output |
| **P0-3** | **Atomic budget + complete usage ledger** | Concurrent requests currently bypass simple preflight cap; cancellation and Unknown cost can hide billed calls | Transactional reservation/settlement, per-client concurrency control, policy for Unknown price, disconnect/timeout tests at all request stages |
| **P0-4** | **Explicit session affinity** | Static combos reset route per request and lose cache/agent continuity | Scoped session ID, pin eligible provider/model/account for active session; fallback only at safe boundary; clients without session IDs never silently guessed |
| **P0-5** | **Explainable rule-based Auto routing** | Existing combos are static and not yet intelligent | Verified model catalog/provenance, hard capability/privacy/spend filters, dynamic eligible candidate selection, `auto` + `auto/cheap` alias, reason in response/usage, no LLM classifier |

### Status of P0 (2026-11-11)

| Item | State | What is still open |
| --- | --- | --- |
| P0-1 | **Built, provider-parity-tested on Anthropic** ([script](../scripts/anthropic-parity.ps1)). Native-format semantic passthrough (key order kept, provider limit headers passed back); direct and gateway gave identical cache writes and reads (9011 / 9011), streamed and not, and the same thinking, tool and citation blocks (claude-haiku-4-5). Cache-preserving translation for OpenRouter, every dropped field reported | 1-hour cache TTL, multi-turn tool round trip, other models; OpenRouter cache behaviour measured |
| P0-2 | **Built, fake-tested.** Error-scoped fallback, quota-group hold across requests, provider error codes, cooldowns from `Retry-After` (seconds or HTTP date) or Gemini `RetryInfo`, no replay after an ambiguous timeout or 408/504/524, **no retry after a 5xx unless the client opts in**, **per-client connection allowlist** | Fixtures per real provider; provider data-use terms (GW-10); GW-7 (fallback before the first token rather than before headers) held, as the response advised |
| P0-3 | **Built, fake-tested.** Attempt journal before sending, atomic exactly-once settlement, **no paid admission while a settlement waits**, estimated admission holds, spend at risk counted, per-client in-flight cap, unknown-price policy, detached handlers | The limit is an estimate, not a hard cap (stated in the UI); strict mode after the P0-5 catalog; no provider-side usage reconciliation |
| P0-4 | **Built, live-tested with headless Claude Code.** Explicit `x-smarttaskrouter-session` only, scoped per client, expired pins deleted; snippets set it. Header on 12 of 12 requests, sub-agent and `/compact` included | Interactive terminal not run; `count_tokens` not observed |
| P0-5 | Not started | Next |

Details and verification: [Implementation Status](IMPLEMENTATION-STATUS.md). Decisions taken while building: [architect decisions](reviews/2026-11-11-architect-decisions.md).

### P0 release proof: benchmark, not a sixth feature

- **Parity baseline:** exact same model/provider direct vs proxy with at least two turns that exercise prompt caching, tools and streaming; cache savings and total billed amount cannot regress materially without explicit explanation.
- **Optimization baseline:** pinned premium vs pinned inexpensive capable vs static combo vs rule-based Auto, with success/quality checks and standardized prompts.
- **Accounting:** include cache write/read, retries, request cancellation, classifier (if any), extraction, time spent verifying/reworking, unknown costs and client overhead.
- **Honesty gate:** do **not** promise percent cost savings or automatic quality retention without a reproducible benchmark and confidence bounds.
- **Security gate:** no unauthorized account/key switches, no denied provider terms/data-use, no budget overrun via concurrency.

## P1: continuity + measured optimization (after P0 gate)

1. **Developer integrations:** verify Claude Code, OpenCode, Aider, Codex and IDE targets one by one with test fixtures; maintain exact version/capability matrix. Package a headless CLI installer/npm launcher after supported OS builds, without cloning the Rust gateway in Node. Current CLI binary is `gateway serve`; `str start` is a proposed future interface.
2. **Owned Auto Chat:** app-controlled conversation store, versioned summary, user-scoped memory facts and transparent context handoff. **External client-managed chat is opaque** and must not have context injected or reconstructed without explicit supported opt-in.
3. **Cache-aware route scoring:** account/model affinity, measured cache read/write, cost to rebuild and quality consequences. Prefer staying when switching adds cost.
4. **Token reduction:** only deterministic, reversible dedup/noisy new tool-output filtering in **shadow and opt-in** first; verify prefix hash/cache parity before enabling defaults. Never silently rewrite external agent history.
5. **Benchmarks and quality feedback:** measured cost per accepted result, failures, user revisions and manual time estimates. LLM classifiers only with a strict input/cost budget and only at explicit classification boundaries.

## P2: only after data demonstrates incremental benefit

- Selective batched long-term fact extraction, SQLite FTS5 and optional vector search; inspect/edit/export/delete/disable memory and apply provider/workspace privacy boundaries.
- Advanced quota/weighted multi-account strategies and entitlements only via official allowed interfaces.
- Semantic caching, advanced compaction and temporal knowledge graph if retrieval/quality tests beat a simpler baseline.
- Curated Skills/Prompts recommendation, optional read-only MCP diagnostics; no unrestricted security-sensitive policy mutations.
- Signing/notarization and broader platform distribution after release contracts.

## Product rules that override outdated speculative docs

- **Billing tier ≠ auth method ≠ capability tier.** Free/Subscription/API Paid/Local/Unknown are per account-model entitlements with provenance, not guaranteed provider-wide facts.
- **Do not automatically switch model within active agent attempt, tool call or partial stream.** Pinned external clients retain prompt ownership and cache semantics.
- **Prefer native passthrough over conversion.** Cross-format routes must pass streaming/tools/images/cache/citation contract tests and reveal feature loss.
- **No account cycling to evade bans, rate limits or provider terms.** Shared quota groups are hard constraints.
- **Do not count free subscription-window tokens as USD savings**; distinguish money, tokens, local operating costs and quality/human rework.
- **Cache-stable conversation prefix** is preserved within pinned epochs; compaction and handoff explicitly account for cache reset overhead.
- **Metrics should never manufacture precision:** unknown price/usage = Unknown; savings require valid baseline.

## Docs and authority order

1. **What is built and verified?** [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md).
2. **What is next and what is the gate?** This [ROADMAP.md](ROADMAP.md).
3. **Why did we decide it?** [DECISIONS.md](DECISIONS.md) (proposed ADRs remain proposals).
4. **How will a feature work?** Existing topic specifications [Universal Memory](UNIVERSAL_SHARED_MEMORY.md), [Account Rotation](ACCOUNT_API_ROTATION.md), [CLI/IDE/npm/Tiers](CLI_IDE_NPM_PROVIDER_TIERS.md), [OmniRoute feature shortlist](OMNIROUTE_FEATURE_RESEARCH.md).
5. **Which objections remain?** [Claude review](reviews/2026-11-11-architecture-review.md) and [review reconciliation](reviews/2026-11-11-review-response.md).

## Ownership decisions still open

- Choose project license before importing upstream *code or datasets*; citations/links to research are not import approval.
- Choose explicit initial model/client benchmark matrix and paid test budget; do not send paid traffic without owner approval.
- Decide whether to expose Auto Chat in the initial commercial launch (product vision stays broad, engineering wedge measurable first).
