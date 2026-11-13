# A real repository: six tasks from CCHT, with Claude Code and OpenCode through Pygg, 2026-11-13

> **Status:** Record · **approved for publication by the owner on 2026-11-13** (the run was the owner's decision, at about $4) · ROADMAP N2's "a real repository layout" · raw results: [`results/2026-11-13-ccht`](../results/2026-11-13-ccht/) · earlier runs on small fixtures: [P0](2026-11-12-p0-optimization.md), [N2](2026-11-12-n2-savings.md)

**In one line:** on six tasks cut from a private production repository (an Express + Prisma API with a 124-check end-to-end suite), **Claude Code with Haiku 5.5 solved 6 of 6** for **$1.50**, Claude Code with Sonnet 5.5 solved 4 of the 5 it ran for $1.19 (one of the five was cut off by Pygg's strict limit, not by the model), and **OpenCode with `auto/cheap` solved 2 of 6** for $0.35, because `auto/cheap` spent most of its requests on free Gemini models that could not take them. On the four tasks both Claude Code arms solved, Haiku cost about half of Sonnet.

## Results for readers

| Case | What is asked | OpenCode + `auto/cheap` | Claude Code + Haiku 5.5 | Claude Code + Sonnet 5.5 |
| --- | --- | --- | --- | --- |
| `ccht-self-approval` (small) | the four-eyes rule on four kinds of records, and the dashboard's counters | failed · 129 s · ≥$0.046 | **passed** · 109 s · $0.070 | **passed** · 82 s · $0.184 |
| `ccht-resigned-guards` (small) | no vouchers or deposit slips for resigned guards | **passed** · 131 s · ≥$0.036 | **passed** · 108 s · $0.089 | **passed** · 75 s · $0.149 |
| `ccht-stocktake-approval` (medium) | the whole stocktake approval handler: guards, adjustment voucher, stock movements, audit | failed · 26 s · $0.000 | **passed** · 157 s · $0.103 | **passed** · 69 s · $0.135 |
| `ccht-accent-search` (medium) | accent-insensitive search over every paged list | **passed** · 282 s · $0.116 | **passed** · 350 s · ≥$0.279 | **passed** · 161 s · $0.421 |
| `ccht-short-transfer` (large) | receiving a transfer short: body, filter, dashboard, audit, Excel sheet, finance row | failed · 95 s · ≥$0.040 | **passed** · 335 s · $0.198 | cut off by the limit after 10 requests · 265 s · $0.305 |
| `ccht-loss-settlement` (large) | settling lost items by compensation or waiver, across the API, reports and Excel | failed · 143 s · ≥$0.111 | **passed** · 309 s · $0.764 | refused by the limit, not run |

| Arm | Solved | Known money | Per solved task | Requests | Input tokens (cache reads) | Output tokens |
| --- | --- | --- | --- | --- | --- | --- |
| OpenCode + `auto/cheap` | 2 of 6 | ≥$0.35 | ≥$0.17 | 124 | 4.6M (4.0M) | 55K |
| Claude Code + Haiku 5.5 | **6 of 6** | ≥$1.50 | ≥$0.25 | 176 | 14.0M (13.4M) | 184K |
| Claude Code + Sonnet 5.5 | 4 of 5 run | $1.19 | $0.30 | 47 | 1.7M (1.5M) | 44K |

- "≥" marks an arm with a request whose cost is unknown: Anthropic answered HTTP 503 ("credential validation failed", or nothing) eight times during the day, and a request the provider may have run is recorded at an unknown cost, never $0. The known part is shown.
- Pass-rate intervals (Wilson, 95%): 6 of 6 is 61% to 100%; 4 of 5 is 36% to 98%; 2 of 6 is 10% to 70%. Six tasks are too few for a narrow claim; the per-case rows are the evidence.
- **Paired, the four tasks both Claude Code arms solved** (self-approval, resigned-guards, stocktake-approval, accent-search): Haiku ≥$0.54, Sonnet $0.89. Haiku solved them for about **40% less**, with one of its four figures a floor. On P0's small fixtures the saving was 79%; a real repository narrows it, because Haiku takes more turns (14 to 42 requests a task against Sonnet's 6 to 17) and each turn carries the same large context.
- **Sonnet was faster** on every task it finished: 69 to 161 s against Haiku's 108 to 350 s.
- **Loss-settlement**, the largest task, was solved only by Haiku, in 62 requests and $0.76: more than half of its arm's money.

## What was run

- **Tasks:** six cases built on 2026-11-12 from CCHT, a private internal tool (uniforms and equipment stock for a security-guard company: an npm-workspaces monorepo, an Express + Prisma API on PostgreSQL in `apps/api`, a React app in `apps/web`, Vietnamese user-facing messages). Each fixture is CCHT's HEAD (`753acb9`, 66 tracked files) with one real feature removed; the prompt describes the feature as a product owner would. The checks are CCHT's own: build (`tsc`), unit tests, and the 124-check end-to-end suite against a throwaway database, about 45 s a run. Test and check files are protected. **The cases are private and are not in this repository**; only the result lines are.
- **Agents:** OpenCode 1.18.4 and Claude Code 2.1.322, headless, one Git worktree per attempt, driven by Pygg's task engine (`bench --engine`), one attempt per case.
- **Routing:** every request through a Pygg gateway on a scratch store (port 8799), with its own session key per attempt; the cost is what that gateway's ledger recorded. Connections: Anthropic (pay-as-you-go key) and Google Gemini (paid key). Prices as in N2 and P0 (Anthropic list prices; Gemini from the reference catalog). The client had a strict monthly limit.
- **The agent's worktree** had no `node_modules` and no `.env` (both are set up by the first check, after the agent is done), so an agent could not run the suite unless it set that up itself. OpenCode did: it ran `npm install` and started a PostgreSQL container of its own to run the end-to-end suite before answering. Claude Code did not. The condition was the same for every arm.
- **Three runs, one after another** (16:46 to 18:03 local), because the first was cut off by a restart of the session that drove it and the second by the strict limit:
  1. 16:46: `auto/cheap` on one case, Haiku on one, Sonnet on one, then the session restarted.
  2. 17:09: `auto/cheap` on all six; Haiku on the five it lacked, stopped by its cap after two; Sonnet on the five it lacked, refused by the strict limit on four, then one passed after the limit was raised.
  3. 17:46: Sonnet on the three small cases; Haiku on its three missing ones; Sonnet on short-transfer, cut off by the limit after ten requests.
  Each case's row above is its last attempt that reached a provider; attempts refused before any request are not counted as runs. The raw files carry every attempt, with its run folder in `source`.
- **Money:** $4.01 known at list prices across the scratch gateway's 405 requests (373 OK, 9 failed, 23 fallbacks, 8 of unknown cost); the client's limit was raised from $4.50 to $7.00 for the third run while the caps bounded it. The owner's benchmark budget stands at about $16.30 spent of $20 after this run.
- **Build:** Pygg main at 1ea2f6e (the bench and gateway binaries of 15:59), schema v26.

## What `auto/cheap` did, and what it found

`auto/cheap` ranks the connected models by the estimated cost of each request, so on this store it ranked Gemini's $0-listed models first. What happened, per the ledger:

- **Gemma 4** (`gemma-4-26b-a4b-it`, `gemma-4-31b-it`): ranked first, answered 19 requests and refused 18 with HTTP 429 ("you exceeded your current quota"), a model the key's paid tier does not serve. Each refusal was a fallback that cost time, not money. Where it answered, the agent got nowhere: on one case "the agent made no changes".
- **`gemini-2.5-flash-lite`**: five more 404s ("no longer available to new users"), the fault P0 reported; this scratch store had it listed again from a fresh model list.
- **`gemini-3.1-flash-lite`**: 65 requests, $0.20, and the two solved cases ran mostly on it.
- **Claude Haiku 5.5** as the fallback after those: on `ccht-stocktake-approval` it refused OpenCode's request with HTTP 400, "this model does not support assistant message prefill": OpenCode had sent a conversation ending in an assistant message. A 400 is the client's fault, so Pygg tried no other model, and the attempt ended after 26 s and six requests. Where Haiku answered, the arm still failed its checks: the cheap models had already written code that did not build.

So `auto/cheap`'s 2 of 6 is not Haiku's result; it is what a cost-first ranking over free models that cannot serve the request does to a coding agent. Two things to take from it:
1. **A model that answers 429 "quota" for every request should drop out of `auto` until it answers again**, as a used-up stated limit does today (ROADMAP E). It cost 18 of 124 requests here.
2. **A 400 "prefill" refusal is a format mismatch, not the model's refusal of the task.** The Anthropic translation could end the conversation with a user turn, or `auto` could move to the next model on that error. Not built; noted for the roadmap.

## What the strict limit did

The client's limit was strict, so each request was held at the most it could cost before it was sent (ROADMAP B4). Claude Code asks for 32,000 output tokens, and the context on this repository reached 90,000 tokens, so a Sonnet request was held at up to $1.39. Once the limit's count ($4.33 at the time, with unknown-cost 503s counted at their most) left less than that, every Sonnet request was refused with 429, and Claude Code retried for three minutes before giving up: four attempts of about 180 s each, with no request sent, until the limit was raised. The last Sonnet attempt, on short-transfer, was cut off the same way after ten requests.

This is the limit working as specified. Two things to take from it:
1. **A strict limit and Claude Code do not mix well on a large repository**: the hold is the output limit at the model's price, whatever the answer will be. The estimated limit (the default) would have let the run finish. The Policies screen should say so next to "strict".
2. **The count includes failed requests of unknown cost at their most**, so eight 503s from Anthropic pushed the count $1.65 above the money known to be spent. That is deliberate (a 503 may have run), but it means a day with provider errors fills a strict limit faster than it fills a bill.

## What this run cannot claim

- **Savings on this repository.** Six tasks, one run each, and two arms that did not finish every task. The paired figure (about 40% less with Haiku on four tasks) is one sample per task.
- **`auto/cheap` against a tuned store.** With Gemma turned off, or with Gemini's paid key alone, it would have been `gemini-3.1-flash-lite` and Haiku from the start.
- **The agents' usual conditions.** The worktree had no dependencies installed, and OpenCode spent its first minutes installing them.
- **Provider bills.** Every figure is a list-price estimate from the gateway's ledger.
- **Other repositories, other languages.** One TypeScript monorepo.

## How to repeat it

The cases are private. For a repository of your own: cut one feature per case out of a commit, keep its tests as the protected checks, write the prompt as the feature's owner would, and run `bench --engine <agent>` through a scratch gateway with a client whose limit is **estimated**, not strict, or strict with enough room for the hold. The raw files here name each attempt's run folder in `source` and hold no file contents, prompts or paths from the private repository beyond the case ids and the checks' last lines.
