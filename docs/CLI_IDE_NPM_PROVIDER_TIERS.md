# CLI, IDE Integrations, npm Distribution & Provider Tiers

> **Status:** Reference · CLI, IDE, npm and billing-tier specification; partly built · reviewed 2026-11-13 · order: [ROADMAP](ROADMAP.md) · built: [Implementation Status](IMPLEMENTATION-STATUS.md)

> **Update 2026-11-11 (ADR-026, ADR-027):** no "Supported OAuth" connections or OAuth callbacks into other products; subscriptions are reached through the vendors' agent CLIs. IDE integration for tasks is the MCP server (`<app> mcp`, [AGENTS.md](AGENTS.md#from-the-ide-pygg-mcp-and-the-task-api)). Consumer Auto Chat is dropped (ADR-028).

> **Update 2026-11-13 (what is built since this was written):**
> - **The console program is `pygg`**, not `str`: `pygg-cli` (`apps/desktop/src-tauri/src/cli/`), run as `pygg` by the npm launcher. `pygg --help` lists every command and `pygg --version` prints the version (commit 98a0afd, 2026-11-12). The command map is in §3.
> - **`pygg setup [<tool>] [--dry-run | --remove]`** (`cli/setup.rs`, commit 1c491f5, 2026-11-12) does the app's one-click set-up from a terminal: `codex-models`, `opencode-models`, `claude-launcher`, and the MCP server in `claude-code`, `codex` or `opencode`. Backup first, the key never printed. Unit test of the tool names; no live run of the command itself is recorded.
> - **`pygg run claude`** (`cli/run.rs`, `launcher.rs`, commit d12443f, 2026-11-12) starts Claude Code through the gateway for that launch only, with a key kept in the Windows Credential Manager; Claude Code's own settings are not changed. Checked live on 2026-11-12 up to the gateway's answer (422, no provider connected); see [AGENTS.md](AGENTS.md).
> - **npm:** `@pygg/cli` (the launcher, command `pygg`) and `@pygg/win32-x64` (the native program), `packages/npm`, ADR-030. Version `0.1.0-dev.0` is published under the tag `dev` (`npm install -g @pygg/cli@dev`; `npm view` on 2026-11-13 also shows it as `latest` for both packages). Developer preview: Windows x64 only, not code-signed. **That published version predates `setup`, `run` and `--version`:** npm's registry records it at 2026-11-12 04:27 UTC (11:27 in Viet Nam), and those commands were committed from 19:42 that day; the packed binary in `packages/npm/win32-x64/bin` answers `unknown command` to `--version` and `setup` (checked 2026-11-13). They reach npm users only with a new published version.
> - The IDE side is in [AGENTS.md](AGENTS.md): the MCP server in three agents, Codex's and OpenCode's models through the gateway, and the Claude Code launcher, each set up in one click.

> Specification v1.0 — 2026-11-11. **Partly built** (above). Do not advertise an integration before its contract tests pass, nor the npm package beyond its developer preview.
>
> Design inspiration: https://docs.9router.com/ and https://github.com/decolua/9router/tree/master/cli . Their installation and provider tier approaches are references, not code or credentials to replicate blindly.

## Current gateway versus future CLI package (review reconciliation)

**Implemented (per [Implementation Status](IMPLEMENTATION-STATUS.md)):** Rust headless binary `gateway serve` and local base URL `http://127.0.0.1:8741`, plus gateway-first desktop client/connection/usage views. Also built: rule-based `auto` (P0-5), the `agent` CLI and the MCP server (`<app> mcp`); since 2026-11-12, the `pygg` console program with `setup`, `run claude` and `--version`, and the npm developer preview `@pygg/cli@dev` (Update above). **Not implemented:** a `start` command that runs the gateway in the background (the app or `pygg gateway serve` serves it), npm/npx for platforms other than Windows x64, signed or integrity-checked releases, broad IDE compatibility and tier-aware rotation; Universal Memory is withdrawn (ADR-028). The `str` command examples below are **design targets only**; the built names are in §3, and any `4318` value below is historical, not the live default.

All client integrations need version-specific base URL, tool, streaming, cache and session-ID tests. Transparent same-format passthrough is preferred; a subscription/CLI login must not be treated as fungible API access.

## 1. Product intent and relationship to AI Cost Optimizer

Pygg should work in two complementary ways:

- **Consumer Auto Chat:** user types normally; application chooses a capable connected model without unnecessary selection prompts, preserving conversation memory.
- **Developer CLI/IDE gateway:** users keep using their preferred terminal agent or IDE; Pygg exposes a local authenticated API, policy-routed aliases, quota management, model usage accounting and optional opt-in task intelligence where the client supplies sufficient context.
- **Agent orchestration:** the application launches a supported CLI in an isolated workspace and makes choices at stage boundaries, without changing the model mid-stream or mid-agent attempt.

Gateway-only routing cannot infer complete intent from arbitrary request payloads, and must **not claim** task-level cost optimization when required context is missing. An explicitly enabled `auto` alias can classify each independent new chat/task or evaluate eligible model candidates within a pinned conversation; provider/model switching keeps to explicit sessions (P0-4); Universal Shared Memory is withdrawn (ADR-028).

## 2. Provider tiers (9Router-inspired, not mutually exclusive per provider)

Five **billing/entitlement tiers**:

| Tier | Definition | Examples of account type | Metering / restrictions |
| --- | --- | --- | --- |
| `FREE` | Confirmed no-bill quota or public free endpoint | Verified free API account/model | Current limits, reset, data terms and source required; free quota can expire |
| `SUBSCRIPTION` | Entitlement under a user's subscription or seat | Officially supported subscription-backed CLI/account | Window usage if available; never assume access to generic API credits or proxy session tokens |
| `API_PAID` | Consumption charged per token/request | Standard API key or metered provider | Actual/estimated charges, explicit budget and alert gates |
| `LOCAL` | Model hosted on user's device or permitted private server | Ollama / compatible local inference | API spend may be 0, but track compute/energy and latency separately |
| `UNKNOWN` | Insufficient verified billing/entitlement metadata | Custom or unverified endpoint | Never display as free or assign zero cost; require policy review |

**Important distinction:** connection method (API key, OAuth, official CLI auth, local endpoint) is **not** the same as billing tier. One provider/account may have free, paid and promotional models simultaneously. Store tier at **account-model entitlement** level with validity dates, provenance and limits; provider-level cards can aggregate counts.

Optional secondary **routing price bands**: `T0–T4` capability tier is orthogonal to billing, `free/cheap/premium` is merely a user-facing strategy preference, and `Subscription` is not a guarantee of any ability to redirect traffic. Subscription access is supported only via official permitted interfaces.

### Default routing policies

1. Hard filters: connected; model capable; terms/privacy; supported wire format; health; sufficient context/quota; allowed scope; budgets.
2. Cost ranking based on effective success cost and cache/rehydration overhead (where measured). For MVP, labeled rules instead of fabricated probability scores.
3. `Save More`: prefer eligible Local/Free, then usable Subscription quota, then paid API, but use a stronger model directly when warranted and stay on a cached model when switching is net-negative.
4. `Balanced`: reliability, context continuity and latency as well as cost.
5. `Quality`: choose best eligible capability tier within budget; prefer continuity when otherwise equal.
6. Never auto-spend above approved per-turn/per-task/client ceilings; unsupported subscription methods are excluded rather than bypassed.

### Quota fields

`entitlement_kind`, `billing_tier`, `source`, `observed_at`, `expires_at`, `reset_at`, `remaining`, `limit`, `unit`, `terms`, `supports_routing`, `supports_embedding`, `supports_tools`, `supports_images`, `context_length`, `account_id`, `provider_model_id`. Unknown source/limit/price is explicit.

## 3. Terminal CLI / IDE Integration Hub

A dedicated **Integrations** page under Connections or as a sidebar item in developer mode (do not overwhelm consumer Auto Chat).

### Screen wireframe

```text
Connections > Integrations
[All] [Terminal CLIs] [IDEs / Extensions]        Search integrations…
Gateway: Running · http://127.0.0.1:<assigned-port>/v1   [Copy]
Policy alias: smarttaskrouter/auto [Select ▾]    Client token: [Generate] [Revoke]

Claude Code          [Detected / Not configured]    [Setup] [Test]
OpenAI Codex         [Detected / Not configured]    [Setup] [Test]
OpenCode             [Detected / Not configured]    [Setup] [Test]
Cursor               [Detected / Not configured]    [Setup] [Test]
Cline / Roo          [Detected / Not configured]    [Setup] [Test]
Continue             [Detected / Not configured]    [Setup] [Test]
Other compatible     [Manual configuration]         [Copy template]
```

**Per integration detail:** compatibility/version; supported request format, tools, streaming, reasoning, images; detected executable/config path; scope (per workspace/global); gateway address; client token scopes; default model alias or combo; status; setup commands; backup/restore; known limitations and tests.

**Integration matrix to verify, not promise:**

| Target | Entry point | Initial integration method | Verification |
| --- | --- | --- | --- |
| Claude Code | Terminal CLI | Official supported base URL/env/config interface where permitted; Anthropic Messages contract | Stream, tool use, auth mode, cache usage and cancel |
| OpenAI Codex | Terminal CLI | Officially supported custom model/provider config when available | Chat/task streaming, tools, cancellation and response semantics |
| OpenCode | Terminal CLI + desktop | Configured OpenAI-compatible provider/base URL | Agent commands, tool streaming and model alias behavior |
| Cursor | IDE | Documented custom endpoint/model integration when available | Availability of custom base URL, editor tools and errors |
| Cline / Roo Code | IDE extension | Custom OpenAI-compatible provider config | Tool calls, patch/apply flow and completion |
| Continue | IDE extension | Configured provider and model aliases | Completion, chat, embeddings separated by capability |
| Aider | CLI | OpenAI/Anthropic-compatible environment/config | Chat, edit format, usage and rollback |
| Other clients | SDK / CLI | Generic OpenAI-compatible and verified Anthropic endpoints | Smoke/contract suite |

Each adapter gets `unsupported`, `experimental`, `verified` or `degraded` state. Do not auto-edit the user's external IDE settings on discovery; configure only on explicit action, back up files and offer **Restore**.

**Provider subscription security:** Never scrape cookies or reuse proprietary tokens, circumvent subscriptions, or promise that paid IDE subscriptions can back arbitrary API clients. A CLI may run using its own official authenticated process as an *agent*, with metering limitations shown.

### CLI commands (proposed interface)

```text
str --help
str start [--port 4318] [--no-browser]          # starts local gateway
str stop
str status
str dashboard
str doctor                                      # local runtime/clients check
str providers list
str models list [--tier free|subscription|api|local|unknown]
str integrations list
str integrations setup <client> [--scope workspace|global]
str integrations test <client>
str integrations restore <client>
str combos list
str usage show
str config get <key>
str config set <key> <value>
```

Avoid implementing invisible data-collection, unattended arbitrary scripts or default system configuration mutations.

**Built, 2026-11-13** (`apps/desktop/src-tauri/src/cli/`; `pygg-cli` in development, `pygg` through npm):

| Proposed | Built as | State |
| --- | --- | --- |
| `str --help` | `pygg --help`, `pygg --version` | built (98a0afd); help opens no store |
| `str start` / `str stop` | the app, or `pygg gateway serve [--port N]` in the foreground (Ctrl+C stops it) | no background service |
| `str status` | `pygg status` | built |
| `str doctor` | `pygg doctor` (status, the store's schema, the agents found and their Pygg set-up) | built |
| `str providers list`, `str models list` | `pygg gateway list` (connections with their number of models, combos, API keys) | connections built; no per-model list, no tier filter |
| `str integrations list` | `pygg setup` | built (1c491f5) |
| `str integrations setup <client>` | `pygg setup <tool>`, with `--dry-run` | built for `codex-models`, `opencode-models`, `claude-launcher`, `claude-code`, `codex`, `opencode`; user scope only, never a project file |
| `str integrations restore <client>` | `pygg setup <tool> --remove` (takes Pygg's entry out and revokes its key; the backup stays in `integration-backups`) | built; no restore of a backup file by command |
| `str integrations test <client>` | none; `pygg gateway test-model --model C/M` sends one request to a model | not built |
| (none) | `pygg run claude [args…]`: Claude Code through the gateway for one launch | built (d12443f) |
| `str combos list`, `str usage show` | `pygg gateway list`, `pygg gateway usage` | built |
| `str config get/set` | none | not built |

## 4. npm / npx packaging

**Goal:** global convenience launcher for the *same* Rust gateway as the desktop app, not a separate Node gateway. Do not modify the currently defined Tauri/Rust backend solely to enable npm.

### UX

```sh
npm install -g smarttaskrouter
str start
# or for a one-time invocation:
npx smarttaskrouter start
```

**Commands above are proposed; package name `smarttaskrouter` and binary `str` are not confirmed available/published.** Verify npm ownership and naming before implementation. A scoped package `@<verified-publisher>/smarttaskrouter` is a fallback.

**Built (2026-11-12, `packages/npm`, ADR-030):** the scoped fallback, as `@pygg/cli` (npm refused the unscoped name `pygg` as too close to existing ones). It installs with `npm install -g @pygg/cli@dev` and gives the command `pygg`. `pygg/bin/pygg.js` is a Node launcher only: it runs the native `pygg-cli.exe` from the optional dependency `@pygg/win32-x64` with the same arguments and standard streams (so `pygg mcp` works as an MCP server), falling back to an installed app's `%LOCALAPPDATA%\Pygg\pygg-cli.exe` or `PYGG_CLI`. `scripts/npm-pack.mjs` packs both from a release build and refuses differing versions; publishing is the owner's step (`packages/npm/PUBLISHING.md`). Published: `0.1.0-dev.0` of both, 2026-11-12; installed back from the registry into an empty folder that day, `pygg doctor` found the running app and `pygg mcp` answered `initialize` and `tools/list` (PUBLISHING.md). That version predates `pygg setup`, `pygg run` and `--version` (Update at the top). Windows x64 only, unsigned, no checksums or provenance yet.

### Architecture

- Rust core contains shared server, configuration, SQLite migrations, keychain, provider adapters and provider-compatible streams.
- A headless Rust binary `str` is built alongside Tauri GUI using shared crates/modules; CLI is first-class but does not require desktop window/WebView.
- Node package contains a minimal cross-platform launcher and signed or integrity-checked platform binaries (or optional platform packages); Node is only a bootstrap/launcher, **not** a second server implementation.
- Host-platform selection: win32 x64/arm64, darwin x64/arm64, linux x64/arm64 subject to actual CI coverage. Clear errors for unsupported targets.
- `npx` one-shot starts the server until interruption or requested exit; no silently persistent service.
- Share one data directory safely between CLI/desktop; file lock, single gateway instance, explicit port collision behavior, Windows/macOS/Linux credential-store behavior and migration concurrency tests.
- Secure releases: pinned checksums/signatures, CI supply-chain provenance, minimal npm lifecycle scripts, user-scoped installation by default and no secrets in npm logs.
- `str doctor` verifies gateway reachability, binary version, keychain, provider connectivity and integrated clients without reading private session tokens.
- CLI self-update is not mandatory; npm version and desktop updater follow explicit supported version compatibility rules.

### Distribution decision gates

- npm package name available and verified; no current publication is claimed.
- Exact binaries and assets produced on supported OS/architectures.
- Standalone service start/stop, OAuth callback (if supported), keychain, update and port binding validated without Tauri UI.
- macOS code signing/notarization and Windows signing planned for release.
- Privacy / telemetry opt-in same as desktop.

## 5. Data, APIs and contracts

Proposed extensions:

```text
provider_accounts(id, provider_id, label, auth_method, enabled, credential_ref, status)
model_entitlements(id, provider_account_id, model_id, billing_tier, price_json,
                   quota_kind, limit_value, remaining_value, reset_at, expires_at,
                   source, checked_at, data_terms, capabilities)
client_integrations(id, client_type, installed_version, config_path, status,
                    configured_endpoint, alias_id, last_tested_at)
client_config_backups(id, integration_id, backup_ref, captured_at, restore_status)
client_tokens(id, token_hash, scope, budget_rule_id, created_at, revoked_at)
gateway_instances(id, instance_key, port, pid, started_at, heartbeat_at)
```

`provider_model_id` and account-specific entitlements must remain distinct, including same model across multiple providers/accounts.

API additions (Tauri commands for UI; bearer-authenticated local HTTP for external clients if enabled):

```text
GET /api/integrations
GET /api/integrations/:id/capabilities
POST /api/integrations/:id/scan
POST /api/integrations/:id/setup
POST /api/integrations/:id/test
POST /api/integrations/:id/restore
GET /api/providers/tiers
GET /api/providers/:id/entitlements
GET /api/cli/status
GET /api/cli/doctor
```

**Security:** require scoped authorization on all local HTTP operations, including diagnostic and setup mutation endpoints; do not expose credentials, user config contents or filesystem paths to unauthenticated callers. Setup/restore needs explicit user action. Prefer OS IPC / Tauri command for local filesystem mutation instead of broad HTTP surface.

## 6. UI changes

### Connections / Providers

- Tabs: `All`, `Free`, `Subscription`, `API`, `Local`, `Unknown`.
- Within each tier display accounts with model counts, per-account quota source, reset time, provider terms and health.
- The same provider may appear in more than one tier; UI should distinguish **connection** from **available entitlements** and explain duplicate providers.
- Connect wizard has `API Key`, `Supported OAuth`, `Local Endpoint`, and `Official CLI Integration` methods where applicable; available connection methods are not identical to tier badges.
- Color and status must not be used as sole indicators, and quotas without provider support show Unknown rather than a made-up percentage.

### Model Combos / Policy routing

- Add tier-aware candidates and per-tier fallback, with a source-and-confidence preview for entitlement/price.
- Show ordered fallbacks, manual approval thresholds, effective token costs and context-switch overhead.
- Simulator includes missing quota and privacy mismatches.

### Integrations

- One-click **guided** setup to produce config snippets, detect installed version, apply an approved config change with backup, test connection and rollback.
- Individual tool cards and verified compatibility badges.
- A **copy command** for manual setup; do not assume every IDE supports changing base URL.
- Promote `npm install` or `npx` only in developer-facing onboarding; consumer chat users can install the desktop app directly.

### Settings > CLI & Gateway

- Local gateway address/port, start with OS, log retention, client tokens, default policy aliases, CLI binary location, npm version state and conflict diagnostics.

## 7. Roadmap aligned with existing ADR-016 (gateway-first)

| Phase | Deliverable | Priority |
| --- | --- | --- |
| G1 | Provider account registry + tier/entitlement schema and categorized Providers UI | Later (ROADMAP E) |
| G2 | Verified OpenAI/Anthropic-compatible gateway, streaming, scoped client keys, quotas and ledger | Built (P0-1 to P0-5) |
| G3 | Terminal CLI client setup for OpenCode, Aider and verified Claude Code/Codex routes | Partly built (2026-11-13): set-up snippets for Claude Code and OpenCode, verified live by 2026-11-11; one-click set-up of Codex's and OpenCode's models through the gateway, each verified live on 2026-11-12; the Claude Code launcher (`pygg run claude`), checked live only up to the gateway's answer; all of them also as `pygg setup <tool>`. Aider not |
| G4 | Rust headless `str` binary, separate npm/npx launcher, release CI and integrity checks | Partly built: the binary is `pygg-cli` (`pygg`), and the npm launcher `@pygg/cli@dev` is published for Windows x64, unsigned (its published version predates `setup` and `run`); no release CI, signing or integrity checks |
| G5 | IDE integration guides/setup for Cline/Roo, Continue, Cursor where supported | Partly built: set-up by hand on CLI Tools (Cline, Continue, SDKs, curl) and MCP snippets for Cursor and other JSON clients; not verified per client |
| G6 | Integration diagnostics, config backup/restore and compatibility test matrix | Partly built: each one-click set-up backs up the tool's configuration first and Remove puts Pygg's change back; `pygg doctor` lists each agent's Pygg set-up; no compatibility matrix |
| G7 | More clients, platform coverage and subscription adapters only through supported interfaces | P2 |

Order follows [ROADMAP](ROADMAP.md); Universal Shared Memory and the consumer Auto Chat UI are withdrawn (ADR-028).

## Account Pool Rotation dependency

Multiple authorized accounts and keys within provider tiers can be selected by Fill First, Round Robin, Sticky Round Robin, Cost Aware or bounded Failover, with client allowlists, cooldown, quota groups and persistent conversation affinity. **See [ACCOUNT_API_ROTATION.md](ACCOUNT_API_ROTATION.md)** for routing, concurrency, UI, schema and safety; do not conflate rotating a client token with switching upstream credentials. Rotation never bypasses limits or suspensions. Built so far (2026-11-13): pools of separate accounts tried in the user's order (ADR-034), and pools of web sessions that send to the account used least today (ADR-035); the other strategies are not built.

## 8. Test and acceptance criteria

1. Connected provider with free **and** paid models appears correctly in both tiers, without duplicating account secrets.
2. Unknown quota and price are never shown as free or zero.
3. Subscription access never silently becomes paid API use; unsupported subscription-backed routing is clearly unavailable.
4. CLI and Tauri desktop read the same SQLite store without concurrent migration corruption or accidental double gateway startup.
5. All verified integrations complete text streaming, error handling and cancellation; agent/tool functionality is tested separately.
6. No third-party config file changes without approval, backup and restoration path.
7. Non-loopback binding disabled by default; scoped client keys required even on localhost.
8. npm install/start/stop/doctor tested on release target OS and architecture; no package publish until name and ownership are verified.
9. Usage ledger attributes every request to client integration, account, model, tier and combo, including failed billed calls.
10. Model Affinity does not break continuity, active tools or conversation cache to save a nominally cheaper token price.
11. Quota and data-use terms retain per-fact provenance, observed times and unknown states.
12. Every UI-supported platform/target has a tested configuration path or an explicit unsupported state.
