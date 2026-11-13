# P0 optimization benchmark: what routing saves, with a second agent, 2026-11-12

> **Status:** Record · **approved for publication by the owner on 2026-11-13** · ROADMAP "P0 release proof" · plan: [2026-11-12 plan](../plans/2026-11-12-p0-optimization-plan.md) · earlier run: [N2](2026-11-12-n2-savings.md)

**In one line:** on the same 12 harder coding tasks as N2, `auto/cheap` through Pygg solved 34 of 36 for **$0.0129 per solved task**, 89% less than pinned Claude Sonnet 5.5 with the same agent (N2's baseline). With Claude Code as the agent, Haiku 5.5 instead of Sonnet 5.5 cost **79% less per solved task** (95% CI 74% to 84%). Rule-based `auto`, which picked Gemini 2.5 Flash, solved only 5 of 12.

## Results for readers

| Agent | How its requests were routed | Solved | Money per solved task | Against pinned Sonnet with the same agent |
| --- | --- | --- | --- | --- |
| OpenCode | pinned Sonnet 5.5 (N2, the baseline) | 34/36 | $0.1142 | — |
| OpenCode | pinned Haiku 5.5 (drift control) | 12/12 | $0.0123 | 89% less (87% to 91%) |
| OpenCode | Pygg `auto/cheap` | 34/36 | $0.0129 | 89% less (88% to 90%) |
| OpenCode | Pygg `auto` | 5/12 | $0.0298 | 74% less (40% to 85%), at a much lower pass rate |
| Claude Code | pinned Sonnet 5.5 | 11/12 | $0.0809 | — |
| Claude Code | pinned Haiku 5.5 | 35/36 | $0.0168 | 79% less (74% to 84%) |

Percent ranges are 95% confidence intervals from a bootstrap over the tasks, paired by task. Pass-rate intervals (Wilson):
- 34/36: 82% to 98%;
- 35/36: 86% to 100%;
- 12/12: 76% to 100%;
- 11/12: 65% to 99%;
- 5/12: 19% to 68%.

## What was run

- **Tasks:** N2's 12 `hard-*` cases ([`benchmark/cases`](../cases)). Each case's checks are its own tests, and its test files are protected.
- **Runs:** three per task for `auto/cheap` and for Claude Code with Haiku; one per task for the drift control, for `auto` and for Claude Code with Sonnet, to stay within the cap.
- **Agents:** OpenCode 1.18.4 and Claude Code 2.1.322, headless, one Git worktree per attempt, driven by Pygg's task engine (`bench --engine`).
- **Routing:**
  - Every request went through a Pygg gateway on a scratch store, with its own session key per attempt. The cost is what that gateway's ledger recorded.
  - Connections: Anthropic (pay-as-you-go key) and Google Gemini (paid key).
  - The client had a strict monthly limit of $5.
  - Unlike N2, there was no Qwen subscription, so `auto` compares money with money.
  - OpenRouter was left out at the owner's choice, since its free tier allows 50 requests a day.
- **Prices:**
  - Anthropic: as in N2 (Sonnet 5.5 $2 / $10 per million, cache reads $0.20; Haiku 5.5 $0.10 / $0.50, cache reads priced as input, so Haiku's figures are an upper bound).
  - Gemini: models.dev list prices, through the reference catalog (ADR-031).
  - All figures are list-price estimates, not invoices.
- **What `auto` and `auto/cheap` chose:**
  - `auto` (the lowest tier at or above the client's minimum, T2) chose `gemini-2.5-flash`.
  - `auto/cheap` (the lowest estimated cost) ranked `gemini-2.5-flash-lite` first. Google answered it with 404 ("no longer available to new users"), so `auto/cheap` fell back to Claude Haiku 5.5, which served all its agent requests.
  - Haiku 5.5 is T1 in Pygg's naming table, below the T2 minimum, so `auto` did not consider it.

## What the numbers say

- **The saving holds with a second agent.**
  - Claude Code sends larger prompts than OpenCode (4.3M input tokens for 36 Haiku runs, against OpenCode's 3.1M). Haiku still cost about a fifth of Sonnet per solved task, and solved 35 of 36 against Sonnet's 11 of 12.
  - As in N2, the intervals overlap, so this does not show that Haiku is better, only that it was not measurably worse here.
- **No drift.** OpenCode with Haiku cost $0.0123 per solved task, against $0.0133 in N2 on the same tasks, and solved 12 of 12.
- **`auto/cheap` here was Haiku.**
  - Its 89% saving is Haiku's saving, reached by ranking and fallback rather than by naming the model.
  - On the way, two product faults showed. Gemini 2.5 Flash-Lite no longer serves new keys. And Google's Lyria music model, listed at $0 per token, received OpenCode's tool-less title requests and refused them.
  - Both cost time, not money, and are fixed (below).
- **Rule-based `auto` picked a weaker model.**
  - `auto` ranks by tier first, from Pygg's naming table, where Gemini 2.5 Flash is T2 and Haiku 5.5 is T1. It chose Gemini 2.5 Flash, which solved 5 of 12 with OpenCode.
  - Ten of its streams ended without any content (HTTP 200 with nothing but `[DONE]`). In the first attempt the agent read such a stream as a finished answer. After the fix below, made before the rerun, each reached the agent as an error.
  - **The naming table's tiers are not a measure of agentic coding ability.** This run is evidence for the roadmap's rule that `auto` makes no savings claim of its own until measured.
  - **Changed after this run, by the owner's decision:** Pygg's tier table is now `curated:v2`, with Claude Haiku 5 at T2 on this evidence, so `auto` can choose it. Stores whose tiers came from v1 are brought up to date at start-up; tiers the user set stay. Older Haiku models stay T1 (not measured).

## Product changes this run caused

All are committed, with tests:

1. **An empty provider stream is a failure, not an empty answer** (`77409d2`). The client gets an error event; the ledger records a stream error with an unknown cost.
2. **Models that make audio, video or images start off** (`f9a37f0`). This applies where the model catalog says so, with the reason shown in Connections.
3. **Anthropic's "credit balance is too low" (HTTP 400) is payment required** (`4cd86c5`). The connection is held for ten minutes, as with a 402, and the next provider may answer.
4. **An agent Pygg starts is not part of the host agent's session** (`5fbee2b`). Run from inside the Claude desktop app, Claude Code had inherited that app's session variables, and Anthropic refused a funded key as out of credit.
5. **A child process's `PWD` is its own folder** (`f3dfbe6`). OpenCode took its working folder from the shell's `PWD` and edited files outside its worktree.
6. **Claude Code through a gateway sends its attempt as the session** (`2c11f0e`), so Pygg's ledger can cost a Claude Code step.

## What this run cannot claim

- **Savings on your work.** The same 12 small Node tasks as N2.
- **That Haiku beats Sonnet.** The intervals overlap.
- **Anything about `auto` in general.** It reflects one tier table and one set of connections. With other connections `auto` picks other models.
- **Sonnet with Claude Code at three runs per task.** It ran once per task (12 runs), within the cap.
- **Provider bills.** These are list-price estimates; Haiku's cache reads are priced as input.
- **`auto`'s first attempt.** It stopped after one task when a stream reported no usage, because the bench refused to go on with a cost it could not read. That run is kept as `opencode-auto-first-try.jsonl`. The arm was run again with unknown costs allowed (the client's strict $5 limit still held). The ten empty streams that rerun met are counted as failures.

## Spend

The scratch gateway's ledger, list prices, smoke runs and setup checks included: **$2.63** (Anthropic $2.13, Gemini $0.50), with 12 requests of unknown cost (Gemini streams that reported no usage). With earlier runs, the benchmark budget stands at about **$12.3 of $20** (ADR-028). The test keys were removed from the Credential Manager afterwards.

## Reproducing

1. **Gateway:** a scratch gateway store with Anthropic and Gemini connections, keys read from the environment (`gateway add-connection --preset anthropic --key-env ANTHROPIC_API_KEY`, likewise `gemini`). Then `gateway lookup`, N2's prices for Sonnet and Haiku (`gateway model … --price-in --price-out …`), and a strict client (`create-client --name bench --monthly-usd 5`, `client --name bench --strict yes`). Serve it on a port.
2. **OpenCode:** a provider `pygg` pointing at it in a scratch `XDG_CONFIG_HOME` ([benchmark README](../README.md#through-the-gateway)).
3. **Claude Code:** in a scratch `CLAUDE_CONFIG_DIR`, with:
   - `ANTHROPIC_BASE_URL` set to the gateway;
   - `ANTHROPIC_AUTH_TOKEN` set to the client's token;
   - `ANTHROPIC_MODEL` and the default Haiku, Sonnet and Opus models set to the rung;
   - any variables inherited from a host agent session removed (Pygg now does this for the agents it starts).
4. **One run per arm**, e.g.:

   ```text
   bench --cases <the 12 hard-* cases> --engine opencode --repeat 3 --gateway-db STORE --rung T1=pygg/auto/cheap --mode single --budget-usd 0.7
   bench --cases <…> --engine claude-code --repeat 3 --gateway-db STORE --rung T2=anthropic/claude-haiku-5-5 --mode single --budget-usd 1.4
   ```

5. **Report:** `python scripts/bench-report.py benchmark/results/2026-11-12-p0/opencode-haiku.jsonl benchmark/results/2026-11-12-p0/opencode-auto-cheap.jsonl benchmark/results/2026-11-12-p0/opencode-auto.jsonl benchmark/results/2026-11-12-n2/sonnet.jsonl --baseline sonnet`, and for Claude Code the two `claude-code-*.jsonl` files.

**Raw results:** [`benchmark/results/2026-11-12-p0/`](../results/2026-11-12-p0). One JSON line per run, with every attempt's checks, tokens and ledger figures, and a `SHA256SUMS` file. Local paths were replaced with `<local path>`, and the files were scanned for keys and tokens before they were committed.
