# Implementation status

This page lists what exists in code today, what is designed but not built, and how each part was verified. It is updated with each change that affects a line. Last update: 2026-11-11, gateway first ([ADR-016](DECISIONS.md)).

## Built

### Gateway (`apps/desktop/src-tauri/src/gateway`)

| Part | What it does | Verified by |
| --- | --- | --- |
| Server (`mod.rs`) | Listens on loopback only (`127.0.0.1:8741` by default). Refuses requests whose `Host` is not a loopback name, which blocks DNS rebinding. Requires a client token (`Authorization: Bearer` or `x-api-key`). Answers `GET /v1/models`, `POST /v1/chat/completions`, `POST /v1/messages` and `/health`. | End-to-end tests against a fake provider; live run against Gemini |
| Routing and fallback | A requested model is either `<connection>/<model>` or a combo name. The next target is tried on 401, 402, 403, 404, 408, 409, 425, 429, 5xx or a connection error. This happens only before any byte reaches the client. A 400 is returned as it is. | Tests: three-step combo (429, then 503, then OK); bad request not retried; live: missing model, then Flash-Lite |
| Streaming | Server-sent events pass through unchanged. Usage is read from the final chunk (`stream_options.include_usage` is requested). If the client disconnects or the stream breaks, the outcome is recorded with an unknown cost. | Tests; live |
| Anthropic surface (`anthropic.rs`) | Translates Messages requests to Chat Completions and back, streamed or not: system prompt, text, images, tools, `tool_choice`, tool calls and tool results. Thinking blocks and cache markers are dropped. `count_tokens` returns an estimate. | Unit tests for the translation and the stream translator; live non-streamed and streamed requests with a tool call and its result |
| Gemini signatures (`signatures.rs`) | Gemini 3 needs `thought_signature` on earlier tool calls. Signatures are remembered by call id and put back on requests to Gemini. Calls made by another model get Google's placeholder. The field is stripped for every other provider. | Unit tests; live round trip that failed with HTTP 400 before this fix and passes now |
| Ledger and pricing (`pricing.rs`, `store.rs`) | Every upstream attempt is recorded, including failed ones. **Actual** means the provider reported the charge (OpenRouter's `usage.cost`). **Estimated** means it was computed from a price the user entered, or the billing type is Free or Local; rejected 4xx requests are recorded as $0, not billed. **Unknown** covers everything else and is never stored as $0. | Unit tests; live ledger checked in the app |
| Client tokens | Stored as SHA-256 hashes and shown once. An optional monthly limit applies to known spend; requests over it get HTTP 429. Tokens can be revoked. | Tests |
| Connections and models | Presets for Gemini, OpenRouter, OpenAI, Groq, DeepSeek and Ollama, plus custom endpoints. Keys go to the OS credential store. The model list comes from `GET {endpoint}/models`. Models can be added by hand. Prices, billing and enabled state are set per model. | Tests; live: 62 Gemini models listed |
| Headless CLI (`bin/gateway.rs`) | `serve`, `add-connection`, `remove-connection`, `refresh-models`, `create-client`, `combo`, `list`, `usage`. It uses the app's store unless `--db` is given. | Used for the live tests |

### Desktop app

| Area | State | Verified by |
| --- | --- | --- |
| Commands (`commands.rs`) | 24 Tauri commands cover the gateway (start, stop, port, autostart), connections, models, prices, combos, clients and usage. Keys go in and never come back out. | Driven end to end inside the running app through WebView2 DevTools (below) |
| Start-up | Opens the store in the app data folder, starts the gateway unless autostart is off, and fits the window to the monitor's work area. | Native app run: `/health` answered, window captured |
| Overview | Gateway card (state, address, copy, start/stop, port settings), first-run checklist, client setup snippets (Claude Code, OpenCode, OpenAI SDK, curl; PowerShell or POSIX), last 24 hours. | Browser preview (demo data and `?empty`); native screenshot |
| Connections | Provider list with status and an on/off switch. Connect Provider sheet. Detail with authentication, endpoint, health, billing, data use and model names. Models table with search, filter, price sheet, add and remove. | Browser preview (connect flow); native screenshot with real Gemini models |
| Combos | Ordered chains. Editor with ordering, add and remove. The fallback rule is spelled out. | Browser preview |
| Clients | Token table with spend against limit. Create sheet shows the token once and a command to keep it in an environment variable. Limit and revoke. | Browser preview |
| Usage | 24 hours, 7 days or 30 days. Actual, estimated and unknown totals. By model, by client, and a ledger with an expandable row for each request. | Browser preview; native screenshot with real requests |
| Smart Tasks · Preview | The task list, detail and New Task on labeled sample data, plus Policies as a "not built yet" page. | Browser preview |
| Look | DownFetch's tokens and green accent (light `#0B7A4B`, dark `#0D8551`, accent text `#3DDC97`), regenerated icons, light and dark. | Browser preview in both appearances |
| i18n | Every string is in English and Vietnamese. | String scan: no untranslated interface strings left (names and sample titles excepted) |

Browser preview (`pnpm dev`) runs on an in-memory demo that is labeled on screen (`src/lib/demo.ts`, ADR-014). Add `?empty` to the URL to see the first-run state.

### Core modules from the bootstrap

`db.rs` (migrations, now at schema v2), `vault.rs`, `redact.rs`, `process.rs`, `isolation.rs` and `checks.rs` are unchanged. `bench.rs` now reads agent cost events and enforces a hard `--budget-usd` cap.

`cargo test`: 49 passed, 1 ignored (the OS credential store test, which passes with `--ignored`).

## Live verification, 2026-11-11

All live tests used `gemini-3.1-flash-lite` with the owner's Gemini key. The total estimate is well under $0.01.

1. Headless gateway on a scratch store:
   - OpenAI-style requests, streamed and not.
   - A combo falling back from a missing model.
   - Anthropic Messages requests, streamed and not.
   - A tool call followed by its result.
   - The same with tool history from another model.
   - An unknown token (refused with 401).
2. The desktop app, debug build, driven through WebView2's DevTools protocol:
   - Connected Gemini and listed its 62 models.
   - Set a price, created a combo and created a client with a $1 limit.
   - Sent OpenAI and Anthropic requests through the app's own gateway.
   - Read the ledger: 2 fallbacks (HTTP 404, $0 estimated) and 2 answers (estimated from the price).
   - Removed everything and deleted the test store. No credential is left in Windows Credential Manager.

## Not verified yet

- **Claude Code against the gateway.** The Messages API was tested with direct requests, including tools and streaming. A full Claude Code session was not run, because running an agent here needs the owner's go-ahead.
- **OpenCode configuration snippet.** Its format follows OpenCode's documented custom-provider config, but it was not run.
- **Providers other than Gemini.** OpenRouter, OpenAI, Groq, DeepSeek and Ollama were tested only against the fake provider.
- **Clicking through the native window.** It was checked through screenshots and DevTools-driven commands. The D12 checks for traffic-light controls (snap layouts, scaling, screen readers) remain open.

## Designed, not built

- Smart task routing: Plan/Run/Result, workspaces, approvals and Policies.
- The Setup Assistant.
- Quota tracking for subscriptions.
- Data-use terms per provider.
- Tray and notifications.

Each is specified in [design/SCREENS.md](design/SCREENS.md).
