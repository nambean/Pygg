# Phase 0 benchmark

Tests the product's core idea (ADR-013): does a cascade that climbs model tiers on check failure reach a comparable pass rate at a lower cost per success than always using one model?

## Layout

```text
benchmark/
  cases/<id>/case.json    task definition
  cases/<id>/repo/        fixture files; the harness turns them into a fresh Git repository per run
  runs/                   raw results, one JSON line per case and run (git-ignored)
  reports/                written-up results (committed)
```

`case.json`:

```json
{
  "id": "js-sum-sign-bug",
  "title": "Fix the sign bug in sum",
  "category": "bug_fix",
  "complexity": "low",
  "prompt": "What the agent is asked to do.",
  "checks": ["node test.js"]
}
```

Every case needs at least one deterministic check. A check passes only if it exits 0, and all of a case's checks must pass. Categories in use: `bug_fix`, `feature`, `refactor`, `test_writing`, `docs`. Complexity is `low`, `medium` or `high`.

The fixtures need only Node.js, so they run anywhere the app is developed.

## Running

The harness is the `bench` binary of the desktop crate. Run it from `apps/desktop/src-tauri`:

```powershell
cargo run --release --bin bench -- `
  --cases ../../../benchmark/cases `
  --out ../../../benchmark/runs `
  --agent '<agent command>' `
  --rung T0=<model> --rung T2=<model> --rung T4=<model> `
  --mode all
```

- `--agent` is a command line run through the shell inside each attempt's worktree. It can use three placeholders: `{prompt_file}` (the task text, kept outside the worktree), `{model}` and `{workdir}`.
- `--mode single` runs each rung alone (the baselines), `--mode cascade` climbs the rungs in tier order, and `--mode all` does both.
- `--only <id>` runs one case.
- `--agent-timeout` and `--check-timeout` set limits in seconds.

Each attempt runs in its own Git worktree and is discarded afterwards. Checks run even when the agent exits with an error.

### OpenCode

OpenCode 1.18.4 has a non-interactive `run` command with `-m provider/model`, `-f` to attach files, `--dir`, `--format json` and `--auto`. A candidate template, **not yet tried against a real model**:

```text
opencode run --pure --auto -m {model} -f "{prompt_file}" --dir "{workdir}" "Do the task in the attached file. Edit files in this directory only."
```

`--auto` approves the agent's file edits without asking. That is acceptable here only because every attempt runs in a throwaway worktree. `--format json` may expose token usage, which would let the harness record costs; this is still to be checked.

## Cost

Agent CLIs report no usage the harness can trust yet, so every attempt is recorded with `cost_status: "unknown"`, and the summary says so. Do not read a missing cost as $0. Real costs come once agent traffic goes through the Phase 1 gateway, or once OpenCode's JSON events prove reliable.

**Running against paid models costs money and needs a budget agreed with the project owner first.**

## Status

There are 2 seed cases so far. The plan calls for 20–40 across all categories before baselines are worth running.
