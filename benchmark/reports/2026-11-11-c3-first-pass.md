# C3 first pass: 24 cases, five arms, 2026-11-11

> **Status:** Record · ROADMAP C3, first pass · run 2026-11-11 · order: [ROADMAP](../../docs/ROADMAP.md) · built: [Implementation Status](../../docs/IMPLEMENTATION-STATUS.md)

**In one line:** every arm that did not run on Qwen passed every case, so this suite cannot tell the models apart and supports **no savings claim**; it did find three gateway defects, now fixed, and it priced the arms.

## Setup

- **Cases:** the 24 in [`benchmark/cases`](../cases): 5 bug fixes, 13 features, 2 refactors, 2 test-writing and 2 docs cases; 7 low, 13 medium and 4 high complexity. Node only. Each fixture was checked to fail as given and pass with a reference solution kept outside the repository. Test and check files are protected: an attempt that changes one fails.
- **Agent:** OpenCode 1.18.4, `opencode run --pure --format json`, one attempt per case and arm (no repeats), 10-minute limit.
- **Routing:** OpenCode's own free model went direct. Everything else went through a headless Pygg gateway on a scratch store, reached through its Anthropic surface (`@ai-sdk/anthropic` pointed at the gateway). Each attempt carried its own session key, and its cost is what the gateway's ledger recorded for that session.
- **Prices:** Anthropic list prices from the Claude API reference: Haiku 5.5 $0.10 / $0.50 per million input / output tokens; Sonnet 5.5 $2 / $10, cache reads $0.20. Cache writes at 1.25 times input. Haiku's cache-read price was not stated, so cache reads were priced as input: its figures are an upper bound. Qwen's token plan is a subscription: quota, not money.
- **Limit:** the gateway client had a **strict** $4 monthly limit, so every request was held at the most it could cost before it was sent. `bench --budget-usd` capped each run too.

## Results

| Arm | Model | Passed | Money | Per passed case | Requests | Input tokens (of which cache reads) | Output tokens | Median time |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Free | `opencode/deepseek-v4.1-flash-free`, direct | **24/24** | $0 | $0 | (direct) | 1,653,458 (1,292,160) | 16,495 | 42 s |
| Pinned inexpensive capable | Claude Haiku 5.5 | **24/24** | $0.1933 | $0.0081 | 77 | 1,623,542 (1,090,707) | 35,211 | 26 s |
| Pinned premium | Claude Sonnet 5.5 | **24/24** | $1.8118 | $0.0755 | 79 | 1,647,186 (1,124,853) | 28,111 | 27 s |
| Auto | `auto`, minimum T2: chose qwen3.8-flash (subscription) every time | **18/24** | $0 (122 quota requests) | $0 | 122 | 1,819,514 (1,585,920) | 34,814 | 38 s |
| Static combo | `bench-combo`: qwen3.8-flash, then Haiku 5.5 on an error | **20/24** | $0 (129 quota requests; Haiku never reached) | $0 | 129 | 1,944,169 (1,701,376) | 39,230 | 38 s |
| Check-driven cascade | free → qwen3.8-flash → Haiku 5.5 → Sonnet 5.5 | **24/24** | $0 | $0 | 0 (every case passed on the free model) | 1,632,938 (1,298,560) | 16,775 | 33 s |

Gateway ledger for the whole benchmark store: 437 requests, 1 failed, $2.1443 known cost, 272 with no money cost stated (all of them Qwen's subscription quota).

### What the numbers say

- **The suite is too easy.** The free model passed all 24, as did Haiku and Sonnet. The cascade never needed its second rung. Nothing here shows a cheaper arm losing quality, so nothing here shows the cascade saving money *relative to what a weaker model would have lost*. That is the question C3 exists to answer, and it is still open (ROADMAP N2).
- **Price, not quality, separated the paid arms.** For the same 24 passes, Sonnet 5.5 cost $1.81 and Haiku 5.5 $0.19: about 9.4 times as much, with the same prompts and almost the same token counts. Prompt caching worked through the gateway (66–68 % of input tokens were cache reads).
- **A combo falls back on errors, not on quality.** The static combo put Qwen first and Haiku second; Qwen never returned an error, so Haiku was never tried, and the four cases Qwen got wrong stayed wrong. Only a check-driven cascade climbs on a wrong answer.
- **Qwen's token plan failed 6 of 24 through `auto` and 4 of 24 through the combo** (38 of 48 in all), and the failures were mostly different cases: only `js-semver-compare` failed in both runs. Through `auto` that was a missing README, a missing test file, and four cases with failing checks. In each failure the agent stopped after one to three steps, changed no file and wrote 120–270 tokens. One of them (`js-matrix-spiral`), run again by hand, passed. Whether these early stops come from the model or from the Anthropic → OpenAI translation, which drops Qwen's `reasoning_content` (ROADMAP B5), was not established in this pass. **Update (N4, the same day):** a captured failure showed the translation was sound (two whole, valid tool calls and `stop_reason: tool_use`); Qwen had asked to read a file by an absolute path it got wrong, outside the worktree, and OpenCode's headless run ended on the refused permission. The bench's folder names, which nested a model name's `/`, made such a guess easier and are now flat.
- **Free is not free of trade-offs:** the free model was the slowest arm (median 42 s against 26–27 s for Claude) and the provider's terms for it were not checked.

## Defects the benchmark found in the gateway (fixed, with tests)

1. **A subscription model was never used under a money limit.** `auto` ranked qwen3.8-flash first at $0, but admission skipped it for any client with a spending limit because it had no price, and the request went on to Sonnet. The first `auto` attempt cost $0.13 that way before the run was stopped. A subscription now holds nothing against a money limit (`gateway/mod.rs`; test `a_subscription_model_is_used_under_a_spending_limit_and_holds_nothing`, which fails without the fix).
2. **A streamed tool call could start without a name.** Qwen sends a call's first chunk with `id: ""` and no name; the translator opened a `tool_use` block with `name: null`, which OpenCode rejected. A call's block now starts once its name arrives, with earlier argument fragments held until then (`gateway/anthropic.rs`; test `a_tool_call_whose_name_comes_later_starts_once_with_it`).
3. **A failed or cut-short subscription request was counted as money.** It was recorded as an unknown amount of money, so a money limit, and the bench's own cap, treated it as possible spend. It is now recorded as quota (test `a_failed_subscription_request_is_quota_not_money`).

## Spend

Paid traffic in C3: **about $2.14** at list prices (Haiku $0.19, Sonnet $1.81, the misrouted `auto` attempt $0.13, a one-case trial $0.007), within the strict $4 limit. With C1's $0.30, about **$2.44 of the $5** budget (ADR-028). The owner approved the remaining $2.56 for N2, N6 and N8.

## What this pass cannot claim

- Any saving from Auto or the cascade: no arm lost quality, so there is no trade-off to measure.
- A pass rate with bounds: each case ran once per arm.
- That the bench measures the product's cascade: the bench resends the same prompt to the next rung, while the task engine hands the failed check's output on (C2). ROADMAP N3 makes the bench drive the task engine.
- Anything about other languages, larger repositories or real projects: the fixtures are small Node modules.

## Reproducing

Gateway store, prices and client: `gateway add-connection --preset anthropic` and `--preset qwen` (token-plan endpoint, billing subscription); `gateway model … --price-in --price-out --price-cached --price-cache-write --price-source`; `gateway create-client --name opencode --monthly-usd 4`, `gateway client --name opencode --strict yes --auto-min-tier T2`; a combo `bench-combo` of qwen3.8-flash then Haiku 5.5. OpenCode configuration and agent template: [benchmark README](../README.md#through-the-gateway). Each arm: `bench --rung T…=<model> --mode single|cascade --budget-usd … --gateway-db <store>`. Raw results were kept outside the repository (scratch folder) and are summarized above.
