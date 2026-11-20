# Architecture

> **Status:** Current · how the parts fit together · reviewed 2026-11-21 · built: [Implementation Status](IMPLEMENTATION-STATUS.md) · why: [DECISIONS](DECISIONS.md)

One Rust core inside a Tauri 2 window, also built as headless console programs. Everything runs on the user's computer (ADR-002).

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
                   (tasks.rs,        (db.rs, v45)    store           endpoints, local runtimes, and
                    on hold)                         (vault.rs)      web sessions (websession/: a
                        │                                            loopback endpoint → a Pygg window)
                        ▼  one Git worktree per attempt (isolation.rs)
                   Agent CLIs with their own sign-in (agents.rs): Claude Code, Codex CLI, Gemini CLI, OpenCode
                        │
                        ▼
                   Approved checks (checks.rs) decide pass or fail
```

| Component | Where | Facts |
| --- | --- | --- |
| The crate | `apps/desktop/src-tauri` (ADR-005) | One crate. The app is the default binary; `pygg-cli`, `gateway`, `agent`, `bench`, `pygg-mcp` (`src/bin/`) run the same library without a window. The window is Cargo feature `app`; `pygg-cli` builds with `--no-default-features` for macOS and Linux |
| The interface | `apps/desktop/src` | React 19 and Vite. Talks to the core only through Tauri commands (`commands.rs`, `src/lib/native.ts`) |
| The store | `store.sqlite3` in the app data folder | `%APPDATA%\app.smarttaskrouter.desktop` (Windows), `~/Library/Application Support/…` (macOS), `$XDG_DATA_HOME` or `~/.local/share/…` (Linux), or `PYGG_DATA_DIR`. Migrations in `migrations/` run in one transaction on open; schema version = number of migrations (v45). Reference: [SCHEMA.md](SCHEMA.md) |
| Secrets | `vault.rs` | OS credential store (Windows Credential Manager, macOS Keychain, Linux Secret Service); the store keeps only a reference. Client tokens stored hashed. Rules: [SECURITY.md](SECURITY.md) |
| Backups | `backup.rs` | Store copied to `backups/` at start-up and about once a day; newest three kept |
| App log | `applog.rs` | `logs/pygg.log`, redacted, rotated at 1 MB |
| Start-up | `startup.rs` | A store that cannot be opened is explained, with an offer to restore the latest copy |
| Gateway lock | `gateway::start` | One gateway per store: OS lock on `<store>.gateway-lock`, held until every attempt it sent is settled |
| Skills & Prompts | `skills.rs` (ADR-039) | Reads agents' global instruction files and skill, command and prompt folders, and the same at each added project's root. Writes one `SKILL.md` per install into `~/.claude/skills`, `~/.agents/skills`, or a project's `.claude/skills` / `.agents/skills`, with Codex's `agents/openai.yaml` beside a prompt in an `.agents/skills` folder. Never downloads or runs anything |

## A request through the gateway (`gateway/`)

1. **Authenticate.** 127.0.0.1 only; non-loopback `Host` refused. Pygg API key required (`Authorization: Bearer` or `x-api-key`). Each client has a cap on requests in flight.
2. **Resolve the model name** (`store::resolve`, then `forward()` in `gateway/mod.rs`):
   - `<connection>/<model>`: one target.
   - `<pool>/<model>`: the pool's connections in the user's order (ADR-034). In a pool of web sessions, an account the guard is resting goes last, then the one with fewest requests today.
   - A **combo**: its members. Strategy `order` keeps them as listed; `cheapest` re-orders per request by estimated cost (`cheapest_first`: free, local and subscription at $0, unknown price last).
   - `auto`, `auto/cheap`, or a role alias (`auto/plan`, `auto/code`, `auto/test`, `auto/review`): hard filters, then rules (`auto.rs`, `tiers.rs`, ADR-006); role aliases rank by the model facts feed (`facts.rs`, ADR-040, ADR-041). Each excluded model has a reason; none left → 422 with the reasons.
   - `x-pygg-session`: goes first to the target that last answered that session, while it is still in the route. A session a fallback took from its home goes back home at request start once the home's cache is still warm or the fallback's has gone cold too (five minutes, `store::Pin::goes_home`) and the home is not cooling down; the fallback stays next. Sessions are never guessed from the prompt.
3. **Expand key rings** (`keys.rs`, ADR-038). One target per key, ordered by strategy (`in_order`, `round_robin`, `least_used`, `quota_aware`), the key that answered this session first. Web sessions have no ring.
4. **Check each target**, in order: client may use the connection (connection list, data-use requirement); the request's format can reach the connection's API; for a web session, `websession::routable` (warning acknowledged, no tools, app running) and no other web session already got the prompt; quota group not held by a refusal elsewhere; connection, model and key not cooling down or blocked. Each skipped target is named with its reason.
5. **Admission.** A client with a monthly limit holds the estimated cost first (strict limit: the largest possible cost). A paid target with unknown price is skipped for such a client. While a ledger settlement waits to be written, no paid request is sent.
6. **Journal, then send.** Journal write first; if it fails, nothing is sent.
   - Same format passes through: JSON rewritten with only `model` changed, same key order, so cache markers and all fields reach the provider.
   - ADR-036 exceptions: a top-level cache marker on an agent's turn to Claude (Anthropic's API, or OpenRouter); the session passed on as OpenRouter's `x-session-id` or OpenAI's `prompt_cache_key`.
   - Translations: Anthropic → OpenAI (`anthropic.rs`), OpenAI → Anthropic (`openai_to_anthropic.rs`), Responses API → Chat Completions or Anthropic (`responses.rs`, ADR-033). Dropped fields named in `x-pygg-dropped`. Gemini's tool-call signatures kept and restored (`signatures.rs`).
7. **On failure**, `policy.rs` decides by error class whether another target may answer and what to mark. Fallback only before any byte reached the client.
   - Ambiguous (dropped connection, read timeout, 408, 504, 524): stop; the provider may have run it.
   - 5xx: goes on only if the client allows paid retries.
   - 401, 402, 403, 429: go on only to other quota groups. In a key ring, 401 blocks the key, 402 or 429 cools it, and the next key is tried.
   - Aggregator 401/402/403/429 marked as the upstream's (`error.type` `upstream_error`, new-api and one-api): cools only that model (401/402/403: ten minutes; 429: the reset given, else 30 s) and goes on to any target.
   - Cooldowns follow `Retry-After`, `retry-after-ms` or Gemini's `RetryInfo`; a 429 without one cools 30 s. A rejected key stays blocked until the user replaces it.
8. **Settle.** Ledger entry (tokens; cost with source: actual, estimated or unknown, never a guessed $0; the key that answered; request id `x-pygg-request-id`) and journal removal in one transaction (`pricing.rs`, `store.rs`). An attempt left in the journal by a crash becomes an interrupted entry for the store's next owner.

Background work while the gateway runs:

| Job | Code | What |
| --- | --- | --- |
| Model lists | `checker.rs` | Reads each API connection's model list every 30 minutes (free, no prompt) |
| Outage probe | `probe.rs` | Re-reads an unreachable connection's list about once a minute; lifts the outage when it answers |
| Estimated savings | `savings.rs` | Re-prices answered requests at one model the user chose, same tokens; unknown and subscription costs counted apart, never $0 |

## Web sessions (`websession/`, ADR-035)

A site with no API, used through a window the user signs in to, served to the gateway as an OpenAI-format provider. App only: `gateway serve` has no window and skips them with the reason. Security rules: [SECURITY.md](SECURITY.md#web-sessions).

| Part | Code | Facts |
| --- | --- | --- |
| Service | `websession/mod.rs` | Chat Completions on a random loopback port (`/web/<connection>/v1/chat/completions`), behind an in-memory token. Per request: refuse tools, ask the guard, wait its turn (one at a time per connection), flatten the conversation into one prompt, open a new chat, type, Send, stream the reply. Errors: Pygg's wording; 401 needs the user, 429 busy or limit, 503 page changed, 504 timeout |
| Guard | `websession/guard.rs` | Always on. Weighs each request by size, refuses long prompts, caps per 10 minutes, hour and UTC day, rests after a reply, backs off after trouble (timeout, usage notice, challenge, changed page). Decides from the store, so it holds across restarts. Only setting: a site's daily request limit, under a ceiling. Numbers: [PROVIDERS](PROVIDERS.md#the-guard) |
| Window | `websession/window.rs` | One Tauri webview per connection, own WebView2 profile, no Tauri capability, hidden while Pygg drives it, closed after 10 minutes unused |
| Recipes | `RECIPES` | One per preset: hosts and selectors. Page scripts fill, Send, read; never read cookies or storage |
| Routing | | `auto` never picks one; used only when named, in a pool or in a combo. Cost $0 on a free plan, unknown on a subscription |

## Tools, the command line and npm

| Part | Code | Facts |
| --- | --- | --- |
| One-click set-up | `integrations.rs`, `codex_catalog.rs` | Points Codex's or OpenCode's models at the gateway (Codex with a catalog of Pygg's models), or registers the MCP server: back up, make its own Pygg API key, write, check, restore the file if a step fails. [AGENTS.md](AGENTS.md) |
| Claude Code launcher | `launcher.rs` | Key in the OS credential store; gateway set in one launch's environment only |
| `pygg-cli` | `cli/` | The one console program; read commands open the store read-only. [design/CLI.md](design/CLI.md) |
| npm `@pygg/cli` | `packages/npm/pygg` | A Node launcher that runs the native program from a per-platform optional dependency (ADR-037) |

## Smart tasks (experimental, on hold)

- Code: `tasks.rs`, `agents.rs`, `isolation.rs`, `checks.rs`, `handoff.rs`, task API `gateway/task_api.rs`, MCP server `mcp.rs`. Details: [AGENTS.md](AGENTS.md).
- A task is a ladder of agent steps fixed at creation, run in a fresh Git worktree of the base commit, judged by the workspace's approved checks (ADR-007, ADR-008).
- A paid step waits for approval in the app; no API approves one (ADR-010, ADR-027). Each agent uses its own CLI and sign-in (ADR-026).
- Built: model facts feed with per-role scores (`gateway/facts.rs`, ADR-040); role aliases `auto/plan|code|test|review` ranked by it (`gateway/auto.rs`); virtual model `pygg/brain` (`gateway/brain.rs`, ADR-041) turning a requirement into a proposed board, a model per card picked by rule, approved in the app; the board (`board.rs`, ADR-042) pulling approved cards into tasks under WIP limits; the Brain Storming screen. Design: [design/BRAIN.md](design/BRAIN.md).

## Boundaries

- **The gateway never changes a client's conversation**: no injected memory, no rewritten history, no model switch after output started (ADR-003, ADR-025).
- **Tools reach the gateway only when the user sets them up.** A task's agent uses its own sign-in unless pointed at the gateway.
- **Sign-ins are never rotated or bridged.** Key rings and pools hold API keys of separate accounts only (ADR-034, ADR-038).
- **No chat in the app** (ADR-028): it monitors, approves and reviews.

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
