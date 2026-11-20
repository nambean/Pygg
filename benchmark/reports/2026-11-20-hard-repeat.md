# The hard cases three times each, after the band order, 2026-11-20

> **Status:** Record · run 2026-11-20 12:38 to 17:35 (UTC+7) by the maintainer, as planned after [the overnight run](2026-11-20-all-models.md) · ROADMAP "A larger benchmark" (three repeats of the routes that mattered) · first run of the role order in bands (commit bfcd626) and of the harness floor for cut-short requests (ac50acf).

**In one line:** on the 17 hardest fixture cases, three attempts each, DeepSeek V4.1 Flash on a monthly plan solved **51/51**, Gemini 4 Hydrogen **50/51 at $0.106 per solved run**, and `auto/cheap` **49/51** on plan quota. `auto/code`, now ranking close facts in bands, chose Gemini 4 Hydrogen (18/20) until the Gemini account's prepaid credit ran out at 14:55; Pygg then held Gemini and ran the rest on the next model of the route, GLM 5.3 on the plan (21/31): **39/51 in all, $0.048 per solved run**.

## Results for readers

Same agent (OpenCode 1.18.4), same checks, three attempts per case and route. A run counts as solved only when the case's own tests pass and the agent left them untouched.

| Route | Solved | Pass rate (95% CI) | Money (ledger, list prices) | Per solved run | Median time |
| --- | --- | --- | --- | --- | --- |
| DeepSeek V4.1 Flash (Alibaba plan) | **51/51** | 100% (93% to 100%) | none metered: 370 requests of plan quota | — | 36 s |
| Gemini 4 Hydrogen | **50/51** | 98% (90% to 100%) | $5.28 | **$0.106** | 81 s |
| `auto/cheap` → Qwen 3.7 Max (plan), two free models for a few runs | **49/51** | 96% (87% to 99%) | none metered: 363 requests of quota, 97 to free models | — | 92 s |
| Qwen 3.8 Max (plan) | 41/51 | 80% (68% to 89%) | none metered: 359 requests of quota | — | 218 s |
| `auto/code` → Gemini 4 Hydrogen, then GLM 5.3 (plan) | 39/51 | 76% (63% to 86%) | $1.86 | $0.048 | 137 s |

`auto/code` by the model that answered (the gateway record of each run):

| `auto/code` answered by | Solved | Money |
| --- | --- | --- |
| Gemini 4 Hydrogen, the route's first model (the first 20 runs) | 18/20 | $1.86 |
| GLM 5.3 on the Alibaba plan, after Gemini answered 402 (the last 31 runs) | 21/31 | quota |

The 12 `hard-*` cases ran once in the overnight run as well; the five added on 2026-11-20 did not exist then.

| Route | Overnight, 1 attempt | Now, 3 attempts | Five new cases, 3 attempts |
| --- | --- | --- | --- |
| DeepSeek V4.1 Flash | 12/12 | 36/36 | 15/15 |
| Gemini 4 Hydrogen | 12/12, $1.20 | 35/36 | 15/15 |
| `auto/cheap` | 12/12 | 34/36 | 15/15 |
| Qwen 3.8 Max | 12/12 | 29/36 | 12/15 |
| `auto/code` | 9/12 on Gemini 4 Carbon, $1.82 | 27/36 (Hydrogen, then GLM 5.3) | 12/15 (GLM 5.3) |
| GLM 5.3 (plan), named | 7/12 | not run | not run |

### What it says about Pygg's choices

- **The band order changed the pick for the better.** Ranked by the single best fact, `auto/code` took Gemini 4 Carbon overnight (9 of the 12 hard cases, $0.152 a run). In bands, Hydrogen (Terminal-Bench #12, within 0.15 of Carbon's #4, and cheaper a token) went first: 18 of its 20 runs solved, at $0.093 a run.
- **The fallback did what it is for.** At 14:55 Google answered `402 Your prepayment credits are depleted`. Pygg marked the account out of credit, held every Gemini model, and each later request went to the route's next model, GLM 5.3 on the Alibaba plan, without the agent seeing an error; Gemini was tried again every ten minutes and refused ten times. No run was lost to the outage; the runs after it were GLM's, and GLM is the weaker model here (21 of 31; 7 of 12 when named overnight).
- **What would rank GLM lower is the user's own evidence**, which this harness does not produce: bench runs are tasks without a card role, so they never reach the first layer of the role order. On a board, ten runs of a model on code cards at GLM's rate here (68% kept) against Hydrogen's (90%) put them in different kept-rate bands ([BRAIN.md](../../docs/design/BRAIN.md) §1). That has not been measured.
- **`auto/cheap` stays the strongest Pygg route on a plan:** 49 of 51 on quota, its two misses on Qwen 3.7 Max (the agent made no change), and 97 requests to free models it tried first while they were eligible.
- **A cheap plan model beat every metered one:** DeepSeek V4.1 Flash solved every run, fastest. On a subscription it is the route to beat; quota is not free, and Pygg does not track a plan's quota left yet.

### Where the cheaper or the chosen route lost

- `auto/code` lost 12 runs: `hard-glob-match` twice on Hydrogen; on GLM 5.3, `hard-line-diff` and `hard-semver-range` three times each, `py-config-includes` twice, `hard-text-table` and `x-refactor-async-modules` once.
- Qwen 3.8 Max lost 10, across seven cases, and was the slowest route (218 s median); overnight it solved all 12 hard cases once.
- `hard-glob-match` beat every route but DeepSeek V4.1 Flash at least once.

## What was run

- **Tasks.** The 12 `hard-*` cases and the 5 added on 2026-11-20 (`x-sqlite-migration`, `x-refactor-async-modules`, `x-feature-discounts`, `py-config-includes`, `py-booking-rules`), from [`benchmark/cases`](../cases): 17 cases, three attempts each (`--repeat 3`).
- **Agent.** OpenCode 1.18.4 headless, one Git worktree per attempt, Pygg's task engine (`bench --engine opencode --mode single`), ten minutes per attempt.
- **Routing.** A headless Pygg gateway (release build with commits bfcd626 and ac50acf) on the same copy of the owner's store as the overnight run, the same strict client ($50 a month and a day), each attempt's id as its session.
- **Groups.** The plan pair (DeepSeek V4.1 Flash, Qwen 3.8 Max; `--budget-usd 2`), Hydrogen (`--budget-usd 10`), the aliases (`auto/cheap`, `auto/code`; `--budget-usd 10`), side by side.
- **Cost reading.** With the floor of ac50acf, no run was left with an unreadable cost (`--allow-unknown yes` was set and never needed).
- **Spend.** $7.21 by the ledger (Hydrogen $5.28, `auto/code` $1.86, the rest quota), taking the benchmark's total to $23.31 of the $50 in ADR-028. The Gemini account's prepaid credit ran out at 14:55; a later run on Gemini needs it topped up.

## Full table

From `python scripts/bench-report.py <results>/*.jsonl`:

| Arm | Runs | Passed | Pass rate (95% CI) | Money | Per run (95% CI) | Per passed run | Median time | Runs with an unreadable cost |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `engine:single:T1=pygg/auto/cheap` | 51 | 49 | 96% (87% to 99%) | $0.0000 | $0.0000 | $0.0000 | 94 s | 0 |
| `engine:single:T3=pygg/auto/code` | 51 | 39 | 76% (63% to 86%) | $1.8596 | $0.0365 ($0.0158 to $0.0587) | $0.0477 | 138 s | 0 |
| `engine:single:T4=pygg/google-gemini/gemini-4-hydrogen` | 51 | 50 | 98% (90% to 100%) | $5.2758 | $0.1034 ($0.0884 to $0.1206) | $0.1055 | 82 s | 0 |
| `engine:single:T0=pygg/qwen-alibaba-model-studio/deepseek-v4.1-flash` | 51 | 51 | 100% (93% to 100%) | $0.0000 | $0.0000 | $0.0000 | 37 s | 0 |
| `engine:single:T4=pygg/qwen-alibaba-model-studio/qwen3.8-max` | 51 | 41 | 80% (68% to 89%) | $0.0000 | $0.0000 | $0.0000 | 220 s | 0 |

## What this does not show

- **Seventeen cases.** Three attempts give a rate per route on these cases, not a ranking of models in general; the confidence intervals overlap for every route above 90%.
- **Plans against money.** Three of the five routes run on a monthly subscription's quota, not comparable in dollars with Gemini.
- **No own evidence.** The role order's first layer had nothing to read (see above); the board's correction of a weak band pick is designed and unmeasured.
- **`auto/code` is two routes in one.** Its first 20 runs are Hydrogen's and its last 31 GLM 5.3's, split by the Gemini credit running out, not by the cases: the runs go in case-name order, so GLM got every case from `hard-json-patch` on, the five new ones among them.
- **Costs are list prices from the ledger**, not provider bills.

## Reproducing

As in [the overnight report](2026-11-20-all-models.md#reproducing), with `--cases` pointing at a folder holding the 17 cases above, `--repeat 3`, and the groups and budgets listed under What was run. Then `python scripts/bench-report.py <results>/*.jsonl`.

**Raw results** are in [`benchmark/results/2026-11-20-hard-repeat/`](../results/2026-11-20-hard-repeat): one file per group, one JSON line per case, attempt and arm, with a `SHA256SUMS` file.
