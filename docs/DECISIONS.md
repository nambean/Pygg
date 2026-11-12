# Decisions

> **Status:** Current · architecture decision records (ADR-001 to ADR-030) · reviewed 2026-11-11 · order: [ROADMAP](ROADMAP.md) · built: [Implementation Status](IMPLEMENTATION-STATUS.md)

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

## Open decisions (not yet finalized)

- ~~First agent benchmark client and model matrix~~: settled by the C3 first pass (OpenCode 1.18.4 through the gateway; see the [report](../benchmark/reports/2026-11-11-c3-first-pass.md)).
- A code-signing certificate (ROADMAP D3) and the npm package name (D2), before distribution.
