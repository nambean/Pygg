# Decisions

> **Status:** Current · architecture decision records (ADR-001 to ADR-034) · reviewed 2026-11-11 · order: [ROADMAP](ROADMAP.md) · built: [Implementation Status](IMPLEMENTATION-STATUS.md)

Architecture decision records, in the style of DownFetch's `docs/DECISIONS.md`. Each decision says what was chosen, why, and what it costs. The product plan is [SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md](SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md); the screen design is [design/SCREENS.md](design/SCREENS.md).

## ADR-001 — Windows first, macOS next

**Decision:** Build and test on Windows first. macOS must build, but it is not release-tested until Phase 4. Linux is best effort.

**Why:** The team develops on Windows, and DownFetch follows the same order.

## ADR-002 — Local first

**Decision:** Everything runs on the user's machine. Credentials, the ledger, task history and workspaces never leave it. No account, and no hosted service in the core path.

**Why:** The app holds API keys and source code. Keeping it local keeps the trust boundary small.

## ADR-003 — Orchestrator first, gateway underneath (sequencing superseded by ADR-016)

**Status:** Historical task architecture; **gateway-first delivery** is the current implementation order. Preserve the no-mid-agent-attempt-switch rule; it does not mean an agent orchestrator has shipped.

**Decision:** Routing decisions happen at task and stage boundaries. The local gateway is the transport and enforcement layer, and it never switches models silently within an agent session. Gateway-only mode pins the model per conversation and falls back only on hard errors before any output is streamed.

**Why:** Switching mid-session breaks prompt caches, tool-call formats and reproducibility, and a gateway sees single requests without task context (plan §0.1).

## ADR-004 — A localhost gateway, loopback only, with client tokens

**Decision:** The gateway listens on `127.0.0.1`. Every request needs a scoped client token with its own budget. Binding beyond loopback requires an explicit user setting.

**Why:** SDKs and agent CLIs reach a model only through a base URL, so a localhost HTTP endpoint is unavoidable. This is a deliberate difference from DownFetch (its ADR-003), which avoids a localhost API.

**Cost:** A new local attack surface, which the token and loopback-only rules are meant to contain.

## ADR-005 — One Tauri crate with modules; same stack as DownFetch

**Decision:**
- Tauri 2 with a single Rust crate (`apps/desktop/src-tauri`) containing the core, gateway and agent runner.
- React 19 + Vite + TypeScript with plain CSS tokens; no Tailwind or shadcn.
- pnpm 9 workspace.
- No Node sidecar.

**Why:** DownFetch already ships this stack, so tooling, patterns and lessons carry over. A single process avoids sidecar packaging.

**Cost:** Rust has fewer multi-provider LLM libraries than Node or Python, so provider adapters are written in-house against recorded fixtures.

## ADR-006 — Rule-based routing in v1

**Decision:** v1 uses a curated tier table (T0–T4) plus minimum-tier rules per task category and complexity, with hard eligibility filters. Measured cost-per-success ranking comes later, and only above a minimum sample size. **Update 2026-11-11:** the curated table is built (`gateway/tiers.rs`, `curated:v1`, ROADMAP B3); minimum-tier rules per category and complexity are not.

**Why:** There is no measured data at the start. Inventing quality scores would violate the plan's principles.

**Consequence:** The UI says "lowest eligible tier under your policy", never "cheapest".

## ADR-007 — Deterministic checks gate the cascade

**Decision:**
- A stage may start at a cheaper tier and escalate only if it has a deterministic check (typecheck, tests, build, schema).
- An LLM review can advise but never passes a stage.
- Without a check there is no automatic cascade. The user may review a direct run, and its result is recorded as **Manually Accepted**.

**Why:** A cheap model's output is only safe to accept when a machine can verify it.

## ADR-008 — A Git worktree per attempt; Git-only writes in MVP

**Decision:** Each attempt runs in its own Git worktree created from a recorded base revision. Failed attempts are discarded. Workspaces that are not Git repositories get read-only planning in MVP. **Update 2026-11-11:** not built; workspaces must be Git repositories (AGENTS.md).

**Why:** Isolation is what makes trying a cheap model first harmless. Snapshot isolation for non-Git folders is possible later, but it is not worth the risk in MVP.

## ADR-009 — Keep as Branch is the MVP result handoff

**Decision:** A passing result is committed to a durable branch `str/<slug>-<short-id>` from its base revision, without switching the user's branch. Export Patch and Discard are the alternatives. Apply to Workspace, with drift detection, comes later.

**Why:** Developers already know how to review and merge a branch. It never touches the working tree.

## ADR-010 — Approvals are durable and cover exactly one attempt

**Decision:**
- An approval names one attempt and a spending cap.
- It survives a restart and is invalidated by any policy, catalog or budget change.
- Nothing is ever approved automatically.
- Notifications never include task names or paths, and never contain an approve button.

**Why:** Spending must be a deliberate, specific decision.

## ADR-011 — Billing types and per-fact provenance

**Decision:**
- Billing is `Free`, `Local`, `Subscription`, `Paid` or `Unknown`.
- Every price, quota and capability value carries its own source and time.
- Unknown is never treated as free or shown as `$0.00`.
- Subscription use is measured as quota-window consumption, never as money.

**Why:** Most providers don't report remaining quota, and false precision would mislead the user about spending.

## ADR-012 — Traffic-light window controls on every platform

**Decision:** The app uses DownFetch's custom traffic-light window controls on Windows and Linux too.

**Why:** The team chose consistency across its apps (2026-11-10).

**Cost:** It is a departure from Windows conventions. SCREENS.md D12 lists the checks that must pass before release: snap layouts, scaling, keyboard use and screen readers.

## ADR-013 — Benchmark before building the router (delivery order superseded)

**Status:** This benchmark remains a mandatory evidence gate before advertising savings, but **ADR-016 supersedes the old benchmark-before-gateway sequence**. See [ROADMAP.md](ROADMAP.md).

**Decision:** Phase 0 runs a benchmark of reproducible coding tasks with deterministic checks. It compares three baselines: always-T4, always-T2, and a check-driven cascade. The router and the full UI are built only if the cascade reaches a comparable pass rate at lower cost per success.

**Why:** The product's core idea is unproven. A benchmark is the cheapest way to find out.

**Note:** Running the benchmark against paid models needs an explicit budget from the owner: under $5 in total (ADR-028).

## ADR-014 — Sample data is always labeled

**Decision:** Until the task engine exists, task screens show sample data under a permanent "Sample tasks for design review" banner. Screens that are designed but not built say so, instead of showing empty data.

**Why:** An empty list would look like a real, empty configuration, and unlabeled sample numbers would look like measurements.

**Update 2026-11-11:** the task engine exists, so the desktop app shows real tasks. Sample tasks remain only in the browser preview (`pnpm dev`), under its demo banner.

## ADR-015 — Commit the pnpm lockfile

**Decision:** `pnpm-lock.yaml` is committed. DownFetch ignores its lockfile; this project does not.

**Why:** Reproducible installs matter more than avoiding lockfile churn for a desktop app with a signed release pipeline ahead.

## ADR-016 — Build the gateway first

**Decision:** The multi-provider gateway is built before the task orchestrator (2026-11-11, owner's choice). It works in the spirit of tools like 9router:
- Clients the user already has (Codex, Cursor, OpenCode, scripts) point at one local base URL.
- Each connection is a provider account (OpenAI-compatible, or Anthropic's own API). Its key is kept in the OS credential store.
- A **combo** is a named, ordered list of models. The gateway falls back down the list on hard errors, but only before any output has reached the client (ADR-003).
- Every request is written to the usage ledger, and each cost carries its provenance.

The orchestrator (worktrees, checks, the cascade) becomes a layer on top of the gateway, and the Phase 0 benchmark is deferred until that layer is built. **Update 2026-11-11:** that layer is built, and the benchmark runs (ROADMAP C3).

**Why:** Clients that already exist can use the gateway on day one, and the orchestrator needs the gateway anyway. The benchmark only tests the orchestrator's premise, so it can wait for it.

**Cost:** For now, routing happens per request and is chosen by the client through the model name. Routing by task comes back with the orchestrator. **Update 2026-11-11:** it did, as a fixed ladder of agent steps per task (task engine v1); a task is not yet analysed into a proposed plan.


## ADR-017 — Universal, provider-independent conversation memory (proposed)

**Decision:** Plan a canonical local conversation event log, versioned summaries, scoped long-term facts, retrieval and task checkpoints so GPT, Claude, Gemini and local models can share **visible, authorized context**. The event log is source of truth; summary and retrieval indexes are derived. Provider KV caches, hidden reasoning and proprietary tool handles cannot be transferred.

**Why:** switching models inside long conversations otherwise loses context or requires expensive replay; memories must belong to the user, not one provider.

**Status:** withdrawn as an app feature by ADR-028 (no Auto Chat in the app); not implemented. Specification kept for reference: [UNIVERSAL_SHARED_MEMORY.md](UNIVERSAL_SHARED_MEMORY.md).

## ADR-018 — Cache-aware model affinity with no per-turn model picker (proposed)

**Decision:** In everyday Auto Chat, select from connected eligible models without asking user to pick a model every turn. Choose **Stay / Switch / Compact & Switch / Escalate** by expected full cost (including cache misses, context rebuild, memory operations, retries) and minimum quality/privacy constraints. Switch at clean turn boundaries only. Explicit spending and sensitive tool actions still require approval.

**Why:** a cheaper output-token price may be overwhelmed by cache loss and rehydration cost; uninterrupted user experience is central to an AI Cost Optimizer.

**Status:** withdrawn by ADR-028 (no Auto Chat in the app). Cache-aware affinity for gateway clients remains as P1-3. Reference: [UNIVERSAL_SHARED_MEMORY.md](UNIVERSAL_SHARED_MEMORY.md).

## ADR-019 — Gateway-first is current, benchmark-first order superseded

**Decision:** ADR-016's gateway-first sequence is current. ADR-013's benchmark-first order is retained as historical rationale but is superseded **for implementation order**. Validate model selection, conversation handoff and coding cascades in reproducible experiments before public savings claims.

**Status:** ADR-016 roadmap clarification. The gateway is now **implemented**, and rule-based `auto` is built (P0-5); the memory engine is withdrawn (ADR-028). Refer to [Implementation Status](IMPLEMENTATION-STATUS.md).

## ADR-020 — Five entitlement tiers per account-model (proposed)

**Decision:** UI groups accounts and models under Free, Subscription, API Paid, Local and Unknown, while keeping connection/auth method separate. A provider can have models in multiple tiers. Capability, pricing and quota are recorded per account-model with provenance and timestamps. Unknown is not free. Subscription entitlements are only used through verified provider-supported methods.

**Status:** partly built: billing (Free, Local, Subscription, Paid, Unknown) is set per connection and per model, with its source; the tier-grouped Providers views and an account-model entitlement schema are not (ROADMAP E). See [CLI / IDE / npm / tiers](CLI_IDE_NPM_PROVIDER_TIERS.md).

## ADR-021 — Headless Rust CLI and npm launcher (proposed)

**Decision:** reuse the same Rust gateway for desktop and a headless `str` CLI; npm/npx publishes a small launcher with verified platform binaries, not a second Node gateway. Ownership of the prospective npm package name is unverified; no released package is implied.

**Status:** proposed. See [CLI / IDE / npm / tiers](CLI_IDE_NPM_PROVIDER_TIERS.md).

## ADR-022 — Integration contract matrix and safe setup (proposed)

**Decision:** user-enabled, version-tested CLI/IDE setup with scoped gateway token, config backup/restore and feature-based tests. Do not claim compatibility for unsupported client APIs or route proprietary subscription credentials through unrelated clients.

**Status:** proposed. See [CLI / IDE / npm / tiers](CLI_IDE_NPM_PROVIDER_TIERS.md).

## ADR-023 — Authorized multi-account rotation with session affinity (proposed)

**Decision:** Per-provider authorized Account Pools may use Fill First, Round Robin, Sticky Round Robin and later cost-aware/weighted selection. Persist account/model affinity across normal conversation turns for cache and context continuity, subject to hard eligibility and policy filters. Retry or fallback only safely before stream output, with per-account health, explicit quota groups and reset-aware cooldown. Enforce client-key allowlists and atomic reservations across concurrent requests.

**Safety:** do not use account/key/IP rotation to circumvent bans, quotas, subscription terms, shared tenant limits or user authorization. Do not silently change an active agent attempt, tool call or stream. 401 and 403 are not normal transient errors.

**Status:** partly built: client allowlists, quota groups, spend reservations, cooldowns and a circuit breaker with a probe; account pools are not (ROADMAP E). See [ACCOUNT_API_ROTATION.md](ACCOUNT_API_ROTATION.md).

## ADR-024 — OmniRoute feature shortlist and validation gates (2026-11-11)

**Decision:** Adapt OmniRoute-style dynamic Auto-Combo candidate construction, verified Free Tier Intelligence with quota-group deduplication, explainable hard filters, bounded pre-stream failover/circuit breakers and conservative tool-output compression. Build on the existing Account Pool design rather than copying the upstream gateway (Universal Shared Memory is withdrawn, ADR-028). Later evaluate cache-aware affinity, quality-calibrated scoring, and opt-in semantic caching.

**Rejected for default product:** multi-model fusion/chaos, dozens of surfaced strategies, unsafe credential or TLS/browser impersonation, aggressive lossy context compression without independent benchmarks, and marketing claims derived solely from upstream benchmarks.

**Evidence:** documentation and public source references inspected; no local OmniRoute benchmark executed. No production savings or completed integration claimed.

**Implementation gate:** provider-verified entitlements, no privacy or budget bypass, safe rollback and controlled quality/cost-per-success evaluation. See [research and detailed adoption matrix](OMNIROUTE_FEATURE_RESEARCH.md).


## ADR-025 — Cost-safe gateway parity and owned vs external memory boundary (accepted; P0 built)

**Decision:** Review reconciliation ([Claude review](reviews/2026-11-11-architecture-review.md), [response](reviews/2026-11-11-review-response.md)) sets **[ROADMAP.md](ROADMAP.md)** as the single delivery-order authority. First priority is not adding new router features; it is preventing the gateway from increasing cost or violating provider rules: native same-format Anthropic passthrough/cache parity, safe error-scoped fallback, atomic budget/usage accounting, explicit session affinity, then explainable rule-based Auto aliases.

**Memory ownership:** ~~app-owned Auto Chat may build scoped context/summary/memory packages at controlled cache-stable checkpoints~~ (withdrawn, ADR-028). External CLI/IDE clients own their own prompts, history and compaction: gateway **never silently inserts memory or rewrites those prompts**. Only explicitly supplied authenticated session IDs may support persistent affinity; do not derive them from a hash of prompt content.

**User scope:** initial benchmark/market wedge is developers with metered API workloads because financial outcomes are measurable; **long-term product vision remains cost optimization for everyday users as well**. This does not authorize claiming cash savings for fixed subscription chat plans.

**Evidence:** code-backed findings were reviewed; synthetic multiplier examples are not real provider measurements. No code implementation is claimed by this ADR. **Update 2026-11-11:** P0-1 to P0-4 are implemented and tested against fake providers, with a live Gemini check; see [Implementation Status](IMPLEMENTATION-STATUS.md) and [architect decisions](reviews/2026-11-11-architect-decisions.md). Native Anthropic parity against a real endpoint was measured later (Implementation Status: Provider parity). Other proposed ADRs remain proposals until accepted as architecture and implemented/tested separately.

## ADR-026 — Official agent CLIs with their own sign-in; no session or OAuth bridges (2026-11-11)

**Decision:** To use the subscriptions and free tiers people already have, Pygg runs the vendors' own agent CLIs (Claude Code, Codex CLI, Gemini CLI, OpenCode) headless, each with the sign-in the user gave it, in a worktree per attempt (ADR-008). Pygg never reads, stores or forwards that sign-in, never exposes it to other clients through the gateway, and does not bridge web sessions, cookies or other products' OAuth tokens, even where an MIT-licensed project does.

**Narrowed by ADR-035** (2026-11-12): a site with no API can be connected as an opt-in web session, signed in to by the user in a Pygg window. Cookie import, other products' OAuth and agents' sign-ins stay out.

**Why:** Owner choice after reviewing OmniRoute's provider list. Bridges depend on terms the user did not agree to with Pygg and break without notice; the official CLIs are supported, and their cost stays honest (ADR-011: recorded as unknown, with the agent's own estimate in the source).

**Cost:** Pygg cannot see which plan an agent uses, so tier and billing of an agent step are the user's classing. Status: built. See [AGENTS.md](AGENTS.md).

## ADR-027 — Tasks start from the user's IDE agent; the app monitors (2026-11-11)

**Decision:** Most people will not treat a router app as an IDE. Smart tasks are started where people already work: their coding agent calls Pygg's MCP server (`<app> mcp`), which forwards to the gateway's task API. The app runs the tasks and is where they are watched, paid steps are approved, and results are reviewed; New Task in the app stays as a manual fallback. A client needs the user's **May start tasks** permission, sees only its own tasks, and cannot approve a paid step (ADR-010).

**Supersedes:** the parts of [SCREENS.md](design/SCREENS.md) and the [functional plan](SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md) that assume the user composes and plans tasks in the app first (Analyze Task, Plan/Run/Result as the primary path). Status: built and verified from real Codex and Claude Code sessions. See [AGENTS.md](AGENTS.md#from-the-ide-pygg-mcp-and-the-task-api).

## ADR-028 — MIT licence, a $5 benchmark budget, and no Auto Chat in the app (2026-11-11)

**Decisions (owner):**
- **Licence:** Pygg is open source under the MIT licence ([LICENSE](../LICENSE)). MIT-licensed code from other projects (OmniRoute among them) may be ported with its copyright and licence notice kept, recorded in `THIRD_PARTY_NOTICES.md`. ADR-026 still applies: no session, cookie or OAuth bridges, whatever their licence.
- **Paid benchmark budget:** under **$5 in total** for the benchmark (ROADMAP C3), enforced with `bench --budget-usd`. **Raised to $20 in total by the owner on 2026-11-11**, after the first pass showed the suite needs harder cases and repeats to say anything; about $2.44 of it was spent by then. Free tiers, local models and agents on the owner's subscriptions do not count against it, but their runs are reported apart.
- **No Auto Chat in the app.** The app is not a place to compose chats or tasks (ADR-027). ADR-017 and ADR-018 are withdrawn as app features: no app-owned conversation store, Memory & Context screen or per-turn model affinity for chats. What survives of that design is what tasks need: the bounded summary a failed attempt hands to the next step, and session affinity in the gateway (P0-4).

**Why:** Open source fits a local tool that holds keys and code; a small budget keeps the benchmark honest about what it can claim; and Auto Chat would make the app a chat client, which ADR-027 rules out.

## ADR-029 — Quality signals in Pygg's own ledger, in TensorZero's shape (2026-11-11)

**Decision:** ROADMAP C4's one-day TensorZero evaluation found the project archived by its owners on 2026-06-12 (last release 2026.6.0, a security fix), needing Postgres or ClickHouse and a second gateway with its own model names, storing whole conversations and sending usage analytics by default. Pygg does not adopt it. It borrows its small, documented data model instead: **metrics** (name, boolean or float, the level they describe, whether more or less is better) and **feedback** rows keyed to a request, an attempt or a task, with their source. Migration 0015 adds both and seeds `checks_passed`, `result_kept`, `retried`, `cancelled`, `declined` and `rework_minutes`; the task engine records them as they happen, and `tasks::quality` gives the known money per kept result, counting every paid step, kept or not.

**Why:** Pygg is one user on one machine with SQLite (ADR-002); a server stack is out of proportion, and an unmaintained gateway in the path is a security cost. The metric model is the part worth having, and keeping its shape leaves an export open if a maintained successor appears.

**Rules:** an unknown signal has no row, never a 0 or false; Pygg never infers quality from conversation content (ADR-025); request-level feedback comes only from a client that sends it.

## ADR-030 — The original-vision RFC: a task handoff inside ADR-028, the integrations hub next (2026-11-12)

**Context:** the owner's [RFC](RFC-2026-11-12-ORIGINAL-VISION-GAPS.md) listed eleven gaps (PV-01 to PV-11) against the original vision, a [counter-review](reviews/2026-11-12-original-vision-gaps-response.md) answered it, and the owner's [open-source study](research/2026-11-12-open-source-adoption-study.md) corrected the counter-review. On 2026-11-12 the owner told the chief architect to decide and carry out what was agreed, and to answer the rest. The decisions are in the [architect's response](reviews/2026-11-12-oss-patterns-response.md).

**Decisions:**
- **A task handoff is what ADR-028 already keeps** ("the bounded summary a failed attempt hands to the next step"). A richer one is built (H1), and it holds only for that task: it is bounded, built from what Pygg recorded, and shown to the user. It is never kept across tasks, never shown as memory, and never sent through the gateway. It ships **off by default** and becomes the default only if its A/B says so. This clarifies ADR-028; it does not reverse it.
- **Portable context between external clients (PV-02) stays deferred by name.** It is not "already built": MCP task dispatch carries a task description, not a typed, source-linked context package. Reopening it needs H1 to help first, and an owner decision on ADR-028.
- **Order:**
  1. PV-11: an honest N2 report and a sanitized results bundle.
  2. H1 with its A/B, out of the $20 benchmark budget.
  3. Recording providers' rate-limit headers (PV-06, a slice that costs nothing).
  4. The integrations hub (PV-05) for Claude Code, Codex CLI and OpenCode.
  5. One native `pygg` binary.
  6. npm, after the name is registered and binaries are signed (owner decisions).
- **H1's A/B (2026-11-12, [report](../benchmark/reports/2026-11-12-h1-handoff-ab.md)):** no case for `evidence` as the default, so **`tail` stays the default**. An exploratory run (3 arms × 36) found one nominal saving, about 12% fewer step-2 input tokens. The pre-registered confirmatory run was cut off at 25 pairs (inconclusive by its rule) and did not repeat that saving. `evidence` stays available per task. Its open question is whether it helps when the first step reaches its checks and fails; that needs a different first step. PV-02 therefore stays deferred: H1 has not yet been shown to help.
- **Deferred, unchanged:** PV-03, PV-07, PV-08 (the adaptive part, which is evaluated only on benchmark replays, never as a live shadow), PV-09 (as a feature; its deterministic rules go into the H1 builder) and PV-10.

## ADR-031 — The user enters a key; the app looks up the rest (2026-11-12)

**Context:** the owner connected eight providers and pointed OpenCode at Pygg. `auto` with tools then refused all 584 models: Claude's list does not state tool support, most models had no tier, no model outside OpenRouter had a price, and OpenRouter had been set as free. The owner: users cannot be expected to know model presets; they enter a key, and looking things up is the app's job.

**Decision:**
- **A reference catalog fills what a provider's own list leaves out.** Pygg reads [models.dev](https://models.dev), the open catalog OpenCode uses, keyed by each provider's own model ids. It fills context, maximum output, tool and image support, and the list price, only where they are unknown. The provider's own list comes first, and what the user set is never touched. A connection is matched to the catalog by its API address first (so Qwen's token plan is not confused with Qwen's pay-as-you-go API), then by its kind; local runtimes are never matched. Every fact names its source (`models.dev`), and a price from it reads "models.dev list price".
- **ADR-011 is kept, and read this way:** a catalog list price is a list price, the same kind of figure as a provider's own model list states. It gives estimated costs, never actual ones, and the user's own price replaces it.
- **Every model gets a tier.** Pygg's naming table (`curated:v1`) comes first; a model it does not know gets one from its list price band (`price-band:v1`: output under $1 per million tokens is T1, under $5 T2, under $20 T3, above T4), and `auto` says "from its list price". A $0 price gives no tier. A tier the user set stays theirs.
- **Claude models take tools** (`curated:v1`, Anthropic documents tool use for every Claude model), since Anthropic's list does not say so.
- **A connection set as free does not make a priced model free:** a model listed at a price above zero is paid, for ranking, holds and the ledger.
- **When nothing is eligible, the refusal groups its reasons** by connection and reason, so hundreds of models still say what to fix.

**How it runs:** at start-up, in the background, and after a connection's model list is refreshed; at most one download a day, kept next to the store (`reference/models.dev.json`) for use offline. The download is a GET of a public file and carries nothing about the user. `gateway lookup` applies it by hand.

**Measured on the owner's store (a copy):** models.dev described 466 of 584 models (Claude 14 of 14, DeepSeek 2 of 2, Z.AI 11 of 12, OpenRouter 373 of 431, Qwen's token plan 14 of 21, Gemini 37 of 63, Cohere 15 of 37; most of the rest are embedding, speech, image or batch variants), filled facts or prices for 98 and tiered 294 by price. `auto` with tools went from no eligible model to a ranked route, Qwen's subscription first.

## ADR-032 — A connection kept for its free models has only those on (2026-11-12)

**Context:** Connect lists OpenRouter under "Free tier" for its `:free` models. The owner connected it and set it as free, and Connections then showed all 432 of its models on, nearly all of them paid. A user who connects OpenRouter for its free models expects only those to be on, and to turn on a paid one when they mean to pay for it. Z.AI's model list, for its part, leaves out its free Flash models altogether.

**Decision:**
- **Two kinds of free.** For most free tiers the account is free (a trial key, Gemini's free tier): every call costs nothing within limits, so a list price does not make a call paid. For OpenRouter and Z.AI the free part is a set of models listed at $0 next to paid ones; their presets say so (`free_models`) and start as free.
- **On such a connection set as free, only models listed at $0 are on and free.** A model with a price is paid; one without a price is unknown, never free, since OpenRouter's own routers (`openrouter/auto`) list none and may pick a paid model. Pygg turns both off (`enabled_source = 'free-connection'`, schema v20), and the Models table says why ("Off: not $0").
- **The user's switch is theirs.** A model the user turns on stays on, billed as paid or unknown. A model Pygg turned off comes back on when that stops being true: the connection is set as paid, or the model's price or billing changes.
- **The ledger follows the same rule** as ranking (`store.rs` `billing!`): a paid OpenRouter model on a free connection is no longer recorded at $0.
- **Missing free models are added from the reference catalog** (ADR-031) for Z.AI only, whose list leaves them out; models answering in text only. OpenRouter's own list is complete and is not added to.

**Measured on a copy of the owner's store:** OpenRouter went from 432 models on to 16, all listed at $0; Z.AI from 12 paid models on to its 3 free Flash models (`glm-4.7-flash`, `glm-4.5-flash`, `glm-4.6v-flash`), which its list had left out; Cohere (a trial key) kept all 37. Z.AI answered a request for `glm-4.7-flash` with "overloaded" (429), not "unknown model".

## ADR-033 — The gateway serves the Responses API, translated through Chat Completions (2026-11-12)

**Context:** Codex CLI 0.163.0 refuses `wire_api = "chat"` ("no longer supported, set wire_api = responses"), so it could reach Pygg only through MCP tasks, never as a gateway client. Captured Codex traffic (a local capture server, 2026-11-12) has `instructions` empty and its prompt in `developer` messages; `input` items of type `message`, `function_call`, `function_call_output`, `custom_tool_call`, `custom_tool_call_output` and `reasoning`; `function`, `namespace` (nested functions) and `web_search` tools, and Codex's free-form `custom` tool `apply_patch` (a Lark grammar) when its model metadata asks for one; for OpenAI's own models, tools in an `additional_tools` input item; `store: false`, `include: ["reasoning.encrypted_content"]`, `reasoning.summary: "auto"`, `prompt_cache_key`, `client_metadata`. It reads `response.output_item.done` for each item and needs `response.completed` with usage.

**Decision:**
- **`POST /v1/responses` is a third surface on the same pipeline** (`gateway/responses.rs`): authentication, routing, `auto` and combos, client connection lists, limits and strict holds, the journal, session pins, health and quota, and the ledger are `forward()`'s, as for the other two.
- **One new translator, Responses ↔ Chat Completions.** An OpenAI-format connection gets Chat Completions. An Anthropic-format connection gets Messages through the existing, tested Chat Completions → Messages translator (B5), and its answer comes back the same two steps. Composing keeps one translator per pair of formats instead of a fourth.
- **Native passthrough only where a preset says so** (`Preset::serves_responses`): OpenAI. A provider with a partial or beta Responses API is translated.
- **Custom tools are functions with one string parameter, `input`**, whose description carries the tool's grammar; the call comes back as a `custom_tool_call` with that string. **Namespace tools are flattened** as `<namespace>__<name>` (Chat Completions names allow no dots) and come back with their namespace. Both are found again by name from the request's own tool list, so nothing is kept between requests.
- **Reasoning:** a provider's reasoning comes back as a `reasoning` item, as summary text when the client asked for a summary and as reasoning text otherwise. Reasoning sent back goes to providers that take it (`takes_reasoning`) as `reasoning_content`. Another provider's encrypted reasoning cannot be passed on, and is named in `x-smarttaskrouter-dropped` (`reasoning`), as are hosted tools (`web_search tool`) and every top-level field the translation does not carry (`prompt_cache_key`, `client_metadata`, …). Nothing is dropped without being named.
- **Pygg stores no responses.** `previous_response_id`, `conversation` and `background` are refused with HTTP 400; the client sends the whole conversation (Codex does with `store = false`).

**Consequences:** Codex can use any connection, metered like any client; the Responses API's own hosted tools (web search, file search, code interpreter) are not available through a translated connection; Anthropic's prompt cache is not used on the translated path (no `cache_control` markers are invented); a Codex model Codex does not know runs with Codex's fallback metadata unless the user gives it a model catalog (AGENTS.md). Follow-up done the same day: "Codex: models through Pygg" in Agents → From Your IDE sets this provider up in one click, with a `pygg` profile file (AGENTS.md).

## ADR-034 — Pools of API-key accounts; never of web sign-ins (2026-11-12)

**Context:** the owner asked for account pools, pointing at providers whose free use comes only through signing in on their website, not through an API key.

**Decision:**
- **A pool groups connections of one provider kind, each a separate account the user holds.** Each member is its own connection with its own key. A connection joins only after the user confirmed it has its own provider quota (CR-02, "Independent quota confirmed"). Several keys of one account cannot be pooled: that would only get around the account's limits, which ADR-026, OR-16 and the product rules forbid.
- **A pool's name is a routing prefix.** `<pool>/<model>` tries the members in the user's order, skipping disabled ones. It moves to the next on the errors that already move across quota groups: 401, 402, 403, 429, and Anthropic's out-of-credit 400. A session stays on the member that first answered it. The ledger names the connection that answered. `GET /v1/models` lists `<pool>/<model>` for the models enabled members offer. The name may not be a connection's id, a combo's name or `auto`.
- **Not pooled:** website sign-ins, cookies and sessions of consumer apps (e.g. a provider whose free tier exists only in its web app). They stay out under ADR-026 and OR-16, as the owner decided before ("no web session, cookie or OAuth bridges"). ADR-035 later added web sessions as an opt-in connection kind; they still cannot join a pool. A provider's own **OAuth flow that issues an API key** is not such a bridge: the user signs in on the provider's page, and what Pygg keeps is an ordinary key for the provider's API. OpenRouter documents one (PKCE), so Connect can offer it.

**Built:** schema v22 (`providers.pool`, `pool_rank`), `store::set_pool` / `move_in_pool` / `pool_members` and `resolve`, the model list, and Connections → Pool (join, create, order, leave), with a test of each rule.

## ADR-035 — Web-session connections for AI sites with no API, opt-in and unofficial (2026-11-12)

**Context:** some AI services have no API and are offered only behind an account sign-in on their website or app. The first one the owner named is [Muse](https://muse.ai) (Meta's personal AI agent: web, app and WhatsApp; sign-in with a phone number or email; no public API). ADR-026, ADR-034 and OR-16 refused web-session bridges. The owner, 2026-11-12: support the cases that have only a sign-in session, and warn the user that it puts their account at risk; the sign-in may be kept, since it never leaves the user's machine.

**How other routers do it** ([research notes](research/2026-11-12-web-session-routers.md), from their source): 9router (MIT) has `muse-web` and seven other web providers, OmniRoute (MIT) has 37. The user pastes cookies into their dashboard (9router's `muse-web` asks for `hatch_sess`, `hatch_gw`, `hatch_vml`, `hatch_native_auth_device`), they are kept in the router's SQLite file, and where a site's own web API is simple they call it over HTTP; for the hard ones (ChatGPT, Gemini and Muse) they drive a pooled Playwright Chromium loaded with those cookies, with stealth builds and other anti-bot measures. 9router's `muse-web` opens a new chat per request, refuses tools, streams by reading the reply every 300 ms, and clicks **deny** on every approval Muse asks for. Meta also has an official API for its Muse models (`https://api.meta.ai/v1`, OpenAI and Anthropic formats, pay as you go); the muse.ai agent app has none.

**Status:** accepted by the owner on 2026-11-12, and built (below).

**Decision:** a new connection kind, **Web session (unofficial)**, for sites with no API. **It is a provider like any other** (owner, 2026-11-12: through the router the user still sends and gets answers, so the user can treat the site as an AI provider): it is chosen in Connect, has its connection row, models, health, limits and ledger entries in Connections and Usage, appears in `GET /v1/models`, and any client reaches it on all three surfaces by naming `<connection>/<model>` or through a combo. What differs is only the transport (a signed-in web page instead of an HTTP API) and the safeguards below. It narrows ADR-026, ADR-034 (pools) and OR-16 for this kind only; everything else they refuse stays refused: no cookie import, no other products' OAuth tokens, no pools of web sign-ins, no TLS impersonation, no limit evasion.

**How it works:**
- **A Pygg-owned, isolated webview per connection, and the sign-in is kept.** Each connection gets its own Tauri webview window (label `web-<connection id>`) with its own WebView2 profile directory under the app's data folder (`websessions/<connection id>`). The user signs in there themselves, on the site's own page, once: the profile keeps the session across restarts like a browser does (WebView2 encrypts its cookie store with the Windows user's DPAPI key), and the site refreshes it itself. Unlike 9router and OmniRoute, nobody copies or pastes cookies, and Pygg never holds them. A webview is opened hidden when a request needs it and closed after 10 minutes unused; **Open sign-in window** shows it. The window has no Tauri capability, so the site's scripts cannot call Pygg's commands; navigation is limited to the recipe's hosts (the site and its sign-in hosts) and anything else is blocked.
- **Its own profile, never its own fingerprint** (owner, 2026-11-12: "each provider its own profile and its own fingerprint, so it is safer"). Each connection already has its own WebView2 profile folder: its own cookies, storage, cache and sign-in, so one site never sees another's session and removing a connection removes all of it. That separation is the safety the owner asked for. Pygg does **not** give a connection a different browser fingerprint: it never spoofs or randomises the user agent, client hints, canvas, WebGL, fonts, screen, time zone or similar, and never hides that a page is driven. Doing so is detection evasion, which this ADR forbids (safeguard 8) and the sites' terms prohibit. Every window presents itself as the Edge WebView2 it is.
- **Free while the user signs in, held to the site while Pygg types** (owner, 2026-11-13: "the window is a real browser"). A recipe's host list could not foresee every sign-in hop: Google's country cookie domain (`accounts.google.com.vn`) and Grok's `auth.grokusercontent.com` each left a live sign-in spinning. So while the user has the window open (Open Sign-In Window, until they close it), it may go to any https address, like a browser. Whenever Pygg itself types or presses Send, the page must be the site's own host (`site_host`, e.g. `chatgpt.com`; sign-in hosts such as Google do not count): otherwise it sends nothing and marks the connection Needs you. Out of the user's hands, the window still follows the recipe's hosts, and a refused address, popup or non-https address is named on the connection (host or scheme only).
- **The wait for a reply follows the request** (2026-11-13 evening, after the [web coding benchmark](../benchmark/reports/2026-11-13-web-coding.md) lost 53 of 72 requests on Muse, Grok and Perplexity to a fixed 60 s first-word wait): the first word of a reply is waited for 60 s plus 1 s for every 100 estimated prompt tokens, up to 240 s, and a page that visibly shows it is still answering (the recipe's "busy" signal, a stop button) has given its first word, so only the whole reply's wait applies, 300 s extended by the same extra. Nothing else in the pace changes: the 10 s rest and the daily limit stand, and Pygg still sends one request at a time.
- **A person's pace, and rotation without bursts** (owner, 2026-11-13, after reading why 9router users are banned: subscription OAuth used outside the vendors' apps, account rotation to get around limits, and bursts of automated requests). Each web session sends one request at a time, rests 10 s after a reply (`PACE_SECS`, fixed, never randomised: Pygg slows down, it does not disguise itself) and stops at its daily limit (50 by default, 1 to 1,000, migration 0026; counted from the ledger since 00:00 UTC, Pygg's own 429 refusals excluded). At the limit it answers 429 `daily_limit` with Retry-After and sends nothing. Web sessions of one site may now pool (the owner's decision), each confirmed as a different account: a pool sends to the account used least today, and each account keeps its own limit and pace, so rotation spreads the load instead of multiplying it. This supersedes "web sessions cannot join a pool" in ADR-034 for web sessions only.
- **A recipe per site** (`gateway/websession.rs`, data in Rust, reviewed like code): the start URL, the hosts, and selectors for the prompt box, the send button, a reply and a "still answering" signal, plus the signs of a sign-in page or a challenge. Pygg types the prompt into the box (`insertText`), clicks Send, waits until the reply stops changing and the site no longer shows it is answering, and reads the reply's text. Where the recipe has no reply selector, the reply is the text that appeared after the prompt. Pygg runs scripts only to read and fill the page; they never read `document.cookie` or storage, and the site's cookies never leave the profile directory.
- **A gateway adapter that makes the site an ordinary OpenAI-format provider.** The app serves each web-session connection as a Chat Completions endpoint on loopback, reachable only with a token that lives in memory, and the connection points at it with `wire = "openai"`. So the gateway's own path (the Anthropic and Responses translators, limits, health, sessions, combos and the ledger) applies unchanged. A request is flattened into one prompt (system text, then the turns labelled), sent in a **new chat** each time so requests do not leak into each other, and the reply comes back as a chat completion; a streaming client gets it as it appears, the page read every 300 ms and only new text sent. Token counts are not reported by the site, so usage is unknown (estimated from characters only where a limit needs it, and marked as such).
- **Not in the headless CLI gateway** (`pygg gateway serve`): there is no webview there, so these connections are skipped with that reason.

**What it cannot do:** no tool or function calling, no images or files, no system-prompt guarantees (the site may have its own), no token usage, no control of the model behind the site. A request with `tools` (or `tool_choice`, or a tool result in its messages) to a web-session model is refused with 400 and the reason, and the model is listed with `tools = false`, so coding agents that send tools are not routed there. A Muse-specific limit: **Muse is an agent that can act on the user's connected apps** (email, calendar, purchases, with its own approvals). Text sent through Pygg can ask it to act. Pygg clicks only the recipe's Send button and, as 9router does, the site's own **deny** on an approval Muse asks for, so the agent never acts on the user's behalf through Pygg; the reply then says so and the connection shows it.

**Safeguards (required, each tested):**
1. **Off by default, labelled "Web session (unofficial)".** Connect lists these presets in their own group with that label, next to the API providers. A new connection starts disabled and stays out of routing until its warning is acknowledged and the user has signed in; the user can turn it off like any connection.
2. **A warning the user acknowledges, per connection,** before Pygg creates it: the site's terms may forbid automated use; the account can be limited or banned; the site can change and break the connection at any time; Pygg drives the site as the signed-in user (for Muse: an agent with access to the user's connected apps). The acknowledgement is stored with the connection (`web_ack_at`, `web_ack_version`); a later change of the warning's text asks again.
3. **No passwords, no cookie import, nothing secret written.** Pygg never asks for or stores a password, and never imports cookies from the user's browsers. Cookies and session data stay in the profile directory: never in logs, diagnostics, the ledger, exports, or error messages (a page's text in an error is cut short and redacted). Diagnostics never include the `websessions` folder.
4. **The profile directory is deleted when the connection is removed.** The window is closed first; if WebView2 still holds the files, the folder is queued and deleted at the next start, before any webview opens.
5. **Never chosen by `auto` or `auto/cheap`.** These models have no tier and `tools = false`, and `auto` excludes the kind outright, with the reason. They are used only when the user names the model or puts it in a combo. They cannot join a pool (ADR-034 stands).
6. ~~**Billing is subscription, cost unknown,** never $0 (ADR-011): the ledger records the request with an unknown cost and says why.~~ **Amended by the owner on 2026-11-13 (schema v27):** a web session is **free by default**: the site is used on the user's own plan, paid by the month or free, never per request, so each request, failed or cut short included, is recorded at **$0** with the source "web session: your plan with the site, no price per request". The user can set it to **Subscription** instead, which counts requests as quota and records the cost as Unknown, as before. Paid and Local are refused. Existing web sessions, all Subscription until then by force, are moved to Free by the migration.
7. **One request at a time per connection,** with a pause of a few seconds between requests and a time limit per reply. A second request waits briefly and is then refused with 429 and a `Retry-After`.
8. **Challenges stop the connection.** On a CAPTCHA, a sign-in page, a login challenge or a bot check, Pygg stops, marks the connection "Needs you" (or "Not signed in"), answers the client with 401 and the reason, which holds the connection until a check finds it signed in again, and tells the user to open the window and deal with it themselves. Pygg never tries to solve, skip or hide from such a check: no user-agent or fingerprint changes, no stealth scripts, no off-screen windows to hide automation, no retries around it. These are the parts of 9router and OmniRoute Pygg does not copy.
9. **An API comes first.** Where a site has an official API (Meta's Muse models at `api.meta.ai`), Connect offers it as an API-key preset, and the web-session preset says it exists.

**Built** (schema v24): `websession/` (recipe, page driving, loopback endpoint, profile folders) and `websession/window.rs` (the Tauri window), the gateway's routing rule and billing, `store::add_web_provider`, `set_web_state` and the pool and billing refusals, `auto`'s exclusion, the commands `web_session_open` and `web_session_check`, and Connections (the preset group, the warning sheet, the sign-in row). The Muse recipe's selectors follow 9router's `muse-web` (MIT, THIRD_PARTY_NOTICES.md). 21 tests cover the safeguards with a scripted page. Muse was verified live by the owner on 2026-11-12. **More sites, 2026-11-12** (the owner: "Grok should be done; drop Gemini, since it already has a free API key"): ChatGPT, Perplexity, DeepSeek and Grok recipes, each with its own sign-in hosts. Gemini web, UC and Veo AI Free are left out ([research](research/2026-11-12-web-session-routers.md)). Grok's Cloudflare checks stop the connection like any other check; if its sign-in cannot finish in the window without disguising it, Grok is dropped rather than worked around.

**Consequences:** the user carries the account risk, knowingly, per connection; Pygg's own risk is a recipe that breaks when the site changes (it fails closed, as "the page has changed", and never falls back to guessing). A web-session request is slow (seconds to minutes) and uncounted in tokens, so it suits a user who names the model for chat-like work, not coding agents. The first recipe (Muse) is written without a signed-in session to look at; it is confirmed or corrected when the owner signs in in the dev build.

## Open decisions (not yet finalized)

- ~~First agent benchmark client and model matrix~~: settled by the C3 first pass (OpenCode 1.18.4 through the gateway; see the [report](../benchmark/reports/2026-11-11-c3-first-pass.md)).
- A code-signing certificate (ROADMAP D3) and the npm package name (D2), before distribution.
