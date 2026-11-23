# Agents and coding tools

> **Status:** Current · coding tools through the gateway, agent CLIs, the MCP server and the task engine · built and verified: [Implementation Status](IMPLEMENTATION-STATUS.md)

A coding tool meets Pygg in one of two ways:

- **Through the gateway** (main path): set up once (CLI Tools, or `pygg setup`), start with `pygg run`.
- **By handing Pygg a task** over the MCP server (experimental, on hold): Pygg runs agent CLIs, each with its own sign-in, in a Git worktree; the workspace's approved checks judge the result.

Pygg never reads, stores or forwards an agent's own sign-in, and never bridges cookies or another product's OAuth tokens into an agent (ADR-026).

## Coding tools through the gateway

| Tool | Set up | Start | What Pygg changes | Status |
| --- | --- | --- | --- | --- |
| Claude Code | `pygg setup claude` | `pygg run claude` | No file; a launcher key in the OS credential store | Live up to the gateway |
| Codex CLI | `pygg setup codex` | `pygg run codex` (or `codex --profile pygg`) | A `pygg` provider in `config.toml`, plus `pygg.config.toml` and `pygg-models.json` | Live (0.163.0), removal byte-identical |
| OpenCode | `pygg setup opencode` | `pygg run opencode`, then a `pygg/…` model | A `pygg` provider in `opencode.json` | Live up to the gateway |
| Others (Cline, Continue, SDKs) | By hand from CLI Tools ([GETTING-STARTED](GETTING-STARTED.md)) | — | — | — |

- **Preview first:** `--dry-run` prints the change and writes nothing; **Remove** (`--remove`) takes out only Pygg's part and revokes its key.
- **Backed up:** on failure the file is restored and the key revoked.
- **Each tool gets its own API key:** it reaches the connections but may not start tasks; limits in **Endpoint → API Keys → Limits…**.
- **Claude Code's settings never change** (another endpoint would drop the user's plan); a plain `claude` keeps its sign-in.
- **Codex's default model and provider never change**; `auto` follows the client's minimum tier (T2 default).
- **Codex dropped features** are named in `x-pygg-dropped` (encrypted reasoning, hosted tools); `previous_response_id` is refused.
- **Each launch is a routing session** (`launch-<uuid>`) that keeps the answering model while eligible.
- Where: `integrations.rs`, `launcher.rs`, `codex_catalog.rs`, `gateway/responses.rs` (ADR-033).

### The session receipt

Printed when a tool started by `pygg run` exits: cost (unknown and subscription counted apart, never $0), cache share, models and fallbacks, request depth, a comparison model, and Git changes since the start.

- Kept in `launches`: counts and commit ids, never file names or code.
- `pygg launches --by-repo` sums cost and commits per repository and week; observations only (ADR-036).
- `pygg run --statusline claude` shows spend, cache and limit in Claude Code's status line.
- Session and day limits (`pygg gateway client --session-usd N --day-usd N`) return 402 once reached; a running request is never cut.
- Claude Code's request-class hint headers are recorded, never forwarded to the provider.
- Status: unit tests; not run live.

## Agent CLIs with their own sign-in

| Agent | Headless command | Status |
| --- | --- | --- |
| Claude Code | `claude -p … --permission-mode acceptEdits` (Read, Glob and Grep pre-allowed; edits accepted inside the copy only; Bash, WebFetch and WebSearch only on **Allow**, else in `--disallowedTools`) | Live through the MCP server; outside writes refused (proof 2026-11-23) |
| Codex CLI | `codex exec --json --sandbox workspace-write --ephemeral` (network only on **Allow** web) | Live (ChatGPT plan) |
| Gemini CLI | `gemini -p … --approval-mode auto_edit` (`yolo` only on **Allow**) | Refused by Google for personal accounts; API key untested |
| OpenCode | `opencode run --format json`, permissions set per run: `bash`, `webfetch` and `websearch` as allowed, `external_directory` denied | Live (free model) |

- **Sign-in stays with the agent;** Pygg shows only the CLI's status, key-shaped text redacted.
- **One worktree per run** (ADR-008); user files and branch untouched; kept as a branch or discarded.
- **Least permission (the safety profile):** shell and web each need **Allow**. New work shows per agent, on this platform, writes and reads outside the copy, shell, network and secrets as blocked, allowed, not guaranteed or best effort: an allowed shell reaches past tool rules; Codex's sandbox is not relied on on Windows; Gemini CLI holds to none of them here. Variables in Pygg's environment that look like secrets (by name, or a URL with a password) and that the agent's own sign-in does not use are withheld, proxies kept; files and credential stores are not covered. 15-minute limit; the whole process tree ends on timeout or cancel.
- **Honest cost:** subscription cost is unknown, never $0.
- Where: `agents.rs`; app **Agents**; headless `pygg agent`.

## The MCP server and the task API (experimental, on hold)

Tasks start from the user's coding agent (ADR-027; a plan may also start on a project's page); the app runs them, approves paid steps and keeps history.

- **Server:** `<app> mcp`, `pygg mcp` or `pygg-mcp` (stdio); config `PYGG_TOKEN`, optional `PYGG_URL` (default `http://127.0.0.1:8741`).
- **MCP tools:** `pygg_run_task`, `pygg_task_status`, `pygg_task_diff`, `pygg_task_brief`, `pygg_keep_result`, `pygg_discard_result`, `pygg_stop_task`, `pygg_list_tasks`, `pygg_agents`, `pygg_brainstorm`, `pygg_model_facts`, `pygg_board`, `pygg_board_move`.
- **Claude Code plugin:** `/plugin install pygg --marketplace nambean/Pygg` (MCP server plus skills `delegate-tasks`, `route-through-pygg`).
- **One-click registration:** **Agents → Let Your Agent Delegate Tasks**, or `pygg mcp setup claude-code|codex|opencode`.
- A tool call waits at most 50 s, then the agent asks `pygg_task_status` again.

| Endpoint | Use |
| --- | --- |
| `GET /pygg/v1/agents` | Agents found |
| `GET\|POST /pygg/v1/tasks` | List or start tasks |
| `GET /pygg/v1/tasks/{id}[/diff\|/brief]` | Status (`?wait=S`), diff, or a brief to continue elsewhere |
| `POST /pygg/v1/tasks/{id}/stop\|keep\|discard` | Stop, keep as branch, discard |
| `GET /pygg/v1/brainstorms`, `/facts`, `/board` | Read-only |
| `POST /pygg/v1/board/{card}/move` | Discovery ↔ Ready only |

**Permission rules:**

- A key starts tasks only with **May start tasks** on (`pygg gateway client --name N --tasks yes`).
- The MCP token may start tasks and reaches no model connection.
- Registration is always user-wide, never a project file (the token would be committed).
- A client sees and acts on only its own tasks.
- **There is no API to approve a paid step;** brainstorms are approved only in the app.
- `pygg gateway serve` answers the task API with 503 unless started with `--tasks yes`.

Status: live delegation from Codex CLI and Claude Code (task run, check passed, kept as a branch); plugin installed from the local marketplace; not clicked through in the app.

## The task engine (experimental, on hold)

A task is a ladder of agent steps fixed at creation, judged by the workspace's **approved** checks (`tasks.rs`).

- Each attempt runs one step in a fresh worktree of the base commit; `PYGG_ATTEMPT_ID` ties ledger entries to it.
- Pygg's own MCP server is off in the runs it starts (OpenCode: `OPENCODE_CONFIG_CONTENT` with `mcp.pygg.enabled=false`, merged with any the user set; Codex: `-c mcp_servers.pygg.enabled=false`; Claude Code runs with a fixed tool list): with it, an agent handed its task back to Pygg instead of doing it.
- What the agent does shows live on the task's screen (`live.rs`): its output is read line by line as it runs, turned into readable lines and redacted, the last 500 lines of an attempt kept in memory.
- A failed attempt is discarded and the next step gets a handoff (`tail` default, `none`, `evidence`, or `brief`: the portable brief; ADR-030).
- Checks pass → **Keep as Branch**, **Export Patch** or **Discard**; no approved checks → needs review.
- **Paid step waits for approval** with a spending cap (ADR-010). Through Pygg's gateway (a `pygg/...` model) the cap holds: each request of the step, from any key naming its session, holds the most it could cost inside what is left, its output limit lowered to fit; a request that cannot fit gets 402 once the cap is spent, or 429 while requests in flight hold the rest; an unpriced model is not used. Elsewhere the cap is checked after the step, and over it the ladder stops.
- **Daily budgets** (UTC) for all tasks and per workspace.
- **Sensitive** workspace asks before every step; detected checks are suggestions until approved.
- Headless: `pygg agent --db STORE task --workspace DIR --prompt TEXT --steps "…" [--check CMD]`.
- Status: live cascade OpenCode → Codex; a paid step approved live in the installed app.

## Not built yet

- Cancelling a single run on the Agents screen (tasks can be stopped).
