# Decisions

> **Status:** Current · architecture decision records (ADR-001 to ADR-042) · reviewed 2026-11-21

ADR numbers never change; a superseded or withdrawn record keeps one line pointing to its replacement. Order: [ROADMAP](ROADMAP.md); built: [Implementation Status](IMPLEMENTATION-STATUS.md); screens: [design/SCREENS.md](design/SCREENS.md).

## ADR-001 — Windows first, macOS next

*Accepted.*
- The desktop app is built and tested on Windows first; macOS must build but is not release-tested; Linux is best effort.
- The headless CLI ships for Windows, macOS and Linux (ADR-037).

## ADR-002 — Local first

*Accepted.*
- Everything runs on the user's machine: credentials, the ledger, task history and workspaces never leave it.
- No account and no hosted service in the core path.

## ADR-003 — Route at task and session boundaries, never mid-session

*Accepted.*
- Routing happens when a task, step or session starts; the gateway never switches models silently inside a session (it would break prompt caches and tool-call formats).
- A session stays on the target that answered it; fallback only on hard errors before any output reached the client.

## ADR-004 — A localhost gateway, loopback only, with client tokens

*Accepted.*
- The gateway listens on `127.0.0.1`; binding beyond loopback needs an explicit user setting.
- Every request needs a scoped client token (an "API key" in the app) with its own limits.

## ADR-005 — One Tauri crate with modules; same stack as DownFetch

*Accepted.*
- Tauri 2, one Rust crate (`apps/desktop/src-tauri`) with the core, gateway and agent runner; the window is the Cargo feature `app`, so the CLI builds without it (ADR-037).
- React 19, Vite, TypeScript, plain CSS tokens (no Tailwind, no shadcn); a pnpm workspace; no Node sidecar.
- Provider adapters are written in-house and tested against recorded fixtures.

## ADR-006 — Rule-based routing

*Accepted; amended by ADR-041 and for measured length and cache.*
- `auto` ranks by fixed rules: the curated tier table T0–T4 (`curated:v2`, `gateway/tiers.rs`), a list-price tier for unknown models (`price-band:v1`, ADR-031), a minimum tier per client, and hard eligibility filters. Minimum tiers per task category are not built; ranking by measured cost per success comes later, above a minimum sample.
- The UI says "lowest eligible tier under your policy", never "cheapest".
- **Amended (ADR-041):** `pygg/brain` lets a model classify work into cards and roles; each card's model is still picked by rule from ADR-040's facts, and the user approves before money is spent.
- **Amended (measured length and cache):** `auto/cheap` ranks by expected request cost, from the model's own answers in the last 30 days to prompts within 4× this one's size (at least 20): mean output and cache-read share. With fewer, the others' median output (within the limit), no cache. The shown estimate, holds and strict limits stay at the output limit. On the MixAICloud benchmark the cheapest by list (Qwen 3.5 Flash) wrote ~9× DeepSeek V4 Flash's output and cost 7× per correct answer.

## ADR-007 — Deterministic checks gate the cascade

*Accepted.*
- A task may start at a cheaper step and moves on only on a deterministic check (typecheck, tests, build, schema); an LLM review may advise but never passes a step.
- Without an approved check there is no automatic cascade: the user reviews, and keeping records **Manually Accepted**.

## ADR-008 — A Git worktree per attempt; Git repositories only

*Accepted.*
- Each attempt runs in its own Git worktree from the task's recorded base commit; failed attempts are discarded.
- Workspaces must be Git repositories; other folders are not supported.

## ADR-009 — Keep as Branch is the result handoff

*Accepted; brainstorms amended by ADR-042.*
- Keeping commits the result to a new branch `str/<slug>-<short-id>` from its base commit, without switching the user's branch.
- Alternatives: Export Patch, Discard. Applying to the working tree (with drift detection) is not built.

## ADR-010 — Approvals are durable and cover exactly one attempt

*Accepted.*
- An approval names one attempt and a spending cap; it survives a restart and is invalidated by a change of policy, catalog, budget or the step's estimate.
- Nothing is approved automatically, and no API can approve (ADR-027).
- Notifications never include task names or paths and never carry an approve button.

## ADR-011 — Billing types and per-fact provenance

*Accepted.*
- Billing is `Free`, `Local`, `Subscription`, `Paid` or `Unknown`, per connection and per model; every price, quota and capability value carries its own source and time.
- Unknown is never free or `$0.00`; subscription use is quota, never money.
- A catalog list price gives an estimated cost, never an actual one (ADR-031).

## ADR-012 — Traffic-light window controls on every platform

*Accepted.*
- DownFetch's traffic-light window controls on Windows and Linux too.
- Release checks (snap layouts, scaling, keyboard, screen readers) are the D12 checks in the [ROADMAP](ROADMAP.md#next-in-order); scaling and screen readers are open.

## ADR-013 — Benchmark before claiming savings

*Accepted.*
- Savings are advertised only from a benchmark of reproducible coding tasks with deterministic checks, comparing always-T4, always-T2 and a check-driven cascade ([benchmark](../benchmark/README.md)).
- Paid runs stay within ADR-028's budget. Running it before the gateway is superseded by ADR-016.

## ADR-014 — Sample data is always labeled

*Accepted.*
- The desktop app shows only real data; the browser preview (`pnpm dev`) runs an in-memory demo (`src/lib/demo.ts`) under an on-screen label; `?empty` shows the first-run state.
- A screen designed but not built says so instead of showing empty data.

## ADR-015 — Commit the pnpm lockfile

*Accepted.*
- `pnpm-lock.yaml` is committed (unlike DownFetch).

## ADR-016 — Build the gateway first

*Accepted; absorbs ADR-019.*
- The multi-provider gateway (in the spirit of 9router) comes first; the task engine is a layer on top. Existing clients (Codex, Cursor, OpenCode, Claude Code, scripts) use one local base URL serving Chat Completions, Anthropic Messages and Responses (ADR-033).
- A connection is a provider account with its key in the OS credential store; a **combo** is a named, ordered model list that falls back on hard errors before any output (ADR-003); every request goes to the usage ledger with cost provenance (ADR-011).
- Outside tasks, routing is per request through the model name (`<connection>/<model>`, a combo or `auto`). The task engine runs a fixed ladder of agent steps ([AGENTS.md](AGENTS.md)); a task is not analysed into a plan.

## ADR-017 — Universal conversation memory

Withdrawn by ADR-028: no app-owned conversation store or memory. Not built.

## ADR-018 — Cache-aware model affinity for Auto Chat

Withdrawn by ADR-028 (no Auto Chat). Cache affinity for gateway clients is ADR-036.

## ADR-019 — Gateway-first order

Merged into ADR-016 (gateway first) and ADR-013 (the benchmark gates savings claims).

## ADR-020 — Entitlement tiers per account and model

Proposed, partly built: billing per connection and per model with its source (ADR-011, ADR-032). Providers views grouped by tier and a per account-model entitlement schema are not built.

## ADR-021 — Headless CLI and npm launcher

Built as ADR-037: one Rust core for the app and headless `pygg-cli`, and an npm launcher with per-platform binaries (no second, Node gateway).

## ADR-022 — Safe setup of CLIs and IDEs

*Built:* one-click setup for Claude Code, Codex CLI and OpenCode (`integrations.rs`, [AGENTS.md](AGENTS.md)).
- Pygg changes a tool's configuration only when asked, after showing the change: it backs up first, gives the tool its own client token, checks the result, and can undo it.
- Compatibility is claimed only for tested client APIs; a subscription sign-in is never routed through an unrelated client.

## ADR-023 — Multi-account rotation

Built as ADR-034 (pools) and ADR-038 (key rings). Its rule stands in both: rotation never gets around bans, quotas, terms or shared limits, and never changes an active attempt, tool call or stream.

## ADR-024 — What to adopt from OmniRoute

*Accepted.*
- An OmniRoute idea is adopted only with entitlements the provider verifies, no privacy or budget bypass, safe rollback and a measured cost per success.
- Not adopted: multi-model fusion, dozens of surfaced strategies, credential/TLS/browser impersonation, lossy context compression without independent benchmarks, claims from upstream benchmarks. Related: ADR-031, ADR-034, ADR-036, ADR-038.

## ADR-025 — The gateway must not raise cost or break provider rules

*Built; tested against fake providers, native Anthropic parity measured against the real endpoint. Amended by ADR-041.*
- Before new router features: native same-format Anthropic passthrough with prompt-cache parity; fallback only on errors that allow it; atomic budget and usage accounting with reservations; explicit session affinity (`x-pygg-session`), never derived from prompt content; explainable rule-based `auto`.
- Clients own their prompts, history and compaction: the gateway never inserts memory or rewrites a prompt; the one addition is ADR-036's cache marker on translated turns. **ADR-041:** only a request naming `pygg/brain` gets Pygg's own prompt and calls to several models, ledgered under the client's request id; nothing it learns reaches another request.
- First users: developers with metered API workloads. No cash saving is claimed for fixed-price subscriptions.

## ADR-026 — Official agent CLIs with their own sign-in; no bridges

*Accepted; narrowed by ADR-035.*
- Pygg runs the vendors' own agent CLIs (Claude Code, Codex CLI, Gemini CLI, OpenCode) headless with the sign-in the user gave each, in a worktree per attempt (ADR-008).
- Pygg never reads, stores or forwards that sign-in, never exposes it through the gateway, and never bridges cookies or other products' OAuth tokens, whatever their licence. Sites with no API: ADR-035.
- An agent step's cost is recorded as unknown, with the agent's own estimate in its source ([AGENTS.md](AGENTS.md)).

## ADR-027 — Tasks start from the user's IDE agent; the app monitors

*Accepted (tasks on hold).*
- Tasks start from the user's coding agent through Pygg's MCP server (`<app> mcp`), forwarded to the gateway's task API; the app runs tasks, approves paid steps and reviews results. New Task in the app is a manual fallback.
- A client needs **May start tasks**, sees only its own tasks, and cannot approve a paid step (ADR-010).
- Replaces the parts of [design/SCREENS.md](design/SCREENS.md) that assume tasks are planned in the app ([AGENTS.md](AGENTS.md#the-mcp-server-and-the-task-api-experimental-on-hold)). ADR-041: a brainstorm starts when the IDE agent calls `pygg/brain` and is approved in the app.

## ADR-028 — MIT licence, a $20 benchmark budget, and no Auto Chat

*Accepted (owner's decisions).*
- **Licence:** MIT ([LICENSE](../LICENSE)). MIT code from other projects may be ported with its notice kept, listed in `THIRD_PARTY_NOTICES.md`; ADR-026 applies whatever another project's licence.
- **Benchmark budget:** paid runs cost at most $20 in total, enforced with `bench --budget-usd` (about $16.7 used); free, local and subscription runs don't count and are reported apart. The larger benchmark ([ROADMAP](ROADMAP.md)) has its own $50 under the same rules; Anthropic's Claude models join it only once the owner's Anthropic usage limit allows, and coding agents such as Claude Code still run through Pygg on other models.
- **No Auto Chat in the app:** no conversation store, memory screen or per-turn model choice (ADR-017, ADR-018 withdrawn); kept are the bounded task handoff (ADR-030) and gateway session affinity. `pygg chat` and `pygg ask` are terminal clients of the gateway (own API key "Pygg CLI (ask)", with routing, fallbacks, web-session guard and ledger); a conversation lives only in the command's memory. `pygg/brain` is not chat (ADR-041): its questions are answered in the client; the app has no reply box.

## ADR-029 — Quality signals in Pygg's own ledger, in TensorZero's shape

*Built.*
- TensorZero is not adopted (archived, needs Postgres or ClickHouse and a second gateway, stores whole conversations, sends analytics by default); Pygg borrows its data model: **metrics** (name, boolean or float, level, better direction) and **feedback** rows keyed to a request, attempt or task, with their source.
- Migration 0015 adds both and seeds `checks_passed`, `result_kept`, `retried`, `cancelled`, `declined`, `rework_minutes`; the task engine records them, and `tasks::quality` gives known money per kept result, counting every paid step.
- An unknown signal has no row (never 0 or false); quality is never inferred from conversation content (ADR-025); request-level feedback comes only from a client that sends it.

## ADR-030 — Task handoff within ADR-028; portable context deferred

*Built; portable context deferred.*
- A next step may be told what earlier attempts did: bounded, from Pygg's records, shown to the user, for that task only, never kept across tasks, shown as memory or sent through the gateway.
- Modes, fixed in the task's policy: `tail` (default: the failed check's command and the end of its output), `none`, `evidence` (every earlier attempt, about 6,000 characters at most; `handoff.rs`). `tail` stays default: the A/B showed no token, time, money or pass-rate gain for `evidence` ([report](../benchmark/reports/2026-11-12-h1-handoff-ab.md)); a first step that reaches its checks and fails is untested.
- Portable context between external clients stays deferred (MCP carries a task description, not a context package); reopening needs evidence and a new decision on ADR-028. An adaptive router is evaluated only on benchmark replays, never as a live shadow.

## ADR-031 — The user enters a key; the app looks up the rest

*Built.*
- [models.dev](https://models.dev), keyed by provider model ids, fills only unknown context, maximum output, tool and image support and list price; the provider's list comes first and the user's settings are never touched. Matched by API address, then kind; local runtimes never. Each fact names its source ("models.dev list price"); the user's own price replaces it (ADR-011).
- Every model gets a tier: `curated:v2` first, else `price-band:v1` by output price per million tokens (under $1 T1, under $5 T2, under $20 T3, above T4), shown as "from its list price"; $0 gives no tier; a user-set tier stays. Claude models take tools (`curated:v1`). A free connection does not make a priced model free (ADR-032). When nothing is eligible, the refusal groups reasons by connection.
- Runs in the background at start-up and after a model list refresh; at most one download a day (a GET of a public file carrying nothing about the user), cached as `reference/models.dev.json`; `gateway lookup` applies it by hand. On a real store of 584 models it described 466.

## ADR-032 — A connection kept for its free models has only those on

*Built.*
- Two kinds of free: a free account (trial key, Gemini's free tier), or $0-listed models beside paid ones (OpenRouter, Z.AI; presets' `free_models`).
- On such a connection set as free, only $0-listed models are on and free; priced models are paid, unpriced ones unknown (OpenRouter's routers list none). Pygg turns both off (`enabled_source = 'free-connection'`) and the Models table says why; a model the user turns on stays on, one Pygg turned off returns when the reason ends. The ledger follows the same rule.
- Missing free models are added from the catalog (ADR-031) for Z.AI only (its free Flash models).

## ADR-033 — The gateway serves the Responses API, translated through Chat Completions

*Built.*
- `POST /v1/responses` is a third surface on the same pipeline (`gateway/responses.rs`). One new translator, Responses ↔ Chat Completions; Anthropic-format connections go on through the existing Chat Completions → Messages translator. Native passthrough only where the preset says (`Preset::serves_responses`): OpenAI.
- Custom tools (Codex's `apply_patch`) cross as functions with one string parameter `input`, grammar in the description; namespace tools flatten to `<namespace>__<name>`; both matched back by name from the request's own tool list. Reasoning returns as a `reasoning` item and goes back as `reasoning_content` where taken. Anything dropped (other providers' encrypted reasoning, hosted tools, uncarried fields) is named in `x-pygg-dropped`.
- No stored responses: `previous_response_id`, `conversation`, `background` get 400. Hosted tools are unavailable on translated connections. The Codex `pygg` profile names a Pygg-written model catalog with context sizes, `apply_patch` and Codex's default instructions (Apache-2.0, bundled; [AGENTS.md](AGENTS.md)). One-click setup: "Codex: models through Pygg".

## ADR-034 — Pools of separate accounts

*Built.*
- A pool groups connections of one provider kind, each a separate account with its own key, joined only after the user confirms it has its own quota; keys of one account cannot be pooled (ADR-038 for keys on one connection).
- `<pool>/<model>` tries members in the user's order, skipping disabled ones, moving on for 401, 402, 403, 429 and Anthropic's out-of-credit 400; a session stays on its first-answering member; the ledger names the answering connection; `GET /v1/models` lists `<pool>/<model>`. A pool name may not be a connection id, combo name or `auto`.
- Web sessions (ADR-035) of one site may pool: least used today first, a resting account last. Agent sign-ins and other apps' cookies are never pooled (ADR-026). A provider's own OAuth that issues an API key (OpenRouter PKCE) is not a bridge.

## ADR-035 — Web-session connections for AI sites with no API

*Built.*
- Connection kind **Web session (unofficial)** for AI sites with no API: a normal provider (Connect, models, health, limits, ledger, `GET /v1/models`, all three surfaces as `<connection>/<model>` or in a combo). Recipes: Muse, ChatGPT, Perplexity, DeepSeek, Grok, Gemini.
- Narrows ADR-026 for this kind only: no cookie import, no other products' OAuth tokens, no TLS impersonation, no limit evasion. The user carries the account risk, per connection.
- Slow and without token counts: for chat-like work on a named model, not coding agents.

**How it works**
- **Window:** one Pygg webview per connection (`web-<connection id>`) with its own WebView2 profile (`websessions/<connection id>`); the user signs in on the site's page; cookies are never copied or held. Opens hidden when needed, closes after 10 minutes unused, has no Tauri capability.
- **No fingerprinting:** never spoofs or randomises user agent, client hints, canvas, fonts or screen, and never hides that a page is driven.
- **Navigation:** with the sign-in window open, any https address; whenever Pygg types or presses Send the page must be the site's own host, else nothing is sent and the connection is Needs you; otherwise only the recipe's hosts.
- **Recipe** (`RECIPES` in `websession/mod.rs`): start URL, hosts, selectors (prompt box, Send, reply, still answering, sign-in page, challenge). Scripts only read and fill the page, never `document.cookie` or storage; a mismatch fails closed as "the page has changed".
- **Adapter:** each session is a loopback Chat Completions endpoint behind an in-memory token; a request is flattened into one prompt in a new chat; a stream reads the page every 300 ms.
- **Waits:** first word 60 s + 1 s per 100 estimated prompt tokens, up to 240 s; once the page shows it is answering, 300 s + the same extra for the whole reply.
- Not in headless `pygg gateway serve` (skipped with that reason).

**The web guard** (`websession/guard.rs`): decides every request; fixed rules, no jitter; slows Pygg, never disguises it. Added after DeepSeek suspended an account sent 24 prompts of 2,000–6,000 tokens in 15 minutes at a 10 s pace.

| Rule | Default | DeepSeek |
| --- | --- | --- |
| Weight | 1 unit per request + 1 per started 2,000 prompt characters | same |
| Prompt cap | over 12,000 characters → 413 `prompt_too_long` | 4,000 characters |
| Rest after a reply | 10 s + 2 s per extra unit + 50 reply characters/s, max 3 min; a wait over 30 s → 429 `web_pace` with Retry-After | 30 s base |
| Per rolling hour | 20 units, 15 requests | 8 units, 6 requests |
| Per 10 minutes | 5 requests | 3 requests |
| Per UTC day | daily request limit (50 default, max 100), 2× that in units | 30 requests |
| Back-off | 5 min, doubled per trouble in a row, max 6 h; floors: usage-limit notice 1 h, slow-down or check 30 min, suspension notice 6 h | same |

- A short "slow down" reply is 429 `site_slow_down`, not an answer; a clean reply clears the back-off; Providers offers **Resume Now**. A suspension notice turns the connection off (`RESTRICTED`), never read as signed out.
- No setting raises a ceiling or turns the guard off; timing outside tests is held at the floors (`Timing::guarded`). State lives in `web_pace` and `providers.web_*`, so restarts don't reset it.

**Safeguards (each tested)**
1. Off by default, in its own Connect group; out of routing until the warning is acknowledged and the user has signed in.
2. Per-connection warning (terms may forbid automation, account may be limited or banned, site may break it, Pygg acts as the user), stored as `web_ack_at`, `web_ack_version`, asked again when its text changes.
3. No passwords, cookie import or secrets written; session data stays in the profile folder, never in logs, diagnostics, ledger, exports or errors.
4. The profile folder is deleted with the connection, or at next start if WebView2 still holds it.
5. Never chosen by `auto` or `auto/cheap` (no tier, `tools = false`); used only when named or in a combo.
6. Billing Free by default ($0, with that source); Subscription allowed (quota, cost Unknown); Paid and Local refused.
7. One request at a time per connection; a second waits briefly, then gets 429 with Retry-After.
8. A CAPTCHA, sign-in page or bot check marks the connection Needs you (or Not signed in) and answers 401 with the reason; Pygg never solves, skips or hides from it.
9. API first: where a site has an official API (Meta's `api.meta.ai`), Connect offers it as an API-key preset and the web preset says so.
10. Pictures, videos, sounds and documents in a reply come back after its text as Markdown links (by address, copied from page memory for `blob:`, or from responses and Blobs the page received). No cookie or header kept, the site's API answers not searched for addresses, nothing on another origin re-fetched; the only click beyond Send and **deny** is Muse's Play, muted. Details: [SECURITY](SECURITY.md#web-sessions).

**Cannot:** tool calls (a request with tools gets 400), files sent to the site, token counts, system-prompt guarantees, choice of the model behind the site. Muse can act on the user's connected apps; Pygg clicks only Send and the site's **deny** on any approval Muse asks for.

## ADR-036 — How Pygg saves tokens: keep the cache, route per session, measure before compressing

*Built; compression measured in shadow only.* In this order:

1. **Keep the provider's cache working.**
   - A session keeps a launch on one model (`pygg run`, `x-pygg-session`); fallback only on failure. A session moved by a fallback goes back home only when no warm cache is lost (home answered within 5 minutes, or the fallback has not for 5 minutes), at request start, never mid-attempt or stream, not while home cools down (`store::Pin::goes_home`, migration 0033; `x-pygg-session: home`, route kind `+home`).
   - An OpenAI-format turn with tools translated for Anthropic's own API, or for an `anthropic/…` model on OpenRouter, gets one top-level `cache_control` (`gateway::adds_cache`; ledger says `added`); an Anthropic-format client's own top-level marker crosses to OpenRouter. Unchanged: client-marked requests, native Anthropic, turns without tools, other models, other providers' Anthropic-format endpoints. One marker suffices (Anthropic looks back at most 20 positions; a run of parallel tool calls or results counts as one).
   - Self-caching providers (OpenAI, DeepSeek, Gemini, Kimi, Z.AI GLM, MiniMax) get nothing added. Alibaba Model Studio gets no marker: its implicit cache served 91% of qwen3.8-flash's input in the N2 benchmark (explicit: hits 10% of input price vs 20%, writes 125%; pay-as-you-go needs a live parity check first). A client's own marker crosses, and its writes are priced (`prompt_tokens_details.cache_creation_input_tokens`).
   - The session is passed on to providers that route by one: OpenRouter's `x-session-id` (sticky 60 minutes) and OpenAI's `prompt_cache_key`, as `pygg-` + SHA-256 of client and session id, never the raw id or prompt content; a client's own `session_id` or `prompt_cache_key` is kept (`gateway::pass_session`).
   - `PYGG_ANTHROPIC_AUTO_CACHE=off` and `PYGG_PASS_SESSION=off` turn these off for a benchmark arm.
   - Measured: Codex on Claude Haiku solved the same 16 runs with and without the added marker, 30% cheaper with it ([report](../benchmark/reports/2026-11-14-codex-claude-prompt-cache.md)).
2. **Route per session or task, not per request.** `auto` chooses when a session starts and stays; no per-prompt learned router.
3. **Show where tokens go before changing anything** (Usage → Token Mix and Routing).
   - Cache share, cache writes after a model change, attempts, fallbacks, and the cause of each cache miss (`store::cache_miss`, migration 0034), in order: provider did not report its cache; prompt under 1,024 tokens; client cache hints dropped in translation; session's first request; last answer from another model; model read nothing from cache in 7 days; over 5 minutes since the session last used the model; else the prompt's start changed. A zero read is a miss only when the provider reported the field (DeepSeek's `prompt_cache_hit_tokens` included).
   - Tools and system prompt per request (migration 0036): estimated tokens and a 64-bit fingerprint, never the text; Token Mix shows per session what the first request carried, how often it changed and how often a later answer missed.
   - Longer cache lifetime, weighed from session pauses on models that bill cache writes (`store::cache_lifetime`): Anthropic's 1-hour lifetime (writes 2× input price vs 1.25×) and a keep-alive (last request re-sent every 4.5 minutes for up to an hour). Neither is offered unless its net stays above zero.
4. **Compression only in shadow.** An agent's history is never rewritten; a filter on new tool output is at most an opt-in experiment. Clearing old tool results is measured for OpenAI-format requests at Anthropic's defaults (past 100,000 prompt tokens, all but the last three; `shadow::clearing`, migration 0037), weighing saved input against extra cache writes (`store::clearing_shadow`). Anthropic-format requests (Claude Code, OpenCode) are never measured or changed. Opt-in only if the net stays above zero.
5. **Output and effort stay the client's:** `effort` and thinking settings pass through; no shorter-output instructions.

- **Not done:** semantic response caching, history dedup or retrieval markers, context as images, RTK's BUSL-licensed engine.
- **Claims by scope:** one bounded task — cost per solved task from checks the agent cannot edit; parallel sessions — that cost is visible and bounded, not that more gets done; a whole project — observations only (cost per launch, per commit, launches that changed nothing), never productivity or return. Each `pygg run` launch is kept with its Git changes (`launches`, schema v38). Every saving shown is traceable in the ledger.

## ADR-037 — One npm command for every platform

*Built.*
- Launcher `@pygg/cli` (command `pygg`) with one optional dependency per platform, restricted by `os`, `cpu` and on Linux `libc`: `@pygg/win32-x64`, `@pygg/darwin-arm64`, `@pygg/darwin-x64`, `@pygg/linux-x64` (glibc), all at the launcher's version (npm refused unscoped `pygg`). No `postinstall` download.
- `packages/npm/pygg/bin/pygg.js` runs the platform's program with the same arguments and streams, passes SIGTERM and SIGHUP, exits with its code; explains when no build exists; restores a lost execute bit; `PYGG_CLI` overrides; on Windows an installed app's `pygg-cli.exe` is the fallback.
- `pygg-cli` builds with `--no-default-features` (no Tauri, no webview; window code stays behind `#[cfg(feature = "app")]`, caught by `cargo check --no-default-features` and the Linux CI job). Credentials: Secret Service, Keychain, Credential Manager; the store is found where the app keeps it, or at `PYGG_DATA_DIR`. Packages come from `scripts/npm-pack.mjs`; CI (`.github/workflows/npm-packages.yml`) builds, packs and installs, never publishes; publishing is manual ([PUBLISHING.md](../packages/npm/PUBLISHING.md)).
- **Not done:** linux-arm64, musl, code signing.

## ADR-038 — Key rings: keys of separate accounts on one connection, used in turn

*Built.*
- A connection may hold a key ring: its own key plus other accounts' keys, each confirmed by the user as a separate account with its own quota (ADR-034's rule). Requests still name `connection/model`; combos and `auto` unchanged.
- Order (`providers.key_strategy`): `in_order` (default), `round_robin`, `least_used` (fewest requests today), `quota_aware` (most room by the account's last rate-limit headers, else least used); a session returns to the key that answered it. Per key (`policy.rs`, `key_health`): 401 or rejected-key 400 blocks it until replaced or removed; 402, out-of-credit or 429 cools it (one model, for a 429) for Retry-After or a default, and the next key is tried at once; other failures stay the connection's; quota groups hold. The ledger names the key (`usage_events.key_id`).
- Secrets in the OS credential store, one reference per key, never read, logged or returned; removing the connection deletes every key. Never rotated: web sessions, subscription and agent sign-ins, local runtimes. UI: Providers → a connection → API keys (state, requests today, remaining limits, order, on/off, remove, **Add Key from Another Account…**).
- **Not done:** weights, a sticky turn count, a simulator, per-key budgets. Pools stay for accounts that need their own endpoint, models or prices.

## ADR-039 — Skills & Prompts: one library, installed into each agent's own skills folder

*Built.*
- A library of skills in the Agent Skills format (a folder with `SKILL.md`: front matter `name`, `description`, then instructions), installed into Claude Code's `~/.claude/skills` (or `$CLAUDE_CONFIG_DIR/skills`) and the shared `~/.agents/skills` (Codex, OpenCode; OpenCode also reads Claude Code's). A library and installer, not a downloading hub.
- A prompt is a skill run only by name: `disable-model-invocation: true` (Claude Code, OpenCode) and, beside each prompt in `~/.agents/skills`, Codex's `agents/openai.yaml` with `policy.allow_implicit_invocation: false`. The eight starter skills (MIT) are all prompts. Each install writes one `SKILL.md` with `metadata.pygg-hash`; Pygg never overwrites a folder it did not write or a copy edited since (line endings are not edits). User skills are rows of `skills` (schema v32); found skills can be copied in; starters are duplicated, never changed.
- **Sent With Every Request:** per agent, its global instruction file (`CLAUDE.md`, `AGENTS.md`; OpenCode falls back to `CLAUDE.md`) counted whole, plus each listed skill's name and description. **Projects** (`skill_projects`, schema v35): user-added folders get `.claude/skills` and `.agents/skills`; the home folder is refused; a folder is forgotten only once its copies are removed. Counted from the project root: `CLAUDE.md`, `.claude/CLAUDE.md`, `CLAUDE.local.md` (Claude Code), `AGENTS.override.md` or `AGENTS.md` (Codex), `AGENTS.md` else `CLAUDE.md` (OpenCode); subfolder files are not counted; projects not added are never read.
- **Not done:** downloading skills (would need a pinned commit, review of every file, quarantine for scripts), skills with scripts or extra files, `~/.codex/prompts` as a target.

## ADR-040 — Model facts: per-role scores with their sources, published by the project

*Built* ([design/BRAIN.md](design/BRAIN.md)).
- A signed JSON **model facts feed**, built weekly by a job in this repository from named public sources; the app fetches it over HTTPS daily, caches it in the store, and never reads a leaderboard itself. A fact: model line (matched as `gateway/tiers.rs`, or exact id), role, score as published, rank within the source (0–1), source name and URL, read date, feed version; no source, no row.
- Roles: `code` (SWE-bench Verified, Aider polyglot, Terminal-Bench); `plan`, `review` (LMArena text, LiveBench, Artificial Analysis, all labelled **derived**); `test` derived from `code`; `image` (LMArena text-to-image; stored, not routable until the gateway has an images path). Each source's terms are recorded before use and disallowing sources are left out; leaderboard names map to provider ids by a hand-kept table.
- Ranking: the user's ledger evidence (`checks_passed`, `result_kept` per model and role, ADR-029) above 10 attempts, then the feed, then the tier table (bands: ADR-041). The feed changes suggestions, never a saved choice. Signed with a CI-held key, public key in the app; an unverified feed is refused; offline uses the last cached feed with its date, else the tier table.

## ADR-041 — `pygg/brain`: the router is the brain

*Built* ([design/BRAIN.md](design/BRAIN.md)); the `image` role waits for an images path.
- Virtual model `pygg/brain` on Chat Completions, Messages and Responses: a requirement in, a **proposal** out (clarifying questions and cards, each with a role and done criterion, a model per role picked by rule). The LLM classifies, rules pick the model, the user approves before money (amends ADR-006 that far).
- Steps: (1) the planner is picked by rule for role `plan` (ADR-040); a saved pick wins; a paid planner's first use waits for the user's pick in the app, a free or local one runs at once; no LLM runs before this. (2) The requirement goes with Pygg's versioned brain prompt (Skills & Prompts, ADR-039) and nothing else; the planner gets no list of connections. (3) A card that does not fit the board schema is dropped; `brain_schema_ok` per planner model (ADR-029). (4) Rules pick each card's model: role, role ranking, then `auto`'s filters under the client's policy. (5) Stored as a **brainstorm** with an id, changed and approved in the app; the client gets a summary with the id and questions; a later call in the same session refines it.
- ADR-025 kept: only `pygg/brain` requests run Pygg's prompt or fan out; every brain call is ledgered under the client's request id, key and session, `route_kind` `brain`; nothing is kept as memory (ADR-030). Role aliases `auto/plan`, `auto/code`, `auto/test`, `auto/review` (`auto`'s filters, then the role's ranking; `x-pygg-auto` names the fact's source and date). A card runs on Pygg's task engine or the user's IDE agent via the role alias, chosen per card. No chat in the app (ADR-028).
- **Amended: facts rank in bands, price decides within a band.** Order: the user's evidence (kept rate in bands of 0.1, then money per kept result), then facts and tiers on one scale in bands of 0.15, cheaper first ([BRAIN.md](design/BRAIN.md) §1); band widths are a judgment for the next run to check. Evidence: Gemini 4 Carbon (rank 0.96) solved 33 of 36 at $0.115 per solved task, Gemini 4 Hydrogen (0.84, a quarter cheaper per token) 36 of 36 at $0.072 ([report](../benchmark/reports/2026-11-20-all-models.md)).

## ADR-042 — Smart tasks run as a Kanban board

*Built* ([design/BRAIN.md](design/BRAIN.md)).
- An approved brainstorm becomes the **board** of a workspace (a Git repository, ADR-008). Fixed columns: Discovery, Ready, Running, Checking, Blocked, Review, Done; a card is a task (a ladder of steps on the card's role model, [AGENTS.md](AGENTS.md)).
- Pull only: **Running** and **Checking** each have a WIP limit, default 1, raisable, never removable; queue columns hold any number. Moves: Running → Checking when the agent ends; Checking → Review when approved checks pass (ADR-007), → Blocked when the ladder's last step fails (without checks, only to Review, marked unverified); Review → Done on Keep as Branch (ADR-009). Blocked (pinned): waiting for approval, over cap or budget, interrupted, or on an unanswered question. Discovery holds questions as cards, answered through `pygg/brain`, never in the app; dependent cards cannot enter Ready. Classes: standard, and expedite (one at a time, front of Ready, still under the WIP limit).
- Measured from records: cycle time (Ready → Done), lead time, throughput per day, time blocked, known money per kept result (ADR-029); no story points or sprints; a card's size is the cost range of the last ten runs. MCP can read the board and move a card between Discovery and Ready; nothing approves a paid step (ADR-010).
- **Amended: a brainstorm builds on one branch.** Approval makes `str/brain-<requirement>-<id>` at the workspace's HEAD; each card starts from its tip, and keeping adds the commit (on top if the branch has not moved, else picked onto its tip). A conflict, or a branch the user has checked out, is kept as its own branch and the task says why. The user's checkout is never touched.

## Open decisions

- A code-signing certificate ([ROADMAP, Release gates: Install](ROADMAP.md#release-gates)), before distribution.
