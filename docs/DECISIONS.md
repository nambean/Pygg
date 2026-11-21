# Decisions

> **Status:** Current · architecture decision records (ADR-001 to ADR-042) · reviewed 2026-11-21

ADR numbers never change; a superseded or withdrawn record keeps one line pointing to its replacement. Order: [ROADMAP](ROADMAP.md); built: [Implementation Status](IMPLEMENTATION-STATUS.md); screens: [design/SCREENS.md](design/SCREENS.md).

## ADR-001 — Windows first, macOS next

*Accepted.*
- The desktop app is built and tested on Windows first; macOS builds but is not release-tested; Linux is best effort.
- The headless CLI ships for Windows, macOS and Linux (ADR-037).

## ADR-002 — Local first

*Accepted.*
- Credentials, the ledger, task history and workspaces never leave the user's machine; no account or hosted service in the core path.

## ADR-003 — Route at task and session boundaries, never mid-session

*Accepted.*
- Routing happens only when a task, step or session starts; the gateway never switches models silently inside a session.
- Fallback only on hard errors before any output reached the client.

## ADR-004 — A localhost gateway, loopback only, with client tokens

*Accepted.*
- The gateway listens on `127.0.0.1`; binding beyond loopback needs an explicit setting.
- Every request needs a scoped client token (an "API key" in the app) with its own limits.

## ADR-005 — One Tauri crate with modules; same stack as DownFetch

*Accepted.*
- Tauri 2, one Rust crate with the core, gateway and agent runner; the window is the Cargo feature `app`, so the CLI builds without it.
- React 19, Vite, TypeScript, plain CSS tokens, a pnpm workspace, no Node sidecar; provider adapters written in-house.

## ADR-006 — Rule-based routing

*Accepted; amended by ADR-041 and for measured length and cache.*
- `auto` ranks by fixed rules: tier table T0–T4, a list-price tier for unknown models (ADR-031), a minimum tier per client, eligibility filters; the UI says "lowest eligible tier under your policy", never "cheapest".
- **Amended:** `auto/cheap` ranks by expected cost from the model's own answers in the last 30 days (output length, cache-read share); `pygg/brain` classifies work while rules still pick models (ADR-041).

## ADR-007 — Deterministic checks gate the cascade

*Accepted.*
- A task moves on only on a deterministic check (typecheck, tests, build, schema); an LLM review never passes a step.
- Without an approved check there is no automatic cascade: the user reviews, and keeping records **Manually Accepted**.

## ADR-008 — A Git worktree per attempt; Git repositories only

*Accepted.*
- Each attempt runs in its own Git worktree from the task's base commit; failed attempts are discarded.
- Workspaces must be Git repositories.

## ADR-009 — Keep as Branch is the result handoff

*Accepted; brainstorms amended by ADR-042.*
- Keeping commits the result to a new branch `str/<slug>-<short-id>` without switching the user's branch.
- Alternatives: Export Patch, Discard; applying to the working tree is not built.

## ADR-010 — Approvals are durable and cover exactly one attempt

*Accepted.*
- An approval names one attempt and a spending cap, survives a restart, and is voided by a change of policy, catalog, budget or estimate.
- Nothing is approved automatically and no API can approve; notifications carry no task names, paths or approve button.

## ADR-011 — Billing types and per-fact provenance

*Accepted.*
- Billing is `Free`, `Local`, `Subscription`, `Paid` or `Unknown` per connection and model; every price, quota and capability carries its source and time.
- Unknown is never `$0.00`; subscription use is quota, never money; a list price gives an estimate, never an actual cost.

## ADR-012 — Traffic-light window controls on every platform

*Accepted.*
- DownFetch's traffic-light window controls on Windows and Linux too.
- Release checks are D12 in the [ROADMAP](ROADMAP.md#next-in-order); scaling and screen readers are open.

## ADR-013 — Benchmark before claiming savings

*Accepted.*
- Savings are advertised only from a benchmark of reproducible coding tasks with deterministic checks ([benchmark](../benchmark/README.md)).
- Paid runs stay within ADR-028's budget.

## ADR-014 — Sample data is always labeled

*Accepted.*
- The desktop app shows only real data; the browser preview (`pnpm dev`) runs a labelled demo; `?empty` shows the first-run state.
- A screen designed but not built says so.

## ADR-015 — Commit the pnpm lockfile

*Accepted.*
- `pnpm-lock.yaml` is committed.

## ADR-016 — Build the gateway first

*Accepted; absorbs ADR-019.*
- The multi-provider gateway comes first: one local base URL serving Chat Completions, Anthropic Messages and Responses (ADR-033); the task engine is a layer on top.
- Routing is per request by model name (`<connection>/<model>`, a combo or `auto`); a combo is an ordered model list that falls back on hard errors before any output.

## ADR-017 — Universal conversation memory

Withdrawn by ADR-028: no app-owned conversation store or memory. Not built.

## ADR-018 — Cache-aware model affinity for Auto Chat

Withdrawn by ADR-028 (no Auto Chat). Cache affinity for gateway clients is ADR-036.

## ADR-019 — Gateway-first order

Merged into ADR-016 (gateway first) and ADR-013 (the benchmark gates savings claims).

## ADR-020 — Entitlement tiers per account and model

Proposed, partly built: billing per connection and model (ADR-011, ADR-032); tier-grouped Providers views and an entitlement schema are not built.

## ADR-021 — Headless CLI and npm launcher

Built as ADR-037: one Rust core for the app and headless `pygg-cli`, plus an npm launcher.

## ADR-022 — Safe setup of CLIs and IDEs

*Built:* one-click setup for Claude Code, Codex CLI and OpenCode ([AGENTS.md](AGENTS.md)).
- Pygg changes a tool's configuration only when asked, after showing the change: it backs up, gives the tool its own token, checks, and can undo.
- A subscription sign-in is never routed through an unrelated client.

## ADR-023 — Multi-account rotation

Built as ADR-034 (pools) and ADR-038 (key rings); rotation never gets around bans, quotas, terms or shared limits, and never changes an active attempt or stream.

## ADR-024 — What to adopt from OmniRoute

*Accepted.*
- An OmniRoute idea is adopted only with provider-verified entitlements, no privacy or budget bypass, safe rollback and a measured cost per success.
- Not adopted: multi-model fusion, credential/TLS/browser impersonation, lossy context compression without independent benchmarks.

## ADR-025 — The gateway must not raise cost or break provider rules

*Built; tested against fake providers, Anthropic parity measured live. Amended by ADR-041.*
- Native Anthropic passthrough with prompt-cache parity, fallback only where errors allow, atomic budget accounting, explicit session affinity (`x-pygg-session`), explainable `auto`.
- The gateway never inserts memory or rewrites a prompt; only ADR-036's cache marker and `pygg/brain` requests (ADR-041) add anything.

## ADR-026 — Official agent CLIs with their own sign-in; no bridges

*Accepted; narrowed by ADR-035.*
- Pygg runs the vendors' own agent CLIs (Claude Code, Codex CLI, Gemini CLI, OpenCode) headless with their own sign-in, in a worktree per attempt.
- Pygg never reads, stores or forwards that sign-in, never bridges cookies or other products' OAuth tokens; an agent step's cost is recorded as unknown.

## ADR-027 — Tasks start from the user's IDE agent; the app monitors

*Accepted (tasks on hold).*
- Tasks start from the user's coding agent through Pygg's MCP server; the app runs them, approves paid steps and reviews results ([AGENTS.md](AGENTS.md#the-mcp-server-and-the-task-api-experimental-on-hold)).
- A client needs **May start tasks**, sees only its own tasks, and cannot approve a paid step.
- Amended 2026-11-21 (owner): a **plan** (Brain Storming) may also start in the app, from a project's page; it runs as the client **Pygg app (Brain Storming)**, under that client's limit and policy, and its questions are answered in the app. Solo tasks still start in the coding agent or **New solo task**.

## ADR-028 — MIT licence, a $20 benchmark budget, and no Auto Chat

*Accepted (owner's decisions).*
- MIT ([LICENSE](../LICENSE)); paid benchmark runs cost at most $20 in total (`bench --budget-usd`, about $16.7 used), the larger benchmark has its own $50; free, local and subscription runs don't count.
- No Auto Chat in the app (no conversation store, memory or per-turn model choice); `pygg chat` and `pygg ask` are terminal clients of the gateway; `pygg/brain` is not chat.

## ADR-029 — Quality signals in Pygg's own ledger, in TensorZero's shape

*Built.*
- TensorZero is not adopted; Pygg borrows its **metrics** and **feedback** rows keyed to a request, attempt or task (`checks_passed`, `result_kept`, `retried`, `cancelled`, `declined`, `rework_minutes`).
- An unknown signal has no row (never 0 or false); quality is never inferred from conversation content.

## ADR-030 — Task handoff within ADR-028; portable context deferred

*Built; portable context deferred.*
- A next step may get a bounded handoff from Pygg's records for that task only: `tail` (default), `none` or `evidence`; `evidence` showed no gain ([A/B](../benchmark/reports/2026-11-12-h1-handoff-ab.md)).
- Portable context between external clients stays deferred until there is evidence and a new decision on ADR-028.

## ADR-031 — The user enters a key; the app looks up the rest

*Built.*
- [models.dev](https://models.dev) fills only unknown context, maximum output, tool and image support and list price, each with its source; the provider's list and the user's settings come first; at most one download a day.
- A model off the tier table gets one from output price per million tokens: under $1 T1, under $5 T2, under $20 T3, above T4; $0 gives no tier.

## ADR-032 — A connection kept for its free models has only those on

*Built.*
- On a connection set as free, only $0-listed models are on and free; priced models are paid, unpriced ones unknown, and Pygg turns both off with the reason shown.
- A model the user turns on stays on; the ledger follows the same rule.

## ADR-033 — The gateway serves the Responses API, translated through Chat Completions

*Built.*
- `POST /v1/responses` is a third surface, translated through Chat Completions; native passthrough only for OpenAI; anything dropped is named in `x-pygg-dropped`.
- No stored responses: `previous_response_id`, `conversation` and `background` get 400; one-click setup: "Codex: models through Pygg".

## ADR-034 — Pools of separate accounts

*Built.*
- A pool groups separate accounts of one provider kind, each confirmed by the user to have its own quota; `<pool>/<model>` tries members in order, moving on for 401, 402, 403, 429 and out-of-credit 400.
- A session stays on its first-answering member; agent sign-ins and other apps' cookies are never pooled.

## ADR-035 — Web-session connections for AI sites with no API

*Built.*
- Connection kind **Web session (unofficial)** for AI sites with no API (Muse, ChatGPT, Perplexity, DeepSeek, Grok, Gemini), signed in inside Pygg's own webview; no cookie import, OAuth bridging, TLS impersonation, fingerprint spoofing or limit evasion; the user carries the account risk ([SECURITY](SECURITY.md#web-sessions)).
- Off by default, out of routing until its warning is acknowledged, never chosen by `auto`; no tool calls (400) or token counts; not in headless `pygg gateway serve`.
- The web guard cannot be loosened by any setting: prompts over 12,000 characters (DeepSeek 4,000) get 413; one request at a time per connection; rest after a reply at least 10 s (DeepSeek 30 s), a wait over 30 s gets 429 `web_pace`.
- Limits: 20 units and 15 requests per hour (DeepSeek 8 and 6), 5 requests per 10 minutes (DeepSeek 3), 50 requests a day by default, at most 100 (DeepSeek 30).
- Trouble backs off 5 min, doubling up to 6 h; a CAPTCHA or sign-in page marks it Needs you (401), a suspension notice turns it off; Pygg never solves or hides from a check.

## ADR-036 — How Pygg saves tokens: keep the cache, route per session, measure before compressing

*Built; compression measured in shadow only.*
- A session stays on one model; after a fallback it goes home only when no warm cache is lost (home answered within 5 minutes, or the fallback has not for 5 minutes).
- OpenAI-format tool turns sent to Anthropic's models get one top-level `cache_control` (30% cheaper for Codex on Claude Haiku, [report](../benchmark/reports/2026-11-14-codex-claude-prompt-cache.md)); self-caching providers and Alibaba get nothing added; `PYGG_ANTHROPIC_AUTO_CACHE=off` and `PYGG_PASS_SESSION=off` turn these off.
- The session reaches OpenRouter (`x-session-id`) and OpenAI (`prompt_cache_key`) only as a SHA-256 hash, never the raw id or prompt content.
- Usage → Token Mix shows cache share, each miss's cause and the tools and system prompt size per session; Anthropic's 1-hour cache or a keep-alive is offered only if its net stays above zero.
- Compression only in shadow: history is never rewritten; clearing old tool results is measured past 100,000 prompt tokens on OpenAI-format requests only; `effort` and thinking pass through.

## ADR-037 — One npm command for every platform

*Built.*
- `@pygg/cli` (command `pygg`) with one optional package per platform: win32-x64, darwin-arm64, darwin-x64, linux-x64 (glibc); no `postinstall` download; publishing is manual ([PUBLISHING.md](../packages/npm/PUBLISHING.md)).
- Not done: linux-arm64, musl, code signing.

## ADR-038 — Key rings: keys of separate accounts on one connection, used in turn

*Built.*
- A connection may hold keys of separate accounts (each confirmed to have its own quota), used `in_order` (default), `round_robin`, `least_used` or `quota_aware`; a session returns to its key; 401 blocks a key, 402 or 429 cools it and the next is tried.
- Never rotated: web sessions, subscription and agent sign-ins, local runtimes; UI: Providers → a connection → API keys.

## ADR-039 — Skills & Prompts: one library, installed into each agent's own skills folder

*Built.*
- A library of Agent Skills installed into Claude Code's `~/.claude/skills` and the shared `~/.agents/skills` (Codex, OpenCode); a prompt is a skill run only by name; eight MIT starters.
- Pygg never overwrites a folder it did not write or a copy edited since; downloading skills is not done.

## ADR-040 — Model facts: per-role scores with their sources, published by the project

*Built* ([design/BRAIN.md](design/BRAIN.md)).
- A signed **model facts feed**, built weekly in this repository from named public sources and fetched daily, scores models per role (`code`, `plan`, `review`, `test`, `image`) with source and date; an unverified feed is refused.
- Ranking: the user's ledger evidence above 10 attempts, then the feed, then the tier table; the feed changes suggestions, never a saved choice.

## ADR-041 — `pygg/brain`: the router is the brain

*Built* ([design/BRAIN.md](design/BRAIN.md)); the `image` role waits for an images path.
- Virtual model `pygg/brain`: a requirement in, a proposal out (questions and cards, each with a role, done criterion and a model picked by rule); a paid planner's first use waits for the user's pick; approval only in the app.
- **Amended:** evidence ranks in bands of 0.1, facts and tiers in bands of 0.15, cheaper first within a band ([BRAIN.md](design/BRAIN.md) §1); role aliases `auto/plan`, `auto/code`, `auto/test`, `auto/review`.

## ADR-042 — Smart tasks run as a Kanban board

*Built* ([design/BRAIN.md](design/BRAIN.md)).
- An approved brainstorm becomes the workspace's **board** (Discovery, Ready, Running, Checking, Blocked, Review, Done); Running and Checking have a WIP limit, default 1, never removable; MCP moves a card only between Discovery and Ready.
- **Amended:** a brainstorm builds on one branch `str/brain-<requirement>-<id>`; a conflict or a checked-out branch is kept as its own branch; the user's checkout is never touched.

## ADR-043 — Answer tasks: a question about the code gets an answer, not a commit

*Built* (2026-11-21, after the owner's first real run turned "read the architecture" into nine cards).
- **Ask about this project** starts an answer task: the agent reads the repository and answers; nothing is kept, no check runs, the agent's last message is the result (state `answered`). Paid steps still ask first.
- The planner (`brain-v3`) returns no cards for a request that only asks to explain code, and points to Ask about this project.
- A step whose model's account is held for a reason waiting does not fix (no credit, a rejected key) is skipped before it asks for approval; the gateway refuses such a request at once with 402, never 429 with a `Retry-After` an agent sleeps through.

## Open decisions

- A code-signing certificate ([ROADMAP, Release gates: Install](ROADMAP.md#release-gates)), before distribution.
