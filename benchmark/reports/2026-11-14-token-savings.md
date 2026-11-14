# Tokens and cost per request: Pygg against LiteLLM, and every paid run with confidence intervals, 2026-11-14

> **Status:** Record · $0 spent for this report (a recording fake provider, and the raw results of earlier paid runs) · raw results: [`results/2026-11-14-cache-replay`](../results/2026-11-14-cache-replay/) · scripts: `benchmark/resilience/cache_replay.py`, `scripts/bench-ci.py` · decisions: [ADR-036](../../docs/DECISIONS.md) (automatic prompt caching)

**Question.** What does Pygg change in the tokens a coding agent pays for, compared with another gateway run locally on the same workload, and how sure are the earlier paid results?

**Answer.**
- **Prompt cache for OpenAI-format agents on Claude.** A 15-turn agent session replayed through each gateway to a fake Anthropic API that bills like Anthropic's prompt cache: Pygg as shipped read **89.5%** of the prompt from the cache and paid **$0.0042** for input instead of **$0.0191** (78% less). LiteLLM with its defaults read **0%**. LiteLLM with its documented `cache_control_injection_points` setup matched Pygg (**89.5%**) when the agent sent Chat Completions (OpenCode and most agents), but read only **39.7%** when it sent the Responses API (Codex), because its marker on "the last message" stayed on the task message while the tool results grew after it.
- **Output limits.** When the agent sets no `max_tokens` (Codex and OpenCode do not), Pygg sends 64,000 for a streamed turn and 16,000 for a whole one, at most the model's maximum as configured; LiteLLM sent 128,000 for both. Neither truncated anything here; a limit that is too low fails long answers (Pygg's old 4,096 made Codex fail three hard tasks on 2026-11-14), and one above the model's real maximum is refused by some providers.
- **Earlier paid runs, now with intervals.** Pygg's measurable savings come from the model you route to and from keeping the cache working, not from changing what the agent sends. Claude Haiku 5.5 instead of Sonnet 5.5 cost **$0.0133 against $0.1142 per solved task** (95% intervals $0.0118–0.0150 and $0.1019–0.1297) with OpenCode; Codex through Pygg's cache cost **$0.0060 against $0.0086** per solved run without it (intervals overlap: $0.0033–0.0099 and $0.0067–0.0113, 16 runs each).

Where Pygg is not better: LiteLLM does the same caching for Chat Completions clients once configured; Pygg adds the marker only for Anthropic's own API (a connection of the `anthropic` kind), not for the Anthropic-format endpoints of DeepSeek, Kimi, Z.AI or MiniMax; and it adds nothing for clients that already mark their cache, such as Claude Code, whose requests it passes through unchanged.

## 1. Prompt-cache replay

### Method

`benchmark/resilience/cache_replay.py`:

```text
python -I benchmark/resilience/cache_replay.py --api chat      --pygg-build 80d35d2 --out benchmark/results/2026-11-14-cache-replay
python -I benchmark/resilience/cache_replay.py --api responses --pygg-build 80d35d2 --out benchmark/results/2026-11-14-cache-replay
```

- **The session** is synthetic and deterministic (seeded): a 16,000-character system prompt, six tool definitions, a task, then 15 tool calls each answered with about 4,000 characters of file content. Turn *k* sends the whole conversation so far with the tools, streamed, without `max_tokens`, as an OpenAI-format agent does. It is sent once as Chat Completions and once as the Responses API (Codex's format).
- **The provider** is `benchmark/resilience/fake_provider.py`'s Anthropic Messages API, in a `python:3.11-slim` container. It cuts each prompt into blocks (tools, system, then each message's content blocks), treats a block's `cache_control` or a top-level `cache_control` (Anthropic's automatic caching: the last block) as a breakpoint, writes the prefix up to each breakpoint to its cache and reads the longest written prefix a later prompt starts with. Tokens are characters / 4. It records the markers and `max_tokens` each request carried.
- **Prices:** Claude Haiku 5.5 as recorded in the [2026-11-14 Codex report](2026-11-14-codex-claude-prompt-cache.md): $0.10 input, $0.01 cache read, $0.125 cache write per million tokens. Output is the same in every arm and is left out.
- **Arms:**
  - `pygg`: the headless gateway built from commit 80d35d2, an Anthropic connection pointed at the fake, as shipped.
  - `pygg-cache-off`: the same with `PYGG_ANTHROPIC_AUTO_CACHE=off`, what a plain translating gateway sends.
  - `litellm`: LiteLLM proxy `ghcr.io/berriai/litellm:v1.106.2` (digest `sha256:7294b90f…3a03f7`), model `anthropic/claude-haiku-5-5` with the fake as `api_base`, defaults ([`litellm-replay.yaml`](../resilience/litellm-replay.yaml)).
  - `litellm-inject`: the same with `cache_control_injection_points` on the system message and on the last message (index −1), as LiteLLM's [prompt-caching tutorial](https://docs.litellm.ai/docs/tutorials/prompt_caching) shows.

### Results

| Agent's API | Arm | Prompt tokens (15 turns) | New input | Cache reads | Cache writes | Read from cache | Input cost |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Chat Completions | **Pygg** | 190,831 | 0 | 170,777 | 20,054 | **89.5%** | **$0.0042** |
| Chat Completions | plain gateway (Pygg, cache off) | 190,831 | 190,831 | 0 | 0 | 0% | $0.0191 |
| Chat Completions | LiteLLM, defaults | 191,251 | 191,251 | 0 | 0 | 0% | $0.0191 |
| Chat Completions | LiteLLM, injection points | 191,251 | 0 | 171,169 | 20,082 | 89.5% | $0.0042 |
| Responses (Codex) | **Pygg** | 190,831 | 0 | 170,777 | 20,054 | **89.5%** | **$0.0042** |
| Responses (Codex) | plain gateway (Pygg, cache off) | 190,831 | 190,831 | 0 | 0 | 0% | $0.0191 |
| Responses (Codex) | LiteLLM, defaults | 191,251 | 191,251 | 0 | 0 | 0% | $0.0191 |
| Responses (Codex) | LiteLLM, injection points | 191,251 | 109,891 | 75,936 | 5,424 | 39.7% | $0.0124 |

- **How Pygg marks it:** one top-level `cache_control` on each turn that carries tools (one of Anthropic's four breakpoints), so the cached prefix moves forward with the conversation.
- **How LiteLLM's injection points mark it:** a block marker on the system message and one on the last message. Through Chat Completions the last message is the newest tool result, so the cache moves forward the same way. Through the Responses API the marker stayed on the task message on every turn (the per-turn records in `litellm-inject.responses.turns.jsonl` show the cache read fixed at 5,424 tokens, the system prompt, tools and task), so each turn's tool results were billed as new input.
- **The real thing:** the [Codex run of 2026-11-14](2026-11-14-codex-claude-prompt-cache.md) measured Pygg's marker on Anthropic's real API: 84% of input read from cache, 68% less on input, 30% less in all, over 16 task runs.

### Output limits

The same short question without `max_tokens`, whole and streamed, and the agent's turns (streamed):

| Gateway | Whole request | Streamed | Source of the value |
| --- | ---: | ---: | --- |
| Pygg | 16,000 | 64,000 | Pygg's defaults, held at the model's maximum output as configured (64,000 here) |
| LiteLLM 1.106.2 | 128,000 | 128,000 | LiteLLM's model catalog for `claude-haiku-5-5` |

A limit is only a ceiling; the model stops when it is done and is billed for what it wrote. These runs say what each gateway sends, not which value a given model accepts.

## 2. Earlier paid runs with confidence intervals

`scripts/bench-ci.py` over the raw results already published (no new spend). Pass rate with a Wilson 95% interval; cost and tokens per solved run (all runs' cost, failed ones included, divided by runs passed) with a percentile bootstrap 95% interval over runs (10,000 resamples, seeded). Tokens are those the gateway's ledger recorded, cache reads and writes included. Costs are list-price estimates from each gateway's ledger, as each report states its prices.

| Run | Agent | Route through Pygg | Passed (95% CI) | $ per solved (95% CI) | Tokens per solved (95% CI) | Input from cache |
| --- | --- | --- | --- | --- | --- | ---: |
| [N2](2026-11-12-n2-savings.md) | OpenCode | pinned Sonnet 5.5 | 34/36 (82–98%) | $0.1142 ($0.1019–0.1297) | 89.6K (80.7–100.5K) | 70% |
| N2 | OpenCode | pinned Haiku 5.5 | 36/36 (90–100%) | $0.0133 ($0.0118–0.0150) | 102.5K (92.8–112.9K) | 72% |
| N2 | OpenCode | `auto` (chose a Qwen subscription model: quota, not money) | 35/36 (86–99%) | $0 metered (329 requests of quota) | 180.0K (138.3–230.9K) | 91% |
| N2 | OpenCode, task engine | cascade: a free hosted model, then Qwen (quota), Haiku, Sonnet | 36/36 (90–100%) | $0.0033 metered, plus quota; every run had an attempt of unknown cost | 119.1K (102.8–137.0K) | 80% |
| N2 | OpenCode, task engine | cascade: local qwen3:8b, then Haiku, Sonnet | 36/36 (90–100%) | $0.0138 ($0.0121–0.0156) | 183.0K (153.2–219.6K) | 77% |
| [P0](2026-11-12-p0-optimization.md) | Claude Code | pinned Sonnet 5.5 | 11/12 (65–98%) | $0.0809 ($0.0691–0.0978) | 103.3K (86.2–133.1K) | 88% |
| P0 | Claude Code | pinned Haiku 5.5 | 35/36 (86–99%) | $0.0168 ($0.0145–0.0195) | 131.9K (116.0–151.4K) | 88% |
| P0 | OpenCode | pinned Haiku 5.5 | 12/12 (76–100%) | $0.0123 ($0.0101–0.0148) | 91.8K (77.5–109.8K) | 68% |
| P0 | OpenCode | `auto/cheap` | 34/36 (82–98%) | $0.0129 ($0.0110–0.0150) | 98.1K (85.6–112.5K) | 70% |
| P0 | OpenCode | `auto` (picked Gemini 2.5 Flash) | 5/12 (19–68%) | $0.0298 ($0.0173–0.0686) | 1.2M (0.14–5.4M) | 92% |
| [CCHT](2026-11-13-ccht-real-repository.md) (private repo) | Claude Code | pinned Haiku 5.5 | 6/6 (61–100%) | $0.2042 known ($0.0562–0.4419); one run's cost unknown | 2.36M (0.94–4.40M) | 96% |
| CCHT | Claude Code | pinned Sonnet 5.5 | 5/10 (24–76%) | $0.2753 ($0.1555–0.5678) | 405K (221–810K) | 89% |
| CCHT | OpenCode | `auto/cheap` | 3/8 (14–69%) | $0.0728 known; five runs' cost unknown | 1.89M (0.96–6.29M) | 87% |
| [Codex cache](2026-11-14-codex-claude-prompt-cache.md) | Codex | Haiku 5.5, cache markers off | 16/16 (81–100%) | $0.0086 ($0.0067–0.0113) | 69.0K (58.1–84.8K) | 0% |
| Codex cache | Codex | Haiku 5.5, **Pygg's cache markers** | 16/16 (81–100%) | $0.0060 ($0.0033–0.0099) | 80.7K (63.7–102.1K) | 84% |

These are per-run intervals; the reports' paired savings (for example Haiku 79% cheaper than Sonnet with Claude Code, 74–84%) pair the same tasks and are tighter. The CCHT per-task figures differ from that report's totals where a run's cost was unknown: this table counts known cost only and says so.

## 3. What was not run, and why

- **Paid runs through LiteLLM.** The difference is in the markers each gateway sends, which the recording fake shows exactly; Anthropic bills what is marked. Running LiteLLM on the owner's Anthropic key would also have meant putting that key into a third-party container shortly after LiteLLM's [2026-10-21 incident](https://docs.litellm.ai/blog/security-incident-oct-21-2026), when its Docker tags pointed at a malicious image. The remaining budget (about $3.3 of $20) was kept.
- **OpenRouter.** Its [prompt-caching guide](https://openrouter.ai/docs/guides/best-practices/prompt-caching) (read 2026-11-14) says Claude models are cached only when the client sends `cache_control`; an OpenAI-format agent sends none, so through OpenRouter alone it would match the "plain gateway" rows. Not measured: it needs a paid key.
- **9router, OmniRoute, claude-code-router.** None documents adding cache markers for OpenAI-format clients (sources in [docs/BENCHMARKS.md](../../docs/BENCHMARKS.md)). They were not run.
- **A local model through each gateway.** Local models (Ollama) have no prompt cache billing and no price, so the result would show only that a gateway does not change what the agent sends.

## Limits

- The replay is one synthetic session; real sessions vary in length and in how much each turn adds. The share read from cache grows with the number of turns.
- The fake's cache follows Anthropic's documented rules in a simplified form: no minimum cacheable length (every prompt here is far above it), no 20-block look-back limit, no expiry (the replay takes seconds).
- One version of each gateway.
