# Agents and coding tools

> **Status:** Current · coding tools through the gateway, agent CLIs, the MCP server and the task engine · reviewed 2026-11-14 · built: [Implementation Status](IMPLEMENTATION-STATUS.md)

A coding tool meets Pygg in one of two ways:

- **Through the gateway** (main path): set up once (CLI Tools, or `pygg setup`), start with `pygg run`.
- **By handing Pygg a task** over the MCP server (experimental, on hold): Pygg runs agent CLIs, each with its own sign-in, in a Git worktree; the workspace's approved checks judge the result.

Pygg never reads, stores or forwards an agent's own sign-in, and never bridges cookies or another product's OAuth tokens into an agent (ADR-026). Web-session connections (ADR-035) are gateway providers, unrelated to agent sign-in.

## Coding tools through the gateway

| Tool | Set up | Start | What Pygg changes |
| --- | --- | --- | --- |
| Claude Code | `pygg setup claude` | `pygg run claude` | No file. A Pygg API key "Claude Code (launcher)" in the OS credential store |
| Codex CLI | `pygg setup codex` | `pygg run codex` (adds `--profile pygg`) | A `[model_providers.pygg]` table in `config.toml` and a new `pygg.config.toml` |
| OpenCode | `pygg setup opencode` | `pygg run opencode`, then pick a `pygg/…` model with `/models` | A `pygg` provider in `opencode.json` |

In the app: **CLI Tools → One-Click Setup** (`integrations.rs`, `launcher.rs`). Every set-up:

- **Previews** the change; `pygg setup <tool> --dry-run` prints it and writes nothing.
- **Backs up, writes, checks**: backup in the app's `integration-backups` folder, a Pygg API key of its own (never printed). On failure the file is put back and the key revoked.
- **Remove** (`--remove`) backs up again, takes out only Pygg's part, revokes the key.
- The key is an ordinary client: reaches the connections, may not start tasks; limits in **Endpoint → API Keys → Limits…**.
- **Each launch is a routing session** (`launch-<uuid>`): the gateway keeps it on the model that answered while that model stays eligible (keeps the prompt cache).
- `pygg run` first checks the tool is set up, the gateway answers, and the tool is installed (PATH or `~/.local/bin`).
- Prints a **receipt** when the tool exits (below).

### Claude Code: the launcher

Pygg never changes Claude Code's settings (another endpoint would drop the user's Claude plan). `pygg run [--model M] claude [Claude Code's arguments…]` sets, for that process only:

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

- Removed first: `ANTHROPIC_API_KEY`, `CLAUDE_CODE_USE_BEDROCK`, `CLAUDE_CODE_USE_VERTEX`, a host session's variables.
- A plain `claude` keeps the user's sign-in and plan. **Remove** revokes the key and deletes it from the credential store.

Verified: unit tests (`launcher::tests`, `cli::run::tests`); live with Claude Code 2.1.322 up to the gateway (key and `auto` reached it; no provider connected → 422). Not yet verified: a session answered by a model through the launcher.

### The session receipt

Printed when a tool started by `pygg run` exits (`launches.rs`):

```text
Pygg · this launch of Claude Code (launch-…)
  Cost       $0.42 for 34 answered requests (2 on a subscription, not counted)
  Cache      91% of the prompt read from cache, 4% written to it
  Models     claude-haiku-5-5 ×30, glm-5 ×4 · 1 failed attempt, 1 answered by the next model
  Depth      a request cost $0.0040 at the start and $0.019 at the end (each turn sends the conversation again)
  Compared   $1.90 at anthropic/claude-sonnet-5-5 with the same tokens, against $0.42 (an estimate)
  Git        F:/work/app (main): 2 commits, 5 files changed since the start, +120 −14
```

| Line | What it shows |
| --- | --- |
| Cost | Ledger's known money for the session, failed attempts with known cost included. Unknown-cost and subscription requests counted beside it, never as $0 |
| Depth | From 20 priced requests: average of the first ten vs the last ten |
| Classes | Claude Code only: money and requests by `main`, `subagent`, `workflow`, `compaction`, `auxiliary` (titles, classifiers); subagent count and most at once. **Fan-out** when one prompt started 10+ subagents |
| Compared | When a comparison model is chosen (Usage → Savings); same estimate as there |
| Git | From the start folder: HEAD, branch, dirty tracked files before; after, commits since and `git diff --shortstat` from the start HEAD to the working tree. Already-uncommitted changes included and flagged |
| Limit | When the key has a session or day limit (Endpoint → API Keys → Limits…, or `pygg gateway client --session-usd N --day-usd N`): counted spend; once reached, further requests get 402 (day limit until 00:00 UTC). A running request is never cut; an estimated (not strict) limit can end slightly over |

- **Kept** in `launches` (schema v38): session id, tool, model, times, exit code, repo folder, branch, commit ids, the counts above; never file names or code. `pygg run` writes only if the table exists (the app migrates; `pygg run` never does), else the receipt says not saved. Rows follow the ledger's history setting.
- **By repository**: `pygg launches --by-repo [--weeks N]` (8 weeks default) per repo and UTC week (Mon–Sun): known cost (`+?` for unknown), commits, cost per commit, launches that changed nothing. Running or cut-off launches counted, not judged. Observations only, never productivity or quality (ADR-036).
- **Request classes**: with `CLAUDE_CODE_GATEWAY_HINT_HEADERS=1`, Claude Code 2.1.273+ sends each request's class and subagent (type, random id per spawn), 2.1.283+ also the prompt id. Kept with the attempt (schema v39, `gateway/hints.rs`), never forwarded to the provider. Unknown class → `other`; an id that is not short printable ASCII is dropped. Shown in Usage → Token Mix → By request kind and `pygg usage`. Codex and OpenCode send no such headers.
- **Status line**: `pygg run --statusline claude` passes `--settings` naming `pygg statusline` for that launch only. It prints e.g. `Pygg $0.42 · 91% cache · claude-haiku-5-5 · 1 fallback`, plus subagents; with a session limit `Pygg $0.42 of $5.00 · …`; once a limit is reached `Pygg $5.10 · session limit $5.00 reached · …`. Finds the launch from `x-pygg-session` in `ANTHROPIC_CUSTOM_HEADERS` or `PYGG_ATTEMPT_ID`; reads the store, never writes.

Verified: unit tests (`launches::tests` incl. Git in a scratch repo; `gateway::hints::tests`); a gateway test checks hint headers are recorded and none reaches the provider; browser preview Usage with demo data. Not yet run live with a real tool session.

### Codex

Codex CLI speaks only the Responses API; the gateway serves `POST /v1/responses` for every connection (`gateway/responses.rs`, ADR-033). Codex's ChatGPT sign-in is not involved.

`pygg setup codex` writes in `CODEX_HOME` (else `~/.codex`), via `toml_edit` so every other key, table, comment and line ending stays:

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

- **Codex's default `model` and `model_provider` never change.** Use `pygg run codex`, `codex --profile pygg`, or `codex --profile pygg -m <connection>/<model>`.
- **Refused before anything is made**: a `[profiles.pygg]` table (Codex 0.134+ reads profiles from `<name>.config.toml` and refuses `--profile pygg` while it exists), invalid TOML (message names the line, quotes nothing), inline `model_providers`, an existing `pygg` provider or profile file.
- **Removal** deletes the profile file only while it still selects Pygg; refused while `model_provider = "pygg"` is Codex's default. Apply then remove leaves `config.toml` byte-identical (a missing final newline is added).
- **`auto` follows the client's minimum tier** (T2 default). With only cheaper models it refuses and says why: name the model, or lower the minimum in Endpoint → API Keys → Limits….
- **Model catalog** (`codex_catalog.rs`), `pygg-models.json`: every model Pygg lists for Codex (`auto`, `auto/cheap`, `auto/plan`, `auto/code`, `auto/test`, `auto/review`, and enabled models that take tools). Without it Codex uses fallback metadata: 272,000-token context, no `apply_patch`. Each entry keeps the fallback values and Codex's default instructions, plus its real context window, freeform `apply_patch` and input kinds; `auto` keeps 272,000. Rewritten when the model list changes, added to older profiles, deleted on removal. Plain `codex` is untouched.
  - Instructions: Codex 0.163.0's `prompt.md`, Apache-2.0, byte for byte in `apps/desktop/src-tauri/third_party/codex/`, credited in [THIRD_PARTY_NOTICES](../THIRD_PARTY_NOTICES.md). Codex requires instructions per catalog model; these are what it uses for unknown models.
  - A file, not a gateway URL: Codex refuses a fetched catalog over 1 MiB; ~21 KB per model × 178 models = 3.9 MB.
  - `apply_patch` crosses the gateway as a function with one string parameter.
- **Dropped** (named in `x-pygg-dropped`): OpenAI encrypted reasoning, hosted tools (`web_search`), `prompt_cache_key`, `client_metadata`. No responses stored: `previous_response_id` is refused; Codex sends the whole conversation with `store = false`.
- **Prompt cache**: tool turns translated for Anthropic's API, or an `anthropic/…` model on OpenRouter, get Anthropic's automatic caching (ADR-036). On OpenRouter and OpenAI the launch session is sent, hashed, as their routing key.
- **Windows sandbox**: in a fresh `CODEX_HOME`, `-s workspace-write` falls back to read-only until `[windows] sandbox = "unelevated"` (or the user's elevated setup) is set.

Verified: unit tests in `integrations.rs`; live with Codex CLI 0.163.0: set-up, an answer on Claude Haiku 5.5, removal byte-identical; `apply_patch` round trips on Anthropic and Qwen; prompt cache in [benchmark/reports/2026-11-14-codex-claude-prompt-cache.md](../benchmark/reports/2026-11-14-codex-claude-prompt-cache.md). Catalog: 178 models written in a scratch `CODEX_HOME`, all loaded by Codex 0.163.0 with `apply_patch` and context sizes (`codex debug models`); a Qwen 3.8 Flash task used the patch tool at the first step (without the catalog, shell `apply_patch` was refused). `pygg run codex`: unit tests only.

### OpenCode

`pygg setup opencode` adds a `pygg` provider to `opencode.json`, keeping other keys in order; refuses non-plain JSON (comments).

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

- Anthropic format, served by the gateway for every connection.
- `models`: the auto aliases (`auto`, `auto/cheap`, `auto/plan`, `auto/code`, `auto/test`, `auto/review`) and enabled models of enabled connections; context/output limits only where known; models known not to take tools left out. No enabled model → set-up refused.
- Kept current at start-up, after a model list refresh, and on model or connection changes; only Pygg's `models`, only when it differs.
- Outside `pygg run` or a task step, `PYGG_ATTEMPT_ID` is empty and the gateway ignores the header.

Verified: unit tests in `integrations.rs` (key order, comment refusal, restore); live: OpenCode reached the gateway through the provider. `pygg run opencode`: unit tests only.

### Other clients

Claude Code without the launcher, Cline, Continue, SDKs, curl: set up by hand from the snippets in CLI Tools. See [GETTING-STARTED](GETTING-STARTED.md).

## Agent CLIs with their own sign-in

Code: `agents.rs`. App: **Agents** (agents found, one-off runs). Headless: `pygg agent` (`cargo run --bin agent` in development).

| Agent | Detected as | Headless command (in the worktree) | Usage read from |
| --- | --- | --- | --- |
| Claude Code | `claude` on PATH or in `~/.local/bin` | `claude -p <task> --output-format json --permission-mode acceptEdits --allowedTools Read,Write,Edit,Glob,Grep[,Bash]` | result JSON: `usage`, `total_cost_usd`, `modelUsage`, `result` |
| Codex CLI | `codex` (also `codex login status`) | `codex exec --json --sandbox workspace-write --ephemeral [--model M] <task>` | JSON lines: `turn.completed.usage`, final `agent_message` |
| Gemini CLI | `gemini` (npm shim → `node …/gemini.js`) | `gemini -p <task> --output-format json --approval-mode auto_edit\|yolo`, with `GEMINI_CLI_TRUST_WORKSPACE=true` | `stats.models.*.tokens`, `response` |
| OpenCode | `opencode` (npm shim → `opencode.exe`) | `opencode run --format json <task> [--model provider/model]` | `step_finish` events |

- **Sign-in stays with the agent** (Claude plan, ChatGPT plan, the agent's API key, OpenCode's providers). Pygg shows only the CLI's own status output, key-shaped text redacted.
- **One worktree per run** (ADR-008): detached, at the current commit; user files and branch untouched. Result kept as a new branch or discarded; worktree removed either way.
- **Least permission**: Claude Code gets file tools only; Gemini `auto_edit`. Shell needs **Allow** (Claude's Bash, Gemini's `yolo`). Codex always in its `workspace-write` sandbox. Time limit 15 min by default; the whole process tree is ended on timeout or cancel (`process.rs`).
- **Honest cost**: tokens as the agent reports them; subscription cost is **unknown**. An agent's list-price estimate (Claude Code `total_cost_usd`, OpenCode step costs) goes into the cost's source, not spend.
- **Windows**: npm `.cmd` shims are bypassed; Pygg starts the target `.exe`, or the `.js` through Node, since free text cannot be quoted safely through `cmd`.

Verified: live with Codex CLI 0.163.0 (ChatGPT plan) and OpenCode 1.18.4 (free model); Claude Code 2.1.322 through the MCP server. Gemini CLI 0.66.0 with a Google account is refused by Google ("no longer supported for Gemini Code Assist for individuals"); a Gemini API key is untested.

## The MCP server and the task API (experimental, on hold)

Tasks start from the user's coding agent (ADR-027); the app runs them, shows them, approves paid steps and keeps history.

- **MCP server** (`mcp.rs`), stdio. Served as `<app> mcp`; `pygg mcp` and `pygg-mcp` are the same server. Workspace = the folder it runs in. Config: `PYGG_TOKEN`, optional `PYGG_URL` (default `http://127.0.0.1:8741`). Holds no state; forwards to the task API.

  ```text
  claude mcp add pygg -e PYGG_TOKEN=<key> -- "C:\path\to\Pygg.exe" mcp
  ```

| Tool | Does |
| --- | --- |
| `pygg_run_task`, `pygg_task_status`, `pygg_task_diff`, `pygg_keep_result`, `pygg_discard_result`, `pygg_stop_task`, `pygg_list_tasks`, `pygg_agents` | Tasks and agents |
| `pygg_brainstorm` | A brainstorm's state, questions and cards (ADR-041, ADR-042) |
| `pygg_model_facts` | Connected models ranked for a role |
| `pygg_board` | The workspace's board: cards by lane with their tasks |
| `pygg_board_move` | A card between Discovery and Ready only; approves nothing |

- **Claude Code plugin** (`plugins/pygg`, listed in `.claude-plugin/marketplace.json`): this MCP server plus skills `delegate-tasks` and `route-through-pygg`. On enable it asks for the token (kept by Claude Code as a secret) and the gateway address. `scripts/mcp.mjs` starts the server without a shell: the app's `pygg-mcp.exe` (Windows, per-user install), else `pygg` on PATH (npm), else `npx @pygg/cli@dev`. MCP Registry entry: `packages/npm/pygg/server.json` (`io.github.nambean/pygg`, the npm package, its `mcpName`), published with the npm release ([PUBLISHING](../packages/npm/PUBLISHING.md)). Verified on Claude Code 2.1.322 with a scratch config folder: validated, installed from the local marketplace, `claude mcp list` connected via the app and via `npx`.
- **One-click registration**: **Agents → Let Your Agent Delegate Tasks**, or `pygg mcp setup claude-code|codex|opencode`. Claude Code and Codex use their own commands (`claude mcp add --scope user`, `codex mcp add`; `… mcp remove`); Pygg edits OpenCode's `opencode.json`. Always user-wide, never a project file (the token would be committed). The token may start tasks and reaches no model connection. A `pygg` entry Pygg did not make is left alone. **Set It Up by Hand**: snippets for Cursor and other JSON clients.
- **Waiting**: a tool call waits at most 50 s by default (many MCP clients allow 60 s), then the agent calls `pygg_task_status` again. Codex: raise `tool_timeout_sec` to wait longer; `codex exec` needs `default_tools_approval_mode = "approve"`.
- **Task API** (`gateway/task_api.rs`):

| Method and path | Use |
| --- | --- |
| `GET /pygg/v1/agents` | Agents found |
| `GET\|POST /pygg/v1/tasks` | List or start tasks |
| `GET /pygg/v1/tasks/{id}?wait=S` | Status, waiting up to S seconds |
| `GET /pygg/v1/tasks/{id}/diff` | The result's diff |
| `POST /pygg/v1/tasks/{id}/stop\|keep\|discard` | Stop, keep as branch, discard |
| `GET /pygg/v1/brainstorms[?session=S]`, `GET /pygg/v1/brainstorms/{id}` | Read-only; a client's own brainstorms |
| `GET /pygg/v1/facts?role=R` | Read-only model facts for a role |
| `GET /pygg/v1/board?workspace=W` | Read-only board |
| `POST /pygg/v1/board/{card}/move` | Body `{"to": "discovery" \| "ready"}` |

- **A brainstorm starts as a request**: the agent asks for model `pygg/brain` with the requirement (`gateway/brain.rs`, ADR-041). The reply names the brainstorm; it is picked, changed and approved into tasks only in the app, never via the API or MCP server.
- **Permission**: a key starts tasks only after **May start tasks** is on (Endpoint → API Keys → Limits…; `pygg gateway client --name N --tasks yes`). A client sees and acts on only its own tasks. **There is no API to approve a paid step.**
- **Where tasks run**: in the app. `pygg gateway serve` answers the task API with 503 unless started with `--tasks yes` (for a computer where the app is not open on that store).

Verified: integration test `integrations::tests::live` with Claude Code 2.1.322, Codex CLI 0.163.0, OpenCode 1.18.4 on scratch config folders (set up, listed, second set-up refused, removed, token revoked); live delegation from Codex CLI 0.163.0 (`codex exec`) and Claude Code 2.1.322 (`claude -p`) through `pygg mcp`: task run, check passed, diff read, kept as a branch. Not yet clicked through in the app.

## The task engine (experimental, on hold)

`tasks.rs`. A task is a ladder of agent steps fixed at creation (`tasks.policy_snapshot_json`), judged by the workspace's **approved** checks.

1. Each attempt runs one step (agent, model, tier, billing) in a fresh worktree of the task's **base commit**. `PYGG_ATTEMPT_ID` = the attempt id; an agent on the gateway sends it as its session header, tying ledger entries to the attempt.
2. Approved checks run in order, stop at the first failure; pass = exit code 0.
3. A failed attempt is discarded; the next step gets the handoff mode (ADR-030):
   - `tail` (default): the failed check's command and the last 1,500 characters of its redacted output.
   - `none`: the task alone.
   - `evidence`: all earlier attempts via `handoff.rs` (cleaned check output, changed paths, the agent's final message marked unverified), ~6,000 characters max. Saved no tokens, time or money in [its A/B](../benchmark/reports/2026-11-12-h1-handoff-ab.md).

   No change = failure. A step whose agent and model timed out on their last two runs in the past 30 minutes with no output token is skipped, unless it is the last step.
4. Checks pass → `checks_passed`, waiting for **Keep as Branch**, **Export Patch** or **Discard**. No approved checks, or a change to a check-defining file (`package.json`, `Cargo.toml`) → `needs_review`. Keeping a reviewed result → `manually_accepted`.
5. **Paid step waits for approval** (ADR-010), shown with the cost range of the last 10 runs of that agent and model, and needs a spending cap. Claude Code gets it as `--max-budget-usd`; others are checked afterwards, and over the cap the ladder stops (`blocked`). Cost = Pygg's ledger when requests carried the attempt id, else the agent's estimate; an unpriced request makes it unknown, never $0. A changed estimate or budget voids the approval. Declining blocks the task.
6. **Daily budgets** (UTC): all tasks, and per workspace. A running paid step holds its cap; free, local and subscription steps do not count.
7. **Stop** ends the agent or check with its process tree. At start-up a `running` task becomes `interrupted` (Retry reruns that step); leftover worktrees are removed.

Workspaces: Git repositories only, managed on the Workspaces screen (checks, privacy, budget). A **sensitive** workspace asks before every step. Detected checks (`package.json` scripts, `cargo check`, `cargo test`) are suggestions until approved. Keep checks inside the repository: an agent may refuse to read outside its worktree.

Headless:

```text
pygg agent detect
pygg agent [--db STORE] run --agent ID --workspace DIR --prompt TEXT [--model M] [--shell yes] [--timeout SECS] [--keep TITLE | --discard yes]
pygg agent --db STORE task --workspace DIR --prompt TEXT --steps "opencode:opencode/some-free-model,codex" [--check CMD] [--keep TITLE | --discard yes]
pygg agent --db STORE quality [--days N]
```

Verified: unit tests in `tasks.rs` with a scripted agent in real worktrees (approvals, stop, interruption, retry); live cascades OpenCode 1.18.4 (free model) → Codex CLI 0.163.0; a paid step approved live in the installed app (OpenCode through the gateway on Claude Haiku 5.5, cost from Pygg's ledger).

## Not built yet

- Cancelling a single run on the Agents screen (tasks can be stopped).
