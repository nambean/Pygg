# SmartTaskRouter

**AI Model & Agent Orchestrator**: routes each stage of a coding task to the cheapest connected model and agent that can pass that stage's checks.

> Status: early implementation. The desktop shell and task list run on labeled sample data; the task engine, gateway and router are not built yet. See [implementation status](docs/IMPLEMENTATION-STATUS.md).

## Product vision

SmartTaskRouter is a desktop app for developers who already use several AI providers and coding agents. Provider, quota and endpoint management are inspired by 9Router. The difference is that SmartTaskRouter routes whole tasks and stages, not single requests: it tries cheaper models first where the result can be verified automatically, and escalates with an explanation.

- **Orchestrator first:** routing happens at task/stage boundaries. The local gateway enforces policy and records usage, and never switches models silently within an agent session.
- **Connected-model-first:** routes only to providers, models and agents the user connected.
- **Free-first only with verification:** a cheaper tier is tried first only when the stage has a deterministic check (typecheck, tests, build). Each attempt runs in an isolated worktree.
- **Honest costs:** billing is `Free`, `Local`, `Subscription`, `Paid` or `Unknown`. Every quota figure shows its source, and unknown is never treated as free.
- **Privacy-aware:** records whether a provider may train on submitted data and enforces it for sensitive workspaces.
- **Measurable:** cost is measured per successful task, against a reproducible benchmark baseline.

## Navigation

The app is list-first like DownFetch: the task list is home, filtered from the sidebar (All Tasks, Running, Needs Approval, Completed, Failed, Stopped) and scoped by workspace.

| Destination | Purpose |
| --- | --- |
| Tasks | Task list; task detail (New Task → Plan → Run → Result); Workspace Settings |
| Connections | Providers, model catalog, agents |
| Policies | Strategies, tier table, rules, budgets, simulator |
| Usage | Ledger analytics, budgets, cost per success |
| Endpoints | Gateway addresses, client tokens, client setup |
| Settings | Language, startup, storage, retention, diagnostics |

Skills & Prompts is planned for Phase 4. See the [screen design](docs/design/SCREENS.md).

## Tech stack

Same stack as DownFetch: Tauri 2 with a Rust core (gateway, router, agent runner, SQLite, OS keychain), a React 19 + Vite + TypeScript interface styled with semantic CSS tokens after Apple's HIG, a pnpm workspace, and design reviews with the `apple-design` skill.

## Development

Requirements: Node.js LTS, pnpm 9 (`corepack enable`), and stable Rust through rustup. On Windows you also need the Visual Studio C++ Build Tools and WebView2, which is built into Windows 11.

Run these from the repo root:

```powershell
pnpm install
pnpm dev                                              # interface in a browser at http://127.0.0.1:1431, sample data
pnpm --filter @smarttaskrouter/desktop tauri:dev     # the desktop app with hot reload
pnpm typecheck
pnpm build
```

The design review skill is pinned in `skills-lock.json` and installed under `.agents/skills/`, which is git-ignored. Decisions are recorded in [docs/DECISIONS.md](docs/DECISIONS.md).

## Roadmap

0. **Benchmark:** test whether a check-driven cascade beats always using one model on cost per success.
1. **MVP:** providers, gateway, rule-based router, Route screen, single-stage tasks with one agent.
2. **Multi-stage and multiple agents.**
3. **Measured routing and context optimization.**
4. **Distribution and Skills Hub.**

See the [full implementation plan](docs/SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md).
