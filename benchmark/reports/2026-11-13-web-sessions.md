# Web-session connections: does an AI website work as a provider, and how fast? 2026-11-13

> **Status:** Record · **approved for publication by the owner on 2026-11-13** · design: [ADR-035](../../docs/DECISIONS.md) · raw results: [`results/2026-11-13-web-sessions`](../results/2026-11-13-web-sessions/)

**In one line:** six signed-in accounts on five AI websites answered **47 of 48** short tasks correctly through Pygg's gateway. The typical answer took **about 18 s**, at the pace Pygg keeps so that use looks like a person's: one request at a time per account, and a 10-second rest after each reply.

## Results for readers

| Site | Requests | Answered | Correct | Median time to first text | Median whole answer | Failures |
| --- | --- | --- | --- | --- | --- | --- |
| Perplexity | 8 | 8 | 8 | 13.6 s | 14.9 s | none |
| DeepSeek | 8 | 8 | 8 | 12.2 s | 16.0 s | none |
| ChatGPT (account 1) | 8 | 8 | 8 | 16.7 s | 18.1 s | none |
| ChatGPT (account 2) | 8 | 7 | 7 | 17.2 s | 18.5 s | 1 timeout (504), its first request |
| Grok | 8 | 8 | 8 | 17.2 s | 18.9 s | none |
| Muse (Meta) | 8 | 8 | 8 | 19.6 s | 21.8 s | none |
| **All** | **48** | **47** | **47** | **16.1 s** | **17.7 s** | |

- Correct, out of all 48: 47, which is 98%. The 95% Wilson interval is 89% to 99.6%.
- Every answered request was correct.
- Slowest answer: 26.3 s. Nine in ten finished within 22.9 s.

### What the times include

"First text" is measured from the moment the request reaches Pygg until the first words come back. It covers everything Pygg does on the page:
- opening a new chat;
- waiting until the page takes a prompt;
- typing;
- pressing Send;
- the site's own wait before it starts answering.

These are not raw model latencies. They are the end-to-end time a coding tool would see. They do not include the 10-second rest after each reply.

## What was run

- **Tasks:** eight short ones, each with an automatic check, in `scripts/bench-web.py`:

  | Task | What it asks | Check |
  | --- | --- | --- |
  | ok | reply with exactly "ok" | answer is "ok" |
  | arithmetic | 17 × 23 | answer contains 391 |
  | time | 09:15 plus 2 h 45 min | answer contains 12:00 |
  | json | return a given JSON object unchanged | parses to the same object |
  | code | a Python palindrome function | answer contains `def is_palindrome` |
  | translate | Vietnamese to English | answer contains "hello" and "world" |
  | summary | one sentence on what a load balancer does | answer mentions servers |
  | list | the three primary colours of light | answer names red, green and blue |

  The prompts are fixed, so results can be repeated. They are short on purpose: this measures whether a site works through Pygg, not how strong its model is.
- **Accounts:** the owner's own accounts, signed in by the owner in Pygg's windows: Muse, ChatGPT ×2, Perplexity, Grok and DeepSeek. The plan of each is whatever the owner has; at least one ChatGPT account is on the free plan. No password or cookie passed through anything but the site's own sign-in page in Pygg's window.
- **Path:** each request went to the running Pygg desktop app's gateway (`http://127.0.0.1:8741/v1/chat/completions`, streamed) with a Pygg API key, then to the connection's web session.
- **Pace:**
  - each account ran its eight tasks one after another, with Pygg's 10-second rest after each reply;
  - the six accounts ran side by side, being different sites, plus a second ChatGPT account;
  - 48 requests in all, in 3 minutes 11 seconds (07:04:49 to 07:08:00 UTC);
  - eight requests per account, against Pygg's default daily limit of 50.
- **Where:** one Windows 11 PC in Viet Nam, on its usual network, on 2026-11-13 around 14:05 local time (07:05 UTC).
- **Build:** Pygg main at 9165aba (schema v26).
- **Cost:** no money. Web sessions are plans with the site, so Pygg records their cost as unknown, never $0.

## What it shows, and what it does not

- **It works.**
  - Through a Pygg window the user signed in to, each of these five sites answered like an OpenAI-format provider, streamed, and got every check right except one first request.
  - Two sign-in problems found before this run were fixed first: Google's country cookie domain and Grok's cookie host.
  - The sign-in window now behaves like a normal browser while the user signs in (ADR-035, 2026-11-13).
- **The one failure** was the first request on the second ChatGPT account. The reply did not appear within the time limit; that account's next seven requests worked. A first request after a fresh sign-in may meet a page that is still settling. One sample cannot say more.
- **Speed.** Web sessions are slower than an API: about 15–20 s for a short answer, against a second or two through an API for answers this short. They suit occasional questions and fallback, not an agent's many tool calls. They also cannot take tools at all, so Pygg never chooses them for `auto`.
- **Not measured:**
  - long answers;
  - code-agent tasks;
  - many days of use;
  - how close each site's own limits are;
  - whether a site restricts accounts for this kind of use.

  Eight requests per account in three minutes is far from the limits. The sites' terms may forbid automated use whatever the pace (ADR-035, the warning every user accepts).
- **Single run**, one machine, one network, one time of day. The sites change their pages without notice, and a recipe may need updating when they do. Pygg then reports "Page changed" rather than failing silently.

## How to repeat it

1. Sign in to each web session in Pygg (Providers → the connection → Open Sign-In Window), then turn the connection on.
2. Create a Pygg API key and put it in `PYGG_BENCH_KEY`.
3. Run:
   ```powershell
   python -I scripts/bench-web.py --models "muse/muse,chatgpt/chatgpt,perplexity/perplexity,grok/grok,deepseek/deepseek" --out benchmark/results/<date>-web-sessions
   ```
   The script never prints the key. It stops an account at its daily limit and writes `results.jsonl` (one line per request) and `summary.md`.
4. Revoke the key afterwards.

The raw lines hold per request:
- the time;
- the model;
- the task;
- the HTTP status;
- the error code;
- time to first text and in total;
- the answer's length in characters;
- whether the check passed.

They hold no prompt answers and nothing from the accounts.
