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

## Not built yet

- Cancelling a run from the app (the time limit applies).
- Deterministic checks after a run (checks.rs exists; wiring it here is next), and a cascade across agents or models.
- A folder picker for the repository (the path is typed or pasted).
- Persisting runs across an app restart: an open run's worktree survives, but the app forgets it. `git worktree prune` cleans up.
