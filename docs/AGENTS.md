# Agents: official agent CLIs with their own sign-in

> **Status:** Current · agent CLIs, the task engine and the MCP server · reviewed 2026-11-11 · order: [ROADMAP](ROADMAP.md) · built: [Implementation Status](IMPLEMENTATION-STATUS.md)

**Decision (owner, 2026-11-11):** to let people use the subscriptions and free tiers they already have, Pygg runs the vendors' own agent CLIs as executors. It does not bridge web sessions, cookies or another product's OAuth tokens (see [OMNIROUTE_FEATURE_RESEARCH.md](OMNIROUTE_FEATURE_RESEARCH.md#provider-reference-review-2026-11-11)). Code: `apps/desktop/src-tauri/src/agents.rs`. The app's screen is Agents; the headless CLI is `cargo run --bin agent`.

## Contract

- **Sign-in stays with the agent.** Pygg starts the agent's own program, which uses whatever the user signed it in with: a Claude plan, a ChatGPT plan, an API key the agent manages, or OpenCode's providers. Pygg never reads, stores or forwards that sign-in, and never exposes the agent to other clients through the gateway. The only sign-in information Pygg shows is what the CLI's own status command prints (`codex login status`), with anything key-shaped redacted.
- **One worktree per task** (ADR-008). The agent works on a detached Git worktree of the repository's current commit. The user's files and branch are not touched. Afterwards the user keeps the result as a new branch or discards it, and the worktree is removed either way.
- **Least permission by default.** Claude Code gets `--permission-mode acceptEdits` with only file tools (Read, Write, Edit, Glob, Grep). Gemini CLI gets `--approval-mode auto_edit`. Shell commands need **Allow** (Claude's Bash tool, Gemini's yolo mode). Codex always runs in its own `workspace-write` sandbox. Each run has a time limit (15 minutes) and the whole process tree is ended on timeout or cancel (process.rs).
- **Honest cost.** Tokens are recorded as the agent reports them. The money depends on how the agent is signed in, and a subscription is not billed per request, so the ledger records the cost as **unknown**. Where the agent gives a list-price estimate (Claude Code's `total_cost_usd`, OpenCode's step costs), that estimate goes into the cost's source, not into spend.

## Agents

| Agent | Detected as | Headless command (in the worktree) | Usage read from |
| --- | --- | --- | --- |
| Claude Code | `claude` on PATH | `claude -p <task> --output-format json --permission-mode acceptEdits --allowedTools Read,Write,Edit,Glob,Grep[,Bash]` | result JSON: `usage`, `total_cost_usd`, `modelUsage`, `result` |
| Codex CLI | `codex` (also `codex login status`) | `codex exec --json --sandbox workspace-write --ephemeral [--model M] <task>` | JSON lines: `turn.completed.usage`, final `agent_message` |
| Gemini CLI | `gemini` (npm shim → `node …/gemini.js`) | `gemini -p <task> --output-format json --approval-mode auto_edit|yolo`, with `GEMINI_CLI_TRUST_WORKSPACE=true` for the task's worktree | `stats.models.*.tokens`, `response` |
| OpenCode | `opencode` (npm shim → `opencode.exe`) | `opencode run --format json <task> [--model provider/model]` | `step_finish` events (as in the benchmark harness) |

On Windows, npm installs `.cmd` shims. Pygg starts their target (an `.exe`, or a `.js` through Node) directly, because a free-text task cannot be quoted safely through `cmd`.

## Live verification, 2026-11-11

A scratch repository with `add()` returning `a - b`; the task was to make it `a + b` and change nothing else.

| Agent | Sign-in | Result |
| --- | --- | --- |
| Codex CLI 0.163.0 | ChatGPT plan (`Logged in using ChatGPT`) | Fixed in 28.5 s, diff +1 −1. 83,166 input tokens (65,664 from cache), 291 output. Recorded with an unknown cost |
| OpenCode 1.18.4 | its free model `opencode/deepseek-v4.1-flash-free` | Fixed in 25.4 s, diff +1 −1, agent's estimate $0.0000 |
| OpenCode 1.18.4 | its default model | Failed: the model's server answered "Unexpected server error". Recorded as an error |
| Gemini CLI 0.66.0 | Google account | Failed. Google answered "This client is no longer supported for Gemini Code Assist for individuals" and points to Antigravity. A Gemini API key set in Gemini CLI is the remaining way; not tested. The headless trust prompt is handled |
| Claude Code | — | Not on PATH at the time; tested later from its native install in `~/.local/bin` (2.1.322, Team plan), see From the IDE below |

Every run, including the failed ones, has a ledger entry under `agent:<id>`.

## Smart tasks: task engine v1 (`tasks.rs`, schema v8)

A task is a ladder of agent steps fixed when it is created (`tasks.policy_snapshot_json`), run against the workspace's **approved** checks:

1. Each attempt runs one step (agent, model, the user's tier and billing classing) in a fresh worktree of the task's **base commit**, recorded at creation, so every attempt starts from the same code.
2. The approved checks run in order in that worktree and stop at the first failure. Passing means exit code 0.
3. If an attempt fails its checks, its worktree is discarded and the next step starts. The new step's prompt gets the failed command and the last 1,500 characters of its (redacted) output. If the agent itself failed (exit, timeout, could not start), the next step gets the task alone. An attempt that changed nothing counts as a failure. A step whose agent and model timed out on their last two runs, both in the past 30 minutes and without one output token, is **skipped** and recorded as such, unless it is the last step left: its service is most likely not answering, and waiting out the time limit again only delays the next step (found by the N2 benchmark, when OpenCode's free model stopped answering).

   **What the next step is told** is the task's handoff mode (ADR-030), fixed in its policy:
   - **`tail`** (the default): the failed check's command and the end of its output.
   - **`none`**: the task alone.
   - **`evidence`** (H1): every earlier attempt, built by `handoff.rs` from what Pygg recorded. That covers each failing check's output (colour codes and redrawn progress lines removed, repeats named rather than copied), the paths a discarded attempt changed, and the agent's own final message, marked unverified. It is capped at about 6,000 characters, the oldest attempts are dropped first, and the drop is said.

   The text sent is stored on the attempt (`attempts.handoff_sent`) and shown in task detail and `pygg_task_status`. Which files an agent read is not reported by the CLIs, so it is never claimed. `evidence` becomes the default only if its A/B says so.
4. When the checks pass, the task is `checks_passed` and the worktree waits for **Keep as Branch** or **Discard**; **Export Patch** saves it as a file `git apply` takes, and leaves it waiting. With no approved checks, the task is `needs_review`: nothing confirmed it. If the attempt changed the file that defines a check (`package.json`, `Cargo.toml`), the task is also `needs_review`, because the checks could pass without the task being done. Keeping a reviewed result makes it `manually_accepted`.
5. A step marked **paid** waits for approval before it runs (ADR-010; schema v10). The approval shows the range of the agent's own list-price estimates over its last 10 runs of that model, or says there is none, and approving needs a **spending cap**. Claude Code is given the cap as `--max-budget-usd` and stops there itself. Other agents cannot stop at a cap, so their reported cost is checked afterwards: over the cap, the ladder stops (`blocked`) instead of trying another step, and when they report no cost the task says the cap could not be checked. If the estimate or a budget changed before the user decided, the approval is invalidated and a new one is asked. There is no API to approve (ADR-027). Declining blocks the task.
6. **Daily budgets** (schema v11) for paid steps: one for all tasks, and one per workspace on top. A finished paid step counts at the agent's own estimate, or at its whole cap when it reported none; a running one holds its cap; free, local and subscription steps do not count. When today's budget is used, a paid step is blocked before it asks; an approval's cap cannot be more than what is left. Days are UTC. Declining blocks the task.
7. **Stop** ends the running agent or check, including its whole process tree. At start-up, a task left `running` becomes `interrupted` (Retry runs that step again), and leftover worktrees in the app's folder are removed.

Every agent run of a task gets `PYGG_ATTEMPT_ID`, the attempt's id, in its environment. An agent the user has pointed at the gateway can send it as its session header (`x-smarttaskrouter-session`), and the gateway's ledger then says which attempt each request belonged to; the benchmark costs attempts this way (`bench --engine`).

Workspaces must be Git repositories, and are managed on the Workspaces screen (checks, privacy, budget). In a **sensitive** workspace every step, free ones too, asks before it runs, since Pygg cannot yet check where an agent sends the code; such an approval needs no cap unless the step is paid. Detected checks (`package.json` scripts, preceded by the package manager's install, and `cargo check` / `cargo test`) are suggestions until the user approves them, and checks run the code the agent wrote, on this computer. Each attempt's ledger entry carries `usage_events.attempt_id`, so a task's cost is the sum of its own entries (unknown for agents).

Headless: `cargo run --bin agent -- --db STORE task --workspace DIR --prompt TEXT --steps "opencode:opencode/some-free-model,codex" --check "CMD" [--keep TITLE | --discard yes]`.

### Live verification, 2026-11-11

A scratch repository with `add()` returning `a - b`, the check `python test_calc.py`, and two steps: OpenCode with `opencode/deepseek-v4.1-flash-free`, then Codex. Step 1 fixed it (+1 −1, 9,781 input / 356 output tokens), the check passed in its worktree in 157 ms, and the task ended `checks_passed` without reaching Codex. Approvals, stop, interruption and retry are covered by `tasks.rs` tests with a scripted agent in real worktrees, not live.

**The cascade, live (ROADMAP C2).** A scratch repository with `slugify()` that neither lower-cases nor cleans punctuation, and an approved check kept outside the repository, so no agent could read it: three cases, `"Hello World"` → `hello-world`, extra spaces collapsed, punctuation dropped. The prompt named only the lower-casing. Steps: OpenCode with `opencode/deepseek-v4.1-flash-free`, then Codex (ChatGPT plan).

| Run | Step 1, OpenCode | Check | Step 2, Codex | Check |
| --- | --- | --- | --- | --- |
| 1 | added `.lower()` (+1 −1, 25,174 / 308 tokens) | exited 1, "1 of 3 passed" | handed the command and its two `FAIL` lines; one-line rewrite (84,124 / 512 tokens) | passed, `checks_passed` |
| 2 | the same (12,464 / 355) | the same | a `re.sub` rewrite (+2 −1, 91,701 / 747) | passed; discarded, no worktree left |

Codex's own sandbox could not run `python`, so it could not test its change; the check Pygg ran is what passed it. No money was spent: a free model and a subscription. The headless CLI now prints the end of a failed check's output, which is what the next step is handed.

## From the IDE: `pygg-mcp` and the task API

**Decision (owner, 2026-11-11):** most people will not treat the router app as an IDE. Tasks are started where people already work, by their coding agent; the app runs them and is where they are watched, paid steps are approved, and history is kept. New Task in the app stays as a simple way to start one by hand.

- **The MCP server** (`src/mcp.rs`) runs over stdio. The installed app's own program serves it when started as `<app> mcp`, so the installer ships nothing extra; `pygg-mcp` is the same server as its own program, for development and headless machines. Tools: `pygg_run_task` (prompt, optional title, steps or strategy, shell, wait), `pygg_task_status` (waits while running), `pygg_task_diff`, `pygg_keep_result`, `pygg_discard_result`, `pygg_stop_task`, `pygg_list_tasks`, `pygg_agents`. The workspace defaults to the folder the server runs in, which the IDE sets to the project. Configuration: `PYGG_TOKEN`, and optionally `PYGG_URL` (default `http://127.0.0.1:8741`). Example: `claude mcp add pygg -e PYGG_TOKEN=… -- "C:\…\Pygg.exe" mcp`. Agents → From Your IDE shows this for Claude Code, Codex CLI (`config.toml`) and JSON-configured IDEs, with the app's real path.
- **One-click setup** (ROADMAP D2, ADR-022; `src/integrations.rs`, 2026-11-12). Agents → From Your IDE lists Claude Code, Codex CLI and OpenCode with whether Pygg's server is registered in each. **Set Up…** shows what will change before anything does. Then Pygg backs up the agent's configuration file into the app's `integration-backups` folder, creates a client token that may start tasks and reaches no model connection (`<Agent> (MCP)`), registers the server and checks the agent now lists it; if any step fails, the file is put back and the token revoked. Claude Code and Codex edit their own files through their own commands (`claude mcp add --scope user`, `codex mcp add`; removal is `… mcp remove`), so their format and comments stay theirs. OpenCode has no such command that runs without asking, so Pygg edits `opencode.json` itself, keeping every other key in its order, and refuses a file that is not plain JSON (comments). The entry is user-wide, never a project file, because a project file would be committed with the token in it; the server takes the project from the folder the agent runs in. **Remove…** removes the entry (after another backup) and revokes the token. An entry named `pygg` that Pygg did not make is shown as set up by hand and left alone unless the user removes it. The agents' own sign-in is never read (ADR-026). The manual snippets stay under **Set It Up by Hand**, for Cursor and other JSON-configured clients.
- **Waiting:** a tool call waits at most 50 s by default, under the 60 s many MCP clients allow (Codex's `tool_timeout_sec`); the agent then calls `pygg_task_status` again. Codex needs `tool_timeout_sec` raised to wait longer, and `default_tools_approval_mode = "approve"` for `codex exec`, which cannot ask before a tool call.
- **The task API** (`gateway/task_api.rs`) on the local gateway: `GET /pygg/v1/agents`, `GET|POST /pygg/v1/tasks`, `GET /pygg/v1/tasks/{id}?wait=S`, `GET …/diff`, `POST …/stop|keep|discard`. `pygg-mcp` holds no state; it forwards to this API.
- **Permission:** a client token may start tasks only after the user turns on **May start tasks** (Clients → Limits…; `gateway client --name N --tasks yes`). A client sees and acts on only the tasks it started. **There is no API to approve a paid step**: that stays with the user in the app.
- **Workspaces:** the folder is resolved to its repository root, which becomes a workspace if it is not one. Checks found there are suggestions until the user approves them, so until then a result is `needs_review`, never `checks_passed`.
- **Where tasks run:** in the app. `gateway serve` answers the task API with 503 unless started with `--tasks yes`, for a computer where the app is not open on that store. The app does not mark tasks interrupted at start-up while another process holds the store.

### Live verification, 2026-11-11

`gateway serve --tasks yes` on a scratch store, a client allowed to start tasks, and `pygg-mcp` driven over stdio from inside a scratch repository (`add()` returning `a - b`, approved check `python test_calc.py`). `pygg_run_task` with OpenCode's free model, then Codex: step 1 fixed it and the check passed (`checks_passed`); `pygg_task_diff` returned the one-line change; `pygg_keep_result` created `str/fix-add-ae739de9` with the checkout unchanged.

**From a real Codex session.** `codex exec` (Codex CLI 0.163.0, ChatGPT plan, read-only sandbox so it could not edit files itself) with the server given by one-off `-c mcp_servers.pygg.*` overrides, so `~/.codex/config.toml` was untouched. Asked to delegate a fix of `area()` (returning `w + h`), Codex called `pygg_run_task`; OpenCode's free model changed it to `w * h`, the approved check `python test_shapes.py` passed, and Codex called `pygg_task_diff` and `pygg_keep_result`: branch `str/fix-area-5855f35a`, checkout unchanged. The first attempt failed because `codex exec` refuses an MCP tool call that needs approval; `default_tools_approval_mode = "approve"` fixed it. **From a real Claude Code session.** Claude Code 2.1.322 (Team plan), installed by its native installer into `~/.local/bin` (not on PATH; Pygg looks there too). `claude -p` with a one-off `--mcp-config` and `--strict-mcp-config`, so the user's Claude Code configuration was untouched, and only Pygg's tools allowed. Asked to delegate a fix of `to_fahrenheit()` (`- 32` instead of `+ 32`), Claude Code called `pygg_run_task`; OpenCode's free model fixed it, the approved check `python test_temps.py` passed, and Claude Code called `pygg_task_diff` and `pygg_keep_result`: branch `str/fix-fahrenheit-9f15e982`, checkout unchanged. 5 turns; Claude Code's own list-price estimate for its session, $0.087, is covered by the subscription.

### One-click setup, live, 2026-11-12

`cargo test --lib integrations::tests::live -- --ignored` with the agents installed here (Claude Code 2.1.322, Codex CLI 0.163.0, OpenCode 1.18.4), each pointed at a scratch configuration folder through its own variable (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `XDG_CONFIG_HOME`), so the owner's own configuration was not touched (checked before and after). For each: set up, the agent listed the server (`claude mcp get pygg` reported user scope; Codex's `mcp get` the command), a second setup was refused, Codex's own comment in `config.toml` was kept, removal left no entry and no live token. Claude Code's health check said "Failed to connect" because the test's server program was the test binary, not the app; the registration itself is what was checked. Not yet clicked through in the native app. Unit tests cover the arguments, the placeholder in what is shown, OpenCode's key order and refusal of comments, and putting a file back.

## Not built yet

- Claude Code's interactive terminal session through the gateway (headless runs pass).
- Cancelling a single run on the Agents screen (tasks can be stopped).
