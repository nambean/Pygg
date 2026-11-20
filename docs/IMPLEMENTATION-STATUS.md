# Implementation status

> **Status:** Authoritative · what is built and how it was verified · reviewed 2026-11-21

What exists in code and how each part was verified. Next: [ROADMAP](ROADMAP.md); why: [DECISIONS](DECISIONS.md). Code paths are under `apps/desktop/src-tauri/src` unless stated.

| Verification level | Meaning |
| --- | --- |
| Unit tests | The module alone |
| Fake-provider tests | End to end through a real gateway against a fake provider (`gateway/tests.rs`) |
| Live | A real provider answered through the gateway; provider and model named |
| Browser preview | The interface on labelled demo data (`pnpm dev`, ADR-014); `?empty` for a first run, `?stress` for long data |
| `pnpm ui-check` | Every screen and a set of clicked states (sheets, menus, tested models) with `?stress` data, headless Edge or Chrome, at 1024×640, 1440×900, 1990×1030, 819×512 and 683×427 (minimum window at text size 125% and 150%); fails on overflow, after first finding a planted one |

**CI:** `cargo clippy -- -D warnings` (app `--all-features`, console program `--no-default-features`), `cargo fmt --check`, `cargo test --lib --tests`, UI typecheck and build, schema reference check, licence check, layout check. `cargo audit` and `pnpm audit` on a lockfile change and weekly.

## P0: cost-safe gateway

| Item | Built | Fake-provider tests | Live | Provider parity |
| --- | --- | --- | --- | --- |
| Same-format passthrough | Yes | Yes | Anthropic native (claude-haiku-4-5, claude-haiku-5-5, claude-sonnet-5-5); translated to Gemini and Qwen | Anthropic: identical cache writes and reads, streamed or not, 5-minute and 1-hour TTL; thinking with a tool; two-turn tool round trip; citations; `count_tokens` |
| Error-scoped fallback | Yes | Yes | Gemini 404 cooldown and skip; OpenCode combo fallback; as fixtures: real 401, 400, 404 from 8 providers, 402 (Requesty, SiliconFlow), 403 (OpenRouter, Gemini), 429 and a timeout after sending (OpenRouter) | n/a |
| Budgets and complete ledger | Yes | Yes | Ledger and prices (Gemini, Anthropic); a strict client limit held every benchmark request | n/a |
| Explicit sessions | Yes | Yes | Headless Claude Code: header on 12 of 12 requests | n/a |
| `auto` routing | Yes | Yes | Gemini, Anthropic, Qwen token plan, headless Claude Code | n/a |

None is release-ready: see the [release gates](ROADMAP.md#release-gates).

## Gateway (`gateway/`)

| Part | Built | Verified |
| --- | --- | --- |
| Server (`mod.rs`) | Loopback only, `127.0.0.1:8741` by default; non-loopback `Host` refused (DNS rebinding); client key required (`Authorization: Bearer` or `x-api-key`); `GET /v1/models`, `POST /v1/chat/completions`, `/v1/messages`, `/v1/responses`, `count_tokens`, `/health` | Fake-provider tests; live (Gemini, Anthropic) |
| One gateway per store | Exclusive OS lock on `<store>.gateway-lock`; a second gateway refused before sending; a stopped gateway holds the store until in-flight attempts settle; ownership generations mark another gateway's entries interrupted | Lifecycle tests; `tests/two_gateways.rs` (two real processes) |
| Same-format passthrough | Wire format `openai` or `anthropic` per connection; JSON in the client's key order, only `model` replaced (plus ADR-036's cache marker and session key); `anthropic-version`, `anthropic-beta`, `cache_control`, thinking, citations pass; answer, rate-limit and request-id headers unchanged | Fake endpoint echoing its input; live parity on three Anthropic models (`scripts/anthropic-parity.ps1`) |
| Anthropic → OpenAI (`anthropic.rs`) | System, text, images, tools, `tool_choice`, tool calls and results, streamed or not; cache markers kept for OpenRouter; `reasoning_content` or `reasoning` returned as a `thinking` block signed `pygg-reasoning-v1`, sent back where the provider takes it; dropped fields named in `x-pygg-dropped` | Unit tests; live: Gemini (tool round trip, `dropped: cache_control`), Qwen token plan (thinking with a tool, two turns, streamed) |
| OpenAI → Anthropic (`openai_to_anthropic.rs`) | Messages, images, tools, tool calls, limits, `stop`, `stream`, usage; `reasoning_effort` → `thinking` per the model's facts (adaptive with effort, or a budget), returned as `reasoning_content`; no output limit → 64,000 tokens streamed, 16,000 otherwise, capped at the model's maximum; agent turns (with tools) to Anthropic's API get automatic prompt caching (ADR-036) | Unit and fake-provider tests; live on claude-haiku-5-5 and claude-haiku-4-5 (plain, tool round trip, stream, thinking); caching: [report](../benchmark/reports/2026-11-14-codex-claude-prompt-cache.md) |
| Responses API (`responses.rs`, ADR-033) | `POST /v1/responses` on the same routing, limits and ledger; translated via Chat Completions, native where the preset serves `/responses`; `previous_response_id`, `conversation`, `background` refused (nothing stored) | Unit tests on captured Codex requests; fake-provider tests; live with Codex CLI on claude-haiku-5-5 and Qwen token plan |
| Streaming | Server-sent events pass through; usage from the last chunk; disconnect or broken stream recorded at unknown cost; a stream of only `[DONE]` is a failure | Fake-provider tests; live |
| Gemini signatures (`signatures.rs`) | `thought_signature` kept per tool call (30 days), put back for Gemini, stripped for others | Unit tests; live Gemini round trip |
| Explicit sessions | `x-pygg-session` (at most 128 visible ASCII characters) pins to the target that last answered, 2 hours sliding, per client key; after a fallback, back home once no warm cache is lost (home answered within 5 minutes, or both caches cold); response header `new`, `pinned` or `home`; never guessed from content; forwarded as `x-session-id` (OpenRouter) and `prompt_cache_key` (OpenAI), as `pygg-` + a hash of client and session, unless the client set its own | Fake-provider tests (the way home and forwarding: fake only); live (Gemini, headless Claude Code) |
| Cache marker (ADR-036) | One top-level `cache_control` on an OpenAI-format agent turn (with tools) to Anthropic's API or an `anthropic/…` model on OpenRouter; an Anthropic-format client's top-level marker crosses to OpenRouter; ledger cache hint `added` | Fake-provider tests; live on Anthropic ([report](../benchmark/reports/2026-11-14-codex-claude-prompt-cache.md)) and on OpenRouter `anthropic/claude-haiku-5.5`, Chat Completions and Responses API: 99.7% of later prompts read from cache ([report](../benchmark/reports/2026-11-16-openrouter-cache-live.md)) |
| Error-scoped fallback (`policy.rs`) | Fallback only before the first byte reaches the client. 401 blocks the connection until its key is replaced, except an aggregator's `upstream_error` with 401, 402, 403 or 429 (cools that model only). 402, 403, 429 cool the model or connection; no move within the quota group during the cooldown. Anthropic 400 "credit balance is too low" = out of credit; 400 "reached your specified API usage limits" held until its "regain access on" date (UTC, up to 45 days); 400 prefill refusal falls back. Resets from `Retry-After`, `retry-after-ms`, Gemini's `RetryInfo`. Read timeout, 408, 504, 524 stop at unknown cost; other 5xx stop unless the client allows retries | Fake-provider test per rule; real error fixtures (`tests/provider_errors.rs`); live: missing model cooled and skipped (Gemini), 429 cooldown (OpenRouter), 402 (DeepSeek), aggregator refusal (MixAICloud's GLM models) |
| Circuit breaker | Wait doubles per failure in a row, up to 15 minutes; an unreachable connection is probed through its free model list | Unit tests |
| Overhead | Median about 4 ms per short request, 5 ms per 400 kB prompt; about 230 requests a second with 16 clients at once | Release-mode `gateway::tests::overhead` ([report](../benchmark/reports/2026-11-14-gateway-overhead.md)) |

## Routing: named models, combos and `auto`

| Part | Built | Verified |
| --- | --- | --- |
| Model names | `<connection>/<model>`, `<pool>/<model>`, a combo name, `auto`, `auto/cheap`; `auto` reserved | Fake-provider tests; live |
| Combos | Members in order, or cheapest first (per request by estimated cost; free, local, subscription at $0; unknown prices last); editor warns when two members share a quota group | In order: live (Gemini, OpenCode); cheapest first: fake-provider test only |
| Combo templates (`lib/comboTemplates.ts`) | Eight templates ([SCREENS](design/SCREENS.md)) resolved into a saved list, with each member's reason and each model left out; unknown facts never qualify | Browser preview; no automated test |
| `auto`, `auto/cheap` (`auto.rs`, ADR-006) | Hard filters, each with a reason: the client's connections, API format, cooldowns and quota holds, known tools/images/thinking support, data-use terms, context and output fit, a statable cost, and for `auto` a tier at or above the client's minimum (T2 by default). Rules, no classifier: `auto` takes the lowest eligible tier, then local, free, subscription, paid, then cost; `auto/cheap` the lowest expected cost from each model's own answers to prompts of this size over 30 days (mean output and cache-read share, from 20 answers). First five form the route; reason in `x-pygg-auto` and the ledger; nothing eligible → 422 with reasons by connection | Unit tests per filter and the ranking; fake-provider tests; live: Gemini, Anthropic, Qwen token plan, headless Claude Code |
| Role aliases `auto/plan`, `auto/code`, `auto/test`, `auto/review` (`auto.rs`, ADR-041) | `auto`'s filters, then the user's own evidence (from ten finished attempts on cards of the role), then facts and tiers in bands, cheaper first within a band ([design/BRAIN.md](design/BRAIN.md)); the reason names the fact; ledger route kind = the alias; facts off → tier alone; listed to every client, Codex's catalog, OpenCode's provider | Unit tests; fake-provider test (header, ledger). `auto/code` measured once, before the bands ([report](../benchmark/reports/2026-11-20-all-models.md)); banded order and the other three not measured, no claim (ADR-013) |
| 429 hold | A model with a 429 and no usable reset, or two in a row, out of `auto` for 10 minutes, doubling up to a day; 429s from one parallel burst count as one strike; named routes unaffected | Fake-provider test |
| Model facts | Tier, context, maximum output, tools, images, thinking, each with a source: provider's model list, models.dev catalog (at most one download a day, ADR-031), tier table (`tiers.rs`, `curated:v2`), price band, or the user; user values never overwritten | Unit tests |
| Stated quotas (`quota.rs`) | Rate-limit headers read as windows (Anthropic, OpenAI and Groq, OpenRouter forms); `auto` skips a used-up model until reset; OpenRouter `/key` and DeepSeek `/user/balance` read at start-up, on refresh, on Check Now; nothing estimated | Unit tests |
| Simulator | Combos → How auto chooses: the route for a described request, every model left out and why; sends nothing | Browser preview |

## Policy, budgets and the ledger

| Part | Built | Verified |
| --- | --- | --- |
| API keys (client keys) | SHA-256 hash in the store, a copy in the OS credential store (Show, Copy); per key: monthly limit (estimated or strict), session and UTC-day limits (off by default), in-flight cap (8 by default), unknown-price policy, retries after a server error (off by default), minimum tier for `auto`, no-training requirement, allowed connections; defaults for new keys in Policies; revoked keys deletable | Fake-provider tests; unit tests. Show and Copy: no test |
| Budgets (P0-3) | Each attempt journaled before sending, else nothing sent; a limited client holds an estimate atomically and settles once from real usage; strict holds the most a request could cost and refuses unknown prices or output limits; at-risk spend counts; no paid send while a settlement waits; a disconnect still leaves an entry | Fake-provider tests (concurrent requests against a limit that fits one, CJK and cache-write estimates, read-only store, disconnect) |
| Session and day limits (schema v40) | Per `x-pygg-session`, and per key since 00:00 UTC; strict counts holds, estimated sends while counted spend is under the limit (a hold is 15 to 20 times an agent's usual turn); when reached, 402 (not 429, which agents retry; `billing_error` on the Anthropic API) naming the limit; a running request never cut | Fake-provider tests; live on Gemini 3.1 Flash Lite, headless gateway, store copy ($0.006): refused by spend, other sessions unaffected, a stream finished after its limit was lowered, the day limit, the status line. Not yet in the installed app |
| Ledger and cost status (`pricing.rs`, `store.rs`) | Every attempt, failed included. Actual: the provider reported the charge. Estimated: from a price, or free or local billing. Unknown: everything else, never $0. Per attempt: request id (`x-pygg-request-id`), fallback place, billing, route choice, cache hint, key-ring key, likely cache-miss cause, size and fingerprint of tools and system prompt (no text). An unreported cache read is empty, not 0 | Unit and fake-provider tests; cache-miss causes and OpenRouter cache writes live ([report](../benchmark/reports/2026-11-16-openrouter-cache-live.md)); live ledgers checked in the app |
| Data-use terms | Per connection: no training, may train, or not recorded, with a source; a client can require no training (filters combos, fallbacks, pins, `auto`, the model list); Pygg fills nothing in | Fake-provider tests |
| Usage screen | Summary (cost chart, token mix with cache-miss causes, by model, by client, routing); Requests (ledger with attempts, 200 at a time); Subscriptions and Local Runtime views; CSV export (unknown cost empty, formula-like fields prefixed) | Unit tests (`token_mix`, `prefix_meter`, `routing_summary`); browser preview. CSV: no test |
| Estimated savings (`savings.rs`) | Answered requests repriced at one user-picked baseline model, against routed cost; counts what it leaves out; unknown never $0; shows where answers came from | Unit test on a built ledger; browser preview. Not on a real ledger |
| Usage alerts | A client at 80% of its limit or over; a day at three times the week's average (at least $0.50); five unknown costs in a day; half of the last hour failing. In Usage, Home and notifications; recorded figures only | Unit test per rule; browser preview |
| Repeated tool output (shadow) | Tool outputs of 200+ characters repeated within a request counted, never removed; in Usage | Unit test per API format; browser preview |
| Cache lifetime (shadow, ADR-036) | Models that bill cache writes: misses after a 5-to-60-minute pause, tokens written again, and what a one-hour lifetime or a 4.5-minute keep-alive would cost and save at current prices (`store::cache_lifetime`, no new column); Usage → Cache Lifetime; sends nothing | Unit test (`a_longer_cache_lifetime_is_weighed_from_the_pauses_in_each_session`); `pnpm ui-check`; browser preview. Not yet read on real sessions |
| Clearing old tool results (shadow, ADR-036) | OpenAI-format clients: what Anthropic's context editing at its defaults would clear (past 100,000 prompt tokens, all but the last three tool results) and the cache writes it would cause, per request (schema v37); Usage → Clearing Old Tool Results nets input saved against those writes; clears nothing | Unit tests (`shadow::clearing`, `clearing_shadow`); `pnpm ui-check`; browser preview. Not yet on a real long session |
| Retention (`retention.rs`) | Ledger kept 30, 90 or 365 days, or forever (default); never deletes the current month's rows or one a task attempt refers to | Unit tests; browser preview |

## Key rings and pools

| Part | Built | Verified |
| --- | --- | --- |
| Key rings (`gateway/keys.rs`, ADR-038) | Several keys on one connection, each confirmed a separate account; in order, round robin, least used today, or quota-aware; a session keeps its key; 401 blocks a key, 402 or 429 cools it, the next key answers in the same request; the ledger names the key; added in Providers or with `pygg gateway key`; not for web sessions, sign-ins, local runtimes | Store tests (`gateway/keys/tests.rs`); fake-provider tests; `pnpm ui-check` with three keys. Not with real providers |
| Pools (ADR-034) | Connections of one provider, each with a confirmed independent quota; `<pool>/<model>` tries them in order across quota-group errors; web sessions of one site may pool (each confirmed another account; least used today first) | Unit tests; browser preview |

## Web sessions and the guard (`websession/`, ADR-035)

| Part | Built | Verified |
| --- | --- | --- |
| Web-session connections | Muse, ChatGPT, Perplexity, DeepSeek, Grok, Gemini, each in its own WebView2 window and profile, one model each; account-risk warning ticked per connection; served as Chat Completions on a loopback port (routing, limits, ledger apply); a new chat per request; no tools or images; never chosen by `auto`; $0 by default; types only on the site's own host; Check Sign-In: Signed in, Not signed in, Needs you, Page changed | Scripted-page tests (`websession/tests.rs`); live on all six (Gemini: sign-in, a text answer, a picture); [benchmark](../benchmark/reports/2026-11-13-web-sessions.md): 47 of 48 short tasks correct |
| Media from replies | Pictures, videos and download links that appear after Send come back as Markdown links after the text; `pygg chat` and `pygg ask` save them | Unit tests (scripted page, local provider); live on Muse (a picture and a video from `blob:` addresses saved by `pygg ask`); a picture from Gemini |
| The guard (`guard.rs`) | Every request weighed; over-long prompts refused (413) before the page; rest after each reply, caps per 10 minutes, hour and UTC day, back-off after a sign of trouble, lower DeepSeek ceilings ([numbers](PROVIDERS.md#the-guard)); a suspension notice turns the connection off; weight and rest written before the site is touched; after Send no other web session gets the prompt; app timing cannot go below the guard's floors | Unit tests (with a replay of the run that got an account suspended); scripted-page tests; live: a 13,000-character prompt refused with 413 before the site. Rests and back-offs not met on a real site |
| Waits | First word: 60 s + 1 s per 100 estimated prompt tokens, up to 240 s; a page still answering counts as started | Unit tests; not re-run on the sites |

## Providers and the catalog

| Part | Built | Verified |
| --- | --- | --- |
| Presets | Cloud providers and local runtimes from each provider's docs ([PROVIDERS](PROVIDERS.md)), plus custom OpenAI- and Anthropic-compatible endpoints; keys in the OS credential store; models from `GET {endpoint}/models` or by hand; per-model price, billing, on/off | Unit tests; live model lists (Gemini, Anthropic, Qwen, OpenRouter) |
| Free-model connections (ADR-032) | A connection kept for free models (OpenRouter, Z.AI): only $0 models on; an out-of-credit 402 does not hold them | Fake-provider tests |
| Sign in with OpenRouter (`oauth.rs`) | OAuth PKCE; the issued key kept like a typed one | Unit tests against a fake; not signed in live |
| Checks and health strip (`checker.rs`) | Model lists read every 30 minutes while the gateway runs, and on Check All or Check Now; kept seven days; 24 hourly dots and a 0–100 score per row | Unit tests in `store.rs`; browser preview. Not watched for a day in the native app |
| Auto readiness | Under the provider list: how many models `auto` can use for a coding tool's request, and why the others are out | Unit test; browser preview |
| Test Model (`model_test.rs`) | One short request to a model or combo through the real route, under its own client; paid or unpriced asks first; also `pygg gateway test-model` | Unit test; live on claude-haiku-5-5 and a missing model (404). Combo test not live |
| Model reliability | Share answered and median time over 30 days, from the ledger | Unit test; browser preview |
| Model facts feed (`gateway/facts.rs`, `scripts/model-facts.py`, ADR-040) | Per-role scores from Terminal-Bench, SWE-bench Verified, Aider, the Arena dataset; built weekly by the `model-facts` workflow, signed (ed25519), published under the `model-facts-v1` tag; fetched and verified once a day; ranks enabled models per role, the user's own evidence first; `pygg models facts`, `pygg gateway facts`; Refresh on Brain Storming. Details: [design/BRAIN.md](design/BRAIN.md) | Unit tests; builder `selftest`; a real build (1,146 facts) ranked on a store copy (187 models); live: the workflow published a feed, `gateway facts --refresh yes` fetched, verified and replaced it on a store copy. Not yet in the installed app |

## Skills & Prompts (`skills.rs`, ADR-039)

| Part | Built | Verified |
| --- | --- | --- |
| Library | Eight starter skills (all prompts, run only by `/name`) and the user's own (`skills`, schema v32); Agent Skills format; starters duplicated, never edited | Unit tests (names, limits, starters valid, save and delete rules); browser preview |
| Install and remove | One `SKILL.md` per install in `~/.claude/skills` or `~/.agents/skills`, with `metadata.pygg-hash`; prompts also get Codex's `agents/openai.yaml` (`allow_implicit_invocation: false`); folders Pygg did not write and edited copies left alone; Remove deletes only Pygg's unchanged copy | Unit tests on a scratch home; `pnpm ui-check`. Not yet installed into real agents |
| Sent With Every Request | Per agent: global `CLAUDE.md` or `AGENTS.md` and each listed skill or command, at the gateway's token estimate; a found skill can be added to the library | Unit test over scratch agent folders; browser preview. Not compared with a provider's token count |
| Projects | Folders the user adds (`skill_projects`, schema v35) as install targets (`.claude/skills`, `.agents/skills`); home folder refused; per-agent count of the root's instruction files, skills and commands | Unit test on a scratch project; `pnpm ui-check`; browser preview. Not yet used from a real agent |

## CLI, npm and coding-tool set-up

| Part | Built | Verified |
| --- | --- | --- |
| `pygg-cli` (`cli/`) | One console program ([CLI](design/CLI.md)): status, providers, models, usage, `doctor`, `setup`, `run`, `mcp setup`, `gateway …`, `diagnostics`; exit codes 0/1/2/3; reads open the store read-only; builds without the window (`--no-default-features`) for Linux and macOS | Unit tests in `cli/`; run read-only on the installed app's store |
| npm (ADR-037) | `npm install -g @pygg/cli@dev`: a launcher running the native program from `@pygg/win32-x64`, `darwin-arm64`, `darwin-x64` or `linux-x64` (glibc), saying what to do where none fits; tags `dev` and `latest`; newest `0.1.0-dev.8` (2026-11-20), the first listed in the MCP Registry ([versions](../packages/npm/PUBLISHING.md)) | Launcher tests (`node --test`); CI packs and installs back on all three systems; Windows x64 and Linux x64 installed from the registry and run. macOS only built and installed back by CI |
| One-click set-up (`integrations.rs`, ADR-022) | CLI Tools: Codex and OpenCode through Pygg, the Claude Code launcher; Agents: Pygg's MCP server for Claude Code, Codex, OpenCode; preview, backup, own key, write, check, undo on failure; Codex's `pygg-models.json` catalog; also `pygg setup <tool>`, `pygg mcp setup <tool>`. Details: [AGENTS](AGENTS.md) | Unit tests (Codex fixtures: comments, profiles, CRLF, Windows paths; OpenCode key order; refusals); live set-up and removal with Claude Code 2.1.322, Codex CLI 0.163.0, OpenCode 1.18.4 on scratch configurations; Codex answered via claude-haiku-5-5; catalog loaded by Codex 0.163.0 (178 models), `apply_patch` used on Qwen 3.8 Flash |
| Launch sessions (`launcher.rs`, `cli/run.rs`) | `pygg run claude\|codex\|opencode` for that launch only, fresh `launch-<uuid>` session, gateway checked first; `pygg run claude` sets every model slot to `auto` (or `--model`) and removes the host's Anthropic and session variables; a plain `claude` keeps the user's plan | Unit tests; live: Claude Code reached the gateway with the launcher's key (422, no provider connected) |
| Session receipt and status line (`launches.rs`) | On exit: known cost, cache share, models, fallbacks, cost per request at start and end, saving estimate, Git changes; kept in `launches` (schema v38, no file names or code); `pygg statusline`, `pygg run --statusline claude`; `pygg launches --by-repo` per repository and UTC week, as observations (ADR-036). Details: [AGENTS](AGENTS.md) | Unit tests (`launches::tests`, Git in a scratch repository, by-repository sums). Not yet live |
| Providers and models from the terminal (`cli/manage.rs`) | `providers`, `models`, `default`, `route`, `combos`, `keys`, `launches`, `savings`, `completion`, `help` ([CLI](design/CLI.md)); writes at this program's schema, never migrates; keys from an environment variable; web sessions refused | Unit tests; live on a scratch store with a local OpenAI-format provider: every command, `remove` refused without `--yes`, web-session preset refused. Not yet on the owner's store |
| Chat from the terminal (`cli/ask.rs`) | `pygg chat` (in memory, slash commands) and `pygg ask` (arguments or a pipe, `--json`, `-x`) ([CLI](design/CLI.md)); streamed through Chat Completions, then model, time, known cost; own key "Pygg CLI (ask)"; nothing kept (ADR-028) | Unit tests; live on a scratch store with a local provider: `ask`, a three-turn `chat`, `/cost`, `/model`, a refused message kept out. Not yet on the owner's Muse connection |
| Request classes (`gateway/hints.rs`) | `pygg run claude` turns on Claude Code's hint headers; the gateway keeps class (`main`, `subagent`, `workflow`, `compaction`, `auxiliary`), subagent and prompt ids (schema v39), forwards none; shown in the receipt, `pygg statusline`, `pygg usage`, Usage → Token Mix → By request kind | Unit tests; gateway test (recorded, not forwarded); `pnpm ui-check`; browser preview. Not yet on a real session |
| Claude Code by hand | Base URL, a client key, `ANTHROPIC_CUSTOM_HEADERS` for the session header | Live: headless Claude Code 2.1.317, 4 turns with a sub-agent and `/compact`, 12 of 12 requests with the header |

## Desktop app and interface

| Part | Built | Verified |
| --- | --- | --- |
| Screens | Gateway: Home, Providers, Endpoint (base URLs, API Keys), CLI Tools, Combos, Usage, Policies, Skills & Prompts. App: Guide, Settings. Smart Tasks (on hold): Tasks (with the Board), Workspaces, Agents, Brain Storming. Command palette (`Ctrl+K`), shortcuts | Browser preview; `pnpm ui-check` (every screen, five sizes); native-window screenshots. No automated interface tests |
| Start-up and tray (`shell.rs`) | Opens the store, starts the gateway unless autostart is off; tray icon; closing keeps the gateway in the tray (a switch); open at sign-in via the user's Run key; notifications name no task, file, client or amount (ADR-010) | Native app on a scratch store: close to tray with the gateway answering, second launch restores the window, notifications seen |
| Diagnostics (`diagnostics.rs`) | Settings → Save Diagnostics or `pygg diagnostics --out`: one redacted zip from a field allowlist ([SECURITY](SECURITY.md)) | Unit tests |
| Look and name | DownFetch's tokens, green accent, light and dark; name and icons from `branding/`; traffic-light window controls (ADR-012); snap layouts on hover through a native overlay (`snap.rs`) | Browser preview, both themes; debug build: title, icon, `/health`. Snap layouts: 100% scaling only |
| Language | English and Vietnamese; strings added after the first pass English only, in `t()`; task-engine messages as English templates translated in the app (`engineText.ts`), kept equal by a Rust test | Rust test of the template lists. Vietnamese incomplete |
| Installer | NSIS (per user) and MSI from `pnpm tauri:build`; ships `pygg-cli`, `pygg-mcp`, the headless programs; the app's program also serves MCP (`<app> mcp`). GitHub prereleases: `v0.1.0-dev.1` (2026-11-14, schema v31), `v0.1.0-dev.2` (2026-11-19, v40, from 67ac99b), `v0.1.0-dev.3` (2026-11-20, v44, from f186dde); each answered `/health` on a scratch store before publishing | Installed, run, uninstalled, reinstalled on the development PC; installed from Explorer. Not on a clean machine; not signed |

## Smart tasks (experimental)

Built in ADR-041's order. Design: [design/BRAIN.md](design/BRAIN.md); agents and task API: [AGENTS](AGENTS.md). The facts feed and role aliases are under Routing.

| Part | Built | Verified |
| --- | --- | --- |
| `pygg/brain` (`gateway/brain.rs`, ADR-041, migration 0042) | Model `pygg/brain` on any of the three APIs; planner picked by rule; reply parsed into questions, cards, dependencies and waves (`brain_schema_ok` per reply); a model and two fallbacks per card by role; refined by later calls in the session; ledgered under the client's session, route kind `brain`; detail screen under Tasks; MCP `pygg_brainstorm`, `pygg_model_facts`; API `/pygg/v1/brainstorms`, `/pygg/v1/facts` | Unit and fake-provider tests; `pnpm ui-check`; live (headless gateway, store copy): planner 402 failed cleanly, a sub-minimum saved pick waited, Gemini 3.8 Flash proposed, refined and closed a 16-card board. Not clicked in the installed app |
| The board (`board.rs`, migrations 0044–0045, ADR-042) | Approval writes the cards and the brainstorm's branch; a 3-second tick settles lanes, moves Discovery → Ready, pulls Ready into tasks under the WIP limits; cards build on the branch; paid steps still wait (ADR-010); measures from five done cards; Board and Brain Storming screens; API `/pygg/v1/board`, MCP `pygg_board`, `pygg_board_move`. Details: [design/BRAIN.md](design/BRAIN.md), [role sheet](design/SCREENS.md#models-for-a-role) | Unit tests; `pnpm ui-check`; live (dev build, store copy): approval, pull under the limit, a paid step approved, a turned-off model's fallback, Keep to Done, a held run, measures, own evidence first, an override, the stopped-gateway header. Not in the installed app |
| Agent CLIs (`agents.rs`, ADR-026) | Claude Code, Codex CLI, Gemini CLI, OpenCode with their own sign-in in a Git worktree; sign-in never read; cost unknown, the agent's estimate as its source | Unit tests; live: Codex (ChatGPT plan) and OpenCode (free model) fixed a bug; Gemini CLI refused by Google |
| Task engine (`tasks.rs`) | Ladder of agent steps against approved checks; a failed check's output handed on; paid steps wait for approval with a cap (ADR-010); daily budgets; stop, interrupted after restart, retry; Keep as Branch, Export Patch, Discard; step cost from the ledger when routed through the gateway | Scripted agent in real worktrees; live: cascade from OpenCode's free model to Codex; a paid step approved in the app, $0.10 cap, $0.0093 from the ledger |
| From the IDE (`mcp.rs`, `task_api.rs`, ADR-027) | MCP task tools ([AGENTS](AGENTS.md)) forwarding to `/pygg/v1/tasks`; only keys with **May start tasks**; no API approves a paid step; Claude Code plugin (`plugins/pygg`, the repository's marketplace); MCP Registry entry `io.github.nambean/pygg` (`packages/npm/pygg/server.json`), listed since `0.1.0-dev.8` | Live from real Codex and Claude Code sessions; plugin validated, installed and its server connected in a scratch Claude Code 2.1.322 |
| Quality signals (ADR-029) | Retry, keep, discard and similar events per result; summed by `agent quality`; not in the interface | Unit tests |

## Benchmark harness

| Part | Built | Verified |
| --- | --- | --- |
| `bench` (`bench.rs`) | Runs cases through agents or the task engine; costs each attempt from the ledger by session; hard `--budget-usd`; fails an attempt that changes a protected test or check file. Method: [BENCHMARKS](BENCHMARKS.md), [benchmark/README](../benchmark/README.md) | Its runs |
| Web scripts | `scripts/bench-web.py` (short tasks), `scripts/bench-web-agent.py` (coding tasks in one reply) | Its runs |
| Reports | Every published report: [BENCHMARKS](BENCHMARKS.md) | — |

## Live runs on record

| Provider or path | What answered |
| --- | --- |
| Anthropic | Native passthrough with cache parity (three models); OpenAI format and Responses API to Claude, with thinking; Codex and Claude Code through the gateway; benchmark runs |
| Gemini | Both API formats, streamed and not; tool calls with signatures; combo fallback; `auto`; headless Claude Code |
| Qwen token plan | Both formats; reasoning across the translation; Codex tool round trip; benchmark runs through `auto` |
| OpenRouter | A free model in both formats, streamed and not; a real 429 cooled that one model |
| Ollama (Docker, GPU) | Both formats; benchmark runs with `qwen3:8b` |
| DeepSeek API | A real 402 (no balance), relayed and recorded |
| OpenAI, Groq, Mistral and others | Real 401 answers only (error fixtures) |
| Web sessions | All six sites through the app's gateway; media from Muse and a picture from Gemini |

## Not verified yet

| Area | Not yet verified |
| --- | --- |
| Installer | `v0.1.0-dev.2` over `v0.1.0-dev.1` from Explorer (store from v31), clicked through; a clean Windows machine; any macOS build |
| Native window | Every screen clicked through; scaling, snap layouts above 100%, Narrator and NVDA, contrast themes (ADR-012) |
| Limits | Session and day limits in the installed app (headless gateway on a store copy only) |
| Real sessions | Launch receipts and request kinds; `pygg chat` and `pygg ask` on the owner's connections; Claude Code in an interactive terminal; the launcher answered by a real provider; `pygg run codex` and `pygg run opencode` on installed tools; `pygg setup` on a real tool's configuration |
| Skills | Installed into real agents and used, globally and in a project |
| Providers | Successful requests to OpenAI, Groq, DeepSeek, Mistral (errors only); Anthropic-compatible endpoints other than Anthropic; DeepSeek thinking with tools across the Anthropic translation (needs a funded account) |
| Caching | Effect of `x-session-id` on OpenRouter beyond its routing, and of OpenAI's `prompt_cache_key` (sent, not measured) |
| Errors and limits | Gemini's 429 `RESOURCE_EXHAUSTED` as a fixture (`RetryInfo` unit-tested only); the 429 hold and prefill fallback in a live `auto/cheap` run; the quota-group hold, HTTP-date resets, the attempt journal and at-risk accounting against a real provider |
| Routing extras | Key rings, cheapest-first combos and combo tests with real providers; estimated savings and the token mix on a real ledger |
| Web sessions | Media from sites other than Muse (apart from a Gemini picture); many days of use; the guard's rests and back-offs on a real site; a web pool of two accounts. The guard is not stress-tested on the owner's accounts |

## Designed, not built

- Quota tracking for subscriptions: their providers state nothing Pygg can read.
- Of the brain and the board (ADR-040 to ADR-042): the `image` role, and the screen parts in [design/BRAIN.md](design/BRAIN.md#410-designed-not-built), among them a removed workspace's read-only board.
- The Vietnamese translation of strings added since the first pass. Done last.
