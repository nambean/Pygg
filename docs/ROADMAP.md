# Roadmap

> **Status:** Authoritative · focus, delivery order and release gates · reviewed 2026-11-23

Built: [Implementation Status](IMPLEMENTATION-STATUS.md); why: [DECISIONS](DECISIONS.md).

## Focus, six weeks from 2026-11-23

Agreed with the owner after market research and two rounds with Codex. Basic routing, fallback and many providers are now bundled free by the large vendors, and the "free, unlimited" routers hold most users. No one escalates only on the repository's own failed checks, under a cap that holds, with evidence attached.

**Positioning:** when your coding plan hits its limit, Pygg keeps your agent moving on connected models, and checks delegated changes before you review them.

**Daily surface:** the gateway's `auto` in the user's own agent. Smart Tasks is the opt-in checked delegation path; a narrow planner feeds it.

Thresholds are decision targets, not claims.

| # | Item | Proof | Status |
| --- | --- | --- | --- |
| 1 | **Caps that hold:** each paid attempt in its own strict gateway session; bypass and unknown-price paths blocked or marked unenforced | 0 cap violations in 1,000 adversarial requests, with concurrency, cancel and restart; reserved and settled spend shown | done 2026-11-23: 0 violations in 10 × 1,000 requests ([report](../benchmark/reports/2026-11-23-step-cap.md)) |
| 2 | **Checks gate readiness:** no check, failed check and passed check told apart; no check ran means not ready | 50 tasks with no check run: 100% stay not ready, 100% show the exact command and result | done 2026-11-23: 80 of 80 as expected, 50 of 50 unchecked results not ready ([report](../benchmark/reports/2026-11-23-checks-gate.md)) |
| 3 | **Help fill check gaps:** propose a check command or a task smoke test for approval; a smoke test passing is called that, never "correct" | 20 real thin-test tasks: a proposed check accepted on 12+, its limits named on 18+ | next |
| 4 | **Continuity when limits hit:** quota exhaustion, fallback, the same session kept, the chosen model and spend shown; no OAuth bridging | 19+ of 20 staged limit events resume without losing the working context | next |
| 5 | **Paired evaluation on users' work** against each user's normal workflow, blind review where practical | 30+ paired tasks, 10 users: completion, defects, time and cost; accepted outcomes no more than 5 points lower | next |
| 6 | **Bounded install and a narrow planner trial:** install to `auto`, then a project goal turned into small tasks with checks the user edits | 8 of 10 fresh users reach a routed request in 15 minutes; 5+ run a planner task unaided, or planner work pauses | next |

**Stop:** expanding the model-facts feed, the board, the provider catalog and web sessions; general-purpose brainstorming (the website run cost $5.78 against $0.20–0.41); treating a green check as proof; claiming hard caps on paths Pygg cannot enforce; building on the brief for hand-off until a measure shows a gain.

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

Built 2026-11-22; proofs run 2026-11-23 ([report](../benchmark/reports/2026-11-23-smart-task-proofs.md)).

| # | Item | Effort | Proof | Result |
| --- | --- | --- | --- | --- |
| 1 | **Task budget and receipt:** a range and a cap before, every attempt and the cost per kept result after | M | Estimate error and overshoot on replayed and interrupted runs | 78% within range; 1.4% over the cap by a median 43%; the cap is now the attempt's gateway limit |
| 2 | **Review evidence card:** requirement, diff, base, checks run and passed, criteria nothing checked | M | Blind reviewers find seeded defects faster; misses reported | 29/50 → 44/50 caught; "no check ran" still passed 6+ in 10 |
| 3 | **Cheap-first ladder shown with its predicted total** and the stop rules; escalate only on a failed check | M | Paired tasks with hidden checks: pass rate and cost per solved task against strong-only | 16/16 vs 17/18, no hidden misses; 1/5.6 the list cost, 1/4 the time |
| 4 | **Sandbox profile** per attempt (files, keys, network), its enforcement shown before it runs | L | Adversarial fixtures per OS | Windows: Claude Code wrote outside (fixed); the rest as stated |
| 5 | **Portable task brief** to export and import between agents | M | Hand-off completeness and recovery time; secrets redacted | 20/20 seeded secrets masked (was 8); no hand-off gain measured |
| 6 | **Coverage-aware usage:** what the gateway saw, what a provider states, what is unknown | S | Reconciled against provider reports | Ledger 7.5% under MixAICloud's counters; 1% unpriced, shown unknown |

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
