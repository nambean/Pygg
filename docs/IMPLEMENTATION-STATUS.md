# Implementation status

This page lists what exists in code today, what is designed but not built, and how each part was verified. It is updated with each change that affects a line. Last update: 2026-11-11, cost-safe gateway parity ([ROADMAP](ROADMAP.md) P0-1 to P0-4, [ADR-025](DECISIONS.md)).

## Built

### Gateway (`apps/desktop/src-tauri/src/gateway`)

| Part | What it does | Verified by |
| --- | --- | --- |
| Server (`mod.rs`) | Listens on loopback only (`127.0.0.1:8741` by default). Refuses requests whose `Host` is not a loopback name, which blocks DNS rebinding. Requires a client token (`Authorization: Bearer` or `x-api-key`). Answers `GET /v1/models`, `POST /v1/chat/completions`, `POST /v1/messages` and `/health`. | End-to-end tests against a fake provider; live run against Gemini |
| Routing | A requested model is either `<connection>/<model>` or a combo name, tried in order. | Tests; live |
| **Same-format passthrough** (ROADMAP P0-1) | Every connection has a wire format (`openai` or `anthropic`). Requests in the same format are passed through unchanged; for Anthropic that includes `cache_control`, thinking and `anthropic-beta`. The answer comes back byte for byte, and Anthropic usage (cache read and write) is read on the way. Anthropic → OpenAI translation keeps cache markers for OpenRouter and reports dropped hints in `x-smarttaskrouter-dropped`. OpenAI → Anthropic is refused, because that translation is not built yet. New presets: Anthropic, Custom (Anthropic-compatible). | Tests against a fake Anthropic endpoint that echoes the fields it received; live: Anthropic request to Gemini translated with `dropped: cache_control` |
| **Error-scoped fallback and cooldown** (P0-2, `policy.rs`) | Fallback happens only before any byte reaches the client. 401 blocks the connection until its key is replaced. 402, 403 and 429 cool the model or connection down, honouring `Retry-After`, and never move to *another connection in the same quota group*; the same connection's other models may still answer. 5xx and refused connections may fall back. A read timeout after sending stops with an Unknown cost and does not pay a second provider. Cooldowns are stored per connection and model, and are shown and clearable in Connections. The quota group defaults to the provider kind; a connection can be marked as its own organization. | Tests: 429 → 503 → OK with later requests skipping the cooled-down models; sibling key not tried; 401 blocks until cleared; timeout not retried; live: missing model cooled down and skipped on the next request |
| **Budgets, concurrency, complete ledger** (P0-3) | Spend is reserved atomically (`BEGIN IMMEDIATE`) from a pessimistic estimate (prompt size plus `max_tokens` at the model's price) and settled from real usage. Reservations left by a crash are released at start. Requests in flight are capped per client (8 by default). A target with an unknown price is blocked for a client with a limit unless the client allows it. Handlers run detached, so a client that disconnects early still leaves a ledger entry. | Tests: two concurrent requests against a limit that fits one (200 and 429); unknown price blocked, then allowed; in-flight cap; disconnect before the answer is still recorded |
| **Explicit session affinity** (P0-4) | `x-smarttaskrouter-session` pins a session to the target that last answered it (2 h sliding) while that target is still on the route; others remain as fallbacks. Sessions are never guessed from prompt content. The Claude Code snippet sets a new session id per terminal through `ANTHROPIC_CUSTOM_HEADERS`. | Test: a session stays on the backup after one fallback while requests without a session return to the first model; live: `x-smarttaskrouter-session: pinned`, 1 call |
| Streaming | Server-sent events pass through unchanged. Usage is read from the final chunk (`stream_options.include_usage` is requested). If the client disconnects or the stream breaks, the outcome is recorded with an unknown cost. | Tests; live |
| Anthropic surface (`anthropic.rs`) | Passes through to Anthropic connections (above). To OpenAI-compatible ones it translates, streamed or not: system prompt, text, images, tools, `tool_choice`, tool calls and tool results. Thinking does not cross. `count_tokens` is exact on Anthropic connections and labeled as an estimate elsewhere. | Unit tests for the translation and the stream translator; live non-streamed and streamed requests with a tool call and its result |
| Gemini signatures (`signatures.rs`) | Gemini 3 needs `thought_signature` on earlier tool calls. Signatures are remembered by call id and put back on requests to Gemini. Calls made by another model get Google's placeholder. The field is stripped for every other provider. | Unit tests; live round trip that failed with HTTP 400 before this fix and passes now |
| Ledger and pricing (`pricing.rs`, `store.rs`) | Every upstream attempt is recorded, including failed ones. **Actual** means the provider reported the charge (OpenRouter's `usage.cost`). **Estimated** means it was computed from a price the user entered, or the billing type is Free or Local; rejected 4xx requests are recorded as $0, not billed. **Unknown** covers everything else and is never stored as $0. | Unit tests; live ledger checked in the app |
| Client tokens | Stored as SHA-256 hashes and shown once. Each has an optional monthly limit (with reservations, above), a cap on requests in flight and an unknown-price policy. Tokens can be revoked. | Tests |
| Connections and models | Presets for Gemini, OpenRouter, OpenAI, Groq, DeepSeek and Ollama, plus custom endpoints. Keys go to the OS credential store. The model list comes from `GET {endpoint}/models`. Models can be added by hand. Prices, billing and enabled state are set per model. | Tests; live: 62 Gemini models listed |
| Headless CLI (`bin/gateway.rs`) | `serve`, `add-connection`, `remove-connection`, `refresh-models`, `create-client`, `combo`, `list`, `usage`. It uses the app's store unless `--db` is given. | Used for the live tests |

### Desktop app

| Area | State | Verified by |
| --- | --- | --- |
| Commands (`commands.rs`) | 26 Tauri commands cover the gateway (start, stop, port, autostart), connections, models, prices, combos, clients and usage. Keys go in and never come back out. | Driven end to end inside the running app through WebView2 DevTools (below) |
| Start-up | Opens the store in the app data folder, starts the gateway unless autostart is off, and fits the window to the monitor's work area. | Native app run: `/health` answered, window captured |
| Overview | Gateway card (state, address, copy, start/stop, port settings), first-run checklist, client setup snippets (Claude Code, OpenCode, OpenAI SDK, curl; PowerShell or POSIX), last 24 hours. | Browser preview (demo data and `?empty`); native screenshot |
| Connections | Provider list with status (key rejected, cooling down) and an on/off switch. Connect Provider sheet. Detail with authentication, endpoint, API format, health with cooldowns and **Try Again Now**, **Own organization** switch for the quota group, billing, data use and model names. Models table with search, filter, per-model cooldown badges, price sheet (input, output, cached input, cache write), add and remove. | Browser preview (connect flow); native screenshot with real Gemini models |
| Combos | Ordered chains. Editor with ordering, add and remove. The fallback rule is spelled out. | Browser preview |
| Clients | Token table with spend against limit, spend held for requests in flight, and each client's policy. Create sheet shows the token once and a command to keep it in an environment variable. **Limits…** sets the monthly limit, the unknown-price policy and the in-flight cap. Revoke. | Browser preview |
| Usage | 24 hours, 7 days or 30 days. Actual, estimated and unknown totals. By model, by client, and a ledger with an expandable row for each request. | Browser preview; native screenshot with real requests |
| Smart Tasks · Preview | The task list, detail and New Task on labeled sample data, plus Policies as a "not built yet" page. | Browser preview |
| Look | DownFetch's tokens and green accent (light `#0B7A4B`, dark `#0D8551`, accent text `#3DDC97`), regenerated icons, light and dark. | Browser preview in both appearances |
| i18n | Every string is in English and Vietnamese. | String scan: no untranslated interface strings left (names and sample titles excepted) |

Browser preview (`pnpm dev`) runs on an in-memory demo that is labeled on screen (`src/lib/demo.ts`, ADR-014). Add `?empty` to the URL to see the first-run state.

### Core modules from the bootstrap

`db.rs` (migrations, now at schema v2), `vault.rs`, `redact.rs`, `process.rs`, `isolation.rs` and `checks.rs` are unchanged. `bench.rs` now reads agent cost events and enforces a hard `--budget-usd` cap.

`cargo test`: 67 passed, 1 ignored (the OS credential store test, which passes with `--ignored`).

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

## Live verification of P0, 2026-11-11 (night)

With the headless gateway on a scratch store and the owner's Gemini key (gemini-3.1-flash-lite, a few cents at most):

- A combo whose first model does not exist: HTTP 404, the model cooled down, and the answer came from the second model.
- The same request again: the missing model was skipped without a call; the explicit session was pinned (`x-smarttaskrouter-session: pinned`, one call).
- An Anthropic request to Gemini was translated, and the response said `x-smarttaskrouter-dropped: cache_control`.
- Afterwards the connection was removed with its key, and the scratch store deleted.

## Not verified yet

- **Native Anthropic passthrough against a real Anthropic or Anthropic-compatible endpoint.** It is tested only against a fake endpoint. No Anthropic key is available here. The cache-parity gate in the ROADMAP (direct versus through the gateway, cache read and write) is still open.
- **OpenRouter's acceptance of `cache_control` on translated content parts.** It follows OpenRouter's documentation but has not been measured.

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
