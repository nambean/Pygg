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
| **Smart Tasks:** Projects | A project is a repository: plan → tasks → output, or a solo task | Built · experimental | §5.0, §5.1 |
| Agents & defaults | Agent CLIs, MCP set-up, and the Brain Storming defaults | Built · experimental | below |
| **App:** Guide | How to use Pygg | Built | below |
| Settings | Language, budgets, background, diagnostics, history | Built | §5.8 |

- The app starts on Home; Projects shows a count of what needs attention (§5.0). All tasks, Project settings and the board stay reachable from Projects and the command palette.
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

### 5.0 Projects

`ProjectsView.tsx`, `ProjectView.tsx`. The owner's model: **create a project → plan it with Brain Storming → the tasks run → take the output**, or **one solo task**.

- **Projects:** **Needs your attention** (brainstorms to review, steps to approve), the projects as cards (path, plans, cards on the board, what runs or waits), **Add project…**, recent **Solo tasks**, **New solo task**, **All tasks**.
- **A project:** the step bar **Plan → Tasks → Output** for the chosen plan (by default the one waiting on you, else the newest approved; progress counts its board cards: done, running, to review, blocked); **1 · Plan** (its brainstorms and **New plan**: **What should be built?**, **Start planning**; with none, the form is open, with how to start one from the coding agent with `pygg/brain` folded below); **2 · Tasks** (the project's board, folded once every card on it is done, and the project's solo tasks); **Waiting for you** under the steps (a paid step to approve, a result to review, a blocked card, each with its button); **Ask about this project** (an answer task, ADR-043) beside **New solo task**; **3 · Output** (the plan's branch with copyable git commands, **Review the changes** as one diff from where the plan started, results that could not join the plan's branch on branches of their own, kept solo results). **Project settings** opens §5.3.

### 5.1 Tasks

`TaskList.tsx`, as **All tasks**. Experimental.

- Filters with counts: All, Running, Needs Approval, Completed, Failed, Stopped, **Board** (§5.1.1).
- **Needs Attention** pinned on top with **Review**; search (`Ctrl+F`); brainstorm rows (§5.2.2).

#### 5.1.1 Board

`Board.tsx`, ADR-042, [BRAIN.md](BRAIN.md) §3. Built; ran live once.

- Columns Discovery, Ready, Running, Checking, Blocked, Review, Done; Running and Checking have WIP limits (**Limit…**).
- Card actions: **Review** / **Open**, **Expedite**, **Move to Ready**, **Back to Discovery**, **Drop**; the engine moves the rest; no drag-and-drop.
- The workspace's approved checks define Done.

### 5.2 Task detail

`TaskDetail.tsx`.

- **Waiting for approval** (inline): **Approve up to $…** with a cap, or **Decline**; a sensitive workspace asks **Allow This Step**.
- **Answer** (an answer task): the agent's answer with **Copy**; nothing to keep.
- **Result:** **Review Changes**, **Keep as Branch**, **Discard…**, **Export Patch…**; once kept, an **Output** box with the branch (copy), the git commands to review and merge, and, for a plan's card, whether its work is on the plan's branch or kept apart.
- **Live activity** under a running attempt: what the agent does, line by line (files read and written, commands, what the model said), from OpenCode and Codex as it happens; Claude Code and Gemini CLI report when they finish. `live.rs`, in memory, the last 500 lines of an attempt.
- Attempts with cost and checks; **Stop Task**, **Retry**; the task's text folded when long.

#### 5.2.1 New Task

`NewTaskView.tsx`.

- Description, workspace, a ladder of agent steps (Free First, Balanced, Quality), shell **Allow** switch; **Start Task**.

#### 5.2.2 Brainstorm

`BrainstormDetail.tsx`, ADR-041, [BRAIN.md](BRAIN.md) §2.

- States: **Waits for your pick**, **Thinking**, **Proposed**, **Approved · board**, **Declined**, **Planner failed**.
- A paid planner waits for **Use <model>** or **Decline**.
- A plan started here (**started here**): an answer box under each open question, **Anything else to change in the plan**, **Send to the planner** (a new revision); one from a coding agent is answered there.
- Cards: model, **Change…** (role sheet, §6), up to 5 fallbacks, **Drop** / **Restore**.
- Approve bar: known estimate, budget left; **Approve <n> cards…** puts them on the board.

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
- Summary: savings, known cost with unknown count apart, chart, token mix, by model and client.
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
