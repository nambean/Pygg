# What Pygg changes: published benchmarks

> **Status:** Current · every published benchmark, each figure linked to its report, and Pygg next to other gateways · reviewed 2026-11-20

Real agents, real models, checks the agent cannot edit; a fault-injecting fake provider where a fault must repeat. Read a row's report before quoting a number; harness: [benchmark/README.md](../benchmark/README.md).

**Spend** ([ADR-028](DECISIONS.md#adr-028--mit-licence-a-20-benchmark-budget-and-no-auto-chat)): about $16.7 for runs to 2026-11-16 ("to 11-16" below), $16.10 overnight 2026-11-20, $7.21 hard-case repeat that day, $0 comparison runs.

## Results

| Run | Setup | Result | Cost | Report |
| --- | --- | --- | --- | --- |
| **Prompt cache, OpenAI-format agents** | Codex → Claude Haiku 5.5, 8 cases × 2, markers off vs Pygg | Both 16 / 16; **30% less in all, 68% less on input** (84% input from cache) | to 11-16 | [Codex on Claude](../benchmark/reports/2026-11-14-codex-claude-prompt-cache.md) |
| Prompt cache, live | Haiku 5.5 via OpenRouter, both APIs, 9 requests | **99.7%** of each later prompt from cache; both misses named | to 11-16 | [OpenRouter live](../benchmark/reports/2026-11-16-openrouter-cache-live.md) |
| Prompt cache vs LiteLLM | 15-turn session, fake Anthropic API | Pygg **89.5%** both APIs; LiteLLM 0% default, 89.5% / 39.7% configured | $0 | [Tokens and cost](../benchmark/reports/2026-11-14-token-savings.md) |
| **Keeps Claude Code's cache** | Claude Code, real repository, 6 tasks | Haiku 5.5 6 / 6, **96% input from cache**; Sonnet 5.5 4 / 5 | $1.50 + $1.19 | [Real repository](../benchmark/reports/2026-11-13-ccht-real-repository.md) |
| **Cheaper model, savings shown** | Same tasks, Haiku vs Sonnet | **$0.0133 vs $0.1142 per solved task** (OpenCode; 95% CI $0.0118–0.0150, $0.1019–0.1297); 79% less with Claude Code | to 11-16 | [N2](../benchmark/reports/2026-11-12-n2-savings.md), [P0](../benchmark/reports/2026-11-12-p0-optimization.md), [Tokens and cost](../benchmark/reports/2026-11-14-token-savings.md) |
| **`auto/cheap`** | OpenCode, 12 hard fixtures × 3; real repository | 34 / 36 at $0.0129 per solved task; **2 / 6** on the real repository | to 11-16 | [P0](../benchmark/reports/2026-11-12-p0-optimization.md), [Real repository](../benchmark/reports/2026-11-13-ccht-real-repository.md) |
| **Fallback before any byte** (401, 402, 429, unreachable) | 100 requests × 3 per fault, fake | **100% served**, ~1.04 attempts per request; LiteLLM 100%, 4 attempts, 4.6 s (88 s with `Retry-After: 30`) | $0 | [Resilience](../benchmark/reports/2026-11-14-resilience.md) |
| **Key ring** (ADR-038) | Three accounts' keys (429, 402, healthy), then all failing | 100% served from the healthy key, then the fallback; live: two revoked keys fell back to Ollama | $0 | [Resilience](../benchmark/reports/2026-11-14-resilience.md) |
| **Session stays on one key** | 20 sessions × 5 requests × 3, three keys | **60 / 60** on one key; all keys used | $0 | [Resilience](../benchmark/reports/2026-11-14-resilience.md) |
| **No request run twice unless allowed** | 500, dropped connection, cut stream | Default **96% / 0% served**; one retry allowed: 5xx **100%**; none answered twice | $0 | [Resilience](../benchmark/reports/2026-11-14-resilience.md) |
| **Records every attempt** | Ledger vs fake provider | One row per attempt, failures and fallbacks kept | $0 | [Resilience](../benchmark/reports/2026-11-14-resilience.md) |
| **Web-session guard** (ADR-035) | Burst, pasted files, limit notice; 24 h virtual clock | 40 at once reached the site **5 times** (DeepSeek 1); after a limit notice **0 of 20**; the suspension workload: 4 prompts in 15 min instead of 24 (DeepSeek 0) | $0 | [Web guard](../benchmark/reports/2026-11-14-web-guard.md) |
| **Every connected model** | OpenCode, 36 cases × 1, 15 routes | **36/36** on DeepSeek V4.1 Flash, Qwen 3.8 Max, `auto/cheap` (plan quota) and Gemini 4 Hydrogen at **$0.072 per solved task**; `auto/code` 33/36 at $0.115; local 8B 3/36 and 0/36 | $16.10 | [All models](../benchmark/reports/2026-11-20-all-models.md) |
| **Role bands, empty account** | OpenCode, 17 hardest × 3, five routes | DeepSeek V4.1 Flash **51 / 51**; Hydrogen 50 / 51 at **$0.106**; `auto/cheap` 49 / 51; `auto/code` moved to GLM 5.3 when credit ran out, no error to the agent | $7.21 | [Hard cases × 3](../benchmark/reports/2026-11-20-hard-repeat.md) |
| **Learns real cost** (`auto/cheap`, ADR-006) | MixAICloud: 17 hardest × 3, 40 light × 3 | Light: Qwen 3.5 Flash **$1.91** → DeepSeek V4 Flash **$0.23** per 1,000 correct, same accuracy; coding **49 / 51 at $0.034** per solved run | sponsored | [MixAICloud](../benchmark/reports/2026-11-20-mixaicloud.md) |
| **Brain Storming lifts a cheap model** (ADR-041, ADR-042) | A whole website from a one-page spec (MixAICloud): DeepSeek V4 Pro alone × 3, Qwen 3.6 Flash alone × 3, Qwen through Brain × 1; hidden tests and two blind reviews | Qwen alone **0/37** API checks every run; through Brain **37/37 and 20/21 UI**, like DeepSeek alone (36–37, 17–20); reviews: Codex 7 vs 7, Opus 5 vs 8; but **$5.78 against $0.20–0.41** (no prompt cache for Qwen) | sponsored | [Website](../benchmark/reports/2026-11-21-stockroom-brain.md) |
| **Smart Tasks: six proofs** | Budget replay of 1,049 paid attempts; seeded defects; hidden checks, 6 hard cases × 3; sandbox fixtures, 4 agents; 20 secret shapes; ledger vs MixAICloud | Cheap-first **16/16 at $0.029** per solved run (list price) vs Qwen 3.8 Max 17/18 at $0.163, no hidden misses; the evidence card 29 → **44 / 50** defects caught; Claude Code wrote outside its copy and the brief masked 8/20 secrets, **both fixed** | $0 (plan quota) | [Six proofs](../benchmark/reports/2026-11-23-smart-task-proofs.md) |
| **Step caps hold** (ROADMAP focus item 1) | 10 × 1,000 adversarial requests at 500 capped steps, fake provider: oversized, concurrent from two keys, cancelled, restart | **0 violations**: no step's ledger or provider bill passed its cap; found and fixed a 1–2 micro-dollar overshoot from rounding | $0 | [Step caps](../benchmark/reports/2026-11-23-step-cap.md) |
| **Web sessions** (opt-in) | 48 short tasks, five sites | 47 / 48, ~18 s each | $0 | [Web sessions](../benchmark/reports/2026-11-13-web-sessions.md) |
| **Time added** | Fake provider, direct vs gateway, 3 runs | **4 ms** short, **5 ms** 400 kB; **230 requests/s**, 16 clients | $0 | [Overhead](../benchmark/reports/2026-11-14-gateway-overhead.md) |

Earlier: [first pass](../benchmark/reports/2026-11-11-c3-first-pass.md), [handoff A/B](../benchmark/reports/2026-11-12-h1-handoff-ab.md), [web coding](../benchmark/reports/2026-11-13-web-coding.md); all in [benchmark/reports](../benchmark/reports).

## Pygg next to other gateways

**M** = measured here (report above). **D** = from the project's docs or source (2026-11-14 unless dated), not run. "—" = not documented. Only LiteLLM was run (image pinned by digest); others are third-party images, not run.

| | **Pygg** | **LiteLLM** | **OpenRouter** | **9router** | **OmniRoute** | **claude-code-router** |
| --- | --- | --- | --- | --- | --- | --- |
| Time added | **4 ms**, ~230 req/s (M) | 2 ms at 1,170 req/s, 4 instances (D) | — | — | — | — |
| Primary failing: served | **100%** (M) | 100% (M) | Any error, `models` list (D) | 401, 402, 403, 429, 5xx (D) | Not on auth errors (D) | Per rule (D) |
| Attempts, rate-limited primary | **1.04**, ~70 ms (M) | 4, ~4.6 s (M) | — | — | — | — |
| Honours `Retry-After` | Yes (M) | Up to 60 s (D, M) | Passes it on (D) | No, fixed (D) | On 429 (D) | Yes (D) |
| Re-sends a possibly-run request | **No by default** (M) | Up to 3 attempts (M) | — | On 5xx, timeouts (D) | Up to 3 (D) | When set (D) |
| Session on one key | **Default**, 60 / 60 (M) | Opt-in: 0 / 60 off, 60 / 60 on (M) | Default (D) | `smart` only (D) | Strategy (D) | — |
| Prompt cache, OpenAI-format on Claude | **Automatic**, 89.5% (M) | Opt-in: 89.5% / 39.7%; 0% default (M) | Client sends `cache_control` (D) | — | — | — |
| Web sessions | 6 sites, guard always on (M) | No | No | 6 sites, no pacing found (D) | 3 sites (D) | No |

Sources: LiteLLM 1.106.2 ([routing](https://docs.litellm.ai/docs/routing), [reliability](https://docs.litellm.ai/docs/proxy/reliability)); [OpenRouter docs](https://openrouter.ai/docs/guides/routing/model-fallbacks); [9router](https://github.com/decolua/9router) at 1855aa4; [OmniRoute](https://github.com/diegosouzapw/OmniRoute) at c5c90b9; claude-code-router 3.1.3 ([routing](https://ccrdesk.top/routing/)).

## Where Pygg is worse

- **Throughput:** 230 vs LiteLLM's [published](https://docs.litellm.ai/docs/benchmarks) 1,170 req/s; built for one person's tools.
- **Provider fails after receiving:** 96% / 0% served by default vs LiteLLM 100%; 5xx retry per key (Endpoint → API Keys → Limits); dropped connections never retried.
- **Provider hangs:** waits up to 10 minutes, no fallback on timeout.
- **Stream cut mid-answer:** not continued on another model.
- **Web sessions are slow by design** ([the guard](PROVIDERS.md#the-guard)).
- **`auto/cheap` can pick too weak a model** (2 / 6); `auto` with a tier floor is the agent default.
- **Platforms:** desktop app Windows x64 only; CLI not yet run on a Mac.

## What Pygg does not claim

- **No "60–95% fewer tokens"**: an [independent replay](https://codepointer.dev/p/cutting-llm-token-costs-with-rtk) found 3.7%; Pygg saves via provider cache, cheaper models, visible routing (ADR-036).
- **Not a quality benchmark**: a handful of tasks per case.
- **Not a provider bill**: ledger costs at list prices.
- **Not a ranking of other projects**: D cells are self-descriptions on one day.
