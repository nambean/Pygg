# SmartTaskRouter

**AI Cost Optimizer**: automatically selects a suitable connected AI model for each everyday chat turn or developer task, minimizing total cost while preserving context, privacy and answer quality.

> Status: the local multi-provider gateway works and the desktop app manages it (gateway first, [ADR-016](docs/DECISIONS.md)). Smart task routing, Universal Shared Memory, account pools and the CLI/npm launcher are designed, not built; task screens run on labeled sample data. See [implementation status](docs/IMPLEMENTATION-STATUS.md).

## Product vision

**Market entry versus long-term vision:** initial validation prioritizes users with metered API/agent workloads where cost-per-success is measurable; SmartTaskRouter's broader goal remains automatic model optimization for everyday users too. Saving tokens is not equivalent to saving subscription money.

SmartTaskRouter is a desktop-first AI Cost Optimizer for everyday AI users and developers who connect multiple providers. The goal is to let people ask normally without choosing a model on every message. Provider, quota and endpoint management are inspired by 9Router. The difference is that SmartTaskRouter routes whole tasks and stages, not single requests: it tries cheaper models first where the result can be verified automatically, and escalates with an explanation.

- **Orchestrator first:** routing happens at task/stage boundaries. The local gateway enforces policy and records usage, and never switches models silently within an agent session.
- **Connected-model-first:** routes only to providers, models and agents the user connected.
- **Free-first only with verification:** a cheaper tier is tried first only when the stage has a deterministic check (typecheck, tests, build). Each attempt runs in an isolated worktree.
- **Honest costs:** billing is `Free`, `Local`, `Subscription`, `Paid` or `Unknown`. Every quota figure shows its source, and unknown is never treated as free.
- **Privacy-aware:** records whether a provider may train on submitted data and enforces it for sensitive workspaces.
- **Measurable:** cost is measured per successful task, against a reproducible benchmark baseline.

## What works today

- **Gateway on `127.0.0.1:8741`:**
  - Serves the OpenAI Chat Completions API and the Anthropic Messages API.
  - Streams responses and passes tool calls in both directions.
  - Refuses requests addressed to anything but loopback.
- **Connections:** presets for Gemini, OpenRouter, OpenAI, Groq, DeepSeek, Ollama and any OpenAI-compatible endpoint. Keys are kept in the OS credential store.
- **Same-format passthrough:** an Anthropic request (Claude Code) to an Anthropic connection goes through unchanged, so its prompt cache keeps working; other pairs are translated and report what they drop.
- **Combos:** a model name such as `coding` stands for an ordered list of models, tried only before any of the answer has reached the client. Rate limits and refusals cool a model down (honouring `Retry-After`) and never move to another key of the same organization; an ambiguous timeout is not retried elsewhere.
- **Automatic routing:** `auto` picks the lowest eligible tier at or above the client's minimum. `auto/cheap` picks the lowest estimated cost. Both pick only among connected models known to support what the request needs; unknown counts as missing. Rules decide, not a classifier, and the response says why. A simulator in the Combos screen shows the route and every model left out.
- **Sessions:** a client that sends `x-smarttaskrouter-session` stays on the model that answered it, so its cache survives.
- **Client tokens:** one per app, stored only as a hash, each with an optional monthly limit (reserved atomically, so concurrent requests cannot pass it), a cap on requests in flight, and a policy for models without a price.
- **Usage ledger:** every attempt is recorded with its cost and that cost's source.
  - **Actual:** the provider reported the charge.
  - **Estimated:** computed from a price you entered, or the connection is free or local.
  - **Unknown:** no source; never shown as $0.

## Universal Shared Memory (planned)

Cross-provider conversations will share a **local, canonical conversation history** with versioned compact summaries, scoped long-term facts and tool/task checkpoints. A **memory-aware cost router** will decide whether to stay on the current model or switch, accounting for cache loss, context rebuilding, quality, privacy and actual usage. It will never switch automatically during an active agent attempt or an unresolved tool call.

Read the [detailed design, OSS references and benchmark plan](docs/UNIVERSAL_SHARED_MEMORY.md). This is a design proposal, not implemented functionality. Consumer Auto Chat is planned alongside the existing developer Tasks mode.

## CLI, IDE and provider tiers (planned)

SmartTaskRouter will also offer a **headless terminal CLI and npm/npx launcher** for the same Rust gateway, alongside guided integration with compatible Claude Code, Codex, OpenCode, Aider, Cline, Roo, Continue and selected IDEs. Provider/model access will be grouped as **Free, Subscription, API Paid, Local, Unknown**; account connection method is separate from billing eligibility. Supported integration combinations require contract tests and official permitted interfaces.

See [CLI / IDE Integrations, npm & Provider Tiers](docs/CLI_IDE_NPM_PROVIDER_TIERS.md). A first headless `gateway` binary exists (below); no npm publication or integration compatibility is claimed yet.

## Multi-account / API key rotation (planned)

Authorized accounts and API connections can be grouped into **Account Pools** with Fill First, Round Robin, Sticky Round Robin, Cost-Aware and bounded failover policies. Session affinity preserves provider prompt caches and Shared Memory continuity; quota resets, provider policy, per-client allowlists and spend caps are hard constraints. See [Account/API Rotation specification](docs/ACCOUNT_API_ROTATION.md). This feature is designed, **not implemented**.

## OmniRoute-inspired feature decisions (planned)

Adopt or adapt a focused subset of OmniRoute, in this order:
1. Dynamic Auto candidate discovery.
2. An accurate Free Tier catalog and shared quota groups.
3. Safe account-aware failover.
4. Minimal prompt and tool-output deduplication.
5. Cache-aware affinity and quality-adjusted scoring.

Reject default model fusion, a broad 20-strategy UI, credential/session impersonation and misleading token-saving claims. See the [research, adoption matrix and acceptance gates](docs/OMNIROUTE_FEATURE_RESEARCH.md). These are scope decisions, not implemented features.

## Navigation

| Group | Screen | Purpose |
| --- | --- | --- |
| Gateway | Overview | Gateway state and port, first-run checklist, client setup snippets, the last 24 hours |
| | Connections | Providers, keys, models and their prices |
| | Combos | Ordered model lists with fallback |
| | Clients | Client tokens and monthly limits |
| | Usage | Costs (actual, estimated, unknown) by model, by client, and per request |
| Smart Tasks · Preview | Tasks, Policies | The task orchestrator's design, on sample data |
| App | Settings | Language |

See the [screen design](docs/design/SCREENS.md).

## Tech stack

The stack is the same as DownFetch's:
- Tauri 2 with a Rust core: gateway, SQLite store, OS keychain and process supervision.
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
cd apps/desktop/src-tauri; cargo test                 # core and gateway tests
```

### Headless gateway

The `gateway` binary uses the same store as the app. Without `--db`, that is the app's own store. API keys are read from an environment variable, so they never appear on a command line.

```powershell
cd apps/desktop/src-tauri
cargo run --bin gateway -- add-connection --preset gemini --key-env GEMINI_API_KEY
cargo run --bin gateway -- refresh-models --id google-gemini
cargo run --bin gateway -- combo --name coding --member google-gemini/gemini-3.1-flash-lite --member google-gemini/gemini-2.5-flash
cargo run --bin gateway -- create-client --name "Claude Code"
cargo run --bin gateway -- serve
```

Without `--label`, a preset's connection is named after the provider, so the Gemini connection's id is `google-gemini`.

The design review skill is pinned in `skills-lock.json` and installed under `.agents/skills/`, which is git-ignored. Decisions are recorded in [docs/DECISIONS.md](docs/DECISIONS.md).

## Architecture review reconciliation (2026-11-11)

Claude's [architecture review](docs/reviews/2026-11-11-architecture-review.md) and the [independent response](docs/reviews/2026-11-11-review-response.md) identified cache, fallback, budget, session identity and roadmap risks. **The [single authoritative roadmap](docs/ROADMAP.md) now puts cost-safe gateway parity ahead of new optimizer features.** The gateway and desktop screens already exist; read [Implementation Status](docs/IMPLEMENTATION-STATUS.md) before treating a proposed capability as shipped. The `gateway serve` binary and port `8741` are implemented; proposed `str start` / npm distribution are not.

## Roadmap

0. **Gateway first (ADR-016), in progress:**
   - Done: provider adapters, compatible endpoints, client tokens and the usage ledger.
   - Next: quotas, then a benchmark of task routing as the orchestrator is built.
1. **MVP:** providers, gateway, rule-based router, Route screen, single-stage tasks with one agent.
2. **Multi-stage and multiple agents.**
3. **Measured routing and context optimization.**
4. **Distribution and Skills Hub.**
5. **Universal Memory & Auto Chat:** canonical history → compaction/context handoff → cache-aware model affinity → optional long-term facts and memory settings; see the linked design and the revised release priorities.

See the [full implementation plan](docs/SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md).
