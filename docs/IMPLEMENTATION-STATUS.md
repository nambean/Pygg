# Implementation status

> **Status:** Authoritative · what is built and how it was verified · reviewed 2026-11-21

Next: [ROADMAP](ROADMAP.md); why: [DECISIONS](DECISIONS.md). Code under `apps/desktop/src-tauri/src` unless stated.

| Verification level | Meaning |
| --- | --- |
| Unit | The module alone |
| Fake provider | Through a real gateway against a fake provider |
| Live | A real provider answered through the gateway |
| Preview | The interface on demo data (`pnpm dev`, ADR-014) |
| ui-check | `pnpm ui-check`: every screen at five sizes with `?stress` data; fails on overflow |

**CI:** clippy (both feature sets), fmt, tests, UI typecheck and build, schema, licence and layout checks; `cargo audit` and `pnpm audit` weekly.

## P0: cost-safe gateway

| Item | Verified |
| --- | --- |
| Same-format passthrough | Live; Anthropic cache parity on three models |
| Error-scoped fallback | Live (Gemini, OpenCode); real error fixtures from 8 providers |
| Budgets and complete ledger | Live (Gemini, Anthropic) |
| Explicit sessions | Live: headless Claude Code, 12 of 12 requests |
| `auto` routing | Live: Gemini, Anthropic, Qwen, headless Claude Code |

All built and fake-provider tested; none release-ready: see the [release gates](ROADMAP.md#release-gates).

## Gateway (`gateway/`)

| Part | Built | Verified |
| --- | --- | --- |
| Server | Loopback `127.0.0.1:8741`; client key required; Chat Completions, Messages, Responses, `/health` | Fake provider; live |
| One gateway per store | OS lock; a second gateway refused before sending | Two real processes |
| Same-format passthrough | Only `model` replaced; headers, cache and thinking fields pass unchanged | Live parity, three Anthropic models |
| Anthropic → OpenAI | Text, images, tools, streaming, thinking; dropped fields named in `x-pygg-dropped` | Unit; live (Gemini, Qwen) |
| OpenAI → Anthropic | Messages, tools, thinking from `reasoning_effort`; agent turns get prompt caching | Unit; live (Claude Haiku) |
| Responses API (ADR-033) | `/v1/responses` on the same routing; stored-response features refused | Live with Codex CLI |
| Streaming | Passed through; a broken stream recorded at unknown cost | Fake provider; live |
| Gemini signatures | `thought_signature` kept per tool call and put back for Gemini | Unit; live |
| Explicit sessions | `x-pygg-session` pins to the last answering target, 2 hours sliding | Live; way home fake only |
| Cache marker (ADR-036) | Top-level `cache_control` on agent turns to Anthropic or OpenRouter `anthropic/…` | Live: [Anthropic](../benchmark/reports/2026-11-14-codex-claude-prompt-cache.md), [OpenRouter, 99.7% read](../benchmark/reports/2026-11-16-openrouter-cache-live.md) |
| Error-scoped fallback | Only before the first byte; 401 blocks, 402/403/429 cool; timeouts stop at unknown cost | Real fixtures; live (Gemini, OpenRouter, DeepSeek) |
| Circuit breaker | Wait doubles per failure, up to 15 minutes | Unit |
| Overhead | Median ~4 ms per request; ~230 requests/s with 16 clients | [Report](../benchmark/reports/2026-11-14-gateway-overhead.md) |

## Routing: named models, combos and `auto`

| Part | Built | Verified |
| --- | --- | --- |
| Model names | `<connection>/<model>`, `<pool>/<model>`, a combo, `auto`, `auto/cheap` | Live |
| Combos | Members in order, or cheapest first | In order live; cheapest fake only |
| Combo templates | Eight templates resolved into a saved list with reasons | Preview |
| `auto`, `auto/cheap` (ADR-006) | Hard filters with reasons, then rule ranking; nothing eligible → 422 | Live |
| Role aliases (ADR-041) | `auto/plan`, `auto/code`, `auto/test`, `auto/review`: own evidence, then facts and tiers | Fake provider; not measured |
| 429 hold | Repeated 429s keep a model out of `auto` 10 minutes, doubling to a day | Fake provider |
| Model facts | Tier, context, output, tools, images, thinking, each with a source | Unit |
| Stated quotas | Rate-limit headers and balance endpoints read; nothing estimated | Unit |
| Simulator | Combos → How auto chooses: route and exclusions; sends nothing | Preview |

## Policy, budgets and the ledger

| Part | Built | Verified |
| --- | --- | --- |
| API keys | Hashed; per-key limits, in-flight cap, minimum tier, no-training, allowed connections | Fake provider |
| Budgets (P0-3) | Each attempt journaled before sending; strict limits hold the worst case | Fake provider |
| Session and day limits | 402 naming the limit; a running request never cut | Live on a store copy |
| Ledger and cost status | Every attempt; Actual, Estimated or Unknown, never $0 for unknown | Live |
| Data-use terms | Per connection, with a source; a client can require no training | Fake provider |
| Usage screen | Summary, Requests, Subscriptions, Local Runtime; CSV export | Unit; preview |
| Estimated savings | Routed cost against one user-picked baseline model | Unit; not on a real ledger |
| Usage alerts | Limit, spend spike, unknown costs, failures | Unit; preview |
| Repeated tool output (shadow) | Repeats counted, never removed | Unit; preview |
| Cache lifetime (shadow, ADR-036) | What a one-hour TTL or keep-alive would cost and save | ui-check; not on real sessions |
| Clearing old tool results (shadow, ADR-036) | What Anthropic's context editing would clear and cost; clears nothing | ui-check; not on real sessions |
| Retention | Ledger kept 30, 90, 365 days or forever (default) | Unit |

## Key rings and pools

| Part | Built | Verified |
| --- | --- | --- |
| Key rings (ADR-038) | Separate accounts on one connection; the next key answers after 401/402/429 | Fake provider; not with real providers |
| Pools (ADR-034) | Connections with independent quotas, tried in order | Unit; preview |

## Web sessions and the guard (`websession/`, ADR-035)

| Part | Built | Verified |
| --- | --- | --- |
| Web-session connections | Muse, ChatGPT, Perplexity, DeepSeek, Grok, Gemini in WebView2; never in `auto` | Live, all six; [47 of 48 tasks](../benchmark/reports/2026-11-13-web-sessions.md) |
| Media from replies | Pictures, videos and links returned as Markdown links | Live (Muse, Gemini) |
| The guard | Over-long prompts refused (413); rests and caps ([numbers](PROVIDERS.md#the-guard)) | Live 413; rests not met live |
| Waits | First word: 60 s + 1 s per 100 prompt tokens, up to 240 s | Unit |

## Providers and the catalog

| Part | Built | Verified |
| --- | --- | --- |
| Presets | Cloud, local and custom endpoints ([PROVIDERS](PROVIDERS.md)); keys in the OS credential store | Live model lists |
| Free-model connections (ADR-032) | Only $0 models on; an out-of-credit 402 does not hold them | Fake provider |
| Sign in with OpenRouter | OAuth PKCE; issued key kept like a typed one | Unit; not live |
| Checks and health strip | Model lists every 30 minutes; 24-hour dots and a score | Unit; preview |
| Auto readiness | How many models `auto` can use, and why not the rest | Unit; preview |
| Test Model | One short request through the real route; paid asks first | Live; combo not live |
| Model reliability | Answer share and median time over 30 days | Unit; preview |
| Model facts feed (ADR-040) | Signed weekly per-role scores, verified daily ([BRAIN](design/BRAIN.md)) | Live on a store copy |

## Skills & Prompts (`skills.rs`, ADR-039)

| Part | Built | Verified |
| --- | --- | --- |
| Library | Eight starter prompts plus the user's own; Agent Skills format | Unit; preview |
| Install and remove | `SKILL.md` into `~/.claude/skills` or `~/.agents/skills`; removes only Pygg's copies | ui-check; not in real agents |
| Sent With Every Request | Per agent: global instructions and skills, with token estimates | Unit; preview |
| Projects | User-added folders as install targets; home folder refused | ui-check; not from a real agent |

## CLI, npm and coding-tool set-up

| Part | Built | Verified |
| --- | --- | --- |
| `pygg-cli` | One console program ([CLI](design/CLI.md)); store read-only; Linux and macOS builds | Unit; installed store |
| npm (ADR-037) | `npm install -g @pygg/cli@dev`; newest `0.1.0-dev.8` (2026-11-20), in the MCP Registry as `io.github.nambean/pygg` | Windows, Linux installed; macOS CI only |
| One-click set-up (ADR-022) | Codex, OpenCode, Claude Code launcher, MCP for three agents ([AGENTS](AGENTS.md)) | Live on scratch configs |
| Launch sessions | `pygg run claude\|codex\|opencode`: a fresh session per launch | Live up to the gateway |
| Session receipt and status line | Cost, cache, models, Git changes on exit; `pygg statusline`, `pygg launches --by-repo` | Unit; not live |
| Providers and models from the terminal | `providers`, `models`, `combos`, `keys` and more; web sessions refused | Live on a scratch store |
| Chat from the terminal | `pygg chat`, `pygg ask`; nothing kept (ADR-028) | Live on a scratch store |
| Request classes | Claude Code's hint headers recorded, never forwarded | Unit; not on a real session |
| Claude Code by hand | Base URL, client key, session header | Live, 12 of 12 requests |

## Desktop app and interface

| Part | Built | Verified |
| --- | --- | --- |
| Screens | Home, Providers, Endpoint, CLI Tools, Combos, Usage, Policies, Skills & Prompts, Guide, Settings; Smart Tasks: Work (needs you, running, recent), Projects (work, board), New work (ask, task, plan), plan page (plan → tasks → output), task page (overview, activity, changes) | Preview; ui-check; native screenshots |
| Start-up and tray | Gateway starts with the app; closing keeps it in the tray | Native app, scratch store |
| Diagnostics | One redacted zip ([SECURITY](SECURITY.md)) | Unit |
| Look and name | Green accent, light and dark; traffic-light controls (ADR-012) | Preview; snap layouts at 100% only |
| Language | English; Vietnamese incomplete | Rust template test |
| Installer | NSIS and MSI; prereleases `v0.1.0-dev.1` to `v0.1.0-dev.3` (2026-11-20) | Dev PC only; unsigned |

## Smart tasks (experimental)

On hold. Design: [design/BRAIN.md](design/BRAIN.md); task API: [AGENTS](AGENTS.md).

| Part | Built | Verified |
| --- | --- | --- |
| `pygg/brain` (ADR-041) | A requirement becomes questions, cards and waves, with models per card | Live on a store copy |
| The board (ADR-042) | Approved cards pulled into tasks under WIP limits; paid steps wait | Live in the dev build |
| Agent CLIs (ADR-026) | Claude Code, Codex, Gemini CLI, OpenCode with their own sign-in, in a worktree | Live (Codex, OpenCode); Gemini refused by Google |
| Task engine | Agent steps against approved checks; paid steps wait with a cap (ADR-010) | Live; a $0.0093 paid step |
| From the IDE (ADR-027) | MCP task tools; only keys with **May start tasks**; no API approves paid steps | Live from Codex and Claude Code |
| Quality signals (ADR-029) | Retry, keep and discard events, summed by `agent quality` | Unit |

## Benchmark harness

| Part | Built | Verified |
| --- | --- | --- |
| `bench` | Cases through agents or tasks; ledger cost; hard `--budget-usd` ([BENCHMARKS](BENCHMARKS.md)) | Its runs |
| Web scripts | `scripts/bench-web.py`, `scripts/bench-web-agent.py` | Its runs |
| Reports | All listed in [BENCHMARKS](BENCHMARKS.md) | — |

## Live runs on record

| Provider | What answered |
| --- | --- |
| Anthropic | Native with cache parity; both translations; Codex and Claude Code; benchmarks |
| Gemini | Both formats; tool signatures; combo fallback; `auto` |
| Qwen token plan | Both formats; reasoning; Codex tools; benchmarks |
| OpenRouter | A free model; a real 429 |
| Ollama (Docker, GPU) | Both formats; benchmarks with `qwen3:8b` |
| DeepSeek API | A real 402 |
| OpenAI, Groq, Mistral, others | Real 401s only |
| Web sessions | All six sites |

## Not verified yet

| Area | Not yet |
| --- | --- |
| Installer | Upgrade from Explorer; a clean Windows machine; macOS |
| Native window | Every screen clicked; scaling, snap above 100%, screen readers, contrast themes |
| Limits | Session and day limits in the installed app |
| Real sessions | Launch receipts, request kinds, `pygg run codex/opencode`, `pygg chat` on the owner's connections |
| Skills | Installed into real agents and used |
| Providers | Successful OpenAI, Groq, DeepSeek, Mistral requests; non-Anthropic Anthropic-format endpoints |
| Caching | OpenRouter `x-session-id`, OpenAI `prompt_cache_key` (sent, not measured) |
| Errors | Gemini 429 fixture; 429 hold and quota-group hold against real providers |
| Routing extras | Key rings, cheapest-first combos, savings on a real ledger |
| Web sessions | Long-term use; the guard's rests live; a two-account pool |

## Designed, not built

- Subscription quota tracking (providers state nothing readable).
- The `image` role and some board screen parts ([BRAIN](design/BRAIN.md#410-designed-not-built)).
- Vietnamese for strings added since the first pass (last).
