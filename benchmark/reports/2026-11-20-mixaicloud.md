# DeepSeek, Qwen, GLM and Kimi through MixAICloud, 2026-11-20

> **Status:** Record · sponsored: the traffic was paid with API credit from [MixAICloud](https://us.mixaicloud.com), the project's first sponsor, who asked how Pygg routes the work their users send to DeepSeek and Qwen (coding) and to GLM and Kimi (lighter and conversation tasks) · run 2026-11-20 14:52 to 23:12 (UTC+7) by the maintainer · the sponsor saw nothing before publication, and a route where Pygg's choice lost is reported like any other.

**In one line:** on the 17 hardest coding cases, three attempts each, the best route through MixAICloud was **DeepSeek V4 Flash with a climb to Qwen 3.8 Max when the checks fail: 49/51 at $0.034 per solved run**; on 40 light tasks × 3, **DeepSeek V4 Flash answered 98% at $0.23 per 1,000 correct answers**, about a fifth of the next model's cost; and the run changed Pygg: `auto/cheap` now learns each model's real answer length and cache use from the ledger, which took it from Qwen 3.5 Flash ($1.91 per 1,000 correct) to DeepSeek V4 Flash ($0.23) on the light tasks.

## For MixAICloud's users

- **Coding:** start on **DeepSeek V4 Flash** and climb to a larger model only when the project's checks fail. Through Pygg that is one combo or a task ladder. It solved 49 of 51 runs, and climbed once.
- **Strongest single model for coding: DeepSeek V4 Pro**, 47/51 at $0.073 per solved run; 86% of its agent prompts were read from cache.
- **Light and conversation tasks:** **DeepSeek V4 Flash** is the fastest (1.0 s median) and cheapest per correct answer. Kimi K2.6 and K3 and Qwen 3.6 Flash were as accurate (98% to 100%) at 5 to 15 times the money.
- **GLM:** could not be measured. MixAICloud's GLM group runs through an OpenRouter account that was nearly out of credit (402 "can only afford" about 100 tokens), and GLM 5.3 answered with its provider's 401. Both are on MixAICloud's side; tell the sponsor before a rerun.
- **Prompt cache:** DeepSeek V4 Flash (58%), DeepSeek V4 Pro (86%), Qwen 3.8 Max (86%) and Kimi K2.5 (92%) read much of each agent prompt from cache through MixAICloud; Qwen 3.6 Plus and Qwen 3.5 Flash read none (0% of 15 million prompt tokens), which is most of why they cost more per solved run.

## Part A: coding

OpenCode 1.18.4 through Pygg on the 12 `hard-*` cases and the 5 added on 2026-11-20 (a schema migration, a five-module async refactor, a four-module feature, two Python cases), three attempts per case and route. A run counts as solved only when the case's own tests pass and the agent left them untouched.

| Route | Solved | Pass rate (95% CI) | Money | Per solved run | Median time |
| --- | --- | --- | --- | --- | --- |
| Cascade: DeepSeek V4 Flash, then Qwen 3.8 Max when the checks fail | **49/51** | 96% (87% to 99%) | $1.64 | **$0.034** | 66 s |
| DeepSeek V4 Pro | 47/51 | 92% (81% to 97%) | $3.43 | $0.073 | 128 s |
| DeepSeek V4 Flash | 45/51 | 88% (77% to 94%) | $1.98 | $0.044 | 73 s |
| `auto/code` → Kimi K2.5 (Pygg's choice) | 43/51 | 84% (72% to 92%) | $2.50 | $0.058 | 138 s |
| Qwen 3.6 Plus | 42/51 | 82% (70% to 90%) | $3.54 | $0.084 | 260 s |
| Qwen 3.8 Max | 42/51 | 82% (70% to 90%) | $4.38 | $0.104 | 227 s |
| `auto/cheap` → Qwen 3.5 Flash (first pass only) | 8/17 | 47% (26% to 69%) | $1.08 | $0.135 | 262 s |

- **The cascade** solved 48 runs on DeepSeek V4 Flash and one more after climbing; it lost `hard-semver-range` twice, the case every route missed at least once.
- **`auto/code`** took Kimi K2.5: the only connected model with a coding benchmark fact (SWE-bench Verified #26 of 110), cheap, and in the same band as the facts above it. It was the cheapest single route at the first pass (15/17 at $0.054) and lost ground in the next two (28/34). The newer DeepSeek and Qwen ids had no fact in the feed yet and ranked by tier only, so Pygg could not prefer DeepSeek V4 Pro on evidence.
- **`auto/cheap`** ranks by the expected cost of one request. On agent work Qwen 3.5 Flash is the cheapest per request and the worst per solved task: more turns, longer thinking, no cache. It ran only the first pass. For coding, `auto/cheap` is the wrong alias; `auto/code` or a cascade is the right one.

## Part B: light and conversation tasks

[`scripts/bench-light.py`](../../scripts/bench-light.py): 40 short tasks, each with a deterministic check: 7 extractions to JSON, 6 classifications, 6 rewrites under constraints, 6 small reasoning tasks, 5 one-function coding tasks (run against tests in a separate process), 6 multi-turn conversations (recall, a system rule to keep or relax, a correction, a language rule) and 4 translations keeping named terms. Three rounds, one request at a time per route, a 4,000-token output limit, money from Pygg's ledger.

| Route | Correct | Median time | Output tokens per task | Money | Per 1,000 correct |
| --- | --- | --- | --- | --- | --- |
| **DeepSeek V4 Flash** | 117/120 | **1.0 s** | 153 | $0.028 | **$0.24** |
| Qwen 3.6 Flash | 120/120 | 7.6 s | 904 | $0.132 | $1.10 |
| Kimi K2.6 | 118/120 | 8.3 s | 542 | $0.266 | $2.25 |
| Kimi K3 | 119/120 | 6.1 s | 202 | $0.410 | $3.45 |
| GLM 5.2 | 0/40 | — | — | $0 | — (402 from MixAICloud's upstream, every request) |
| `auto/cheap`, before the change below → Qwen 3.5 Flash | 116/120 | 11.8 s | 1,531 | $0.221 | $1.91 |
| `auto/cheap`, after it → DeepSeek V4 Flash | 117/120 | 1.0 s | 153 | $0.027 | **$0.23** |
| `auto` (lowest tier) → Qwen 3.5 Flash, both runs | 233/240 | 10 s | about 1,850 | $0.531 | $2.28 |

- **DeepSeek V4 Flash's three misses** are one task: told to give a discount code only to students, it refused a user who said they were one.
- **Qwen 3.5 Flash thinks past the limit.** One request with a 4,000-token limit was billed 81,932 output tokens; sent straight to MixAICloud with a 300-token limit it returned 2,982, almost all reasoning. A request that runs out of tokens while thinking returns no answer, which is most of its misses.

### What changed in Pygg

`auto/cheap` used to rank by list price at the request's output limit, which made Qwen 3.5 Flash ($0.12 / $1.20 per million tokens) look cheapest. It now ranks by the request's expected cost: from each model's own answers in the last 30 days to prompts of about the same size, the mean output length and the share of the prompt read from cache (commit 7951c51, ADR-006 amended). The rerun above is the same 40 tasks × 3 after the change.

## Part C: what Pygg did on top

- **Fallback and holds.** MixAICloud passes its upstream's refusals through as `upstream_error`. At first Pygg read GLM 5.3's upstream 401 as a bad key and the OpenRouter 402 as the account out of credit, which held every MixAICloud connection: DeepSeek, Qwen and Kimi too. Both were fixed during the run (commits 7578e1f and 28f2cdc): an upstream refusal now cools only that model, and custom connections share limits only with the same host. Afterwards a GLM 402 cooled GLM alone, and `auto` routes moved on.
- **Dropped connections.** MixAICloud cut 11 streams mid-answer (2 at 15:35, 4 at 15:52, 3 at 15:55, 2 around 17:06), and at 15:52 and 15:55 five requests could not be sent at all, on every connection at once. Pygg does not re-send a request the provider may have run: it held the host for 15 to 30 s and the routes resumed. The coding runs those requests belonged to are counted as they ended.
- **The ledger.** 4,023 requests, every one with its cost and source; 25 were cut by the agent itself (OpenCode closing a stream). No coding run had an unreadable cost: a request cut at the time limit is costed as a floor at the tokens seen.
- **The spending limit.** Two strict Pygg clients held the run under a hard cap: $25 for coding and $3 for light tasks.

## What was run

- **Gateway.** A headless Pygg gateway on a scratch store holding only MixAICloud: four connections, one per key group (`dsv4`, `qwen`, `zhipu`, `kimi`), OpenAI format, prices set from MixAICloud's public price list (`/api/pricing`) by its ratios. Release builds from this repository: the first coding pass and the first light run on commit 28f2cdc, the later passes and the light rerun on c8febdd.
- **Models and list prices** (input / output per million tokens): DeepSeek V4 Flash $0.44 / $1.32, V4 Pro $1.32 / $3.96; Qwen 3.5 Flash $0.12 / $1.20, 3.6 Flash $0.20 / $1.20, 3.6 Plus $0.30 / $2.00, 3.8 Max $1.80 / $5.40; Kimi K2.5 $0.60 / $3.00 (in MixAICloud's `qwen` group), K2.6 $1.00 / $4.00, K3 $3.00 / $15.00; GLM 5.2 $1.20 / $4.20. Cached input at MixAICloud's cache ratios.
- **Left out.** GLM 5.3 and 5.3 Flash (upstream 401); two DeepSeek ids listed at $75 per million tokens, which look like placeholders; the `default` group (Claude, Gemini, GPT and others), outside what the sponsor asked.
- **Agent.** OpenCode 1.18.4 through Pygg's Anthropic surface, Pygg's task engine (`bench --engine opencode`), ten minutes per attempt, output limit 16,000 tokens (the models' recorded maximum).
- **Spend.** $20.57 by the ledger at list prices: $18.94 coding, $1.62 light tasks.

## What this does not show

- **Seventeen coding cases and 40 light tasks.** Rates on these, not a ranking of the models in general; most intervals overlap.
- **One provider route each.** Every model was reached through MixAICloud; the same model elsewhere may cache, think or fail differently.
- **GLM**, for the reasons above.
- **Own evidence.** Bench runs are tasks without a card role, so the role order's first layer, the user's kept results, had nothing to read.
- **Costs are list prices from the ledger**, not MixAICloud's bill. MixAICloud's own usage counters for the four keys, read after the run, came to 11,058,888 quota units: about $22.12 at new-api's default of 500,000 units a dollar (an assumption; the instance's rate is not published), 7.5% above the ledger's $20.57. Per key: DeepSeek 5.2% above, Qwen 9.2%, Kimi 0.8%.

## Reproducing

1. A Pygg store with one custom connection per MixAICloud key group (`gateway add-connection --preset custom --endpoint https://us.mixaicloud.com/v1 --key-env …`), `gateway refresh-models`, prices from `/api/pricing`, and a strict client (`create-client`, then `client --strict yes --monthly-usd … --day-usd …`); `gateway serve --port 8797`.
2. Coding: as in [the overnight report](2026-11-20-all-models.md#reproducing), with an OpenCode configuration that lists the models used, `--repeat 3`, the rungs in the table, and `--mode cascade` with `T0=pygg/mixai-deepseek/deepseek-v4-flash --rung T4=pygg/mixai-qwen/qwen3.8-max` for the cascade.
3. Light tasks: `python -I scripts/bench-light.py --base http://127.0.0.1:8797 --key-file <Pygg key> --gateway-db <store> --routes <routes> --rounds 3 --workers 1`.

**Raw results** are in [`benchmark/results/2026-11-20-mixaicloud/`](../results/2026-11-20-mixaicloud): the coding runs per group and pass, the light tasks before and after the change, with a `SHA256SUMS` file. Local machine paths in check output were replaced.
