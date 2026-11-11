# Pygg — Open-source architecture lessons and adoption gates (2026-11-12)

> **Status:** Reference / research, **not an accepted ADR or implementation**.  
> **Scope:** Read publicly documented repositories and selected current implementation files; compare them to Pygg's built Rust/Tauri/SQLite/Agent stack. No dependency imported, no tests run, no provider calls or paid traffic.  
> **Why:** The owner explicitly wants the original shared-context, npm/CLI and routing vision evaluated against well-known open source before adopting Claude's [counter-review](../reviews/2026-11-12-original-vision-gaps-response.md).  
> **Authority:** [docs index](../README.md) → [ROADMAP](../ROADMAP.md) → [Implementation Status](../IMPLEMENTATION-STATUS.md) → [Decisions](../DECISIONS.md). This research does **not** reverse ADR-026, 027 or 028. Compare with the [original-vision RFC](../RFC-2026-11-12-ORIGINAL-VISION-GAPS.md).

## Executive decision: learn patterns, not entire frameworks

**Read in depth first:** **OpenCode, LangGraph, Mem0, Letta Code, Aider** for continuity, followed by **LiteLLM, vLLM Semantic Router, 9Router and OmniRoute** for safe routing. Use **Graphiti** as the later-stage temporal-data reference, **Vercel Skills** for npm and multi-agent installation UX, and **Anthropic / VoltAgent skills** for the catalog/skill security model.

All this work is **pattern research**. Pygg already has a local gateway, session pins, owned task attempts, Git worktrees, audit records, ledger and MCP. **Do not transplant** a Python agent platform, a graph DB, a second proxy, an always-on memory service, a login/session bridge or a generic plugin system into its current architecture.

### What was actually verified versus inferred

- **Verified from primary documentation/code:** OpenCode's current compaction algorithm and prompt template; LangGraph's checkpointer/store separation; Mem0 scoped memory operations; Letta Code's active repository and Git-tracked memory; Aider's repo map and token budget; Graphiti's temporal provenance model; LiteLLM's proxy/virtual-key/fallback pattern; vLLM Semantic Router's per-call boundary; 9Router/OmniRoute's self-described provider/route patterns; Vercel Skills' CLI and per-agent installs.
- **Inference for Pygg:** implementation size, best module boundaries, expected cost/quality benefit, migration design, portability and priority. These are **hypotheses, not features or measured results**.
- **Not verified:** commercial compatibility of every upstream feature; exact latest dependency APIs for an import; full upstream source/security reviews; provider permission for any account rotation; performance of copied patterns on Pygg. Recheck license at an exact pinned commit **before copying any code**.

## 1. Source comparison: memory and handoff

| Project / exact source | Specific pattern worth learning | Pygg application | Do **not** copy / caveat | Priority |
| --- | --- | --- | --- | --- |
| **[OpenCode](https://github.com/anomalyco/opencode)** — [compaction docs](https://opencode.ai/docs/compaction), [source](https://github.com/anomalyco/opencode/blob/dev/packages/core/src/session/compaction.ts) | Versioned, structured summary with Objective, Requirements, Decisions, Completed/Active/Blocked, Next Move, Relevant Files; preserves recent verbatim context; compacts at a context threshold, keeping raw history. Auto compaction has a token budget; native provider checkpoint is **not portable** across models. | Build a **bounded, source-linked Task Handoff Package** at an attempt boundary. Include prior failures, check evidence, constraints and relevant files. Record exact package and omitted sections. Gate summary by token budget. | Do not read OpenCode's hidden state or inject compaction into its unrelated conversations; no model per-turn rewriting or claiming provider cache transfer. | **H1 reference 1** |
| **[LangGraph](https://github.com/langchain-ai/langgraph)** — [persistence docs](https://github.com/langchain-ai/docs/blob/main/src/oss/langgraph/persistence.mdx) | **Checkpoint** for in-flight thread/workflow state versus **Store** for durable cross-thread data; resume, human approval, distinct scopes; persistent checkpointer rather than RAM-only memory. | Treat task/attempt/check state as durable checkpoints and *retrievable evidence*, separate from optional project memory. Reuse existing attempts and audit instead of introducing an entirely new orchestration runtime. | Its graph scheduler and Python/Postgres stack are not needed; read-only pending provider work and Git state cannot always be resumed. | **H1 reference 2** |
| **[Mem0](https://github.com/mem0ai/mem0)** — [search](https://github.com/mem0ai/mem0/blob/main/docs/core-concepts/memory-operations/search.mdx), [scope/filter API](https://github.com/mem0ai/mem0/blob/main/docs/open-source/features/rest-api.mdx) | Memory **add/search/update/delete/history**, scoped by user/agent/run, retrieval top-k/filter and metadata. Most important: explicit scope, mutation history and deletion. | Design *optional* task/workspace-owned retrieval, scoped by task/workspace/client and only verified/authorized evidence. Provenance and deletion before embeddings. | Its default semantic-memory extraction requires more inference/storage and may turn hypotheses into facts. No new vector database by default. | **H2 reference 1; H1 scoping** |
| **[Letta Code](https://github.com/letta-ai/letta-code)** (active codebase; [old Letta repo](https://github.com/letta-ai/letta) now directs to it) | **MemFS**: versioned memory/context files tracked with Git; visible persistence, query across agent messages, memory and skills boundaries. The agent may evolve its own memory over time; SDK/server are optional integrations. | For Pygg-owned task context, preserve a human-readable, versioned **evidence artifact** (or at minimum hashes and origins), checkpoint/rollback and inspect what the next Agent received. | Do **not** port identity, auto-dreaming, cross-user memory server or self-editing harness; no opaque Claude/Codex state extraction. Old Letta V1/archive docs are not the current implementation. | **H2 reference 2** |
| **[Aider](https://github.com/Aider-AI/aider)** — [repo map](https://aider.chat/docs/repomap.html) | Repository map of significant symbols, dependencies, files, **ranked and fitted to a token budget** rather than shipping the whole codebase. | Add on-demand **source-relevant file/symbol pointers** to the task handoff, preferring checks/changed paths, with stable Git base revision. Start with filename/hash + approved check, then consider Tree-sitter/symbol extraction only if A/B proves value. | Avoid blindly inserting broad repo maps into every agent prompt; existing agent may already build one, doubling context cost. | **H1 experiment after baseline** |
| **[Graphiti](https://github.com/getzep/graphiti)** — [temporal provenance](https://github.com/getzep/graphiti) | Temporal facts with validity intervals, supersession and episodic/raw-source provenance; hybrid retrieval across graph, keyword and vectors. | Borrow only **fact supersession and source-backed decisions** as a possible future small schema. “Earlier attempt assumed X, check disproved it” must not become durable truth. | Its graph database, extraction calls and Python infrastructure are disproportionate to a local SQLite task handoff. No H1 Graphiti dependency. | **P2 research only** |

**Proposed H1 contract, derived from those patterns:**

```json
{
  "schema": "pygg.task-handoff.v1",
  "task_id": "task-id",
  "base_commit": "exact-git-sha",
  "requirements": [{"text": "Preserve existing API contract", "source": "user-task"}],
  "verified_checks": [{"command": "cargo test", "status": "failed", "source_id": "check-id"}],
  "prior_attempts": [{"agent": "opencode", "status": "discarded", "changed_paths": ["src/api.rs"]}],
  "hypotheses": [{"text": "Possible parsing bug", "state": "unverified", "source_id": "attempt-id"}],
  "next_move": "Inspect the reported test failure",
  "omitted": [{"kind": "tool-output", "reason": "redacted-or-token-limit"}]
}
```

The schema is illustrative. **A new worktree starts from the original base commit.** Prior attempt edits do **not** magically exist there. Raw/check events, authenticated workspace scopes and source hashes remain authoritative; summaries are derived and versioned. The receiving agent's official CLI controls its own private conversation/tool state. Only **explicit** task/MCP context transport is permitted; no silent injection by the gateway.

### H1 implementation as the smallest testable slice

- **Reuse first:** `tasks.rs` attempts and `check_results_json`, `summary`, `log_tail`, `escalation_reason`, `base_revision`, the active policy snapshot, Git diff paths and approved checks. The current previous-failure tail is 1,500 characters. Determine the minimum new storage (likely one handoff snapshot field) after code review, not a suite of new memory tables.
- **Compose at a safe attempt boundary:** deterministic sections, explicit evidence labels, source IDs, token cap, known privacy scope, raw references, dedup for repeated check output, exact omission warnings. No LLM extraction by default.
- **Inspect before use:** preview handoff in Task detail and `pygg_task_status`, allow “disable” and optional user note, and track what was actually delivered.
- **Experiment with feature flag off:** compare (a) original task alone, (b) current failure tail, (c) H1 handoff; include hard cases where earlier attempts actually discovered useful evidence. Compare tokens, success, time and repeated failures by **paired tasks**. Do not infer from the N2 results alone, which did not test richer handoff.
- **Abort/rollback:** if omitted facts, prompt-injection contamination, cross-workspace data or higher cost per kept result occurs without quality gain, H1 stays off.

**H2 decision gate:** Only if H1 helps and actual tasks need retrieval after multiple attempts, test task-scoped indexed evidence/FTS5 and explicit export. Do **not** reopen consumer Auto Chat or globally shared personal memory under ADR-028 by implication.

## 2. Source comparison: routing, cost, quotas and policies

| Project | Pattern to inspect | What Pygg should adopt | What to reject |
| --- | --- | --- | --- |
| **[LiteLLM](https://github.com/BerriAI/litellm)** — [budget fallbacks](https://github.com/BerriAI/litellm-docs/blob/main/docs/proxy/budget_fallbacks.md) | Unified providers, virtual keys, per-model budgets, fallback chains, router health and cost provenance | Compare error taxonomy, capability matrix, budget fallback behavior, price provenance and realistic multi-provider tests with Pygg's Rust gateway. Record any mismatch as fixtures; do **not** bypass Pygg's ledger or replay-safety contract. | Running a second LiteLLM proxy inside Pygg; accepting permissive enterprise assumptions. **License check required** on exact module, since repository has separately licensed enterprise code. |
| **[vLLM Semantic Router](https://github.com/vllm-project/semantic-router)** — [design](https://github.com/vllm-project/semantic-router/blob/main/website/docs/intro.md) | **Decision layer** separate from harness; policy signals for task type, modality, context, latency, privacy and budget; replay/evaluation | Keep request routing in `gateway/auto.rs` and task-level routing in `tasks.rs`; add an explainable **policy evaluation harness** rather than a black-box model classifier. Evaluate by cost per verified result. | Full Envoy/service infrastructure, embedding classifier or heavy model server for a local app before benefit is measured. |
| **[9Router](https://github.com/decolua/9router)** | Developer-first provider/CLI integrations, combo fallback, free-tier discoverability and multi-format routing | UX/workflow reference for **D2 Integrations Hub**: identify client → preview changes → consent → test → restore. Compare quota-source and per-connection state without trusting marketing savings. | Cookie/session/OAuth bridging, blanket “unlimited free”, automatic quota evasion or provider ToS violations. |
| **[OmniRoute](https://github.com/cheaper-inference/OmniRoute)** — [existing Pygg decision](../OMNIROUTE_FEATURE_RESEARCH.md) | Dynamic candidate factory, quota-group logic, RTK-style compression, safe fallback, client compatibility, free-tier catalog | Reuse its reference patterns **selectively**: per-account eligibility, route-exclusion reasons, quota-source verification, deterministic *owned-log-only* compression and client integration matrix. Existing Pygg review already covers it; **do not duplicate** the whole assessment. | Its advertised large percentage token savings without reproducing them in Pygg, automatic credential rotation, broad account/session bridges, speculative retries, unverified free-provider catalog as guaranteed entitlement. |

### Boundaries that are more important than “smarter routing”

1. The **Agent executor** owns a task attempt's tools/worktree; the **Gateway** chooses an eligible per-request model. These are separate control loops.
2. Never change a model **inside** a live tool call, partially emitted stream or opaque external agent attempt.
3. Quality and cost live at different levels: **input/output cost per request**, **quota/latency per account**, and **cost/time/quality per kept task**. A subscription's marginal metered USD = 0 does not mean that its quota and time are free.
4. Routing policies must first enforce approved privacy, credentials, capabilities, explicit allowlists, budget, shared quota and replay safety; ranking happens **only after eligibility**.
5. Candidate-scoring models/classifiers are not justified solely by their popularity upstream. Start with benchmark replay, deterministic shadow logging and calibrated measurements where an actual alternative outcome is available.

## 3. Source comparison: npm, integrations and skills

| Project / source | Concrete feature | Pygg lesson | Gate |
| --- | --- | --- | --- |
| **[Letta Code npm](https://github.com/letta-ai/letta-code)** | `npm install -g @letta-ai/letta-code` and `letta server` provide a CLI/server workflow with shared agent system | A clear **single public `pygg` command** with `serve`, `status`, `mcp`, `doctor`, `integrations`; one Rust core for desktop and headless, no second Node gateway. | Verify public npm package/scope ownership, release/asset hashes, Windows native binary, clean-machine support. |
| **[Vercel Skills](https://github.com/vercel-labs/skills)** — [CLI](https://github.com/vercel-labs/skills#install-a-skill) | `npx skills add`, `find`, `list`, `check`, `remove`; project-scoped installs by default and explicit `-g` for global | Copy the **interaction design pattern**, not the code: preview first, workspace default, explicit global opt-in, config backup, actionable doctor/test/restore. For skills, audit `SKILL.md` metadata and dependencies before any install. | Start with version-tested Claude Code / Codex / OpenCode integrations and existing MCP. No unverified agent/IDE support claim. |
| **[Anthropic Agent Skills](https://github.com/anthropics/skills)** | Skill directory/metadata and discipline of task-specific instructions | Catalog metadata and progressive loading reference only. | **Inspect individual license files**; a public repository is not a blanket right to copy prompts/scripts/assets. E.g. [PDF skill LICENSE.txt](https://github.com/anthropics/skills/blob/main/skills/pdf/LICENSE.txt) includes significant reuse restrictions. |
| **[VoltAgent Awesome Agent Skills](https://github.com/VoltAgent/awesome-agent-skills)** | Cross-agent discovery/index of skills from many authors | Discoverability/reference catalog. Track original origin, commit, declared license and trust review *per skill*, rather than downloading everything. | Maintainers explicitly disclaim security audits of individual entries. No auto-executed downloaded scripts; manual consent and rollback. |

**Distribution security:** npm launcher should launch verified platform-native binaries for an explicitly supported target; preserve the current SQLite store ownership lock, upgrade/migration safety, local service boundary, and OS vault secrets. A checksum establishes **integrity only against the trusted expected checksum**; signing adds publisher identity and OS trust. Whether code signing is a *hard prerequisite for a limited developer preview* or only for a public release remains an owner/release decision; document SmartScreen limitations, don't claim unsigned means secure or that signing alone prevents compromise.

## 4. What these sources say about Claude's PV-01/PV-02 reply

Claude's [counter-review](../reviews/2026-11-12-original-vision-gaps-response.md) is correct to **avoid a speculative global memory subsystem** before evidence, and correct that today `pygg_run_task` already transports the user's *explicit task description*.

But that is **not equivalent to portable context with source lineage**:

- **Task dispatch**: the IDE agent tells Pygg to do a task and sees the outcome. **Built.**
- **Bounded error handoff**: Pygg carries a trailing failure excerpt. **Built.**
- **Inspectable, structured task evidence handoff**: multiple attempts' verified checks, files, decisions, provenance, omitted sections, explicit package version. **Not fully built; H1 experiment.**
- **Portable task-context export across compatible external clients**: opt-in and explicitly client-controlled; only visible data that the client sends/receives, with a typed format and field-loss notes. **Not built as a general contract; H2 research, not a launch blocker.**
- **Seamless migration of opaque third-party conversations, tool state, hidden reasoning, provider cache**: **not generally feasible or authorized**; not a Pygg product claim.
- **App-owned consumer Auto Chat / global personal memory**: **withdrawn by ADR-028**, not reauthorized by this study.

Thus the more accurate PV-02 verdict is **“narrow task dispatch built; general portable context contract not built; defer wider scope”**, rather than a blanket “already built”.

### A caution against building memory for its own sake

OpenCode and Letta are **agent harnesses that own conversation/session state**. Pygg's gateway deliberately does **not** own opaque external-client sessions. Their “memory across models” patterns cannot be applied to Pygg's external gateway without changing its contract. **Copy their state-management principles only into the Pygg-owned task engine**, and test a real deficiency first.

## 5. Research-to-implementation work packages for Claude

No work package is automatically approved. Each should produce a short source-backed investigation and tests *before* delivery:

| Research ID | Reference files / docs | Concrete Pygg diff to consider | Evidence to collect | Current priority |
| --- | --- | --- | --- | --- |
| **OSS-01** | OpenCode compaction.ts + docs, LangGraph persistence, Mem0 scope API, Letta MemFS, Aider repo map | Minimal deterministic handoff builder, one stored rendered package, task detail + MCP status preview; feature flag | 3-arm A/B (task alone/current tail/rich handoff), correctness/latency/token and cross-workspace leak tests | H1 experiment |
| **OSS-02** | Vercel Skills setup UX, Letta CLI, official Claude Code/Codex/OpenCode setup docs | Integration version/format matrix, consented config diff/backup/restore, scoped token and active handshake | Idempotent setup, failed config rollback, no credential leakage, one live test/client | D2 first |
| **OSS-03** | LiteLLM budgets, vLLM SR policies, 9Router/OmniRoute candidate lists | Document gaps in auto/task policy, classify eligibility versus ranking and source of quota facts | Fake-provider policy fixtures and held-out benchmark replay; no new paid calls | After evidence gate |
| **OSS-04** | Mem0 scoped search, Graphiti temporal facts, Letta versioned memory | Optional task-only FTS5/provenance/supersession *if H1 succeeds* | Compare FTS5 to exact recall; zero unauthorized cross-workspace retrieval; deletion | H2 conditional |
| **OSS-05** | Letta npm, Vercel `npx skills`, Pygg's existing headless binary | Native `pygg` subcommands + verified npm launcher for a tested OS | Windows x64 clean install, checksum/tamper test, coexistence with Desktop, update rollback | D2 next |
| **OSS-06** | Skills catalogs and individual license notices | Read-only Skills preview first, with pinned upstream references | License and supply-chain review, no code execution on discovery | P2 |

**No code-licence shortcut:** MIT-licensed Pygg may *study* MIT/Apache projects. If porting code, audit the **exact source file, dependencies, license, NOTICE obligations, attribution and provider terms**, record origin/commit in `THIRD_PARTY_NOTICES.md`. For skills/prompts and enterprise subtrees, individual licenses may be more restrictive than the repository banner. Research links ≠ permission to copy.

## 6. Decision questions to resolve before another architecture rejection

1. **Why did OpenCode need versioned, structured compaction beyond the last failure tail?** Which *owned task evidence* of Pygg is actually lost at a model/agent change, and can a hard-case fixture demonstrate that loss?
2. Is the present `attempts`/audit schema enough for a **source-backed package**, or does a single new column/index make provenance reliable? Show raw source references and exact omitted sections, not a generic “summary” field.
3. What is the **smallest bounded context package** that aids a different agent on a discarded-worktree restart without implying its prior edits exist?
4. Which aspects of Mem0/Letta/Graphiti should remain out-of-scope until H1 A/B evidence? State SQLite/FTS5 versus vector/graph cost, privacy and test implications.
5. For PV-02, does current MCP **task dispatch** really satisfy structured **portable context export/import**? If not, defer the latter by name rather than mark it “already built”.
6. For npm, distinguish **technical release integrity**, **Windows publisher trust/signing** and **npm registry ownership**. Can a clearly identified Windows-only preview be evaluated without claiming a public signed production release?
7. With vLLM SR/LiteLLM/OmniRoute as examples, which route policies can be **measured** from Pygg's existing task outcomes without adding a per-request classifier or a second proxy?
8. Which upstream projects are **templates for patterns** versus legitimate code-import candidates, with exact licenses and security review?
9. Update the PV-01/PV-02/PV-04/PV-05/PV-11 prioritization **only after** comparison against built code, accepted ADRs and owner decisions. No invented savings, time estimate or provider compatibility.

**Requested Claude deliverable:** A line/commit-backed **source comparison response**, ideally `docs/reviews/2026-11-12-oss-patterns-response.md`, with Accept/Partial/Reject/Defer for OSS-01…OSS-06, exact source links and a limited test proposal. **Do not implement / revise ROADMAP or ADR-028 just because this reference study exists.**

## Verified source links (retrieved 2026-11-12)

- OpenCode [repository](https://github.com/anomalyco/opencode), [current compaction spec](https://opencode.ai/docs/compaction), [compaction source](https://github.com/anomalyco/opencode/blob/dev/packages/core/src/session/compaction.ts).
- LangGraph [repository](https://github.com/langchain-ai/langgraph), [checkpoint versus store](https://github.com/langchain-ai/docs/blob/main/src/oss/langgraph/persistence.mdx).
- Mem0 [repository](https://github.com/mem0ai/mem0), [search and scopes](https://github.com/mem0ai/mem0/blob/main/docs/core-concepts/memory-operations/search.mdx).
- Letta [old overview](https://github.com/letta-ai/letta), [actively developed Letta Code](https://github.com/letta-ai/letta-code), [agent SDK](https://github.com/letta-ai/letta-agent-sdk).
- Aider [repository](https://github.com/Aider-AI/aider), [repo-map explanation](https://aider.chat/docs/repomap.html).
- Graphiti [repository](https://github.com/getzep/graphiti).
- LiteLLM [repository](https://github.com/BerriAI/litellm), [budget fallbacks](https://github.com/BerriAI/litellm-docs/blob/main/docs/proxy/budget_fallbacks.md).
- vLLM [semantic-router](https://github.com/vllm-project/semantic-router), [routing architecture](https://github.com/vllm-project/semantic-router/blob/main/website/docs/intro.md).
- [9Router](https://github.com/decolua/9router), [OmniRoute](https://github.com/cheaper-inference/OmniRoute).
- [Vercel Skills CLI](https://github.com/vercel-labs/skills), [Anthropic Skills](https://github.com/anthropics/skills), [VoltAgent Skills index](https://github.com/VoltAgent/awesome-agent-skills).

**End.** This file is a research guide with direct-source references and falsifiable proposals; **no production implementation or general performance claim**.
