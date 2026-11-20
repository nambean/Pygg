# Pygg — Screens

> **Status:** Current · the app's screens as built · reviewed 2026-11-20

Code: `apps/desktop/src/main.tsx` (shell), `apps/desktop/src/components` and `components/gateway` (screens), `apps/desktop/src/styles.css` (tokens, layout). Code comments cite the section numbers below.

Signature element: the **attempt sequence** (`AttemptSequence.tsx`), one chip per attempt (tier, outcome) joined by arrows.

## 1. Principles

- **Gateway first.** Gateway screens lead the sidebar in set-up order; Smart Tasks are their own group, marked Experimental.
- **Honest values.** Unknown, estimated and actual figures stay apart; an unknown cost is never $0 (§3).
- **Approvals only in the app.** A paid task step waits in task detail; no notification or API approves it (§2.5).
- **Confirm before spending or deleting** (§6); the command palette only opens the screen that asks.
- **Status in place, not banners.** Gateway status sits in the sidebar footer and Home's gateway card; a problem shows next to what it affects.
- **Writes only to Git worktrees.** The user's own checkout is never changed.

### D12: traffic-light window controls

On Windows and Linux the native title bar is off and the app draws macOS-style traffic lights (`WindowControls.tsx`), as DownFetch does; macOS draws its own.

- Each button has an accessible name; the green one is **Maximize** or **Restore**.
- Windows 11 snap layouts work: an invisible native window over the green button (`snap.rs`) maximizes/restores and reports hover.
- Glyphs show on hover and keyboard focus; buttons dim when the window is inactive.

## 2. Shell

### 2.1 Layout

- Sidebar 224px; toolbar 52px; body text 13px.
- Hide the sidebar with the toolbar button or `Ctrl+Shift+L`.
- Below 900px width (or 1024px at 125% text), the sidebar becomes a 56px icon rail: labels on hover and for screen readers, counts as badges.
- **Skip to content** is the first Tab stop. After navigation, focus goes to the page heading unless the page already took focus.
- The browser preview (not the desktop app) shows a line saying it runs on demo data.

### 2.2 Sidebar and the gateway screens

| Group | Screens | Shortcut |
| --- | --- | --- |
| **Gateway** | Home, Providers, Endpoint, CLI Tools, Combos, Usage, Policies, Skills & Prompts | `Ctrl+1` … `Ctrl+6` for Home to Usage |
| **Smart Tasks · Experimental** | Tasks, Workspaces, Agents, Brain Storming | `Ctrl+7` Tasks, `Ctrl+8` Agents |
| **App** | Guide, Settings | `Ctrl+/` Guide, `Ctrl+,` Settings |

- Rows 28px, accent icon, quiet selected background.
- **Tasks** shows an emphasized count of tasks needing attention (§5.1); **Workspaces** a count of workspaces whose checks need review.
- The app always starts on Home.
- **Footer:** gateway status (a button that opens Home) and the app version. States: `Gateway Running · <port>`, `Gateway Stopped`, `Gateway Couldn't Start`, `Connecting…`, `Core isn't responding`. A provider outage shows on that connection (§5.4), never as gateway offline.

Providers is §5.4, Usage §5.6, Endpoint §5.7; the other gateway screens follow.

#### Home

`Overview.tsx`. Is Pygg working, and what does it cost:

- **Gateway card:** state, "Loopback only · other devices can't reach it", **Start Gateway** / **Stop**, Copy URL, **Configure…** (§6).
- **Usage alerts:** ledger items worth a look (e.g. unpriced models); each opens Usage or Providers.
- **Estimated Savings** (`Savings.tsx`, `gateway/savings.rs`), once there are requests in the last 24 hours. Answered requests are repriced at a user-chosen comparison model with the same tokens.
  - Shows the difference and percentage, requests compared, and three figures: cost at the comparison model, cost through Pygg, difference.
  - **Where the answers came from:** one bar of free, local, subscription, paid and billing-unknown answers, with counts and shares.
  - Lists what was left out: unknown cost, no token counts, subscription answers, failed attempts that may have been billed.
  - **How this estimate works** (disclosure).
  - With no comparison model, it offers the most-used paid model clients asked for by name; accept or pick another.
  - Never claims more than the ledger shows; free, local and plan answers and fallbacks are not savings in themselves.
- **Get Started**, until done:
  1. **Connect a provider:** a connection is on, has its key, lists models, and `auto` has a model that qualifies for a coding agent's request.
  2. **Set up your coding tool:** opens CLI Tools.
  3. **First request answered:** from one of the user's own keys.
- **Last 24 Hours:** requests, known cost, unknown-cost count (not in the total), fallbacks, failures; the six newest requests (model, client, tokens, cost, outcome); **Open Usage**.

#### CLI Tools

`CliTools.tsx`. Point a coding tool at Pygg.

- **One-Click Setup:** each tool gets its own Pygg API key.
  - **Claude Code: models through Pygg** (`launcher.rs`): **Set Up** makes a key kept in the Windows Credential Manager; the row shows `pygg run claude`. Claude Code's settings are untouched, so plain `claude` keeps the user's plan.
  - **Codex**, **OpenCode** (`integrations.rs`): **Set Up…** previews the change, then backs up, writes and checks. **Remove…** restores the config and revokes the key.
- **Status per tool,** from the newest request with its key (nothing is sent): *Configured, waiting for its first request*, *Working: answered by <model> <time>*, or *Its last request did not succeed* with **View Failed Requests**.
- **Before set-up:** a note when `auto` has no qualifying model for that tool's API format, linking **Connect Provider…** or **How auto chooses**; with no provider, one warning above the list. Set Up still works.
- **Set up by hand** (disclosure): Claude Code without the launcher, OpenCode, editors, OpenAI SDK, curl. Each has a model choice (auto, auto/cheap, combos, enabled models) and a shell choice where it matters.

#### Combos

`Combos.tsx`. Named, ordered model lists; a request for the combo's name tries each member in turn. Two tabs (segmented control).

**Combos tab:**

- One card per combo: strategy (**Ordered fallback**, or **Cheapest first**: members ordered per request by estimated cost); members as a chain with billing, tier, tool support, and a mark when the connection is disabled or removed; connections sharing a quota group; **Copy Name**, **Test**, **Edit…**, Delete.
- **Test:** one short request after a confirmation; says which member answered and how many failed first.
- **New Combo…** / **Edit…** open the combo editor (sheet): type part of a model id or connection name (every word must match); the list stays open, arrows move, Enter adds and keeps focus; models already in the combo are marked.
- **From Template…** (`lib/comboTemplates.ts`): Free first, Use my plans first, Subscription only, Strong coding with a cheap backup, No metered spend, Local only, Long-context coding, Cheapest for each request. Previews the picked members with reasons, and left-out models with reasons.
- Disclosure: the next member is tried on a rejected key, exhausted quota or unavailable model, never once an answer has started streaming.

**How auto chooses tab** (`AutoRouting.tsx`):

- What `auto` and `auto/cheap` mean.
- Simulator (sends nothing): alias, client (or a new client with defaults), request format, tools, images. Shows the models tried, in order, with reasons, and every excluded model with its reason.

#### Policies

`Policies.tsx`. Every rule the gateway applies:

- **Defaults for New API Keys:** minimum tier, monthly limit, limit mode (Estimated / Strict), unknown prices (Block / Allow). Apply to keys made anywhere: Endpoint, `pygg gateway create-client`, a tool's set-up.
- **API Key Policies:** per key: monthly limit, minimum tier, requests at once, unknown prices, paid retries, allowed providers. **Limits…** opens the Endpoint sheet.
- **Connection Policies:** data-use terms, quota (own, shared, assumed shared), pool, billing.
- **Always On** (not settings): loopback only; keys in the Credential Manager; unknown cost never $0; paid task steps approved only in the app; no telemetry.
- **Smart Tasks (on hold):** daily limit for paid steps (set in Settings), link to Workspaces.

#### Skills & Prompts

`Skills.tsx`, `skills.rs`, ADR-039. Skills in the Agent Skills format, installed into each agent's skills folder. Primary action: **New Skill…** (also in the palette).

- **Sent With Every Request:** for Claude Code, Codex, OpenCode, what global files add to every request (instruction files whole; skills and commands by name and description) with estimated tokens; warning badge above 5,000. A skill found there offers **Add to Library**.
- **Library:** starter and user skills, marked **Pygg** / **Yours** and **Prompt** / **Skill**, with cost (a prompt: nothing until `/name` is typed; a skill: name and description on every request where installed).
  - **Install for Claude Code**, **Install for Codex and OpenCode**, **Install for All**; per-target state: Installed, Older version, Edited there.
  - More menu: Update and Remove per target; **View…** (starter) or **Edit…**; **Duplicate…**; **Delete…** (user's own, confirmed).
- **Projects:** added with **Add Project…**; what root files add per agent, how many Pygg skills it holds, **Install Skills…** (sheet: Install, Update, Remove per skill and target), **Remove from Pygg…** (changes nothing in the folder). A missing folder is marked.
- **Skill sheet:** name (typed as `/name`), description (what the agent reads to choose), instructions (`$ARGUMENTS` = text after the name), **Run only when I type its name (a prompt)**. A starter opens read-only with **Duplicate…**.
- **States:** loading, load error, no skills yet, no projects yet (with **Add Project…**).

#### Agents

`Agents.tsx`. Official agent CLIs on this computer; their sign-in is never read.

- **Agents on This Computer:** version and sign-in, or **How to Install**.
- **Run a Task:** one-off run in a worktree of a chosen repository.
- **Let Your Agent Delegate Tasks:** one-click MCP set-up for Claude Code, Codex CLI, OpenCode; **Set It Up by Hand** for Cursor and other JSON-configured clients.

#### Brain Storming

`BrainStorming.tsx`, settings keys `brain.*`, ADR-042, [BRAIN.md](BRAIN.md) §4. One-time `pygg/brain` set-up for every project. Settings only: sends nothing to a model.

- **Models by role** (plan, code, test, review): the saved pick (`connection/model`, tier, billing, ranking evidence: your runs, a fact with source and date, or the tier), **Change…** (role sheet, §6), **Clear**. A cleared role reads "By the rules today" with the rule's current pick, or that nothing is eligible. A saved model no longer eligible is kept and marked.
- **Defaults for a new brainstorm:** runner; WIP limits for Running and Checking (min 1); whether a planner picked in a brainstorm is saved as the plan role (sets the pick's checkbox default); what a card's **Change…** offers first: ask (this card or all of its role), this card only, or all cards of its role.
- **Facts and prompt:** facts switch, build date, fetch time, **Refresh**, sources with licences, row counts and dates; the prompt `brain-v1` with **Open** (into Skills & Prompts).
- **Projects that differ:** each workspace's differences (`role → model`, `Running 2 · Checking 1`), **Override…** / **Edit…** (project sheet, §6), **Use defaults** (clears its picks and limits); **Workspaces** link in the heading. Overrides are `brain_picks` rows scoped to the workspace and its `board_settings` row; a brainstorm's **Change…** and the board's **Limit…** write the same rows.
- **How it starts:** the command per agent; no text box.
- **Which level wins** (picks, WIP limits, runner): workspace override, then this screen's default, then the rules (your runs, facts feed, tier). Each value says its level.

#### Guide

`Guide.tsx`. How to use Pygg, every command copyable. Sections: Quick start, Claude Code, Codex CLI, OpenCode, Editors and SDKs, Command line and npm, auto and combos, Costs and limits, Troubleshooting, FAQ. Sends nothing, changes no setting.

### 2.3 Toolbar

- Sidebar toggle, screen title, command palette button, primary action: **Connect Provider…** (Providers), **Create API Key…** (Endpoint), **New Combo…** (Combos), **New Skill…** (Skills & Prompts), **New Task** (Tasks, task detail).
- On Tasks and task detail, the subtitle (`BudgetSubtitle.tsx`): `<n> running · <n> waiting · <recorded> recorded + <reserved> reserved / <limit> today`; with no daily limit, `<amount> on paid steps today`.
- Clicking it opens **Today's budget for paid steps**: recorded, reserved for running work, daily limit, runs with unknown cost. Rules: a run with no reported cost counts at its cap; free, local and subscription steps excluded; resets 00:00 UTC.

### 2.5 Tray and notifications

`shell.rs`; controlled in Settings → Background (§5.8).

- **Tray menu:** **Open Pygg**, **Quit Pygg**.
- **Keep running in the tray:** closing the window keeps the gateway and tasks running; a notification says so the first time.
- **Open Pygg when you sign in:** starts in the tray, no window.
- **Task notifications:** waiting for approval; ready (checks passed); needs review; did not finish.
- **Spending notifications** (each once a day, checked every 10 minutes): client reached its monthly limit; client near it; spending is up; many requests failing.
- Never names a task or file; never has an approve button.

### 2.6 Keyboard and command palette

`Ctrl` on Windows and Linux, `Cmd` on macOS.

| Shortcut | Action |
| --- | --- |
| `Ctrl+1` … `Ctrl+6` | Home, Providers, Endpoint, CLI Tools, Combos, Usage |
| `Ctrl+7` / `Ctrl+8` | Tasks / Agents |
| `Ctrl+/` | Guide |
| `Ctrl+,` | Settings |
| `Ctrl+K` | Command palette |
| `Ctrl+Shift+L` | Show or hide the sidebar |
| `Ctrl+N` | New Task (on Tasks and task detail) |
| `Ctrl+F` | Search tasks (on Tasks) |

- An open sheet takes every key; these shortcuts do nothing.
- **Command palette** (`CommandPalette.tsx`): groups Go to, Gateway, Tasks, App. Typing filters (every word must match); arrows move, `Ctrl+Home` / `Ctrl+End` jump to the ends, Enter runs, `Esc` closes. No command spends or deletes; those open the screen that asks.

## 3. Showing values honestly

### 3.1 Identity

- A gateway model is `<connection>/<model>`, the name a client sends.
- A provider row shows the label (may truncate) and its `id/…` prefix (never truncates).
- A task attempt names its step's agent and model, with tier and billing.
- Ids, paths and model names are selectable.

### 3.2 Provenance

- Ledger cost source: reported by the provider, your price × reported tokens, connection marked free, local model, no price recorded, or failed after reaching the provider (billing unknown).
- Rate limits: "as the provider last said", from its last answer's headers.
- Account balance: the endpoint it was read from, and when.
- Health check: when it ran.
- Data-use terms: the user's own record, not checked by Pygg.

### 3.3 Money

`lib/money.ts`, `lib/format.ts`.

| Kind | Shown as |
| --- | --- |
| Actual | `$0.08` |
| Estimated | `≈$0.0031`; small amounts get four decimals; under $0.0001 is `<$0.0001` |
| Unknown | "Unknown", never `$0.00`, never in a total; totals say how many requests were unknown |
| Subscription | counts against the plan's quota, not money; unknown in API Costs |
| Local | `$0` API money; electricity and machine time not measured |
| Task cost | `API $0 · local`, `<actual> + ≈<estimated> est.` or `<n> calls unknown` |

### 3.4 Status words

A status always pairs an icon or dot with a word; colour is never the only signal.

- **Task states:** Checks passed · Finished, needs your review · Manually accepted · Check failed · Blocked by policy · Canceled · Interrupted · needs review · Waiting for approval.
- **Attempt outcomes:** Passed · Finished, not checked · Check failed · Running · Waiting for approval · Stopped · Skipped: not answering.

## 4. Visual tokens

DownFetch's semantic tokens and green accent, in `styles.css`. They follow the system's light, dark and increased-contrast settings; no in-app theme switch.

| Token | Light | Dark |
| --- | --- | --- |
| `--window` | `#F5F5F7` | `#1E1E1E` |
| `--sidebar` | `#ECECEF` | `#262628` |
| `--surface` | `#FFFFFF` | `#2C2C2E` |
| `--label` | `#1D1D1F` | `#F5F5F7` |
| `--secondary` | `#636366` | `#A1A1A6` |
| `--accent` (fills) | `#0B7A4B` | `#0D8551` |
| `--accent-text` | `#0A6B42` | `#3DDC97` |
| `--accent-icon` (sidebar icons) | `#0B7A4B` | `#3DDC97` |
| `--orange` (warnings) | `#C93400` | `#FF9F0A` |
| `--red` | `#D70015` | `#FF6961` |

- **Contrast:** white semibold on accent 5.4:1 light / 4.7:1 dark; accent text on surface 6.6:1 / 9.6:1. Increased contrast strengthens separators and secondary text.
- **Type:** body 13px, toolbar title 15px, sidebar group headings 11px, monospace 12px for ids, commands, paths, output. Money and tokens use tabular figures.
- **Colour:** accent marks the primary action; warnings are orange, always with icon and text.
- **Focus:** 3px accent outline, survives Windows high contrast.
- **Reduced motion:** spinners and switch transitions stop; status text stays.

## 5. Screens

### 5.1 Tasks

`TaskList.tsx`.

- **Filters** (segmented, with counts): All, Running (excludes waiting for approval), Needs Approval, Completed (checks passed, manually accepted, needs review), Failed (check failed, blocked), Stopped (canceled, interrupted), and **Board** (§5.1.1) last, with the count of cards waiting on the user.
- **Search** by title (`Ctrl+F`).
- **Needs Attention** (on All, pinned on top, not repeated): waiting for approval, interrupted, plan needs revalidation, needs review. Their button is **Review**; others **Open**.
- **Row:** state icon, title, workspace, starting client, status line (phase, attempt, time), attempt sequence, cost so far (§3.3).
- **Brainstorm rows** (§5.2.2) appear on All, and under Needs Approval while waiting on the user; **Review** opens one.
- **States:** loading; load error with **Try Again** (never an empty list); Ready for Your First Task with **New Task**; No Tasks Match "…" with **Clear Search**; an empty state per filter.

#### 5.1.1 Board

`Board.tsx`, ADR-042, [BRAIN.md](BRAIN.md) §3. A workspace's approved brainstorm cards as Kanban columns. Reloads every 3 s; the app's tick pulls cards into Running under the limit.

- **Header:** workspace (switcher when several have cards); `Running 1/1 · Blocked 2 · Ready 6 · Review 1 · Done 3`; **Measures** disclosure once 5 cards are done (before that, how many are): cycle time median (with coverage when not all), cards done today, time blocked today, known money per kept result by role with unpriced count; **Use the default limits** when the workspace has its own.
- **Columns:** Discovery, Ready, Running, Checking, Blocked, Review, Done. Running and Checking show `count / limit` and **Limit…** (inline edit, focused; Enter saves, Escape cancels; min 1). Blocked is named in orange. Empty column: **Nothing here**.
- **Card:** title (▲ when expedited), role chip, attempt chips, the ladder's first model and how many follow, Blocked reason and `blocked 2 h 10 min` (hours/days when long), phase and time while running (*Waiting for Checking* when held by Checking's limit), "held by you" or what it waits for (Discovery), "next in line" (Ready), "checks passed" or "finished" (Review, Done).
- **Buttons:** **Review** (Blocked, Review) or **Open** (elsewhere) opens task detail with **‹ Board**; Ready: **Expedite** / **Standard**, **Back to Discovery**; Discovery (held card): **Move to Ready**; **Drop** (sheet naming the card) on a card without a task; **Hand to the engine** on a card waiting for the IDE agent. Running, Checking, Review, Done are engine-only. No drag-and-drop.
- **Workspace rules:** its approved checks define Done for every card (none reaches Review unverified); its paid steps per day and privacy apply.
- **Narrow (< 980px board width, container query):** one list grouped by column, in order Blocked, Review, Running, Checking, Ready, Discovery, Done.
- **States:** loading; load error with **Try Again**; no cards yet (with how a brainstorm starts); no workspace; gateway stopped or failed (state and **Start** in the header; Ready waits).

### 5.2 Task detail

`TaskDetail.tsx`. Reloads every 2 s while running; **‹ Tasks** goes back.

- **Waiting for approval** (inline, not a sheet):
  - **Paid step:** cost range of the last runs of that agent and model (agent's own estimate) or "no estimate yet"; today's budget left; a cap field: **Spending cap** if the agent can stop itself, else **Stop further steps above**. Buttons **Approve up to $…**, **Decline**.
  - **Step in a sensitive workspace:** **Allow This Step**, **Decline**.
- **Result** (checks passed or needs review): **Review Changes** (file-by-file diff, `DiffView.tsx`, **Copy Diff**); branch title with **Keep as Branch**, **Discard…** (confirms), **Export Patch…** (a file `git apply` takes). After keep or discard, the page says so.
- **Attempts:** one row each: step, tier, billing; cost (Pygg's ledger, agent's estimate, or unknown) with the cap; checks; disclosures for the handoff it received, check output, agent output. Below: cost so far and why any cost is unknown.
- **Task:** steps, approved checks, starting commit and folder.
- **Actions:** **Stop Task** while running; **Retry** when interrupted.

#### 5.2.1 New Task

`NewTaskView.tsx`.

- **Task:** the description.
- **Workspace:** pick a repository or **Add…**.
- **Steps:** a strategy (Free First, Balanced, Quality) proposes the ladder. Each step has agent, model, tier, billing; move up/down or remove; **Add Step**. Paid steps wait for approval.
- **Shell commands:** **Allow** switch; off means read and edit files only. Codex always runs in its own sandbox.
- **Start Task** creates the task and opens it.
- No agent installed: says so and points to Agents.

#### 5.2.2 Brainstorm

`BrainstormDetail.tsx`, ADR-041, [BRAIN.md](BRAIN.md) §2. A brainstorm from `pygg/brain`, reloaded every 2 s while it waits or thinks.

- **Row under Tasks:** lightbulb, requirement's first line, session, state, planner (or "no planner yet"), open questions, cards. Pinned under Needs Attention while waiting for a pick, proposed, or failed.
- **States:** **Waits for your pick**, **Thinking**, **Proposed · revision <n>**, **Approved · board**, **Declined**, **Planner failed**.
- **Header:** requirement, state, whether a coding agent sent it, session, planner and prompt version (`brain-v1`).
- **Waits for your pick** (inline; the first paid request Pygg would start on its own): the plan role's ranking under the client's policy as radio rows (`connection/model`, tier, billing, reason), rule's pick first; **Remember this planner for every project** (on by default); **Decline**, **Use <model>**. Nothing eligible: the reasons and only **Decline**.
- **Questions:** read-only, open or answered, with ids (`q3`); answers come from the coding agent. A card waiting on a question names it.
- **Cards** by wave, heading with cards kept and how many the schema rejected. Each: role chip, title, `connection/model` (selectable, wraps), the rule's reason (*unpriced* if no price), disclosure with done criterion, dependencies, runner (Pygg engine or My IDE agent), class.
  - **Change…** opens the role sheet (§6); apply to **This card** or **All <role> cards**, or **Cancel**, unless Brain Storming's setting decides.
  - **Fallbacks** under the model ("then …" with reasons): the two the rule suggested from the role's route, **Remove** each, **Add a fallback…** (role sheet without models already in the ladder), max 5. On approval each model is a step of the card's task, tried in order.
  - **Drop** strikes a card through; **Restore** undoes.
- **Approve bar** (sticky): `Known estimate ≈$1.20 for one run of every card · unpriced cards: 2 · today's task budget left $1.52 · Running limit 1` (*Estimate: Unknown* when nothing is priced; *no daily task limit* when unset); count of kept cards; workspace; agent (installed ones; OpenCode first, the only agent that runs a card on Pygg's picked model); **Decline brainstorm**; **Approve <n> cards…** (confirms with the same line, then shows **Open Board**). Disabled while a card has no eligible model; with no workspace, says to add the repository on Workspaces. An approved brainstorm keeps **Open Board** in its header.
- **Planner failed:** reason, planner's answer (disclosure), **Retry with <planner>**, a pick to choose another planner, **Decline**. A load error shows **Try Again**, never a spinner.

### 5.3 Workspaces

`WorkspacesView.tsx`. Repositories tasks run in; a task from an IDE adds its repository itself.

- **Add Workspace…** takes a Git repository folder.
- **Each workspace:** folder; **Privacy** (Standard, or Sensitive: every step asks before an agent sees the code); **Paid steps per day here** (on top of the overall limit); **Checks** (detected ones are suggestions until approved; add by hand; **Detect** rescans; approved checks run in order, stop at first failure); **Remove**.
- A badge marks checks waiting for review. With no approved check, a finished attempt waits for review.

### 5.4 Providers

`Connections.tsx`. Primary action **Connect Provider…** (§6.2).

- **List row:** logo, label, `id/…` prefix; preset, billing, model count, auto-eligible count; last 24 h as hourly dots with a health score; state badge; on/off switch.
- **Above the list:** **Check All** (enabled API connections). Over three connections: search and filter (all, needs attention, local, Anthropic API).
- **Auto readiness line** (under the list): how many models `auto` may choose for a coding tool, and how many miss a fact, are unavailable, or are outside the rules. Each count opens the connection with most of them, models filtered.
- **Detail:**
  - **Authentication:** **Add Key…** / **Replace Key…**.
  - **Health:** dot strip, last check, **Check Now** (**Check Sign-In** for a web session), cooldowns and blocks.
  - **Key ring:** keys from other accounts, used in turn.
  - Rate limits as last stated, account balance where reported, billing.
  - **Advanced:** endpoint, API format, independent quota (confirms), pool, data-use terms, model names.
  - **Check & Refresh Models**, **Remove Connection…**.
- **Models:**
  - Search; filter: all, enabled, free, paid, unknown (no price), auto groups. **Add Model…** by name.
  - Fixed-column table: Model, Price per 1M tokens, Enabled, actions.
  - Each line: billing, auto facts, whether auto would choose it (or why not); once measured, 30-day answer rate and median latency with sample size.
  - **Test** = Test Model (§6.2). More menu: **Edit Price…** / **Set Price…**, **Auto Routing Facts…**, **Remove Model**.
- **Empty:** No Connections Yet with **Connect Provider…**. After the first connection, a note offers **Open CLI Tools**.

### 5.6 Usage

`Usage.tsx`.

- **Top bar:** API Costs / Subscriptions / Local Runtime; period 24 Hours / 7 Days / 30 Days; **Export CSV…**.
- **API Costs:** usage alerts, then two views.
- **Summary view:**
  - Estimated Savings (§2.2, Home).
  - Total: known cost (actual + estimated) and unknown-cost count (not in total).
  - Chart: cost, requests or tokens per hour or day; periods with unknown costs are marked; a **Comparison** metric shows savings over time.
  - Token Mix: new, cached, cache-written, generated; as tokens or cost.
  - Routing: whether requests stayed on one model.
  - The comparison by client and by how the model was asked for.
  - Repeated tool output (measured only).
  - By Model, By Client, Task Results tables. A row opens Requests filtered to it.
- **Requests view:**
  - Filters: client, model; all / fallbacks and failures / unknown cost.
  - Columns: time, client, model, tokens in → out, cost, source, time taken, result. **Show more** goes further back.
  - Row detail: model asked for and model that answered, cost source, how and why the route was chosen, key, prompt cache, attempts, routing session.
- **Subscriptions:** requests and tokens per subscription connection, limits as last stated or Unknown. No prices.
- **Local Runtime:** requests and tokens per local connection, API cost $0.
- **Empty:** "No gateway requests in this period" with **Open CLI Tools**.

### 5.7 Endpoint

`Endpoint.tsx`, `Clients.tsx`. Where tools send requests, and their keys.

- **Gateway card** as on Home, plus Copy OpenAI URL and Copy Anthropic URL.
- **Base URLs:** OpenAI-compatible `<base>/v1` (Chat Completions, Responses); Anthropic at the root; the model `auto`. The Pygg API key goes as bearer token or `x-api-key`.
- **API Keys** (one per app):
  - Table: name, token (**Show** reads it from the Credential Manager), last used, this month's spend, **Limits…**, **Revoke…**. A revoked key can be deleted.
  - **Create API Key…:** name, optional monthly limit, policy; then the key with **Copy API Key** and how to set `PYGG_API_KEY`.
  - **Limits…:** monthly limit, strict limit, unknown price, requests at once, behaviour after a server error, auto minimum tier, may start tasks, data use, allowed connections.

### 5.8 Settings

`SettingsView.tsx`. Sections in order:

| Section | Holds |
| --- | --- |
| General | Language: System, English, Tiếng Việt |
| Task budgets | Daily spending limit for paid task steps |
| Background (desktop only) | Open at sign-in, keep running in the tray, notify about spending, notify about tasks (§2.5) |
| Gateway | Link to Endpoint for port and start-up |
| Appearance | Note that the app follows the system |
| Diagnostics | **Save Diagnostics…**: one redacted zip, same as `pygg-cli diagnostics --out PATH`; contents listed before saving |
| Usage History | Ledger and route-decision retention: 30 days, 90 days, 1 year, Forever. Shortening confirms with how many requests would be deleted |
| About | Version, licence, Source Code, Report an Issue, third-party licences, log and daily backup locations |

## 6. Sheets

A sheet (`Sheet` in `ui.tsx`) is a modal dialog:

- The window behind is inert; focus moves in, Tab stays inside, focus returns on close.
- `Esc` closes unless busy; clicking the backdrop does not.
- `ConfirmSheet` asks before a destructive or costly action and names what will happen.

### Models for a role

`RoleSheet.tsx`, ADR-040. Opened by a card's **Change…** and **Add a fallback…**, the planner pick's "Why these models?", and **Change…** on Brain Storming.

- **Header:** `Models for code · facts as of 2026-11-18`; or *ranked by tier; facts off* (with **Turn facts on** in the footer); *no facts yet; ranked by tier*; *could not refresh* when the kept feed is stale.
- **Bar:** search; with a brainstorm, **Eligible under my policy** / **All connections**.
- **Row:** `connection/model · tier · billing · ≈$0.05 per run` (route estimate; without a brainstorm, output price per Mtok or *price unknown*); evidence line (your runs, a fact with linked source, or tier); *current*, *your saved pick*; **Not eligible: <reason>** under the client's policy; **Why this model?** (chain saved pick › your runs › facts feed › tier, winner marked, then each source line: what it measures, agent, effort, votes, licence, link; facts over a year old marked as not ranking); **Use**. Rows follow the role's order ([BRAIN.md](BRAIN.md) §1).
- **States:** loading; load error; no search match; nothing eligible (hint: All connections); no enabled model.
- **Footer:** **Refresh facts**, **Cancel**. Escape closes; focus returns to the opener.

### A project's overrides

`BrainOverride.tsx`, opened by **Override…** / **Edit…** on Brain Storming's Projects that differ. Title `<name>: what differs`.

- One row per role: `Default: <Brain Storming pick or by the rules>`, `This project: <its pick or the default>`, **Override…** (role sheet) or **Change…**, **Use default**. A pick saves when chosen.
- WIP limits row: **Own limits for this project**, Running, Checking, **Save limits** (off: follows defaults).
- **Done** closes.

### 6.2 Connect Provider and Test Model

**Connect Provider…:** a provider picker with search, grouped Free tier, Trial credits, Paid, On this computer, Custom, Web session (unofficial). Then:

- An API key (stored in the Windows Credential Manager, never shown again), or **Sign In with OpenRouter…**. A local runtime needs no key.
- **Name, endpoint and billing:** the name (also the model prefix), endpoint, billing.
- Connecting reads the model list to check the key; nothing is billed. If the list can't be read, the connection is still saved ("Account Saved; Models Not Read").
- A web session first shows its risks, acknowledged per connection.

**Test Model** (separate step): **Test** on a model row (§5.4) sends one short request through the gateway (`gateway/model_test.rs`). A paid or unpriced model asks first. Shows the answer and time; the request appears in Usage.

## 7. Components

- **Shared** (`components/ui.tsx`): `Sheet`, `ConfirmSheet`, `Switch`, `Segmented` (radio group, one Tab stop, arrows and Home/End move), `MenuButton`, `CopyButton`, `CodeBlock`, `Badge`, `FormRow`, `Empty`, `ErrorNote`, `Spinner`, `SkeletonRows`.
- **Screen-specific:** attempt sequence (`AttemptSequence.tsx`), diff viewer (`DiffView.tsx`), command palette (`CommandPalette.tsx`), health dots (`Connections.tsx`), cost chart (`CostOverTime.tsx`).
- **Rules:** a failed load shows the error and a retry, never an empty state; essential actions are never hover-only; ids, paths, commands, output and ledger values stay selectable; every string goes through `t()` (`lib/i18n.ts`), Vietnamese in `locales/vi`.
