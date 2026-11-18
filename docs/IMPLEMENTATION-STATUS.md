# Implementation status

> **Status:** Authoritative · what is built and how it was verified · reviewed 2026-11-15

What exists in code, and how each part was verified. What comes next is in the [ROADMAP](ROADMAP.md); why, in [DECISIONS](DECISIONS.md). Code paths are under `apps/desktop/src-tauri/src` unless they say otherwise.

**Levels used in "Verified by":**

- **Unit tests**: tests of the module alone.
- **Fake-provider tests**: end-to-end tests through a real gateway against a fake provider (`gateway/tests.rs`).
- **Live**: a real provider answered through the gateway. The providers and models are named.
- **Browser preview**: the interface on labelled demo data (`pnpm dev`, ADR-014), with `?empty` for a first run and `?stress` for long data. `pnpm ui-check` loads every screen and a set of clicked states (sheets, menus, tested models) with `?stress` data in headless Edge or Chrome at five sizes (1024×640, 1440×900, 1990×1030, and 819×512 and 683×427, the minimum window at Windows text size 125% and 150%), and fails on overflow. It first plants a known overflow and stops if it cannot find it.

CI runs `cargo clippy -- -D warnings` for the app (`--all-features`) and the console program (`--no-default-features`), `cargo fmt --check`, the library and integration tests (`cargo test --lib --tests`), the UI typecheck and build, the schema reference check, the licence check and the layout check. `cargo audit` and `pnpm audit` run when a lockfile changes and weekly.

## P0: cost-safe gateway

| Item | Built | Fake-provider tests | Live | Provider parity |
| --- | --- | --- | --- | --- |
| Same-format passthrough | Yes | Yes | Anthropic native (claude-haiku-4-5, claude-haiku-5-5, claude-sonnet-5-5); translation to Gemini and Qwen | Anthropic: identical cache writes and reads, streamed and not, 5-minute and 1-hour TTL; thinking with a tool; two-turn tool round trip; citations; `count_tokens` |
| Error-scoped fallback | Yes | Yes | 404 cooldown and skip (Gemini); combo fallback (OpenCode); real 401, 400 and 404 answers from 8 providers, and 402 (empty balance: Requesty, SiliconFlow), 403 (OpenRouter, Gemini), 429 (OpenRouter) and a timeout after sending (OpenRouter), as fixtures | Not applicable |
| Budgets and complete ledger | Yes | Yes | Ledger and prices (Gemini, Anthropic); a strict client limit held every request of the benchmark runs | Not applicable |
| Explicit sessions | Yes | Yes | Headless Claude Code: header on 12 of 12 requests | Not applicable |
| `auto` routing | Yes | Yes | Gemini, Anthropic, Qwen token plan, headless Claude Code | Not applicable |

None of the five is release-ready: the release gates are in the ROADMAP.

## Gateway (`gateway/`)

| Part | What it does | Verified by |
| --- | --- | --- |
| Server (`mod.rs`) | Loopback only (`127.0.0.1:8741` by default). Refuses a non-loopback `Host` (DNS rebinding). Requires a client key (`Authorization: Bearer` or `x-api-key`). Serves `GET /v1/models`, `POST /v1/chat/completions`, `POST /v1/messages`, `POST /v1/responses`, `count_tokens` and `/health`. | Fake-provider tests; live (Gemini, Anthropic) |
| One gateway per store | An exclusive OS file lock on `<store>.gateway-lock`. A second gateway on the same store is refused before it sends anything. A stopped gateway keeps the store until every attempt in flight is settled; ownership generations mark entries left by another gateway as interrupted. | Lifecycle tests (stop with work in flight, quick restarts, failed recovery); `tests/two_gateways.rs` runs two real processes |
| Same-format passthrough | Each connection has a wire format (`openai` or `anthropic`). Same format: the JSON is rewritten with keys in the client's order and only `model` replaced (apart from ADR-036's cache marker and session key, below); `anthropic-version`, `anthropic-beta`, `cache_control`, thinking and citations pass. The answer and the provider's rate-limit and request-id headers come back unchanged. | Fake Anthropic endpoint echoing what it received; live parity on three Anthropic models (`scripts/anthropic-parity.ps1`) |
| Anthropic → OpenAI translation (`anthropic.rs`) | System, text, images, tools, `tool_choice`, tool calls and results, streamed or not. Cache markers kept for OpenRouter. A provider's `reasoning_content` or `reasoning` comes back as a `thinking` block signed `pygg-reasoning-v1`, and goes back where the provider takes it. Every dropped field is named in `x-pygg-dropped`. | Unit tests; live: Gemini (tool round trip, `dropped: cache_control`), Qwen token plan (thinking with a tool, two turns, streamed) |
| OpenAI → Anthropic translation (`openai_to_anthropic.rs`) | Messages, images, tools, tool calls, limits, `stop`, `stream`, usage. `reasoning_effort` becomes `thinking` per the model's facts (adaptive with effort, or a budget); Claude's thinking comes back as `reasoning_content`. A request with no output limit gets 64,000 tokens streamed and 16,000 otherwise, at most the model's maximum. An agent's turn (it carries tools) to Anthropic's own API gets automatic prompt caching (ADR-036). | Unit and fake-provider tests; live on claude-haiku-5-5 and claude-haiku-4-5 (plain, tool round trip, stream, thinking). Caching measured: [report](../benchmark/reports/2026-11-14-codex-claude-prompt-cache.md) |
| Responses API (`responses.rs`, ADR-033) | `POST /v1/responses` on the same routing, limits and ledger. Translated through Chat Completions to OpenAI-format and Anthropic connections; native passthrough where the preset serves `/responses`. Custom and namespace tools cross; `previous_response_id`, `conversation` and `background` are refused (nothing is stored). | Unit tests on captured Codex requests; fake-provider tests; live with Codex CLI on claude-haiku-5-5 and Qwen's token plan |
| Streaming | Server-sent events pass through. Usage is read from the last chunk. A client disconnect or broken stream is recorded with an unknown cost. A stream holding only `[DONE]` is a failure. | Fake-provider tests; live |
| Gemini signatures (`signatures.rs`) | `thought_signature` remembered per tool call (stored 30 days) and put back on requests to Gemini; stripped for other providers. | Unit tests; live round trip on Gemini |
| Explicit sessions | `x-pygg-session` (at most 128 visible ASCII characters) pins a session to the target that last answered it, 2 hours sliding, scoped to the client key. After a fallback the session goes back to its home once no warm cache is lost (home answered within 5 minutes, or both caches cold); `x-pygg-session` answers `new`, `pinned` or `home`. Never guessed from content. The session goes on to OpenRouter as `x-session-id` and to OpenAI as `prompt_cache_key`, as `pygg-` and a hash of the client and session, unless the client named its own. | Fake-provider tests (the way home and the session passed on: fake provider only, no live check yet); live (Gemini, headless Claude Code) |
| Cache marker (ADR-036) | An OpenAI-format agent's turn (with tools) gets one top-level `cache_control` when it goes to Anthropic's own API, translated, or to an `anthropic/…` model on OpenRouter. An Anthropic-format client's top-level marker crosses to OpenRouter. The ledger's cache hint says `added`. | Fake-provider tests; live on Anthropic's API ([prompt-cache report](../benchmark/reports/2026-11-14-codex-claude-prompt-cache.md)) and on OpenRouter with `anthropic/claude-haiku-5.5`, Chat Completions and Responses API: 99.7% of later prompts read from cache ([report](../benchmark/reports/2026-11-16-openrouter-cache-live.md)) |
| Error-scoped fallback (`policy.rs`) | Fallback only before any byte reaches the client. 401 blocks the connection until its key is replaced. 402, 403 and 429 cool the model or connection down and never move to another connection in the same quota group while the cooldown lasts. Anthropic's 400 "credit balance is too low" counts as out of credit; its 400 prefill refusal falls back. Resets from `Retry-After`, `retry-after-ms` or Gemini's `RetryInfo`. A read timeout, 408, 504 and 524 stop with an unknown cost. Other 5xx stop unless the client allows retries. | Fake-provider tests for each rule; real error fixtures (`tests/provider_errors.rs`); live: missing model cooled down and skipped (Gemini), 429 cooldown (OpenRouter), 402 (DeepSeek) |
| Circuit breaker | A failing connection waits twice as long after each failure in a row (up to 15 minutes); an unreachable one is probed through its free model list. | Unit tests |
| Overhead | About 4 ms per short request and 5 ms per 400 kB prompt at the median; about 230 requests a second with 16 clients at once. | Release-mode test `gateway::tests::overhead`: [report](../benchmark/reports/2026-11-14-gateway-overhead.md) |

## Routing: named models, combos and `auto`

| Part | What it does | Verified by |
| --- | --- | --- |
| Model names | `<connection>/<model>`, `<pool>/<model>`, a combo name, `auto` or `auto/cheap`. `auto` is reserved. | Fake-provider tests; live |
| Combos | Members tried in order, or **cheapest first**: re-ordered per request by estimated cost (free, local and subscription at $0; unknown prices last). The editor offers models of enabled connections, finds a model by part of its name, reorders by drag or keyboard, and warns when two members share a quota group. | Ordered: live (Gemini, OpenCode). Cheapest first: fake-provider test only |
| Combo templates (`lib/comboTemplates.ts`) | Free first, Use my plans first, Subscription only, Strong coding with cheap backup, No metered spend, Local only, Long-context coding, Cheapest for each request: rules resolved over the user's models into a saved list, with every member's reason and every model left out. Unknown facts never qualify. | Browser preview; no automated test |
| `auto` and `auto/cheap` (`auto.rs`, ADR-006) | Hard filters first, each with a reason: the client's connections, API format, cooldowns and quota holds, known support for tools, images and the thinking an Anthropic request asks for, data-use terms, context and output fit, a statable cost, and for `auto` a tier at or above the client's minimum (T2 by default). Then rules only, no classifier: `auto` takes the lowest eligible tier, then local, free, subscription, paid, then cost; `auto/cheap` the lowest estimated cost. The first five form the route. The reason is in `x-pygg-auto` and the ledger. Nothing eligible: 422 with the reasons grouped by connection. | Unit tests for every filter and the ranking; fake-provider tests; live: Gemini, Anthropic, Qwen token plan, headless Claude Code |
| 429 hold | A model answering 429 without a usable reset, or twice in a row, is held out of `auto` for 10 minutes, doubling up to a day. The 429s of requests sent before the last strike was recorded (one burst of parallel requests) count as one strike. Named routes are not affected. | Fake-provider test |
| Model facts | Tier, context, maximum output, tools, images, thinking, each with its source: the provider's model list, the reference catalog (models.dev, at most one download a day, ADR-031), Pygg's tier table (`tiers.rs`, `curated:v2`, by each vendor's naming), a price band, or the user. What the user sets is never overwritten. | Unit tests (catalog parsing, user facts kept, tier table and its upgrade) |
| Stated quotas (`quota.rs`) | Rate-limit headers read as windows (Anthropic, OpenAI and Groq, OpenRouter forms); `auto` skips a model whose stated limit is used up until reset. OpenRouter's `/key` and DeepSeek's `/user/balance` read at start-up, on refresh and with Check Now. Nothing is estimated. | Unit tests |
| Simulator | Combos → How auto chooses: the route for a described request, with every model left out and why. Sends nothing. | Browser preview |

## Policy, budgets and the ledger

| Part | What it does | Verified by |
| --- | --- | --- |
| API keys (client keys) | The store holds a SHA-256 hash; a copy goes to the OS credential store so Show and Copy work. Each key has a monthly limit (estimated or strict), a cap on requests in flight (8 by default), an unknown-price policy, retries after a server error (off by default), a minimum tier for `auto`, a no-training requirement and an optional list of allowed connections. Defaults for new keys are set in Policies. Revoked keys can be deleted. | Fake-provider tests (allowlist on every path, limits, in-flight cap); unit tests (deletion, defaults). Show and Copy: no test |
| Budgets (P0-3) | Each attempt is journaled before it is sent; if the journal cannot be written, nothing is sent. A limited client holds an estimate atomically and settles from real usage, exactly once. A **strict** limit holds the most a request could cost and refuses unknown prices or output limits. Spend at risk counts against the limit. While a settlement waits to be written, no paid target is sent. A client that disconnects still leaves an entry. | Fake-provider tests: two concurrent requests against a limit that fits one, CJK and cache-write estimates, at-risk spend, read-only store, interrupted entries, disconnect |
| Ledger and cost status (`pricing.rs`, `store.rs`) | Every attempt is recorded, failed ones included. **Actual**: the provider reported the charge. **Estimated**: from a price, or free or local billing. **Unknown**: everything else, never stored as $0. Each attempt records its request id (`x-pygg-request-id`), its place in a fallback, the billing it was sent under, how the route was chosen, cache-hint handling, for key rings the key, for an answer that read nothing from the cache its likely cause, and the estimated size and a fingerprint of its tools and system prompt (never their text). A cache read the provider did not report is left empty, not 0. | Unit and fake-provider tests; cache-miss causes and OpenRouter's cache writes checked live ([report](../benchmark/reports/2026-11-16-openrouter-cache-live.md)); live ledgers checked in the app |
| Data-use terms | Per connection: no training, may train or not recorded, with a source. A client can require no training; this filters combos, fallbacks, pins, `auto` and the model list. Pygg does not fill terms in. | Fake-provider tests |
| Usage screen | Summary (cost chart by cost, requests or tokens; token mix, with why answers missed the cache and each session's tools and system prompt; by model; by client; routing: fallbacks, sessions, changes of model inside a session) and Requests (ledger with each request's attempts, 200 at a time). Subscriptions and Local Runtime views. CSV export (unknown cost left empty, formula-like fields prefixed). | Unit tests (`token_mix`, `prefix_meter`, `routing_summary`, subscription and local usage); browser preview. CSV: no test |
| Estimated savings (`savings.rs`) | Answered requests priced again at one baseline model the user picks, against what routing cost. It counts what it leaves out (unknown costs, missing token counts, subscription requests), never treats unknown as $0, and shows where answers came from (free, local, subscription, paid, unknown). | Unit test on a built ledger; browser preview. Not checked against a real ledger |
| Usage alerts | A client at 80% of its limit or over, a day costing three times the week's average (at least $0.50), five unknown costs in a day, half of the last hour failing. In Usage, Home and as notifications. Recorded figures only. | Unit test per rule; browser preview |
| Repeated tool output (shadow) | Tool outputs of 200 characters or more repeated within a request are counted, never removed. Shown in Usage. | Unit test per API format; browser preview |
| Cache lifetime (shadow, ADR-036) | On models that bill cache writes: the answers that missed the cache after a pause of five minutes to an hour, the tokens they wrote again, and what a one-hour cache lifetime or a keep-alive every 4.5 minutes would have cost and saved, at current prices (`store::cache_lifetime`, from the ledger, no new column). Usage → Cache Lifetime. Nothing is sent. | Unit test (`a_longer_cache_lifetime_is_weighed_from_the_pauses_in_each_session`: pauses under five minutes, within the hour and over it, a model without cache writes left out); `pnpm ui-check`; browser preview. Not yet read on real sessions |
| Clearing old tool results (shadow, ADR-036) | For OpenAI-format clients' requests: what Anthropic's context editing at its defaults would clear (past 100,000 prompt tokens, all but the last three tool results) and the tokens a new clearing would make the provider write again, per request (schema v37). Usage → Clearing Old Tool Results weighs the input saved against those writes at current prices. Nothing is cleared. | Unit tests (`shadow::clearing` per format, `clearing_shadow` net with and without sessions, prices and cache reads); `pnpm ui-check`; browser preview. Not yet read on a real long session |
| Retention (`retention.rs`) | Keeps the ledger 30, 90 or 365 days, or forever (the default). Never deletes a row of the current month or one a task attempt refers to. | Unit tests; browser preview |

## Key rings and pools

| Part | What it does | Verified by |
| --- | --- | --- |
| Key rings (`gateway/keys.rs`, ADR-038) | Several keys on one connection, each confirmed as a separate account. In order, round robin, least used today, or quota-aware. A session keeps its key. 401 blocks a key, 402 or 429 cools it, and the next key answers in the same request. The ledger names the key. A key from another account is added in Providers or with `pygg gateway key`. Not for web sessions, sign-ins or local runtimes. | Store tests (`gateway/keys/tests.rs`); fake-provider tests (rate-limited and rejected keys skipped, round robin, sessions, per-key limits); `pnpm ui-check` with a ring of three keys. Not used with real providers |
| Pools (ADR-034) | Connections of one provider, each with a confirmed independent quota; `<pool>/<model>` tries them in order across quota-group errors. Web sessions of one site may pool, each confirmed as another account; the account used least today goes first. | Unit tests; browser preview |

## Web sessions and the guard (`websession/`, ADR-035)

| Part | What it does | Verified by |
| --- | --- | --- |
| Web-session connections | Muse, ChatGPT, Perplexity, DeepSeek and Grok, each through a sign-in in its own WebView2 window and profile, with one model. The user must tick the account-risk warning. Each connection is served as a Chat Completions endpoint on a loopback port, so routing, limits and the ledger apply. Each request opens a new chat, types the flattened conversation and streams the reply. No tools or images; never chosen by `auto`. Billed as free ($0) by default. Pygg types only on the site's own host. Check Sign-In records Signed in, Not signed in, Needs you or Page changed. | Tests with a scripted page (`websession/tests.rs`); live on all five sites; [benchmark](../benchmark/reports/2026-11-13-web-sessions.md): 47 of 48 short tasks correct |
| The guard (`guard.rs`) | Every request is weighed. Prompts over 12,000 characters are refused (413) before the page is touched. A rest after each reply (10 s base plus weight and reading time, at most 3 minutes); at most 5 requests in any 10 minutes, 15 an hour, and a daily limit per connection (50 by default). A usage-limit notice, a check, a changed page or a "slow down" reply backs the account off (floors from 30 minutes to 6 hours, doubling). A suspension notice turns the connection off. DeepSeek has lower limits (4,000-character prompts, 6 requests an hour, 30 a day). Weight and rest are written before the site is touched. Once Send was pressed, no other web session gets that prompt. The app's timing cannot go below the guard's floors. | Unit tests in `guard.rs` (including a replay of the run that got an account suspended); scripted-page tests; live: a 13,000-character prompt refused with 413 before the site. Rests and back-offs have not met a real site |
| Waits | First word: 60 s plus 1 s per 100 estimated prompt tokens, up to 240 s; a page still answering counts as started. | Unit tests; not re-run on the sites |

## Providers and the catalog

| Part | What it does | Verified by |
| --- | --- | --- |
| Presets | Cloud providers and local runtimes from each provider's own docs ([PROVIDERS](PROVIDERS.md)), plus custom OpenAI- and Anthropic-compatible endpoints. Keys go to the OS credential store. Models from `GET {endpoint}/models`, or added by hand; prices, billing and on/off per model. | Unit tests; live model lists (Gemini, Anthropic, Qwen, OpenRouter) |
| Free-model connections (ADR-032) | A connection kept for its free models (OpenRouter, Z.AI) has only its $0 models on; an out-of-credit 402 does not hold them. | Fake-provider tests |
| Sign in with OpenRouter (`oauth.rs`) | OpenRouter's OAuth PKCE; the issued key is kept like a typed one. | Unit tests against a fake; not signed in live |
| Checks and health strip (`checker.rs`) | Each enabled API connection's model list is read every 30 minutes while the gateway runs; Check All and Check Now on demand; checks kept seven days. Each row shows 24 hourly dots and a 0–100 score. | Unit tests in `store.rs`; browser preview. Not watched for a day in the native app |
| Auto readiness | Under the provider list: how many models `auto` can use for a coding tool's request, and why the others are left out (missing facts, unavailable now, outside the rules). | Unit test; browser preview |
| Test Model (`model_test.rs`) | One short request to a model or a combo through the real route, recorded under its own client. Paid or unpriced models ask first. Also `pygg gateway test-model`. | Unit test; live on claude-haiku-5-5 and a missing model (404). Combo test: no live run |
| Model reliability | Share answered and median time over 30 days, from the ledger. | Unit test; browser preview |

## Skills & Prompts (`skills.rs`, ADR-039)

| Part | What it does | Verified by |
| --- | --- | --- |
| Library | Pygg's eight starter skills (all prompts: run only by `/name`) and the user's own (`skills`, schema v32). Name, description and instructions in the Agent Skills format; starters are duplicated, never edited. | Unit tests (names, limits, starters valid, save and delete rules); browser preview |
| Install and remove | One `SKILL.md` per install in `~/.claude/skills` (Claude Code; OpenCode reads it too) or `~/.agents/skills` (Codex and OpenCode), with `metadata.pygg-hash`; a prompt there also gets Codex's `agents/openai.yaml` (`allow_implicit_invocation: false`), so Codex leaves it out of every request too. A folder Pygg did not write, or a copy edited since, is left alone; an older copy of Pygg's is updated. Remove deletes only Pygg's unchanged copy. | Unit tests on a scratch home (install, update, hand edit refused, other skill refused, removal, Codex's policy file written, updated, refused when edited and removed); `pnpm ui-check` with installs, failures and a long library. Not yet installed into real agents and used from them |
| Sent With Every Request | Per agent, the global `CLAUDE.md` or `AGENTS.md` counted whole and each listed skill or command by name and description, as the gateway estimates tokens. Codex's count leaves out a skill whose `agents/openai.yaml` turns implicit use off. A skill found there can be added to the library. | Unit test over scratch agent folders; browser preview. Estimates not yet compared with a provider's token count |
| Projects | Folders the user adds (`skill_projects`, schema v35) become places to install: `.claude/skills` and `.agents/skills` at the project's root, Codex's `agents/openai.yaml` included. Only listed folders are written to; the home folder is refused; a folder is forgotten only once Pygg's copies in it are removed. Per project, what each agent adds to every request from the root's `CLAUDE.md`, `.claude/CLAUDE.md`, `CLAUDE.local.md`, `AGENTS.override.md` or `AGENTS.md` and its skills and commands. | Unit test on a scratch project (refused before it is added, home folder refused, install in both folders, count per agent, import, removal); `pnpm ui-check` with projects, a long path, a missing folder and failures; browser preview. Not yet used from a real agent in a project |

## CLI, npm and coding-tool set-up

| Part | What it does | Verified by |
| --- | --- | --- |
| `pygg-cli` (`cli/`) | One console program: a bare `pygg` shows the gateway, providers, tools and the next command; `providers`, `models`, `logs`, `usage`, `status`, `doctor` (with `--json`); `setup`, `run`, `mcp setup`, `gateway …` (serve, connections, keys, combos, model facts, clients, `auto` simulation), `diagnostics`. Exit codes 0/1/2/3. Reads open the store read-only. Builds without the window (`--no-default-features`) for Linux and macOS. | Unit tests in `cli/`; run read-only against the installed app's store |
| npm (ADR-037) | `npm install -g @pygg/cli@dev`: a launcher that runs the native program from `@pygg/win32-x64`, `darwin-arm64`, `darwin-x64` or `linux-x64` (glibc), and says what to do where none fits. Packed by CI on all three systems and installed back. `0.1.0-dev.2` published for all four, the `latest` tag included ([steps](../packages/npm/PUBLISHING.md)). | Launcher tests (`node --test`); CI install-back on all three systems; the published Windows and Linux packages installed from the registry and run. The macOS packages only built and installed back by CI, not run on a Mac |
| One-click set-up (`integrations.rs`, ADR-022) | CLI Tools: Codex and OpenCode models through Pygg, and the Claude Code launcher. Agents: Pygg's MCP server for Claude Code, Codex and OpenCode. Each previews, backs up, makes its own key, writes, checks, and undoes on failure; removal revokes the key. Codex's `config.toml` is edited with `toml_edit` and left byte-identical after removal; its profile is `pygg.config.toml`, which names `pygg-models.json`, a catalog of Pygg's models with their context sizes and Codex's `apply_patch` (kept current, removed with the set-up). Claude Code's own settings are never changed. Also `pygg setup <tool>` and `pygg mcp setup <tool>`. | Unit tests (Codex `config.toml` fixtures with comments, profiles, CRLF and Windows paths; OpenCode key order; refusals); live set-up and removal with Claude Code 2.1.322, Codex CLI 0.163.0 and OpenCode 1.18.4 on scratch configurations; Codex through the one-click row answered via claude-haiku-5-5; the catalog loaded by Codex 0.163.0 (178 models) and its `apply_patch` used on Qwen 3.8 Flash |
| Launch sessions (`launcher.rs`, `cli/run.rs`) | `pygg run claude|codex|opencode` starts the tool through Pygg for that launch only, with a fresh `launch-<uuid>` session, after checking the gateway answers. `pygg run claude` sets every model slot to `auto` (or `--model`) and removes the host's Anthropic and session variables; a plain `claude` keeps the user's plan. | Unit tests; live: Claude Code reached the gateway with the launcher's key (refused with 422, no provider connected) |
| Session receipt and status line (`launches.rs`) | When a tool started by `pygg run` exits: the launch's known cost (unknown and subscription requests counted apart), cache share, models, fallbacks, cost per request at the start and end of the session, the saving estimate for the session, and what changed in the Git repository it ran in (commits, changed lines since the start). The launch is kept in `launches` (schema v38) with the repository's folder, branch and commit ids only. `pygg statusline` prints the launch's cost in one line; `pygg run --statusline claude` sets it up for that launch. | Unit tests (`launches::tests`, Git in a scratch repository). Not yet run live with a tool's real session |
| Request classes (`gateway/hints.rs`) | `pygg run claude` turns on Claude Code's gateway hint headers; the gateway keeps each request's class (`main`, `subagent`, `workflow`, `compaction`, `auxiliary`), subagent type and id and prompt id (schema v39) and forwards none of them. The receipt, `pygg statusline`, `pygg usage` and Usage → Token Mix → By request kind show money by class, subagents, the most started by one prompt and the most working at once. | Unit tests (header checks, classes, agents and their overlap); gateway test (recorded, not forwarded); `pnpm ui-check`; browser preview. Not yet read on a real Claude Code session |
| Claude Code by hand | Base URL, a client key and `ANTHROPIC_CUSTOM_HEADERS` for the session header. | Live: headless Claude Code 2.1.317, 4 turns with a sub-agent and `/compact`, 12 of 12 requests with the session header |

## Desktop app and interface

| Part | What it does | Verified by |
| --- | --- | --- |
| Screens | Gateway: Home, Providers, Endpoint (base URLs, API keys), CLI Tools, Combos, Usage, Policies, Skills & Prompts. App: Guide, Settings. Smart Tasks (on hold): Tasks, Workspaces, Agents. Command palette (`Ctrl+K`) and shortcuts. | Browser preview; `pnpm ui-check` on every screen at five sizes; screenshots of the native window. No automated interface tests |
| Start-up and tray (`shell.rs`) | Opens the store, starts the gateway unless autostart is off. Tray icon; closing keeps the gateway running in the tray (a switch); open at sign-in through the user's Run key. Notifications name no task, file, client or amount (ADR-010). | Native app on a scratch store: close to tray with the gateway answering, second launch restores the window, notifications seen |
| Diagnostics (`diagnostics.rs`) | Settings → Save Diagnostics or `pygg diagnostics --out`: one redacted zip from an allowlist of fields ([SECURITY](SECURITY.md)). | Unit tests |
| Look and name | DownFetch's tokens with a green accent, light and dark. Pygg's name and icons from `branding/`. Traffic-light window controls (ADR-012); snap layouts on hover through a native overlay (`snap.rs`). | Browser preview in both themes; debug build: window title, icon, `/health`. Snap layouts: debug build at 100% scaling only |
| Language | English and Vietnamese. Strings added after the first translation pass are English only, written with `t()`. Task-engine messages are English templates translated in the app (`engineText.ts`), kept equal by a Rust test. | Rust test of the template lists. Vietnamese incomplete |
| Installer | NSIS (per user) and MSI from `pnpm tauri:build`; ships `pygg-cli`, `pygg-mcp` and the headless programs. The app's program also serves MCP (`<app> mcp`). | Installed, run, uninstalled and reinstalled on the development PC; installed by hand from Explorer. Not on a clean machine; not signed |

## Smart tasks (experimental, on hold)

Built and kept, with no new work until they are re-evaluated. Details: [AGENTS](AGENTS.md).

| Part | What it does | Verified by |
| --- | --- | --- |
| Agent CLIs (`agents.rs`, ADR-026) | Claude Code, Codex CLI, Gemini CLI and OpenCode run with their own sign-in in a Git worktree; Pygg never reads that sign-in. Cost recorded as unknown, with the agent's own estimate as its source. | Unit tests; live: Codex (ChatGPT plan) and OpenCode (free model) fixed a bug; Gemini CLI refused by Google |
| Task engine (`tasks.rs`) | A ladder of agent steps against the workspace's approved checks; a failed check hands its output to the next step; paid steps wait for approval with a spending cap (ADR-010); daily budgets; stop, interrupted after restart, retry; Keep as Branch, Export Patch, Discard. A step's cost comes from Pygg's ledger when its requests went through the gateway. | Tests with a scripted agent in real worktrees; live: the cascade escalating from OpenCode's free model to Codex; a paid step approved in the app with a $0.10 cap, cost $0.0093 from the ledger |
| From the IDE (`mcp.rs`, `task_api.rs`, ADR-027) | MCP tools `pygg_run_task`, `pygg_task_status`, `pygg_task_diff`, `pygg_keep_result`, `pygg_discard_result`, `pygg_stop_task`, `pygg_list_tasks`, `pygg_agents`, forwarding to `/pygg/v1/tasks`. Only keys with **May start tasks**; no API approves a paid step. | Live from real Codex and Claude Code sessions |
| Quality signals (ADR-029) | Retry, keep, discard and similar events recorded per result; summed by `agent quality`. Not shown in the interface. | Unit tests |

## Benchmark harness

| Part | What it does | Verified by |
| --- | --- | --- |
| `bench` (`bench.rs`) | Runs cases through agents or the task engine, costs each attempt from the gateway's ledger by its session, enforces a hard `--budget-usd`, fails an attempt that changes a protected test or check file. Cases and method: [BENCHMARKS](BENCHMARKS.md), [benchmark/README](../benchmark/README.md). | Its runs, below |
| Web scripts | `scripts/bench-web.py` (short tasks) and `scripts/bench-web-agent.py` (coding tasks in one reply). | Its runs, below |
| Reports | [C3 first pass](../benchmark/reports/2026-11-11-c3-first-pass.md), [N2 savings](../benchmark/reports/2026-11-12-n2-savings.md), [P0 optimization](../benchmark/reports/2026-11-12-p0-optimization.md), [handoff A/B](../benchmark/reports/2026-11-12-h1-handoff-ab.md), [real repository](../benchmark/reports/2026-11-13-ccht-real-repository.md), [web sessions](../benchmark/reports/2026-11-13-web-sessions.md), [web coding](../benchmark/reports/2026-11-13-web-coding.md), [prompt cache](../benchmark/reports/2026-11-14-codex-claude-prompt-cache.md), [gateway overhead](../benchmark/reports/2026-11-14-gateway-overhead.md) | — |

## Live runs on record

| Provider or path | What answered |
| --- | --- |
| Anthropic | Native passthrough with cache parity (three models); OpenAI format and Responses API to Claude, with thinking; Codex and Claude Code through the gateway; benchmark runs |
| Gemini | Both API formats, streamed and not; tool calls with signatures; combo fallback; `auto`; headless Claude Code |
| Qwen token plan | Both formats; reasoning across the translation; Codex tool round trip; benchmark runs through `auto` |
| OpenRouter | A free model in both formats, streamed and not; a real 429 cooled down that one model |
| Ollama (Docker, GPU) | Both formats; benchmark runs with `qwen3:8b` |
| DeepSeek API | A real 402 (no balance), relayed and recorded |
| OpenAI, Groq, Mistral and others | Real 401 answers only (error fixtures) |
| Web sessions | All five sites through the app's gateway |

## Not verified yet

- **Successful API requests to OpenAI, Groq, DeepSeek and Mistral.** Only their error answers were seen live.
- **Anthropic-compatible endpoints other than Anthropic's own.**
- **What `x-session-id` adds on OpenRouter**, apart from its own routing, and OpenAI's `prompt_cache_key`. The header is sent; its effect is not measured.
- **Gemini's quota answer (429 `RESOURCE_EXHAUSTED`) as a fixture.** Needs a model the key has no quota for; its `RetryInfo` is read in a unit test. The 429 hold and the prefill fallback are fake-provider tested only.
- **The quota-group hold, HTTP-date resets, the attempt journal and at-risk accounting against a real provider.**
- **DeepSeek's thinking mode with tools across the Anthropic translation.** Needs a funded account.
- **Claude Code in an interactive terminal**, and the launcher answered by a real provider.
- **`pygg run codex` and `pygg run opencode` against the installed tools**, and `pygg setup` against a real tool's configuration.
- **Key rings, cheapest-first combos and combo tests with real providers.**
- **Estimated savings and the token mix on a real ledger.**
- **Web sessions over many days**, the guard's rests and back-offs on a real site, and a web pool of two accounts. The guard will not be stress-tested on the owner's accounts.
- **The native window clicked through**: most screens are checked in the browser preview only; scaling, snap layouts above 100%, Narrator and NVDA, and contrast themes (ADR-012) are open.
- **The installer on a clean Windows machine**, and any build on macOS.

## Designed, not built

- The Setup Assistant ([SCREENS](design/SCREENS.md)).
- Quota tracking for subscriptions: their providers state nothing Pygg can read.
- Smart task routing beyond the task engine (task analysis, proposed plans, task policies). On hold.
- The Vietnamese translation of strings added since the first pass. Done last.
