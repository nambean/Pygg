# Every connected model on every task kind, 2026-11-20

> **Status:** Record · run overnight 2026-11-19 21:56 to 2026-11-20 06:11 (UTC+7) by the maintainer's order · ROADMAP "A larger benchmark" (first part: every connected model, every task kind) · **published by the owner's order on 2026-11-20**; the README's figures are not replaced by it, since each case ran once (the ROADMAP asks for three).

**In one line:** on all 36 fixture cases, one attempt each, OpenCode through Pygg solved **36/36 on four routes that cost no metered money** (DeepSeek V4.1 Flash and Qwen 3.8 Max on a monthly plan, and `auto/cheap`, which chose Qwen 3.7 Max on that plan) and on **Gemini 4 Hydrogen at $0.072 per solved task**; the dearest route, `auto/code` on Gemini 4 Carbon, solved 33/36 at $0.115, and the two local 8B models solved 3/36 and 0/36.

## Results for readers

Same agent (OpenCode 1.18.4), same 36 tasks, same checks; only the model behind it changes. A task counts as solved only when its own tests pass and the agent left the tests untouched. One attempt per task, so every pass rate is a single outcome, not a rate.

| Route | Solved | Money (ledger, list prices) | Per solved task | Median time |
| --- | --- | --- | --- | --- |
| `auto/cheap` → Qwen 3.7 Max (plan) | **36/36** | none metered: 244 requests of plan quota | — | 33 s |
| DeepSeek V4.1 Flash (Alibaba plan) | **36/36** | none metered: 227 requests of quota | — | 25 s |
| Qwen 3.8 Max (plan) | **36/36** | none metered: 229 requests of quota | — | 65 s |
| Gemini 4 Hydrogen | **36/36** | $2.58 | **$0.072** | 50 s |
| `auto` → Qwen 3.7 Plus (plan) | 35/36 | none metered: 233 requests of quota | — | 44 s |
| Qwen 3.7 Plus (plan) | 35/36 | none metered | — | 50 s |
| Qwen 3.8 Flash (plan) | 34/36 | none metered | — | 56 s |
| Gemini 3.8 Flash | 33/36 | $2.94 | $0.089 | 134 s |
| `auto/code` → Gemini 4 Carbon | 33/36 | $3.80 | $0.115 | 162 s |
| Gemini 4 Carbon | 32/36 | $3.96 | $0.124 | 152 s |
| GLM 5.3 (Alibaba plan) | 30/36 | none metered | — | 41 s |
| Gemini 3.1 Flash Lite | 28/36 | $1.46 known, 2 runs unknown | $0.052 (known money only) | 26 s |
| Ollama `qwen3:8b` (local, RTX 3060) | 3/36 | $0 | — | 131 s |
| Ollama `qwen2.5-coder:7b` (local) | 0/36 | $0 | — | 17 s |
| OpenRouter free models | not run: see below | | | |

Pass-rate intervals (Wilson, 95%): 36/36 is 90% to 100%, 33/36 is 78% to 97%, 28/36 is 62% to 88%, 3/36 is 3% to 22%. Money intervals are in the full table. A plan's requests are quota the owner pays for monthly, not money per request, so those routes are **not comparable** with the metered ones on cost; they are comparable on what they solved.

### By task kind

| Route | Bug fixes (8) | Features (21) | Refactors (3) | Test writing (2) | Docs (2) |
| --- | --- | --- | --- | --- | --- |
| `auto/cheap` → Qwen 3.7 Max | 8 | 21 | 3 | 2 | 2 |
| DeepSeek V4.1 Flash | 8 | 21 | 3 | 2 | 2 |
| Qwen 3.8 Max | 8 | 21 | 3 | 2 | 2 |
| Gemini 4 Hydrogen | 8 | 21 | 3 | 2 | 2 |
| `auto` → Qwen 3.7 Plus | 8 | 20 | 3 | 2 | 2 |
| Qwen 3.7 Plus | 8 | 20 | 3 | 2 | 2 |
| Qwen 3.8 Flash | 7 | 20 | 3 | 2 | 2 |
| Gemini 3.8 Flash | 8 | 18 | 3 | 2 | 2 |
| `auto/code` → Gemini 4 Carbon | 8 | 18 | 3 | 2 | 2 |
| Gemini 4 Carbon | 8 | 17 | 3 | 2 | 2 |
| GLM 5.3 | 8 | 15 | 3 | 2 | 2 |
| Gemini 3.1 Flash Lite | 8 | 14 | 3 | 2 | 1 |
| Ollama `qwen3:8b` | 1 | 0 | 0 | 1 | 1 |
| Ollama `qwen2.5-coder:7b` | 0 | 0 | 0 | 0 | 0 |

Every miss but three is a `hard-*` feature (a module written against a specification with many edge cases): `hard-semver-range` and `hard-line-diff` were missed by five routes each, `hard-glob-match` by four, `hard-expr-eval` and `hard-ini-parse` by three. The other misses: `js-sum-sign-bug` (Qwen 3.8 Flash), `js-lru-cache` (Qwen 3.7 Plus), `js-merge-intervals` (GLM 5.3), `js-topo-sort` (Gemini 3.8 Flash), `js-docs-jsdoc` and `js-word-wrap` (Flash Lite). Bug fixes, refactors, test writing and docs were solved by every hosted route but one case each on two of them.

### Where the cheaper or the chosen route lost

- **`auto/code` lost to a cheaper Gemini.** The role alias picked Gemini 4 Carbon for every case (the facts feed's #4 for `code` on Terminal-Bench; this store holds no own runs, so the own-evidence layer was empty) and solved 33/36 at $0.115 per solved task, while Gemini 4 Hydrogen, one place below it in the same feed and 25% cheaper per token, solved 36/36 at $0.072. One benchmark place is not a reliable order between two neighbouring models; the alias's reason line says what it ranked on, and the sheet lets the user pick the other.
- **The feed's #4 lost to a plan model that has no fact at all.** DeepSeek V4.1 Flash and Qwen 3.8 Max, which the feed does not rank (no public fact names them), solved every case; the role alias cannot pick them until the user's own runs give them a line (ten attempts on cards of the role, [BRAIN.md](../../docs/design/BRAIN.md) §1).
- **`auto` picked a plan model and lost one case** (`hard-line-diff`) that `auto/cheap`'s plan model solved; both routes are quota, so nothing was saved or lost in money.
- **Local models are not a route for these tasks.** `qwen3:8b` timed out on 10 cases at five minutes and made no change on 4; `qwen2.5-coder:7b` made no change on any (its 7B instruct variant does not drive OpenCode's tools). The two earlier cascades that started local and climbed ([N2](2026-11-12-n2-savings.md)) remain the way to use them.
- **Gemini 3.1 Flash Lite looped.** On three cases it kept calling tools until the ten-minute limit: 191 requests and 11.7 million input tokens on `hard-text-table` ($0.51), 266 requests on `js-word-wrap` ($0.36) and 221 on `hard-json-patch`, where a request was still in flight when the agent was stopped, so that attempt's cost is unknown (as is one more) and its money is a floor. Its cheap price per token did not make it the cheap route.

## What was run

- **Tasks.** All 36 cases in [`benchmark/cases`](../cases): 8 bug fixes, 21 features, 3 refactors, 2 test-writing and 2 docs cases; 7 low, 17 medium and 12 high complexity (the 12 `hard-*`). One attempt per case and route (the ROADMAP asks for three; this night had time for one).
- **Agent.** OpenCode 1.18.4 in headless mode, Node 24.15, one Git worktree per attempt, driven by Pygg's task engine (`bench --engine opencode --mode single`), ten minutes per attempt (five for the local and free routes).
- **Routing.** Every model was reached through a headless Pygg gateway (release binaries of 2026-11-19 14:38; `bench.rs` as of commit 3499816) on a copy of the owner's store, over its Anthropic-format surface, with each attempt's id as its session so the ledger costed it. The gateway client was strict with a $50 monthly and daily limit, so every request was held at the most it could cost before it was sent.
- **Models.** Google Gemini API: 3.1 Flash Lite, 3.8 Flash, 4 Carbon, 4 Hydrogen (metered). Alibaba Model Studio's token plan (a monthly subscription, quota): DeepSeek V4.1 Flash, Qwen 3.8 Flash, Qwen 3.7 Plus, GLM 5.3, Qwen 3.8 Max. Ollama in Docker on an RTX 3060: `qwen3:8b`, `qwen2.5-coder:7b`. The aliases `auto/cheap`, `auto` and `auto/code` under the client's default policy, each resolved by the gateway per request (the model it chose is in every attempt's gateway record: one model per route all night).
- **Left out.** Anthropic's Claude models (the owner's usage limit; they join within the same $50 later). SiliconFlow: every model answered HTTP 402 (no balance) in the probe. OpenRouter's free models: the key hit OpenRouter's `free-models-per-day` limit during the probe, so the first free route's 13 attempts all timed out and the free group was stopped; those 13 rows are in the raw results and in no table. Web sessions, by ADR-035. Claude Code and Codex as agents: the ROADMAP asks for them; this night ran OpenCode only.
- **Prices.** The gateway's prices from models.dev: Gemini 3.1 Flash Lite $0.25 / $1.50 per million input / output tokens (cache read $0.025), 3.8 Flash $0.75 / $3.75 ($0.075), 4 Carbon $2 / $10 ($0.10), 4 Hydrogen $1.50 / $7.50 ($0.075). Plan requests are recorded with no money. Costs are estimates at list price, not a bill.
- **Spend.** $16.10 by the ledger for the night, of the $50 the owner set for this benchmark: Gemini $13.1 (including $0.8 of a first start of the Gemini and alias groups that stopped on an unknown cost and was rerun with `--allow-unknown yes`), `auto/code` $3.8, everything else quota or local. $33.9 of the $50 is left.

## Full table

From `python scripts/bench-report.py <results>/*.jsonl`:

| Arm | Runs | Passed | Pass rate (95% CI) | Money | Per run (95% CI) | Per passed run | Median time | Runs with an unreadable cost |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `engine:single:T0=pygg/qwen-alibaba-model-studio/deepseek-v4.1-flash` | 36 | 36 | 100% (90% to 100%) | $0.0000 | $0.0000 | $0.0000 | 25 s | 0 |
| `engine:single:T1=pygg/qwen-alibaba-model-studio/qwen3.8-flash` | 36 | 34 | 94% (82% to 98%) | $0.0000 | $0.0000 | $0.0000 | 56 s | 0 |
| `engine:single:T2=pygg/qwen-alibaba-model-studio/qwen3.7-plus` | 36 | 35 | 97% (86% to 100%) | $0.0000 | $0.0000 | $0.0000 | 50 s | 0 |
| `engine:single:T3=pygg/qwen-alibaba-model-studio/glm-5.3` | 36 | 30 | 83% (68% to 92%) | $0.0000 | $0.0000 | $0.0000 | 41 s | 0 |
| `engine:single:T4=pygg/qwen-alibaba-model-studio/qwen3.8-max` | 36 | 36 | 100% (90% to 100%) | $0.0000 | $0.0000 | $0.0000 | 65 s | 0 |
| `engine:single:T0=pygg/ollama/qwen3:8b` | 36 | 3 | 8% (3% to 22%) | $0.0000 | $0.0000 | $0.0000 | 131 s | 0 |
| `engine:single:T1=pygg/ollama/qwen2.5-coder:7b` | 36 | 0 | 0% (0% to 10%) | $0.0000 | $0.0000 | - | 17 s | 0 |
| `engine:single:T2=pygg/openrouter/nvidia/nemotron-3.5-super-vl:free` (stopped, daily limit) | 13 | 0 | 0% (0% to 23%) | $0.0000 | $0.0000 | - | 302 s | 0 |
| `engine:single:T1=pygg/google-gemini/gemini-3.1-flash-lite` | 36 | 28 | 78% (62% to 88%) | $1.4646 | $0.0407 ($0.0143 to $0.0785) | $0.0523 | 26 s | 2 |
| `engine:single:T2=pygg/google-gemini/gemini-3.8-flash` | 36 | 33 | 92% (78% to 97%) | $2.9388 | $0.0816 ($0.0590 to $0.1135) | $0.0891 | 134 s | 0 |
| `engine:single:T3=pygg/google-gemini/gemini-4-carbon` | 36 | 32 | 89% (75% to 96%) | $3.9616 | $0.1100 ($0.0949 to $0.1263) | $0.1238 | 152 s | 0 |
| `engine:single:T4=pygg/google-gemini/gemini-4-hydrogen` | 36 | 36 | 100% (90% to 100%) | $2.5791 | $0.0716 ($0.0629 to $0.0818) | $0.0716 | 50 s | 0 |
| `engine:single:T1=pygg/auto/cheap` | 36 | 36 | 100% (90% to 100%) | $0.0000 | $0.0000 | $0.0000 | 33 s | 0 |
| `engine:single:T2=pygg/auto` | 36 | 35 | 97% (86% to 100%) | $0.0000 | $0.0000 | $0.0000 | 44 s | 0 |
| `engine:single:T3=pygg/auto/code` | 36 | 33 | 92% (78% to 97%) | $3.8043 | $0.1057 ($0.0892 to $0.1228) | $0.1153 | 162 s | 0 |

Gateway traffic per route, for the record: the plan and Gemini 4 routes made 218 to 267 requests over the 36 cases with 3.3 to 5.7 million input tokens, 73% to 86% of them read from the provider's cache; Gemini 3.8 Flash made 528 requests (16.9 million input tokens, 88% from cache) and Flash Lite 1,105 (38 million, 91%: the loops above); `qwen2.5-coder:7b` made 72 requests and changed nothing.

## What this does not show

- **One attempt per case.** A 36/36 says the route solved each task once; it does not say it would again. Three repeats are the ROADMAP's rule for the figures that replace the README's.
- **Plans against money.** Four of the five best routes run on a monthly subscription whose quota is not priced per request. They are the cheapest routes in practice for a subscriber and not comparable in dollars with Gemini.
- **No Claude, no Codex, no Claude Code, no SiliconFlow, no OpenRouter free models**, for the reasons above; no case outside JavaScript and none cut from a real repository. The ROADMAP's larger benchmark still owes those.
- **The role alias had nothing of the user's.** `auto/code` ran on the feed alone, since the store copy holds no attempts on cards; with ten kept runs of a model on code cards the own-evidence layer ranks it first ([BRAIN.md](../../docs/design/BRAIN.md) §1), as it did on the development store the same day.
- **Costs are list prices from the ledger**, not provider bills; the two unknown-cost attempts of Flash Lite make its money a floor.

## Reproducing

1. A gateway store with the Gemini, Alibaba Model Studio (billing subscription) and Ollama connections, and a strict client: `gateway --db STORE create-client --name bench --monthly-usd 50` then `gateway --db STORE client --name bench --strict yes --tasks yes --day-usd 50`; `gateway --db STORE serve --port 8798`.
2. OpenCode's Pygg provider pointed at that port with the client's token and the header `x-pygg-session: {env:PYGG_ATTEMPT_ID}` (`OPENCODE_CONFIG`, [benchmark README](../README.md#through-the-gateway)).
3. One run per group, from `apps/desktop/src-tauri`, each rung an arm:

   ```powershell
   bench --cases ../../../benchmark/cases --out <results> --engine opencode --mode single `
     --gateway-db STORE --agent-timeout 600 --budget-usd 20 --allow-unknown yes `
     --rung T1=pygg/google-gemini/gemini-3.1-flash-lite --rung T2=pygg/google-gemini/gemini-3.8-flash `
     --rung T3=pygg/google-gemini/gemini-4-carbon --rung T4=pygg/google-gemini/gemini-4-hydrogen
   ```

   The other groups change the rungs: the five Alibaba plan models; `T0=pygg/ollama/qwen3:8b T1=pygg/ollama/qwen2.5-coder:7b` with `--agent-timeout 300`; `T1=pygg/auto/cheap T2=pygg/auto T3=pygg/auto/code`. `--allow-unknown yes` is needed: an attempt stopped at its time limit with a request in flight has no cost, and without the flag the run stops there; the client's strict limit bounds the spend instead.
4. `python scripts/bench-report.py <results>/*.jsonl`.

**Raw results** are in [`benchmark/results/2026-11-20-all-models/`](../results/2026-11-20-all-models): one file per group, one JSON line per case and arm, holding every attempt's checks, tokens and ledger figures, with a `SHA256SUMS` file. Local machine paths in check output were replaced.
