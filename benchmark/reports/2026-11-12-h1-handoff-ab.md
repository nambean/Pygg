# H1 handoff A/B: what the next step is told after a failed one (2026-11-12)

> **Status:** Complete. No evidence for `evidence` as a default, so **`tail` stays the default.** The exploratory run showed one nominal saving. The confirmatory run was pre-registered in commit `2da7619` before it started. It was cut off at 25 of 36 pairs, which is inconclusive by its own rule, and it did not repeat the saving.

## Question

When a cascade step fails, the task engine hands the next step one of three things (ADR-030, `handoff.rs`, docs/AGENTS.md):
- **`none`**: the task alone.
- **`tail`**: today's default. The failed check's command and the last 1,500 characters of its output, or the task alone if the agent itself failed.
- **`evidence`**: every earlier attempt, built from what Pygg recorded. For each attempt, the failing check's output (cleaned, with repeats named rather than copied), the paths it changed, and the agent's own final message marked unverified. Capped at about 6,000 characters.

Does `evidence` help the next step finish the task, or finish it with less?

## Set-up (both runs)

- **Harness:** `bench --engine opencode --mode cascade --repeat 3`. These are the 12 hard cases of the N2 report (`benchmark/cases/hard`), 3 rounds each, so 36 runs per arm.
- **Agent:** OpenCode 1.18.4 through Pygg's gateway, on a scratch store.
- **Cascade:**
  - step 1 is `qwen3:8b` on Ollama, on this PC's GPU in Docker (free);
  - step 2 is `claude-haiku-5-5` through Pygg's Anthropic connection (paid, list price).
- **Limits:** agent time limit 300 s per step. Each arm is capped at $1.
- **Only difference between arms:** `--handoff`.
- **Pairing:** the arms ran at the same time on the same machine, so the run for case *c*, round *r* in one arm is paired with case *c*, round *r* in the other.
- **Measures:** pass means the case's approved checks passed. Tokens and money come from the gateway's ledger at the gateway's prices. They are list-price estimates, not a provider bill.

## Exploratory run (three arms, 2026-11-12, 01:00–04:30)

Results: [`benchmark/results/2026-11-12-h1-exploratory/`](../results/2026-11-12-h1-exploratory/), with `SHA256SUMS`. In those files the `run` field lacks the `:handoff=…` suffix, because that `bench` build dropped it. The file name is the arm. The bug was fixed in the same commit as this report. The analysis can be reproduced with `python scripts/handoff-ab.py tail=tail.jsonl evidence=evidence.jsonl none=none.jsonl`.

| Arm | Passed | Step 1 reached the checks | Mean step-2 time | Mean step-2 input tokens | Mean step-2 output tokens | Money (36 runs) |
| --- | --- | --- | --- | --- | --- | --- |
| none | 36/36 | 5 | 43.4 s | 109,552 | 6,329 | $0.533 |
| tail | 36/36 | 4 | 41.8 s | 103,183 | 6,109 | $0.506 |
| evidence | 36/36 | 3 | 41.3 s | 91,157 | 6,208 | $0.464 |

All 108 runs passed, and every one passed at step 2. The 95% Wilson interval for 36/36 is [90.4%, 100%] in each arm.

Paired differences, tail minus evidence, as the mean per run with 95% and 99.6% bootstrap intervals (12 comparisons, Bonferroni):

| Measure | Mean | 95% | 99.6% |
| --- | --- | --- | --- |
| Step-2 input tokens | +12,027 | [+2,745, +21,552] | [−1,459, +26,331] |
| Step-2 output tokens | −100 | [−897, +540] | [−1,382, +763] |
| Step-2 time | +0.4 s | [−2.8, +3.2] s | [−4.7, +4.2] s |
| Money per run | +$0.0012 | [−$0.00004, +$0.0023] | [−$0.0006, +$0.0029] |

Neither `tail` nor `none` differs from the other on anything beyond its interval. `evidence` minus `none` shows −18,395 input tokens and −$0.0019 per run. Both are beyond the 95% intervals, and neither is beyond the 99.6% intervals.

### What this run can and cannot say

- **Pass rate could not discriminate.** Every arm passed every run, so the run says nothing about whether `evidence` makes the next step *succeed* more often.
- **The part of `evidence` that differs most from `tail` was hardly exercised.** That part is the failing check's output across attempts and the changed paths. Step 1 reached the checks in only 3 to 5 runs per arm. The rest of the time it timed out (17 to 19 runs) or changed nothing (13 to 15 runs). In those runs `tail` hands over the task alone, and `evidence` hands over what Pygg recorded of the attempt: that it timed out or changed nothing, the paths it changed, and its own account. So the comparison mostly measures "a short labelled account of a failed attempt" against "nothing".
- **One measure moved.** Step-2 input tokens fell by about 12% with `evidence`. That is beyond the 95% interval, but not once the 12 comparisons are allowed for, and it is the only measure that moved. Input tokens are summed over every request the agent made, so a fall means fewer or shorter requests, not a shorter handoff. The handoff itself is at most about 1,500 tokens.
- **The "repeated failing change" rate was not measured.** Step 1 changed files in only 5 to 9 runs per arm, and the result files do not keep step 2's diff next to step 1's. The question is still open.
- **The run had contention.** The three arms ran at the same time on one GPU, which made step-1 time-outs more likely. The arms were paired, so each arm had the same conditions.

**Decision from this run: none.** ADR-030's rule was "H1 becomes the default only if `evidence` beats `tail` on pass rate, or matches it at lower tokens or time, beyond the paired intervals." Read literally, the 95% interval on one of several measures meets it. But that measure was picked after seeing 12 comparisons, so switching the default on it would be choosing the test after the result. The run is treated as exploratory, and one confirmatory test is registered below.

## Confirmatory run: pre-registration

Written and committed before the run starts.

- **Arms:** `tail` and `evidence` only. Everything else is as in the set-up above: the same 12 hard cases × 3 rounds, the same steps, the same time limit, $1 per arm, run at the same time, with the `bench` build that records the arm's label.
- **Primary measure (the only one that decides):** step-2 gateway input tokens per run, paired by case and round, as `tail` minus `evidence`.
- **`evidence` becomes the default if both of these hold:**
  - the 95% paired bootstrap interval of that difference lies entirely above 0 (20,000 resamples, seed 7, `scripts/handoff-ab.py`);
  - `evidence` passes no fewer runs than `tail`.
- **Otherwise** `tail` stays the default. `evidence` stays available per task (`handoff` in the task API, `--handoff` in `bench` and `agent task`), and the negative result is recorded here.
- **Inconclusive:** if a budget stop or errors leave fewer than 30 complete pairs, the result is inconclusive and `tail` stays.
- **Secondary measures, reported but not deciding:** pass rate, step-2 time, step-2 output tokens, money, and how often step 1 reached the checks.
- **Not claimed either way:** that `evidence` helps when step 1 fails its checks. That needs a cascade whose first step reaches its checks and fails, which this set-up rarely produces. It is the next H1 benchmark.

## Confirmatory run: result

**Inconclusive by the pre-registered rule. `tail` stays the default.**

The run started at 04:34, after commit `2da7619`. It was cut off when the session driving it ended, which stopped both `bench` processes and the scratch gateway. At that point `tail` had 25 runs and `evidence` had 26, giving 25 complete pairs. That is fewer than the 30 the registration requires. Results: [`benchmark/results/2026-11-12-h1-confirmatory/`](../results/2026-11-12-h1-confirmatory/), with `SHA256SUMS`.

| Arm | Passed | Step 1 reached the checks | Mean step-2 time | Mean step-2 input tokens | Mean step-2 output tokens | Money |
| --- | --- | --- | --- | --- | --- | --- |
| tail | 25/25 | 4 | 38.5 s | 102,731 | 5,568 | $0.343 |
| evidence | 26/26 | 7 | 37.2 s | 99,875 | 5,309 | $0.346 |

Paired over the 25 pairs, tail minus evidence:

| Measure | Mean | 95% |
| --- | --- | --- |
| Step-2 input tokens (primary) | +1,472 | [−15,706, +15,247] |
| Step-2 output tokens | +172 | [−547, +955] |
| Step-2 time | +0.8 s | [−2.9, +4.4] s |
| Money per run | +$0.0002 | [−$0.0017, +$0.0017] |

The cut-off alone makes the run inconclusive. The data point the same way: the 12% input-token saving seen in the exploratory run did not repeat. The estimate is about 1%, with an interval centred near zero. Taken together, the two runs give no evidence that `evidence` saves tokens, time or money, or changes the pass rate, in a cascade whose first step mostly times out or changes nothing.

**What follows:**
- `tail` stays the default.
- `evidence` stays available per task.
- The open H1 question is still the one registered as "not claimed": whether `evidence` helps when step 1 reaches its checks and fails. Testing it needs a first step that gets that far. This set-up does not produce one, because the local `qwen3:8b` step mostly timed out.

## Spend

| Run | Money at list price |
| --- | --- |
| Exploratory (3 arms, 108 runs) | $1.503 |
| Confirmatory (2 arms, 51 runs, cut off) | $0.689 |
| **H1 total** | **$2.192** |
