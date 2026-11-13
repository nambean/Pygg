# Pygg — Screen Design

> **Status:** Current in §2.2 (the navigation as built since 2026-11-12), Historical elsewhere (the v0.1 screen design, kept for its reasoning) · reviewed 2026-11-13 · order: [ROADMAP](../ROADMAP.md) · built: [Implementation Status](../IMPLEMENTATION-STATUS.md)

> **Update 2026-11-11 (ADR-027):** tasks are started from the user's IDE agent through MCP; the app monitors, approves and reviews. New Task → Analyze → Plan → Run as the main path, "Needs Revalidation" and the first-run screens that end in analyzing a task are superseded. See [ROADMAP](../ROADMAP.md#revised-plan-after-adr-026-and-adr-027-2026-11-11-docs-review).


> Version: 0.1 | Date: 2026-11-10 | Status: historical task-first design; gateway-first screens now implemented (see Implementation Status)
>
> Inputs: [plan v1.1](../SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md), the DownFetch desktop UI (`F:\2026\DownFetch\apps\desktop\src`), and the `apple-design` skill (Apple HIG). Claude and Codex each designed the screens independently, then settled their differences in a second round. All model names, prices, quotas and results below are illustrative.

The app should feel like a task runner with an auditable route, not a provider dashboard. Its signature element is the **attempt sequence**: chosen model → check → outcome → next decision.

> **Design status note (2026-11-11, navigation renamed 2026-11-12):** Gateway-first Home (was Overview), Providers (was Connections), Endpoint and API Keys (was Clients), CLI Tools, Combos, Usage, Policies, Guide, Agents, Tasks and Workspaces screens are implemented per [Implementation Status](../IMPLEMENTATION-STATUS.md). The task-first/six-destination design in this file is a historical concept, **not current implementation**. Future priority follows [ROADMAP.md](../ROADMAP.md) and the [review reconciliation](../reviews/2026-11-11-review-response.md). Current local gateway port is **8741**; older `4318` examples below are superseded.

## 1. Decisions

| # | Decision | Notes |
| --- | --- | --- |
| D1 | **List-first, no Overview.** Home is the task list, as in DownFetch. | Items that need action are pinned under "Needs Attention" at the top |
| D2 | **Six destinations:** Tasks, Connections, Policies, Usage, Endpoints, Settings | "Route" is renamed Tasks; there is no History tab |
| D3 | **Task filters live in the sidebar:** All Tasks, Running, Needs Approval, Completed, Failed, Stopped | Same pattern as DownFetch's Downloads group |
| D4 | **Workspaces get a sidebar group and a Workspace Settings page.** | Privacy class, checks, isolation and policy overrides belong to the workspace, not the task |
| D5 | **Task detail has phases:** New Task → Plan → Run → Result | One screen whose state changes; the prompt and route explanation stay visible throughout |
| D6 | **MVP handoff is Keep as Branch**, with Export Patch… and Discard Result… as alternatives | Apply to Workspace comes later |
| D7 | **Status goes in the toolbar subtitle, not a separate strip.** Gateway status goes in the sidebar footer and tray. | Blocking gateway failures also appear next to the affected action |
| D8 | **An approval is durable, covers exactly one attempt, and is never automatic.** | Notifications never contain task names or paths |
| D9 | **Each fact carries its own provenance**, with no generic `Live` badge | Unknown, estimated and stale are separate properties |
| D10 | **One no-check rule:** without a deterministic check there is no automatic cheap-first cascade. The user can review a direct run at the strategy's normal tier instead. | Manual acceptance is never shown as "Checks Passed" |
| D11 | **MVP writes only to Git workspaces** that can be isolated. | Other workspaces get read-only planning (not built: workspaces must be Git repositories) |
| D12 | **Traffic-light window controls on Windows too**, as in DownFetch (`WindowControls.tsx`). | Chosen by the team on 2026-11-10 so its apps look alike |

### D12: a deliberate platform departure

Codex recommended native Windows controls instead. The team chose traffic lights for consistency with DownFetch, so this is recorded as a deliberate departure from Windows conventions. The custom controls must pass these checks before release:

- Close, minimize and maximize/restore work with the mouse and the keyboard; double-clicking the drag region maximizes and restores.
- Windows snap layouts and `Win+Arrow` snapping still work.
- Hit targets stay usable at 100–200% display scaling.
- Each control has an accessible name and role that screen readers announce correctly.
- Inactive-window and hover states match DownFetch.

## 2. Shell

### 2.1 Layout

1440×900 baseline: a 224px sidebar and a 1216px main area. The toolbar is 52px. Body text is 13px. The sidebar can be hidden. At 1024px or with enlarged text, inspectors collapse into full-width detail views.

```text
+----------------------+--------------------------------------------------------------+
| ● ● ●  Pygg           | [◧] All Tasks · smarttaskrouter            [New Task] [...]  |
|                      |     2 running · 1 waiting · $3.42 recorded + $0.30 reserved  |
| Tasks                |     / $10 today                                              |
| > All Tasks      18  +--------------------------------------------------------------+
|   Running         2  | (banners: setup incomplete, approval waiting elsewhere, …)    |
|   Needs Approval  1  |                                                              |
|   Completed      11  |  content                                                     |
|   Failed          2  |                                                              |
|   Stopped         2  |                                                              |
|                      |                                                              |
| Workspaces           |                                                              |
| > smarttaskrouter    |                                                              |
|   docs-site          |                                                              |
|   Add Workspace…     |                                                              |
|                      |                                                              |
| Manage               |                                                              |
|   Connections     !  |                                                              |
|   Policies           |                                                              |
|   Usage              |                                                              |
|   Endpoints          |                                                              |
|                      |                                                              |
| App                  |                                                              |
|   Settings           |                                                              |
|----------------------|                                                              |
| ● Gateway Running    |                                                              |
| v0.1.0               |                                                              |
+----------------------+--------------------------------------------------------------+
```

### 2.2 Sidebar

**Navigation since 2026-11-12 (owner: setup as familiar as 9router and OmniRoute; reviewed with Codex).** Most people turn Pygg on and then work in their own IDE or CLI. The sidebar therefore follows the order of setting it up, with the names those gateways use:

| Screen | What it holds |
| --- | --- |
| **Home** | Gateway status. Before the first use: two steps, *Connect a provider* (done once a connection is on, has its key and lists models) and *Set up your coding tool* (opens CLI Tools). After: today's requests, cost, unknown-cost count, alerts |
| **Providers** | The connections (code: `Connections.tsx`). Each row shows its last 24 hours as dots and a health score (2026-11-13), and the header has **Check All**. The detail shows authentication, health (the strip, the last check, Check Now), account and billing; endpoint, API format, rate limits, quota, pool, data use and model names sit under **Advanced** |
| **Endpoint** | Gateway card (start/stop, port), the base URLs to copy (OpenAI-compatible `/v1`, Anthropic root, model `auto`), and **API Keys**: Pygg's own client tokens with their limits (code: `Clients.tsx`) |
| **CLI Tools** | One-click setup (Codex, OpenCode: backup, own key, write, check, undo) and *Set Up by Hand* (Claude Code, OpenCode, editors such as Cline and Continue, OpenAI SDK, curl), then whether the newest request worked |
| **Combos** | Tabs **Combos** (cards: ordered fallback, members with billing, tier and tools, how many are off or missing) and **How auto chooses** (explainer and simulator). **From Template…**: Free first, Use my plans first, Subscription only, Strong coding with cheap backup, No metered spend, Local only, Long-context coding, as rules over the user's models (`lib/comboTemplates.ts`) resolved into a saved list, with a preview of members, reasons, exclusions and missing stages |
| **Usage** | The Estimated Savings hero, then spending, routing, requests and task results |
| **Policies** | Defaults for new API keys (applied in `store::create_client`, whoever creates the key), each key's policy (the Limits sheet), each connection's terms, quota and pool, the guarantees that are not settings, and the on-hold task rules |
| **Smart Tasks · Experimental** | Tasks, Workspaces, Agents (agent CLIs, MCP delegation, Run a Task). On hold; the app no longer opens on a waiting task, the count still shows it |
| **Guide** (App) | Quick start, Claude Code, Codex, OpenCode, editors and SDKs, the command line and npm, auto and combos, costs, troubleshooting, FAQ; every command with Copy (`Ctrl+/`) |
| **Settings** | App preferences, background, diagnostics, task budgets |

Home leads with the **Estimated Savings** hero after the gateway card and alerts: the amount, the percentage, the comparison model, how many answered requests were compared and what was left out. With no comparison model chosen, it proposes the paid, priced model clients asked for by name most often, never the most expensive one, and the user accepts it.

Not built yet from the Combos review: a combo route preview and a live combo test, drag to reorder, and the strategies worth adding next (cost order within a combo, round robin across independent quota groups, lowest measured latency).

Shortcuts: `Ctrl+1`…`Ctrl+6` the gateway screens in that order, `Ctrl+7` Tasks, `Ctrl+8` Agents, `Ctrl+,` Settings.

Built the same day, from that review:
- Each set-up tool shows *Configured, waiting for its first request → Working (answered by which model, when)* or *its last request failed*, from the newest request of the tool's own key (`client_latest_requests`).
- **Claude Code launcher** (`launcher.rs`, `pygg run claude`): a key of its own kept in the Windows Credential Manager and handed to Claude Code at launch with the gateway's address and `auto`. Claude Code's settings are not changed, so plain `claude` keeps the user's Claude plan.
- **Estimated Savings** (`gateway/savings.rs`) on Usage and Home: the answered requests priced again at a model the user picks, with the same tokens. It counts what it leaves out (unknown cost, no tokens, failed attempts that may have been billed, subscription requests) and is labelled as an estimate.

Built 2026-11-13: a note under each tool, before Set Up, when `auto` has no eligible model for that tool's requests on its API surface, linking to Connect Provider or How auto chooses. `auto` is described as choosing an eligible model within the user's rules: it does not judge how hard a prompt is.

The notes below predate this navigation where they name Overview, Connections or Clients.

- Groups and rows follow DownFetch (`SidebarGroup`, `SidebarItem`): 28px rows, accent icons, quiet selected background, counts in secondary text. **Needs Approval** uses the emphasized count.
- **Tasks** filters are views of one collection. Running excludes tasks waiting for approval. Stopped covers canceled and interrupted tasks, with the difference shown in the row. Completed is a navigation category, not a correctness claim; rows still say **Checks Passed** or **Manually Accepted**.
- **Workspaces:** selecting a workspace scopes the current filter and its counts to that workspace. Selecting it again returns to all workspaces. The toolbar title always names the scope ("All Tasks · smarttaskrouter") so filtered counts are not mistaken for totals.
- **Connections** shows a `!` when a connection needs attention (expired credentials, unreachable local runtime).
- **Footer:** gateway status, like DownFetch's `RuntimeStatus`, and the app version. Clicking the status opens a popover with address, upstream summary and **Open Endpoints**.

Gateway status wording separates the local service from upstream availability:

| State | Footer text |
| --- | --- |
| Running | `Gateway Running` (popover: `127.0.0.1:4318`) |
| Running, upstream issue | `Gateway Running · 1 provider unavailable` |
| Starting | `Gateway Starting…` |
| Stopped | `Gateway Stopped` |
| Unknown | `Gateway Status Unavailable · checked 14:32` |

A provider outage never turns a healthy local gateway into "Offline".

### 2.3 Toolbar

Contains the sidebar toggle, the title with a subtitle, the screen's primary action, and a More menu (`MoreMenu`).

- **Subtitle (task screens):** `2 running · 1 waiting · $3.42 recorded + $0.30 reserved / $10 today`. When unknown costs make the remaining budget uncertain, it reads `2 running · 1 waiting · Budget needs review`. Clicking it opens the **budget popover**, which shows actual vs. estimated spend, reservations, calls with unknown cost, and which limit applies (task, workspace, daily, monthly, client).
- Reservations are capacity held for running work, not spend. Subscription allowance and local operating cost are never added to the API figure.

### 2.4 Banners

Banners sit under the toolbar, using DownFetch's `banner` / `copied-banner` patterns: one line, an action, and dismissible when that is safe.

- `Setup isn't finished.` → **Continue Setup**
- `One task is waiting for approval.` → **Review** (shown when another screen is focused; it does not take focus)
- `Gateway stopped. Tasks and clients can't reach models.` → **Start Gateway**
- `One interrupted task needs review.` → **Review**

### 2.5 Approvals when away

| Context | Behavior |
| --- | --- |
| Viewing the affected task | Persistent approval panel above the output, with **Review Escalation** |
| Another screen focused | Quiet banner; focus is not taken |
| Window unfocused or closed to tray | Tray state changes and one OS notification is sent (if permitted) |
| Notifications blocked | The task keeps waiting; the tray menu and the next window opening show it |
| Another sheet is open | The approval is queued; only one sheet is shown at a time |
| App restarted | Pending approvals are recovered and revalidated; if anything changed, a fresh decision is required |

Notification: title **Task Waiting for Approval**, body *A task is paused before a paid attempt.* There is no task name or path, and no approve button in the notification. Clicking it opens that task (not built yet, ROADMAP A1).

Tray menu:

```text
Pygg
Gateway Running
2 Tasks Running
1 Task Waiting for Approval
─────────────────────────
Open Pygg
Review Waiting Task
─────────────────────────
Quit Pygg…
```

Closing the window keeps the core running when **Keep Running When Window Closes** is on; this is explained once, at the first close. **Quit** with active work warns that quitting stops execution and may leave billable partial requests.

### 2.6 Keyboard and command palette

`Ctrl` on Windows, `Cmd` on macOS.

| Shortcut | Action |
| --- | --- |
| `Ctrl+N` | New Task |
| `Ctrl+1` … `Ctrl+6` | All Tasks, Running, Needs Approval, Completed, Failed, Stopped |
| `Ctrl+K` | Command palette (groups: Tasks, Navigate, Connections, Diagnostics) |
| `Ctrl+F` | Search the current list, output or diff |
| `Ctrl+,` | Settings |
| `Ctrl+Shift+L` | Show/hide sidebar |
| `Ctrl+Enter` | **Analyze Task** while editing a task |
| `Ctrl+S` | Save in Policies, Workspace Settings and Settings editors only |
| `Esc` | Close palette, sheet or popover; **never** cancels a run or discards edits |

- No shortcut moves from analysis to **Run Task**, and Enter never authorizes spending.
- Spending and destructive commands in the palette open their review UI instead of acting directly.
- A sheet makes the rest of the window inert (`inert`, as in DownFetch) and takes all keys.

## 3. Showing values honestly

### 3.1 Identity

A model is always shown as **Provider / Account · Model**, e.g. `OpenRouter / Personal · openai/gpt-4.1-mini`. The same model can have different billing and terms on different accounts. Full IDs are selectable and copyable.

### 3.2 Provenance

Each fact carries its own source and time:

| Display | Meaning |
| --- | --- |
| `Provider API · checked 14:32` | From an official endpoint |
| `Response headers · observed 14:28` | From rate-limit headers |
| `Ledger estimate · since Nov 1` | Computed from Pygg's own ledger |
| `Unknown · provider does not report this` | No reliable source |
| `Stale · last checked Nov 3` | Older than the freshness window; shown with a clock icon |

Unknown, estimated and stale can apply together (a ledger estimate can be stale). Stale never means zero or disconnected.

### 3.3 Money and allowance

- Estimate: `≈ $0.04–$0.12`, labeled "Estimated".
- Actual: `$0.08`.
- Unknown: the word `Unknown` in secondary text with an explanation. Never `$0.00`, never sorted as zero.
- Subscription: `Window usage unknown`, or a figure with its source. Never converted to money.
- Local: `API cost: $0 · local operating cost: not configured`.
- Rate limits, plan allowance and credits are kept apart; their percentages are never combined.

### 3.4 Status words

Status always pairs an icon with a word; color is never the only signal. Outcome words: **Checks Passed**, **Manually Accepted**, **Check Failed**, **Canceled**, **Interrupted**, **Blocked by Policy**, **Waiting for Approval**.

## 4. Visual tokens

Reuse DownFetch's semantic tokens (`--window`, `--sidebar`, `--surface`, `--label`, `--secondary`, `--separator`, …) unchanged, **including its green accent**. The owner chose this on 2026-11-11; it replaces the orange accent first proposed here.

| Role | Light | Dark |
| --- | --- | --- |
| Window | `#F5F5F7` | `#1E1E1E` |
| Sidebar | `#ECECEF` | `#262628` |
| Surface | `#FFFFFF` | `#2C2C2E` |
| Label | `#1D1D1F` | `#F5F5F7` |
| Secondary | `#636366` | `#A1A1A6` |
| Accent fill (`--accent`) | `#0B7A4B` | `#0D8551` |
| Accent icon (`--accent-icon`, sidebar icons) | `#0B7A4B` | `#3DDC97` |
| Accent text (`--accent-text`) | `#0A6B42` | `#3DDC97` |

These are DownFetch's own values and contrast notes: white semibold text on `#0B7A4B` 5.4:1 (light) and on `#0D8551` 4.7:1 (dark), light accent text 6.6:1 on the surface, dark accent text 9.6:1 on the surface. `--accent-icon` is the one token DownFetch doesn't have; it reuses the accent text color in dark mode so sidebar icons stay legible. Re-check these ratios with a contrast tool before release.

**Gateway first (ADR-016):** the sidebar now leads with **Gateway** (Overview, Connections, Combos, Clients, Usage); the task screens sit under **Smart Tasks** (Tasks, Policies), with Agents and Workspaces beside them. Endpoints (§5.7) became Overview plus Clients, and Connections (§5.4) has the Providers part with models and prices inline. Agents is built (and Tasks with it); Models-as-a-tab is not.

- Type: 13px body, 11–12px supporting text, 15px toolbar title, 17px section emphasis; 12px monospace for IDs, checks, paths and output. Use tabular figures for money and tokens.
- Controls are 26–32px high; grouped boxes have 8–10px corners; content margins are 20–24px.
- The green accent marks the primary action. Warnings use orange, always with an icon and text.
- Follows the system's light, dark and increased-contrast settings, with no theme switch. Reduced motion removes animated activity effects but keeps the status text.

## 5. Screens

Wireframes show only the 1216px main area. The shell from §2.1 is implied.

### 5.1 Task list (home)

**Goal:** see what needs action and find any task. **Primary action:** **New Task**.

```text
| All Tasks · smarttaskrouter                                  [New Task] [...] |
| 2 running · 1 waiting · $3.42 recorded + $0.30 reserved / $10 today          |
+------------------------------------------------------------------------------+
| [Search Tasks…                                  ]        [Workspace Settings…]|
|                                                                              |
| Needs Attention                                                              |
| ⚠ Fix timeout handling          Waiting for approval · attempt 2   [Review]   |
|   T0 ✕ ─→ T1 ◌                                                               |
|                                                                              |
| Tasks                                                                        |
| ◌ Add retry tests               Checking · attempt 1 · 1m 12s      [Open]     |
|   T0 ◌                                                    API $0 · local     |
| ◌ Update parser                 Generating · attempt 2             [Open]     |
|   T1 ✕ ─→ T2 ◌                                            $0.03 so far       |
| ✓ Update README                 Checks passed · 8 min ago          [Open]     |
|   T0 ✓                                                    API $0             |
| ✕ Rename config key             Canceled · 23 min ago              [Open]     |
|   T1 ✕                                                    $0.02              |
|                                                                              |
| 1–25 of 68 tasks                                          [<] [1] [2] [3] [>] |
```

- **Row:** status icon, title, phase and attempt, elapsed or relative time, actions. A second line shows the **attempt sequence** (tier chips with outcome, connected by arrows) and cost so far. The workspace name appears in the row only when the scope is all workspaces.
- **Needs Attention** holds waiting approvals, interrupted tasks and plans that need revalidation. These rows are not repeated in the list below.
- Uses DownFetch's list bar, search, rows, row details disclosure and `Pager` (25/50/100 per page).
- **States:**
  - First launch: empty list "Route Your First Task — Connect a way to run tasks, add a workspace, and review a route before anything runs." → **Set Up Routing**. The Setup Assistant sheet opens automatically once.
  - Ready, no tasks: "Ready for Your First Task" → **New Task**.
  - Filter empty: "No Failed Tasks", or "No Tasks Match 'timeout'" → **Clear Search**.
  - Loading: "Restoring tasks…"; no placeholder running rows.
  - Load error: "Tasks couldn't be loaded." → **Retry**. A failed load is never shown as an empty list.
- **Keyboard:** arrows select, Enter opens, `Ctrl+F` searches. Returning from a task restores the filter and scroll position.

### 5.2 Task detail

One screen with four phases. The toolbar title becomes the task title and the subtitle shows its state.

#### 5.2.1 New Task

**Goal:** describe the work within the workspace's boundaries. **Primary action:** **Analyze Task**.

```text
| New Task · smarttaskrouter                                                   |
+------------------------------------------------------------------------------+
| Task                                                                         |
| +--------------------------------------------------------------------------+ |
| | Fix timeout handling and add regression tests.                           | |
| |                                                                          | |
| +--------------------------------------------------------------------------+ |
|                                                                              |
| Strategy      [Free First ▾]                                                 |
|                                                                              |
| From Workspace                                    [Workspace Settings…]     |
| ┌──────────────────────────────────────────────────────────────────────────┐ |
| │ Path        F:\2026\smarttaskrouter · Git · worktree isolation ready     │ |
| │ Privacy     Sensitive · providers with unknown data terms excluded       │ |
| │ Checks      npm run typecheck · npm test                    [Choose…]    │ |
| │ Permissions Files: workspace · Shell: checks + agent tools · Network:    │ |
| │             provider requests only                   [Restrict…]         │ |
| └──────────────────────────────────────────────────────────────────────────┘ |
|                                                                              |
| [Analyze Task]                                                 Ctrl+Enter    |
```

- The workspace comes from the sidebar scope; if the scope is all workspaces, a workspace picker appears first.
- Privacy, isolation and permissions are inherited from the workspace. A task can **tighten** them (**Restrict…**) or pick among approved checks, but cannot loosen them.
- Detected checks are suggestions until approved in Workspace Settings. "Detected from package.json; not yet run."
- **States:** no way to run tasks yet ("Connect a provider and agent, or a supported subscription agent." → **Open Connections**); no workspace ("Add a Workspace"); scanning ("Finding available checks…"); no check ("No deterministic check is selected, so automatic escalation is off." → **Choose Check** or **Review Direct Run**); workspace not isolatable ("This workspace can't be isolated for changes." → read-only planning); analysis needs a paid call (shown with its estimate and approval before any context is sent); analysis error → **Retry Analysis**.
- **Keyboard:** focus starts in the task field; Enter adds a line; `Ctrl+Enter` analyzes.

#### 5.2.2 Plan

**Goal:** understand and authorize the route. **Primary action:** **Run Task**.

```text
| Fix timeout handling                                        [Edit Request]  |
| Plan ready · Bug fix · low complexity · tools required  [Analysis Details]  |
+------------------------------------------------------------------------------+
| Proposed Route                                                               |
| ┌──────────────────────────────────────────────────────────────────────────┐ |
| │ ▾ Implement Fix and Tests                                                │ |
| │   OpenCode · Ollama / This Computer · qwen2.5-coder:7b     Local · T0   │ |
| │   Check   npm run typecheck && npm test                                  │ |
| │   Cost    API $0 · local operating cost not configured                   │ |
| │                                                                          │ |
| │   Why This Model   Lowest eligible tier for this task · supports tools   │ |
| │   On Check Failure Next eligible T1 model — paid, asks you first         │ |
| │   [3 Excluded Models ▸]  [Change Model…]                                 │ |
| └──────────────────────────────────────────────────────────────────────────┘ |
|                                                                              |
| Up to 3 attempts · task API limit $1.00                                      |
| Full estimate unavailable: later attempts depend on check results.           |
|                                                                              |
| [Run Task]                                            [Recheck Eligibility]  |
```

- **One expandable row per stage**, with three always-visible lines (who runs it, the check, the cost). Reasons, exclusions and fallback sit inside the row. Multi-stage plans repeat the same row.
- **Excluded Models** expands to `Model — reason` lines: "No tool calls", "Context 8k < needed 24k", "Terms unknown (Sensitive workspace)", "Quota exhausted · Provider API · 14:30".
- **Change Model…** lists eligible candidates only, each with its why line.
- The copy says "lowest eligible tier", not "cheapest": v1 does not measure total cost.
- **States:** no eligible candidate (the stage stays visible and blocked, with reasons); estimate unavailable (the reason is given, never $0); **Needs Revalidation** after a catalog, connection or workspace change; model disabled after planning (Run is disabled, with an explanation); privacy denial (shows the deciding rule).
- Any edit to the task, check, model, strategy or workspace invalidates the affected analysis. Eligibility and budget are checked again when Run is pressed.

#### 5.2.3 Run

**Goal:** follow the work and respond when needed. **Primary action:** **Review Escalation** while waiting; otherwise **Cancel Run** is the main control.

```text
| Fix timeout handling                                          [Cancel Run]  |
| Waiting for approval · 4m 02s                                                |
+------------------------------------------------------------------------------+
| ⚠ Attempt 1 failed its check. A paid T1 attempt is available.               |
|                                                        [Review Escalation]  |
|                                                                              |
| Attempts                                                                     |
| ✕ 1  Ollama / This Computer · qwen2.5-coder:7b · Local T0 · 2m 14s          |
|      npm test: 1 failed · changes discarded                                  |
| ◌ 2  OpenRouter / Personal · openai/gpt-4.1-mini · Paid T1 · awaiting approval|
|                                                                              |
| [Output] [Checks] [Route Decision] [Usage]                                   |
| ┌──────────────────────────────────────────────────────────────────────────┐ |
| │ 14:32:04  Regression test failed                                         │ |
| │           timeout.test.ts: expected rejection after 500 ms               │ |
| │ 14:32:06  Attempt worktree discarded                                     │ |
| │ 14:32:06  Escalation requested                                           │ |
| └──────────────────────────────────────────────────────────────────────────┘ |
| [Find in Output]                                       Follow Latest: On     |
|                                                                              |
| API $0 recorded · local operating cost not configured                        |
```

- Attempt rows are the signature element at full size. Each attempt's model identity is fixed once it starts.
- "1 of 3 attempts used" is a limit, not progress. Use activity indicators for generating and checking, and never a fake percentage.
- **Output:** selectable monospace text with secrets redacted. Auto-follow stops when the user scrolls up, and **Follow Latest** resumes it. New lines never move focus. Screen readers get summarized state changes, not every token.
- **States:** preparing isolation; queued; generating; checking; waiting for approval; rate-limited (retry time only if the provider gave one); output disconnected ("Live output is unavailable. Checking task status…"); canceling ("Stopping the agent and its tools…", cancel disabled); interrupted (state is reconciled before retry is offered); budget blocked ("The next attempt would exceed the available budget."); no permitted escalation (ends with the check failure and the reasons).
- **Cancel confirmation:** "Cancel this run? The current attempt's changes will be discarded. Usage already incurred stays in the ledger." → **Keep Running** / **Cancel Run**.

#### 5.2.4 Result

**Goal:** judge the passing result and take it home. **Primary action:** **Keep as Branch**.

```text
| Fix timeout handling                  [Keep as Branch] [Export Patch…] [...] |
| Checks passed · 2 attempts · 4m 08s · $0.08 actual                           |
+------------------------------------------------------------------------------+
| Changes are isolated. Your working tree has not been touched.                |
|                                                                              |
| [Changes] [Checks] [Attempts] [Route Decision] [Usage]                       |
| ┌───────────────────────┬──────────────────────────────────────────────────┐ |
| │ Files                 │ src/router.ts                                    │ |
| │ ▸ src/router.ts  +21-6│ @@ -40,7 +40,9 @@ timeout handling              │ |
| │   timeout.test.ts +13 │ -  return await operation();                     │ |
| │                       │ +  return await withTimeout(operation, ms);      │ |
| │ 2 files · +34 −8      │                                                  │ |
| └───────────────────────┴──────────────────────────────────────────────────┘ |
| Checks   typecheck passed · 18 tests passed                                  |
| Base     7d3e9c1 (main)                                       [Copy]         |
| Estimate ≈ $0.05–$0.15 · actual $0.08 (incl. $0 for failed attempt 1)        |
```

- **Keep as Branch** commits the passing result and creates a durable branch `str/fix-timeout-<short-id>` from the base revision. The user's current branch is never switched. Afterwards the screen shows the branch name and copyable commands ("`git switch str/fix-timeout-3f2a`" / "`git merge str/fix-timeout-3f2a`"). Kept branches are exempt from automatic cleanup.
- **Export Patch…** saves a `.patch` file. **Discard Result…** (in More) removes the unkept result but keeps history and ledger records.
- The diff is unified by default; side-by-side only when there is room. All text is selectable, and `Ctrl+F` searches the diff.
- Say **Checks Passed**, never "Correct" or "Safe".
- **States:** no file changes ("Checks passed, but this attempt made no file changes."); failed or canceled tasks (diagnostics and usage only, no result actions); base moved since the run ("main has moved 3 commits since this run. The branch still starts from 7d3e9c1."); result removed by retention ("The diff was removed under your retention settings. Run details remain."); keep or export failure (the result is kept, with **Try Again**).
- Later: **Apply to Workspace…** with drift and conflict detection, shown as a separate **Applied** state.

### 5.3 Workspaces

**Add Workspace…** opens a sheet: choose a folder → detect Git and isolation readiness → detect checks → choose privacy class → **Add Workspace**. A folder without Git can be added for read-only planning only (not built: a workspace must be a Git repository).

**Workspace Settings** is a page opened from the toolbar or the task list, with a back button to the list.

```text
| ‹ Tasks   Workspace Settings · smarttaskrouter                   [Save]     |
+------------------------------------------------------------------------------+
| Location                                                                     |
| ┌──────────────────────────────────────────────────────────────────────────┐ |
| │ Folder        F:\2026\smarttaskrouter                      [Show]        │ |
| │ Repository    Git · main · clean                                         │ |
| │ Isolation     Git worktree per attempt · ready                           │ |
| └──────────────────────────────────────────────────────────────────────────┘ |
| Privacy                                                                      |
| ┌──────────────────────────────────────────────────────────────────────────┐ |
| │ Privacy class [Sensitive ▾]                                              │ |
| │               Excludes providers that may train on data or whose terms   │ |
| │               are unknown.                                               │ |
| │ Context       Files in this folder only                                  │ |
| └──────────────────────────────────────────────────────────────────────────┘ |
| Checks                                                                       |
| ┌──────────────────────────────────────────────────────────────────────────┐ |
| │ ☑ npm run typecheck     Detected · package.json                          │ |
| │ ☑ npm test              Detected · package.json                          │ |
| │ ☐ npm run lint          Detected · package.json                          │ |
| │ [Add Check…]                                                             │ |
| └──────────────────────────────────────────────────────────────────────────┘ |
| Policy                                                                       |
| ┌──────────────────────────────────────────────────────────────────────────┐ |
| │ Effective strategy  Free First (global default)                          │ |
| │ Overrides           Daily API limit $5.00            [Edit in Policies…] │ |
| └──────────────────────────────────────────────────────────────────────────┘ |
|                                                                              |
| [Remove Workspace…]                                                          |
```

- Uses DownFetch's `SettingsView` group boxes.
- Changes apply to new plans; existing tasks keep their snapshots; unexecuted plans need revalidation.
- **Remove Workspace…** explains that it removes the workspace from Pygg only and leaves files and kept branches untouched.

### 5.4 Connections

Tabs: **Providers · Models · Agents**. Each tab is a searchable list on top, with the selected item's details in grouped boxes below or beside it (a stacked inspector at narrow widths).

#### Providers

**Primary action:** **Connect Provider…**

```text
| Connections                                           [Connect Provider…]   |
+------------------------------------------------------------------------------+
| [Providers] [Models] [Agents]          [Search Connections…    ] [Refresh]  |
|                                                                              |
| Provider / Account          Status            Allowance                      |
| ▸ OpenRouter / Personal     ● Connected       Unknown                        |
|   OpenAI / Project API      ⚠ Key rejected    Last known $8.20 · Stale Nov 3 |
|   Ollama / This Computer    ● Running         Not applicable                 |
|                                                                              |
| OpenRouter / Personal                                                        |
| ┌──────────────────────────────────────────────────────────────────────────┐ |
| │ Authentication  API key · stored in Windows Credential Manager           │ |
| │ Models          12 enabled                                               │ |
| │ Remaining quota Unknown · provider does not report this                  │ |
| │ Health          Reachable · 420 ms · checked 14:32                       │ |
| │ Data use        Terms unknown                         [Review Terms…]    │ |
| └──────────────────────────────────────────────────────────────────────────┘ |
| [Refresh Metadata] [Reconnect…] [...]                                        |
```

- "Connected" means the credentials and connection worked; it does not mean every model is eligible.
- Errors are distinguished: **Key rejected**, **Provider unreachable**, **Local runtime not running**, **Quota exhausted**, **Rate-limited**, **Disabled**.
- More menu: **Rotate Credentials…**, **Disable Connection**, **Remove Connection…** (explains the effect on running tasks). A test that sends an inference asks for consent and shows the possible cost.

#### Models

**Primary action:** review a model.

```text
| [Providers] [Models] [Agents]   [Search Model IDs…  ] [Provider ▾] [Billing ▾] [Filters ▾] |
|                                                                              |
| Model                       Account                Billing  Tier  Status    |
| ▸ qwen2.5-coder:7b          Ollama / This Computer Local    T0    Available |
|   openai/gpt-4.1-mini       OpenRouter / Personal  Paid     T1    Available |
|   custom-code-model         Custom / Lab           Unknown  —     Missing capability data |
|                                                                              |
| qwen2.5-coder:7b · Ollama / This Computer                                   |
| ┌──────────────────────────────────────────────────────────────────────────┐ |
| │ Tool calls    Verified by adapter test · Nov 10                          │ |
| │ Context       32k configured · provider maximum not verified             │ |
| │ API price     Not applicable                                             │ |
| │ Tier          T0 · curated table v1                  [Override Tier…]    │ |
| │ Reliability   Not measured yet                                           │ |
| └──────────────────────────────────────────────────────────────────────────┘ |
| [Test Model…] [Disable Model] [...]                                          |
```

- A new sortable **table** component (DownFetch has none); follow the HIG guidance on lists and tables.
- Billing and provider filters are always shown; the rest sit in a **Filters** popover.
- A model is never labeled "Eligible" globally, because eligibility depends on the task. Unknown capability is shown as **Unknown** and fails the matching hard requirement.
- Reliability appears only once measured, with the sample size.

#### Agents

**Primary action:** **Add Agent…** In Phase 1 this shows the single supported agent.

```text
| Agent        Authentication         Status                                   |
| ▸ OpenCode   Gateway-routed         ● Ready                                  |
|   Codex      Native subscription    ⚠ Needs sign-in                         |
|                                                                              |
| OpenCode                                                                     |
| ┌──────────────────────────────────────────────────────────────────────────┐ |
| │ Executable        C:\Tools\opencode.exe              [Choose…]           │ |
| │ Version           detected · checked 14:32                               │ |
| │ Gateway URL       supported                                              │ |
| │ Formats           verified combinations only                             │ |
| │ Usage visibility  Reported by gateway                                    │ |
| │ Budget limits     Enforced by gateway                                    │ |
| └──────────────────────────────────────────────────────────────────────────┘ |
```

For a native subscription agent the box reads: *Usage visibility: limited to what the agent reports. Remaining allowance: unknown. Gateway API limits don't apply.* Sign-in happens in the agent's own flow; the app never asks for subscription credentials in a form.

### 5.5 Policies

**Goal:** set routing and spending boundaries. **Primary action:** **Save Policy**.

```text
| Policies                                            [Revert] [Save Policy]  |
+------------------------------------------------------------------------------+
| Scope [Global Defaults ▾]          Strategy [Free First ▾]                   |
| [Routing] [Limits & Approvals] [Privacy] [Advanced]                          |
|                                                                              |
| Limits & Approvals                                                           |
| ┌──────────────────────────────────────────────────────────────────────────┐ |
| │ Task API limit                                         [$1.00   ]        │ |
| │ Daily API limit                                        [$10.00  ]        │ |
| │ Monthly API limit                                      [$100.00 ]        │ |
| │ Paid escalation                                [Ask Every Time ▾]        │ |
| │ Unknown price                                          [Block ▾]         │ |
| │ Subscription allowance unknown               [Ask Before Using ▾]        │ |
| └──────────────────────────────────────────────────────────────────────────┘ |
| Limits reset at midnight, local time.                                        |
|                                                                              |
| Effective Policy                                                             |
| Sensitive workspaces exclude providers with unknown data terms.              |
| Applies to new tasks; running tasks keep their snapshot.      [Try a Task…]  |
```

- **Routing:** starting tier per category, maximum tier, attempts per tier, latency target. **Privacy:** privacy classes and the providers each excludes. **Advanced:** the tier table, minimum-tier rules, and allow lists.
- Each value can show where it comes from (**Inherited from Global Defaults** or **Workspace Override**), with **Use Inherited Value** to remove an override.
- Explicit save and revert, as in DownFetch's `RulesEditor`. Leaving with unsaved edits offers **Keep Editing** / **Discard Changes** / **Save Policy**.
- **Try a Task…** is a dry run against the draft policy. It runs no inference and no tools; an optional classifier call is disclosed separately.
- **States:** defaults only (**Customize Policy**); invalid value or fallback cycle (error next to the field, Save disabled); conflict (shows the deciding rule and its scope); changed elsewhere ("This policy changed elsewhere." → **Review Changes**).

### 5.6 Usage

**Goal:** reconcile recorded usage and find expensive or uncertain work. **Primary action:** inspect a ledger row.

```text
| Usage                                   [This Month ▾] [All Workspaces ▾]    |
+------------------------------------------------------------------------------+
| API Costs                                                                    |
| $12.48 actual · $1.20 estimated · 7 calls with unknown cost                  |
| Unknown costs are not included in totals.                                    |
|                                                                              |
| Budget   $13.68 recorded + $0.60 reserved / $100 this month  [Edit Limit…]   |
|                                                                              |
| [API Ledger] [Subscriptions] [Local Runtime]                                 |
| [Provider ▾] [Cost Status ▾] [Task ▾]                       [Export Ledger…] |
|                                                                              |
| Time   Task / Client       Provider / Account     Cost      Status           |
| 14:34  Fix timeout         OpenRouter / Personal  $0.08     Actual           |
| 14:12  Rename config       OpenAI / Project API   $0.02     Actual           |
| 13:48  Update README       Codex (native)         Unknown   Not reported     |
| 13:20  client: editor      OpenRouter / Personal  $0.01     Estimated        |
|                                                                              |
| Includes failed and canceled attempts that were billed.                      |
```

- Gateway-only traffic shows the client name, not a task outcome.
- **Subscriptions** tab: each plan window with known usage, source and reset time, or **Unknown**. **Local Runtime** tab: runtime and resource use; optional operating cost kept separate.
- **Edit Limit…** opens Policies with the right scope selected.
- **States:** "No Usage Recorded Yet — Run a task or connect a client to the gateway."; reconciliation pending; late provider corrections; limited native-agent reporting (coverage explained).
- Later: cost per success, defined as cohort cost (including failed attempts) divided by tasks with checks passed, shown with sample size and unknown-cost coverage. No savings charts.

### 5.7 Endpoints

**Goal:** connect an external client with clear semantics and limited access. **Primary action:** **Create Client Token…**

```text
| Endpoints                                            [Create Client Token…] |
+------------------------------------------------------------------------------+
| ● Gateway Running · 127.0.0.1:4318 · loopback only           [Configure…]    |
|                                                                              |
| Client Setup                                                                 |
| Mode [Pass-through ▾]   Format [OpenAI-compatible ▾]   Client [SDK ▾]        |
| Base URL  http://127.0.0.1:4318/v1                                 [Copy]    |
| ┌──────────────────────────────────────────────────────────────────────────┐ |
| │ base_url = "http://127.0.0.1:4318/v1"                                    │ |
| │ api_key  = os.environ["SMARTROUTER_TOKEN"]                               │ |
| └──────────────────────────────────────────────────────────────────────────┘ |
| Pass-through uses the model you request. No checks or cascade.              |
|                                                                              |
| Client Tokens                                                                |
| Name          Scope            Last Used    Budget                           |
| Local editor  Inference only   14:31        $2/day                           |
| Test script   Inference only   Never        $1/day                           |
| [Rotate Token…] [Revoke…]                                                    |
```

- Modes: **Pass-through** (requested model, no checks); **Policy Alias** (model fixed for a conversation, permitted pre-stream fallback recorded); **Task API** (later). Only supported modes are selectable.
- A new token is shown once; existing tokens stay masked and are never copied automatically. Snippets use environment-variable placeholders.
- Binding and port settings live here (**Configure…**). Turning on non-loopback binding requires an explicit explanation and confirmation.
- **States:** gateway stopped (**Start Gateway**); port in use (**Choose Port…**); no tokens; unsupported client/format combination.

### 5.8 Settings

**Goal:** control app behavior and local data. Tabs: **General · Privacy & Data · Updates · Diagnostics**. Uses DownFetch's `SettingsView` directly.

- **General:** language (System, English, Tiếng Việt), Launch at Sign-in, Keep Running When Window Closes, Notify When Approval Is Needed (with OS permission status and **Open System Settings**). Appearance is a note only: "Follows the system's light, dark and contrast settings."
- **Privacy & Data:** data folder, retention (logs, result artifacts; ledger is always kept), log redaction, telemetry (off by default, opt-in).
- **Updates:** version and check for updates (signed updater).
- **Diagnostics:** export a redacted diagnostics bundle for review before saving; benchmark entry point (Phase 1).
- Links to Endpoints for gateway settings and to Policies for limits; Settings does not duplicate them.

## 6. Sheets

One sheet at a time, with focus trapped inside, the parent `inert`, and focus returned to the launcher on close. Clicking the backdrop does not close a sheet. Uses DownFetch's `TorrentSheet` / `SetupAssistant` structure.

### 6.1 Setup Assistant

Four steps: **Choose How Tasks Run** (API/local provider + agent, or a supported subscription agent) → **Connect and Verify** → **Workspace and Checks** → **Limits and First Task**. It ends with **Analyze Sample Task**, never a paid run.

- **Finish Later** leaves a "Setup isn't finished" banner, and setup resumes at the incomplete step.
- Opening Connect Provider replaces the assistant sheet and then returns to it with state preserved, so sheets are never stacked.
- Notifications are requested when background approvals are explained. Workspace access is requested when a workspace is chosen. There are no launch-time permission prompts.

### 6.2 Connect Provider

```text
┌────────────────────────────────────────────────┐
│ Connect Provider                               │
│                                                │
│ Provider       [OpenRouter ▾]                  │
│ Account Label  [Personal                    ]  │
│ API Key        [••••••••••••••••            ]  │
│                Stored in Windows Credential    │
│                Manager.                        │
│ Data-use terms Unknown   [Review Source…]      │
│                                                │
│ Connecting checks the key and reads the model  │
│ list. Testing a model is a separate step.      │
│                                                │
│                  [Cancel] [Connect Provider]   │
└────────────────────────────────────────────────┘
```

- Local providers show a URL field and no key field.
- Errors: "This key was rejected. Check the key and its permissions." / "The provider couldn't be reached. Your key hasn't been checked yet."
- Partial success completes as **Connected · Model list unavailable**; the user is not asked to enter the key again.
- If verification needs a billable call, that is disclosed first.

### 6.3 Approve Paid Attempt

```text
┌──────────────────────────────────────────────────┐
│ Approve Paid Attempt                             │
│ Fix timeout handling · attempt 2                 │
│                                                  │
│ Why   The local attempt failed the regression    │
│       test. Its changes were discarded.          │
│                                                  │
│ From  Ollama / This Computer · qwen2.5-coder:7b  │
│       Local · T0                                 │
│ To    OpenRouter / Personal · openai/gpt-4.1-mini│
│       Paid · T1 · terms: no training             │
│                                                  │
│ Estimated cost        ≈ $0.04–$0.12              │
│ Spending cap          [$0.20]                    │
│ Task spent so far     $0                         │
│ Task API limit        $1.00                      │
│ Sends   selected files + short failure summary   │
│         [View What Is Sent]                      │
│                                                  │
│ [Not Now]  [Stop Task]        [Approve Attempt]  │
└──────────────────────────────────────────────────┘
```

- Approval covers this attempt only, up to the cap. An estimate is not a cap; the sheet explains how the cap is enforced, including any delay in usage reporting.
- **Not Now** / Esc leaves the task waiting. **Stop Task** denies the attempt and ends the task cleanly, keeping usage and diagnostics.
- Initial focus goes to the heading. Enter alone never approves. Double clicks cannot start two attempts.
- **States:** unknown token estimate with a known price (an explicit cap is required); unknown price (blocked by the default policy); budget changed while open ("The available budget changed. Review the new limit."); candidate unavailable (Approve disabled, back to the plan); approval invalidated by a policy or catalog change (never reused); already resolved (shows the outcome and **Close**).

## 7. Components

| From DownFetch | Used for |
| --- | --- |
| `main.tsx` shell: sidebar, toolbar, banners, inert sheets, shortcuts | The whole shell |
| `DownloadList` rows, details disclosure, inline confirmation, `Pager` | Task list, attempts, connection lists |
| `SettingsView` group boxes, rows, switches, pop-up buttons, footnotes | Settings, Workspace Settings, Policies forms, inspectors |
| `SetupAssistant` | Setup Assistant |
| `TorrentSheet` | Connect Provider, Approve Paid Attempt, Add Workspace |
| `SecurityView` factual wording | Isolation, checks, terms, enforcement |
| `RulesEditor` save/revert/preview | Policies |
| `RuntimeStatus`, `AppVersion` | Sidebar footer |
| `lib/i18n.ts`, `locales/vi` | All text |

New components: **attempt sequence** (chips and full rows), **sortable table**, **tabs**, **popover** (budget, gateway, filters), **output pane** (follow/pause, search, redaction markers), **diff viewer** (file list + unified diff), **code snippet with copy**.

Do not copy DownFetch's blank `Suspense` fallbacks for important screens, and never turn a failed fetch into an empty state. Essential actions are never hover-only; IDs, paths, output and ledger values stay selectable.

## 8. Phase 1 scope

| Area | Phase 1 | Later |
| --- | --- | --- |
| Shell | Six destinations, sidebar filters, workspaces group, tray, banners, approvals visible everywhere, keyboard-complete | Customizable toolbar |
| Tasks | Single-stage New Task → Plan → Run → Result; exclusions; checks; escalation; cancel; **Keep as Branch**, Export Patch, Discard; search and filters | Multi-stage editing, Apply to Workspace, run comparison |
| Workspaces | Add (Git only for changes), settings, checks, privacy class | Non-Git snapshot isolation |
| Connections | Providers, models, one supported agent with enforcement disclosure | Several agents, model comparison |
| Policies | Strategy, tiers, attempts, limits, paid approval, unknown-price rule, try-a-task dry run | Full hierarchy editor, conflict explorer |
| Usage | Ledger with actual/estimated/unknown, failed/canceled costs, subscription and local tabs, reservations | Cost per success, latency, anomalies |
| Endpoints | Pass-through and policy alias for verified formats, tokens, binding | Task API, more translations |
| Settings | Language, tray/startup, notifications, retention, redaction, telemetry opt-in, diagnostics | Backup and portability |
| Recovery | Restart reconciliation, durable approvals, interrupted state, kept results | Smarter resume |
| Accessibility | Focus management, screen-reader status, contrast, zoom/reflow, reduced motion | Full audit with the `apple-design` skill |

## 9. Next step

Build these screens in `apps/desktop` with mock data, following DownFetch's `desktop-mock` approach, so they can be reviewed in the real window rather than as static drawings. Review each screen with the `apple-design` skill, then replace the old SVG wireframes.

## 10. Proposed consumer Auto Chat and Memory & Context (not yet agreed or built)

> **Dropped 2026-11-11 (ADR-028):** the app hosts no chat of its own. This section is kept for reference only.

~~The six-destination, list-first developer task UI remains the current implemented design. A consumer-first Auto Chat flow is an additional product direction.~~ Superseded: the app is gateway-first (see the status note at the top), and Auto Chat is withdrawn (ADR-028).

**Auto Chat:** chat history/sidebar, message composer with attachments, an `Auto` strategy default; no per-turn model selection. A quiet expandable "Why this model?" inspector shows provider, routed model, usage, rationale, cache and optional switch history. Only privacy, cost-over-limit and sensitive tool actions interrupt normal chat.

**Memory & Context (Settings):** scope selector (account/workspace/conversation), list of remembered facts with source and validity, edit/delete/disable controls, conversation context usage, compaction snapshots, cross-provider disclosure, export and privacy options. Deleted memory cannot be included in newly built context packages.

**Handoff transparency:** after a provider switch, an inspector can show exactly which recent messages, summaries, approved facts and tool checkpoints were included; unsupported provider payloads are rejected rather than silently omitted.

**Source:** [Universal Shared Memory proposal](../UNIVERSAL_SHARED_MEMORY.md). No UI is implemented by this documentation update; current SVG previews predate the updated UX.

## 11. Planned CLI / IDE Integrations and provider tier UI

The design for developer mode adds **Connections → Integrations** with tabs for Terminal CLIs and IDEs/extensions, client status, setup walkthrough, generated scoped gateway token, exact local endpoint, model alias/route, connection test and approved config backup/restore. Candidate clients include Claude Code, Codex, OpenCode, Aider, Cline/Roo and Continue; actual compatibility must be verified by version and supported format. An unknown client can use manual OpenAI/Anthropic-compatible templates when appropriate.

The existing Connections → Providers tab adds filters **All · Free · Subscription · API · Local · Unknown**, showing source/date/limit for each account-model entitlement, independent of key/OAuth/CLI auth methods. One provider can legitimately appear in more than one tier. No unsupported quota estimates or promised free access.

Settings / Endpoints gain CLI health, gateway port/token controls, setup commands and npm/npx onboarding documentation. The package name and distribution are not yet verified or released.

See [full spec](../CLI_IDE_NPM_PROVIDER_TIERS.md). These are designs and do not change the implemented navigation until reviewed.

## 12. Planned Account Pools & Rotation controls

Connections gains **Accounts** with an aggregated view filtered by Free / Subscription / API Paid / Local / Unknown; each account shows source-proven quota, expiration/reset, health/cooldown and model entitlements. Policies gains **Account Selection & Failover**: Fill First, Round Robin, Sticky Round Robin, Cost Aware, pinning, per-client allowlist, maximum fallbacks and safe pre-stream retry.

The **Pool Editor** lists explicitly authorized accounts and models, priority/weight, quota group, budget and privacy requirements. A **Simulation** panel explains how a new session and a continuing conversation choose different accounts and why excluded accounts are not selected. Usage supports per-account metrics and switches; Active Sessions shows account affinity and allows releasing it at the next safe boundary.

**Reference:** [Account/API Rotation design](../ACCOUNT_API_ROTATION.md). This is a planned screen, not currently built. Do not use fake account quotas as live data.

## 13. Approved OmniRoute-inspired feature surfacing (planned)

**Consumer Auto Chat** (withdrawn, ADR-028): preserve Auto as the default; user can optionally pick Save More, Balanced or Quality. Do not show 20 routing strategies, ask a model-selection question each turn, or imply estimated savings are actual.

**Connections > Free Intelligence:** model and account entitlements, free vs promotional quotas, pooled limit/reset, terms, source and freshness; show potentially free catalog separately from personally usable free entitlement.

**Policies > Advanced routing:** Auto aliases, account pool settings, privacy/budget hard filters and clear route reasons. In developer mode, expose Health/Circuit Breaker and pre-stream fallback status.

**Usage > Optimization:** actual input/output/cache tokens, compression shadow-mode measurement, cache switches and true total cost per accepted result. Show an explicit `Unknown` for unmeasured costs.

**Compression controls:** Off, Safe (conservative exact dedup), and Experimental (only after opt-in), with a before/after preview and protected spans. Never silently compress code/tool schemas or active tool calls.

These are design decisions and not implemented screens. Full criteria: [OmniRoute feature research](../OMNIROUTE_FEATURE_RESEARCH.md).
