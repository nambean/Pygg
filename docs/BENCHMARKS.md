# What Pygg changes: published benchmarks

> **Status:** Current · every published benchmark in one table, each figure linked to its report · reviewed 2026-11-14

Pygg is a local gateway between your coding tools and your providers. These runs measure what that changes, with real agents, real models and checks the agent cannot edit. All of them together cost about $16.7 of a $20 benchmark budget. Each report gives its method, raw results and what it left out; read it before quoting a number. The harness is described in [benchmark/README.md](../benchmark/README.md).

| What Pygg does | What was measured | Result | Report |
| --- | --- | --- | --- |
| **Turns on Anthropic's prompt cache for OpenAI-format agents** (Codex on Claude) | Codex through a gateway to Claude Haiku 5.5, 8 cases × 2, cache markers off vs Pygg | Both 16 / 16; **30% less in all, 68% less on input** (84% of input read from cache) | [Codex on Claude, prompt cache](../benchmark/reports/2026-11-14-codex-claude-prompt-cache.md) |
| **Keeps Claude Code's own cache** (Anthropic requests pass through unchanged) | Claude Code through Pygg on a real production repository, 6 tasks | Haiku 5.5 solved 6 / 6 for $1.50 with **96% of input from cache**; Sonnet 5.5 4 / 5 for $1.19 | [Real repository](../benchmark/reports/2026-11-13-ccht-real-repository.md) |
| **Lets you pick a cheaper model and see what it saved** | The same tasks solved by Haiku and Sonnet, paired | Haiku about **40% cheaper** per solved task on the real repository, 79% on small fixtures | [Real repository](../benchmark/reports/2026-11-13-ccht-real-repository.md), [P0](../benchmark/reports/2026-11-12-p0-optimization.md) |
| **`auto/cheap` routing** to the cheapest eligible model | OpenCode with `auto/cheap` on the real repository | 2 / 6 for $0.35: cheap, but free models failed the larger tasks; `auto` with a tier floor is the default for agents | [Real repository](../benchmark/reports/2026-11-13-ccht-real-repository.md) |
| **Web sessions** (sites without an API, opt-in, unofficial) | 48 short tasks on five sites | 47 / 48 answered correctly, about 18 s each, $0 | [Web sessions](../benchmark/reports/2026-11-13-web-sessions.md) |
| **Costs it can trace** | Every attempt in every run | Each recorded as actual, estimated or unknown, never unknown as $0; fallbacks and failed attempts kept | all reports |
| **Adds little time to a request** | The same requests to a local fake provider directly and through the gateway, 3 runs | About **4 ms** added per short request and **5 ms** per 400 kB prompt, the ledger written to disk before and after each send; about **230 requests/s** with 16 clients at once | [Gateway overhead](../benchmark/reports/2026-11-14-gateway-overhead.md) |

The other reports in [benchmark/reports](../benchmark/reports) cover earlier passes: the [first pass](../benchmark/reports/2026-11-11-c3-first-pass.md), [harder cases (N2)](../benchmark/reports/2026-11-12-n2-savings.md), the [handoff A/B (H1)](../benchmark/reports/2026-11-12-h1-handoff-ab.md) and [web sessions on coding tasks](../benchmark/reports/2026-11-13-web-coding.md).

## What Pygg does not claim

- **No "60–95% fewer tokens".** Compression tools advertise that; an [independent replay](https://codepointer.dev/p/cutting-llm-token-costs-with-rtk) of 500 real Claude Code sessions found 3.7%. Pygg's savings come from keeping the provider's cache working, from cheaper models you choose, and from routing you can see (ADR-036 in [DECISIONS.md](DECISIONS.md)).
- **Not a quality benchmark.** Pass rates come from each case's own checks on a handful of tasks. They show that an approach did not break the work, not that one model is better.
- **Not a provider bill.** Costs are each gateway's ledger at list prices.
