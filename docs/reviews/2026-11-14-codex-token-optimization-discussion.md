# Fifth discussion with Codex: token optimisation, features and UI/UX, 2026-11-14

> **Status:** Record and plan · nothing built in this round · the study it discussed: [token optimisation study](../archive/research/2026-11-14-token-optimization-study.md) (corrected after Part A) · earlier rounds: [fourth](2026-11-14-codex-providers-web-guard-review.md), [third](2026-11-13-codex-screens-savings-npm-review.md) · order: [ROADMAP](../ROADMAP.md)

**How it was run.** Codex CLI 0.163.0 (ChatGPT plan), `codex exec --sandbox read-only --ephemeral`, given the study, the code and three screenshots of the browser preview (Usage, Home, Providers). It re-summed the CCHT benchmark files, read OmniRoute's, Headroom's and RTK's documentation and the independent replay, and the Anthropic, OpenAI, OpenCode and Codex docs on caching and headers. No model request, no money spent.

**Its verdict.** Build cost visibility, launch sessions and route-change diagnostics first; keep any compression in shadow. Once prompt caching works, compressing what agents read saves little (an independent replay of 500 Claude Code sessions: 3.7%); the bill is mostly cache writes and output.

## A. Corrections to the study (all applied)

| # | Codex | Applied |
| --- | --- | --- |
| 1 | The token totals reproduce, but the percentages are a price model, not "shares of the bill"; Sonnet 5.5 reads cost 0.05×, not 0.1× (about 15% / 46% / 40%); the files hold every attempt, the report only the last per case | Relabelled as "token-weighted cost share under an illustrative price model", with both notes |
| 2 | "OpenCode's uncached input cost nothing" is unsupported: Flash-Lite cost $0.20 in that run | Removed |
| 3 | `pygg run claude` already sends a fresh session per launch (`cli/run.rs`, `launcher.rs`); the OpenCode and Codex setups read `PYGG_ATTEMPT_ID`, set only inside tasks; Home's manual OpenCode example uses the literal `opencode` | Corrected; T3 is now "finish it for OpenCode and Codex" |
| 4 | The switches followed 18 Gemma 429s and 5 Flash-Lite 404s; the records show targets tried, not that each switch was a cold restart | Corrected |
| 5 | The replay ran Headroom directly; RTK and Caveman are estimates; no task was re-run | Added |
| 6 | Headroom's prefix protection is its default cache mode, not every mode; OmniRoute's doc gives two different defaults; Pygg's shadow counter measures repeats within one request and knows no cache position; a dropped cache hint is only a response header | Added |

## B. Features, in Codex's order

| Rank | Feature | Decision | Smallest useful version |
| --- | --- | --- | --- |
| 1 | **T1 Token mix** | Build | Usage shows new input, cache reads, cache writes and output, as tokens and, where the price is known, as estimated cost; "Input served from cache: 89%"; coverage ("complete token detail: 52 of 55 attempts"). No migration for tokens; a price snapshot per event later for trustworthy historical cost |
| 2 | **T3 Launch sessions** | Finish | A fresh id per launch for OpenCode and Codex through `pygg run`, fed to their existing header mappings; fix the manual example; call it a **launch session**, never "your conversation" |
| 3 | **T2 Route changes** | Build, conservatively | Changes between answered targets inside identified sessions, apart from first-answer selection; a request id and attempt number per chain. Label "Cache writes after route changes", never "switching cost": the extra cost is a counterfactual |
| 4 | **Cache-hint diagnostics** | Build with T1/T2 | Per attempt: preserved, translated, dropped, unsupported or unknown, with the field; in request detail and Providers |
| 5 | **T5 Live-zone filter** | Shadow only | Ruleset-versioned observations of ANSI and progress noise in new tool output, priced by session replay (first appearance plus observed later reads, less extra prefix misses, negative allowed). No active filter: the same rewrite would have to be reproduced every time the client resends its history |
| 6 | T4 Cache-aware fallback | Defer | Until T2 has evidence; "same provider family" does not share a cache |
| 7 | T6 Concise output | Defer | Later as a user-visible, client-owned instruction, judged on whole tasks |
| 8 | T7 Semantic response cache | Reject | A similar coding prompt can mean a different repository state |
| 9 | T8 History dedup, CCR markers, context as images | Reject | They rewrite an external agent's history |

Also: RTK's engine is BUSL-1.1, so it is not copied into MIT Pygg; an optional user-installed RTK is another matter. Provider caching is **not** Pygg's saving: if shown, it is "Estimated cache-read discount · the provider billed these input tokens at its cached-input rate", apart from the routing hero (Headroom corrected its own dashboard the same way).

## C. UI/UX

**Usage: Summary and Requests.** A view switch inside API Costs, with the range shared and filters carried across; a summary row opens Requests filtered. Summary: the routing comparison, known cost, the cost chart, **Token mix**, one **Routing continuity** row, and disclosures for the comparison method and tool-output observations. Requests: filters, the ledger, the selected request with its token categories, fallback chain, session scope and cache-hint handling. By Model and By Client move into drill-downs; the Repeated Tool Output card becomes a collapsed observation.

**Token mix** (Summary, under the cost chart): "See which tokens were new, read from cache, written to cache, or generated." Four figures, one stacked bar with Tokens / Estimated cost, "Input served from cache · complete token detail N of M", a disclosure by client and model. States: no usage yet; detail missing for N attempts; provider reported no cache detail; Unpriced with Set Price. Category colours with text labels; strong green stays for the routing comparison. Home gets only "View token mix".

**Launch sessions** (CLI Tools, in each tool's card): "Routing session · Per launch" or "Not supplied by this launch", with Copy Launch Command; for an editor, "This editor launch shares one routing session across its conversations." Request detail: session scope (Launch, Conversation, Not supplied), route (kept, first selection, changed), a short copyable reference. Never "cache optimized".

**Routing continuity** (Summary, one row): "2 route changes · 18 fallback attempts · sessions identified on 74% of attempts · Review Changes", with "Route changes compare answered requests within identified sessions; fallback attempts can happen before any answer." Requests then groups by session: time, from → to, reason, cache writes afterwards, known cost, and "Usage after a change is observed; the extra cost caused by the change is not known."

Not copied: OmniRoute's engine taxonomy as navigation and percentages of eligible payloads shown as whole-bill savings; overlapping savings totals across cards.

## D. Plan

1. T1 with the Summary / Requests split. **Done 2026-11-14**: Token Mix (tokens or the recorded known cost split by current prices) in Summary; Requests as its own view. Not done: per-event price snapshots (historical splits follow today's prices, and say so), Home's "View token mix" link, and folding Repeated Tool Output into a disclosure.
2. T3 for OpenCode and Codex launches. **Done 2026-11-14**: `pygg run opencode` and `pygg run codex`, `launch-` session ids, the scope in a request's detail, the manual example fixed. Not done: the session-coverage states on CLI Tools ("18 of 30 attempts included a session ID").
3. T2 with request chains and cache-hint diagnostics (**done 2026-11-14**, schema v30: request id and attempt, billing and route kind as sent, cache hints, Usage → Routing and the attempt chain); then decide from T5's shadow observations whether any active experiment is worth it.
