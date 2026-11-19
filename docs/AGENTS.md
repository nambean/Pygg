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
- **A receipt when the tool exits** (below).

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
CLAUDE_CODE_GATEWAY_HINT_HEADERS 1
```

`ANTHROPIC_API_KEY`, `CLAUDE_CODE_USE_BEDROCK`, `CLAUDE_CODE_USE_VERTEX` and a host session's variables are removed first. A plain `claude` keeps the user's sign-in and plan. **Remove** revokes the key and deletes it from the credential store.

Verified: unit tests (`launcher::tests`, `cli::run::tests`); live with Claude Code 2.1.322 up to the gateway (key and `auto` reached it; with no provider connected it answered 422). A session answered by a model through the launcher is not yet verified.

### The session receipt

When a tool started by `pygg run` exits, Pygg prints what the launch cost and what came of it (`launches.rs`):

```text
Pygg · this launch of Claude Code (launch-…)
  Cost       $0.42 for 34 answered requests (2 on a subscription, not counted)
  Cache      91% of the prompt read from cache, 4% written to it
  Models     claude-haiku-5-5 ×30, glm-5 ×4 · 1 failed attempt, 1 answered by the next model
  Depth      a request cost $0.0040 at the start and $0.019 at the end (each turn sends the conversation again)
  Compared   $1.90 at anthropic/claude-sonnet-5-5 with the same tokens, against $0.42 (an estimate)
  Git        F:/work/app (main): 2 commits, 5 files changed since the start, +120 −14
```

- **Cost** is the ledger's known money for the launch's session, failed attempts with a known cost included. Requests with no known cost and subscription requests are counted beside it, never taken as $0.
- **Depth** appears from 20 priced requests on: the average of the first ten against the last ten. A high cache share means caching works, not that the session was cheap.
- **Classes** (Claude Code only): money and requests by what each request was, from Claude Code's gateway hint headers: `main`, `subagent`, `workflow`, `compaction`, `auxiliary` (side requests such as titles and classifiers). For subagents, how many there were and how many worked at once. **Fan-out** is added when one prompt started 10 or more subagents: each starts its own conversation and writes its own cache.
- **Compared** appears when a model to compare with is chosen (Usage → Savings); it is the same estimate as there, for this session.
- **Git** is read from the folder `pygg run` was started in: HEAD, branch and the tracked files with uncommitted changes before the tool starts; after it exits, the commits made since and `git diff --shortstat` from the starting HEAD to the working tree. Changes that were already uncommitted at the start are included and said so. A change in Git is not proof the work was good; it lets cost be set against what came of it.
- **Limit** appears when the key the launch used has a session or day limit (Endpoint → API Keys → Limits…, or `pygg gateway client --session-usd N --day-usd N`): what each has counted, and once one is reached, that Pygg refuses the launch's further requests with 402 (until 00:00 UTC for the day's). A request already running is never cut, and an estimated (not strict) limit lets a request go while the counted spend is under it, so the session can end a little over its limit.
- **The launch is kept** in `launches` (schema v38): the session id, tool, model, times, exit code, the repository's folder, branch, commit ids and the counts above, never file names or code. `pygg run` writes the row only when the store already has the table (the app migrates it; `pygg run` never does); otherwise the receipt says it was not saved. Rows go with the ledger's history setting.
- **By repository.** `pygg launches --by-repo [--weeks N]` sums the kept launches per repository and UTC week (Monday to Sunday, 8 weeks by default): their known cost (requests with no known cost marked `+?`), the commits they made, cost per commit, and the launches that changed nothing (settled, in a repository, no commit and no change against the starting commit). Launches still running or cut off are counted but not judged. These are observations from the ledger and Git, never a measure of productivity or quality (ADR-036): a commit is not a unit of work, and a launch that changed nothing may have answered a question.

**Request classes.** The launcher sets `CLAUDE_CODE_GATEWAY_HINT_HEADERS=1`, so Claude Code (2.1.273 and later) tells the gateway what each request is, which subagent sent it (its type and a random id per spawn) and which prompt it serves (2.1.283 and later). The gateway keeps the class, agent type, agent id and prompt id with the attempt (schema v39, `gateway/hints.rs`) and forwards none of these headers to the provider. A class outside Claude Code's documented list is kept as `other`; an id that is not short printable ASCII is dropped. Usage → Token Mix → By request kind and `pygg usage` show the same split for the period, with the subagent types, the most subagents one prompt started and the most working at once in a session (each agent's span runs from the start of its first request to the end of its last). Codex and OpenCode send no such headers.

**Status line.** `pygg run --statusline claude` starts Claude Code with `--settings` naming `pygg statusline` as its status line, for that launch only, so its own setting is untouched. `pygg statusline` prints the launch's cost so far, cache share, the model that answered and fallbacks (`Pygg $0.42 · 91% cache · claude-haiku-5-5 · 1 fallback`). It also counts the launch's subagents. With a session limit on the key it reads `Pygg $0.42 of $5.00 · …`, and once a session or day limit is reached, `Pygg $5.10 · session limit $5.00 reached · …`. It finds the launch from `x-pygg-session` in `ANTHROPIC_CUSTOM_HEADERS`, or `PYGG_ATTEMPT_ID`, and reads the store without writing it.

Verified: unit tests (`launches::tests`, including Git in a scratch repository; `gateway::hints::tests`); a gateway test sends the hint headers and checks they are recorded and that none reaches the provider; the browser preview's Usage with demo data. Not yet run live with a tool's real session.

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
model_catalog_json = "<CODEX_HOME>/pygg-models.json"

# pygg-models.json, a new file next to it: the catalog of Pygg's models (below)
```

- **Codex's default `model` and `model_provider` are never changed.** Use `pygg run codex`, or `codex --profile pygg`, or `codex --profile pygg -m <connection>/<model>`.
- Codex 0.134 and later read a profile from `<name>.config.toml` and refuse `--profile pygg` while `config.toml` has a `[profiles.pygg]` table, so set-up refuses such a table.
- Set-up is also refused, before anything is made, for a file that is not valid TOML (the message names the line and quotes nothing), an inline `model_providers`, and an existing `pygg` provider or profile file.
- Removal deletes the profile file only while it still selects Pygg's provider. While `model_provider = "pygg"` is Codex's default, removal is refused. Apply then remove leaves `config.toml` byte-identical (a file without a final newline gets one).
- **`auto` follows the client's minimum tier** (T2 by default). With only cheaper models connected it refuses and says why: name the model, or lower the minimum in Endpoint → API Keys → Limits….
- **The model catalog** (`codex_catalog.rs`). For a model it does not know, Codex uses fallback metadata: its default instructions, a 272,000-token context and no `apply_patch`, so it edits through the shell and compacts at the wrong size. The `pygg` profile names a catalog, `pygg-models.json`, with every model Pygg lists for Codex (the auto aliases, `auto`, `auto/cheap`, `auto/plan`, `auto/code`, `auto/test` and `auto/review`, and the enabled models that take tools). Each keeps the fallback's values and Codex's own default instructions, and gains its real context window, the freeform `apply_patch` tool and its input kinds; `auto` keeps the 272,000 default, since its model is chosen per session. Pygg rewrites the file when the model list changes and adds it to a profile written before it existed; removal deletes it. A plain `codex` without the profile is untouched.
  - The instructions are Codex 0.163.0's `prompt.md`, Apache-2.0, kept byte for byte in `apps/desktop/src-tauri/third_party/codex/` and credited in [THIRD_PARTY_NOTICES](../THIRD_PARTY_NOTICES.md) (owner's decision, 2026-11-16). Codex requires instructions for every model in a catalog; these are the ones it uses for an unknown model, so nothing new is added.
  - A file, not an address the gateway serves: Codex refuses a fetched catalog over 1 MiB, and with about 21 KB of instructions per model the owner's 178 models make 3.9 MB.
  - `apply_patch` crosses the gateway as a function with one string parameter.
- **What does not cross:** OpenAI's encrypted reasoning, hosted tools (`web_search`), `prompt_cache_key` and `client_metadata`, named in `x-pygg-dropped`. Pygg stores no responses, so `previous_response_id` is refused; Codex sends the whole conversation with `store = false`.
- **Prompt cache.** A Codex turn with tools translated for Anthropic's own API, or for an `anthropic/…` model on OpenRouter, gets Anthropic's automatic caching (ADR-036). On OpenRouter and OpenAI the launch session goes on as their routing key, hashed, so turns reach the machine holding the cache.
- **Windows sandbox:** in a fresh `CODEX_HOME`, `-s workspace-write` falls back to read-only until `[windows] sandbox = "unelevated"` (or the user's own elevated setup) is set.

Verified: unit tests in `integrations.rs`; live with Codex CLI 0.163.0: set-up, an answer through the gateway on Claude Haiku 5.5, removal leaving `config.toml` byte-identical; tool round trips with `apply_patch` on Anthropic and Qwen connections; the prompt-cache run in [benchmark/reports/2026-11-14-codex-claude-prompt-cache.md](../benchmark/reports/2026-11-14-codex-claude-prompt-cache.md). The catalog: `pygg setup codex` in a scratch `CODEX_HOME` wrote it with 178 models, Codex 0.163.0 loaded all 178 with `apply_patch` and their context sizes (`codex debug models`), and a task on Qwen 3.8 Flash created its file with the patch tool at the first step, where without the catalog the model had tried `apply_patch` through the shell and been refused. `pygg run codex`: unit tests only.

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
- `models` lists the auto aliases (`auto`, `auto/cheap`, `auto/plan`, `auto/code`, `auto/test`, `auto/review`) and the enabled models of enabled connections, with context and output limits only where Pygg knows them. Models known not to take tools are left out. Set-up with no enabled model is refused.
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

- **The MCP server** (`mcp.rs`) runs over stdio. The app's own program serves it as `<app> mcp`; `pygg mcp` (and `pygg-mcp`) is the same server. Tools: `pygg_run_task`, `pygg_task_status`, `pygg_task_diff`, `pygg_keep_result`, `pygg_discard_result`, `pygg_stop_task`, `pygg_list_tasks`, `pygg_agents`, and for `pygg/brain` (ADR-041) `pygg_brainstorm` (a brainstorm's state, questions and cards) and `pygg_model_facts` (the connected models ranked for a role). The workspace is the folder the server runs in. Configuration: `PYGG_TOKEN`, and optionally `PYGG_URL` (default `http://127.0.0.1:8741`).

  ```text
  claude mcp add pygg -e PYGG_TOKEN=<key> -- "C:\path\to\Pygg.exe" mcp
  ```

- **One-click registration**: **Agents → Let Your Agent Delegate Tasks**, or `pygg mcp setup claude-code|codex|opencode`. Claude Code and Codex edit their own files through their own commands (`claude mcp add --scope user`, `codex mcp add`; removal `… mcp remove`). OpenCode's `opencode.json` is edited by Pygg. The entry is user-wide, never a project file, which would be committed with the token in it. The token may start tasks and reaches no model connection. An entry named `pygg` that Pygg did not make is left alone. **Set It Up by Hand** shows snippets for Cursor and other JSON-configured clients.
- **Waiting.** A tool call waits at most 50 s by default, under the 60 s many MCP clients allow; the agent then calls `pygg_task_status` again. Codex needs `tool_timeout_sec` raised to wait longer, and `default_tools_approval_mode = "approve"` for `codex exec`, which cannot ask before a tool call.
- **The task API** (`gateway/task_api.rs`): `GET /pygg/v1/agents`, `GET|POST /pygg/v1/tasks`, `GET /pygg/v1/tasks/{id}?wait=S`, `GET …/diff`, `POST …/stop|keep|discard`, and read-only `GET /pygg/v1/brainstorms[?session=S]`, `GET /pygg/v1/brainstorms/{id}` (a client's own) and `GET /pygg/v1/facts?role=R`. The MCP server holds no state; it forwards to this API.
- **A brainstorm starts as a request**, not a tool call: the agent asks for the model `pygg/brain` with the requirement (`gateway/brain.rs`, ADR-041). The reply names the brainstorm; the app is where it is picked, changed and approved into tasks, and nothing in the API or the MCP server approves one.
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

- Cancelling a single run on the Agents screen (tasks can be stopped).
