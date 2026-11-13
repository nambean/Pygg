# Fourth review by Codex: provider checks, the health strip and the web guard, 2026-11-14

> **Status:** Record · Codex's findings in short, and what was done · earlier rounds: [first](2026-11-12-codex-ux-review.md), [second](2026-11-12-codex-review-round2.md), [third](2026-11-13-codex-screens-savings-npm-review.md) · built: [Implementation Status](../IMPLEMENTATION-STATUS.md)

**How it was run.** Codex CLI 0.163.0 (ChatGPT plan), `codex exec --sandbox read-only --ephemeral`, given five screenshots of the browser preview (Providers with a detail open, Providers first run, Home, Usage, Endpoint; dark, demo data) and the source. It read the three earlier reviews, 9router's `ConnectionRow.js` and OmniRoute's monitoring guide; it found no dot-strip formula in 9router's source, so Pygg borrows the look, not a claimed algorithm. Static review: Codex ran no tests and contacted no provider. Every finding was checked against the code before it was acted on, and every fix has a test.

## A. What was built on 2026-11-13

| # | Finding (Codex) | Answer |
| --- | --- | --- |
| 1 | A web request is written to the guard only after it ends; a crash loses its weight and rest; an unreadable record lets the request through | **Done.** `store::web_guard_reserve` writes the weight and a first rest before the site is touched; the end updates that row. A record Pygg cannot read or write refuses with 429 (`GUARD_UNREADABLE`) |
| 2 | The guard's "account" is a connection: one account signed into two connections gets two budgets; deleting a connection forgets its history | **Later.** The owner's two ChatGPT connections are two accounts, so a site-wide budget would halve them. An explicit "same account" grouping, and keeping the history of a deleted connection for a day, are the next guard change |
| 3 | A request waiting in line does not read the connection again: it can go out after the user turned it off or a suspension was recorded | **Done.** `admit` runs under the connection's lock and again after any wait: the connection on, the warning acknowledged, the day's count, the shares and the back-off |
| 4 | The daily limit counts the ledger before the lock, and the ledger is written after the slot is released; every 429 is excluded, including a site's quota notice | **Done.** The daily count is the guard's reservations, read under the lock; what Pygg refused itself never gets one |
| 5 | A suspension notice that appears while the reply is read is not seen | **Done.** The reply reader reports a notice outside the replies; it stops the request, records the suspension and turns the connection off. A notice the user quoted in the prompt is not taken for the site's |
| 6 | A sign-in check skipped because a request held the session was recorded as a fresh check | **Done.** `Service::check` returns `Checked::Seen` or `Checked::Skipped(why)`; only a real look is recorded; Check Sign-In shows why it did not run |
| 7 | Check Sign-In ignores the guard's rest and back-off; Check All loads every site once an hour | **Done.** A sign-in check waits out the rest and back-off, rests the account after it, and a check that finds trouble backs it off. Check All leaves web sessions out entirely; each has its own Check Sign-In |
| 8 | Fallback can send a prompt that already reached one site to another web session | **Done.** Once the site's Send was pressed, a failed answer carries `x-pygg-web-sent`, and the gateway skips every further web target for that request ("Pygg sends a prompt to one site at most"); an API target may still answer under the usual retry rules. Test: without the rule the second account got the prompt |
| 9 | The guard uses wall-clock time; a clock change can release capacity early; the day is a UTC day | **Later.** Documented as a UTC day. A clock-jump hold is low risk next to items 1–8 |
| 10 | The score mixes model-list checks with answers: two good checks and one failed answer score 67 when every answer failed; a busy hour outweighs a quiet hour's outage | **Done.** An hour with requests is judged by them alone; an hour without, by its checks. Each hour gives its own share that went well, weighted by recency, so volume and Check Now cannot move the score |
| 11 | A model list read proves less than it seems; any JSON without a list counts as success | **Done in part.** An answer without a `data` or `models` list is a failure; a provider that keeps no list (404, 405) is reachable, with "the key was not checked". A list success still clears only "unreachable", never a rejected key or a cooldown |
| 12 | Timer, Check Now, Check All and Refresh Models can check one connection at once; a check with the old key can land after the new key's | **Done.** One check at a time per connection; each reads the connection afresh when its turn comes. Refresh Models goes through the same turn |
| 13 | Checks share the 600 s generation timeout and run one after another; the loop ignores stop mid-round | **Done.** 20 s for a list read, four providers at once, and the loop stops mid-round. Backing off a local runtime that is off is later |
| 14 | Failure classes come from text ("Not sent"); a busy web session's refusal lacked it | **Done in part.** The busy refusal now says "Not sent". A stored failure class (local, auth, quota, transport, upstream, malformed) is later, with the per-event facts of Part B #2 |
| 15 | A row from the future (a clock set back) lands in the newest hour | **Done.** Both queries stop at `now`; tested |
| 16 | The dots explain less than the score implies; "as long as a person would read it" overstates a fixed rule | **Done.** The strip's label gives the score, how many went well and failed, and the hours with failures; the Health text says how hours are scored; the guard note says the wait "slows Pygg down, and does not make an account safe" |
| 17 | The demo's checks change nothing; its check sat in the wrong hour | **Done.** Demo checks are kept per connection and counted by the same rules; Check Now and Check All add to them; the demo's Ollama check fails, so the failed state shows |
| — | The weight's doc said "each started block", the code counts full blocks | **Done.** The doc says full blocks (1,999 characters weigh 1, 2,000 weigh 2), as the UI and the existing test say |

## B. What to build next (Codex's ranking)

Codex: fix the guard first (done above), and do **not** route by the health score: Pygg's cooldowns, outage probes and auto holds already decide routing, and an aggregate score would be a second, weaker authority.

| Rank | Proposal (Codex) | Smallest useful version | Schema |
| --- | --- | --- | --- |
| 1 | **Auto readiness on Providers** · **Done 2026-11-14** | "2 models eligible for coding auto · 3 missing routing facts · 1 cooling down", from `auto::plan`'s own exclusions, each linking to its models. Built as proposed: the planner tags each exclusion (`facts`, `unavailable`, `policy`, `web`); a line under the provider list, a count per row, a verdict per model and four model filters | None |
| 2 | **Billing and routing kept on each usage event** | `billing_at_request` and a structured `routing_code` recorded at admission; old rows say "not recorded", never today's settings | One migration |
| 3 | **A logical request id across fallback attempts** | One id at gateway ingress, attempt numbers, a response header; Usage groups attempts and says "Answered after fallback" | Same migration as #2 |
| 4 | **Usage: Summary and Requests** | Two views; CLI Tools opens Requests filtered by its client; filters run in the query, not on the loaded page | None |
| 5 | **Quota already observed, shown** | The connection's reported limits and account balance beside its detail, "observed 3 minutes ago", kept apart from Pygg's client budget and the web guard | None |

Not to build, in Codex's view: routing by the health score; OS notifications before deduplicated in-app alerts; a universal quota tracker for plans with no trustworthy source; any automatic testing, quota discovery or repeated sign-in polling of web accounts; more paid benchmarks for these slices (mock servers and seeded stores cover them).
