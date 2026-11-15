# Architecture

> **Status:** Current · how the parts fit together · reviewed 2026-11-14 · built: [Implementation Status](IMPLEMENTATION-STATUS.md) · why: [DECISIONS](DECISIONS.md)

Pygg is one program: a Rust core inside a Tauri 2 window, with the same core also built as headless console programs. Everything runs on the user's computer (ADR-002). This page says where each responsibility lives.

## The pieces

```text
                 IDE agent (Claude Code, Codex, …)          Clients the user already has
                         │ MCP over stdio                    (Claude Code, Codex, OpenCode, SDKs, scripts)
                         ▼                                              │ HTTP, Pygg API key
               <app> mcp / pygg mcp ─── task API ──┐                    ▼
                                       /pygg/v1/*   │      ┌───────────────────────────────┐
  Tauri window (React) ── commands ──► Rust core ◄──┴──────┤ Gateway  127.0.0.1:8741      │
  pygg (npm) → pygg-cli ─────────────►    │                │ /v1/chat/completions          │
                                          │                │ /v1/messages, /v1/responses   │
                                          │                │ /v1/models, /health           │
                                          │                └──────────────┬────────────────┘
                        ┌─────────────────┼──────────────┐                │ provider APIs (keys from
                        ▼                 ▼              ▼                ▼ the OS credential store)
                   Task engine       SQLite store    OS credential   OpenAI-compatible and Anthropic
                   (tasks.rs,        (db.rs, v33)    store           endpoints, local runtimes, and
                    on hold)                         (vault.rs)      web sessions (websession/: a
                        │                                            loopback endpoint → a Pygg window)
                        ▼  one Git worktree per attempt (isolation.rs)
                   Agent CLIs with their own sign-in (agents.rs): Claude Code, Codex CLI, Gemini CLI, OpenCode
                        │
                        ▼
                   Approved checks (checks.rs) decide pass or fail
```

- **One crate** (`apps/desktop/src-tauri`, ADR-005). The app is the default binary. `pygg-cli`, `gateway`, `agent`, `bench` and `pygg-mcp` (`src/bin/`) run the same library without a window. The window is the Cargo feature `app`, so `pygg-cli` also builds with `--no-default-features` for macOS and Linux.
- **The interface** is React 19 and Vite in `apps/desktop/src`. It talks to the core only through Tauri commands (`commands.rs`, `src/lib/native.ts`).
- **Skills & Prompts** (`skills.rs`, ADR-039) reads the agents' global instruction files and skill, command and prompt folders, and writes one `SKILL.md` per install into `~/.claude/skills` or `~/.agents/skills`. It never downloads or runs anything.
- **One store**: `store.sqlite3` in the app data folder (`%APPDATA%\app.smarttaskrouter.desktop` on Windows, `~/Library/Application Support/…` on macOS, `$XDG_DATA_HOME` or `~/.local/share/…` on Linux, or `PYGG_DATA_DIR`). Migrations in `migrations/` run in one transaction when it opens; the schema version is the number of migrations (v33). Reference: [SCHEMA.md](SCHEMA.md).
- **Secrets** live in the OS credential store (`vault.rs`: Windows Credential Manager, macOS Keychain, Linux Secret Service). The store keeps only a reference. Client tokens are stored hashed.
- **Backups and logs**: a copy of the store goes to `backups/` at start-up and about once a day, newest three kept (`backup.rs`). The app's log is `logs/pygg.log`, redacted, rotated at 1 MB (`applog.rs`). A store that cannot be opened at start-up is explained, with an offer to restore the latest copy (`startup.rs`).
- **One gateway per store**: `gateway::start` takes an OS lock on `<store>.gateway-lock` and holds it until every attempt it sent is settled.

## A request through the gateway (`gateway/`)

1. **Authenticate.** The gateway listens on 127.0.0.1 only and refuses a `Host` that is not a loopback name. Every request needs a Pygg API key (`Authorization: Bearer` or `x-api-key`). A client also has a cap on requests in flight.
2. **Resolve the model name** (`store::resolve`, then `forward()` in `gateway/mod.rs`):
   - `<connection>/<model>`: one target.
   - `<pool>/<model>`: the pool's connections in the user's order (ADR-034). A pool of web sessions puts an account the guard is resting last, then the one that sent fewest requests today.
   - a **combo**: its members. Strategy `order` keeps them as listed; `cheapest` re-orders them per request by estimated cost (`cheapest_first`: free, local and subscription at $0, unknown price last).
   - `auto` or `auto/cheap`: a route built from the connected models, hard filters first, then rules (`auto.rs`, `tiers.rs`, ADR-006). Each model left out has a reason; with none left the answer is 422 with the reasons.
   - A request with `x-pygg-session` goes first to the target that last answered that session, while it is still in the route. A session a fallback took away from its home (the target it was meant for) goes back home when the request starts, once the home's cache is still warm or the fallback's has gone cold too (five minutes, `store::Pin::goes_home`), and the home is not cooling down; the fallback stays right behind it. Sessions are never guessed from the prompt.
3. **Expand key rings** (`keys.rs`, ADR-038). A connection with several keys becomes one target per key, ordered by its strategy (`in_order`, `round_robin`, `least_used`, `quota_aware`), the key that answered this session first. Web sessions have no ring.
4. **Check each target in turn.** In order: the client may use the connection (its connection list and data-use requirement); the request's format can reach the connection's API; for a web session, `websession::routable` (warning acknowledged, no tools, the app running) and no other web session already got the prompt; the quota group is not held by a refusal elsewhere; the connection, model and key are not cooling down or blocked. Each skipped target is named with its reason.
5. **Admission.** A client with a monthly limit holds the estimated cost before anything is sent (with a strict limit, the largest possible cost). A paid target with an unknown price is skipped for such a client. While a ledger settlement waits to be written, no paid request is sent.
6. **Journal, then send.** The attempt is written to the journal first; if that fails, nothing is sent. Same format passes through natively: the JSON is rewritten with only `model` changed, in the same key order, so cache markers and every other field reach the provider. Other pairs are translated: Anthropic to OpenAI (`anthropic.rs`), OpenAI to Anthropic (`openai_to_anthropic.rs`), and the Responses API to Chat Completions or Anthropic (`responses.rs`, ADR-033). What is dropped is named in `x-pygg-dropped`. Gemini's tool-call signatures are kept and put back (`signatures.rs`).
7. **On failure**, `policy.rs` decides by error class: whether another target may answer, and what to mark. Fallback happens only before any byte reached the client.
   - An ambiguous failure (dropped connection, read timeout, 408, 504, 524) stops: the provider may have run it.
   - A 5xx may go on only if the client allows paid retries.
   - 401, 402, 403 and 429 go on only to other quota groups. With a key ring, a 401 blocks the key and a 402 or 429 cools it; the next key is tried.
   - Cooldowns follow the provider's `Retry-After`, `retry-after-ms` or Gemini's `RetryInfo`. A rejected key stays blocked until the user replaces it.
8. **Settle.** The ledger entry (tokens, cost with its source: actual, estimated or unknown, never a guessed $0; the key that answered; the request id, `x-pygg-request-id`) and the journal removal are one transaction (`pricing.rs`, `store.rs`). An attempt left in the journal by a crash becomes an interrupted entry for the next owner of the store.

Background work while the gateway runs: `checker.rs` reads each API connection's model list every 30 minutes (free, no prompt); `probe.rs` re-reads an unreachable connection's list about once a minute and lifts the outage when it answers. **Estimated savings** (`savings.rs`) price the answered requests again at one model the user chose, with the same tokens; unknown costs and subscription requests are counted apart, never as $0.

## Web sessions (`websession/`, ADR-035)

A site with no API, used through a window the user signs in to, and served to the gateway as an ordinary OpenAI-format provider. Only the app runs them; `gateway serve` has no window and skips them with the reason.

- **The service** (`websession/mod.rs`): a Chat Completions endpoint on a random loopback port (`/web/<connection>/v1/chat/completions`), behind a token kept only in memory. Per request it refuses tools, asks the guard, waits its turn (one request at a time per connection), flattens the conversation into one prompt, opens a new chat, types, presses Send and streams the reply. Errors carry Pygg's own wording and a status the policy reads (401 needs the user, 429 busy or limit reached, 503 page changed, 504 timeout).
- **The guard** (`websession/guard.rs`), always on: each request is weighed by its size; prompts over about 12,000 characters are refused; an account has caps per hour, per 10 minutes and per UTC day; after a reply it rests as long as a person would take to read it; after a sign of trouble (timeout, usage notice, challenge, changed page) it backs off from 5 minutes up to 6 hours. It decides from what is recorded in the store, so it holds across restarts. Some sites have lower limits. The only setting is a site's daily request limit, held under a ceiling.
- **The window** (`websession/window.rs`): one Tauri webview per connection with its own WebView2 profile, no Tauri capability, hidden while Pygg drives it and closed after 10 minutes unused.
- **The recipes** (`RECIPES`, one per preset): the site's hosts and selectors. The page scripts fill the prompt, press Send and read the reply; they never read cookies or storage.
- `auto` never picks a web session; it is used only when named, in a pool or in a combo. Its cost is $0 on a free plan, unknown on a subscription.

## Tools, the command line and npm

- **One-click set-up** (`integrations.rs`) points Codex's or OpenCode's models at the gateway, or registers the MCP server: back up, make a Pygg API key of its own, write, check, and put the file back if a step fails. **The Claude Code launcher** (`launcher.rs`) keeps its key in the OS credential store and sets the gateway in one launch's environment only. Details: [AGENTS.md](AGENTS.md).
- **`pygg-cli`** (`cli/`) is the one console program; read commands open the store read-only. The npm package `@pygg/cli` is only a Node launcher that runs the native program from a per-platform optional dependency (ADR-037). Details: [design/CLI.md](design/CLI.md).

## Smart tasks (experimental, on hold)

`tasks.rs`, `agents.rs`, `isolation.rs`, `checks.rs`, `handoff.rs`, the task API (`gateway/task_api.rs`) and the MCP server (`mcp.rs`). A task is a ladder of agent steps fixed at creation, run in a fresh Git worktree of the base commit, judged by the workspace's approved checks (ADR-007, ADR-008). A paid step waits for the user's approval in the app; there is no API to approve (ADR-010, ADR-027). Each agent runs its own CLI with its own sign-in (ADR-026). Details: [AGENTS.md](AGENTS.md).

## Boundaries

- **The gateway never changes a client's conversation**: no injected memory, no rewritten history, no model switch after output started (ADR-003, ADR-025).
- **Tools reach the gateway only when the user sets them up.** A task's agent runs with its own sign-in unless the user pointed it at the gateway.
- **Sign-ins are never rotated or bridged.** Key rings and pools hold API keys of separate accounts only (ADR-034, ADR-038).
- **No chat in the app** (ADR-028): the app monitors, approves and reviews.

## Where to change what

| Change | Start in |
| --- | --- |
| A provider preset | `gateway/presets.rs`, and its source in [PROVIDERS.md](PROVIDERS.md) |
| How an error is handled | `gateway/policy.rs`, with a fixture in `tests/fixtures/provider-errors` |
| Key rings | `gateway/keys.rs` |
| What `auto` may choose | `gateway/auto.rs` (filters, ranking), `gateway/tiers.rs` (starting tiers) |
| A combo strategy | `gateway/mod.rs` (`forward`, `cheapest_first`), `combos.strategy` in a new migration |
| A translation between formats | `gateway/anthropic.rs`, `gateway/openai_to_anthropic.rs`, `gateway/responses.rs` |
| A web-session site | `websession/mod.rs` (`RECIPES`), its preset in `gateway/presets.rs`, its hosts in [PROVIDERS.md](PROVIDERS.md#web-sessions-unofficial) |
| The web guard's limits | `websession/guard.rs` |
| How a web session's window behaves | `websession/window.rs` |
| The savings estimate | `gateway/savings.rs` |
| A tool's one-click set-up | `integrations.rs`, and its name in `cli/setup.rs` |
| The Claude Code launcher | `launcher.rs`, `cli/run.rs` |
| A `pygg` command | `cli/mod.rs`, `cli/read.rs`; the npm launcher in `packages/npm/pygg/bin/pygg.js` |
| A new agent CLI | `agents.rs` (`AgentSpec`, arguments, usage parsing) |
| The task ladder | `tasks.rs` |
| The schema | a new numbered file in `migrations/`, then `python scripts/schema-reference.py` |
