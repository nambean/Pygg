# RFC: Pygg original-vision gaps — shared context, npm/CLI, intelligent orchestration

> **Status:** Proposal for independent Claude counter-review; **not approved**, **not implemented by this RFC**.  
> **Date:** 2026-11-12. **Product:** Pygg — *One Flow. Many Agents.*  
> **Baseline:** `docs/ROADMAP.md`, `docs/IMPLEMENTATION-STATUS.md`, `docs/DECISIONS.md` and `docs/README.md` on `main` through `fecee7e`.  
> **Purpose:** Recover the user's original product requirements that are missing, deferred, or narrower than intended, and give Claude falsifiable technical proposals. **The authoritative implementation status and delivery order do not change until the owner approves new ADR/ROADMAP changes.**

## 0. Why this RFC exists

Pygg has evolved from an AI cost-saving model gateway into a **local-first, IDE-triggered AI Agent and Task Orchestrator**. The shipped foundation is substantial: Rust/Tauri desktop plus localhost gateway, `auto` and `auto/cheap`, model capability and data-use filters, explicit session affinity, cost ledger, approvals, Git-worktree-isolated agent cascades, MCP/task API, native Claude Code/Codex/OpenCode execution, Windows installer and measured N2 benchmark. **Do not propose rebuilding these.**

The owner's earlier vision had additional product capabilities:

1. A **provider-independent shared context** so switching Claude ↔ GPT ↔ Gemini ↔ Qwen ↔ local models and different agents does not lose the useful *visible* task/conversation state.
2. A **developer-first npm/npx + native CLI** experience, installable without the desktop GUI, plus one-click safe IDE/agent integration.
3. A **true cost-and-quality orchestrator** choosing an eligible agent/model/account with attention to quota, cache, task outcome and rework, not just list token prices.
4. Optional **skills/prompt discovery**, policy simulation, controlled multi-account resource routing and safe compression, with auditable evidence.

Most are present as old reference designs or ROADMAP E/P1/D entries; **being documented is not the same as being built or being given adequate priority**. The N2 benchmark (12 Node coding cases, three repeats each; the report says Haiku versus Sonnet 88% lower *estimated list-price money per solved task* on that corpus) strengthens the need to optimize **cost per successful result**, but does not establish a universal savings rate.

### Source hierarchy and scope conflict

- [Documentation index](../README.md), [ROADMAP](../ROADMAP.md) and [Implementation Status](../IMPLEMENTATION-STATUS.md) remain authoritative about sequence and actual verification; [Decisions](../DECISIONS.md) controls accepted ADRs.
- **ADR-026:** Only official agent CLIs with their own authorized sign-in; **no credential/session/cookie/OAuth bridge**, no sharing those sign-ins through the gateway.
- **ADR-027:** Users delegate tasks from their IDE/agent; Pygg monitors, runs, approves and reviews. The desktop is **not another IDE**.
- **ADR-028:** No app-owned Auto Chat. ADR-017/018's consumer chat and app-managed turn-by-turn conversational memory are **withdrawn**. This document **does not silently reverse that decision**.
- **ADR-025:** For opaque external CLI/IDE clients, never silently inject memory into prompts, rewrite history, infer session identifiers from prompt content, alter hidden reasoning or try to transfer provider KV cache.
- **Existing safety:** no mid-stream model changes, no automatic replay after ambiguous execution, no quota evasion, scoped keys and privacy, paid approvals, exactly-once ledger/Unknown+at-risk.

**Important reconciliation proposal:** Re-open only a **task-scoped shared working context** owned by Pygg's existing task engine, because a task/cascade is already Pygg-owned. Do *not* re-open consumer Auto Chat by default. More ambitious provider-independent conversation handoff for external clients is explicitly opt-in and requires the client to export visible conversation state through a supported API/MCP call. If Claude believes even this violates ADR-028, it must identify the exact boundary and offer an alternative.

## 1. Gap register and recommended order

| ID | Earlier product intent | Current evidence/status | Proposed priority | Estimated effort and feasibility |
| --- | --- | --- | --- | --- |
| **PV-01** | Shared task memory and portable context handoff across Agent/model changes | Only bounded failed-check output passed to next step; old universal conversation memory withdrawn | **P1 candidate**, start with deterministic task state | **M**, high feasibility for Pygg-owned tasks; **L**, limited for opaque client sessions |
| **PV-02** | Explicit shared-context integration for existing IDE clients | MCP task tools built; no opt-in client-to-Pygg conversation-state export/import contract | **P2/conditional** | **M–L**, client cooperation required |
| **PV-03** | Cache-aware Stay/Switch/Escalate with switching cost | Session pin/cache metrics built; no measured optimization of cross-attempt handoff cost | **P1 experiment** | **M**, moderate; savings unproven |
| **PV-04** | Install/use headless Pygg via `npm`/`npx` and a small CLI | `gateway serve` and `Pygg.exe mcp` exist; native universal CLI and npm package unshipped | **D2 — keep near-term** | **M**, high on validated OS targets |
| **PV-05** | Safe one-click IDE and Agent Integrations Hub | MCP/snippets and agent detection built; backup/restore, version matrix and guided installer absent | **D2 — keep near-term** | **M**, high for first supported clients |
| **PV-06** | Accurate free/subscription/API/local quota and entitlement intelligence | Billing labels, prices, model facts, health and quota groups built; per-account/model live entitlement tracking absent | **E → measured pilot** | **M–L**, provider API dependent |
| **PV-07** | Authorized API account pools, fair usage, sticky/account-aware failover | Connection, quotas, cooldown and affinity built; pooling not built | **E, gated** | **L**, medium with restricted supported providers |
| **PV-08** | Task-aware policies and shadow routing: quality-adjusted cost per result | Fixed task ladder + rule-based request routing + deterministic checks, quality feedback built; adaptive cross-task policy not built | **P1 experiment before broad E** | **M–L**, contingent on better benchmarks |
| **PV-09** | Safe token/context compression and dedup | Spec exists; not enabled as a product feature | **P1 shadow-only** | **S–M** for owned logs; high risk for opaque prompts |
| **PV-10** | Curated Skills & Prompts Hub with provenance | Historical UI proposal, deferred ROADMAP E/P2 | **P2**, after integration safety | **M–L**, nontrivial supply-chain risk |
| **PV-11** | Reproducible cost/quality/continuity measurements and feedback loop | N2, quality signals and ledger exist; limited corpus/agent, incomplete report placeholders and raw files outside repo | **Immediate evidence gate** | **S–M**, high if raw artifacts available |

Effort sizes are relative engineering complexity (S/M/L), **not durations or delivery promises**. “High feasibility” means implementable with existing Rust/Tauri/SQLite and official interfaces, **not** that its business value or provider compatibility has been proven.

## 2. PV-01 — Task-scoped Universal Shared Working Context (highest vision gap)

### Why Pygg needs it

A coding task may be attempted by a cheap local model, then a subscription agent, then a premium API agent. Today the second step can receive a short failure tail, but may lose the previous attempt's verified findings, files inspected, constraints, disproven hypotheses, approved checks and context gained at cost. If the task repeats large source context or blind exploratory tool calls, routing saves model prices but wastes tokens/time and can reintroduce failed edits. **The same task's visible evidence should survive an Agent/model change, without requiring reuse of one vendor's conversation or hidden state.**

### Product behavior

- One **canonical Pygg task ID**; each stage/attempt has its own attempt ID, agent identity, model, source base commit/worktree, recorded permissions and policy snapshot.
- Visible, authorized evidence captured: original goal, confirmed user constraints, approved checks, Git base hash, inspected path names/hashes, relevant *non-secret* tool outputs, failing checks, prior approach summary, verified result and next safe action. Preserve references to original raw artifacts, not just mutable summaries.
- At an **attempt boundary**, build a context handoff from that evidence for the next Agent. The next Agent starts in its own fresh worktree of the original base commit (existing invariant), so **never claim prior code edits are present** unless a separately approved patch/branch was applied.
- Explicitly distinguish `observed`, `user_decision`, `agent_hypothesis`, `check_verified` and `superseded`. Contradictions are kept with citations; agent guesses never become user facts.
- A sensitive workspace is isolated: no memory flows to another workspace, client, provider or agent unless workspace/step permissions allow it. The existing sensitive-workspace approval is mandatory even when information is reused.
- A user can inspect “What will the next agent receive?”, remove/redact items, disable context carryover, set retention and delete derived entries.
- Default first release: **deterministic task working context**, no model call to summarize each request; no global personal-memory graph or consumer chat.

### Implementation proposal (reuse existing engine)

Extend `tasks.rs`, `checks.rs`, `isolation.rs`, `agents.rs` and the existing SQLite store with *new sequential migrations* (do not guess a migration number):

```text
task_context_events(
  id, task_id, attempt_id?, workspace_id, sequence, event_kind,
  source_kind, source_id?, content_ref?, content_hash,
  evidence_state, privacy_class, created_at, supersedes_id?
)
task_context_checkpoints(
  id, task_id, source_max_sequence, base_commit_sha,
  constraints_json, findings_json, files_json, checks_json,
  open_questions_json, next_actions_json, version, created_at
)
task_handoff_packages(
  id, task_id, from_attempt_id?, to_attempt_id, checkpoint_id,
  recipient_agent, recipient_model?, privacy_policy_version,
  source_ids_json, token_budget, estimated_tokens, payload_hash,
  payload_ref, delivery_status, created_at
)
task_context_audit(
  id, task_id, action, actor_kind, source_id?, occurred_at
)
```

The schema is illustrative. Use foreign keys, cascade/delete semantics, size limits, indexes, provenance and migrations consistent with `docs/SCHEMA.md` and the current schema; do not duplicate task/usage tables unnecessarily. For sensitive raw tool logs, retain redacted references with configurable retention and size caps; keep credentials in OS vault.

**Context construction order:** (1) original user objective and explicit constraints; (2) active approved checks and last verified failures; (3) base commit/worktree identity and path hashes; (4) confirmed findings + non-repeated prior attempts; (5) relevant recent evidence; (6) unresolved questions; (7) bounded source pointers. Fit a provider/agent-specific token budget, reserve output, and track omitted sections. **No hidden chain-of-thought or proprietary provider state.** If the next agent cannot receive necessary context/tool types, report incompatibility rather than silently losing critical facts.

First use the deterministic check/audit data Pygg already owns; add SQLite FTS5 retrieval for **task-scoped, user-authorized textual evidence** only if simple exact retrieval cannot fit the budget. Embeddings, memory-fact extraction, temporal graphs and general chat storage are not v1.

**Example handoff:**

```json
{
  "task_id": "t-123",
  "base_commit": "sha256-or-git-sha",
  "objective": "Fix retry and timeout semantics",
  "requirements": [{"text": "No duplicate paid calls", "source": "owner/task"}],
  "verified": [{"check": "test_retry", "result": "failed", "source": "check-event-8"}],
  "observed": [{"file": "src/router.rs", "finding": "timeout branch bypasses guard", "source": "tool-event-13"}],
  "unverified_hypotheses": [{"text": "lock release races a stream", "source": "agent-event-16"}],
  "previous_attempt": {"agent": "local", "discarded": true, "changes_present_in_new_worktree": false},
  "next_action": "Investigate the guard, then rerun approved checks",
  "omitted_context": ["large-redacted-build-log"]
}
```

**Feasibility:** high for task-owned checkpoints; low for reconstructing arbitrary Claude Code/Codex private conversation state because official CLIs may not expose it. **Risk:** stale or poisoned summaries, prompts hidden in tool outputs, token overhead and leaking workspace content. **Gate:** deterministic source verification, untrusted-output markers, user controls and measured value.

**Acceptance tests:** two-step task in different agent/model runtimes; next agent receives verified failing check and relevant file identifiers; no prior speculative claim treated as a decision; no prior worktree edits magically transferred; cross-workspace access blocked; deleted entries never included; unchanged task succeeds without memory enabled; accounting includes handoff token cost; reproducible comparison against current bounded failure tail.

## 3. PV-02 — Optional cross-client, cross-model context handoff (not silent gateway injection)

### Why and boundary

The original desire to change Claude/GPT/Gemini/Qwen while keeping a long thread **cannot be fulfilled by a transparent API proxy alone**. Claude Code, Codex and other IDE clients own their prompts, tool state, local context and hidden reasoning. The gateway cannot safely infer or rewrite that state; cache blobs are provider-specific.

### Feasible narrow design

- Expose **explicit opt-in MCP/SDK operations** for authorized clients, for example `pygg_context_export`, `pygg_context_preview` and `pygg_context_import`, **only if the client explicitly supplies portable visible messages/task facts**. The names are design examples; no currently published tool is implied.
- Store an export as a versioned, provider-neutral **Context Envelope** with source app/version, user confirmation, workspace, attachments by permissioned reference, tool-result snapshots and warnings about nonportable content.
- Import is **suggested context for the receiving agent**, not an assertion that its hidden conversation was transferred. The receiving client controls whether/where it inserts it; Pygg does not modify opaque client history.
- Keep authority boundaries: no API to fetch passwords, hidden reasoning, proprietary server-side cache, private OAuth sessions, or unapproved cross-workspace data. No universal transcript scraping or clipboard interception.
- A single `task_context` MCP read call is a safer initial API than attempting consumer-wide seamless chat history.

**Feasibility:** medium where clients expose supported integrations; **not generally feasible** with arbitrary third-party closed-source clients. **Gate:** a two-client test with explicit export/import and visible field-loss report; unchanged behavior for clients that do not support it. **Decision requested:** Should it be limited to tasks, or should the owner explicitly revisit ADR-028 to support owned chat later? Claude cannot assume the latter.

## 4. PV-03 — Cache-aware Stay / Switch / Escalate

### Why

A model priced lower per input token may cost more after a switch because the new model cannot reuse the old provider's prefix cache, receives a rehydrated context and may require more retries or validation. Pygg already has explicit session affinity and cache usage fields; the missing component is an **economically justified switching decision across safe task boundaries**.

### Design

- Compute **incremental expected cost**, not an opaque global “smart score”: inference input/output, measured cache read/write, context reconstruction, summary/retrieval, retries, validation, and optionally local runtime/latency.
- Cache observations are keyed by **provider + account/connection + model + session/prefix epoch** and expiration/TTL where known. Unknown cache effects stay Unknown, **not zero**.
- **Stay** during the same external client's active attempt/stream/tool call. In task cascades, **Switch** only between attempts when checks failed or a permissible new stage begins; factor in the next stage's capabilities and budget.
- Apply conservative **hysteresis** (do not switch for marginal predicted benefit); track policy version, estimation confidence and rationale. No cross-provider KV-cache transfer claims.
- Phase 1: **shadow score only** and record `would_stay/would_switch` decisions without additional inference. Phase 2: compare baseline static cascade versus affinity-aware decisions on a fixed corpus. Only then opt-in execution.

**Feasibility:** medium with existing ledger/cache/session evidence; quality estimation and future-turn prediction are uncertain. **Acceptance:** zero mid-attempt swaps, explanation of cache misses, no new unapproved paid call, measurable cost-per-kept-task improvements or a recorded negative result and deferral.

## 5. PV-04 — npm/npx distribution and first-class headless CLI

### Why

Users should be able to install/run Pygg on a workstation or CI/dev machine without a Tauri GUI. A single package enables reproducible IDE/agent setup, developer documentation, integration tests and broad adoption. Existing `gateway serve` and `Pygg.exe mcp` are pieces, **not** a published npm distribution.

### Target UX (names are placeholders pending registry verification)

```sh
# Names/package ownership MUST be verified before publication:
npx <verified-pygg-package> doctor
npx <verified-pygg-package> start --port 8741
npm install -g <verified-pygg-package>
pygg status
pygg providers list
pygg models list
pygg integrations list
pygg integrations setup opencode --scope workspace --dry-run
pygg integrations test opencode
pygg integrations restore opencode
pygg usage show
pygg config backup
```

`pygg` is the proposed public command; old `str`/`smarttaskrouter` names remain technical aliases only if migration is necessary. **Do not assume `pygg` or an npm scope is available or controlled by this project.**

### Technical architecture

- **One Rust gateway/task engine and SQLite schema** shared by Desktop, native headless binary, MCP and the Node launcher. No parallel JavaScript gateway; Node performs version/platform selection, executable invocation and useful install diagnostics only.
- Publish platform-specific pinned artifacts from reproducible CI (Windows x64/arm64, macOS x64/arm64, Linux x64/arm64 only where actually built/tested); use optional platform packages or a checksum-verified download. Verify binary SHA-256, release tag and owner signing/provenance before execution. Provide offline/air-gapped install path or explicit failure; no surprise remote code in postinstall.
- Default **user-scoped data directory**, reuse/migrate safely, one-store owner lock (existing), CLI status detects an already-running Desktop instance and **connects** instead of racing it; `start` must never fork competing gateways on the same DB.
- `npx ... start` lifetime is foreground and ends with Ctrl+C unless explicitly installed as a service. No silent autostart, telemetry or config rewrite. Provider secrets remain in OS credential storage or explicitly documented headless env input, never in command arguments, npm logs or project config backups.
- `doctor` verifies OS architecture, binary/version, data-dir lock, port, keychain, model availability (when permitted), MCP handshake and integration compatibility without reading unrelated tokens.
- Compatibility rules: CLI/desktop schema migrations and version skew, signed binary updates, rollback on incompatible format, Windows npm shim behavior, clean-machine tests.

**Feasibility:** high on Windows x64 with current Rust/Tauri code; other platforms require CI/test/signing. **Acceptance:** clean-machine install, no GUI dependency, one-store contention/handoff, secure checksum test and tamper rejection, no credentials in logs, uninstall/reinstall preserves user data by explicit policy.

## 6. PV-05 — Safe IDE / Agent Integrations Hub

### Why

Pygg already provides snippets, MCP and local task APIs, but copying endpoint/header/token snippets across Claude Code, Codex, OpenCode and other clients is error-prone. The product promise is **one flow, many agents**, requiring a trustworthy setup surface.

### Design

- Registry of supported clients with **version ranges, protocols (OpenAI, Anthropic, MCP), capabilities (tools, streaming, images, thinking, session headers), setup method, detected config path, verification date and tested result**.
- In UI and CLI: `Detect → Preview config diff → Explain scope → Backup → Apply with user consent → Handshake/contract test → Roll back`. Each step independent; user can adopt only one integration.
- Prefer **workspace-local setup** by default; global changes require explicit choice. Preserve unknown config keys/comments where feasible; refuse unsafe rewrite if parsing is lossy.
- Generate a **scoped client token** with least privileges (task-start permission only when needed, provider/connection allowlist and privacy policy); never store raw tokens in exported backup. Do not touch the Agent's own sign-in.
- Native integration: existing `Pygg.exe mcp` and `pygg-mcp` tools; endpoint-style client integrations require protocol parity as documented. Subscription-backed external CLI agents run through their own official sign-in, **not** a subscription proxy.
- Test matrix: initial Claude Code, Codex CLI, OpenCode; others only after explicit version/capability tests. Keep examples for VS Code-based clients and Aider as **unverified** until tested.

**Feasibility:** high for first three clients; editors and versions vary. **Acceptance:** setup and restore are idempotent, backup is lossless, failing handshake never leaves broken config, no sensitive token printed or copied to backups, tests cover JSON/TOML and Windows paths.

## 7. PV-06 — Per-account/model entitlements and quota intelligence

### Why

An entry marked Free or Subscription is *not* proof of unlimited present availability. Shared organization limits, expiration, per-minute requests/tokens and monthly quota affect real routing. N2's Qwen subscription arm used hundreds of quota requests and much more time, so `$0 metered` is not `zero effective cost`.

### Design

- Data belongs to **connection/account × exact model × quota scope**, not just provider: `billing_tier`, `auth_method`, `quota_kind`, `limit`, `remaining`, `unit`, `reset_at`, `expiration`, `shared_group`, `data_terms`, `source`, `observed_at` and confidence. Preserve Unknown as Unknown.
- Distinguish **model is potentially free** (public catalog) from **this authenticated connection has usable free entitlement now** (provider/API response and account-specific evidence).
- Fetch through **official supported quota/rate-limit APIs or response headers**, with backoff, no login scraping, no invented quota percentages. Let user enter self-reported limits marked as such; they are not equivalent to verified provider data.
- Reserve when units and reset windows are reliable; otherwise use conservative rate/concurrency limits, cooldowns and user warnings. Enforce shared quota groups atomically to avoid multiplying one allowance across keys/models.
- Present separate economics: known API money, subscription quota/time, local GPU/electricity estimates when user supplies data, and unknown values; do not convert subscription requests directly into USD.

**Feasibility:** medium for providers with published APIs; low for opaque subscription Agent CLIs. **Acceptance:** shared-group 429 test, quota reset crossing, unknown remaining handling, no false “free unlimited”, reconciliation accuracy, no quota evasion.

## 8. PV-07 — Authorized Account Pools (API keys/token plans only)

### Why

Multiple explicitly authorized API connections may differ in health, allowed models, reset time or price. Safe account-level failover can improve availability and reduce spend **when terms allow**, but can also violate quotas or break cache/session state.

### Design

- Explicit pool membership and owner confirmation for **API-key or official token-plan connections only**. Never pool personal Claude Code/Codex/other Agent login sessions or share their subscription credentials. Never rotate accounts/IPs to avoid a ban, 403, contractual rate limit or shared organization quota.
- Supported first policies: `pinned`, `failover_only`, `fill_first`, `sticky_round_robin` for **new independent sessions/tasks**, not arbitrary turns. Weighted/cost-aware rotation stays experimental until quotas can be measured.
- Eligibility first: client/workspace allowlists, provider terms/data-use, verified model capabilities, quota-group health, limits, budget, and session pin. Account selection plus reservation occurs transactionally.
- Persist selected account, model, session ID, quota group, policy snapshot and error scope in ledger; preserve cache affinity. Before a retry, check idempotency/replay status and whether any provider output was emitted.
- Exclude `401/403` evasion; respect `429/Retry-After`; no duplicate charged attempts after ambiguous execution without explicit user policy.

**Feasibility:** medium technically but provider-specific authorization is the primary gate. **Acceptance:** two user-owned API keys with independent verified quotas, shared-org quota negative case, concurrent reservation, sticky sessions, 401/403/429 safety and ledger exactness.

## 9. PV-08 — Task-aware policy engine, shadow routing and quality-adjusted choices

### Why

`auto` is request-level, whereas users judge whether a **whole task** was completed. Fixed cheap-first cascades can waste time on unavailable/inadequate models (seen in N2) even if nominal inference is free. Pygg's differentiated value is choosing the **cheapest verified path to an acceptable result**, not the cheapest next token.

### Design

- Policy object per task/workspace: declared category, risk/privacy, deterministic checks, hard minimum capabilities, preferred latency budget, quality goal, eligible agents, usage/quota, max attempts, paid approval cap.
- Apply **hard constraints before scoring**. In v1 the user chooses the policy; deterministic metadata/heuristics and check availability choose a starting rung. No default per-request LLM classifier and no fabricated “quality score”.
- **No-check tasks:** do not label a model answer “verified”; require manual acceptance or route directly to an appropriately capable stage. A failure of checks may escalate, but never escalate above the budget/approval/policy snapshot.
- Shadow decision per task: `would_start_with`, `would_escalate_to`, excluded reasons, predicted total cost/confidence **without sending additional provider requests**. Evaluate on completed tasks using `checks_passed`, `result_kept`, retries, rework and latency; insufficient sample falls back to rule-based.
- Only after controlled benchmark: optional task-category and complexity scoring, calibrated historical success probability and route selection by expected **cost per accepted result**. Guard against learning from leaked/hardcoded benchmarks or unjustified extrapolation to unknown repositories.
- If the external agent controls its own internal loop, Pygg only governs **task/attempt boundaries it owns**, not undisclosed model changes within the external agent.

**Feasibility:** high for policy and shadow audit using current task engine; medium/uncertain for general predictive optimization. **Acceptance:** fixed-policy versus shadow comparison, no paid request from shadow, measurable improvement on held-out real repositories, quality non-inferiority confidence bounds, correct no-check behavior, reversible feature flag.

## 10. PV-09 — Reversible context compression and retrieval (opt-in)

### Why

Long logs and repeated tool outputs waste tokens and can overwhelm the next Agent. Pygg can compress **its own task context packets**, but touching opaque external prompts would break native cache and alter the client's semantics.

### Design

- **Only Pygg-owned context by default:** deterministic exact-duplicate blocks, ANSI/progress noise removal for views, bounded excerpts with an explicit omission marker and immutable raw content reference.
- Preserve exit codes, stack traces, failing assertions, exact file paths, protected code/test snippets, recent user constraints, tool invocation IDs and safety-relevant errors. Never summarize away contradictory or failed-check evidence.
- Stable source hashes, rule/version in a `context_package` log, token estimates and actual usage where available. Compression/truncation must not silently change a cached prefix of an external client.
- Shadow mode computes estimated context differences **without modifying requests**. Opt-in only for tested categories. LLM summarization is a separate paid action with a tracked token/cost budget and provenance.

**Feasibility:** high for deterministic owned logs, low/risky for arbitrary prompt rewriting. **Acceptance:** perfect reconstruction from raw references, no loss of protected fields, cross-workspace isolation, no cache-parity regression, lower measured total cost per kept result or documented rejection.

## 11. PV-10 — Curated Skills & Prompts Hub (optional)

### Why

Developer users reuse skills/prompts for coding, testing and review. A curated, permission-aware catalog can improve task success and reduce repeated expensive context, but downloading arbitrary “skills” is a code-execution and prompt-injection risk.

### Design

- Discovery and preview only at first: source repository URL, commit SHA, license, maintainer, update date, declared capabilities, required permissions and file manifest. GitHub repository text is **untrusted content**, not instructions to Pygg's security controls.
- Pin every install to a reviewed commit/hash, inspect scripts and dependencies, quarantine high-risk files, require explicit install and enable confirmation, provide clean uninstall/rollback.
- Target existing agents' **official skills/config locations** only where version-tested and authorized. No automatic execution of downloaded scripts, no silent prompt injection into unrelated agent tasks.
- Progressive loading: put references/short descriptors in context and load full skill only when selected by the agent/user within permission and token budget.
- Security gate: malicious prompt/skill fixtures, executable payload rejection, permission escalation attempts, revoked source and version drift.

**Feasibility:** medium; catalog UI is straightforward, trustworthy execution and version portability are harder. **Priority:** after reliable integrations and policy/permissions, not in the critical launch path.

## 12. PV-11 — Independent, reproducible evaluation and product-evidence loop

### Why

Routing and context memory are only valuable if task outcomes improve. The current N2 report is meaningful but limited; it is **not a benchmark for shared-memory handoff, npm install success or cache-affinity savings**.

### Required evidence work

1. Finish the N2 report's remaining placeholders (e.g. `FULL_TABLE`, `CASCADE_FREE_*`) using the true run artifacts, or mark that arm **incomplete/unavailable**; never fabricate figures.
2. Preserve **sanitized** per-run raw records or a reproducible artifact bundle with cases, hashes, model/agent/client versions, prompts where safe, price snapshot, timeouts, ledger rows, check results and random seeds when possible. Do not commit credentials, private source or sensitive upstream outputs.
3. Expand from small Node fixtures to nontrivial real open-source repositories (TypeScript, Rust, Python), multiple agent CLIs, longer context, failed tools, sensitive-workspace denial and high-cost tasks. Separate training/development and held-out test cases.
4. Compare **fixed premium**, **fixed capable cheap**, **static combo**, **Auto**, **cascade**, **task handoff**, **cache affinity** and **compression**, each against its corresponding baseline, not a single global 88% claim.
5. Report **cost per verified/kept result**, success intervals, p50/p95 latency, retry rate, rework minutes, context omission/continuity errors, paid approval incidents, quota usage and Unknown proportion. Use paired estimates and honest confidence intervals, without claiming uniform effects from 12 tasks.
6. Require explicit owner-approved spend caps for any new paid experiments. No automatic paid benchmarking from installing npm or enabling a feature.

**Feasibility:** high using the existing benchmark harness, ledger and `tasks_quality` metrics; multi-provider results depend on budget and accounts. **Acceptance:** independent replay from a documented sanitized bundle, no missing placeholders in the published report, and reproducible feature on/off comparisons that can fail honestly.

## 13. Suggested architecture, dependencies and compatibility

```text
IDE / official Agent CLI
       |  MCP task API (existing)         external chat API (opaque)
       v                                  |
 Pygg Task Engine ---- worktrees/checks   |  Gateway: metadata + policy + ledger
       |                                  |  (NO silent prompt rewrite)
       +--> Task Context Event Log <------+- explicit client export ONLY (future opt-in)
       |        | 
       |   Evidence / checkpoints
       |        |
       +--> Context Package Builder (new, scope + budget + provenance)
       |        |
       +--> Agent/Model Selection at safe task boundaries
       |        +--> Shadow cost/quality/cache score (new)
       |        +--> Current auto / combo / provider policy (existing)
       v
  Approved Agent Attempt
       |
  Checks -> task outcome -> ledger / quality feedback -> next handoff
       ^
 Desktop Pygg + native CLI + npm launcher (same Rust core, same SQLite DB)
```

**Reuse:** existing `tasks.rs`, `agents.rs`, `gateway/mod.rs`, `gateway/auto.rs`, `gateway/store.rs`, `gateway/task_api.rs`, `mcp.rs`, `checks.rs`, `isolation.rs`, `pricing.rs`, `gateway/tiers.rs`, `docs/SCHEMA.md`, `scripts/schema-reference.py`. Names are indicative; verify the current module layout before coding. Avoid a second server, standalone memory daemon, new vector database, cloud relay or browser session bridge.

**Dependency path (not an automatic roadmap rewrite):**

- **Gate 0 / immediate:** PV-11 honest report completion; N6 real paid approval flow and N9 clean-Windows installer check; fix safety issues found in that validation.
- **Track D2 / near-term:** PV-05 integration diff/backup/restore; PV-04 native headless CLI then verified npm launcher; add versioned contract tests.
- **Track Memory pilot / owner decision first:** PV-01 minimal task-evidence handoff, feature-flagged, compared against present bounded failure summary. PV-02 is conditional and must not be rolled into PV-01 by default.
- **Track Optimization experiment:** PV-03 cache-affinity shadow, PV-08 task-aware shadow, PV-09 reversible compression; use PV-11 quality data before enabling any by default.
- **After evidence and provider permission:** PV-06 quota intelligence; PV-07 explicitly allowed account pools; PV-10 curated skills.
- **Still deferred:** consumer Auto Chat, generic cross-client full conversational memory, vector/knowledge graphs, automatic LLM classifier, aggressive semantic compaction, speculative fan-out, anything using unofficial sign-in extraction.

### Compatibility invariants

1. Existing gateway, CLI and desktop serve the **same SQLite store** safely with the current singleton owner, migrations and data directory.
2. Session affinity and native-format prompt/cache parity remain unchanged unless an explicit, tested adapter contract permits change.
3. Every extra cost source (summary, embedding, retrieval, handoff, retry, validation, local usage) is attributed; Unknown never $0 and subscription quota never unqualified “free money”.
4. No account rotation to evade terms, no unauthorised key use and no policy bypass.
5. No active tool call/stream/agent attempt is switched to another model or provider.
6. Workspace boundaries, approval caps and explicit client permissions outrank any optimization.
7. No marketing claim is upgraded from `fake-tested` or a small corpus to general production evidence.

## 14. Requested Claude counter-review — structured, falsifiable response

**Do not implement or edit ROADMAP/ADRs solely from this proposal.** First produce a response in `docs/reviews/2026-11-12-original-vision-gaps-response.md`, preserving this RFC as a historical proposal. For each PV-01…PV-11, provide:

| ID | Accept / Partial / Reject / Defer | Current evidence + precise source | ADR conflict / owner decision | Simplest valid design | New tests + release gate | Effort/risk | Proposed roadmap slot |
| --- | --- | --- | --- | --- | --- | --- | --- |
| PV-01 | | | | | | | |
| PV-02 | | | | | | | |
| PV-03 | | | | | | | |
| PV-04 | | | | | | | |
| PV-05 | | | | | | | |
| PV-06 | | | | | | | |
| PV-07 | | | | | | | |
| PV-08 | | | | | | | |
| PV-09 | | | | | | | |
| PV-10 | | | | | | | |
| PV-11 | | | | | | | |

Answer **these decision questions explicitly**:

1. Can **task-scoped shared working context** be built without reversing ADR-028's ban on owned Auto Chat? Where is the precise owned/opaque boundary? What *cannot* be transferred between Claude Code, Codex, Qwen and GPT?
2. Is the current “failed command + last 1,500 characters” a meaningful bottleneck? Propose an A/B benchmark that could **disprove** the value of richer handoff; avoid shipping memory purely because it sounds advanced.
3. Which npm package and executable names can actually be controlled? How will a launcher use the **same Rust core**, release verified binaries and coexist with the installed Pygg Desktop database? What is the realistic minimum supported platform matrix?
4. How will the integration hub edit user configs **reversibly and with consent** while preserving native agent logins? Name official version-tested integration targets.
5. Which subscription/free account quota facts are genuinely obtainable from APIs and which must stay Unknown? Which account-pool modes are provider-permitted?
6. Which parts of task-aware routing and cache-affinity could reduce **cost per kept result** rather than just raw tokens? What data size and test set must exist before enabling dynamic scoring?
7. What is the smallest commercially meaningful release? Distinguish **must-have**, **measurable experiment**, and **defer/reject**, and give a staged migration/test plan without replacing the existing product.
8. Identify any redundant or already-built items **by exact code and test**, rather than re-implementing them; cite precise conflicts with accepted ADRs.

**Counter-review quality bar:** address failure modes, privacy, provider ToS, subscription quota economics, startup/shutdown concurrency, migration rollback, Windows-first packaging, reproducibility and user-visible UX. Partial acceptance with a smaller safer option is preferred to a generic “build everything”. Keep implementation evidence separate from proposed behavior. **Owner approves any ADR reversal, scope expansion or new paid testing.**

## Related existing documents

- [docs index](../README.md), [ROADMAP](../ROADMAP.md), [Implementation Status](../IMPLEMENTATION-STATUS.md), [Decisions](../DECISIONS.md), [Architecture](../ARCHITECTURE.md), [Security](../SECURITY.md), [Agents and task engine](../AGENTS.md).
- [Withdrawn Universal Shared Memory proposal](UNIVERSAL_SHARED_MEMORY.md) — source of the original memory concept, **not active authority**.
- [CLI / IDE / npm / tiers reference](CLI_IDE_NPM_PROVIDER_TIERS.md), [authorized account rotation reference](ACCOUNT_API_ROTATION.md), [OmniRoute features assessment](OMNIROUTE_FEATURE_RESEARCH.md), [historical functional plan](SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md).
- [N2 measured benchmark](../../benchmark/reports/2026-11-12-n2-savings.md) and [2026-11-12 owner progress report](../reports/2026-11-12-progress.md).

**End of RFC. This file makes no code-delivery, supported npm package, seamless external-chat memory, universal provider compatibility or generalized savings claim.**
