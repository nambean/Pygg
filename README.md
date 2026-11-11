<p><img src="branding/pygg-logo.svg" alt="Pygg — One Flow. Many Agents." width="420"></p>

# Pygg

**One Flow. Many Agents.** AI task and agent routing orchestration.

The repository and its technical identifiers keep the engineering name SmartTaskRouter. Documentation index: [docs/README.md](docs/README.md). Brand assets: [branding](branding/README.md).

**AI Cost Optimizer**: automatically selects a suitable connected AI model for each request from the clients you already use, and for each developer task, minimizing total cost while preserving context, privacy and answer quality. The app hosts no chat of its own (ADR-028).

> Status: the local multi-provider gateway works and the desktop app manages it (gateway first, [ADR-016](docs/DECISIONS.md)). On top of it, the vendors' own agent CLIs run with their own sign-in, a first task engine runs checked tasks across agent steps, and IDE agents start those tasks through MCP. Account pools and the `str`/npm launcher are designed, not built; Universal Shared Memory is not pursued (ADR-028). See [implementation status](docs/IMPLEMENTATION-STATUS.md) and the [roadmap](docs/ROADMAP.md).

## Product vision

**Market entry versus long-term vision:** initial validation prioritizes users with metered API/agent workloads where cost-per-success is measurable; everyday users are reached through the clients they already use, pointed at the gateway. Saving tokens is not equivalent to saving subscription money.

Pygg is a desktop-first AI Cost Optimizer for people who connect several providers and agents. Provider, quota and endpoint management are inspired by 9Router. The difference is that Pygg can also route whole tasks: it tries cheaper agents first where the result can be verified automatically, and escalates with an explanation.

- **Your IDE starts tasks, the app watches** ([ADR-027](docs/DECISIONS.md)): your coding agent hands a subtask to Pygg through MCP; the app runs it, and is where you see it, approve paid steps and review results. The gateway never switches models silently within an agent session.
- **Connected-model-first:** routes only to providers, models and agents you connected.
- **Official agents, their own sign-in** ([ADR-026](docs/DECISIONS.md)): Claude Code, Codex CLI, Gemini CLI and OpenCode run as themselves. Pygg never reads their sign-in and never bridges web sessions or other products' OAuth.
- **Free-first only with verification:** a cheaper step is tried first only against deterministic checks you approved (typecheck, tests, build). Each attempt runs in its own Git worktree.
- **Honest costs:** billing is `Free`, `Local`, `Subscription`, `Paid` or `Unknown`. Every figure shows its source, and unknown is never treated as free.
- **Privacy-aware:** record per connection whether the provider may train on submitted data, with where you read it; a client can require providers that do not.
- **Measurable:** cost is measured per successful task, against a reproducible benchmark baseline, before any savings claim.

## What works today

- **Gateway on `127.0.0.1:8741`:**
  - Serves the OpenAI Chat Completions API and the Anthropic Messages API.
  - Streams responses and passes tool calls in both directions.
  - Refuses requests addressed to anything but loopback. Only one gateway serves a store at a time.
- **Connections:** 26 provider presets and 6 local runtimes ([PROVIDERS.md](docs/PROVIDERS.md)), plus custom OpenAI- and Anthropic-compatible endpoints for any provider outside the list. Keys are kept in the OS credential store.
- **Same-format passthrough:** an Anthropic request (Claude Code) to an Anthropic connection goes through unchanged, so its prompt cache keeps working; other pairs are translated and report what they drop.
- **Combos:** a model name such as `coding` stands for an ordered list of models, tried only before any of the answer has reached the client. Rate limits and refusals cool a model down (honouring `Retry-After`) and never move to another key of the same organization; an ambiguous timeout is not retried elsewhere, and a server error is not retried on another model unless the client allows it.
- **Automatic routing:** `auto` picks the lowest eligible tier at or above the client's minimum. `auto/cheap` picks the lowest estimated cost. Both pick only among connected models known to support what the request needs; unknown counts as missing. Rules decide, not a classifier, and the response says why. A simulator in the Combos screen shows the route and every model left out.
- **Sessions:** a client that sends `x-smarttaskrouter-session` stays on the model that answered it, so its cache survives.
- **Client tokens:** one per app, stored only as a hash. Each has an optional monthly limit (checked against an estimate before each request, so the provider's bill can end up higher), a cap on requests in flight, a policy for models without a price, an optional list of the only connections it may use, and permission to start tasks.
- **Usage ledger:** every attempt is recorded with its cost and that cost's source.
  - **Actual:** the provider reported the charge.
  - **Estimated:** computed from a price you entered, or the connection is free or local.
  - **Unknown:** no source; never shown as $0. Agent runs are unknown, because their cost depends on how the agent is signed in.
- **Agents:** finds the agent CLIs on this computer and runs one on a repository in a worktree; the result is kept as a branch or discarded ([AGENTS.md](docs/AGENTS.md)).
- **Smart tasks:** a task is a ladder of agent steps judged by the workspace's approved checks. A failed check hands what failed to the next step; a paid step waits for your approval; a passing result waits for you to keep it as a branch or discard it. Tasks survive a restart as interrupted, with Retry.
- **From your IDE:** register `<app> mcp` as an MCP server in Claude Code, Codex or an IDE that takes JSON MCP configuration (Cursor and others), and your agent gets `pygg_run_task` and friends. Verified from real Codex and Claude Code sessions; other clients not yet.

## Universal Shared Memory (not pursued)

The app hosts no chat of its own ([ADR-028](docs/DECISIONS.md)), so the app-owned conversation store, Memory & Context screen and per-turn model affinity designed in [UNIVERSAL_SHARED_MEMORY.md](docs/UNIVERSAL_SHARED_MEMORY.md) are not planned. What tasks need of it is built: a failed attempt hands a bounded summary to the next step, and the gateway keeps a session on one model.

## CLI, IDE and provider tiers (partly built)

Built: the headless `gateway` and `agent` binaries, and the MCP server for IDE agents. Planned: a `str` CLI and npm/npx launcher for the same Rust core, and guided, version-tested setup with config backup and restore for more clients. See [CLI / IDE Integrations, npm & Provider Tiers](docs/CLI_IDE_NPM_PROVIDER_TIERS.md). No npm publication is claimed.

## Multi-account / API key rotation (planned)

API-key and token-plan connections could be grouped into **Account Pools** with Fill First, Round Robin, Sticky Round Robin and cost-aware policies, under session affinity, quota groups, allowlists and spend caps. Subscriptions are not pooled: they are used through the vendors' agent CLIs (ADR-026). See [Account/API Rotation](docs/ACCOUNT_API_ROTATION.md). Designed, **not implemented**.

## OmniRoute-inspired feature decisions

From a focused subset of OmniRoute, dynamic `auto` candidates, quota groups and error-scoped failover with cooldowns are built. Prompt and tool-output deduplication and cache-aware scoring are planned. Model fusion, a 20-strategy UI, credential or session impersonation and token-saving claims without a benchmark are rejected. See the [research and adoption matrix](docs/OMNIROUTE_FEATURE_RESEARCH.md).

## Navigation

| Group | Screen | Purpose |
| --- | --- | --- |
| Gateway | Overview | Gateway state and port, first-run checklist, client setup snippets, the last 24 hours |
| | Connections | Providers, keys, models, prices and what `auto` knows about each model |
| | Combos | Ordered model lists with fallback, and the `auto` simulator |
| | Clients | Client tokens, limits and permissions |
| | Usage | Costs (actual, estimated, unknown) by model, by client, and per request |
| Agents | Agents | Agent CLIs on this computer, IDE (MCP) setup, and a single run |
| Smart Tasks | Tasks, Policies | Tasks from your IDE or the app, approvals and results; Policies is not built yet |
| App | Workspaces, Settings | Workspaces: checks, privacy, budget. Settings: language, background (tray), task budgets |

See the [screen design](docs/design/SCREENS.md).

## Tech stack

The stack is the same as DownFetch's:
- Tauri 2 with a Rust core: gateway, task engine, SQLite store, OS keychain and process supervision.
- A React 19 + Vite + TypeScript interface, styled with semantic CSS tokens after Apple's HIG, with DownFetch's green accent.
- A pnpm workspace.
- Design reviews with the `apple-design` skill.

## Development

Requirements:
- Node.js LTS and pnpm 9 (`corepack enable`).
- Stable Rust, installed through rustup.
- On Windows, the Visual Studio C++ Build Tools and WebView2. WebView2 is built into Windows 11.

Run these from the repo root:

```powershell
pnpm install
pnpm dev                                              # interface in a browser at http://127.0.0.1:1431 with demo data (?empty for first run)
pnpm --filter @smarttaskrouter/desktop tauri:dev     # the desktop app with the real gateway
pnpm typecheck
pnpm build
cd apps/desktop/src-tauri; cargo test                 # core, gateway and task engine tests
```

### Headless

The `gateway` binary uses the same store as the app. Without `--db`, that is the app's own store. API keys are read from an environment variable, so they never appear on a command line.

```powershell
cd apps/desktop/src-tauri
cargo run --bin gateway -- add-connection --preset gemini --key-env GEMINI_API_KEY
cargo run --bin gateway -- refresh-models --id google-gemini
cargo run --bin gateway -- combo --name coding --member google-gemini/gemini-3.1-flash-lite --member google-gemini/gemini-2.5-flash
cargo run --bin gateway -- create-client --name "Claude Code"
cargo run --bin gateway -- serve                      # add --tasks yes to run smart tasks where the app is not open
```

Without `--label`, a preset's connection is named after the provider, so the Gemini connection's id is `google-gemini`. `cargo run --bin agent -- detect` lists the agent CLIs; `agent --db STORE task …` runs a smart task headless ([AGENTS.md](docs/AGENTS.md)).

The design review skill is pinned in `skills-lock.json` and installed under `.agents/skills/`, which is git-ignored. Decisions are recorded in [docs/DECISIONS.md](docs/DECISIONS.md).

## Licence

MIT, see [LICENSE](LICENSE). Nothing has been ported from other projects yet; code that is will keep its notice in `THIRD_PARTY_NOTICES.md`.

## Roadmap

The [roadmap](docs/ROADMAP.md) is the single authority for order; [docs/README.md](docs/README.md) lists every document and its status. In short: the everyday IDE task flow (A) and gateway correctness B1–B4 are built, the live verification debt (C1) and a live cascade (C2) are done, and the benchmark (C3) is running now, before any savings claim; then the remaining translation gaps (B5, B6) and distribution (D). Reviews and their answers are in [docs/reviews](docs/reviews).
