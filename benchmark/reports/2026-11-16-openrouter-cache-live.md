# Claude through OpenRouter: the cache marker and the session, live, 2026-11-16

> **Status:** Record · a short live check approved by the owner on 2026-11-16, $0.0066 in all · raw results: [`results/2026-11-16-openrouter-cache-live`](../results/2026-11-16-openrouter-cache-live/) · the decision it checks: [ADR-036](../../docs/DECISIONS.md)

**In one line:** an agent's OpenAI-format turns to `anthropic/claude-haiku-5.5` on OpenRouter got Pygg's top-level cache marker. After each session's first turn, **99.7% of every prompt was read from the cache**. The ledger named the two misses correctly (a session's first request, a changed system prompt). The check found one bug, now fixed: OpenRouter's cache writes were not recorded.

## What was run

- **Gateway:** the build of commit e11f23c (`gateway serve`) on a scratch copy of the owner's store, with the OpenRouter connection already there. One client was created for the check with a $0.30 monthly limit.
- **Model:** `anthropic/claude-haiku-5.5` on OpenRouter, about 12,000 prompt tokens per turn (six tools and a long system prompt), 20 output tokens, `x-pygg-session` set on every request.
- **Session `live-s1`, Chat Completions:** three turns, then the system prompt changed by one line, then two more turns.
- **Session `live-s2`, the Responses API** (Codex's format, translated to Chat Completions for OpenRouter): two turns.
- **Session `live-s3`:** two turns after the fix below, to see the ledger record the writes.

## Results

| Session, turn | Prompt tokens | Read from cache | Written to cache | Ledger: cache hint, miss cause | Cost (OpenRouter's) |
| --- | --- | --- | --- | --- | --- |
| s1 · 1 | 11,937 | 0 | 11,933 | added, first request | $0.00150 |
| s1 · 2 | 11,964 | 11,933 | 27 | added, hit | $0.00013 |
| s1 · 3 | 11,991 | 11,960 | 27 | added, hit | $0.00013 |
| s1 · 4, system prompt changed | 12,030 | 0 | 12,026 | added, prompt start changed | $0.00151 |
| s1 · 5 | 12,057 | 12,026 | 27 | added, hit | $0.00013 |
| s2 · 1, Responses API | 11,929 | 0 | (not reported back) | added, first request | $0.00150 |
| s2 · 2, Responses API | 11,948 | 11,925 | (not reported back) | added, hit | $0.00013 |
| s3 · 1, after the fix | 11,015 | 0 | 11,011 | added, first request | $0.00139 |
| s3 · 2, after the fix | 11,034 | 11,011 | 19 | added, hit | $0.00012 |

- **The marker works on OpenRouter.** A turn that read from the cache cost about a tenth of the first one.
- **The cache-miss causes read correctly** from the ledger: `first_request` on each session's first turn, `prefix_changed` when the system prompt changed while the same model had answered seconds before.
- **The tools-and-system-prompt meter** recorded each request's estimate: 974 tokens of tools and 11,729 of system prompt (11,741 after the change). The provider counted about 11,940 for the whole prompt, so the estimate leans about 6% high, as the gateway's estimate is meant to. The change was counted once, and turn 4 as one resend.
- **The session went on as `x-session-id`.** Every later turn reached a machine that held the cache. This run cannot say how much of that is the header and how much OpenRouter's own routing: an A/B would need many more requests.

## The bug it found

OpenRouter reports what it wrote to an Anthropic model's cache in `usage.prompt_tokens_details.cache_write_tokens`. Pygg read only `cached_tokens` from the OpenAI format, so the ledger recorded 0 written, and Token Mix showed the writes as new input. The cost was right all along, since it is OpenRouter's own `cost` field. Fixed in `pricing::Usage::from_json`; an Anthropic-format client now also gets `cache_creation_input_tokens` back from a translated answer. Session `live-s3` shows the writes recorded.

## Not covered

- **The way home after a fallback** needs a live failure, so it was not exercised; it remains tested on the fake provider only.
- **OpenAI's `prompt_cache_key`**: no OpenAI key on this computer.
- One provider, one model, nine requests: a check that the mechanism works, not a measure of savings. The Codex benchmark of [2026-11-14](2026-11-14-codex-claude-prompt-cache.md) measures savings on Anthropic's own API.
