# Codex on Claude through a gateway: with and without the prompt cache, 2026-11-14

> **Status:** Record · run under the owner's standing benchmark budget (2026-11-14: "benchmark token-usage scenarios to publish how Pygg differs"), about $0.38 in all · raw results: [`results/2026-11-14-codex-claude-cache`](../results/2026-11-14-codex-claude-cache/) · the decision it tests: [ADR-036](../../docs/DECISIONS.md)

**In one line:** Codex CLI working through Pygg on Claude Haiku 5.5 solved **16 of 16** task runs either way; with Pygg's automatic prompt caching (ADR-036) the same work cost **30% less in all** ($0.096 against $0.138) and **68% less on the input side** ($0.034 against $0.103), because 84% of the input was read from Anthropic's cache instead of billed as new.

## Why this matters

Codex speaks OpenAI's Responses API. To use a Claude model, a gateway translates each turn into Anthropic's Messages API. Anthropic caches only what a request marks, and an OpenAI-format client has no way to mark it, so a plain translating gateway sends every turn of the agent's conversation, which grows with each tool call, as new input at full price. Pygg adds one top-level `cache_control` to such a turn when it carries tools and goes to Anthropic's own API, and the ledger records it (`cache hint: added`).

## What was run

- **Agent:** Codex CLI 0.163.0, `codex exec --json -s workspace-write`, through a provider entry pointing at a scratch Pygg gateway, with a session header per attempt.
- **Model:** `anthropic/claude-haiku-5-5` at Anthropic's list price ($0.10 input, $0.50 output, $0.01 cache read, $0.125 cache write per million tokens), costed from each gateway's own ledger.
- **Arms:** two gateways from the same build (commit 758c6f0), one with `PYGG_ANTHROPIC_AUTO_CACHE=off` (what a translating gateway without cache markers does), one as Pygg ships.
- **Cases:** eight of the repository's benchmark cases (five medium features and a refactor, and three hard ones: INI parsing, JSON Patch, semver ranges), each run twice per arm, each in a fresh Git worktree, judged only by the case's own checks (protected test files).

## Results

| Arm | Passed | Requests | Input tokens | of which cache reads | Cache writes | Output tokens | Cost | Per solved run |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Cache off (plain gateway) | 16 / 16 | 56 | 1.03M | 0 | 0 | 69K | $0.138 | $0.0086 |
| **Pygg** | 16 / 16 | 62 | 1.17M | 0.98M (84%) | 0.19M | 126K | **$0.096** | **$0.0060** |

| Cost by kind | Cache off | Pygg |
| --- | --- | --- |
| Input (new, read, written) | $0.103 | $0.034 |
| Output | $0.035 | $0.063 |

- **The saving is on the input side.** Per million input tokens sent, the cache-off arm paid $0.100 and Pygg's arm $0.029: 71% less.
- **Output varied with the agent, not the gateway.** The Pygg arm happened to write 126K output tokens against 69K, mostly on `hard-semver-range` (both arms solved it); output is billed the same either way. With equal output the total saving would have been about 45%.
- **Per case** (average of two runs): the small tasks fell from $0.0057–0.0071 to $0.0021–0.0040; the hard tasks, whose cost is mostly output, moved less.
- A one-off question costs a little *more* with caching (the 1.25× write is not read back); Pygg therefore adds it only to agent turns, which carry tools.

## What the run found besides

Before the fix of commit 758c6f0, Pygg sent `max_tokens` 4096 when an OpenAI-format client set no output limit, and Codex failed all three hard cases ("Incomplete response returned, reason: max_output_tokens") in a first round (5 of 8 passed per arm, kept as `round1-*` in the raw results). Pygg now gives such a request 64,000 tokens when streamed, at most the model's maximum. Both arms of the reported round ran with the fix.

## Limits

- One model, one agent, eight small-repository cases, two runs each. 16 of 16 says the cache did not hurt answers here; it does not measure quality differences.
- Prices are Anthropic's list prices recorded in each gateway; neither is a provider invoice.
- Claude Code needs none of this: it marks its own cache, and Pygg passes Anthropic requests through unchanged (the CCHT run: 96% of Haiku's input read from cache).
- Gemini and OpenAI models cache automatically; this result is about Claude models reached from OpenAI-format clients.
