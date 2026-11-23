# Checks gate readiness, 2026-11-23

> **Status:** Record · run 2026-11-23 (UTC+7) by the maintainer · ROADMAP focus item 2 · $0: the task engine with fake agents · raw results: [`results/2026-11-23-checks-gate`](../results/2026-11-23-checks-gate/)

**In one line:** all 50 results that no check ran on stayed not ready: a plain keep was refused, the result stayed open, and what an IDE agent reads said NOT READY. All 30 results that checks ran on showed each check's exact command and exit.

## What changed

- A task's `checked` verdict tells four cases apart: `passed`, `failed`, `not_checked` (no check ran) and `definitions_changed` (the checks passed, but the result changed a file that defines them).
- Only `passed` is ready. A `not_checked` or `definitions_changed` result is kept only with the user's word that they judged it themselves. In the app that is **Keep Without Checks…** and a confirmation. Elsewhere it is `acceptUnchecked` in the task API, `accept_unchecked` in MCP, and `--unchecked yes` in the CLI. A plain keep is refused with the reason.
- `pygg_task_status` opens such a result with NOT READY and lists every check as `` check `command`: passed|FAILED (exited N, T s) ``. The evidence card shows the same exit and time.
- The audit log records whether a kept result was unchecked.
- Why: in the [evidence-card proof](2026-11-23-smart-task-proofs.md#2-evidence-card), model reviewers waved "no check ran" through at least 6 times in 10, even with the card.

## Method

`checks_gate_proof` in `apps/desktop/src-tauri/src/tasks.rs` runs 80 tasks through the engine with fake agents and real Git worktrees. For each, it reads the task as the API returns it and runs MCP's `describe` on it.

```text
cargo test --release --lib checks_gate_proof -- --ignored --nocapture   (PYGG_PROOF_OUT=FILE for one row per task)
```

| Kind | Tasks | Set-up | Expected |
| --- | --- | --- | --- |
| No check ran | 50 | Workspace with no approved check. The agent edits one of five files; every fifth task gets there on its second step | `not_checked`; plain keep refused; still open; NOT READY shown |
| Check passed | 10 | Check `findstr /c:good result.txt`; the agent writes `good` | `passed`; ready; the command and `exited 0` shown |
| Check failed | 10 | The same check; the agent writes `bad` | `failed`; the command and `exited 1` shown |
| Definition changed | 10 | Check `exit 0` defined by `package.json`; the agent edits `package.json` | `definitions_changed`; plain keep refused; NOT READY shown |

## Results

| Kind | Tasks | As expected |
| --- | --- | --- |
| No check ran | 50 | **50** |
| Check passed | 10 | 10 |
| Check failed | 10 | 10 |
| Definition changed | 10 | 10 |

- **Not measured:** whether people or model reviewers now keep fewer unchecked results. The gate stops a keep from passing silently. It does not judge the code.
- **Next (focus item 3):** help a workspace with no checks get one, since every one of its results is now not ready.
