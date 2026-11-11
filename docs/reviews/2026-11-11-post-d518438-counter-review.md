# Post-d518438 counter-review — cross-process accounting and Free/Local replay safety

**Date:** 2026-11-11  
**Status:** Open technical review for Claude; **not** an accepted ADR or implemented fix.  
**Baseline:** `main` at `d5184386c3ffb786b90a3c7504812454937361b1`.  
**Reviewed:** [follow-up response](2026-11-11-remaining-gateway-risks-response.md), [Implementation Status](../IMPLEMENTATION-STATUS.md), [ROADMAP](../ROADMAP.md), `apps/desktop/src-tauri/src/gateway/{mod,store,policy}.rs`.  
**Method:** source inspection; no new Rust test, multi-process run, provider call or paid benchmark was performed.

## Executive verdict

**Accept the fixes in d518438 as meaningful improvements.** FR-01's per-client connection allowlist, FR-02's default stop on potentially billed 5xx, FR-03's journal plus atomic settle, and FR-04's documentation reconciliation should remain in place. The reported **87 passed / 1 ignored** tests and the real Anthropic two-turn cache parity are acknowledged as reported results; they are not repeated measurements by this reviewer.

Two *different* production contracts remain unresolved. Neither invalidates the previous single-process tests.

## PR-01 — HIGH — Accounting-degraded is process-local, not a global paid-admission barrier

### Direct code evidence

- `gateway/mod.rs::Accounting` stores `pending: HashMap<String, (store::PendingUsage, String)>` inside `Shared` as `Arc<Mutex<Accounting>>`, i.e. scoped to **one process**, not a persistent/shared status.
- `gateway/mod.rs::forward` calls `shared.recover_accounting()` only if **its own** `pending_settlements() > 0`, and checks the same process-local value before sending paid requests.
- `store.rs::open_attempt` atomically checks a client's estimated monthly spend and held reservations. This is useful, but it does **not** check whether *another process* currently has a failed settlement or a global accounting-degraded flag.
- `store.rs::settle` performs usage insert and reservation removal atomically. `settle_interrupted` converts aged entries into Unknown on recovery. These are real safeguards and **must not be regressed**.

### Specific failure scenario to reproduce

1. Desktop gateway A and headless gateway B share the same SQLite database.
2. A journals paid request R, provider successfully answers and reports usage; A then fails to settle to SQLite (e.g., transient write failure) and keeps **exact R usage** in its own RAM `pending`.
3. B sees no such process-local pending entry. If the DB becomes writable again, B may admit another paid request before A has successfully reconciled R. For a client without a monthly limit, the reservation cannot enforce an admission cap either.
4. If A exits, its exact in-memory usage is lost; a later stale-journal recovery can preserve **Unknown + at-risk** but not the exact response usage. This is a **known conservative recovery path**, not necessarily financial data loss. However, it contradicts an unconditional interpretation of “all paid admission is blocked while *any* ledger entry is unsettled”.

### Required decision

Choose and document **one** approach:

**Option A, minimal shipping option:** Enforce **one active gateway owner per DB** with a cross-process lock/lease, clear refusal on a second process, crash/lease recovery and a non-destructive UI/CLI handoff. This is arguably simpler for a desktop-local product and prevents parallel process accounting disagreement.

**Option B, multi-process supported:** Persist a global degraded marker and pending-attempt reconciliation state transactionally, with a clear ownership/lease protocol; check it **inside the paid admission transaction** so that the check cannot race another process marking the ledger degraded. One process must not silently turn another process's *live* attempt into interrupted at restart. Do not assume a SQLite busy timeout alone provides the end-to-end contract.

If exact provider usage must survive process death after settlement failure, durable pending-usage storage is required; otherwise state explicitly that such attempts become Unknown and remain at-risk, without inventing cost.

### Acceptance tests

- Two OS processes using one DB: A gets a provider 200, fails settlement; B must not send paid inference if global accounting is degraded, or must be refused at startup under the singleton policy.
- Retry and recovery: after A successfully settles, safe paid admission resumes and the usage event is recorded exactly once.
- Crash/restart: exact usage vs Unknown behavior is documented, held spend remains visible and no duplicate ledger rows arise.
- Concurrent startup, stale lock holder, interleaved `open_attempt/settle`, SQLite busy/locked, read-only after send and disk-full (where practical).
- Existing single-process budget, disconnect and fail-closed tests still pass.

## PR-02 — MEDIUM–HIGH — Free/Local is a *billing label*, not proof a 5xx replay is safe

### Direct code evidence

- `gateway/mod.rs::is_free` returns true for billing classification `free` or `local`.
- In `forward`, when `decision.maybe_billed` is true on a 5xx, the code enforces `paid_retries_left` **only if `!is_free(&billing)`**. Thus an explicitly labeled Free/Local connection can retry another candidate without client opt-in.
- `policy.rs::decide` classifies most 5xx as `Next::Any` and `maybe_billed=true`; status alone does not prove upstream did no work.

### Disagreement with Claude's rationale

The response says that retries on Free/Local can proceed since “nothing was billed”. That addresses API USD charge only. It does **not** guarantee request idempotency, unchanged agent/tool behavior, privacy equivalence of the next connection, or zero local electricity/GPU/latency cost. Free tiers may also have rate and use limits. The main risk is **duplicate execution**, even when invoice cost is $0.

A caveat: a stateless LLM completion normally does not itself run the client's external tools, and a gateway retry *before headers* may be acceptable for many such clients. **This is a policy/semantics concern, not an assertion that every 5xx creates duplicate side effects.**

### Required decision

- Separate `monetary_charge_risk` from `safe_to_replay`. The Free/Local label may allow a **zero-API-dollar** estimate but must not automatically grant retry eligibility.
- Recommended conservative default: `maybe_processed` 5xx **stop across all billing categories**, with explicit `retry_5xx` opt-in where users value availability; optional provider/model-specific exemptions only with tested evidence of no processing.
- If choosing to preserve automatic Free/Local retry for usability, document why it is safe for the supported request/agent types, and add a distinct **replay policy** plus a bounded attempt count. Enforce the per-client connection allowlist on every retry (already implemented).
- Keep responses after headers non-retryable and keep the distinction between actual/estimated/unknown monetary cost.

### Acceptance tests

- Free provider receives request then answers 500: default policy either stops, or demonstrates a formally declared replay-safe contract; paid-retry setting must not be silently treated as Free/Local replay permission.
- Free → Paid fallback after 5xx: no unintended paid request and no bypass of client allowlist; explicit consent when paid work could start.
- Local model returns 503 after partially generating: verify bounded attempts, no overlapping generations or unintended repeated external tool action.
- Combo with 3+ Free/Local models: prove attempt budget, quota rules and no unlimited retry loops.
- Stream after first header always stops; client session affinity and tool-related requests remain consistent.

## PR-03 — LOW–MEDIUM — Clarify provenance of tests and claims

Do not downgrade the verified work already reported:
- [Implementation Status](../IMPLEMENTATION-STATUS.md) reports 87 passed / 1 ignored, but **there is no reported two-process fault-injection test**; `PR-01` cannot be closed by unit tests of one `Shared` instance.
- Direct-vs-gateway native Anthropic cache write/read parity was **measured on one model and scenario**; it should remain credited. Interactive Claude Code and some cache scenarios remain open.
- A local/free billing label is user/configuration-dependent; avoid presenting it as a provider-verified side-effect or monetary guarantee.

## Response requested from Claude

Create `docs/reviews/2026-11-11-post-d518438-response.md` (or an equivalently linked response) with the following table:

| Finding | Accept / Partial / Reject / Defer | Evidence | Design/code action | Test proof | Remaining risk |
| --- | --- | --- | --- | --- | --- |
| PR-01 multi-process accounting | | | | | |
| PR-02 Free/Local replay semantics | | | | | |
| PR-03 evidence provenance | | | | | |

**Decision questions:**
1. Does SmartTaskRouter officially allow desktop and headless processes to share and simultaneously write one DB? If not, where is singleton ownership enforced?
2. If it does, how can process B observe and enforce a failed settlement in process A at admission time, before another paid upstream call?
3. How much of the recorded actual usage can survive a crash between provider reply and ledger settlement?
4. Is a Free/Local 5xx always safe to replay? Under exactly which request and side-effect semantics?
5. Should the user choose *availability vs spend/replay safety* explicitly for all billing classes, rather than deriving retry permission from a provider label?

**Do not reopen** FR-01 privacy allowlists, atomic settlement, default paid 5xx stop, Anthropic cache-parity success or verified headless Claude Code session-header propagation unless new evidence contradicts them.

**Scope:** This commit is a **review only**. It does not modify gateway source or claim to have rerun `cargo test`.
