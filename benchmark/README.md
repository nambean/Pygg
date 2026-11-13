# Benchmark

> **Status:** Current · the benchmark harness and its cases · reviewed 2026-11-11 · order: [ROADMAP](../docs/ROADMAP.md) · built: [Implementation Status](../docs/IMPLEMENTATION-STATUS.md)

Tests the product's core idea (ADR-013): does a cascade that climbs model tiers on check failure reach a comparable pass rate at a lower cost per success than always using one model? It also compares the baselines in [ROADMAP](../docs/ROADMAP.md) "P0 release proof": a pinned premium model, a pinned inexpensive capable one, a static combo and Auto. Paid traffic is capped at **$5 in total** (ADR-028). Results, each approved for publication by the owner: [the first pass](reports/2026-11-11-c3-first-pass.md) (every arm but Qwen passed every case, so a harder suite was next); [N2](reports/2026-11-12-n2-savings.md) (12 harder cases: Haiku 5.5 instead of Sonnet 5.5 cost 88% less per solved task); [P0](reports/2026-11-12-p0-optimization.md) (`auto/cheap` 89% less than pinned Sonnet with OpenCode, Claude Code with Haiku 79% less than with Sonnet); [web sessions](reports/2026-11-13-web-sessions.md) (five AI websites as providers: 47 of 48 short tasks, about 18 s each).

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
  "checks": ["node test.js"],
  "protected": ["test.js"]
}
```

- Every case needs at least one deterministic check. A check passes only if it exits 0, and all of a case's checks must pass.
- `protected` lists fixture files the agent must leave alone: the tests and the check scripts. An attempt that changes or deletes one fails before its checks run, since a check the agent rewrote proves nothing.
- Categories: `bug_fix`, `feature`, `refactor`, `test_writing` (the agent writes tests; a script runs them against the real code, which must pass, and against altered versions, each of which must fail), and `docs` (a script checks the README or JSDoc for what the prompt asks). Complexity is `low`, `medium` or `high`.
- The fixtures need only Node.js.
- Each case was checked when written: its fixture as given fails, and with a reference solution (not in the repository, so no agent can read it) it passes.

There are 36 cases: 8 bug fixes, 21 features, 3 refactors, 2 test-writing cases and 2 docs cases; 7 low, 17 medium and 12 high complexity. The 12 whose id starts with `hard-` were added for ROADMAP N2 after every arm passed every one of the first 24 ([first pass](reports/2026-11-11-c3-first-pass.md)): each asks for a module written against a specification with many edge cases (passing solutions added 56–211 lines), a concurrency or off-by-one fix, or a fix or refactor spread over three files.

## Running

The harness is the `bench` binary of the desktop crate. Run it from `apps/desktop/src-tauri`:

```powershell
cargo run --release --bin bench -- `
  --cases ../../../benchmark/cases `
  --out ../../../benchmark/runs `
  --agent '<agent command>' `
  --rung T0=<model> --rung T2=<model> --rung T4=<model> `
  --mode all --budget-usd 1 [--gateway-db <gateway store>]
```

- `--agent` is a command line run through the shell inside each attempt's worktree. It can use three placeholders: `{prompt_file}` (the task text, kept outside the worktree), `{model}` and `{workdir}`.
- `--mode single` runs each rung alone (the baselines), `--mode cascade` climbs the rungs in tier order, and `--mode all` does both. The tier only orders the rungs.
- `--budget-usd` is a hard cap on the run's estimated spend: no attempt starts once it is reached, and the run stops after an attempt whose money cost is unknown.
- `--only <id>` runs one case; `--agent-timeout` and `--check-timeout` set limits in seconds.

Each attempt runs in its own Git worktree and is discarded afterwards. Checks run even when the agent exits with an error.

### Through the task engine (`--engine`)

`--engine AGENT` runs each case as a smart task, as the app would, instead of running `--agent`'s command: the fixture becomes a workspace, its checks are approved (protected files first, as `git diff --quiet HEAD -- <files>`), and the rungs become the task's steps for that agent. A failed check hands its output to the next step, so a cascade is the product's own. Each attempt runs with `PYGG_ATTEMPT_ID` set to its id; point the agent's gateway session header at it (`{env:PYGG_ATTEMPT_ID}` in OpenCode's configuration) and `--gateway-db` costs each attempt from the gateway's ledger. The budget is checked between cases: a ladder, once started, runs to its end, so keep the gateway client's limit strict. `--repeat N` (up to 10) runs every case N times per arm, for a pass rate rather than a single outcome. `--allow-unknown yes` keeps a run going after an attempt whose cost could not be read (a free model that timed out leaves no usage); use it only when every paid route goes through a gateway client with a strict limit, which then bounds the spend.

### Through the gateway

With `--gateway-db`, each attempt runs with its own session key in `PYGG_BENCH_SESSION`. The agent's configuration sends it as `x-smarttaskrouter-session`, and the attempt's cost is what the gateway's ledger recorded for that session, at the gateway's prices: the agent cannot know which model a route chose. Requests on a subscription count as quota, not money. An attempt that sent nothing through the gateway (a model the agent reaches directly) is costed from the agent's own report. Put a spending limit on the gateway client too, and make it strict (Clients → Limits): the gateway then holds each request at the most it could cost, so the limit cannot be passed.

OpenCode (1.18.4) through the gateway's Anthropic surface: a provider in the file `OPENCODE_CONFIG` points to.

```json
{
  "provider": {
    "pygg": {
      "npm": "@ai-sdk/anthropic",
      "options": {
        "baseURL": "http://127.0.0.1:<port>/v1",
        "apiKey": "{file:<file with the client token>}",
        "headers": { "x-smarttaskrouter-session": "{env:PYGG_BENCH_SESSION}" }
      },
      "models": {
        "auto": { "tool_call": true, "limit": { "context": 128000, "output": 32000 } },
        "anthropic/claude-haiku-5-5": { "tool_call": true, "limit": { "context": 200000, "output": 32000 } }
      }
    }
  }
}
```

The agent template (on Windows; `--title` avoids an extra request for a session title, and `-f` comes last because it takes several files):

```text
opencode run "Do the task described in the attached file. Edit files in this directory only." --pure --format json --title bench -m {model} --dir "{workdir}" -f "{prompt_file}"
```

A rung is then `pygg/auto`, `pygg/<combo>` or `pygg/<connection>/<model>`; OpenCode's own models (`opencode/<model>`) go direct. OpenCode sends `cache_control`, so Anthropic's prompt cache works through the gateway.

## Cost

Costs are estimates at list price (the gateway's prices, or the agent's catalog), not provider bills. An agent run through its own sign-in, or an attempt whose cost could not be read, is recorded as `unknown`, never as $0.

**Running against paid models costs money.** The owner's budget for the benchmark is under $5 in total (ADR-028); anything beyond it needs a new decision.
