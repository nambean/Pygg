# Benchmark

> **Status:** Current · the benchmark harness, its cases and how to run it · reviewed 2026-11-14

The harness runs real coding agents on fixed tasks and judges each attempt only by checks the agent cannot edit. It measures pass rate and cost per solved task for each arm: a pinned model, a combo, `auto`, or a cascade that climbs model tiers when a check fails (ADR-013). Paid traffic stays within the budget in [ADR-028](../docs/DECISIONS.md#adr-028--mit-licence-a-20-benchmark-budget-and-no-auto-chat).

The published results, with what each one does not claim, are summarised in [docs/BENCHMARKS.md](../docs/BENCHMARKS.md). Each run has a report in [reports/](reports) with its method, cost and raw results.

## Layout

```text
benchmark/
  cases/<id>/case.json    task definition
  cases/<id>/repo/        fixture files; the harness turns them into a fresh Git repository per run
  runs/                   raw output of local runs, one JSON line per case and run (git-ignored)
  results/<run>/          raw results of published runs, with SHA256SUMS (committed)
  reports/                one written-up report per published run (committed)
  resilience/             the fault-injecting fake provider and the runs that compare gateways against it
  readiness/              the readiness benchmark: run.py, the latest SCORECARD.md and trend.jsonl (one line per run)
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
- The fixtures need Node.js (22 or later, for `node:sqlite` in `x-sqlite-migration`), and Python 3 for the two `py-` cases.
- Each case was checked when written: its fixture as given fails, and with a reference solution (not in the repository, so no agent can read it) it passes.

There are 41 cases: 9 bug fixes, 23 features, 4 refactors, 2 test-writing cases and 2 docs cases; 7 low, 17 medium and 17 high complexity. The 12 whose id starts with `hard-` each ask for a module written against a specification with many edge cases, a concurrency or off-by-one fix, or a fix or refactor spread over three files. The 5 added on 2026-11-20, after every hosted model solved nearly all of the first 36, are harder in other ways: `x-sqlite-migration` (a schema change with a data migration that merges duplicates and keeps every order), `x-refactor-async-modules` (five modules from callbacks to async/await, behaviour and audit order kept), `x-feature-discounts` (a feature across four modules to a written spec with two rounding rules), and in Python `py-config-includes` (includes, interpolation and exact error messages) and `py-booking-rules` (six broken rules of a room scheduler).

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
- `--repeat N` (up to 10) runs every case N times per arm, for a pass rate rather than a single outcome.
- `--allow-unknown yes` keeps a run going after an attempt whose cost could not be read (a free model that timed out leaves no usage). Use it only when every paid route goes through a gateway key with a strict limit, which then bounds the spend.
- An attempt stopped at its time limit while a request through the gateway was still streaming leaves that request unknown in the ledger (its usage never arrived). The bench counts it at the tokens the gateway saw and the model's price, as a floor: the attempt's `costStatus` is `floor`, its money counts toward `--budget-usd`, and the run goes on without `--allow-unknown`. A request with no price stays unknown.

Each attempt runs in its own Git worktree and is discarded afterwards. Checks run even when the agent exits with an error.

### Through the gateway

With `--gateway-db`, each attempt runs with its own session key in `PYGG_BENCH_SESSION`. The agent's configuration sends it as `x-pygg-session`, and the attempt's cost is what the gateway's ledger recorded for that session, at the gateway's prices: the agent cannot know which model a route chose. Requests on a subscription count as quota, not money. An attempt that sent nothing through the gateway (a model the agent reaches directly) is costed from the agent's own report. Put a spending limit on the gateway key too, and make it strict (Endpoint → API Keys → Limits): the gateway then holds each request at the most it could cost, so the limit cannot be passed.

OpenCode through the gateway's Anthropic surface: a provider in the file `OPENCODE_CONFIG` points to.

```json
{
  "provider": {
    "pygg": {
      "npm": "@ai-sdk/anthropic",
      "options": {
        "baseURL": "http://127.0.0.1:<port>/v1",
        "apiKey": "{file:<file with the Pygg API key>}",
        "headers": { "x-pygg-session": "{env:PYGG_BENCH_SESSION}" }
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

### Through the task engine (`--engine`)

`--engine AGENT` runs each case as a smart task, as the app would, instead of running `--agent`'s command. The fixture becomes a workspace, its checks are approved (protected files first, as `git diff --quiet HEAD -- <files>`), and the rungs become the task's steps for that agent. A failed check hands its output to the next step, so the cascade is the product's own; `--handoff none|tail|evidence` sets what the next step is told (ADR-030). Each attempt runs with `PYGG_ATTEMPT_ID` set to its id; point the agent's gateway session header at it (`{env:PYGG_ATTEMPT_ID}` in OpenCode's configuration) and `--gateway-db` costs each attempt from the gateway's ledger. The budget is checked between cases: a ladder, once started, runs to its end, so keep the gateway key's limit strict.

### One reply, no tools (`scripts/bench-web-agent.py`)

For a provider that takes no tools, `--agent "python -I scripts/bench-web-agent.py {prompt_file} {model} {workdir}"` sends one request through the running gateway with the task and every fixture file, writes the files that come back, and exits; the checks then run. It reads a Markdown reply and the page-text form a web session returns. `PYGG_BENCH_KEY_FILE` names a file holding a Pygg API key. The cost is unknown to the bench, so run with `--allow-unknown yes`.

**Do not point it at a web-session account you need.** Its prompts are long, and a run of them got an account suspended. Pygg's web guard now refuses prompts over 12,000 characters and caps each account's hour by weight, so most cases are refused on a web session.

## Readiness

`python -I benchmark/readiness/run.py` scores the current build against the release gates in [docs/ROADMAP.md](../docs/ROADMAP.md#release-gates), with free and local traffic only: the fake provider, and Ollama (`qwen3:8b`) when it answers on port 11434. It builds the headless binaries, then checks both API formats, streaming, tools, the prompt cache, sessions, fallback on 429, 401 and 404, a strict budget under concurrency, loopback only, the ledger (also across a gateway killed with requests in flight), `auto` decisions, `pygg setup` and `--remove` on scratch configurations, the overhead test, the provider-error and web-guard tests, `pnpm ui-check` and the npm packages installed from their tarballs. A gate is red when a check failed, open when its checks pass but it still needs evidence the harness cannot produce (listed), green otherwise. It takes about 10 minutes; `--skip ui,npm` or `--only gateway` runs part of it (a partial run is not added to the trend). Each run writes `runs/<time>-<commit>/` (git-ignored); a full run also replaces `SCORECARD.md` and adds a line to `trend.jsonl`. It sends nothing to a paid provider or a web session.

## Other scripts

| Script | What it does |
| --- | --- |
| `scripts/bench-report.py RESULTS.jsonl […] [--baseline LABEL]` | Summarises `bench` result files into Markdown tables: pass rates with 95% Wilson intervals, money per run and per passed run with bootstrap intervals, median time, and the comparison with a baseline arm |
| `scripts/handoff-ab.py ARM=RESULTS.jsonl …` | Paired comparison of task-engine handoff arms from `bench --engine` runs |
| `scripts/bench-web.py --models muse/muse,chatgpt/chatgpt,…` | Short checked tasks through the running gateway's web sessions, at Pygg's pace; `PYGG_BENCH_KEY` holds the Pygg API key. Results in `results.jsonl` and `summary.md` |
| `scripts/bench-light.py --base URL --key-file FILE --gateway-db STORE --routes A,B [--rounds 3]` | 40 short checked tasks (extraction, classification, rewrites, reasoning, one function, multi-turn conversation, translation) through a running gateway, each request costed from the gateway's ledger. Results in `results.jsonl` and a summary per run |
| `cargo test --release --lib gateway::tests::overhead -- --ignored --nocapture` | What the gateway adds to a request, against the fake provider; `PYGG_OVERHEAD_OUT=file.json` saves the figures |
| `scripts/bench-ci.py RESULTS.jsonl […]` | Pass rate (Wilson), money and gateway tokens per solved run (bootstrap intervals) and the share of input read from cache, per results file |
| `benchmark/resilience/fake_provider.py [PORT]` | A fake provider (OpenAI Chat Completions, Anthropic Messages with a simulated prompt cache) whose failures are set per provider and key at run time (`POST /_admin/config`); run it in a `python:3.11-slim` container on a Docker network so gateways in containers reach it |
| `benchmark/resilience/run.py --gateway direct\|pygg\|litellm --out DIR` | Fault scenarios (402, 429, 401, 5xx, unreachable, dropped, cut stream, hang, key rings, affinity, `auto` hold) through a gateway against the fake; `summarise.py DIR` makes the tables. `PYGG_GATEWAY_EXE` names the headless `gateway` binary; LiteLLM runs from a digest-pinned image with `litellm-config.yaml` |
| `benchmark/resilience/cache_replay.py --api chat\|responses --out DIR` | One agent session replayed through Pygg and LiteLLM to the fake's Anthropic API: cache markers, cache reads and `max_tokens` per gateway |
| `cargo test --release --lib websession::tests::guard_bench -- --ignored --nocapture --test-threads 1` | The web guard against an agent's load on a fake chat site, and 24 h of its decisions on a virtual clock |

## Cost

Costs are estimates at list price (the gateway's prices, or the agent's catalog), not provider bills. An agent run through its own sign-in, or an attempt whose cost could not be read, is recorded as `unknown`, never as $0.

**Running against paid models costs money.** Published runs stay within the budget in ADR-028; anything beyond it needs a new decision.
