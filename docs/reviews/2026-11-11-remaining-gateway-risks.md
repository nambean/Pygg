# Follow-up counter-review: privacy fallback, paid retry risk, accounting recovery

**Date:** 2026-11-11  
**Status:** Answered in [the follow-up response](2026-11-11-remaining-gateway-risks-response.md); not an accepted ADR, not a code patch.  
**Base:** current `main` following counter-review changes `be6a3c0` and the subsequent real-world validation commits `72146e3` and `e580734`.  
**Inputs:** [Claude counter-review response](2026-11-11-counter-review-response.md), [architect decisions](2026-11-11-architect-decisions.md), [authoritative roadmap](../ROADMAP.md), [implementation status](../IMPLEMENTATION-STATUS.md), and inspected Rust `gateway/policy.rs`, `gateway/mod.rs`, `gateway/store.rs`.

## Important verification update — acknowledge actual progress

This review **supersedes** earlier statements that Anthropic cache parity is entirely untested or that Claude Code session propagation is entirely untested.

- Per [Implementation Status](../IMPLEMENTATION-STATUS.md#provider-parity-anthropic-direct-versus-through-the-gateway-2026-11-11), a **real Anthropic direct-vs-proxy test with claude-haiku-4-5** observed cache creation of **9,011 tokens on turn 1** and cache reads of **9,011 tokens on turn 2** in both arms. Thinking/tool and citations were also checked according to that report. This is positive **scenario-specific evidence**, not a claim that all models/TTL/tool rounds have parity.
- Per [Implementation Status](../IMPLEMENTATION-STATUS.md#live-verification-claude-code-through-the-gateway-2026-11-11), **Claude Code 2.1.317 headless** sent the explicit session header on **12/12 requests** including sub-agent/compaction requests. An interactive terminal session and token-count path remain unverified.
- Gateway accounting moved to a **pre-send attempt journal** and atomic settle; failure status and unknown spend are now surfaced. Do not reopen the already-accepted CR-06 as though no fix exists.
- Earlier review responses that say parity is "unmeasured" are historical; consult the newest Implementation Status.

These are **reported results from repository documentation**. This reviewer did not rerun paid calls or cargo tests.

## FR-01 — HIGH — Cross-provider fallback can bypass a workspace-specific privacy policy

**Observed, not hypothetical:** [counter-review-response](2026-11-11-counter-review-response.md) explicitly defers provider data-use terms/privacy compatibility to P0-5. Current `gateway/mod.rs::forward` resolves the user's ordered combo and checks protocol, health, quota and client budget; its documented route selection does not include a workspace confidentiality or provider data-use filter.

**Disagreement:** Claude argues that manually adding a backup provider to a combo constitutes deliberate fallback authorization. It authorizes the *route list*, but it does **not** establish that *all requests and workspaces* are permitted to send sensitive content to every member. A provider-approved endpoint and a provider with acceptable data retention/training terms are separate properties.

**Required decision:** Do not imply that gateway P0 is privacy-safe for sensitive workspaces without one of:
1. A **minimal fail-closed per-client allowed-connections policy**, with sensitive clients pinned to explicit connections/organizations, even before P0-5; or
2. A plainly labeled **no privacy guarantees across fallback** state, with cross-provider failover disabled by default for high-sensitivity requests until provider terms and account-model entitlements are verified.

Prefer a small connection allowlist on scoped client tokens as a P0 hardening step, not a new complex policy engine. Must preserve user's existing ordered combos for ordinary low-sensitivity workflows.

**Tests:** client A permits Anthropic only, client B permits Anthropic + another vetted provider; same combo gets 403/429/5xx; A must never send data to B-only provider; budget/affinity cannot override allowlist; disabled/unknown terms fail closed for sensitive profile; no credential or prompt leakage in route diagnostics.

## FR-02 — HIGH — Retrying after 5xx is bounded, but remains a risk of duplicate provider charges

**Observed:** `gateway/policy.rs` presently treats most 500–599 as `Next::Any` and `maybe_billed=true`, while 408/504/524 stop. `mod.rs` journals an unknown-cost attempt, and `store.rs` tracks `at_risk_micro_usd` against the estimated budget. This is good accounting; it does **not prevent** the first provider from having performed/charged work before returning a 500/503.

**Disagreement:** "The combo is user-selected" is not equivalent to "the user opted into a possibly doubled bill on an ambiguous server failure." A route configured for availability may still require a separate retry-spend policy. No blanket ban on 5xx fallback is necessary; automatic paid risk must be disclosed and budgeted.

**Required decision:** Add a configurable per-client/combo policy: `stop_on_possible_billing` (cost-conservative default) vs `retry_5xx_with_budget` (explicit opt-in, at most N attempts, capped incremental estimated exposure), with provider-specific exceptions only when documented. For external agent requests with side effects, preserve session/tool boundaries and avoid replay. Keep 425 semantics separate from actual retry safety if the upstream does not honor standard early-data behavior.

**Tests:** fake provider accepts/bills attempt A then replies 500; check default stops before B; opt-in invokes B and journals both (first Unknown + at risk, second actual), keeps budget reserves until settled; 503 with `Retry-After` obeys policy; stream after headers never retried; client policy cannot be bypassed with a different combo.

## FR-03 — HIGH — Ledger durability requires fault-injection and an explicit fail-closed degraded state

**Observed:** `gateway/store.rs::open_attempt` atomically journals a request before sending; `store::settle` writes usage and deletes the journal in one transaction. `gateway/mod.rs::Attempt::settle` retries settlement and exposes accounting error state. This is a strong improvement over the previous implementation.

**Remaining gap to verify:** A DB error on a request *after* the upstream call can leave an unsettled journal (or the client may receive a response even though settlement is deferred). The code's `accounting` state is surfaced via `/health`; verify whether admission of **new paid requests** is actually blocked while the journal cannot be reliably settled. Merely displaying a red status does not guarantee spending safety. Recovery of an Unknown/at-risk amount also needs a documented path to reconcile against the provider's own usage/billing when available.

**Required decision:** Adopt **fail-closed paid admission on accounting-degraded**, but keep local/free traffic permitted if no paid ledger risks remain and it is safe to do so. Define persistent journal recovery and *exactly-once logical ledger settlement* under retries. Never erase a held attempt simply because its age exceeds the cleanup threshold without creating a durable interrupted entry. Provide an operator recovery flow with explicit `Unknown` charges until reconciled; do not invent actual cost.

**Tests:** SQLite busy/lock, disk-full, read-only transitions between open and settle, failure after upstream 200, crash while streaming, two concurrent gateway processes, startup reconciliation, repeated settle/interrupted calls, and no paid admission while accounting health is degraded. Test successful recovery re-enables admission and does not double-count.

## FR-04 — MEDIUM — Source-of-truth docs need to reflect new real-world parity tests

The recent `Implementation Status` reports a live cache-parity run and headless Claude Code session propagation, while parts of `ROADMAP.md` and historical counter-review wording may still say "fake-tested only / parity unmeasured". The single source of truth is Implementation Status, but avoid contradictory status summaries in the roadmap.

**Required decision:** Bring ROADMAP P0-1 and P0-4 verification rows up to date, while retaining open cases (1-hour cache TTL, multi-turn tool round trip, other models, interactive CLI). Define acceptance gates with evidence links to scripts and retained test artifacts/usage counts where safe to share. Do not mark release-ready based on a single model/sample.

## What should NOT change

- Keep native-format Anthropic passthrough and its measured 9,011-token two-turn cache parity; do not revert to translation.
- Preserve explicit session ID; no prompt hashing or stealth memory insertion into Claude Code/OpenCode histories.
- Preserve pre-send journaling and atomic settlement; do not revert to earlier simplistic preflight budget checks.
- Preserve bounded, documented provider quota handling; no rotation to evade user/org quota or suspension.
- Keep generalized Auto Router P0-5 as the next product feature, **after** minimal privacy and reliability gates are resolved.

## Claude response contract

Please add **`docs/reviews/2026-11-11-remaining-gateway-risks-response.md`** or extend the existing response, addressing each `FR-01` to `FR-04` with:
- **Accept / Partial / Reject / Defer**, source citation, implemented/planned status, and tests.
- If rejecting, provide an explicit counterexample and explain why cost/privacy risk is acceptable under default settings.
- If implemented, update [Implementation Status](../IMPLEMENTATION-STATUS.md), [ROADMAP](../ROADMAP.md) and security/privacy documentation consistently.
- Clearly distinguish actual Anthropic cache parity already measured from remaining tests.

This note is **review-only**. Its suggested policies are not currently implemented by this commit and no new tests or paid provider calls were run.
