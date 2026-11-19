# Decisions

> **Status:** Current · architecture decision records (ADR-001 to ADR-039) · reviewed 2026-11-15

Each record says what Pygg decided, why, and what it costs. Numbers never change, because code and other documents cite them; a record that was superseded or withdrawn keeps one line that points to what replaced it. Delivery order is in [ROADMAP](ROADMAP.md), what is built in [Implementation Status](IMPLEMENTATION-STATUS.md), and the screens in [design/SCREENS.md](design/SCREENS.md).

## ADR-001 — Windows first, macOS next

**Decision:** The desktop app is built and tested on Windows first. macOS must build but is not yet release-tested; Linux is best effort. The headless CLI ships for Windows, macOS and Linux (ADR-037).

**Why:** Development happens on Windows, and DownFetch follows the same order.

## ADR-002 — Local first

**Decision:** Everything runs on the user's machine. Credentials, the ledger, task history and workspaces never leave it. There is no account and no hosted service in the core path.

**Why:** The app holds API keys and source code. Keeping it local keeps the trust boundary small.

## ADR-003 — Route at task and session boundaries, never mid-session

**Decision:** Routing decisions happen when a task, a step or a session starts. The gateway is the transport and enforcement layer: it never switches models silently inside a session. A session stays on the target that answered it, and fallback happens only on hard errors before any output has reached the client. Building the gateway before the orchestrator is ADR-016.

**Why:** Switching mid-session breaks prompt caches, tool-call formats and reproducibility, and a single request carries no task context.

## ADR-004 — A localhost gateway, loopback only, with client tokens

**Decision:** The gateway listens on `127.0.0.1`. Every request needs a scoped client token (an "API key" in the app) with its own limits. Binding beyond loopback requires an explicit user setting.

**Why:** SDKs and agent CLIs reach a model only through a base URL, so a local HTTP endpoint is unavoidable. This is a deliberate difference from DownFetch, which avoids a localhost API.

**Cost:** A new local attack surface, which tokens and loopback-only binding contain.

## ADR-005 — One Tauri crate with modules; same stack as DownFetch

**Decision:**
- Tauri 2 with one Rust crate (`apps/desktop/src-tauri`) holding the core, the gateway and the agent runner. The window is the Cargo feature `app`, so the CLI builds without it (ADR-037).
- React 19, Vite and TypeScript with plain CSS tokens; no Tailwind or shadcn.
- A pnpm workspace. No Node sidecar.

**Why:** DownFetch already ships this stack, so tooling and lessons carry over. One process avoids sidecar packaging.

**Cost:** Rust has fewer multi-provider LLM libraries than Node or Python, so provider adapters are written in-house and tested against recorded fixtures.

## ADR-006 — Rule-based routing

**Decision:** `auto` ranks models with fixed rules: a curated tier table T0–T4 (`curated:v1`, `gateway/tiers.rs`), a tier from the list price for models the table does not know (`price-band:v1`, ADR-031), a minimum tier per client, and hard eligibility filters. Minimum tiers per task category and complexity are not built. Ranking by measured cost per success comes later, and only above a minimum sample size.

**Why:** There is no measured quality data to start from, and invented quality scores would mislead.

**Consequence:** The UI says "lowest eligible tier under your policy", never "cheapest".

## ADR-007 — Deterministic checks gate the cascade

**Decision:**
- A task may start at a cheaper step and move to the next only on a deterministic check (typecheck, tests, build, schema).
- An LLM review can advise but never passes a step.
- Without an approved check there is no automatic cascade. The user reviews the result, and keeping it records it as **Manually Accepted**.

**Why:** A cheap model's output is only safe to accept when a machine can verify it.

## ADR-008 — A Git worktree per attempt; Git repositories only

**Decision:** Each attempt runs in its own Git worktree created from the task's recorded base commit. Failed attempts are discarded. Workspaces must be Git repositories; folders that are not are not supported.

**Why:** Isolation makes trying a cheap model first harmless. Snapshot isolation for other folders is possible later but not worth its risk now.

## ADR-009 — Keep as Branch is the result handoff

**Decision:** A result is kept by committing it to a new branch `str/<slug>-<short-id>` from its base commit, without switching the user's branch. Export Patch and Discard are the alternatives. Applying to the working tree, with drift detection, is not built.

**Why:** Developers already know how to review and merge a branch, and it never touches the working tree.

## ADR-010 — Approvals are durable and cover exactly one attempt

**Decision:**
- An approval names one attempt and a spending cap.
- It survives a restart and is invalidated by a change of policy, catalog, budget or the step's estimate.
- Nothing is approved automatically, and no API can approve (ADR-027).
- Notifications never include task names or paths, and never carry an approve button.

**Why:** Spending must be a deliberate, specific decision.

## ADR-011 — Billing types and per-fact provenance

**Decision:**
- Billing is `Free`, `Local`, `Subscription`, `Paid` or `Unknown`, set per connection and per model.
- Every price, quota and capability value carries its own source and time.
- Unknown is never treated as free or shown as `$0.00`.
- Subscription use is measured as quota consumption, never as money.
- A list price from a catalog gives an estimated cost, never an actual one (ADR-031).

**Why:** Most providers do not report remaining quota, and false precision would mislead the user about spending.

## ADR-012 — Traffic-light window controls on every platform

**Decision:** The app uses DownFetch's traffic-light window controls on Windows and Linux too.

**Why:** Consistency across the team's apps.

**Cost:** A departure from Windows conventions. The release checks for it (snap layouts, scaling, keyboard use, screen readers) are listed as D12 in [design/SCREENS.md](design/SCREENS.md); scaling and screen readers are still open.

## ADR-013 — Benchmark before claiming savings

**Decision:** Savings are advertised only from a benchmark of reproducible coding tasks with deterministic checks that compares always-T4, always-T2 and a check-driven cascade ([benchmark](../benchmark/README.md)). Paid runs stay within the budget of ADR-028. Running this benchmark before building the gateway is superseded by ADR-016.

**Why:** The product's core idea is unproven until measured.

## ADR-014 — Sample data is always labeled

**Decision:** The desktop app shows only real data. The browser preview (`pnpm dev`) runs on an in-memory demo (`src/lib/demo.ts`) under an on-screen label; `?empty` shows the first-run state. A screen that is designed but not built says so instead of showing empty data.

**Why:** An empty list looks like a real, empty configuration, and unlabeled sample numbers look like measurements.

## ADR-015 — Commit the pnpm lockfile

**Decision:** `pnpm-lock.yaml` is committed. DownFetch ignores its lockfile; this project does not.

**Why:** Reproducible installs matter more than lockfile churn for an app with a release pipeline.

## ADR-016 — Build the gateway first

**Decision:** The multi-provider gateway comes first, in the spirit of 9router, and the task engine is a layer on top of it:
- Clients the user already has (Codex, Cursor, OpenCode, Claude Code, scripts) point at one local base URL. The gateway serves Chat Completions, Anthropic Messages and the Responses API (ADR-033).
- Each connection is a provider account; its key is kept in the OS credential store.
- A **combo** is a named, ordered list of models. The gateway falls back down the list on hard errors, only before any output has reached the client (ADR-003).
- Every request is written to the usage ledger, and each cost carries its provenance (ADR-011).

The task engine runs a fixed ladder of agent steps per task (AGENTS.md). A task is not analysed into a proposed plan.

**Why:** Existing clients can use the gateway on day one, and the task engine needs it anyway.

**Cost:** Outside tasks, routing is per request, chosen by the client through the model name (`<connection>/<model>`, a combo or `auto`).

## ADR-017 — Universal conversation memory

Withdrawn by ADR-028: there is no app-owned conversation store or memory. Not built.

## ADR-018 — Cache-aware model affinity for Auto Chat

Withdrawn by ADR-028 (no Auto Chat). Keeping gateway clients on one model for their cache is ADR-036.

## ADR-019 — Gateway-first order

Merged into ADR-016 (gateway first) and ADR-013 (the benchmark gates savings claims).

## ADR-020 — Entitlement tiers per account and model

Proposed, partly built: billing is set per connection and per model with its source (ADR-011, ADR-032). Providers views grouped by tier and a per account-model entitlement schema are not built.

## ADR-021 — Headless CLI and npm launcher

Built as ADR-037: one Rust core for the app and the headless `pygg-cli`, and an npm launcher with per-platform binaries instead of a second, Node gateway.

## ADR-022 — Safe setup of CLIs and IDEs

**Decision:** Pygg changes a tool's configuration only when the user asks, after showing what will change. It backs the file up first, gives the tool a client token of its own, checks the result, and can undo it. Compatibility is claimed only for client APIs that were tested. A subscription sign-in is never routed through an unrelated client. Built as one-click setup for Claude Code, Codex CLI and OpenCode (`integrations.rs`, AGENTS.md).

**Why:** A tool's configuration is the user's, and a failed setup must leave it as it was.

## ADR-023 — Multi-account rotation

Built as ADR-034 (pools of connections) and ADR-038 (key rings). Its safety rule stands in both: rotation never gets around bans, quotas, terms or shared limits, and never changes an active attempt, tool call or stream.

## ADR-024 — What to adopt from OmniRoute

**Decision:** An idea from OmniRoute is adopted only when it meets these gates: entitlements the provider verifies, no privacy or budget bypass, safe rollback and a measured cost per success. Not adopted: multi-model fusion, dozens of surfaced strategies, credential, TLS or browser impersonation, lossy context compression without independent benchmarks, and claims taken from upstream benchmarks. Related decisions: ADR-031 (explainable filters), ADR-034 and ADR-038 (accounts), ADR-036 (compression only in shadow).

**Why:** Pygg builds on its own gateway and rules rather than copying another router.

## ADR-025 — The gateway must not raise cost or break provider rules

**Decision:** Before new router features, the gateway keeps cost and provider rules safe:
- native, same-format Anthropic passthrough with prompt-cache parity;
- fallback only on errors that allow it;
- atomic budget and usage accounting, with reservations;
- explicit session affinity (`x-pygg-session`), never derived from prompt content;
- explainable, rule-based `auto`.

External clients own their prompts, history and compaction. The gateway never inserts memory or rewrites a client's prompt; the one addition is ADR-036's cache marker on translated turns.

The first users are developers with metered API workloads, because their savings can be measured. The long-term aim includes everyday users. No cash saving is claimed for fixed-price subscription plans.

**Status:** Built and tested against fake providers; native Anthropic parity was measured against the real endpoint (Implementation Status).

## ADR-026 — Official agent CLIs with their own sign-in; no bridges

**Decision:** To use the subscriptions and free tiers people already have, Pygg runs the vendors' own agent CLIs (Claude Code, Codex CLI, Gemini CLI, OpenCode) headless, each with the sign-in the user gave it, in a worktree per attempt (ADR-008). Pygg never reads, stores or forwards that sign-in, never exposes it to other clients through the gateway, and does not bridge cookies or other products' OAuth tokens, whatever their licence. ADR-035 narrows this for sites with no API: the user signs in to them in a Pygg window.

**Why:** Bridges depend on terms the user did not agree to with Pygg and break without notice. The official CLIs are supported, and their cost stays honest.

**Cost:** Pygg cannot see which plan an agent uses. An agent step's cost is recorded as unknown, with the agent's own estimate in its source. See [AGENTS.md](AGENTS.md).

## ADR-027 — Tasks start from the user's IDE agent; the app monitors

**Decision:** Tasks start where people already work: their coding agent calls Pygg's MCP server (`<app> mcp`), which forwards to the gateway's task API. The app runs tasks and is where they are watched, paid steps are approved and results are reviewed. New Task in the app stays as a manual fallback. A client needs the user's **May start tasks** permission, sees only its own tasks, and cannot approve a paid step (ADR-010).

**Why:** Most people will not treat a router app as an IDE.

This replaces the parts of [design/SCREENS.md](design/SCREENS.md) that assume tasks are composed and planned in the app first. See [AGENTS.md](AGENTS.md#the-mcp-server-and-the-task-api-experimental-on-hold).

## ADR-028 — MIT licence, a $20 benchmark budget, and no Auto Chat

**Decisions:**
- **Licence:** Pygg is open source under the MIT licence ([LICENSE](../LICENSE)). MIT-licensed code from other projects may be ported with its copyright and licence notice kept, listed in `THIRD_PARTY_NOTICES.md`. ADR-026 still applies, whatever another project's licence.
- **Benchmark budget:** paid benchmark runs cost at most $20 in total, enforced with `bench --budget-usd`. Free tiers, local models and agents on subscriptions do not count against it; their runs are reported apart.
- **No Auto Chat in the app.** The app is not a place to compose chats or tasks (ADR-027). There is no app-owned conversation store, memory screen or per-turn model choice (ADR-017 and ADR-018 withdrawn). What tasks need survives: the bounded handoff from a failed attempt to the next step (ADR-030), and session affinity in the gateway.

**Why:** Open source fits a local tool that holds keys and code. A fixed budget keeps the benchmark honest about what it can claim. Auto Chat would make the app a chat client.

**Chat from the command line (owner's decision, 2026-11-18).** `pygg chat` and `pygg ask` talk to any connected model through the gateway from a terminal, like other AI command-line tools, with Pygg's routing, fallbacks, web-session guard and ledger. They are clients of the gateway with an API key of their own ("Pygg CLI (ask)"). A conversation lives in the command's memory and is forgotten when it ends: there is still no conversation store, memory screen or chat in the app.

## ADR-029 — Quality signals in Pygg's own ledger, in TensorZero's shape

**Decision:** Pygg does not adopt TensorZero: it is archived by its maintainers, needs Postgres or ClickHouse and a second gateway with its own model names, stores whole conversations and sends usage analytics by default. Pygg borrows its data model instead: **metrics** (name, boolean or float, the level they describe, whether more or less is better) and **feedback** rows keyed to a request, an attempt or a task, with their source. Migration 0015 adds both and seeds `checks_passed`, `result_kept`, `retried`, `cancelled`, `declined` and `rework_minutes`. The task engine records them as they happen, and `tasks::quality` gives the known money per kept result, counting every paid step.

**Rules:** An unknown signal has no row, never a 0 or false. Pygg never infers quality from conversation content (ADR-025). Request-level feedback comes only from a client that sends it.

**Why:** One user, one machine, SQLite (ADR-002): a server stack is out of proportion, and an unmaintained gateway in the path is a security cost. Keeping the shape leaves an export open.

## ADR-030 — Task handoff within ADR-028; portable context deferred

**Decision:**
- A task's next step may be told what earlier attempts did. The handoff holds only for that task: bounded, built from what Pygg recorded, shown to the user, never kept across tasks, never shown as memory, never sent through the gateway.
- Modes, fixed in the task's policy: **`tail`** (the default: the failed check's command and the end of its output), **`none`**, and **`evidence`** (every earlier attempt, about 6,000 characters at most; `handoff.rs`).
- `tail` stays the default because the A/B did not show `evidence` saving tokens, time or money, or changing the pass rate ([report](../benchmark/reports/2026-11-12-h1-handoff-ab.md)). Whether it helps when a first step reaches its checks and fails is untested.
- **Portable context between external clients stays deferred.** MCP task dispatch carries a task description, not a typed context package. Reopening it needs evidence that the handoff helps, and a new decision on ADR-028.
- An adaptive router is evaluated only on benchmark replays, never as a live shadow.

**Why:** It is what ADR-028 already keeps, made richer only where it can be measured.

## ADR-031 — The user enters a key; the app looks up the rest

**Decision:**
- **A reference catalog fills what a provider's own list leaves out.** Pygg reads [models.dev](https://models.dev), keyed by each provider's own model ids, and fills context, maximum output, tool and image support and the list price only where they are unknown. The provider's list comes first, and what the user set is never touched. A connection is matched to the catalog by its API address first, then by its kind; local runtimes are never matched. Every fact names its source, and a price reads "models.dev list price".
- **A catalog list price is a list price** (ADR-011): it gives estimated costs, and the user's own price replaces it.
- **Every model gets a tier.** `curated:v1` comes first; otherwise the list price band (`price-band:v1`: output under $1 per million tokens is T1, under $5 T2, under $20 T3, above T4), and `auto` says "from its list price". A $0 price gives no tier. A tier the user set stays theirs.
- **Claude models take tools** (`curated:v1`), since Anthropic's list does not say so.
- **A connection set as free does not make a priced model free** (ADR-032).
- **When nothing is eligible, the refusal groups its reasons** by connection and reason.

**How it runs:** at start-up in the background and after a model list refresh; at most one download a day, cached as `reference/models.dev.json` for offline use. The download is a GET of a public file and carries nothing about the user. `gateway lookup` applies it by hand.

**Why:** Users cannot be expected to know model presets. They enter a key, and looking things up is the app's job. On a real store of 584 models, the catalog described 466, and `auto` with tools went from no eligible model to a ranked route.

## ADR-032 — A connection kept for its free models has only those on

**Decision:**
- **Two kinds of free.** For most free tiers the account is free (a trial key, Gemini's free tier): every call costs nothing within limits. For OpenRouter and Z.AI, the free part is a set of models listed at $0 next to paid ones; their presets say so (`free_models`).
- **On such a connection set as free, only models listed at $0 are on and free.** A priced model is paid; one without a price is unknown, never free (OpenRouter's own routers list none and may pick a paid model). Pygg turns both off (`enabled_source = 'free-connection'`), and the Models table says why.
- **The user's switch is theirs.** A model the user turns on stays on, billed as paid or unknown. A model Pygg turned off comes back when the reason ends.
- **The ledger follows the same rule** as ranking: a paid model on a free connection is not recorded at $0.
- **Missing free models are added from the catalog** (ADR-031) for Z.AI only, whose list leaves out its free Flash models.

**Why:** A user who connects OpenRouter for its free models expects only those to be on, and to turn on a paid one deliberately.

## ADR-033 — The gateway serves the Responses API, translated through Chat Completions

**Decision:**
- **`POST /v1/responses` is a third surface on the same pipeline** (`gateway/responses.rs`): authentication, routing, `auto` and combos, limits and holds, sessions, health, quota and the ledger work as for the other two.
- **One new translator, Responses ↔ Chat Completions.** An Anthropic-format connection is reached through the existing Chat Completions → Messages translator, so there is one translator per pair of formats.
- **Native passthrough only where a preset says so** (`Preset::serves_responses`): OpenAI.
- **Custom tools** (Codex's `apply_patch`) cross as functions with one string parameter, `input`, carrying the grammar in the description. **Namespace tools** are flattened as `<namespace>__<name>`. Both are matched back by name from the request's own tool list; nothing is kept between requests.
- **Reasoning** comes back as a `reasoning` item, and goes back as `reasoning_content` to providers that take it. Another provider's encrypted reasoning, hosted tools and top-level fields that are not carried are named in `x-pygg-dropped`. Nothing is dropped silently.
- **Pygg stores no responses.** `previous_response_id`, `conversation` and `background` are refused with 400; the client sends the whole conversation.

**Why:** Codex CLI speaks only the Responses API, so without it Codex could not use Pygg as a gateway.

**Cost:** Hosted tools (web search, file search, code interpreter) are unavailable on translated connections. A model Codex does not know runs with Codex's fallback metadata; the `pygg` profile names a catalog Pygg writes, with each model's context size and `apply_patch` and Codex's own default instructions (Apache-2.0, bundled by the owner's decision of 2026-11-16; AGENTS.md). One-click setup is "Codex: models through Pygg".

## ADR-034 — Pools of separate accounts

**Decision:**
- **A pool groups connections of one provider kind, each a separate account the user holds,** with its own key. A connection joins only after the user confirms it has its own provider quota. Several keys of one account cannot be pooled: that would only get around the account's limits.
- **A pool's name is a routing prefix.** `<pool>/<model>` tries the members in the user's order, skipping disabled ones, and moves on for 401, 402, 403, 429 and Anthropic's out-of-credit 400. A session stays on the member that first answered it. The ledger names the connection that answered. `GET /v1/models` lists `<pool>/<model>`. The name may not be a connection's id, a combo's name or `auto`.
- **Web sessions** (ADR-035) of one site may pool, each confirmed as another account; such a pool sends to the account used least today, and the web guard sends to a resting account last. Agents' sign-ins and cookies of other apps are never pooled (ADR-026).
- **A provider's own OAuth flow that issues an API key** (OpenRouter's PKCE) is not a bridge: Pygg keeps an ordinary API key.

**Why:** Accounts a user really holds can share load; one account's keys cannot multiply its quota.

**Cost:** Pygg cannot check that accounts are separate; it depends on the user's confirmation. For several keys on one connection, see ADR-038.

## ADR-035 — Web-session connections for AI sites with no API

**Decision:** A connection kind, **Web session (unofficial)**, for AI sites that offer no API. It is a provider like any other: chosen in Connect, with models, health, limits and ledger entries, listed in `GET /v1/models`, and reached on all three surfaces as `<connection>/<model>` or through a combo. Only the transport (a signed-in web page) and the safeguards differ. Recipes exist for Muse, ChatGPT, Perplexity, DeepSeek and Grok. It narrows ADR-026 for this kind only: no cookie import, no other products' OAuth tokens, no TLS impersonation, no limit evasion.

**Why:** Some services exist only behind a website sign-in. The user carries the account risk, knowingly and per connection; the sign-in never leaves the machine.

**How it works:**
- **One Pygg-owned webview per connection** (label `web-<connection id>`), with its own WebView2 profile folder (`websessions/<connection id>`). The user signs in on the site's own page; the profile keeps the session like a browser. Nobody copies cookies, and Pygg never holds them. The window opens hidden when needed and closes after 10 minutes unused. It has no Tauri capability, so the site's scripts cannot call Pygg.
- **Its own profile, never its own fingerprint.** Pygg never spoofs or randomises the user agent, client hints, canvas, fonts, screen or similar, and never hides that a page is driven. Every window is the Edge WebView2 it is.
- **Navigation.** While the user has the sign-in window open, it may go to any https address, like a browser. Whenever Pygg types or presses Send, the page must be the site's own host; otherwise nothing is sent and the connection is marked Needs you. Otherwise the window follows the recipe's hosts, and a refused address is named on the connection.
- **A recipe per site** (`gateway/websession.rs`, data in Rust, reviewed like code): start URL, hosts, and selectors for the prompt box, Send, the reply, a "still answering" signal, a sign-in page and a challenge. Scripts only read and fill the page; they never read `document.cookie` or storage. A recipe that no longer matches fails closed as "the page has changed".
- **An OpenAI-format adapter.** Each web session is served as a Chat Completions endpoint on loopback, reachable only with an in-memory token, so the gateway's translators, limits, sessions, combos and ledger apply unchanged. A request is flattened into one prompt and sent in a new chat each time; a stream reads the page every 300 ms.
- **Waiting for a reply** follows the request: the first word is awaited 60 s plus 1 s per 100 estimated prompt tokens, up to 240 s; a page that shows it is answering gets the whole-reply limit, 300 s plus the same extra.
- **Not in the headless gateway** (`pygg gateway serve`): there is no webview, so these connections are skipped with that reason.

**The web guard** (`websession/guard.rs`) decides every request, whoever sends it. It slows Pygg down and never disguises it: fixed rules, no jitter. It was added after DeepSeek suspended an account that had received 24 prompts of 2,000 to 6,000 tokens in 15 minutes at a fixed 10 s pace.
- **Weight:** one unit per request, plus one per started 2,000 characters of prompt.
- **No pasted files:** a prompt over 12,000 characters is refused with 413 `prompt_too_long`.
- **A reader's rest** after a reply: 10 s, plus 2 s per extra unit, plus 50 characters a second of reply, at most 3 minutes. A request that would wait more than 30 s is refused with 429 `web_pace` and Retry-After.
- **Shares:** at most 20 units and 15 requests per rolling hour, 5 requests per 10 minutes, and per UTC day the daily request limit (50 by default, at most 100) and twice that in units.
- **Back-off after trouble:** 5 minutes, doubled for each one in a row, at most 6 hours, with a floor by kind: a usage-limit notice an hour, a request to slow down or a check half an hour, a suspension notice six hours. A short reply asking to slow down is an error (429 `site_slow_down`), not an answer. A clean reply clears the back-off; Providers offers **Resume Now**.
- **A suspension notice turns the connection off** (`RESTRICTED`); it is never read as "signed out".
- **Per-site ceilings** (`guard::Limits`). DeepSeek's are lower: 4,000-character prompts, 8 units and 6 requests an hour, 3 requests in 10 minutes, 30 a day, and a 30 s base rest. No setting raises a ceiling or turns the guard off, and timing outside tests is held at the guard's floors (`Timing::guarded`).
- State is kept in `web_pace` and `providers.web_*`, so a restart does not reset it.

**Safeguards (each tested):**
1. Off by default, labelled "Web session (unofficial)" in its own group in Connect. A new connection stays out of routing until its warning is acknowledged and the user has signed in.
2. A per-connection warning the user acknowledges: the site's terms may forbid automated use, the account can be limited or banned, the site can break the connection, and Pygg acts as the signed-in user. It is stored (`web_ack_at`, `web_ack_version`) and asked again when its text changes.
3. No passwords, no cookie import, nothing secret written. Session data stays in the profile folder, never in logs, diagnostics, the ledger, exports or errors.
4. The profile folder is deleted with the connection, or at the next start if WebView2 still holds it.
5. Never chosen by `auto` or `auto/cheap`: these models have no tier and `tools = false`, and `auto` excludes the kind with its reason. They are used only when named or in a combo.
6. **Billing is Free by default:** the site is used on the user's own plan, never paid per request, so each request is recorded at $0 with that source. The user may set Subscription instead, which counts requests as quota and records the cost as Unknown. Paid and Local are refused.
7. One request at a time per connection; a second waits briefly and is then refused with 429 and Retry-After.
8. Challenges stop the connection. On a CAPTCHA, a sign-in page or a bot check, Pygg marks the connection Needs you (or Not signed in), answers 401 with the reason, and tells the user to deal with it in the window. Pygg never solves, skips or hides from such a check.
9. An API comes first: where a site has an official API (Meta's at `api.meta.ai`), Connect offers it as an API-key preset, and the web-session preset says so.

10. **What the site makes comes back (2026-11-18).** Pictures, videos and sounds the site shows with its reply are taken by address (a `blob:` one copied out of the page's memory), and returned after the reply's words as Markdown links. A file or sound with no address on the page (Muse's PDF card, its audio player before Play) is taken from what the page receives: the window listens to WebView2's response events and keeps, until the reply takes them, the responses that are a sound or a document (2026-11-18, the owner's call for another way than the page's markup, after pressing the site's Download control was tried and dropped). The site's API answers are not searched for addresses: on 2026-11-19 ChatGPT's page received JSON naming zips of its own tools, and taking those as the reply's files was wrong. No cookie or header is kept; the app log says what was received by type, host and size. A site that sends its content over a WebSocket (Muse does) leaves nothing to read there, so the window also lists the Blobs in its page's memory through the DevTools protocol on its own WebView2 (`Runtime.queryObjects` on `Blob.prototype`; no port is opened and nothing outside the app attaches): those that are a sound or a document, 1 KB to 100 MB, and were not there before the prompt are read with the page's FileReader and returned as data URLs, and the handles are released (owner's choice, 2026-11-18). Muse fetches a sound only when its player's Play is pressed, so for Muse (`Recipe::play_audio`) Pygg presses Play once on each audio player in the reply just received, the window always muted, pauses it as soon as a sound player has a new address, and copies the sound from that `blob:` address (Muse drops the Blob itself and keeps only the player's address, seen in the live page on 2026-11-19); nothing else is clicked and no request is changed or made.

**What it cannot do:** tool calls, files sent to the site, token counts, system-prompt guarantees or choice of the model behind the site. A request with tools is refused with 400. Muse can act on the user's connected apps; Pygg clicks only Send and the site's own **deny** on any approval Muse asks for.

**Cost:** Requests are slow and uncounted in tokens, so this suits a user who names the model for chat-like work, not coding agents.

## ADR-036 — How Pygg saves tokens: keep the cache, route per session, measure before compressing

**Decision, in this order:**
1. **Keep the provider's cache working.** It is the largest saving and costs no quality. Sessions keep a launch on one model (`pygg run`, `x-pygg-session`); fallbacks happen only on failure. A session a fallback took away goes back to its home (the target it was meant for) only when no warm cache is lost: the home answered it within the last five minutes, or the fallback has not for five minutes either. It goes back when a request starts, never inside an attempt or a stream, and not while the home is cooling down (`store::Pin::goes_home`, migration 0033; `x-pygg-session: home`, route kind `+home`). When Pygg translates an OpenAI-format turn that carries tools for Anthropic's own API, it turns on Anthropic's automatic caching with one top-level `cache_control`, since the client cannot (`gateway::adds_cache`; the ledger says `added`). OpenRouter documents the same top-level marker for Anthropic's models, so an OpenAI-format agent's turn to an `anthropic/…` model there gets it too, and an Anthropic-format client's own top-level marker crosses the translation to OpenRouter. A request the client marked, a native Anthropic request, a turn without tools, other models and other providers' Anthropic-format endpoints are not changed. One marker is enough: Anthropic looks back at most 20 positions for the previous request's cache, and an agent's turn adds a few (a run of parallel tool calls, and of their results, counts as one). Providers that cache on their own (OpenAI, DeepSeek, Gemini, Kimi, Z.AI's GLM, MiniMax) need nothing added. Alibaba Model Studio also has an explicit cache (a `cache_control` on a content block: hits at 10% of the input price instead of the implicit cache's 20%, writes at 125%); Pygg adds no marker there, since its implicit cache already served 91% of qwen3.8-flash's input in the N2 benchmark (on a token plan, where tokens are quota, not money), and a gain on pay-as-you-go needs a live parity check first. A client's own marker crosses, and the writes it causes are priced (`prompt_tokens_details.cache_creation_input_tokens`). **The session is passed on** to providers that route by one, so repeated requests reach the machine that holds their cache: OpenRouter's `x-session-id` header (sticky for 60 minutes) and OpenAI's `prompt_cache_key`. What goes is `pygg-` and a SHA-256 of the client and its session id, never the id as sent and never prompt content; a client that named its own `session_id` or `prompt_cache_key` keeps it (`gateway::pass_session`). `PYGG_ANTHROPIC_AUTO_CACHE=off` and `PYGG_PASS_SESSION=off` turn either off for a benchmark arm.
2. **Route per session or task, not per request.** `auto` chooses when a session starts and stays. A per-prompt learned router is not built.
3. **Show where tokens go before changing anything** (Usage → Token Mix and Routing): cache share, cache writes after a model change, attempts and fallbacks, and why each answer read nothing from the cache (`store::cache_miss`, migration 0034). The cause is the ledger's best reading, worked out when the answer is settled, in this order: the provider did not report its cache; the prompt was under 1,024 tokens; the client's cache hints were dropped in translation; the session's first request; the session's last answer came from another model; the model has read nothing from its cache in seven days; more than five minutes since the session last used the model; otherwise the start of the prompt changed. A request without a session says only that. A zero cache read counts as a miss only when the provider reported the field (DeepSeek's `prompt_cache_hit_tokens` included). **The tools and system prompt** of each request are measured (migration 0036): their estimated tokens and a 64-bit fingerprint, never the text. Token Mix shows, per session, what its first request carried, how often it changed, and how often a later answer read nothing from a cache the provider reports, so the tools and system prompt went again in full or as a cache write. **A longer cache lifetime** is weighed from the pauses in each session, on models that bill cache writes (`store::cache_lifetime`, no new column): the answers that missed only because more than five minutes had passed, against two ways of keeping the cache, Anthropic's one-hour lifetime (every write at twice the input price instead of 1.25 times) and a keep-alive (the session's last request re-sent every 4.5 minutes for up to an hour, each a cache read, paid also after a session's last answer). Neither is offered unless its net stays above zero.
4. **Compression only in shadow** until measurements show it pays. An agent's history is never rewritten; a filter on new tool output is at most an opt-in experiment. **Clearing old tool results** (Anthropic's context editing) is measured for OpenAI-format clients' requests at Anthropic's defaults: past 100,000 prompt tokens, every tool result but the last three (`shadow::clearing`, migration 0037). Usage weighs the input it would save against the cache writes it would cause, since each request that clears one more result makes the provider write the rest of the conversation again (`store::clearing_shadow`). Anthropic-format requests (Claude Code, OpenCode) are never measured or changed. It becomes an opt-in only if the net stays above zero.
5. **Output and effort stay the client's.** Pygg passes `effort` and thinking settings through and does not inject instructions to write shorter.

**Not done:** semantic response caching, history dedup or retrieval markers, context as images, and RTK's BUSL-licensed engine.

**Why:** Once prompt caching works, the bill is mostly cache writes and output, so compression saves far less than it advertises. Caches are scoped to a model, so switching per request re-sends the whole conversation uncached. Measured: Codex on Claude Haiku through Pygg solved the same 16 runs with and without the added cache marker, and cost 30% less with it ([report](../benchmark/reports/2026-11-14-codex-claude-prompt-cache.md)).

**Consequence:** Every saving Pygg shows is one it can trace in the ledger.

**What is claimed, by scope (2026-11-18).** Savings measured on single tasks do not carry over to long work: cheaper models lose far more on feature-sized and release-sized benchmarks (SWE-EVO, FeatureBench) than on SWE-bench Verified. Parallel agents multiply tokens (about 4× chat for one agent, 15× for a multi-agent system) while people can review only a few at once. So Pygg claims:
- **One bounded task:** cost per solved task, from checks the agent cannot edit, on the user's repository where measured.
- **Parallel sessions:** that their cost can be seen and bounded (per session, per request class, with limits), not that more agents get more done.
- **A whole project:** observations only (cost per launch, per commit, launches that changed nothing), never productivity or return on spend.

Each `pygg run` launch is kept with what changed in Git (`launches`, schema v38) so cost can be set against what came of it.

## ADR-037 — One npm command for every platform

**Decision:** A small launcher with one optional dependency per platform, each restricted by `os`, `cpu` and, on Linux, `libc`, so npm fetches only the matching one (as esbuild, Biome, SWC and Turbo do).
- Packages: `@pygg/cli` (the launcher, command `pygg`) and `@pygg/win32-x64`, `@pygg/darwin-arm64`, `@pygg/darwin-x64`, `@pygg/linux-x64` (glibc), all at the launcher's version. npm refused the unscoped name `pygg`, so the scope stays.
- The launcher (`packages/npm/pygg/bin/pygg.js`) runs the platform's program with the same arguments and streams, passes on SIGTERM and SIGHUP, and exits with its code. Where no build exists it says what to do; it restores a lost execute bit; `PYGG_CLI` overrides; on Windows an installed app's `pygg-cli.exe` is the fallback.
- **The window is a Cargo feature.** `app` (default) brings Tauri; `pygg-cli` builds with `--no-default-features` and links no webview. Credentials use the Secret Service on Linux, the Keychain on macOS and the Credential Manager on Windows. The CLI finds the store where the app keeps it on each platform, or at `PYGG_DATA_DIR`.
- Platform packages are generated by `scripts/npm-pack.mjs` and packed on their own runner. CI (`.github/workflows/npm-packages.yml`) builds, packs and installs each, and never publishes; publishing is a manual step ([PUBLISHING.md](../packages/npm/PUBLISHING.md)).

**Why:** One package with every binary costs every user about 60 MB; a `postinstall` download breaks with `--ignore-scripts`, proxies and offline caches, and adds a second source to trust.

**Not done:** linux-arm64 and musl builds, and code signing (no signing during development).

**Cost:** The window's code must stay behind `#[cfg(feature = "app")]`, which `cargo check --no-default-features` and the Linux CI job catch.

## ADR-038 — Key rings: keys of separate accounts on one connection, used in turn

**Decision:** A connection may hold a **key ring**: its own key plus keys of other accounts, each confirmed by the user as another account with its own quota (the ADR-034 rule). Requests keep naming `connection/model`; combos and `auto` are unchanged.
- **Order per connection** (`providers.key_strategy`): `in_order` (default), `round_robin`, `least_used` (fewest requests today) or `quota_aware` (most room by the limits that key's account last stated, read from its rate-limit headers; without them, least used). A session goes back to the key that answered it, for that account's prompt cache.
- **Per-key cooldowns and blocks** (`policy.rs`): a 401 or rejected-key 400 blocks the key until the user replaces or removes it; a 402, an out-of-credit answer or a 429 cools the key (or one model, for a 429) for Retry-After or a default, and the next key is tried at once. These mark the key (`key_health`), not the connection. Other failures stay the connection's. Quota groups still hold.
- **The ledger names the key** (`usage_events.key_id`).
- **Secrets** stay in the OS credential store, one reference per key; nothing reads, logs or returns a key. Removing the connection deletes every key.
- **Never rotated:** web sessions, subscription and agent sign-ins, local runtimes.
- **Providers → a connection → API keys** shows each key's state, requests today and remaining stated limits, and offers ordering, on and off, removal and **Add Key from Another Account…**.

**Not done:** weights, a sticky turn count, a simulator and per-key budgets. Pools (ADR-034) remain for accounts that need their own endpoint, models or prices.

**Why:** Ten accounts in a pool mean ten copies of one model list, and clients must name the pool. A ring keeps one connection and lets the next account answer in the same request.

**Cost:** The rule against rotating one account's keys depends on the user's confirmation; Pygg cannot check it.

## ADR-039 — Skills & Prompts: one library, installed into each agent's own skills folder

**Decision:** Pygg keeps a library of skills in the open Agent Skills format (a folder holding `SKILL.md`: front matter with `name` and `description`, then the instructions) and installs them where the agents already look. It is a library and an installer, not a hub that downloads.
- **Targets:** Claude Code's `~/.claude/skills` (or `$CLAUDE_CONFIG_DIR/skills`), and the shared `~/.agents/skills` that Codex and OpenCode read. OpenCode also reads Claude Code's folder.
- **A prompt is a skill run only by name.** It carries `disable-model-invocation: true`, so Claude Code and OpenCode leave it out of the list they send with every request. Codex ignores that field and has its own: `agents/openai.yaml` with `policy.allow_implicit_invocation: false` keeps a skill out of the model's context until it is called as `$name`. Pygg writes that file beside each prompt it installs in `~/.agents/skills`. Pygg's eight starter skills (written for Pygg, MIT) are all prompts, so installing them adds nothing to a request until one is used.
- **What Pygg writes:** one `SKILL.md` per install, with `metadata.pygg-hash`, a hash of the rest of the file, and for a prompt in the shared folder Codex's `agents/openai.yaml`, Pygg's only while it reads exactly as written. A copy installed before that file existed shows as outdated until it is installed again. Pygg never overwrites a folder it did not write and never removes or rewrites a copy edited since; it says so instead. Line endings are not an edit.
- **The user's own skills** are rows of `skills` (schema v32), so a store backup holds them. A skill found in an agent's folder can be copied into the library and then installed for the other agents. Starters cannot be changed or deleted; they are duplicated.
- **Sent With Every Request:** for each agent, its global instruction file (`CLAUDE.md`, `AGENTS.md`) counted whole, and each listed skill or command by its name and description, estimated as the gateway estimates tokens. OpenCode falls back to `CLAUDE.md` when it has no `AGENTS.md`.
- **Projects:** a folder the user adds (`skill_projects`, schema v35) becomes a place to install: its own `.claude/skills` (Claude Code, OpenCode) and `.agents/skills` (Codex, OpenCode), so a team's skills travel with the repository. Pygg writes only into folders listed there, refuses the home folder (its `.claude` and `.agents` are the global ones), and forgets a folder only once its copies there are removed. For each project it counts what each agent adds on top of its global files from the project's root: `CLAUDE.md`, `.claude/CLAUDE.md` and `CLAUDE.local.md` for Claude Code, `AGENTS.override.md` or `AGENTS.md` for Codex, `AGENTS.md` or else `CLAUDE.md` for OpenCode, and the listed skills and commands. Files in folders below the root, read only when the agent works there, are not counted. A project the user did not add is never read.

**Not done:** downloading skills from GitHub or a catalog (each would need a pinned commit, a review of every file and a quarantine for scripts; the earlier plan's supply-chain gate), skills with scripts or extra files, and Codex's `~/.codex/prompts` as a target.

**Why:** The three agents now share one format and two folders, and each keeps only a skill's name and description in context until the skill is used. A library written once and installed into those folders saves retyping without a format of Pygg's own. The always-on context is the part of every request a user can shrink themselves, and only the files on their machine say how large it is.

**Cost:** Pygg follows the agents' folder conventions; when an agent moves its folder, the target changes with it.

## Open decisions

- A code-signing certificate (ROADMAP D3), before distribution.
