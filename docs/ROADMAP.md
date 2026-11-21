# Roadmap

> **Status:** Authoritative · delivery order and release gates · reviewed 2026-11-22

Built: [Implementation Status](IMPLEMENTATION-STATUS.md); why: [DECISIONS](DECISIONS.md).

## Next, in order

| # | Item | Status |
| --- | --- | --- |
| 1 | Close the gaps in [Not verified yet](IMPLEMENTATION-STATUS.md#not-verified-yet), within ADR-028's budget | in progress |
| 2 | Every screen clicked in the installed app; D12 checks (ADR-012) | next |
| 3 | Clean Windows machine: install, first run, a tool set up, uninstall | next |
| 4 | Token and cost work (ADR-036): read the cache-lifetime and clearing shadows and session receipts on real use; opt-in changes only if they pay | in progress |
| 5 | Measure Codex with and without Pygg's model catalog | next |
| 6 | macOS npm packages on a real Mac; the app on macOS and Linux (ADR-001) | next |
| 7 | Demo data replays recorded snapshots | next |
| 8 | Vietnamese translation | next (last) |
| 9 | A larger benchmark: more cases, every coding-capable model, three clients, three repeats, results per task category | later |

## What users lack elsewhere (2026-11-22)

From public evidence (an analysis of 66,320 Reddit complaint posts about AI coding tools, Aug 2025 to Sep 2026; Sonar's developer survey; vendor docs), worked through with Codex against Pygg's code. Directional: complaint shares, not user counts.

| Gap | Evidence | Pygg today | Missing |
| --- | --- | --- | --- |
| Unpredictable cost, wasted work | Token consumption 9.1% → 13.7% of complaints, now third; pricing second | Ledger per attempt, limits, launch receipts | Cost per kept result before and after, on every path |
| Verification eats the saving | 48% always check AI code; most spend more time reviewing than writing | Checks, worktrees, diff, keep or discard | One evidence card: requirement, diff, checks, what nothing checked |
| Control and safety while it runs | Lack of control 4.8% → 6.9%; agent security risks 1.2% → 2.9% | Paid steps wait for approval; worktrees | A worktree guards the checkout, not the machine, keys or network |
| Limits, switching, lost context | Independent reset clocks; context rebuilt when changing tools | Fallback, session affinity, shared skills, hand-off between attempts | A portable task brief; what Pygg cannot see marked unknown |

### Smart Tasks next, from the gap study

| # | Item | Effort | Proof |
| --- | --- | --- | --- |
| 1 | **Task budget and receipt:** a range and a cap before, every attempt and the cost per kept result after | M | Estimate error and overshoot on replayed and interrupted runs |
| 2 | **Review evidence card:** requirement, diff, base, checks run and passed, criteria nothing checked | M | Blind reviewers find seeded defects faster; misses reported |
| 3 | **Cheap-first ladder shown with its predicted total** and the stop rules; escalate only on a failed check | M | Paired tasks with hidden checks: pass rate and cost per solved task against strong-only |
| 4 | **Sandbox profile** per attempt (files, keys, network), its enforcement shown before it runs | L | Adversarial fixtures per OS |
| 5 | **Portable task brief** to export and import between agents | M | Hand-off completeness and recovery time; secrets redacted |
| 6 | **Coverage-aware usage:** what the gateway saw, what a provider states, what is unknown | S | Reconciled against provider reports |

Not to build: a quota scraper, cookie or OAuth bridges into subscriptions, estimates sold as bill guarantees, passing checks presented as proof of safety, or a promise that cheap routing always saves.

## Release gates

- **Parity:** same model direct and through Pygg, no unexplained cache or cost regression.
- **Honesty:** no saving claim without a reproducible benchmark; unknown cost stays Unknown.
- **Security:** no unconfirmed account switch or refused data-use terms; limits hold under concurrency; loopback only.
- **Errors:** real 403, 429, quota and timeout answers as fixtures.
- **Install:** clean-machine install and uninstall; signed installer.
- **Interface:** every screen clicked natively; D12 checks; `pnpm ui-check`; Vietnamese complete.
- **Web sessions:** the guard always on; account-risk warning per connection (ADR-035).

## Later (when data or a request justifies it)

- Subscription quota tracking, if a provider states it.
- The `image` role, once the gateway has an images path (ADR-041).
- A root `SECURITY.md` and a `CHANGELOG.md`.
- Billing-tier views (ADR-020); minimum-tier rules by task category (ADR-006).
- Idempotency keys, provider usage reconciliation, fallback after headers: held.
- The benchmark run from the app.
- Aider and Cursor verified as clients.
- A signed updater.
- `auto/fast`, `auto/local`, `auto/quality`: only if the benchmark needs them.
- Semantic caching, advanced compaction: only if a test beats a simpler baseline.
- Skills from a catalog, pinned to a reviewed commit (ADR-039).

Withdrawn: Auto Chat and app-owned memory (ADR-028); bridging subscription sign-ins (ADR-026).
