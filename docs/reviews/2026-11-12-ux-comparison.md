# Pygg's interface against comparable products, 2026-11-12

> **Status:** Record · a comparison of the desktop interface with gateways and agent-task tools, and what was changed · order: [ROADMAP](../ROADMAP.md) · earlier review: [Codex UI/UX review](2026-11-12-codex-ux-review.md)

**How it was made.**
- **Two independent passes.**
  - Codex CLI (ChatGPT plan, read-only) was given the 12 screenshots and the source, and asked where a developer comparing products would find Pygg behind in the first ten minutes.
  - A separate research pass read each product's own documentation, README and reviews, with a link for each.
- **What it rests on:** documentation and reviews. No competitor was installed or signed in to, so layouts are as documented, and details that could not be confirmed are marked unverified.

## What the others do

**Gateways**
- **9router** ([GitHub](https://github.com/decolua/9router)):
  - providers grouped as OAuth, free and API key;
  - several accounts per provider;
  - combos as ordered lists;
  - quota with reset countdowns for subscriptions;
  - a CLI Tools page that applies a model to a tool (one click only for some tools).
- **OmniRoute** ([GitHub](https://github.com/diegosouzapw/OmniRoute)):
  - ready-made `auto/*` combos and a tiered fallback (subscription, key, cheap, free);
  - per-key spend quotas;
  - a decision header on each response;
  - `omniroute configure` / `run` for 7 to 11 CLIs.
- **LiteLLM admin UI** ([docs](https://docs.litellm.ai/docs/proxy/ui)):
  - virtual keys with budgets, expiry and alerts;
  - a model hub;
  - teams and roles;
  - a usage tab and logs.
  - Its issue tracker has many reports of the Usage and Logs pages failing or slowing down.
- **OpenRouter** ([activity](https://openrouter.ai/docs/guides/features/activity), [logs](https://openrouter.ai/docs/guides/features/logs)):
  - activity overview, trends and a chart builder, with one date range and set of filters shared across them;
  - every chart bar or table row opens the logs already filtered;
  - logs show model, provider, status, attempts, key and latency;
  - a detail sheet has a cost breakdown and a waterfall of provider attempts;
  - keys can have limits that reset.
- **Portkey** ([logs](https://portkey.ai/docs/product/observability/logs)):
  - each log row shows cache, retries and whether fallback or load balancing acted;
  - a side panel has the raw request, a share link and replay;
  - analytics include how many requests a fallback saved.
  - Reviews complain the product is complex for newcomers.
- **Helicone** ([docs](https://docs.helicone.ai/features/sessions)):
  - a request table with cost, latency and tokens, plus filters;
  - nested sessions.
  - Mintlify acquired it in March 2026, and third parties report it is in maintenance mode.

**Agent and task tools**
- **Vibe Kanban** ([GitHub](https://github.com/BloopAI/vibe-kanban)):
  - a board whose cards move by themselves (to do, in progress, in review, done);
  - inline diff comments sent to the agent;
  - one worktree per task.
  - The company shut down in April 2026.
- **Conductor** ([diff viewer](https://www.conductor.build/docs/reference/diff-viewer), [checks](https://www.conductor.build/docs/reference/checks)):
  - a changed-file list with a unified diff and line comments;
  - a Checks tab (git, CI, reviews, to-dos) that discourages merging while something fails;
  - a suggested next action.
  - Mac only.
- **Claude Squad** ([GitHub](https://github.com/smtg-ai/claude-squad)): a terminal list of sessions with Preview and Diff tabs, attach, pause and resume. It needs tmux.
- **Codex app** ([docs](https://developers.openai.com/codex/app)): threads by project, local, worktree or cloud runs, a diff pane with the last turn's changes, and commit or PR from the app.
- **Cursor cloud agents** ([docs](https://cursor.com/docs/cloud-agent)):
  - started from the IDE, the web, Slack or the API;
  - evidence per run (screenshots, video, logs);
  - take-over;
  - a spend limit asked on first use.
  - Its forum has threads on runaway cost: one session over $600, and paying for failed tasks ([thread](https://forum.cursor.com/t/cursor-needs-better-pricing-guardrails-when-one-session-can-cost-600/156172)).
- **Sculptor** ([docs](https://docs.imbue.com/features/pairing-mode)): one container per agent, live mirroring into the user's IDE, diffs, to-dos and checks.

## Where Pygg was behind, and what was changed

Both passes named the same first gaps.

| Gap a developer sees early | Who does it better | Change made (2026-11-12) |
| --- | --- | --- |
| **Reviewing a result's code feels like reading a log.** The diff was plain text, collapsed | Conductor, Vibe Kanban, Codex app, Claude Squad | **Done.** The diff of a result waiting for a decision loads and shows at once, file by file, with added and removed lines coloured and counted (`DiffView.tsx`). Line comments sent back to the agent are not built |
| **Requests are hard to investigate.** The log sat under the summaries with one filter | OpenRouter, Portkey, LiteLLM, Helicone | **Done.** Filters by client and model; a row of By Model or By Client opens its requests; a latency column; a visible chevron for each request's details; a link to the requests from the top of Usage. Route reason, translation, session and error were already in each request's details |
| **Setting up a client never reaches "it works".** | 9router and OmniRoute (per-tool setup) | **Done, without sending anything.** Under the client setup, the newest request says it worked (which client, which model) or did not, or that none has arrived yet. A one-click per-tool apply is not built |
| **Agent output is hidden for a successful attempt.** | Claude Squad, Cursor | **Done** for finished attempts. Output while an attempt runs needs the engine to store it as it comes; not built |
| **Starting a task asks for orchestration knowledge.** | Cursor, Codex | Partly answered earlier: Overview now points IDE users to Agents. New Task's ladder behind an "Advanced" fold is left for later |
| **Navigation leads to an unbuilt screen.** | (general) | Answered earlier: Policies points to where its controls live today |

## Where Pygg is ahead, to keep

- **The attempt sequence:** each step's agent, checks, why it moved on (a skipped step included) and cost, on one screen. Neither request dashboards nor agent chat views explain an escalation this directly.
- **Honest cost:**
  - actual, estimated and unknown are kept apart;
  - a subscription counts as quota, not money;
  - failed requests are listed.
- **Approval with a cap before a paid step**, and daily budgets. This is the gap Cursor's users complain about; Pygg should say it plainly where people decide.
- **Checks and privacy belong to the workspace**, and a result is kept as a branch or a patch without touching the user's checkout.
- **Windows first.** Conductor and Sculptor are Mac-only or container-based, and Claude Squad needs tmux.

## Not built, from this comparison

In rough order of value:
1. Inline diff comments sent to the next attempt.
2. Charts over time and "top N" cards, with drill-down.
3. Subscription quota with reset countdowns, and several accounts per provider (ROADMAP E).
4. A per-request attempt waterfall.
5. One-click setup per tool (D2, the integrations hub).
6. Live agent output.
7. Key limits that reset daily or weekly, and alerts.
8. A board view of tasks.

Two risks seen across this category also apply to Pygg:
- Projects shutting down (Vibe Kanban, Helicone, Crystal).
- Usage pages slowing down as logs grow (LiteLLM). Pygg's request list is capped at 200 per period for this reason.
