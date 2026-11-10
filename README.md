# SmartTaskRouter

**AI Cost Optimizer**: automatically selects a suitable connected AI model for each everyday chat turn or developer task, minimizing total cost while preserving context, privacy and answer quality.

> Status: early implementation. The desktop shell and task list run on labeled sample data; the task engine, gateway and router are not built yet. See [implementation status](docs/IMPLEMENTATION-STATUS.md).

## Product vision

SmartTaskRouter is a desktop-first AI Cost Optimizer for everyday AI users and developers who connect multiple providers. The goal is to let people ask normally without choosing a model on every message. Provider, quota and endpoint management are inspired by 9Router. The difference is that SmartTaskRouter routes whole tasks and stages, not single requests: it tries cheaper models first where the result can be verified automatically, and escalates with an explanation.

- **Orchestrator first:** routing happens at task/stage boundaries. The local gateway enforces policy and records usage, and never switches models silently within an agent session.
- **Connected-model-first:** routes only to providers, models and agents the user connected.
- **Free-first only with verification:** a cheaper tier is tried first only when the stage has a deterministic check (typecheck, tests, build). Each attempt runs in an isolated worktree.
- **Honest costs:** billing is `Free`, `Local`, `Subscription`, `Paid` or `Unknown`. Every quota figure shows its source, and unknown is never treated as free.
- **Privacy-aware:** records whether a provider may train on submitted data and enforces it for sensitive workspaces.
- **Measurable:** cost is measured per successful task, against a reproducible benchmark baseline.

## Universal Shared Memory (planned)

Cross-provider conversations will share a **local, canonical conversation history** with versioned compact summaries, scoped long-term facts and tool/task checkpoints. A **memory-aware cost router** will decide whether to stay on the current model or switch, accounting for cache loss, context rebuilding, quality, privacy and actual usage. No automatic switch during an active agent attempt or unresolved tool call.

Read the [detailed design, OSS references and benchmark plan](docs/UNIVERSAL_SHARED_MEMORY.md). This is a design proposal, not implemented functionality. Consumer Auto Chat is planned alongside the existing developer Tasks mode; the present task-list UI remains the implemented shell.

## CLI, IDE and provider tiers (planned)

SmartTaskRouter will also offer a **headless terminal CLI and npm/npx launcher** for the same Rust gateway, alongside guided integration with compatible Claude Code, Codex, OpenCode, Aider, Cline, Roo, Continue and selected IDEs. Provider/model access will be grouped as **Free, Subscription, API Paid, Local, Unknown**; account connection method is separate from billing eligibility. Supported integration combinations require contract tests and official permitted interfaces.

See [CLI / IDE Integrations, npm & Provider Tiers](docs/CLI_IDE_NPM_PROVIDER_TIERS.md). No npm publication or integration compatibility is claimed yet.

## Multi-account / API key rotation (planned)

Authorized accounts and API connections can be grouped into **Account Pools** with Fill First, Round Robin, Sticky Round Robin, Cost-Aware and bounded failover policies. Session affinity preserves provider prompt caches and Shared Memory continuity; quota resets, provider policy, per-client allowlists and spend caps are hard constraints. See [Account/API Rotation specification](docs/ACCOUNT_API_ROTATION.md). This feature is designed, **not implemented**.

## OmniRoute-inspired feature decisions (planned)

Adopt/adapt a focused subset of OmniRoute: **dynamic Auto candidate discovery, accurate Free Tier catalog and shared quota groups, safe account-aware failover, minimal prompt/tool-output deduplication, then cache-aware affinity and quality-adjusted scoring**. Reject default model fusion, broad 20-strategy UI, credential/session impersonation and misleading token-saving claims. [Research, adoption matrix and acceptance gates](docs/OMNIROUTE_FEATURE_RESEARCH.md). These are scope decisions, not implemented features.

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

0. **Gateway first (ADR-016):** implement provider adapters, compatible endpoints, quotas and the usage ledger; then benchmark task routing as the orchestrator is built.
1. **MVP:** providers, gateway, rule-based router, Route screen, single-stage tasks with one agent.
2. **Multi-stage and multiple agents.**
3. **Measured routing and context optimization.**
4. **Distribution and Skills Hub.**
5. **Universal Memory & Auto Chat:** canonical history → compaction/context handoff → cache-aware model affinity → optional long-term facts and memory settings; see linked design and revised release priorities.

See the [full implementation plan](docs/SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md).
