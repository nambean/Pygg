# Implementation status

This page lists what exists in code today, what is designed but not built, and how each part was verified. It is updated with each change that affects a line. Last update: 2026-11-11, answers to the [follow-up review](reviews/2026-11-11-remaining-gateway-risks-response.md) (client connection allowlist, no paid retry by default, no paid admission while the ledger is degraded) and to the [architect counter-review](reviews/2026-11-11-counter-review-response.md), on top of cost-safe gateway parity ([ROADMAP](ROADMAP.md) P0-1 to P0-4, [ADR-025](DECISIONS.md)).

## Verification level of P0

The levels are cumulative. **Fake-tested** means end-to-end tests against the fake provider in `gateway/tests.rs`. **Live-tested** means a real provider answered through the gateway. **Provider-parity-tested** means the same request went direct and through the gateway, and cost and fields were compared. **Release-ready** means all of that, plus the ROADMAP's benchmark and security gates.

| Item | Built | Fake-tested | Live-tested | Provider-parity-tested | Release-ready |
| --- | --- | --- | --- | --- | --- |
| P0-1 same-format passthrough | Yes | Yes | Native Anthropic (claude-haiku-4-5); translation (Gemini) | **Yes for native Anthropic**: cache write and read, streamed and not; thinking with a tool call; citations. Translation: not applicable | No |
| P0-2 error-scoped fallback | Yes | Yes | 404 cooldown and skip (Gemini) | Not applicable | No |
| P0-3 budgets and complete ledger | Yes | Yes | Ledger and prices (Gemini); reservations not live | Not applicable | No |
| P0-4 explicit sessions | Yes | Yes | Pinning (Gemini, direct requests); Claude Code headless session carried the header on 12 of 12 requests | Not applicable | No |
| P0-5 Auto routing | No | No | No | No | No |

## Built

### Gateway (`apps/desktop/src-tauri/src/gateway`)

| Part | What it does | Verified by |
| --- | --- | --- |
| Server (`mod.rs`) | Listens on loopback only (`127.0.0.1:8741` by default). Refuses requests whose `Host` is not a loopback name, which blocks DNS rebinding. Requires a client token (`Authorization: Bearer` or `x-api-key`). Answers `GET /v1/models`, `POST /v1/chat/completions`, `POST /v1/messages` and `/health`. | End-to-end tests against a fake provider; live run against Gemini |
| Routing | A requested model is either `<connection>/<model>` or a combo name, tried in order. | Tests; live |
| **Same-format passthrough** (ROADMAP P0-1) | Every connection has a wire format (`openai` or `anthropic`). Requests in the same format go through as a **native-format semantic passthrough**: the JSON is written again with its keys in the client's order and only `model` replaced; for Anthropic that keeps `cache_control`, thinking, citations and every other field, and forwards `anthropic-version` and `anthropic-beta`. The answer body comes back unchanged with the provider's rate-limit and request-id headers, and Anthropic usage (cache read and write) is read on the way. Anthropic → OpenAI translation keeps cache markers for OpenRouter and names everything it drops in `x-smarttaskrouter-dropped`. OpenAI → Anthropic is refused, because that translation is not built yet. Field matrix: [counter-review response](reviews/2026-11-11-counter-review-response.md#native-passthrough-fields-and-headers-cr-08). | Tests against a fake Anthropic endpoint that echoes the fields it received; key order and limit headers; dropped-field list; live: Anthropic request to Gemini translated with `dropped: cache_control` |
| **Error-scoped fallback and cooldown** (P0-2, `policy.rs`) | Fallback happens only before any byte reaches the client. 401 blocks the connection until its key is replaced. 402, 403 and 429 cool the model or connection down and never move to *another connection in the same quota group*, in that request **or later ones while the cooldown lasts**; the same connection's other models may still answer. OpenAI's `insufficient_quota` cools the whole connection. The reset is read from `Retry-After` (seconds or HTTP date, measured against the response's `Date`), `retry-after-ms` or Gemini's `RetryInfo`, and kept as stated (up to 7 days). A read timeout after sending, and HTTP 408, 504 and 524, stop with an Unknown cost and do not pay a second provider. A refused connection may fall back. Other 5xx stop too unless the client allows paid retries (1 to 5) or the connection is free or local. A client token can be restricted to named connections: a restricted client never reaches another one, by combo, fallback, session pin, direct request or `count_tokens`. Cooldowns are stored per connection and model, and are shown and clearable in Connections. The quota group defaults to the provider kind; a connection can be confirmed as having an independent quota. Full matrix: [counter-review response](reviews/2026-11-11-counter-review-response.md#fallback-matrix-cr-01-cr-05). | Tests: 429 → 503 → OK with later requests skipping the cooled-down models; sibling key not tried, now or in later requests; account out of credit; 24 h reset kept; 408 and 504 not retried; 401 blocks until cleared; timeout not retried; every Retry-After form; live: missing model cooled down and skipped on the next request |
| **Budgets, concurrency, complete ledger** (P0-3) | **Every attempt is journaled before it is sent**; if the journal cannot be written, nothing is sent (HTTP 503). The ledger entry and the journal removal are one transaction. An attempt never settled (a panic, a crash) becomes an *interrupted* entry with an unknown cost. A client with a limit holds an **estimate** of each attempt's cost atomically (`BEGIN IMMEDIATE`): the prompt with non-ASCII characters counted as a token each, at the higher of the input and cache-write prices, plus `max_tokens` (or 4096) at the output price. It is settled from real usage. The limit is an estimated admission limit, not a hard cap: the bill can exceed it, and the UI says so. Spend **at risk** (an unknown cost after the request reached the provider) counts against the limit at its held amount. Requests in flight are capped per client (8 by default). A target with an unknown price is blocked for a client with a limit unless the client allows it. Handlers run detached, so a client that disconnects early still leaves a ledger entry. A settlement that cannot be written is kept with its exact figures and retried before every request; **while one waits, no paid target is sent**. Settlement is exactly once. It shows in `/health` and on Overview. | Tests: two concurrent requests against a limit that fits one (200 and 429); spend above the estimate recorded and the next request refused; server error counted at risk; estimate for CJK, Vietnamese and cache writes; no journal entry left after five attempts; nothing sent with a read-only store; interrupted entries; unknown price blocked, then allowed; in-flight cap; disconnect before the answer is still recorded |
| **Explicit session affinity** (P0-4) | `x-smarttaskrouter-session` (at most 128 visible ASCII characters, anything else ignored) pins a session to the target that last answered it (2 h sliding) while that target is still on the route; others remain as fallbacks. Pins are scoped to the client token, and expired pins are deleted. Sessions are never guessed from prompt content. The Claude Code snippet sets a new session id per terminal through `ANTHROPIC_CUSTOM_HEADERS`. | Tests: a session stays on the backup after one fallback while requests without a session return to the first model; same id from another client not shared; long and non-ASCII ids ignored; expired pins deleted; live: `x-smarttaskrouter-session: pinned`, 1 call |
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
| Connections | Provider list with status (key rejected, cooling down) and an on/off switch. Connect Provider sheet. Detail with authentication, endpoint, API format, health with cooldowns and **Try Again Now**, **Independent quota confirmed** switch for the quota group (with a confirmation that says what it means, and the date it was given), billing, data use and model names. Models table with search, filter, per-model cooldown badges, price sheet (input, output, cached input, cache write), add and remove. | Browser preview (connect flow); native screenshot with real Gemini models |
| Combos | Ordered chains. Editor with ordering, add and remove. The fallback rule is spelled out. | Browser preview |
| Clients | Token table with spend against limit, spend possibly charged (at risk), an overrun, spend held for requests in flight, and each client's policy. The limit is described as checked against an estimate. Create sheet shows the token once and a command to keep it in an environment variable. **Limits…** sets the monthly limit, the unknown-price policy, the in-flight cap, retries after a server error (off by default) and the connections the client may use. Revoke. | Browser preview |
| Usage | 24 hours, 7 days or 30 days. Actual, estimated and unknown totals. By model, by client, and a ledger with an expandable row for each request. | Browser preview; native screenshot with real requests |
| Smart Tasks · Preview | The task list, detail and New Task on labeled sample data, plus Policies as a "not built yet" page. | Browser preview |
| Look | DownFetch's tokens and green accent (light `#0B7A4B`, dark `#0D8551`, accent text `#3DDC97`), regenerated icons, light and dark. | Browser preview in both appearances |
| i18n | Every string is in English and Vietnamese. | String scan: no untranslated interface strings left (names and sample titles excepted) |

Browser preview (`pnpm dev`) runs on an in-memory demo that is labeled on screen (`src/lib/demo.ts`, ADR-014). Add `?empty` to the URL to see the first-run state.

### Core modules from the bootstrap

`db.rs` (migrations, now at schema v4), `vault.rs`, `redact.rs`, `process.rs`, `isolation.rs` and `checks.rs` are unchanged. `bench.rs` now reads agent cost events and enforces a hard `--budget-usd` cap.

`cargo test`: 87 passed, 1 ignored (the OS credential store test, which passes with `--ignored`).

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

## Provider parity: Anthropic direct versus through the gateway, 2026-11-11

With the owner's key and go-ahead, `scripts/anthropic-parity.ps1` sent the same requests to `api.anthropic.com` directly and through the headless gateway (connection preset Anthropic, native passthrough), model `claude-haiku-4-5`. Each arm used its own nonce in the system prompt, so neither could read the other's cache.

| Case | Direct | Gateway |
| --- | --- | --- |
| Turn 1, not streamed: input / cache write / cache read | 9 / 9011 / 0 | 9 / 9011 / 0 |
| Turn 2, not streamed | 9 / 0 / 9011 | 9 / 0 / 9011 |
| Turn 1, streamed | 9 / 9011 / 0 | 9 / 9011 / 0 |
| Turn 2, streamed | 9 / 0 / 9011 | 9 / 0 / 9011 |
| Thinking (1024 budget) with a tool | `thinking,tool_use`, signed, `stop_reason: tool_use` | the same |
| Document with citations enabled | text with citations, `end_turn` | the same |

The token counts are identical, so at the same price the billed amount is identical: on this endpoint the gateway adds no cache cost. The ledger recorded each gateway request with the whole prompt (9020 tokens, cache included) and route `native`. About $0.06 in all. The key was put in the credential store for the run and removed afterwards.

Not covered: the 1-hour cache TTL, a multi-turn tool-result round trip, other Claude models, and an Anthropic-compatible endpoint other than Anthropic's own.

## Live verification: Claude Code through the gateway, 2026-11-11

With the owner's go-ahead, Claude Code 2.1.317 ran a headless session (`claude -p`, four invocations of one session) against the headless gateway on a scratch store, routed to `gemini/gemini-3.1-flash-lite` through the Anthropic → OpenAI translation:

- `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN` (a client token), `ANTHROPIC_MODEL` and the default Haiku, Sonnet, Opus and sub-agent models all set to that route, `ANTHROPIC_CUSTOM_HEADERS="x-smarttaskrouter-session: cc-live-1"`. Its own config folder, so none of the owner's settings were used. Tools limited to reading and editing files and sub-agents in a scratch folder; no shell, no web.
- Turns: create a file and read it back (write and read tools); a sub-agent that lists the folder; `/compact`; an edit and a read.
- All four turns completed. The gateway's request log (`SMARTTASKROUTER_LOG_REQUESTS=1`) shows **12 of 12 requests to `/v1/messages` carrying the session header**, including the sub-agent's and the compaction's. No `count_tokens` call was made. The ledger has 12 entries, all `translated:anthropic->openai` with `session=cc-live-1`. Prompt input totaled about 117k tokens and output about 1.2k: a few cents at Flash-Lite prices.
- Afterwards the connection was removed with its key, and the scratch store, config and token deleted.

## Not verified yet

- **Anthropic-compatible endpoints other than Anthropic's own**, and the parity cases listed as not covered above.
- **OpenRouter's acceptance of `cache_control` on translated content parts.** It follows OpenRouter's documentation but has not been measured.

- **Claude Code in an interactive terminal.** A headless session was run (below); an interactive one may send background requests that `-p` does not, and was not run.
- **The counter-review changes against a real provider.** The quota-group hold, HTTP-date resets, the attempt journal and the at-risk accounting were tested against the fake provider only.
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
