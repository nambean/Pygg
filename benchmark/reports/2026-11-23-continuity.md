# Continuity when a limit is hit, 2026-11-23

> **Status:** Record · run 2026-11-23 (UTC+7) by the maintainer · ROADMAP focus item 4 · $0: a fake provider in the gateway's own tests · raw results: [`results/2026-11-23-continuity`](../results/2026-11-23-continuity/)

**In one line:** 20 of 20 staged limit events resumed. The next turn was answered by the fallback, which received the whole conversation, and the turn after stayed there. The first run resumed 18 of 20: an Anthropic 529 overload reached Claude Code as an error instead of moving on. That is fixed.

## What changed

- **Anthropic's 529 `overloaded_error` falls back.** It refuses the request before it runs, so it is not billed. The next target takes it, even for a key that allows no paid retry. Before, every 5xx was treated as possibly billed, which stops the request by default.
- **The status line shows the model a session is on now.** After a fallback it reads `now <model> (was <model>)`, not only the model used most.
- **No OAuth bridging.** Fallback goes only to connections the user added: API keys, coding plans, local models.

## Method

`continuity_proof` in `apps/desktop/src-tauri/src/gateway/tests.rs` runs the gateway over HTTP with a fake provider. In each event:

1. An agent's session works for 3 or 8 turns on a combo's first model. Every turn resends the conversation: a system prompt, then per turn a user message, a tool call and its result.
2. The first model's quota then runs out.
3. Two more turns are sent.

An event **resumes** when all of these hold:

- the turn at the limit gets HTTP 200 from the fallback;
- what the fallback received holds every earlier user message, tool call id and tool result;
- the next turn is answered by the fallback too;
- both answers name the model in `x-pygg-route`.

```text
cargo test --release --lib continuity_proof -- --ignored --nocapture   (PYGG_PROOF_OUT=FILE for one row per event)
```

| Surface (agent) | Limits staged | Fallback | Events |
| --- | --- | --- | --- |
| Chat Completions (OpenCode) | 429 with `Retry-After`, `insufficient_quota`, 402 no credit, a 24 h limit | Another connection, same format | 8 |
| Anthropic Messages (Claude Code) | 429, the account's spend limit (400), 529 overloaded | A Chat Completions model, translated | 6 |
| Responses (Codex) | 429, `insufficient_quota`, 402 | Another connection, through Chat Completions | 6 |

## Results

| Run | Resumed | Context lost | Stayed on the fallback |
| --- | --- | --- | --- |
| Before the fix | 18 / 20 | 0 | 18 / 20 (the 2 Anthropic 529 events answered the agent with 529) |
| After | **20 / 20** | 0 | 20 / 20 (`x-pygg-session: pinned`) |

- **Not covered here:**
  - Streamed answers. Fallback before the first byte of a stream is in the [resilience benchmark](2026-11-14-resilience.md).
  - A limit hit in the middle of a streamed answer, which cannot move without losing that answer.
  - The prompt cache, which a new model starts cold. Context is kept; cost is not.
- **A fallback must be set up.** A session pointed at one model with nothing behind it gets the limit as an error. A combo or an `auto` route is what keeps it going.
