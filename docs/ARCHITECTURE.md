# Architecture

> **Status:** Current · how the parts fit together · reviewed 2026-11-13 · order: [ROADMAP](ROADMAP.md) · built: [Implementation Status](IMPLEMENTATION-STATUS.md)

Pygg is one desktop program: a Rust core inside a Tauri 2 window, with the same core also built as headless command-line programs. Everything runs on the user's machine (ADR-002). This page says where each responsibility lives; what is built and how it was verified is in [Implementation Status](IMPLEMENTATION-STATUS.md), and why it is so in [DECISIONS](DECISIONS.md).

## The pieces

```text
                 IDE agent (Claude Code, Codex, …)          Clients the user already has
                         │ MCP over stdio                    (Claude Code, OpenCode, SDKs, scripts)
                         ▼                                              │ HTTP, client token
               <app> mcp / pygg-mcp ──── task API ──┐                   ▼
                                       /pygg/v1/*   │      ┌───────────────────────────────┐
  Tauri window (React) ── commands ──► Rust core ◄──┴──────┤ Gateway  127.0.0.1:8741      │
                                       │                   │ /v1/chat/completions          │
                                       │                   │ /v1/messages, /v1/responses   │
                                       │                   │ /v1/models                    │
                                       │                   └──────────────┬────────────────┘
                        ┌──────────────┼───────────────┐                  │ provider APIs (keys from
                        ▼              ▼               ▼                  ▼ the OS credential store)
                   Task engine    SQLite store    OS credential     OpenAI-compatible and
                   (tasks.rs)     (db.rs, v26)    store (vault.rs)  Anthropic endpoints, local runtimes,
                                                                    and web sessions (websession/: a
                                                                    loopback endpoint → a Pygg window)
                        │
                        ▼  one Git worktree per attempt (isolation.rs)
                   Agent CLIs with their own sign-in (agents.rs): Claude Code, Codex CLI, Gemini CLI, OpenCode
                        │
                        ▼
                   Approved checks (checks.rs) decide pass or fail
```

- **One crate** (`apps/desktop/src-tauri`, ADR-005). The app is the default binary; `pygg-cli`, `gateway`, `agent`, `bench` and `pygg-mcp` are the same library run headless. `pygg-cli` (`cli/`) is the one console program: `status`, `doctor`, `mcp`, `gateway …`, `agent …`, `setup` (`cli/setup.rs`), `run claude` (`cli/run.rs`), `diagnostics` and `--version`. The npm package `@pygg/cli` (`packages/npm`) is only a Node launcher that runs it as `pygg`. The interface is React 19 and Vite in `apps/desktop/src`, talking to the core only through Tauri commands (`commands.rs`, `lib/native.ts`).
- **One store**, `store.sqlite3` in the app data folder (`%APPDATA%\app.smarttaskrouter.desktop` on Windows, or `PYGG_DATA_DIR`). Migrations run in one transaction when it opens. Schema: [SCHEMA.md](SCHEMA.md), generated from the migrations.
- **One gateway per store**: `gateway::start` takes an OS file lock next to the store, held until every attempt it sent is settled.

## A request through the gateway (`gateway/`)

1. **Authenticate** the client token (`Authorization: Bearer` or `x-api-key`); only loopback `Host` names are served.
2. **Resolve the model name** (`store::resolve`, then `forward()` in `gateway/mod.rs`): `<connection>/<model>`; `<pool>/<model>`, the pool's connections in the user's order (ADR-034), or for a pool of web sessions the account that sent fewest requests today first; a combo; or `auto` / `auto/cheap`, which build a route from the connected models with hard filters first, then rules (`auto.rs`, ADR-006). **A combo has a strategy** (schema v25, `combos.strategy`): `order` keeps its members as listed; `cheapest` has `forward()` re-order them for each request by that request's estimated cost (`cheapest_first`: free, local and subscription members at $0, an unknown price last), with the reason in the route header. The fallback rules are the same for both. An explicit `x-smarttaskrouter-session` pins a session to the target that answered it.
3. **For each target in turn**: the client's connection list and data-use requirement, the API format, for a web session its own conditions (`websession::routable`: warning acknowledged, no tools, the app running), health and cooldowns, quota-group holds, then **admission**: a client with a limit holds the estimated (or, if strict, the largest possible) cost before anything is sent. The attempt is **journaled** before it is sent.
4. **Send**: the same format passes through as a native-format passthrough (key order and cache markers kept); Anthropic to OpenAI-compatible is translated (`anthropic.rs`), OpenAI to Anthropic (`openai_to_anthropic.rs`), and the Responses API to Chat Completions or, through it, to Anthropic (`responses.rs`, ADR-033); what is dropped is named in `x-smarttaskrouter-dropped`. A web session is reached as an OpenAI-format provider at the app's loopback endpoint (below).
5. **On failure**, `policy.rs` decides by error class: try another model, another connection, or stop (a 5xx or an ambiguous timeout stops unless the client allows retries), and what to mark: a key rejected, a model cooling down, a quota group held. Fallback happens only before any byte reached the client.
6. **Settle**: the ledger entry (tokens, cost with its source: actual, estimated or unknown, never a guessed $0) and the journal removal are one transaction. While a settlement cannot be written, no paid request is admitted.

A background **probe** (`probe.rs`) reads the free model list of an unreachable connection once a minute and lifts the outage when it answers.

**Estimated savings** (`gateway/savings.rs`, Home and Usage) read the ledger afterwards: the answered requests are priced again at one model the user chose (`savings.baseline`), with the same tokens, and compared with what routing cost, failed attempts with a known cost included. Requests with an unknown cost or no token counts, and subscription requests, are counted apart, never taken as $0.

## A web session (`websession/`, ADR-035)

A site with no API, used through a window the user signs in to, served to the gateway as an ordinary OpenAI-format provider. The app only: `gateway serve` has no window, so these connections are skipped there with the reason.

- **The service** (`websession/mod.rs`, `Service`, started in `lib.rs`): a Chat Completions endpoint on a random loopback port (`/web/<connection>/v1/chat/completions`), behind a token that lives only in memory. Per request it refuses tools, checks the connection's daily limit (`store::web_requests_today`, schema v26), waits its turn (one request at a time per connection, 10 s after the last reply), flattens the conversation into one prompt, opens a new chat, types, presses Send and streams the reply as it appears. Errors carry Pygg's own wording and the status the gateway's policy reads (401 needs the user, 429 busy, quota or daily limit, 503 page changed, 504 timeout).
- **The window** (`websession/window.rs`, `AppWindows`): one Tauri webview per connection (label `web-<connection>`) with its own WebView2 profile folder (`websessions/p-<random>`), hidden while Pygg drives it and closed after 10 minutes unused. It has no Tauri capability. While Pygg drives it, it keeps to the recipe's hosts; while the user has it open to sign in, it may go to any https address. A Check Sign-In that finds the session signed in hides it.
- **The recipes** (`RECIPES` in `websession/mod.rs`, one per preset in `gateway/presets.rs`): the site, its new-chat address, its hosts, the selectors for the prompt box, Send, a reply, "still answering", a sign-in page, a challenge and an approval request. The page scripts built from them (`state_script`, `fill_script`, `send_script`, `read_script`, `deny_script`) read and fill the page only; `fill_script` and `send_script` act only on the site's own host (`site_host`).
- **The store** (`gateway/store.rs`): `add_web_provider`, `set_web_state`, `set_web_blocked`, `set_web_daily_limit`, `web_requests_today`; web-session pools in `set_pool` and `resolve`.

## A task (`tasks.rs`, `agents.rs`, `isolation.rs`, `checks.rs`)

1. A task is created from the app or from an IDE agent through MCP and the task API (ADR-027). Its **ladder of steps** (agent, model, tier, billing) and its workspace's **approved checks** are fixed then, with the base commit.
2. A **paid step waits for approval** with an estimate and a cap (ADR-010); a sensitive workspace asks before every step. There is no API to approve.
3. Each attempt runs the **agent's own CLI with its own sign-in** (ADR-026) in a fresh worktree of the base commit, with `PYGG_ATTEMPT_ID` set. Pygg never reads that sign-in.
4. The approved checks run in the worktree. A failure discards it and hands the failed command and the end of its output to the next step. A pass leaves the result for **Keep as Branch**, **Export Patch** or **Discard**.
5. The agent's reported tokens go to the ledger with an unknown cost (a subscription is not billed per request); its own list-price estimate is kept beside it.

## Setting a tool up (`integrations.rs`, `launcher.rs`, `cli/setup.rs`, `cli/run.rs`)

- **One-click set-up** (`integrations.rs`): the MCP server in Claude Code, Codex CLI or OpenCode (Agents), and Codex's or OpenCode's models through the gateway (CLI Tools). Each backs up the tool's configuration, makes a Pygg API key of its own, writes the change and checks it, putting the file back if a step fails; Remove takes Pygg's change out (after another backup) and revokes the key. `pygg setup [<tool>] [--dry-run | --remove]` (`cli/setup.rs`) runs the same code from a terminal.
- **The Claude Code launcher** (`launcher.rs`): set up in CLI Tools or with `pygg setup claude-launcher`, it keeps a Pygg API key in the OS credential store. `pygg run claude` (`cli/run.rs`) reads it, checks the gateway answers, and starts Claude Code with the gateway's address, that key and `auto` in that one process's environment, without `ANTHROPIC_API_KEY` or the host session's variables. Claude Code's own settings are never changed.

## Boundaries

- **The gateway never changes an external client's conversation**: no injected memory, no rewritten history, no model switch after output started (ADR-003, ADR-025).
- **Agents reach the gateway only when the user sets them up.** A task's agent runs with its own sign-in. Pygg points a tool at the gateway only on the user's explicit set-up (Codex's or OpenCode's models, the Claude Code launcher, or the user's own configuration, as the benchmark does with OpenCode); its requests are then metered there and can carry the attempt id as their session.
- **No chat in the app** (ADR-028): the app monitors, approves and reviews.

## Where to change what

| Change | Start in |
| --- | --- |
| A provider preset | `gateway/presets.rs`, and its source in [PROVIDERS.md](PROVIDERS.md) |
| How an error is handled | `gateway/policy.rs`, with a fixture in `tests/fixtures/provider-errors` |
| What `auto` may choose | `gateway/auto.rs` (filters, ranking), `gateway/tiers.rs` (starting tiers) |
| A combo strategy | `gateway/mod.rs` (`forward`, `cheapest_first`), `combos.strategy` in a new migration |
| A web-session site | `websession/mod.rs` (`RECIPES`), its preset in `gateway/presets.rs`, its hosts in [PROVIDERS.md](PROVIDERS.md#web-session-unofficial) |
| How a web session's window behaves | `websession/window.rs` |
| The savings estimate | `gateway/savings.rs` |
| A tool's one-click set-up | `integrations.rs`, and its name in `cli/setup.rs` |
| The Claude Code launcher | `launcher.rs`, `cli/run.rs` |
| A new agent CLI | `agents.rs` (`AgentSpec`, arguments, usage parsing) |
| The task ladder | `tasks.rs` |
| The schema | a new numbered file in `migrations/`, then `python scripts/schema-reference.py` |
