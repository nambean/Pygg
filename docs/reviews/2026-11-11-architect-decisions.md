# Architect decisions on the review response, 2026-11-11

> **Update 2026-11-11 (later the same day):** of the items listed as not done below, P0-5, the Anthropic parity benchmark and the single-store lock are done; see [Implementation Status](../IMPLEMENTATION-STATUS.md). GW-8 and the tray part of GW-9 are planned in [ROADMAP](../ROADMAP.md) (B6, A1).


Inputs: [the architecture review](2026-11-11-architecture-review.md), the owner's [triage and response](2026-11-11-review-response.md), and the [roadmap](../ROADMAP.md). This note records what was accepted, what was decided while building P0-1 to P0-4, and what is left open. What runs and how it was verified is in [Implementation Status](../IMPLEMENTATION-STATUS.md).

## The response is accepted, including its corrections to the review

| Correction in the response | Decision |
| --- | --- |
| GW-5 overstated: streaming already recorded disconnects after headers | **Accepted.** The real gap was before headers and on non-streamed answers. Fixed by running every handler detached, rather than by rewriting streaming. |
| The 3.3× figure in the review's cost model depends on pricing cache writes at 1.25× | **Accepted.** At 1.0× the figure is 2.7×. Either way it is a synthetic illustration, not a measurement, and is not repeated as a finding. |
| No hashing of prompts to guess a session | **Accepted.** Only an explicit header pins a session. Clients that send none get request-scoped routing and no continuity promise. |
| GW-1 must block by quota scope, not every account of a provider | **Accepted,** with a default (below). |
| GW-7 (fallback before the first token) should be held | **Accepted.** Fallback stays "before headers"; buffering the start of a stream is not built. |
| GW-8 (persisting Gemini signatures) needs care: encryption, TTL, scope | **Deferred.** Signatures stay in memory. After a restart, earlier tool calls use Google's placeholder, as before. |

## Decisions taken while building

1. **The default quota group is the provider kind.** Two keys of the same provider are assumed to share limits until the user marks one as **Own organization**. The default is conservative because a false "independent" can break a provider's terms, while a false "shared" only costs a fallback. The rule is scoped as the response asked: after 402, 403 or 429, *the same connection's other models* may still answer (quotas are often per model); only *other connections* of the group are skipped. A 401 marks the whole connection, so its own models are skipped too.
2. **Ambiguous means anything after the request was sent.** A refused or failed connection never reached the provider and may fall back. Any other transport error (read timeout, reset) stops with an Unknown cost. The read timeout went from 300 s to 600 s, so long non-streamed reasoning answers are not cut off.
3. **The reservation estimate is pessimistic:** the serialized prompt at about four characters a token, plus `max_tokens` (or 4096) at the model's output price. It is settled from real usage, so the effect is only on admission near the limit. Free and local targets reserve nothing.
4. **An unknown price defaults to Block, but only for clients that have a limit.** Clients without a limit are unaffected. A client can switch to Allow; the UI says what that means.
5. **No OpenAI → Anthropic translation yet.** Such targets are skipped with a reason rather than translated untested, per "translation only on tested pairs".
6. **Cache markers survive translation only where documented:** OpenRouter, flagged per preset. Every translated response lists what it dropped.
7. **`x-smarttaskrouter-attempts` counts upstream calls,** not route positions: skipped targets cost nothing and are not counted.
8. **The binary stays `gateway serve`.** `str start` remains the planned packaging name, as the response decided.

> **Revised after the [counter-review](2026-11-11-architect-counter-review.md):** decision 1 now holds across requests, not only within one, and its switch is "Independent quota confirmed"; decision 2 adds HTTP 408, 504 and 524 to "ambiguous"; decision 3 was wrong to call the estimate pessimistic. It is an estimate that can be exceeded, and it is now labeled that way. See the [response](2026-11-11-counter-review-response.md).

## Not done, and why

- **P0-5 (`auto`)** has not started. It depends on P0-1 to P0-4 and on a sourced model catalog.
- **The parity benchmark** (direct versus gateway on the same Anthropic endpoint) needs an Anthropic key and the owner's go-ahead for paid traffic. It is the gate before any savings claim.
- **Data-use terms per preset (GW-10)** are not started. Stating terms needs a source and a date per provider and tier, and none was verified tonight.
- **Tray and single-instance lock across app and CLI (GW-9)** are not started.
