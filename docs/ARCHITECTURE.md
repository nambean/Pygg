# Architecture

> **Status:** Current · how the parts fit together · reviewed 2026-11-11 · order: [ROADMAP](ROADMAP.md) · built: [Implementation Status](IMPLEMENTATION-STATUS.md)

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
                                       │                   │ /v1/messages, /v1/models      │
                                       │                   └──────────────┬────────────────┘
                        ┌──────────────┼───────────────┐                  │ provider APIs (keys from
                        ▼              ▼               ▼                  ▼ the OS credential store)
                   Task engine    SQLite store    OS credential     OpenAI-compatible and
                   (tasks.rs)     (db.rs, v14)    store (vault.rs)  Anthropic endpoints, local runtimes
                        │
                        ▼  one Git worktree per attempt (isolation.rs)
                   Agent CLIs with their own sign-in (agents.rs): Claude Code, Codex CLI, Gemini CLI, OpenCode
                        │
                        ▼
                   Approved checks (checks.rs) decide pass or fail
```

- **One crate** (`apps/desktop/src-tauri`, ADR-005). The app is the default binary; `gateway`, `agent`, `bench` and `pygg-mcp` are the same library run headless. The interface is React 19 and Vite in `apps/desktop/src`, talking to the core only through Tauri commands (`commands.rs`, `lib/native.ts`).
- **One store**, `store.sqlite3` in the app data folder (`%APPDATA%\app.smarttaskrouter.desktop` on Windows, or `PYGG_DATA_DIR`). Migrations run in one transaction when it opens. Schema: [SCHEMA.md](SCHEMA.md), generated from the migrations.
- **One gateway per store**: `gateway::start` takes an OS file lock next to the store, held until every attempt it sent is settled.

## A request through the gateway (`gateway/`)

1. **Authenticate** the client token (`Authorization: Bearer` or `x-api-key`); only loopback `Host` names are served.
2. **Resolve the model name**: `<connection>/<model>`, a combo (an ordered list), or `auto` / `auto/cheap`, which build a route from the connected models with hard filters first, then rules (`auto.rs`, ADR-006). An explicit `x-smarttaskrouter-session` pins a session to the target that answered it.
3. **For each target in turn**: the client's connection list and data-use requirement, the API format, health and cooldowns, quota-group holds, then **admission**: a client with a limit holds the estimated (or, if strict, the largest possible) cost before anything is sent. The attempt is **journaled** before it is sent.
4. **Send**: the same format passes through as a native-format passthrough (key order and cache markers kept); Anthropic to OpenAI-compatible is translated (`anthropic.rs`), and what is dropped is named in `x-smarttaskrouter-dropped`.
5. **On failure**, `policy.rs` decides by error class: try another model, another connection, or stop (a 5xx or an ambiguous timeout stops unless the client allows retries), and what to mark: a key rejected, a model cooling down, a quota group held. Fallback happens only before any byte reached the client.
6. **Settle**: the ledger entry (tokens, cost with its source: actual, estimated or unknown, never a guessed $0) and the journal removal are one transaction. While a settlement cannot be written, no paid request is admitted.

A background **probe** (`probe.rs`) reads the free model list of an unreachable connection once a minute and lifts the outage when it answers.

## A task (`tasks.rs`, `agents.rs`, `isolation.rs`, `checks.rs`)

1. A task is created from the app or from an IDE agent through MCP and the task API (ADR-027). Its **ladder of steps** (agent, model, tier, billing) and its workspace's **approved checks** are fixed then, with the base commit.
2. A **paid step waits for approval** with an estimate and a cap (ADR-010); a sensitive workspace asks before every step. There is no API to approve.
3. Each attempt runs the **agent's own CLI with its own sign-in** (ADR-026) in a fresh worktree of the base commit, with `PYGG_ATTEMPT_ID` set. Pygg never reads that sign-in.
4. The approved checks run in the worktree. A failure discards it and hands the failed command and the end of its output to the next step. A pass leaves the result for **Keep as Branch**, **Export Patch** or **Discard**.
5. The agent's reported tokens go to the ledger with an unknown cost (a subscription is not billed per request); its own list-price estimate is kept beside it.

## Boundaries

- **The gateway never changes an external client's conversation**: no injected memory, no rewritten history, no model switch after output started (ADR-003, ADR-025).
- **Agents are not routed through the gateway by Pygg.** A user may point an agent at the gateway in the agent's own configuration (the benchmark does, with OpenCode); its requests are then metered there and can carry the attempt id as their session.
- **No chat in the app** (ADR-028): the app monitors, approves and reviews.

## Where to change what

| Change | Start in |
| --- | --- |
| A provider preset | `gateway/presets.rs`, and its source in [PROVIDERS.md](PROVIDERS.md) |
| How an error is handled | `gateway/policy.rs`, with a fixture in `tests/fixtures/provider-errors` |
| What `auto` may choose | `gateway/auto.rs` (filters, ranking), `gateway/tiers.rs` (starting tiers) |
| A new agent CLI | `agents.rs` (`AgentSpec`, arguments, usage parsing) |
| The task ladder | `tasks.rs` |
| The schema | a new numbered file in `migrations/`, then `python scripts/schema-reference.py` |
