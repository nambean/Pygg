# Pygg — Screens

> **Status:** Current · the app's screens as built · reviewed 2026-11-14

How Pygg's interface is laid out and how each screen behaves. Code: `apps/desktop/src/main.tsx` (the shell), `apps/desktop/src/components` and `apps/desktop/src/components/gateway` (the screens), `apps/desktop/src/styles.css` (tokens and layout). Code comments cite the section numbers below.

Most people turn Pygg on, point their coding tool at it and keep working in their IDE or terminal, so the gateway screens come first and follow the order of setting it up. Smart tasks are an experimental second part. Their signature element is the **attempt sequence** (`AttemptSequence.tsx`): one chip per attempt, showing its tier and outcome, joined by arrows, so a failed attempt visibly leads to the next step.

## 1. Principles

- **Gateway first.** The sidebar leads with the gateway's screens, named as other local AI gateways name theirs. Smart tasks sit in their own group, marked Experimental.
- **Honest values.** Unknown, estimated and actual figures are kept apart, and an unknown cost is never shown as $0 (§3).
- **Approvals happen in the app.** A paid task step waits for the user in task detail. A notification never names a task or a path and has no approve button, and no API can approve a step.
- **Nothing spends or deletes without a confirmation.** Spending actions and destructive actions ask first. The command palette only opens the screen that asks.
- **Status in place, not in banners.** Gateway status sits in the sidebar footer and on Home's gateway card. A problem is shown next to the thing it affects.
- **Writes only to Git workspaces.** A task works in a worktree of a Git repository, and the user's own checkout is never changed.

### D12: traffic-light window controls

On Windows and Linux the native title bar is turned off and the app draws macOS-style traffic-light buttons (`WindowControls.tsx`), the same as the sibling DownFetch app. On macOS the system draws its own. This departs from Windows conventions on purpose, so the controls make up for it:

- Each button has an accessible name. The green one is named **Maximize** or **Restore**, depending on the window's state.
- Windows 11 snap layouts work: the core lays an invisible native window over the green button (`snap.rs`), which maximizes or restores on click and reports hover so the glyphs still show.
- The glyphs show on hover and keyboard focus. The buttons dim when the window is inactive.

## 2. Shell

### 2.1 Layout

- A 224px sidebar and the main area. The toolbar is 52px high. Body text is 13px.
- The sidebar can be hidden with the toolbar's sidebar button or `Ctrl+Shift+L`.
- Below 900px of window width (or a 1024px window at 125% text size), the sidebar becomes a 56px rail of icons. Labels stay available to screen readers and show on hover, and counts become small badges.
- A **Skip to content** link is the first Tab stop. After moving to another screen, focus goes to the page heading, unless the page has already put focus in itself.
- The browser preview (not the desktop app) shows one line under the toolbar saying it runs on demo data.

### 2.2 Sidebar and the gateway screens

The navigation:

| Group | Screens | Shortcut |
| --- | --- | --- |
| **Gateway** | Home, Providers, Endpoint, CLI Tools, Combos, Usage, Policies | `Ctrl+1` … `Ctrl+6` for Home to Usage |
| **Smart Tasks · Experimental** | Tasks, Workspaces, Agents, Brain Storming | `Ctrl+7` Tasks, `Ctrl+8` Agents |
| **App** | Guide, Settings | `Ctrl+/` Guide, `Ctrl+,` Settings |

- Rows are 28px high, with an accent-coloured icon and a quiet selected background.
- **Tasks** shows an emphasized count of tasks needing attention (§5.1). **Workspaces** shows an emphasized count of workspaces whose checks need review.
- The app always starts on Home.

**Footer.** The gateway's status and the app version. The status is a button that opens Home. It names the local gateway's own state: `Gateway Running · <port>`, `Gateway Stopped`, `Gateway Couldn't Start`, `Connecting…`, or `Core isn't responding` when the core does not answer. A provider outage is reported on that connection (§5.4) and never turns a healthy gateway into "offline".

The gateway screens that have no section of their own in §5 are described here. Providers is §5.4, Usage §5.6 and Endpoint §5.7.

#### Home

Is Pygg working, and what is it costing:

- **Gateway card.** Running, stopped or couldn't start, "Loopback only · other devices can't reach it", **Start Gateway** or **Stop**, Copy URL, and **Configure…** (§6).
- **Usage alerts.** Things in the ledger worth a look, such as unpriced models. Each opens Usage or Providers.
- **Estimated Savings**, shown once there are requests in the last 24 hours (`Savings.tsx`, `gateway/savings.rs`). Answered requests are priced again at a comparison model the user chooses, with the same tokens:
  - The section shows the difference and the percentage, how many answered requests were compared, and three figures: the cost priced at the comparison model, the cost through Pygg, and the difference.
  - **Where the answers came from** is one bar of free, local, subscription, paid and billing-unknown answers, with counts and shares.
  - What was left out is listed: answers with an unknown cost, answers without token counts, subscription answers, and failed attempts that may have been billed.
  - **How this estimate works** is a disclosure that explains the method.
  - With no comparison model chosen, the section offers the user's most-used paid model that clients asked for by name in the period. The user accepts it or picks another.
  - It never claims more than the ledger shows. An answer from a free, local or plan model used what the user already has, and a fallback is not a saving in itself.
- **Get Started**, until all three steps are done:
  1. **Connect a provider.** Done once a connection is on, has its key and lists models, and `auto` has a model that qualifies for a coding agent's request.
  2. **Set up your coding tool.** Opens CLI Tools.
  3. **First request answered.** Done when a request from one of the user's own keys is answered.
- **Last 24 Hours.** Requests, known cost, how many requests had an unknown cost (not in the total), fallback attempts and failures, then the six newest requests with their model, client, tokens, cost and outcome, and **Open Usage**.

#### CLI Tools

Point a coding tool at Pygg (`CliTools.tsx`):

- **One-Click Setup.** Each tool gets a Pygg API key of its own.
  - **Claude Code: models through Pygg** (`launcher.rs`): **Set Up** makes a key that is kept in the Windows Credential Manager, and the row shows the command that starts Claude Code through Pygg (`pygg run claude`). Claude Code's own settings are not changed, so plain `claude` keeps the user's Claude plan.
  - **Codex** and **OpenCode** (`integrations.rs`): **Set Up…** shows what will change. Pygg then backs up the tool's configuration, writes the change and checks it. **Remove…** puts the configuration back and revokes the key.
- **Status of each set-up tool,** read from the newest request sent with its key: *Configured, waiting for its first request*, *Working: answered by <model> <time>*, or *Its last request did not succeed*, with **View Failed Requests**. Nothing is sent to find out.
- **Before set-up,** a note appears when `auto` has no model that qualifies for that tool's requests on its API format, linking to **Connect Provider…** or **How auto chooses**. With no provider at all, one warning sits above the list instead. Set Up still works.
- **Set up by hand** (a disclosure): Claude Code without the launcher, OpenCode, editors, the OpenAI SDK and curl. Each has a model choice (auto, auto/cheap, combos, enabled models) and a shell choice where it matters.

#### Combos

Named, ordered model lists (`Combos.tsx`). A request for the combo's name tries each member in turn. A segmented control switches between two tabs.

**Combos tab:**

- One card per combo, showing:
  - its strategy (**Ordered fallback**, or **Cheapest first**, which orders members for each request by estimated cost),
  - the members as a chain, each with its billing, tier and tool support, and marked when its connection is disabled or removed,
  - the connections that share a quota group,
  - **Copy Name**, **Test**, **Edit…** and Delete.
- **Test** sends one short request to the combo after a confirmation, and says which member answered and how many failed before it.
- **New Combo…** and **Edit…** open the combo editor (a sheet):
  - Members are found by typing part of a model id or a connection name. Every word typed must match.
  - The list stays open under the field. The arrow keys move through it, and Enter adds the model while the field keeps focus.
  - A model already in the combo is marked as such.
- **From Template…** builds a combo from rules over the user's models (`lib/comboTemplates.ts`): Free first, Use my plans first, Subscription only, Strong coding with a cheap backup, No metered spend, Local only, Long-context coding, and Cheapest for each request. Before saving, it previews the members it picked, why it picked them, and the models it left out with their reasons.
- A disclosure explains when the next member is tried: a rejected key, exhausted quota or an unavailable model, and never once an answer has started streaming.

**How auto chooses tab** (`AutoRouting.tsx`):

- What `auto` and `auto/cheap` mean.
- A simulator: choose the alias, a client (or a new client with the defaults), the request format, tools and images. It shows which models would be tried, in what order and why, and every excluded model with its reason. It sends nothing.

#### Policies

Every rule the gateway applies, in one place (`Policies.tsx`):

- **Defaults for New API Keys:** minimum model tier, monthly limit, limit mode (Estimated or Strict) and unknown prices (Block or Allow). These apply whoever creates the key: Endpoint, `pygg gateway create-client`, or a tool's set-up.
- **API Key Policies:** one row per key, showing its monthly limit, minimum tier, requests at once, unknown prices, paid retries and allowed providers. **Limits…** opens the same sheet as Endpoint.
- **Connection Policies:** each connection's data-use terms, quota (its own, shared, or assumed shared), pool and billing.
- **Always On:** the guarantees that are not settings.
  - The gateway listens on loopback only.
  - Keys stay in the Credential Manager.
  - An unknown cost is never $0.
  - Paid task steps are approved only in the app.
  - There is no telemetry.
- **Smart Tasks (on hold):** the daily limit for paid steps (changed in Settings), and a link to Workspaces.

#### Agents

The official agent CLIs on this computer (`Agents.tsx`), each with its own sign-in, which Pygg never reads:

- **Agents on This Computer:** each agent's version and sign-in, or **How to Install**.
- **Run a Task:** a one-off run in a worktree of a chosen repository.
- **Let Your Agent Delegate Tasks:** one-click MCP set-up for Claude Code, Codex CLI and OpenCode, and **Set It Up by Hand** for Cursor and other JSON-configured clients.

#### Brain Storming

The one-time set-up for `pygg/brain` applied to every project (`BrainStorming.tsx`, ADR-042, [BRAIN.md](BRAIN.md) §4.7b). Settings only: nothing on it sends anything to a model.

- **Models by role:** plan, code, test and review, each with the saved pick (`connection/model`, tier, billing, the evidence that ranks it: your runs, a fact with its source and date, or the tier), **Change…** (the role's ranking in a select) and **Clear**; a cleared role reads "By the rules (no saved pick)".
- **Defaults for a new brainstorm:** the runner, the WIP limits for Running and Checking, whether a planner picked inside a brainstorm is saved as the plan role, and what **Change…** on a card offers first.
- **Facts and prompt:** the model facts switch, its build date, when it was fetched, **Refresh**, the sources with their licences, row counts and dates, and the brain prompt `brain-v1` with **Open** into Skills & Prompts.
- **Projects that differ:** every workspace, with its own picks when it has any, **Use defaults**, and **Workspaces**.
- **How it starts:** the command per agent that starts a brainstorm; there is no text box.

#### Guide

How to use Pygg, in the order people set it up, with every command ready to copy (`Guide.tsx`). The sections are Quick start, Claude Code, Codex CLI, OpenCode, Editors and SDKs, Command line and npm, auto and combos, Costs and limits, Troubleshooting, and FAQ. The Guide sends nothing and changes no setting.

### 2.3 Toolbar

The toolbar holds:

- the sidebar toggle,
- the screen title,
- the command palette button,
- the screen's primary action: **Connect Provider…** on Providers, **Create API Key…** on Endpoint, **New Combo…** on Combos, and **New Task** on Tasks and task detail.

On Tasks and task detail, the subtitle (`BudgetSubtitle.tsx`) reads `<n> running · <n> waiting · <recorded> recorded + <reserved> reserved / <limit> today`. With no daily limit set, the money part reads `<amount> on paid steps today`.

Clicking the subtitle opens the popover **Today's budget for paid steps**. It shows the amount recorded, the amount reserved for running work, the daily limit and the number of runs with an unknown cost. It also explains the rules:

- A run that reported no cost counts at its cap.
- Free, local and subscription steps are not included.
- The budget resets at 00:00 UTC.

### 2.4 Banners

Replaced. The desktop app shows no banners. Gateway state is on Home and in the sidebar footer, and problems appear next to what they affect.

### 2.5 Tray and notifications

`shell.rs`. Settings → Background (§5.8) controls all of this.

- **Tray icon.** Its menu has **Open Pygg** and **Quit Pygg**.
- **Keep running in the tray.** When this is on, closing the window keeps the gateway and tasks running. The first time, a notification says Pygg is still running.
- **Open Pygg when you sign in.** Pygg starts in the tray, without its window.
- **Task notifications:**
  - a task is waiting for your approval,
  - a task is ready (its checks passed),
  - a task needs your review,
  - a task did not finish.
- **Spending notifications,** once a day each and checked every ten minutes:
  - a client has reached its monthly limit,
  - a client is near its monthly limit,
  - spending is up,
  - many requests are failing.
- A notification never names a task or a file, and never carries an approve button.

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

- While a sheet is open, these shortcuts do nothing. The sheet takes every key.
- The **command palette** (`CommandPalette.tsx`) has the groups Go to, Gateway, Tasks and App.
  - Typing filters the commands; every word must match.
  - The arrow keys move, `Ctrl+Home` and `Ctrl+End` jump to the ends of the list, Enter runs the command and `Esc` closes the palette.
  - No command spends money or deletes something. Those commands only open the screen that asks.

## 3. Showing values honestly

### 3.1 Identity

A gateway model is shown as `<connection>/<model>`, which is the name a client sends. The connection's name is the prefix. A provider row shows the connection's label next to its `id/…` prefix; a long label is truncated, the prefix never is. A task attempt names the agent and model of its step, with its tier and billing. Ids, paths and model names are selectable.

### 3.2 Provenance

A figure says where it comes from:

- A ledger row's cost has a source, such as reported by the provider, your price × reported tokens, connection marked free, local model, no price recorded for this model, or failed after reaching the provider (billing unknown).
- Rate limits are shown "as the provider last said", from the headers of its last answer.
- An account balance shows the provider endpoint it was read from and when.
- A health check shows when it ran.
- A connection's data-use terms are the user's own record, not something Pygg checks.

### 3.3 Money

`lib/money.ts`, `lib/format.ts`.

- **Actual** amounts look like `$0.08`. **Estimated** amounts look like `≈$0.0031`. Small amounts get four decimals, and an amount under $0.0001 shows as `<$0.0001`.
- **Unknown** is the word "Unknown", never `$0.00`, and it is never added to a total. Totals say how many requests had an unknown cost.
- **Subscription** requests count against the plan's quota, not money. In API Costs they count as unknown.
- **Local** models cost `$0` in API money. Electricity and the computer's time are not measured.
- **A task's cost** reads like `API $0 · local`, `<actual> + ≈<estimated> est.` or `<n> calls unknown`.

### 3.4 Status words

A status always pairs an icon or dot with a word, so colour is never the only signal.

**Task states:**

- Checks passed
- Finished, needs your review
- Manually accepted
- Check failed
- Blocked by policy
- Canceled
- Interrupted · needs review
- Waiting for approval

**Attempt outcomes:**

- Passed
- Finished, not checked
- Check failed
- Running
- Waiting for approval
- Stopped
- Skipped: not answering

## 4. Visual tokens

These are DownFetch's semantic tokens and green accent, defined in `styles.css`. They follow the system's light, dark and increased-contrast settings, and there is no theme switch in the app.

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

- **Contrast,** as noted in the stylesheet:
  - White semibold text on the accent: 5.4:1 in light mode, 4.7:1 in dark mode.
  - Accent text on the surface: 6.6:1 in light mode, 9.6:1 in dark mode.
  - Increased contrast strengthens separators and secondary text.
- **Type:** 13px body text, 15px toolbar title, 11px sidebar group headings, and 12px monospace for ids, commands, paths and output. Money and token figures use tabular figures.
- **Colour:** the accent marks the primary action. Warnings use orange, always with an icon and text.
- **Focus:** a 3px accent outline, which also survives Windows high contrast.
- **Reduced motion:** spinners and switch transitions stop. The status text stays.

## 5. Screens

### 5.1 Tasks

The task list (`TaskList.tsx`).

- **Filters.** A segmented control at the top: All, Running, Needs Approval, Completed, Failed and Stopped, each with its count.
  - Running excludes tasks that are waiting for approval.
  - Completed covers checks passed, manually accepted and needs review.
  - Failed covers check failed and blocked.
  - Stopped covers canceled and interrupted.
- **Search** filters by title (`Ctrl+F`).
- **Needs Attention.** On All, these tasks are pinned on top and not repeated below: waiting for approval, interrupted, plan needs revalidation, and needs review. Their button reads **Review**; other rows read **Open**.
- **Row.** A state icon, the title, the workspace and the client that started the task, a status line (phase, attempt, time), the attempt sequence, and the cost so far (§3.3).
- **Brainstorm rows** (§5.2.2) sit beside the tasks on All, and under Needs Approval while they wait on the user; **Review** opens one.
- **Board** (§5.1.1), the last entry of the segmented control, with the count of cards waiting on the user.
- **States:**
  - Loading.
  - A load error with **Try Again**. A failed load is never shown as an empty list.
  - Ready for Your First Task, with **New Task**.
  - No Tasks Match "…", with **Clear Search**.
  - An empty state for each filter.

#### 5.1.1 Board

The cards of a workspace's approved brainstorms as Kanban columns (`Board.tsx`, ADR-042, [BRAIN.md](BRAIN.md) §4.6). It reloads every three seconds; the app's tick pulls cards into Running under the limit.

- **Header:** the workspace, or a switcher when several have cards; `Running 1/1 · Blocked 2 · Ready 6 · Review 1 · Done 3`; **Measures** as a disclosure (cycle time median, cards done today, time blocked today) once five cards are done, before that how many are; **Use the default limits** when the workspace has its own.
- **Columns:** Discovery, Ready, Running, Checking, Blocked, Review, Done. Running and Checking show `count / limit` and **Limit…**, which edits the number in place (never below 1). Blocked is named in orange. An empty column says **Nothing here**.
- **Card:** the title (▲ when expedited), the role chip, the task's attempt chips, the first model of the ladder and how many follow, the Blocked reason and `blocked 2 h 10 min`, the phase and time while it runs, "held by you" or what it waits for in Discovery, "next in line" in Ready, "checks passed" or "finished" in Review and Done.
- **Buttons:** **Review** (Blocked, Review) or **Open** (elsewhere) opens the task detail; in Ready, **Expedite** / **Standard** and **Back to Discovery**; in Discovery on a held card, **Move to Ready**; **Drop** (confirmed) on a card without a task; **Hand to the engine** on a card waiting for the IDE agent. Running, Checking, Review and Done are the engine's; there is no drag.
- **Width:** a single list grouped by column, Blocked first, when the board is narrower than 980 px.
- **States:** loading; a load error with **Try Again**; no cards yet, with how a brainstorm starts; no workspace yet.

### 5.2 Task detail

What the task is waiting on, its result, and every attempt (`TaskDetail.tsx`). It reloads every two seconds while the task runs, and **‹ Tasks** goes back to the list.

- **Waiting for approval.** This is shown inline, not in a sheet.
  - **A paid step:** the range of what the last runs of that agent and model cost (by the agent's own estimate), or a note that there is no estimate yet. It also shows what is left of today's budget, and a cap field. The cap is **Spending cap** when the agent can stop itself, or **Stop further steps above** when it cannot. The buttons are **Approve up to $…** and **Decline**.
  - **A step in a sensitive workspace:** **Allow This Step** or **Decline**.
- **Result.** Shown when the checks passed or the task needs review.
  - **Review Changes** shows a file-by-file diff (`DiffView.tsx`) with **Copy Diff**.
  - Then a branch title and **Keep as Branch**, **Discard…** (which confirms first) and **Export Patch…**, which saves a file `git apply` takes.
  - Once the result is kept or discarded, the page says so.
- **Attempts.** One row per attempt, each showing:
  - the step, tier and billing,
  - its cost: Pygg's ledger, the agent's own estimate, or unknown, with the cap,
  - its checks,
  - disclosures for what it was told about earlier attempts, the check output and the agent output.
  
  Below the rows: the cost so far, and why a cost is unknown.
- **Task.** The steps, the approved checks, and the commit and folder the task started from.
- **Actions:** **Stop Task** while it runs, and **Retry** for an interrupted task.

#### 5.2.2 Brainstorm

A brainstorm from `pygg/brain` (`BrainstormDetail.tsx`, ADR-041, [BRAIN.md](BRAIN.md) §4.4). It is a row under Tasks (a lightbulb, the requirement's first line, the session, the planner, open questions and cards), pinned under Needs Attention while it waits for a pick, is proposed, or failed; it reloads every two seconds while it waits or thinks.

- **Waits for your pick.** Inline, like a paid step: the plan role's ranking under the client's policy as radio rows (`connection/model`, tier, billing, the reason), the rule's pick first, **Remember this planner for every project** on by default, **Decline** and **Use <model>**. With nothing eligible, the reasons and only **Decline**.
- **Questions.** Read-only: each open or answered. The caption says answers come from the coding agent.
- **Cards**, grouped by wave, a role chip each: the title, `connection/model` (selectable, wraps), the reason the rule gave, and a disclosure with the done criterion, the dependencies, the runner (Pygg engine, or My IDE agent) and the class. **Change…** opens a select of the role's ranking (each line with its source and date, or its tier) and asks whether to apply the choice to every card of the role; **Drop** strikes a card through and offers **Restore**. Under the model, the card's **fallbacks** ("then …", each with its reason): the two the rule suggested from the role's route, **Remove** on each, and **Add a fallback…**, a select of the same ranking, up to five. On approval each model is a step of the card's task, tried in order when an attempt fails.
- **Approve.** The count of kept cards, a workspace, an agent (installed ones; OpenCode first, since only OpenCode through Pygg runs a card on the model Pygg picked), **Decline brainstorm**, and **Approve <n> cards…**, which confirms and then lists the tasks made. The button waits while a card has no eligible model.
- **Planner failed:** the reason, what the planner answered behind a disclosure, and the pick to choose another planner and run again.

#### 5.2.1 New Task

`NewTaskView.tsx`.

- **Task:** the description of the work.
- **Workspace:** a repository chosen from the list, or added with **Add…**.
- **Steps:** a strategy (Free First, Balanced or Quality) proposes the ladder of agent steps.
  - Each step has an agent, a model, a tier and a billing, and can be moved up or down or removed.
  - **Add Step** adds one.
  - Steps marked paid wait for approval before they run.
- **Shell commands:** an **Allow** switch. When it is off, agents may read and edit files only. Codex always runs in its own sandbox.
- **Start Task** creates the task and opens its detail.
- With no agent installed, the screen says so and points to Agents.

### 5.3 Workspaces

The repositories tasks run in (`WorkspacesView.tsx`). A task from an IDE adds its repository by itself.

- **Add Workspace…** takes the folder of a Git repository.
- **Each workspace** shows:
  - its folder,
  - **Privacy**: Standard, or Sensitive, where every step asks before an agent sees the code,
  - **Paid steps per day here**: a limit on top of the overall one,
  - **Checks**: detected checks are suggestions until approved. A check can be added by hand, and **Detect** looks again. Approved checks run in order and stop at the first failure.
  - **Remove**.
- A badge marks a workspace whose checks are waiting for review. With no approved check, a finished attempt waits for review instead of being confirmed.

### 5.4 Providers

The connections (`Connections.tsx`). The toolbar's primary action is **Connect Provider…** (§6.2).

- **List.** Each row shows:
  - the provider logo, the label and its `id/…` prefix,
  - the preset, billing and model count, and how many models are eligible for auto,
  - the last 24 hours as one dot per hour, with a health score,
  - a state badge and an on/off switch.
- **Above the list:** **Check All** checks every enabled API connection. With more than three connections there is also a search and a filter (all, needs attention, local, Anthropic API).
- **Auto readiness line.** Under the list: how many models `auto` may choose for a coding tool's request, and how many miss a fact, are unavailable now, or are outside the rules. Each count opens the connection that has most of them, with its models filtered.
- **Detail** of the selected connection:
  - **Authentication,** with **Add Key…** or **Replace Key…**.
  - **Health:** the dot strip, the last check and **Check Now** (or **Check Sign-In** for a web session), and current cooldowns and blocks.
  - **Key ring:** keys from other accounts, used in turn.
  - Rate limits as the provider last stated them, the account balance where the provider reports one, and billing.
  - **Advanced:** endpoint, API format, independent quota (which confirms first), pool, data-use terms and model names.
  - **Check & Refresh Models** and **Remove Connection…**.
- **Models.**
  - Search and a filter: all, enabled, free, paid or unknown with no price, and the auto groups.
  - **Add Model…** adds a model by name.
  - A fixed-column table of Model, Price per 1M tokens, Enabled, and actions.
  - Each model line shows its billing, its auto facts, and whether auto would choose it (or why not). Once measured, it also shows its answer rate and median latency over 30 days, with the sample size.
  - **Test** is Test Model (§6.2). **Edit Price…** (or **Set Price…**), **Auto Routing Facts…** and **Remove Model** are in a More menu.
- **Empty state:** No Connections Yet, with **Connect Provider…**. After the first connection, a note offers **Open CLI Tools**.

### 5.5 Policies

Replaced. Policies is a gateway screen now, described in §2.2.

### 5.6 Usage

What went through the gateway (`Usage.tsx`).

- **Top bar:** API Costs, Subscriptions or Local Runtime; the period (24 Hours, 7 Days or 30 Days); and **Export CSV…**.
- **API Costs** opens with the usage alerts, then a choice of two views.
- **Summary view:**
  - Estimated Savings (§2.2, Home).
  - The total: known cost, made of actual and estimated amounts, and how many requests had an unknown cost (not in the total).
  - A chart of cost, requests or tokens per hour or per day. A period with unknown costs carries a marker. A **Comparison** metric shows the savings estimate over time.
  - Token Mix: new, cached, cache-written and generated tokens, as tokens or as cost.
  - Routing: whether requests stayed on one model.
  - The comparison taken apart by client and by how the model was asked for.
  - Repeated tool output (measured only).
  - By Model and By Client tables, and Task Results.
  
  A row of any of these tables opens Requests, filtered to that row.
- **Requests view:**
  - Filters for client, model, and all, fallbacks and failures, or unknown cost.
  - The ledger columns are time, client, model, tokens in → out, cost, source, time taken and result. **Show more** goes further back.
  - Opening a row shows the model asked for and the one that answered, the cost source, how the route was chosen and why, the key, the prompt cache, the attempts of the request and the routing session.
- **Subscriptions:** requests and tokens for each subscription connection, and the limits as the provider last stated them, or Unknown. No request has a price here.
- **Local Runtime:** requests and tokens for each local connection, at an API cost of $0.
- **Empty state:** "No gateway requests in this period", with **Open CLI Tools**.

### 5.7 Endpoint

Where tools send their requests, and the keys they use (`Endpoint.tsx`, `Clients.tsx`).

- **Gateway card,** as on Home, with Copy OpenAI URL and Copy Anthropic URL.
- **Base URLs** to copy:
  - the OpenAI-compatible `<base>/v1` (Chat Completions and Responses),
  - Anthropic at the root,
  - the model `auto`.
  
  A Pygg API key is sent as the bearer token or `x-api-key`.
- **API Keys:** Pygg's own keys, one per app.
  - The table shows each key's name, the token (Show reads it back from the Credential Manager), last used, and this month's spend, with **Limits…** and **Revoke…**. A revoked key can be deleted.
  - **Create API Key…** asks for a name, an optional monthly limit and a policy, then shows the key with **Copy API Key** and how to set it as `PYGG_API_KEY`.
  - **Limits…** covers the monthly limit, a strict limit, unknown price, requests at once, behaviour after a server error, the auto minimum tier, whether the key may start tasks, data use, and which connections it may use.

### 5.8 Settings

`SettingsView.tsx`. Sections in order:

- **General:** language (System, English, Tiếng Việt).
- **Task budgets:** the daily spending limit for paid task steps.
- **Background** (desktop only): open Pygg at sign-in, keep running in the tray, notify about spending, notify about tasks (§2.5).
- **Gateway:** a link to Endpoint for the port and start-up.
- **Appearance:** a note that the app follows the system.
- **Diagnostics:** **Save Diagnostics…** saves one redacted zip, the same as `pygg-cli diagnostics --out PATH`. What it holds is listed before anything is saved.
- **Usage History:** how long the ledger and route decisions are kept (30 days, 90 days, 1 year or Forever). A shorter period confirms first, with how many requests would be deleted.
- **About:** the version, the licence, Source Code, Report an Issue, third-party licences, and where the log and daily backups are kept.

## 6. Sheets

A sheet (`Sheet` in `ui.tsx`) is a modal dialog:

- The window behind it is inert.
- Focus moves into the sheet, Tab stays inside it, and focus returns to where it was when the sheet closes.
- `Esc` closes it unless it is busy. Clicking the backdrop does not.
- `ConfirmSheet` asks before a destructive or costly action, and names what will happen.

### 6.1 Setup Assistant

Replaced by Home's **Get Started** steps (§2.2).

### 6.2 Connect Provider and Test Model

**Connect Provider…** opens a sheet with a picker of providers. They are grouped by how to start (Free tier, Trial credits, Paid, On this computer, Custom, and Web session (unofficial)), with search.

Then:

- An API key (stored in the Windows Credential Manager and never shown again), or **Sign In with OpenRouter…** for OpenRouter.
- Under **Name, endpoint and billing:** the name, which is also the model prefix, the endpoint, and the billing.
- A local runtime needs no key.
- Connecting checks the key by reading the model list, and no request is billed. If the model list can't be read, the connection is still saved and the sheet says so ("Account Saved; Models Not Read").
- A web session first shows its risks, which the user acknowledges for that connection.

**Testing a model is a separate step.** **Test** on a model row (§5.4) sends one short request through the gateway (`gateway/model_test.rs`):

- A paid or unpriced model asks first, saying it may cost a little.
- The result shows the answer and the time taken, and the request appears in Usage.

### 6.3 Approve Paid Attempt

Replaced. A paid step is approved inline in task detail (§5.2).

## 7. Components

Shared controls in `components/ui.tsx`:

- `Sheet` and `ConfirmSheet`
- `Switch`
- `Segmented`: a radio group with one Tab stop, where the arrow keys and Home/End move the choice
- `MenuButton`
- `CopyButton` and `CodeBlock`
- `Badge`
- `FormRow`
- `Empty`
- `ErrorNote`
- `Spinner` and `SkeletonRows`

Screen-specific pieces:

- the attempt sequence (`AttemptSequence.tsx`)
- the diff viewer (`DiffView.tsx`)
- the command palette (`CommandPalette.tsx`)
- the health dots (`Connections.tsx`)
- the cost chart (`CostOverTime.tsx`)

Rules:

- A failed load is never turned into an empty state. It shows the error and a way to try again.
- Essential actions are never hover-only.
- Ids, paths, commands, output and ledger values stay selectable.
- Every string goes through `t()` (`lib/i18n.ts`), with Vietnamese in `locales/vi`.
