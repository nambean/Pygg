# Pygg — Screens

> **Status:** Current · the app's screens as built · Smart Tasks screens built, experimental and on hold

Code: `apps/desktop/src/main.tsx` (shell), `src/components` and `components/gateway` (screens), `src/styles.css` (tokens). Code comments cite the § numbers below.

## 1. Principles

- **Gateway first:** gateway screens lead the sidebar in set-up order; Smart Tasks are their own group, marked Experimental.
- **Honest values:** unknown, estimated and actual figures stay apart; an unknown cost is never $0 (§3).
- **Approvals only in the app:** a paid task step waits in task detail; no notification or API approves it (§2.5).
- **Confirm before spending or deleting** (§6); the command palette only opens the screen that asks.
- **Status in place, not banners:** gateway status in the sidebar footer and on Home; a problem shows next to what it affects.
- **Writes only to Git worktrees:** the user's own checkout is never changed.

### D12: traffic-light window controls

- Windows and Linux draw macOS-style window buttons (`WindowControls.tsx`); Windows 11 snap layouts work (`snap.rs`). Built.

## 2. Shell

### 2.1 Layout

- Hide the sidebar with the toolbar button or `Ctrl+Shift+L`; below 900px width it becomes an icon rail.
- **Skip to content** is the first Tab stop; the browser preview says it runs on demo data.

### 2.2 Sidebar and the gateway screens

| Screen | For | Status | § |
| --- | --- | --- | --- |
| **Gateway:** Home | Is Pygg working, what it costs | Built | below |
| Providers | Provider accounts, models, prices | Built | §5.4 |
| Endpoint | Gateway URL and Pygg API keys | Built | §5.7 |
| CLI Tools | Point a coding tool at Pygg | Built | below |
| Combos | Named model fallback lists; how `auto` chooses | Built | below |
| Usage | The ledger: cost, tokens, requests | Built | §5.6 |
| Policies | Every rule the gateway applies | Built | below |
| Skills & Prompts | Agent skills and what global files cost | Built | below |
| **Smart Tasks:** Work | Every plan, task and question: needs you, running, recent | Built · experimental | §5.0 |
| Projects | The repositories; a project's work and board | Built · experimental | §5.0, §5.1.1 |
| Agents & defaults | Agent CLIs, MCP set-up, and the Brain Storming defaults | Built · experimental | below |
| **App:** Guide | How to use Pygg | Built | below |
| Settings | Language, budgets, background, diagnostics, history | Built | §5.8 |

- The app starts on Home; Work shows a count of what needs you (§5.0). **New work** (`Ctrl+N`) is in the toolbar of every Smart Tasks page; `Ctrl+7` Work, `Ctrl+8` Projects, `Ctrl+9` Agents & defaults.
- **Footer:** gateway status (opens Home) and version: `Gateway Running · <port>`, `Gateway Stopped`, `Gateway Couldn't Start`, `Connecting…`, `Core isn't responding`; a provider outage shows on that connection (§5.4), never as gateway offline.

#### Home

`Overview.tsx`.

- **Gateway card:** state, "Loopback only", **Start Gateway** / **Stop**, Copy URL, **Configure…** (§6).
- **Usage alerts**, each opening Usage or Providers.
- **Estimated Savings** (`gateway/savings.rs`): answered requests repriced at a comparison model the user picks; never claims more than the ledger shows.
- **Get Started** until done: connect a provider, set up a coding tool, first request answered.
- **Last 24 Hours:** requests, known cost, unknown-cost count, fallbacks, failures, newest requests; **Open Usage**.

#### CLI Tools

`CliTools.tsx`.

- **One-Click Setup**, a Pygg API key per tool: Claude Code through `pygg run claude` (its settings untouched); Codex and OpenCode: **Set Up…** previews, backs up, writes, checks; **Remove…** restores and revokes.
- Status per tool from its newest request: *Configured*, *Working*, or *Its last request did not succeed*.
- **Set up by hand:** snippets for other tools, editors, SDKs, curl.

#### Combos

`Combos.tsx`.

- Combo cards: strategy (**Ordered fallback** or **Cheapest first**), members, **Test** (one short request, confirmed first), **Edit…**, Delete; **New Combo…**, **From Template…** (`lib/comboTemplates.ts`).
- The next member is tried on a rejected key, exhausted quota or unavailable model, never once an answer has started streaming.
- **How auto chooses** tab (`AutoRouting.tsx`): a simulator listing the models tried and excluded, with reasons; sends nothing.

#### Policies

`Policies.tsx`.

- Defaults for new API keys; each key's and each connection's policies.
- **Always On:** loopback only; keys in the Credential Manager; unknown cost never $0; paid task steps approved only in the app; no telemetry.

#### Skills & Prompts

`Skills.tsx`, ADR-039.

- **Sent With Every Request:** what global files add per agent, with estimated tokens.
- **Library:** starter and user skills; **Install** per agent; **New Skill…**, **Edit…**, **Duplicate…**, **Delete…**.
- **Projects:** **Add Project…**, **Install Skills…** per project.

#### Agents

`Agents.tsx`. An agent's sign-in is never read.

- Agents found with version, or **How to Install**; **Run a Task** once in a worktree.
- **Let Your Agent Delegate Tasks:** one-click MCP set-up for Claude Code, Codex CLI, OpenCode; snippets for others.

#### Brain Storming

`BrainStorming.tsx`, ADR-042, [BRAIN.md](BRAIN.md) §4. Settings only; sends nothing to a model.

- **Models by role** (plan, code, test, review): the saved pick, **Change…** (role sheet, §6), **Clear**.
- **Defaults for a new brainstorm:** runner, WIP limits for Running and Checking.
- **Facts and prompt:** facts feed switch, **Refresh**, prompt `brain-v3`.
- **Projects that differ:** **Override…** / **Edit…** (project sheet, §6), **Use defaults**.
- **Which level wins:** workspace override, then this screen, then the rules.

#### Guide

`Guide.tsx`. Every command copyable: quick start, each tool, costs and limits, troubleshooting, FAQ. Changes nothing.

### 2.3 Toolbar

- Sidebar toggle, title, palette button, the screen's primary action (**Connect Provider…**, **Create API Key…**, **New Combo…**, **New Skill…**, **New Task**).
- On Tasks, the budget subtitle (`BudgetSubtitle.tsx`): running, waiting, today's paid-step spend against the daily limit; click for the breakdown; resets 00:00 UTC.

### 2.5 Tray and notifications

`shell.rs`; set in Settings → Background (§5.8).

- Tray: **Open Pygg**, **Quit Pygg**; closing the window can keep the gateway and tasks running; can open at sign-in.
- Notifications: tasks waiting, ready, needing review or failed; spending limits reached or near, spending up, many failures (each once a day).
- Never names a task or file; never has an approve button.

### 2.6 Keyboard and command palette

| Shortcut (`Cmd` on macOS) | Action |
| --- | --- |
| `Ctrl+1` … `Ctrl+6` | Home, Providers, Endpoint, CLI Tools, Combos, Usage |
| `Ctrl+7` / `Ctrl+8` | Tasks / Agents |
| `Ctrl+/` / `Ctrl+,` | Guide / Settings |
| `Ctrl+K` | Command palette (`CommandPalette.tsx`); no command spends or deletes |
| `Ctrl+Shift+L` | Show or hide the sidebar |
| `Ctrl+N` / `Ctrl+F` | New Task / search tasks (on Tasks) |

## 3. Showing values honestly

### 3.1 Identity

- A model is `<connection>/<model>`; the prefix never truncates; ids, paths and model names are selectable.

### 3.2 Provenance

- Every cost, rate limit, balance, health check and fact says where it came from and when.

### 3.3 Money

`lib/money.ts`, `lib/format.ts`.

| Kind | Shown as |
| --- | --- |
| Actual | `$0.08` |
| Estimated | `≈$0.0031` |
| Unknown | "Unknown", never `$0.00`, never in a total; totals count unknown requests |
| Subscription | Plan quota, not money |
| Local | `$0` API money |

### 3.4 Status words

- A status pairs an icon or dot with a word; colour is never the only signal.

## 4. Visual tokens

- DownFetch's semantic tokens and green accent in `styles.css`; they follow the system's light, dark and increased-contrast settings.
- Focus is a 3px accent outline; reduced motion stops spinners; warnings are orange with icon and text.

## 5. Screens

### 5.0 Work and Projects

`WorkList.tsx`, `ProjectsView.tsx`, `ProjectView.tsx`. The owner's model: **a project → Brain Storming → the tasks run → the output**, or **one task**, or **a question**. Shaped after GitHub's agent sessions, Jules, Codex and Linear (one list of work with what needs you first; one page per piece of work that reads top to bottom; the board as a view).

- **Work** (home): **Needs you** (a plan to review or pick a planner for, a paid step, a result to review, a failure), **Running**, **Recent** (25, **Show more**); search. Rows: **Plan**, **Task** or **Ask**, the project, the source (**from your coding agent**), status, **Review** or **Open**. A plan's cards are not rows: they are on the plan's page.
- **Projects:** the repositories as cards (path, what waits, what runs), **Add project…**.
- **A project:** path, **Project settings**, **New work**; **Work** (the same list, this project only) or **Board** (§5.1.1).
- Each page opened from a list has a back link to that list (Work, a project, a plan).
- Partial loads say which part failed, each with **Try Again**.

### 5.1 New work

`NewTaskView.tsx`. **What do you want?**: **Ask** (a question; nothing changes, ADR-043), **Task** (one change, checked, kept as a branch), **Plan** (Brain Storming); **Project** (with **Add…**); the text; for a task the project's checks; **Cost** (if the first step succeeds, up to what if every step runs, each step free, quota, its recent runs or unknown, the stop rules, **Stop the task at $**); **Safety** (per agent: files outside its copy, shell, web, your secrets: blocked, allowed by you, or not enforced); **Agent settings** folded (strategy, the ladder, **Web access** and shell **Allow**); **Ask**, **Start task** or **Create plan**. Opens the task or the plan.

#### 5.1.1 Board

`Board.tsx`, ADR-042, [BRAIN.md](BRAIN.md) §3. Built; ran live once.

- Columns Discovery, Ready, Running, Checking, Blocked, Review, Done; Running and Checking have WIP limits (**Limit…**).
- Card actions: **Review** / **Open**, **Expedite**, **Move to Ready**, **Back to Discovery**, **Drop**; the engine moves the rest; no drag-and-drop.
- The workspace's approved checks define Done.

### 5.2 Task detail

`TaskDetail.tsx`. Tabs **Overview · Activity · Changes**.

- **Overview**, top to bottom: **Evidence** for a result (asked, started from, changed and test files, checked by, not checked: no checks, no test changed, a card's done criterion); the reason line; **Answer** (an Ask: the answer with **Copy**); the **Output** box once kept (branch, git commands; for a plan's card whether it is on the plan's branch); the result to decide (**Review Changes** diff, **Keep as Branch**, **Discard…**, **Export Patch…**); a paid or sensitive step to approve (**Approve up to $…**, **Decline**, **Allow This Step**); **Stop Task**, **Retry**; **Now**: the running step's **Live activity** (files, commands, what the model said; OpenCode and Codex as it happens, Claude Code and Gemini CLI when they finish; `live.rs`, the last 500 lines); **Steps** as a receipt (each attempt, its cost or how it is paid, its outcome and why it moved on; the total, the cap, per kept result); **Task** (text folded when long, steps, what is allowed, checks, base; **Copy brief for another agent**).
- **Activity:** every attempt in full: checks, cost, tokens, what it was told, check output, the agent's output tail.
- **Changes** (when an attempt changed files): the result's diff, or a kept solo branch against where it started.

#### 5.2.2 Plan

`BrainstormDetail.tsx`, `planParts.tsx`, ADR-041, [BRAIN.md](BRAIN.md) §2. One page from requirement to output, with the steps **Plan → Tasks → Output** on top.

- States: **Waits for your pick**, **Thinking**, **Proposed**, **Approved · board**, **Declined**, **Planner failed**.
- A paid planner waits for **Use <model>** or **Decline**.
- A plan started here (**started here**): an answer box under each open question, **Anything else to change in the plan**, **Send to the planner** (a new revision); one from a coding agent is answered there.
- Cards: model, **Change…** (role sheet, §6), up to 5 fallbacks, **Drop** / **Restore**.
- Approve bar: project (chosen explicitly), agent, known estimate, budget left; **Approve <n> cards…** puts them on the board.
- Approved: **Cards** (attention first, each with its lane and reason and **Approve…**, **Review**, **Open**, **Run again**, **Hand to Pygg** or **Move to Ready**; **View board**), **Output** (the plan's branch, git commands, **Review the changes** as one diff from where it started, results on branches of their own; marked in progress until every card is done), the plan as approved folded.

### 5.3 Workspaces

`WorkspacesView.tsx`.

- **Add Workspace…** (a Git repository); per workspace: **Privacy** (Standard or Sensitive), paid steps per day, **Checks** (detected ones are suggestions until approved), **Remove**.

### 5.4 Providers

`Connections.tsx`.

- **Connect Provider…** (§6.2); list with billing, model count, 24-hour health, on/off switch; **Check All**.
- Detail: **Add Key…** / **Replace Key…**, **Check Now**, key ring, rate limits, **Check & Refresh Models**, **Remove Connection…**.
- Models: enable, **Test** (§6.2), **Set Price…**, **Auto Routing Facts…**; answer rate and latency once measured.

### 5.6 Usage

`Usage.tsx`.

- Tabs API Costs, Subscriptions, Local Runtime; period 24 Hours / 7 Days / 30 Days; **Export CSV…**.
- Summary: savings, known cost with unknown count apart, **What Pygg sees** (priced requests, requests without a price, agent runs with no cost, subscriptions that state limits or not, tools not pointed at Pygg), chart, token mix, by model and client.
- Requests: each request with model asked and answered, cost source, route reason, attempts.

### 5.7 Endpoint

`Endpoint.tsx`, `Clients.tsx`.

- Gateway card; OpenAI-compatible `<base>/v1` and Anthropic at the root; model `auto`.
- **API Keys:** **Create API Key…**, **Show** (from the Credential Manager), **Limits…** (monthly, strict, tier, may start tasks, connections), **Revoke…**.

### 5.8 Settings

`SettingsView.tsx`: General (language), Task budgets, Background (§2.5), Gateway (link to Endpoint), Appearance, Diagnostics (**Save Diagnostics…**, a redacted zip), Usage History (retention), About.

## 6. Sheets

- A sheet (`ui.tsx`) is modal: focus moves in and returns on close; `Esc` closes unless busy.
- `ConfirmSheet` asks before a destructive or costly action and names what will happen.

### Models for a role

`RoleSheet.tsx`, ADR-040.

- Every connected model for the role, ranked ([BRAIN.md](BRAIN.md) §1), with tier, billing, estimate and evidence; **Why this model?**, **Use**, **Refresh facts**.

### A project's overrides

`BrainOverride.tsx`.

- Per role: the default and this project's pick, **Override…**, **Use default**; own WIP limits with **Save limits**.

### 6.2 Connect Provider and Test Model

- **Connect Provider…:** pick a provider (free, trial, paid, local, custom, web session); key kept in the Credential Manager, or **Sign In with OpenRouter…**; reading the model list bills nothing.
- A web session shows its risks first, acknowledged per connection.
- **Test Model** (`gateway/model_test.rs`): one short request through the gateway; a paid or unpriced model asks first.

## 7. Components

- Shared parts in `components/ui.tsx`; a failed load shows the error and a retry, never an empty state; essential actions are never hover-only; every string goes through `t()`.
