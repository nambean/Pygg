# Web sessions on coding tasks: twelve modules in one reply each, 2026-11-13

> **Status:** Record · **approved for publication by the owner on 2026-11-13** · design: [ADR-035](../../docs/DECISIONS.md) · earlier run on short tasks: [web sessions](2026-11-13-web-sessions.md) · raw results: [`results/2026-11-13-web-coding`](../results/2026-11-13-web-coding/) · agent: [`scripts/bench-web-agent.py`](../../scripts/bench-web-agent.py)

**In one line:** asked to write a whole module from a specification in one reply, with no tools and no second try, the owner's **ChatGPT accounts solved 17 and 21 of 24** runs and **DeepSeek 17 of 24**, which is as many as Claude Haiku 5.5 through its API solved the same way (16 of 24), at no money; **Muse, Grok and Perplexity solved 3 to 4 of 24**, almost always because Pygg's web session gave up before the site started writing its long answer.

## Results for readers

| Provider | Runs | Solved | 95% interval | Median time of a usable reply | No usable reply | Reply came, checks failed | Money |
| --- | --- | --- | --- | --- | --- | --- | --- |
| ChatGPT, account 2 (web) | 24 | **21** (88%) | 69% to 96% | 25 s | 1 | 2 | $0 |
| ChatGPT, account 1 (web) | 24 | **17** (71%) | 51% to 85% | 25 s | 0 | 7 | $0 |
| DeepSeek (web) | 24 | **17** (71%) | 51% to 85% | 16 s | 0 | 7 | $0 |
| Claude Haiku 5.5 (API, same harness) | 24 | **16** (67%) | 47% to 82% | 14 s | 2 | 6 | $0.07 |
| Gemini 3.1 Flash-Lite (API, same harness) | 24 | **10** (42%) | 24% to 61% | 3 s | 0 | 14 | ≈$0.01 |
| Perplexity (web) | 24 | **4** (17%) | 7% to 36% | 24 s | 14 | 6 | $0 |
| Grok (web) | 24 | **4** (17%) | 7% to 36% | 39 s | 18 | 2 | $0 |
| Muse (web) | 24 | **3** (12%) | 4% to 31% | 55 s | 21 | 0 | $0 |

- "No usable reply": the request failed (HTTP 504 from the web session, below) or the reply held no file in the asked form. "Reply came, checks failed": a module came back and did not pass the case's tests.
- Intervals are Wilson 95%. Twelve cases, each run twice, per provider.
- Money: web sessions are the owner's plans with the sites, $0 a request (Pygg's rule since the evening of 2026-11-13; at the time of the run the app still recorded them as unknown). The two API arms are list-price estimates from the gateway's ledger.

Per case, solved out of two runs:

| Case | Muse | ChatGPT 1 | ChatGPT 2 | Perplexity | Grok | DeepSeek | Haiku 5.5 (API) | Flash-Lite (API) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `hard-async-pool` | 2 | 2 | 2 | 0 | 0 | 1 | 2 | 0 |
| `hard-cart-rules` | 1 | 2 | 2 | 2 | 2 | 2 | 2 | 2 |
| `hard-cron-next` | 0 | 2 | 2 | 0 | 2 | 2 | 2 | 1 |
| `hard-expr-eval` | 0 | 2 | 2 | 2 | 0 | 2 | 2 | 1 |
| `hard-glob-match` | 0 | 1 | 1 | 0 | 0 | 1 | 1 | 0 |
| `hard-ini-parse` | 0 | 2 | 2 | 0 | 0 | 2 | 1 | 1 |
| `hard-json-patch` | 0 | 2 | 2 | 0 | 0 | 2 | 0 | 1 |
| `hard-line-diff` | 0 | 2 | 2 | 0 | 0 | 1 | 0 | 0 |
| `hard-refactor-registry` | 0 | 1 | 2 | 0 | 0 | 2 | 2 | 2 |
| `hard-semver-range` | 0 | 0 | 2 | 0 | 0 | 1 | 0 | 0 |
| `hard-sliding-window` | 0 | 1 | 2 | 0 | 0 | 0 | 2 | 2 |
| `hard-text-table` | 0 | 0 | 0 | 0 | 0 | 1 | 2 | 0 |

## What was run

- **Tasks:** the 12 `hard-*` cases of [`benchmark/cases`](../cases), the suite N2 and P0 used: each asks for a module against a specification with many edge cases, a concurrency or off-by-one fix, or a change across three files; each has its own tests, which are protected.
- **The agent is one request.** A web session takes no tools, so no coding agent can drive it. [`scripts/bench-web-agent.py`](../../scripts/bench-web-agent.py) sends one message through the running Pygg gateway: the task, the rules, and every file of the fixture, and asks for each changed or new file back in full, under a header naming the file. It writes what comes back into the worktree and exits; the bench's checks then run. Nothing is executed before the reply and nothing is retried. The two API arms went through the same agent and the same gateway, so they measure the same thing: what one reply is worth, not what an agent with tools can do (P0 measured that: Haiku with OpenCode solved 34 of 36 of these cases).
- **Accounts:** the owner's own, signed in by the owner in Pygg's windows: Muse, ChatGPT ×2, Perplexity, Grok, DeepSeek. At least one ChatGPT account is on the free plan; the sites' models and plans are whatever the accounts have, and the sites may route a long request to a different model than a short one.
- **Pace:** per account, one request at a time with Pygg's 10 s rest; the eight arms side by side. 24 requests per account, under the daily limit of 50. 17:55 to 18:10 local, about 15 minutes for the fastest arms, 90 minutes for the slowest.
- **Path:** `http://127.0.0.1:8741/v1/chat/completions`, streamed, with a Pygg API key made for the run (to be revoked by the owner). Build: the owner's installed app of the morning (web-session benchmark build 9165aba) serving the sessions; bench and agent from main at d72e18c.
- **What a request looks like:** about 2,000 to 6,000 tokens in (the fixture's files, 2 to 4 of them), a module of 50 to 200 lines out.

## What it shows

- **ChatGPT and DeepSeek through a web session write working modules.** 21 of 24 for the second ChatGPT account, 17 of 24 for the first and for DeepSeek, against 16 of 24 for Haiku 5.5 through its API in the same one-reply harness. Their replies took 16 to 25 s as a median, against 14 s for Haiku and 3 s for Flash-Lite.
- **The failures of Muse, Grok and Perplexity are Pygg's limits, not mostly the models'.** The web session waits 60 s for the first word of a reply and 300 s for its end (`websession::Timing`). A page that thinks before it writes a 150-line module passes 60 s, and Pygg answers 504 ("the provider may have run the request"): 21 of Muse's 24, 18 of Grok's, 14 of Perplexity's. Where their reply did arrive it was usually right: Muse 3 of 3, Grok 4 of 6, Perplexity 4 of 10. The first-word wait should scale with the request, or the recipes should treat "thinking" on the page as a first word. Not changed in this run; a change for the roadmap.
- **A web session returns the page's text, not Markdown.** A code block comes back as the file name, a language word, the page's own "Copy" and "Download" buttons, then the code, with no fence. The agent reads that form; a tool that expects Markdown fences would not. The recipes could strip the buttons and restore fences; noted.
- **Two API replies were not in the asked form** (Haiku on `hard-semver-range`, both runs): the reply held no file under a header. A one-reply harness is only as good as the model's compliance with the format; agents with tools do not have this problem.
- **Cost.** The web arms cost no money. The same 24 runs cost $0.07 through Haiku's API and about a cent through Flash-Lite. A web session is for occasional questions and for fallback, as ADR-035 says: at 16 to 55 s a reply and one request at a time it cannot serve an agent's loop, and the sites' terms may forbid the use at all.

## What it cannot claim

- **The sites' models.** A site's plan, model and routing are its own and may differ between accounts and days; the two ChatGPT accounts differ by 4 of 24 on the same prompts.
- **Agent work.** One reply with no tools, no test run and no second try is a different task from what Claude Code or OpenCode do. The API arms are here only to calibrate the harness.
- **More than one day, one machine, one network.** Same as the earlier web-session run. Twelve cases run twice give wide intervals.
- **Whether a site restricts accounts for this use.** None did during the run; the sites' terms still apply.

## How to repeat it

1. Sign in to each web session in Pygg and turn it on; create a Pygg API key and save it in a file.
2. From `apps/desktop/src-tauri`, per provider (the providers can run side by side, being different accounts):
   ```powershell
   $env:PYGG_BENCH_KEY_FILE = "<file with the key>"
   .\target\debug\bench.exe --cases ..\..\..\benchmark\cases --out <dir> --agent "python -I ..\..\..\scripts\bench-web-agent.py {prompt_file} {model} {workdir}" --rung "T1=chatgpt/chatgpt" --mode single --repeat 2 --allow-unknown yes --agent-timeout 900
   ```
   (`--only hard-*` is not a filter; copy the `hard-*` cases into a folder of their own.)
3. Revoke the key afterwards.

The raw files are the bench's own result lines: per attempt, whether it passed, each check's exit and last lines, the agent's exit and time, and the cost (unknown for a web session at the time). They hold no prompts, no replies and nothing from the accounts.
