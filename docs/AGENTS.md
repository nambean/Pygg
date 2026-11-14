# Agents and coding tools

> **Status:** Current · coding tools through the gateway, agent CLIs, the MCP server and the task engine · reviewed 2026-11-14 · built: [Implementation Status](IMPLEMENTATION-STATUS.md)

A coding tool meets Pygg in one of two ways:

- **Its model requests go through the gateway.** The user sets the tool up once (CLI Tools in the app, or `pygg setup`), then starts it through Pygg (`pygg run`). This is the main path.
- **It hands a task to Pygg** through the MCP server (experimental, on hold). Pygg then runs agent CLIs, each with its own sign-in, in a Git worktree, and the workspace's approved checks judge the result.

Pygg never reads, stores or forwards an agent's own sign-in, and never bridges cookies or another product's OAuth tokens into an agent (ADR-026). Web-session connections (ADR-035) are gateway providers and have nothing to do with an agent's sign-in.

## Coding tools through the gateway

| Tool | Set up | Start | What Pygg changes |
| --- | --- | --- | --- |
| Claude Code | `pygg setup claude` | `pygg run claude` | No file. A Pygg API key "Claude Code (launcher)" in the OS credential store |
| Codex CLI | `pygg setup codex` | `pygg run codex` (adds `--profile pygg`) | A `[model_providers.pygg]` table in `config.toml` and a new `pygg.config.toml` |
| OpenCode | `pygg setup opencode` | `pygg run opencode`, then pick a `pygg/…` model with `/models` | A `pygg` provider in `opencode.json` |

The same rows are in the app under **CLI Tools → One-Click Setup** (`integrations.rs`, `launcher.rs`). For every set-up:

- **Preview first.** The app shows the change before it is made; `pygg setup <tool> --dry-run` prints it and writes nothing.
- **Backup, write, check.** The tool's configuration is backed up into the app's `integration-backups` folder, a Pygg API key of its own is made (never printed), the change is written and checked. If a step fails, the file is put back and the key revoked.
- **Remove** (`--remove`) backs up again, takes out only Pygg's part and revokes the key.
- The key is a client like any other: it reaches the connections, may not start tasks, and its limits are set in **Endpoint → API Keys → Limits…**.
- **Each launch is a routing session.** `pygg run` gives each launch a fresh session id (`launch-<uuid>`). The gateway keeps that launch on the model that answered it while the model stays eligible, which keeps the provider's prompt cache. A long-lived tool shares one session until it is started again.
- `pygg run` checks that the tool is set up, that the gateway answers and that the tool is installed (on PATH or in `~/.local/bin`) before it starts anything.

### Claude Code: the launcher

Pygg never changes Claude Code's own settings, since Claude Code on another endpoint stops using the user's Claude plan. Instead, `pygg run [--model M] claude [Claude Code's arguments…]` starts Claude Code with this environment, for that one process only:

```text
ANTHROPIC_BASE_URL              http://127.0.0.1:<port>
ANTHROPIC_AUTH_TOKEN            the launcher's Pygg API key, read from the OS credential store
ANTHROPIC_MODEL                 auto (or --model: auto/cheap, a combo, <connection>/<model>)
ANTHROPIC_DEFAULT_OPUS_MODEL    the same
ANTHROPIC_DEFAULT_SONNET_MODEL  the same
ANTHROPIC_DEFAULT_HAIKU_MODEL   the same
ANTHROPIC_CUSTOM_HEADERS        x-pygg-session: launch-<uuid>
```

`ANTHROPIC_API_KEY`, `CLAUDE_CODE_USE_BEDROCK`, `CLAUDE_CODE_USE_VERTEX` and a host session's variables are removed first. A plain `claude` keeps the user's sign-in and plan. **Remove** revokes the key and deletes it from the credential store.

Verified: unit tests (`launcher::tests`, `cli::run::tests`); live with Claude Code 2.1.322 up to the gateway (key and `auto` reached it; with no provider connected it answered 422). A session answered by a model through the launcher is not yet verified.

### Codex

Codex CLI speaks only the Responses API. The gateway serves `POST /v1/responses` for every connection (`gateway/responses.rs`, ADR-033), so Codex can use any of them, metered like any client. Codex's own ChatGPT sign-in is not involved.

What `pygg setup codex` writes (in `CODEX_HOME`, else `~/.codex`), edited with `toml_edit` so every other key, table, comment and line ending stays:

```toml
# config.toml, added at the end
[model_providers.pygg]
name = "Pygg"
base_url = "http://127.0.0.1:8741/v1"
wire_api = "responses"
experimental_bearer_token = "<a Pygg API key of its own>"
env_http_headers = { "x-pygg-session" = "PYGG_ATTEMPT_ID" }

# pygg.config.toml, a new file next to it
model_provider = "pygg"
model = "auto"
```

- **Codex's default `model` and `model_provider` are never changed.** Use `pygg run codex`, or `codex --profile pygg`, or `codex --profile pygg -m <connection>/<model>`.
- Codex 0.134 and later read a profile from `<name>.config.toml` and refuse `--profile pygg` while `config.toml` has a `[profiles.pygg]` table, so set-up refuses such a table.
- Set-up is also refused, before anything is made, for a file that is not valid TOML (the message names the line and quotes nothing), an inline `model_providers`, and an existing `pygg` provider or profile file.
- Removal deletes the profile file only while it still selects Pygg's provider. While `model_provider = "pygg"` is Codex's default, removal is refused. Apply then remove leaves `config.toml` byte-identical (a file without a final newline gets one).
- **`auto` follows the client's minimum tier** (T2 by default). With only cheaper models connected it refuses and says why: name the model, or lower the minimum in Endpoint → API Keys → Limits….
- **Tools.** For a model it does not know, Codex uses fallback metadata: its shell and function tools, but no `apply_patch`, so it edits through the shell. A model catalog (`model_catalog_json`, listing the model with `apply_patch_tool_type = "freeform"` and `base_instructions`) gives it `apply_patch`, which crosses as a function with one string parameter. Pygg does not write this catalog yet.
- **What does not cross:** OpenAI's encrypted reasoning, hosted tools (`web_search`), `prompt_cache_key` and `client_metadata`, named in `x-pygg-dropped`. Pygg stores no responses, so `previous_response_id` is refused; Codex sends the whole conversation with `store = false`.
- **Prompt cache.** A Codex turn with tools translated for Anthropic's own API gets Anthropic's automatic caching (ADR-036).
- **Windows sandbox:** in a fresh `CODEX_HOME`, `-s workspace-write` falls back to read-only until `[windows] sandbox = "unelevated"` (or the user's own elevated setup) is set.

Verified: unit tests in `integrations.rs`; live with Codex CLI 0.163.0: set-up, an answer through the gateway on Claude Haiku 5.5, removal leaving `config.toml` byte-identical; tool round trips with `apply_patch` on Anthropic and Qwen connections; the prompt-cache run in [benchmark/reports/2026-11-14-codex-claude-prompt-cache.md](../benchmark/reports/2026-11-14-codex-claude-prompt-cache.md). `pygg run codex`: unit tests only.

### OpenCode

`pygg setup opencode` adds a `pygg` provider to `opencode.json`, keeping every other key in its order. It refuses a file that is not plain JSON (comments).

```json
"provider": {
  "pygg": {
    "npm": "@ai-sdk/anthropic",
    "name": "Pygg",
    "options": {
      "baseURL": "http://127.0.0.1:8741/v1",
      "apiKey": "<a Pygg API key of its own>",
      "headers": { "x-pygg-session": "{env:PYGG_ATTEMPT_ID}" }
    },
    "models": { "auto": { "name": "auto", "tool_call": true } }
  }
}
```

- The provider speaks the Anthropic format, which the gateway serves for every connection.
- `models` lists `auto`, `auto/cheap` and the enabled models of enabled connections, with context and output limits only where Pygg knows them. Models known not to take tools are left out. Set-up with no enabled model is refused.
- Pygg keeps that list current: at start-up, after a model list refresh, and when a model or connection changes, it rewrites only its own provider's `models`, and only when the list differs.
- Outside `pygg run` or a task step, `PYGG_ATTEMPT_ID` is empty and the gateway ignores the header.

Verified: unit tests in `integrations.rs` (key order, refusal of comments, putting a file back); live: OpenCode reached the gateway through the provider. `pygg run opencode`: unit tests only.

### Other clients

Claude Code without the launcher, Cline, Continue, SDKs and curl are set up by hand: CLI Tools shows the snippets with the gateway's address. See [GETTING-STARTED](GETTING-STARTED.md).

## Agent CLIs with their own sign-in

Code: `agents.rs`. In the app: **Agents** (agents found, one-off runs). Headless: `pygg agent` (`cargo run --bin agent` in development).

| Agent | Detected as | Headless command (in the worktree) | Usage read from |
| --- | --- | --- | --- |
| Claude Code | `claude` on PATH or in `~/.local/bin` | `claude -p <task> --output-format json --permission-mode acceptEdits --allowedTools Read,Write,Edit,Glob,Grep[,Bash]` | result JSON: `usage`, `total_cost_usd`, `modelUsage`, `result` |
| Codex CLI | `codex` (also `codex login status`) | `codex exec --json --sandbox workspace-write --ephemeral [--model M] <task>` | JSON lines: `turn.completed.usage`, final `agent_message` |
| Gemini CLI | `gemini` (npm shim → `node …/gemini.js`) | `gemini -p <task> --output-format json --approval-mode auto_edit\|yolo`, with `GEMINI_CLI_TRUST_WORKSPACE=true` | `stats.models.*.tokens`, `response` |
| OpenCode | `opencode` (npm shim → `opencode.exe`) | `opencode run --format json <task> [--model provider/model]` | `step_finish` events |

- **Sign-in stays with the agent**: a Claude plan, a ChatGPT plan, an API key the agent manages, or OpenCode's providers. The only sign-in information Pygg shows is what the CLI's own status command prints, with anything key-shaped redacted.
- **One worktree per run** (ADR-008): a detached Git worktree of the current commit. The user's files and branch are not touched. The result is kept as a new branch or discarded, and the worktree is removed either way.
- **Least permission by default.** Claude Code gets only file tools; Gemini CLI gets `auto_edit`. Shell commands need **Allow** (Claude's Bash tool, Gemini's `yolo`). Codex always runs in its own `workspace-write` sandbox. Each run has a time limit (15 minutes by default), and the whole process tree is ended on timeout or cancel (`process.rs`).
- **Honest cost.** Tokens are recorded as the agent reports them. A subscription is not billed per request, so the cost is **unknown**. An agent's own list-price estimate (Claude Code's `total_cost_usd`, OpenCode's step costs) goes into the cost's source, not into spend.
- On Windows, npm installs `.cmd` shims. Pygg starts their target (an `.exe`, or a `.js` through Node) directly, because a free-text task cannot be quoted safely through `cmd`.

Verified: live with Codex CLI 0.163.0 (ChatGPT plan) and OpenCode 1.18.4 (a free model); Claude Code 2.1.322 through the MCP server (below). Gemini CLI 0.66.0 signed in with a Google account is refused by Google ("no longer supported for Gemini Code Assist for individuals"); a Gemini API key is untested.

## The MCP server and the task API (experimental, on hold)

Tasks are started where people already work, by their coding agent (ADR-027). The app runs them, and is where they are watched, paid steps approved, and history kept.

- **The MCP server** (`mcp.rs`) runs over stdio. The app's own program serves it as `<app> mcp`; `pygg mcp` (and `pygg-mcp`) is the same server. Tools: `pygg_run_task`, `pygg_task_status`, `pygg_task_diff`, `pygg_keep_result`, `pygg_discard_result`, `pygg_stop_task`, `pygg_list_tasks`, `pygg_agents`. The workspace is the folder the server runs in. Configuration: `PYGG_TOKEN`, and optionally `PYGG_URL` (default `http://127.0.0.1:8741`).

  ```text
  claude mcp add pygg -e PYGG_TOKEN=<key> -- "C:\path\to\Pygg.exe" mcp
  ```

- **One-click registration**: **Agents → Let Your Agent Delegate Tasks**, or `pygg mcp setup claude-code|codex|opencode`. Claude Code and Codex edit their own files through their own commands (`claude mcp add --scope user`, `codex mcp add`; removal `… mcp remove`). OpenCode's `opencode.json` is edited by Pygg. The entry is user-wide, never a project file, which would be committed with the token in it. The token may start tasks and reaches no model connection. An entry named `pygg` that Pygg did not make is left alone. **Set It Up by Hand** shows snippets for Cursor and other JSON-configured clients.
- **Waiting.** A tool call waits at most 50 s by default, under the 60 s many MCP clients allow; the agent then calls `pygg_task_status` again. Codex needs `tool_timeout_sec` raised to wait longer, and `default_tools_approval_mode = "approve"` for `codex exec`, which cannot ask before a tool call.
- **The task API** (`gateway/task_api.rs`): `GET /pygg/v1/agents`, `GET|POST /pygg/v1/tasks`, `GET /pygg/v1/tasks/{id}?wait=S`, `GET …/diff`, `POST …/stop|keep|discard`. The MCP server holds no state; it forwards to this API.
- **Permission.** A key may start tasks only after the user turns on **May start tasks** (Endpoint → API Keys → Limits…; `pygg gateway client --name N --tasks yes`). A client sees and acts on only its own tasks. **There is no API to approve a paid step.**
- **Where tasks run:** in the app. `pygg gateway serve` answers the task API with 503 unless started with `--tasks yes`, for a computer where the app is not open on that store.

Verified: integration test `integrations::tests::live` with Claude Code 2.1.322, Codex CLI 0.163.0 and OpenCode 1.18.4 on scratch configuration folders (set up, listed by the agent, second set-up refused, removed, token revoked); live delegation from Codex CLI 0.163.0 (`codex exec`) and Claude Code 2.1.322 (`claude -p`) through `pygg mcp`: task run, check passed, diff read, result kept as a branch. Not yet clicked through in the app.

## The task engine (experimental, on hold)

`tasks.rs`. A task is a ladder of agent steps fixed when it is created (`tasks.policy_snapshot_json`), run against the workspace's **approved** checks.

1. Each attempt runs one step (agent, model, tier, billing) in a fresh worktree of the task's **base commit**, so every attempt starts from the same code. `PYGG_ATTEMPT_ID` is set to the attempt's id; an agent pointed at the gateway sends it as its session header, and the gateway's entries are then tied to the attempt.
2. The approved checks run in order and stop at the first failure. Passing means exit code 0.
3. A failed attempt is discarded and the next step starts. What the next step is told is the task's handoff mode (ADR-030):
   - `tail` (default): the failed check's command and the last 1,500 characters of its redacted output.
   - `none`: the task alone.
   - `evidence`: every earlier attempt, built by `handoff.rs` (check output cleaned, paths changed, the agent's final message marked unverified), about 6,000 characters at most. It did not save tokens, time or money in [its A/B](../benchmark/reports/2026-11-12-h1-handoff-ab.md).

   An attempt that changed nothing counts as a failure. A step whose agent and model timed out on their last two runs in the past 30 minutes, with no output token, is skipped, unless it is the last step left.
4. Checks pass: the task is `checks_passed` and waits for **Keep as Branch**, **Export Patch** or **Discard**. No approved checks, or a change to the file that defines a check (`package.json`, `Cargo.toml`): `needs_review`. Keeping a reviewed result: `manually_accepted`.
5. **A paid step waits for approval** (ADR-010) with the cost range of the last 10 runs of that agent and model, and needs a spending cap. Claude Code gets the cap as `--max-budget-usd`; other agents are checked afterwards, and over the cap the ladder stops (`blocked`). A step's cost is Pygg's ledger when its requests went through the gateway with the attempt id, else the agent's own estimate; an unpriced request makes it unknown, never $0. A changed estimate or budget invalidates the approval. Declining blocks the task.
6. **Daily budgets** (UTC): one for all tasks and one per workspace. A running paid step holds its cap; free, local and subscription steps do not count.
7. **Stop** ends the running agent or check with its process tree. At start-up a task left `running` becomes `interrupted` (Retry runs that step again), and leftover worktrees are removed.

Workspaces must be Git repositories and are managed on the Workspaces screen (checks, privacy, budget). A **sensitive** workspace asks before every step. Detected checks (`package.json` scripts, `cargo check`, `cargo test`) are suggestions until approved. Keep checks inside the repository: an agent may refuse to read outside its worktree.

Headless:

```text
pygg agent detect
pygg agent [--db STORE] run --agent ID --workspace DIR --prompt TEXT [--model M] [--shell yes] [--timeout SECS] [--keep TITLE | --discard yes]
pygg agent --db STORE task --workspace DIR --prompt TEXT --steps "opencode:opencode/some-free-model,codex" [--check CMD] [--keep TITLE | --discard yes]
pygg agent --db STORE quality [--days N]
```

Verified: unit tests in `tasks.rs` with a scripted agent in real worktrees (approvals, stop, interruption, retry); live cascades with OpenCode 1.18.4 (free model) then Codex CLI 0.163.0; a paid step approved live in the installed app, OpenCode through the gateway on Claude Haiku 5.5, its cost taken from Pygg's ledger.

## Not built yet

- Codex's model catalog for the models Pygg lists, so Codex gets `apply_patch` and real context sizes.
- Cancelling a single run on the Agents screen (tasks can be stopped).
