# Architecture

> **Status:** Current · how the parts fit together · reviewed 2026-11-21 · built: [Implementation Status](IMPLEMENTATION-STATUS.md) · why: [DECISIONS](DECISIONS.md)

One Rust core inside a Tauri 2 window, also built as headless console programs; everything runs on the user's computer (ADR-002).

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

Rust paths below are under `apps/desktop/src-tauri/src/`; `gateway/` file names in a cell share that folder.

| Component | Path | What |
| --- | --- | --- |
| Crate | `apps/desktop/src-tauri` | One crate: the app plus `pygg-cli`, `gateway`, `agent`, `bench`, `pygg-mcp` without a window (ADR-005) |
| Interface | `apps/desktop/src` | React 19 + Vite; talks to the core only through Tauri commands (`commands.rs`) |
| Store | `db.rs`, `src-tauri/migrations/` | SQLite `store.sqlite3` in the app data folder (or `PYGG_DATA_DIR`), schema v45 ([SCHEMA](SCHEMA.md)) |
| Secrets | `vault.rs` | OS credential store; client tokens hashed ([SECURITY](SECURITY.md)) |
| Backups, log, start-up | `backup.rs`, `applog.rs`, `startup.rs` | Daily store copy (newest three); redacted log rotated at 1 MB; restore offer if the store will not open |
| Gateway | `gateway/` | Routing, translation, fallback, ledger; one gateway per store (OS lock) |
| Auto and roles | `gateway/auto.rs`, `gateway/tiers.rs`, `gateway/facts.rs` | `auto`, `auto/cheap` and role aliases (ADR-006, ADR-040) |
| Key rings | `gateway/keys.rs` | Several accounts' keys on one connection (ADR-038) |
| Error policy | `gateway/policy.rs` | Which errors may fall back and what cools down |
| Translations | `gateway/anthropic.rs`, `openai_to_anthropic.rs`, `responses.rs` | Between Chat Completions, Messages and Responses (ADR-033) |
| Background jobs | `gateway/checker.rs`, `probe.rs`, `savings.rs` | Model lists every 30 min, outage probe, savings estimate |
| Web sessions | `websession/` | Signed-in site windows served as an OpenAI-format provider, app only; guard always on (ADR-035, [SECURITY](SECURITY.md#web-sessions)) |
| Tool set-up | `integrations.rs`, `codex_catalog.rs`, `launcher.rs` | One-click Codex/OpenCode/MCP set-up and the Claude Code launcher ([AGENTS](AGENTS.md)) |
| Command line | `cli/`, `packages/npm/pygg` | `pygg-cli` and its npm launcher (ADR-037, [design/CLI](design/CLI.md)) |
| Skills & Prompts | `skills.rs` | Reads agents' instruction and skill folders; installs one `SKILL.md` (ADR-039) |
| Smart tasks (on hold) | `tasks.rs`, `agents.rs`, `isolation.rs`, `checks.rs`, `mcp.rs`, `gateway/task_api.rs` | Agent ladder in a worktree, judged by approved checks (ADR-007, ADR-008, ADR-010) |
| Brain and board | `gateway/brain.rs`, `board.rs` | `pygg/brain` proposes cards; the board turns approved cards into tasks (ADR-041, ADR-042, [design/BRAIN](design/BRAIN.md)) |

## A request through the gateway

1. **Authenticate**: loopback only, Pygg API key, per-client in-flight cap.
2. **Resolve the model**: `<connection>/<model>`, a pool, a combo, `auto` or a role alias; a session header keeps its last target; none left → 422 with reasons.
3. **Expand and check targets**: key rings, client permissions, format, cooldowns; each skipped target named with its reason.
4. **Admit**: a limited client holds the estimated cost; no paid request while the ledger cannot be written.
5. **Journal, then send**: same format passes through with only `model` changed; otherwise translated, dropped fields in `x-pygg-dropped`.
6. **Fall back or settle**: `policy.rs` decides by error class, only before any byte reached the client; the ledger entry and journal removal commit together.

## Boundaries

- The gateway never changes a client's conversation (ADR-003, ADR-025).
- Sign-ins are never rotated or bridged (ADR-034, ADR-038).
- No chat in the app (ADR-028).
