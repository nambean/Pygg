# Agents: official agent CLIs with their own sign-in

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
| Claude Code | — | Not on PATH on this machine (only inside the Claude desktop app); not tested here |

Every run, including the failed ones, has a ledger entry under `agent:<id>`.

## Smart tasks: task engine v1 (`tasks.rs`, schema v8)

A task is a ladder of agent steps fixed when it is created (`tasks.policy_snapshot_json`), run against the workspace's **approved** checks:

1. Each attempt runs one step (agent, model, the user's tier and billing classing) in a fresh worktree of the task's **base commit**, recorded at creation, so every attempt starts from the same code.
2. The approved checks run in order in that worktree and stop at the first failure. Passing means exit code 0.
3. If an attempt fails its checks, its worktree is discarded and the next step starts. The new step's prompt gets the failed command and the last 1,500 characters of its (redacted) output. If the agent itself failed (exit, timeout, could not start), the next step gets the task alone. An attempt that changed nothing counts as a failure.
4. When the checks pass, the task is `checks_passed` and the worktree waits for **Keep as Branch** or **Discard**. With no approved checks, the task is `needs_review`: nothing confirmed it. If the attempt changed the file that defines a check (`package.json`, `Cargo.toml`), the task is also `needs_review`, because the checks could pass without the task being done. Keeping a reviewed result makes it `manually_accepted`.
5. A step marked **paid** waits for approval before it runs (an `approvals` row; agents report no cost before they run, so the approval has no cap). Declining blocks the task.
6. **Stop** ends the running agent or check, including its whole process tree. At start-up, a task left `running` becomes `interrupted` (Retry runs that step again), and leftover worktrees in the app's folder are removed.

Workspaces must be Git repositories. Detected checks (`package.json` scripts, preceded by the package manager's install, and `cargo check` / `cargo test`) are suggestions until the user approves them, and checks run the code the agent wrote, on this computer. Each attempt's ledger entry carries `usage_events.attempt_id`, so a task's cost is the sum of its own entries (unknown for agents).

Headless: `cargo run --bin agent -- --db STORE task --workspace DIR --prompt TEXT --steps "opencode:opencode/some-free-model,codex" --check "CMD" [--keep TITLE | --discard yes]`.

### Live verification, 2026-11-11

A scratch repository with `add()` returning `a - b`, the check `python test_calc.py`, and two steps: OpenCode with `opencode/deepseek-v4.1-flash-free`, then Codex. Step 1 fixed it (+1 −1, 9,781 input / 356 output tokens), the check passed in its worktree in 157 ms, and the task ended `checks_passed` without reaching Codex. The cascade itself (a failed check handing off to the next step, approvals, stop, interruption and retry) is covered by `tasks.rs` tests with a scripted agent in real worktrees, not live.

## Not built yet

- Starting a task from where the user already works (an IDE agent, a terminal), rather than from the app. The headless `agent task` is the first step.
- Cancelling a single run on the Agents screen (tasks can be stopped).
- A folder picker for the repository (the path is typed or pasted).
- A daily budget for tasks, and a cost estimate on the approval for a paid step.
