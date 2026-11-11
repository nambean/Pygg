# Gateway lifecycle counter-review after 2f4c9a8 — start/stop, detached work, journal recovery

**Date:** 2026-11-11  
**Status:** Answered in [the lifecycle response](2026-11-11-gateway-lifecycle-response.md). This is **not** a code fix or an accepted ADR.  
**Code baseline:** `main` commit `2f4c9a84f567a095b8c20fb4f7f3d1ae5156d870`.  
**Inspected:** `apps/desktop/src-tauri/src/gateway/mod.rs`, `gateway/store.rs`, `gateway/tests.rs`, `tests/two_gateways.rs`, [previous response](2026-11-11-post-d518438-response.md), [Implementation Status](../IMPLEMENTATION-STATUS.md), [ROADMAP](../ROADMAP.md).

## Verdict and what remains accepted

The **exclusive OS file lock** addresses the original cross-process ownership risk, and the **uniform no-automatic-5xx-retry** rule addresses the Free/Local replay criticism. Keep these. The two-process kill/restart test is valuable, but it does not prove **Stop → Start in the same desktop process while in-flight/detached work still exists**.

The following findings are **source-backed lifecycle risks or missing guarantees, not claims of a reproduced production bug**. No tests were executed by this reviewer. Claude reported 89 passed, 1 ignored, plus a two-process integration test at this baseline.

## LC-01 — HIGH — Process-level OnceLock timestamp is not a gateway-instance epoch

**Observed:** `gateway/mod.rs::process_start_ms()` is a `static OnceLock<i64>` initialized from `Shared::with_read_timeout`; `gateway::start()` calculates `age = db_now() - process_start_ms()` after acquiring the owner lock and calls `store::settle_interrupted(db, None, age)`. For store-backed starts, `settle_interrupted` turns rows older than the resulting cutoff into Unknown/Interrupted.

**Counterexample:** A desktop process launches gateway A, an attempt is journaled and not settled, the gateway stops, then gateway B starts **in the same OS process**. The cutoff still points to process creation, not to the *previous gateway ownership epoch*. An orphan journal created since process creation may remain forever unless explicitly cleaned up. Conversely, the cutoff need not accurately distinguish a live detached attempt from an orphan.

**Proposed contract:** Replace the static process-start inference with a **per-owner epoch** (ideally a persisted attempt `owner_generation`/UUID), assigned only after acquiring the lock. On restart, recover attempts from *earlier, definitively finished ownership generations*, not every row older than a timestamp with millisecond-resolution ambiguity. If refusing to implement an epoch, justify a simpler design with deterministic tests that prove orphan cleanup and no false interruption.

**Tests:** (1) start → open attempt → stop → start in **same process**; (2) stop/start quickly in the same millisecond; (3) two independent `Shared::for_store` instances in same process; (4) different process crash/restart; verify old orphan becomes exactly one Unknown entry and live work is never prematurely reconciled.

## LC-02 — CRITICAL contract risk — Owner lock may be released while detached upstream/stream pumps still run

**Observed:** `Server::stop` sends graceful-shutdown signal and waits at most 5 seconds for the Axum server task, then aborts that server task. Ownership is stored as `Server._owner: Option<File>` and released on `Server` drop. Meanwhile `detached()` spawns an independent Tokio task for request processing; streaming handlers spawn their own `tokio::spawn` pumps to consume provider output and call `record_success`. The server join handle is **not a demonstrated join of those detached tasks**.

**Risk:** A streamed or long-running provider call can continue writing the ledger after Stop releases the lock. A second gateway can acquire the lock and initiate startup recovery while the first still owns an outstanding in-memory attempt. That can turn a live attempt into Interrupted; a later `settle` becomes a no-op because the journal row is gone, losing exact usage, or concurrent sessions can violate the intended single-owner accounting contract. Distinguish worker tasks completing from the HTTP listener shutting down.

**Required behavior:** Ownership of the store must span **all upstream attempts and usage settlement tasks**, not just the listening socket. Define the shutdown policy:
- stop accepting new requests;
- track all detached handlers **and stream pumps** in an owner-scoped task tracker; optionally reject new work during draining;
- drain through a bounded grace period without allowing a replacement server to recover these journal entries;
- if draining is incomplete, either **keep the lock** until work is finished (a supervised owner task) or **abort and finalize** each outstanding attempt to durable Unknown/Interrupted *before* releasing lock; never silently abandon unsettled tasks;
- only release lock after the ownership generation is quiescent/durably recovered.

If a long upstream call cannot be safely aborted or cost is uncertain, record Unknown+at-risk, do not claim $0, and never automatically replay it.

**Tests:** Fake provider sleeps >5 seconds; Stop A → Start B on same DB; observe lock ownership, downstream response, and final usage. Also do streamed SSE >5 seconds, early client disconnect, and a pending settlement blocked by SQLite. Test both app stop/start and headless CLI termination, verifying each attempt gets one and only one ledger record.

## LC-03 — HIGH — Startup recovery errors are discarded

**Observed:** In `gateway::start()`, both `store::settle_interrupted` and `store::purge_sessions` errors are discarded via `let _ = shared.with_db(...)` before the gateway binds/listens.

**Risk:** A readonly/busy/corrupt store can prevent journal reconciliation while the gateway nevertheless reports a successfully started service; `/health` may return `accounting: ok` if the local `Accounting.pending` map is empty. New paid calls can be admitted without operator understanding of recovery failure, even if other admission writes will usually fail safely.

**Required behavior:** Treat **failed journal startup recovery as an explicit accounting-degraded/start failure**, fail closed for paid traffic, surface the error in UI and `/health`, and provide a recovery/retry path. Session cleanup can be best effort if unrelated to paid accounting, but distinguish it clearly. Propagate or persist the recovery error; do not silently ignore it.

**Tests:** Make SQLite readonly before `start()`, inject busy/IO failure in recovery, restore writes, verify health and paid admission, run recovery successfully, verify no duplicate usages. Verify that a failed purge does not mask accounting state.

## LC-04 — MEDIUM — Verification should exercise the actual desktop lifecycle

The current real two-process test `tests/two_gateways.rs` verifies the lock and recovery after killing a process. It inserts a journal entry directly rather than proving behavior during a real streaming attempt. It does **not** cover multiple start/stop cycles inside one process or live detached tasks crossing the owner-lock boundary. Keep it, but add deterministic integration coverage with a fake upstream gate/notification channel and verification of the actual journal table and `usage_events`.

Suggested acceptance matrix:

| Scenario | Required result |
| --- | --- |
| Two processes share DB simultaneously | Second refused (already tested) |
| Crash kills owner; later gateway starts | Old attempts become Unknown/at-risk exactly once (already partially tested) |
| Stop/start same process with orphan | Orphan recovered, not forgotten |
| Stop with in-flight pre-headers request | Cannot release owner while old work can settle |
| Stop with streaming pump >5s | Same; no interrupted/actual double-count |
| Ledger blocked during stop | No new paid admission without accounted recovery |
| Startup journal recovery fails | Explicit degraded/refused status; never silently green |
| Multiple rapid restarts | No duplicate ledger events, no phantom reservations |

## Requested Claude response and delivery

Please create `docs/reviews/2026-11-11-gateway-lifecycle-response.md` with **Accept / Partial / Reject / Defer** for LC-01…LC-04, exact source citations, what is implemented versus proposed, test names and residual risk.

**Priority:** Fix or conclusively disprove LC-02 before adding P0-5 Auto Routing; resolve LC-01 and LC-03 in the same lifecycle hardening pass. Preserve the existing singleton file lock and default safe 5xx behavior. Do not expand scope to a new routing engine, SDK, memory system, or branding rename.

**Evidence limitation:** This note reviews source and existing test claims; it does not assert a reproduced bug, independently rerun `cargo test`, or send paid provider traffic.
