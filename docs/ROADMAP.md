# SmartTaskRouter — Authoritative Product & Delivery Roadmap

> v1.0 · 2026-11-11 · This file is the **single source of truth for global implementation priority**, superseding phase numbering in design/RFC documents. Always consult [Implementation Status](IMPLEMENTATION-STATUS.md) for what actually runs. Nothing marked Planned here is a code-delivery claim.
>
> **North star:** AI Cost Optimizer for everyday AI users *and* developers: automatically choose an eligible provider/model/account, preserve user experience and context, and minimize **quality-adjusted total cost per successful answer/task**, not nominal token price. **Initial measured market wedge:** API-heavy developers using multiple authorized providers/agent CLIs, because pay-per-use ROI and task verification can be benchmarked. Everyday users are reached through the clients they already use, pointed at the gateway; the app hosts no chat of its own (ADR-028).

## Current built foundation (as reported by Implementation Status, not independently rerun here)

A local Rust gateway at `127.0.0.1:8741` with OpenAI Chat Completions and Anthropic Messages surfaces (native passthrough to Anthropic, translation elsewhere), 26 provider presets and 6 local runtimes plus custom OpenAI- and Anthropic-compatible endpoints for providers outside the list, combos with error-scoped fallback, client tokens with limits, a cost-provenance ledger, `auto` routing, a headless `gateway serve`, and desktop Overview/Connections/Combos/Clients/Usage screens. On top of it: official agent CLIs run with their own sign-in (ADR-026), task engine v1 with checks and a cascade across agent steps, and tasks started from IDE agents through MCP (ADR-027). Live verification: Gemini (routing, fallback, ledger), Anthropic (native passthrough, with cache parity measured direct versus gateway on claude-haiku-4-5) and a headless Claude Code session (session header on every request). No claim of parity for other models, the 1-hour cache TTL, OpenRouter's cache, or an interactive Claude Code session. Client tokens can be restricted to named connections; a server error does not move to another model unless the client allows it; only one gateway serves a store at a time. Rule-based `auto` and `auto/cheap` exist and were tried live on Gemini, Anthropic and Claude Code; they make no savings claim until the optimization benchmark has run. [Implementation Status](IMPLEMENTATION-STATUS.md) is authoritative, including the verification level of each P0 item.

> **Independent counter-review (2026-11-11):** [Architect counter-review](reviews/2026-11-11-architect-counter-review.md) challenges error/quota scope, budget guarantee, accounting durability, Retry-After HTTP-date, HTTP 408/409/425 retry, session isolation and native Anthropic parity. **Answered item by item** in the [counter-review response](reviews/2026-11-11-counter-review-response.md): six accepted, three in part, with the code and tests that implement them. Since then P0-1 has measured cache parity on one Anthropic model, and P0-4 a headless Claude Code run; see the status table below.

> **Follow-up review (2026-11-11):** [Remaining Gateway Risks](reviews/2026-11-11-remaining-gateway-risks.md) requests Claude's response on per-client privacy-safe fallback, paid retry exposure after 5xx, SQLite fault-injection/accounting-degraded behavior, and documentation updates following real Anthropic/Claude Code tests. **Answered** in the [follow-up response](reviews/2026-11-11-remaining-gateway-risks-response.md): all four accepted. Client tokens can be restricted to named connections, paid retries after a server error are off by default, and no paid request is admitted while a ledger entry waits to be written. Provider data-use terms (GW-10) and provider-side usage reconciliation are deferred. Check [Implementation Status](IMPLEMENTATION-STATUS.md) for the latest parity evidence.

> **Post-`d518438` engineering review (2026-11-11):** [Cross-process accounting and Free/Local replay safety](reviews/2026-11-11-post-d518438-counter-review.md) challenges process-local degraded-ledger admission and the assumption that Free/Local 5xx requests are automatically replay-safe. Requests a documented singleton-or-multi-process contract and tests. **Answered** in the [post-d518438 response](reviews/2026-11-11-post-d518438-response.md): one gateway per store, enforced by an OS file lock and tested with two real processes; a 5xx stops on every billing type unless the client opts in.

> **Gateway lifecycle follow-up (2026-11-11):** [Stop/start ownership and journal-recovery counter-review](reviews/2026-11-11-gateway-lifecycle-counter-review.md) raises four review findings: per-gateway ownership epochs, detached/streaming work outliving the file lock, ignored startup recovery errors and missing same-process restart tests. **Answered** in the [lifecycle response](reviews/2026-11-11-gateway-lifecycle-response.md): all four accepted. The store's ownership is held by the server, every attempt in flight and every waiting settlement, and released only when the last one is settled. Ownership generations replace the process timestamp. A failed start-up recovery fails the start. Five lifecycle tests cover the review's matrix.

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
| P0-3 | **Built, fake-tested.** Attempt journal before sending, atomic exactly-once settlement, **no paid admission while a settlement waits**, **one gateway per store** (OS lock, two-process test), **ownership held until every attempt of a stopped gateway is settled** (generations, lifecycle tests), estimated admission holds, spend at risk counted, per-client in-flight cap, unknown-price policy, detached handlers | The limit is an estimate, not a hard cap (stated in the UI); strict mode after the P0-5 catalog; no provider-side usage reconciliation |
| P0-4 | **Built, live-tested with headless Claude Code.** Explicit `x-smarttaskrouter-session` only, scoped per client, expired pins deleted; snippets set it. Header on 12 of 12 requests, sub-agent and `/compact` included | Interactive terminal not run; `count_tokens` not observed |
| P0-5 | **Built, live-tested** (Gemini, Anthropic, Claude Code; see Implementation Status). `auto` (lowest eligible tier at or above the client's minimum, then local, free, subscription, paid, then cost) and `auto/cheap` (lowest estimated cost). Hard filters first: allowlist, API format, health, quota group, known tool, image, context and output support, statable cost. Unknown counts as missing. No classifier. Model facts carry a source (provider list or user). The reason is in `x-smarttaskrouter-auto` and the ledger, sessions stay pinned, and a simulator sits in the Combos screen | Request features as filters (adaptive thinking, effort, from Anthropic's model list: Claude Code sends them to any model it does not know, and older models refuse them); a curated, sourced tier table (tiers are the user's for now); data-use terms (GW-10) as a filter; the optimization benchmark (pinned premium against static combo against Auto) before any savings claim |

Details and verification: [Implementation Status](IMPLEMENTATION-STATUS.md). Decisions taken while building: [architect decisions](reviews/2026-11-11-architect-decisions.md).

### P0 release proof: benchmark, not a sixth feature

- **Parity baseline:** exact same model/provider direct vs proxy with at least two turns that exercise prompt caching, tools and streaming; cache savings and total billed amount cannot regress materially without explicit explanation.
- **Optimization baseline:** pinned premium vs pinned inexpensive capable vs static combo vs rule-based Auto, with success/quality checks and standardized prompts.
- **Accounting:** include cache write/read, retries, request cancellation, classifier (if any), extraction, time spent verifying/reworking, unknown costs and client overhead.
- **Honesty gate:** do **not** promise percent cost savings or automatic quality retention without a reproducible benchmark and confidence bounds.
- **Security gate:** no unauthorized account/key switches, no denied provider terms/data-use, no budget overrun via concurrency.

## P1: continuity + measured optimization (after P0 gate)

1. **Agent CLIs as executors (started 2026-11-11, owner decision):** Claude Code, Codex CLI, Gemini CLI and OpenCode run with their own sign-in in a Git worktree per task; Pygg never reads or forwards that sign-in, and never bridges web sessions or other products' OAuth. Built: detection, a run with diff, keep as branch or discard, and ledger entries; task engine v1 with approved checks, a cascade across agent steps, approval before a paid step, stop and recovery after a restart. Owner decision (2026-11-11): tasks are started from where users already work, by their IDE agent through `pygg-mcp` and the gateway's task API; the app runs them and is where they are watched and paid steps approved. Built, and verified from a real Codex session; the app's own program serves MCP (`<app> mcp`), so the installer ships nothing extra. Next: Claude Code. See [AGENTS.md](AGENTS.md#from-the-ide-pygg-mcp-and-the-task-api).
1. **Developer integrations:** verify Claude Code, OpenCode, Aider, Codex and IDE targets one by one with test fixtures; maintain exact version/capability matrix. Package a headless CLI installer/npm launcher after supported OS builds, without cloning the Rust gateway in Node. Current CLI binary is `gateway serve`; `str start` is a proposed future interface.
2. ~~**Owned Auto Chat**~~: withdrawn (ADR-028, no Auto Chat in the app). **External client-managed chat stays opaque**: no context injected or reconstructed.
3. **Cache-aware route scoring:** account/model affinity, measured cache read/write, cost to rebuild and quality consequences. Prefer staying when switching adds cost.
4. **Token reduction:** only deterministic, reversible dedup/noisy new tool-output filtering in **shadow and opt-in** first; verify prefix hash/cache parity before enabling defaults. Never silently rewrite external agent history.
5. **Benchmarks and quality feedback:** measured cost per accepted result, failures, user revisions and manual time estimates. LLM classifiers only with a strict input/cost budget and only at explicit classification boundaries.

## Revised plan after ADR-026 and ADR-027 (2026-11-11 docs review)

A review of every planning doc against the build ([DECISIONS](DECISIONS.md), the [functional plan](SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md), [SCREENS](design/SCREENS.md), [Account Rotation](ACCOUNT_API_ROTATION.md), [CLI/IDE/npm/Tiers](CLI_IDE_NPM_PROVIDER_TIERS.md), [Universal Memory](UNIVERSAL_SHARED_MEMORY.md), the [OmniRoute research](OMNIROUTE_FEATURE_RESEARCH.md), the [benchmark README](../benchmark/README.md), the root README and every file in [reviews](reviews)). In order:

**A. Make the IDE flow usable every day**
1. **Built 2026-11-11:** tray and notifications for a paid step waiting, a task ready, needing review or not finished (SCREENS §2.5). Per ADR-010, no task names or paths, and no approve button, in a notification. Closing the window keeps the gateway and tasks running from the tray (review GW-9); both are switches in Settings → Background. Not yet: a click on a notification opening the task.
2. **Built 2026-11-11:** approvals as ADR-010 specifies (SCREENS §6.3). The estimate is the range of the agent's own list-price estimates over its last 10 runs of that model; approving needs a cap; Claude Code stops at it itself (`--max-budget-usd`), and for other agents a reported cost over the cap stops the ladder. An approval is invalidated and asked again when its estimate changed before the user decided. A budget change (A3) also invalidates it.
3. **Built 2026-11-11:** task budgets per day, overall and per workspace (functional plan §2.4; schema v11), for paid steps only. A finished paid step counts at the agent's own estimate, or at its cap when it reported none; a running one holds its cap. A used-up budget blocks a paid step before it asks; a cap cannot pass what is left; a budget change invalidates a waiting approval. Days are UTC. Set in Settings → Task budgets; the toolbar shows today's figures.
4. **Built 2026-11-11:** the Workspaces screen (SCREENS §5.3): each workspace's checks to approve (a sidebar badge counts workspaces whose suggested checks wait, as IDE-created ones do), its privacy class, its daily budget, and removal while it has no tasks; Add Workspace with the system's folder picker. In a **sensitive** workspace every step asks before an agent sees the code, free ones included, until data-use terms (B2) can say where agents send it. The folder picker has not been clicked through in the native app.
5. **Built 2026-11-11:** Export Patch next to Keep as Branch (ADR-009): the result saved through the system's save dialog as a patch `git apply` takes (binary files included, a short header naming the task and its base commit); the result stays open to keep or discard.
6. **Done 2026-11-11:** Claude Code through MCP, verified like Codex (AGENTS.md).

**B. Gateway correctness**
1. `auto` filters by request features (adaptive thinking, effort), from the provider's model list.
2. Data-use terms per provider (GW-10) and the workspace privacy class as hard filters.
3. A curated tier table with sources (P0-5), replacing tiers that only the user sets.
4. Circuit-breaker health probing (Account Rotation §5) and a strict budget mode.
5. Translation gaps: OpenAI → Anthropic (refused today), and Qwen's `reasoning_content`.
6. Smaller deferred review items: Gemini thought signatures kept across a restart (GW-8); a combo-editor warning for two connections of one provider (GW-1); idempotency keys (CR-05). Held until an adapter can fill them: provider-verified organization and quota scope (CR-02), and reconciliation against the provider's own usage (FR-03). GW-7 (fallback before the first token) stays held.

**C. Evidence before any savings claim**
1. Verification debt, live: the parity cases still open (1-hour cache TTL, a multi-turn tool round trip, other Claude models, Anthropic-compatible endpoints other than Anthropic's own); OpenRouter `cache_control` on translated requests; an interactive Claude Code session through the gateway, with `count_tokens`; error fixtures per real provider (401, 403, 429, timeouts, quota groups); OpenRouter, OpenAI, Groq, DeepSeek, Ollama and the OpenCode snippet with its session header.
2. A live cascade where a real failed check escalates to the next agent.
3. The benchmark: 20–40 cases (2 exist) and a report, on the baselines of "P0 release proof" above (pinned premium, pinned inexpensive capable, static combo, Auto) plus the check-driven cascade for tasks. Paid traffic under **$5 in total** (ADR-028), enforced with `bench --budget-usd`; free tiers, local models and subscription agents carry most cases.
4. Quality signals from behaviour (a retry, "use a better model", a cancel) logged per result, and human rework in the cost per success (review RE-3); a one-day TensorZero evaluation before building more of this (review OSS-1).

**D. Distribution**
1. An NSIS installer tried on a clean machine.
2. The integrations hub with config backup and restore (ADR-022), then the `str` CLI and npm launcher (ADR-021).
3. Signing, which needs a certificate the owner pays for.

**E. Later, after data or reconfirmation**
- Account pools, for API-key and token-plan connections only: agents keep their own sign-in, so subscriptions cannot be pooled (ADR-026).
- The Policies page with strategies and a simulator; multi-stage plans.
- Quota tracking for free tiers and subscriptions, where a provider or agent states it (OmniRoute OR-03 §6).
- Compression, deduplication and cache-aware scoring (P1-3, P1-4; OmniRoute OR-06 to OR-08), shadow and opt-in first.
- More `auto` aliases (`auto/fast`, `auto/coding`, `auto/local`, `auto/available`, `auto/quality`; OmniRoute OR-01, OR-20): **not planned** until the benchmark shows `auto` and `auto/cheap` are not enough.

**Housekeeping:** a CONTRIBUTING file with the review's rules (RM-1); one schema reference generated from the migrations (RM-4); demo data replaying recorded snapshots instead of copying the core's rules (OSS-3). Licence decided: MIT (ADR-028); ported code keeps its notice in `THIRD_PARTY_NOTICES.md` (OSS-2).

**Superseded by ADR-027:** the New Task → Analyze → Plan → Run flow as the main path, `POST /api/routes/analyze`, the tasks API in the functional plan §6 (the built one is `/pygg/v1/tasks`), "Needs Revalidation", and first-run screens that end in analyzing a task. **Superseded by ADR-026:** "Supported OAuth" connections and rotation of subscription sessions.

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

## Ownership decisions

Decided 2026-11-11 ([ADR-028](DECISIONS.md)): **MIT licence**; a paid benchmark budget **under $5 in total**; **no Auto Chat in the app**.

Still open: the benchmark's model and client matrix, within that budget. Paid traffic beyond it needs the owner's approval.
