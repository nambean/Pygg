# Decisions

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

**Decision:** v1 uses a curated tier table (T0–T4) plus minimum-tier rules per task category and complexity, with hard eligibility filters. Measured cost-per-success ranking comes in Phase 3, and only above a minimum sample size.

**Why:** There is no measured data at the start. Inventing quality scores would violate the plan's principles.

**Consequence:** The UI says "lowest eligible tier under your policy", never "cheapest".

## ADR-007 — Deterministic checks gate the cascade

**Decision:**
- A stage may start at a cheaper tier and escalate only if it has a deterministic check (typecheck, tests, build, schema).
- An LLM review can advise but never passes a stage.
- Without a check there is no automatic cascade. The user may review a direct run, and its result is recorded as **Manually Accepted**.

**Why:** A cheap model's output is only safe to accept when a machine can verify it.

## ADR-008 — A Git worktree per attempt; Git-only writes in MVP

**Decision:** Each attempt runs in its own Git worktree created from a recorded base revision. Failed attempts are discarded. Workspaces that are not Git repositories get read-only planning in MVP.

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

**Note:** Running the benchmark against paid models needs an explicit budget from the owner.

## ADR-014 — Sample data is always labeled

**Decision:** Until the task engine exists, task screens show sample data under a permanent "Sample tasks for design review" banner. Screens that are designed but not built say so, instead of showing empty data.

**Why:** An empty list would look like a real, empty configuration, and unlabeled sample numbers would look like measurements.

## ADR-015 — Commit the pnpm lockfile

**Decision:** `pnpm-lock.yaml` is committed. DownFetch ignores its lockfile; this project does not.

**Why:** Reproducible installs matter more than avoiding lockfile churn for a desktop app with a signed release pipeline ahead.

## ADR-016 — Build the gateway first

**Decision:** The multi-provider gateway is built before the task orchestrator (2026-11-11, owner's choice). It works in the spirit of tools like 9router:
- Clients the user already has (Codex, Cursor, OpenCode, scripts) point at one local base URL.
- Each connection is an OpenAI-compatible provider account. Its key is kept in the OS credential store.
- A **combo** is a named, ordered list of models. The gateway falls back down the list on hard errors, but only before any output has reached the client (ADR-003).
- Every request is written to the usage ledger, and each cost carries its provenance.

The orchestrator (worktrees, checks, the cascade) becomes a layer on top of the gateway, and the Phase 0 benchmark is deferred until that layer is built.

**Why:** Clients that already exist can use the gateway on day one, and the orchestrator needs the gateway anyway. The benchmark only tests the orchestrator's premise, so it can wait for it.

**Cost:** For now, routing happens per request and is chosen by the client through the model name. Routing by task comes back with the orchestrator.


## ADR-017 — Universal, provider-independent conversation memory (proposed)

**Decision:** Plan a canonical local conversation event log, versioned summaries, scoped long-term facts, retrieval and task checkpoints so GPT, Claude, Gemini and local models can share **visible, authorized context**. The event log is source of truth; summary and retrieval indexes are derived. Provider KV caches, hidden reasoning and proprietary tool handles cannot be transferred.

**Why:** switching models inside long conversations otherwise loses context or requires expensive replay; memories must belong to the user, not one provider.

**Status:** proposed design, not implemented. Detailed specification: [UNIVERSAL_SHARED_MEMORY.md](UNIVERSAL_SHARED_MEMORY.md).

## ADR-018 — Cache-aware model affinity with no per-turn model picker (proposed)

**Decision:** In everyday Auto Chat, select from connected eligible models without asking user to pick a model every turn. Choose **Stay / Switch / Compact & Switch / Escalate** by expected full cost (including cache misses, context rebuild, memory operations, retries) and minimum quality/privacy constraints. Switch at clean turn boundaries only. Explicit spending and sensitive tool actions still require approval.

**Why:** a cheaper output-token price may be overwhelmed by cache loss and rehydration cost; uninterrupted user experience is central to an AI Cost Optimizer.

**Status:** proposed and requires cross-provider evaluation. Reference: [UNIVERSAL_SHARED_MEMORY.md](UNIVERSAL_SHARED_MEMORY.md).

## ADR-019 — Gateway-first is current, benchmark-first order superseded

**Decision:** ADR-016's gateway-first sequence is current. ADR-013's benchmark-first order is retained as historical rationale but is superseded **for implementation order**. Validate model selection, conversation handoff and coding cascades in reproducible experiments before public savings claims.

**Status:** ADR-016 roadmap clarification. The gateway is now **implemented**, but the memory engine/auto cost optimizer remain proposed. Refer to [Implementation Status](IMPLEMENTATION-STATUS.md).

## ADR-020 — Five entitlement tiers per account-model (proposed)

**Decision:** UI groups accounts and models under Free, Subscription, API Paid, Local and Unknown, while keeping connection/auth method separate. A provider can have models in multiple tiers. Capability, pricing and quota are recorded per account-model with provenance and timestamps. Unknown is not free. Subscription entitlements are only used through verified provider-supported methods.

**Status:** specified; implementation pending. See [CLI / IDE / npm / tiers](CLI_IDE_NPM_PROVIDER_TIERS.md).

## ADR-021 — Headless Rust CLI and npm launcher (proposed)

**Decision:** reuse the same Rust gateway for desktop and a headless `str` CLI; npm/npx publishes a small launcher with verified platform binaries, not a second Node gateway. Ownership of the prospective npm package name is unverified; no released package is implied.

**Status:** proposed. See [CLI / IDE / npm / tiers](CLI_IDE_NPM_PROVIDER_TIERS.md).

## ADR-022 — Integration contract matrix and safe setup (proposed)

**Decision:** user-enabled, version-tested CLI/IDE setup with scoped gateway token, config backup/restore and feature-based tests. Do not claim compatibility for unsupported client APIs or route proprietary subscription credentials through unrelated clients.

**Status:** proposed. See [CLI / IDE / npm / tiers](CLI_IDE_NPM_PROVIDER_TIERS.md).

## ADR-023 — Authorized multi-account rotation with session affinity (proposed)

**Decision:** Per-provider authorized Account Pools may use Fill First, Round Robin, Sticky Round Robin and later cost-aware/weighted selection. Persist account/model affinity across normal conversation turns for cache and context continuity, subject to hard eligibility and policy filters. Retry or fallback only safely before stream output, with per-account health, explicit quota groups and reset-aware cooldown. Enforce client-key allowlists and atomic reservations across concurrent requests.

**Safety:** do not use account/key/IP rotation to circumvent bans, quotas, subscription terms, shared tenant limits or user authorization. Do not silently change an active agent attempt, tool call or stream. 401 and 403 are not normal transient errors.

**Status:** specified, not implemented. See [ACCOUNT_API_ROTATION.md](ACCOUNT_API_ROTATION.md).

## ADR-024 — OmniRoute feature shortlist and validation gates (2026-11-11)

**Decision:** Adapt OmniRoute-style dynamic Auto-Combo candidate construction, verified Free Tier Intelligence with quota-group deduplication, explainable hard filters, bounded pre-stream failover/circuit breakers and conservative tool-output compression. Build on existing Account Pool and Universal Shared Memory designs rather than copying the upstream gateway. Later evaluate cache-aware affinity, quality-calibrated scoring, and opt-in semantic caching.

**Rejected for default product:** multi-model fusion/chaos, dozens of surfaced strategies, unsafe credential or TLS/browser impersonation, aggressive lossy context compression without independent benchmarks, and marketing claims derived solely from upstream benchmarks.

**Evidence:** documentation and public source references inspected; no local OmniRoute benchmark executed. No production savings or completed integration claimed.

**Implementation gate:** provider-verified entitlements, no privacy or budget bypass, safe rollback and controlled quality/cost-per-success evaluation. See [research and detailed adoption matrix](OMNIROUTE_FEATURE_RESEARCH.md).

## Open decisions (not yet finalized)

- **License.** Not chosen yet; this is a decision for the owner.
- **First agent benchmark client and paid test budget.** OpenCode is a candidate, but version, CLI flags, compatibility and measurable usage must be tested against the exact installed version; external provider calls need explicit user approval.

## ADR-025 — Cost-safe gateway parity and owned vs external memory boundary (accepted design, code pending)

**Decision:** Review reconciliation ([Claude review](reviews/2026-11-11-architecture-review.md), [response](reviews/2026-11-11-review-response.md)) sets **[ROADMAP.md](ROADMAP.md)** as the single delivery-order authority. First priority is not adding new router features; it is preventing the gateway from increasing cost or violating provider rules: native same-format Anthropic passthrough/cache parity, safe error-scoped fallback, atomic budget/usage accounting, explicit session affinity, then explainable rule-based Auto aliases.

**Memory ownership:** app-owned Auto Chat may build scoped context/summary/memory packages at controlled cache-stable checkpoints. External CLI/IDE clients own their own prompts, history and compaction: gateway **never silently inserts memory or rewrites those prompts**. Only explicitly supplied authenticated session IDs may support persistent affinity; do not derive them from a hash of prompt content.

**User scope:** initial benchmark/market wedge is developers with metered API workloads because financial outcomes are measurable; **long-term product vision remains cost optimization for everyday users as well**. This does not authorize claiming cash savings for fixed subscription chat plans.

**Evidence:** code-backed findings were reviewed; synthetic multiplier examples are not real provider measurements. No code implementation is claimed by this ADR. Other proposed ADRs remain proposals until accepted as architecture and implemented/tested separately.
