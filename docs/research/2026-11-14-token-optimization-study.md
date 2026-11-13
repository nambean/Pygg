# Token optimisation in OmniRoute and other open-source tools, and what Pygg could build, 2026-11-14

> **Status:** Research · the owner asked to keep studying OmniRoute and open-source token-saving tools and to discuss features and UI/UX with Codex · discussion: [Codex review 5](../reviews/2026-11-14-codex-token-optimization-discussion.md) · earlier: [OmniRoute feature research](../OMNIROUTE_FEATURE_RESEARCH.md), [OmniRoute docs study](2026-11-13-omniroute-docs-study.md), [ROADMAP P1-4](../ROADMAP.md)

Upstream figures below are the projects' own claims unless marked as measured. None was run here.

## 1. What the tools do

| Tool | Where it sits | What it removes | Recovery | Licence |
| --- | --- | --- | --- | --- |
| **OmniRoute** v3.8.54 ([repo](https://github.com/diegosouzapw/OmniRoute), [compression engines](https://github.com/diegosouzapw/OmniRoute/blob/main/docs/compression/COMPRESSION_ENGINES.md)) | The gateway | Modes `off`, `lite`, `standard`, `aggressive`, `ultra`, `rtk`, `omniglyph`, `stacked`. Its doc is inconsistent about the default: the mode table says `session-dedup → lite` (text already seen earlier in the session elided, then filler removed), the stacked-pipeline section says `rtk → caveman`. Engines: Caveman (prose), RTK (49 JSON filters for command output), LLMLingua-2 (token pruning, optional), CCR (large blocks replaced by `[CCR retrieve hash=…]`), SmartCrusher (JSON arrays to columns), ionizer (head/middle/tail sampling), an MCP result filter (on by default, truncates over 50,000 characters), OmniGlyph (context sent as images) | CCR through a retrieval tool, when the caller's tools include it | MIT |
| **RTK** ([repo](https://github.com/rtk-ai/rtk)) | A Claude Code / Codex `PreToolUse` hook that rewrites `git status` into `rtk git status` | Test runs to failures only; compact git, grouped search results, linters grouped by rule; over 100 commands | `rtk recall`, `rtk bypass`; raw output when a filter fails | CLI Apache-2.0; engine and proxy BUSL-1.1 |
| **Headroom** ([repo](https://github.com/chopratejas/headroom), [LiteLLM integration](https://docs.litellm.ai/blog/headroom-integration)) | A proxy, a library or an MCP server; a LiteLLM pre-call step | In its default cache mode, only the **live zone** (new tool output, the latest turn); the earlier prefix stays byte-identical so the provider's cache survives. Token-focused modes exist too. Per content type: AST-aware code, SmartCrusher JSON, a trained prose model, logs | `headroom_retrieve` for a configured time | Apache-2.0 |
| **Caveman** ([repo](https://github.com/JuliusBrussee/caveman)) | Session hooks that tell the model to write tersely | Output prose; keeps code, paths and warnings | None needed | — |
| **LiteLLM, Portkey, Bifrost** | Gateways | Semantic response caches (embeddings, a similarity threshold) | — | various |

OmniRoute's routing side adds `cacheAffinity` (send a request to the connection likely to hold its prompt-cache prefix), **off by default** (weight 0), a `cache-optimized` strategy, a free-tier dashboard and a "Context & Cache" analytics page per engine ([auto-combo doc](https://github.com/diegosouzapw/OmniRoute/blob/main/docs/routing/AUTO-COMBO.md)).

## 2. What the measurements say

**An independent replay** ([codepointer.dev](https://codepointer.dev/p/cutting-llm-token-costs-with-rtk)) ran Headroom, RTK and Caveman over 500 real Claude Code sessions (614M tokens, $926 baseline). Together they cut spend by **3.7%** (Headroom 2.8%, RTK 0.5%, Caveman 0.4%), against 33–99% on ideal single inputs. Headroom's compressor was replayed directly; RTK's and Caveman's figures are estimates from their published reductions, and no task was re-run, so the replay says nothing about answer quality. RTK reached only 22% of tool-output tokens, because file reads bypass the shell hook. The bill was mostly **cache writes (42%) and output (29%)**, which these tools barely touch; they mostly shrink cache reads, the cheapest tokens.

**Pygg's own benchmark** (CCHT real repository, 2026-11-13, the gateway's ledger, every recorded attempt including superseded ones; the published report counts only the last attempt per case) shows the same shape. The last column is a **token-weighted cost share under an illustrative Anthropic five-minute-cache price model** (uncached input 1, cache read 0.1, cache write 1.25, output 5), not a reconstruction of the mixed-model bill. At Sonnet 5.5's current cache-read price (0.05×), Sonnet's split is about 15% / 46% / 40%:

| Run | Requests | Input tokens | of which cache reads | Cache writes | Output | Bill: cache read / cache write / output / uncached |
| --- | --- | --- | --- | --- | --- | --- |
| Claude Code, Haiku | 176 | 13.98M | 13.41M (96%) | 0.57M | 0.18M | 45% / 24% / 31% / 0% |
| Claude Code, Sonnet | 55 | 1.98M | 1.76M (89%) | 0.22M | 0.05M | 26% / 40% / 35% / 0% |
| OpenCode, `auto/cheap` | 152 | 5.60M | 4.85M (87%) | 0.25M | 0.06M | 30% / 19% / 20% / 31% |

The aggregate counters do not say which model the OpenCode run's uncached input went to (Gemini Flash-Lite cost $0.20 in that run, so it was not all free). Every OpenCode task tried two or three models (Gemma, Gemini Flash-Lite, Claude Haiku) after 18 Gemma 429s and 5 Flash-Lite 404s. The records show which targets were tried, not the order of answered switches, so they do not prove each switch was a cold restart: a failure before the first answer is route selection, and a return to an earlier model may find its cache warm. The installed app's own ledger (234 requests, mostly live checks) is too small to say anything, and `repeated_tokens` was zero there.

**What follows for Pygg**

1. Compressing what agents read saves little money once prompt caching works; the large items are **cache writes, output, and cache lost by switching models**.
2. A rewrite of earlier history breaks the cached prefix and can cost more than it saves. Only the live zone may be touched, as Headroom does, and ROADMAP P1-4 already says.
3. Every saving claim needs the token mix and the cache effect beside it. Pygg's ledger already records input, cache read, cache write and output per request; Usage does not show the mix.

## 3. What Pygg already has

- The cache hint passes through on the same format, and `cache_control` reaches OpenRouter on translated requests (`presets::accepts_cache_control`); a dropped hint is reported.
- Explicit sessions are pinned to the model that answered (`x-smarttaskrouter-session`), for two hours since last use per client, session and requested route. `pygg run claude` already sends a fresh id per launch (`cli/run.rs`, `launcher.rs`). The OpenCode and Codex setups map the header to `PYGG_ATTEMPT_ID`, which is set only inside Pygg tasks, so their ordinary launches send none; the manual OpenCode example on Home uses the literal `opencode`, which groups unrelated conversations. The decision of 2026-11-11 says never to derive a session from a prompt hash ([UNIVERSAL_SHARED_MEMORY.md](../UNIVERSAL_SHARED_MEMORY.md)).
- Repeated tool output is measured in shadow (`gateway/shadow.rs`): identical tool-output text repeated within one request, 200 characters or more, at characters / 4. It does not know the cache position. Nothing is removed.
- A dropped cache hint is returned in a response header, not kept for Usage; top-level automatic caching is not detected.
- Savings are an estimate against a chosen baseline with coverage counts ([Codex review 3](../reviews/2026-11-13-codex-screens-savings-npm-review.md)).

## 4. Candidates for discussion

| # | Candidate | Why | Risk |
| --- | --- | --- | --- |
| T1 | **Token mix on Usage**: uncached input, cache read, cache write and output, as tokens and as cost, per client and model; cache hit rate | Shows where the bill is before anything is changed; the data is recorded | None (read-only) |
| T2 | **Cost of switching**: count switches inside a session and the cache writes after each, "cache lost by fallback" | The OpenCode run's switches are invisible today | None (read-only) |
| T3 | **A session per launch** for OpenCode and Codex as `pygg run claude` already has (OpenCode provider `headers`, Codex `env_http_headers`): an explicit id, not a prompt hash | Keeps a tool's conversation on one model and its cache, within the 2026-11-11 decision | Low; a stuck pin falls back as today |
| T4 | **Cache-aware fallback order**: after a failure, prefer a model of the same provider family or one that already holds this session's cache; say in the ledger what the switch cost | Fewer cold restarts | Medium: changes routing |
| T5 | **Live-zone tool-output filter, shadow first**: deterministic filters on the newest tool result only (ANSI, progress bars, repeated lines, passing tests collapsed), measured as "would have removed N tokens, about $X counting the cache write and later reads", then opt-in per client | The only compression that does not break the cache | Medium: quality; needs the measurement first |
| T6 | **Concise-output opt-in** (Caveman's idea) as a per-client instruction appended where it keeps the prefix, with a holdout to measure | Output is about a third of the bill | Quality, and it changes what the user asked the agent |
| T7 | Semantic response cache | Common in gateways | Rejected for coding agents: a near match is a wrong answer |
| T8 | History dedup (OmniRoute's `session-dedup`), CCR retrieval markers, context as images | Upstream defaults | Rejected as defaults: rewrite history, break the prefix, inject protocol text the agent did not ask for |

## 5. Second pass: routing and caching projects (2026-11-14)

| Project | What it does | What Pygg takes |
| --- | --- | --- |
| **RouteLLM** ([repo](https://github.com/lm-sys/RouteLLM), Apache-2.0) | A learned router (matrix factorisation, BERT, a causal LLM) estimates whether the strong model would win on a prompt and sends it there only above a threshold; up to 85% cheaper at 95% of GPT-4's quality on MT-Bench, trained on Arena preferences for one model pair | Not built: it decides per prompt, and in an agent loop each switch forfeits the cache (caches are per model). Pygg chooses per session |
| **claude-code-router** ([repo](https://github.com/musistudio/claude-code-router), MIT) | A local gateway for Claude Code with routing conditions on headers and bodies, ordered fallbacks and cost estimates in its logs | The same place in the stack as Pygg; nothing new to borrow beyond what Pygg has |
| **LiteLLM prompt caching** ([docs](https://docs.litellm.ai/docs/tutorials/prompt_caching)) | `cache_control_injection_points`, and a flag that adds Anthropic cache checkpoints for clients that never set them; never double-injects; warns that injecting on Gemini switches it to paid explicit caching | **Built (ADR-036)**: Pygg turns on Anthropic's automatic caching for translated agent turns to Anthropic's own API, never on Gemini or other providers |
| **Anthropic's caching guidance** | A top-level `cache_control` caches up to the last block and moves with the conversation (one of four breakpoints, 1.25x write, read at 0.05-0.1x); try the stronger model at lower effort before a cascade, because a cascade forfeits cache reuse | The basis of ADR-036 |

The settled approach is [ADR-036](../DECISIONS.md): keep the cache, route per session, measure before compressing.
