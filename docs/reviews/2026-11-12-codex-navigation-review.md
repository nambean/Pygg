# Navigation and setup like 9router and OmniRoute: review by Codex, 2026-11-12

> **Status:** Record · Codex's review as it wrote it (read-only, no files changed), and what was done · order: [ROADMAP](../ROADMAP.md) · built: [Implementation Status](../IMPLEMENTATION-STATUS.md)

Codex (read-only, with 14 screenshots) was asked to reorganise the interface so setup feels like 9router's and OmniRoute's. **What was built from it:**
- e0b236f: the navigation (Home, Providers, Endpoint, CLI Tools, Combos, Usage).
- d12443f: the Claude Code launcher and each tool's state from its own key.
- a87000e: estimated savings.

## The review, as written

Make the main path **Connect provider → Set up tool → Work in your IDE**. Home then answers: “Is Pygg working, and what is it costing me?”

These are product recommendations based on the screenshots and code. No files changed.

**1. Information architecture**

Sidebar, in this order:

| Screen | Contents and existing sections to move |
|---|---|
| **Home** | Compact gateway status, setup progress before first use; connected tools, recent requests, costs and savings afterward. Reuse `Overview.tsx`, removing its configuration snippets. |
| **Providers** | Rename Connections. Provider accounts, availability, model counts, billing type, Connect Provider. Keep `Connections.tsx`; simplify its detail panel. |
| **Endpoint** | Copyable OpenAI base URL `http://127.0.0.1:8741/v1`, Anthropic base URL `http://127.0.0.1:8741`, default model `auto`, and **API Keys**. Move `Clients.tsx` here as a tab. |
| **CLI Tools** | Gateway configuration for Claude Code, Codex, OpenCode, compatible editors and SDKs. Move gateway integrations from `Agents.tsx` and snippets from `Overview.tsx`. |
| **Combos** | Ordered fallback lists. Put the existing routing simulator under **Automatic routing → Advanced**. |
| **Usage** | Costs, routing outcomes, savings comparison and request ledger. Retain existing exports and cost provenance. |
| **Settings** | App preferences, gateway startup/port, diagnostics, retention, and **Smart Tasks — Experimental, on hold**. |

Inside Smart Tasks retain Tasks, Workspaces, Policies, detected agents, MCP setup and Run a Task. Preserve existing work and show an unobtrusive attention badge when it needs review; stop redirecting launch automatically into tasks.

Use familiar gateway names, but keep Pygg’s distinctions:

- **API Keys**, with “Pygg client tokens” as explanatory text, distinguishes them from provider credentials.
- Free availability belongs in Providers and Usage. A separate “Free tiers” screen would imply reliable remaining quotas that Pygg may not know.
- Keep `auto` and `auto/cheap`; don’t introduce competitors’ aliases without matching backend behavior.

**2. First-run flow**

No mandatory wizard; show a two-step checklist on Home.

1. **Connect a provider.** User selects a provider and supplies its key, or chooses a local runtime. Pygg stores provider credentials in Windows Credential Manager, discovers models and checks availability. Show **“Connected · N models available”**, with a recoverable error if checking fails.
2. **Set up your tool.** User selects a tool; `auto` is preselected. Pygg checks whether eligible models support that tool’s request format, creates a dedicated client token, and applies configuration or provides manual instructions. Show **“Configured — waiting for your first request.”**
3. **Use the tool normally.** User sends a request from their IDE or CLI. A successful response recorded against that token changes the card to **“Working · answered by [model] · [time]”**.

Pygg starts the localhost gateway automatically. A port conflict gets an inline fix. Endpoint copying, token creation and combo creation are not separate onboarding requirements.

Before setup, Home shows gateway status and the checklist. After setup, it shows connected tools, recent activity, known cost, unknown-cost count and the savings comparison. Keep **Connect another provider** and **Set up another tool** as secondary actions.

**Critical prerequisite:** current `auto` defaults to a T2 minimum and excludes models with missing required metadata. First-run must resolve eligibility automatically where metadata is trusted, or explain the blocker. Never silently lower quality requirements or claim setup is ready when `auto` has no eligible model.

**3. CLI Tools**

Each card has: **model picker → setup action → configuration status → latest request**.

Shared model choices: `auto` by default; `auto/cheap`, combos and eligible individual models under “Choose another model.” Explain `auto` as **“Chooses an eligible model within your quality and spending rules.”** Today it does not classify prompt difficulty.

| Card | Action and behavior |
|---|---|
| **Claude Code** | Recommend **Launch through Pygg**. Add a launcher that retrieves its dedicated token from Credential Manager and injects gateway URL, authentication, model aliases and session header into the child process. Preserve the user’s normal Claude configuration and login. Avoid persistent `~/.claude/settings.json` changes as the default. |
| **Codex** | **Apply** reuses the existing provider/profile setup and backup flow. Save the selected model in the Pygg profile; show the command that selects that profile. Add **Launch through Pygg** so Apply alone is not mistaken for changing every Codex session. |
| **OpenCode** | **Apply** reuses its existing backed-up provider configuration. Configure the chosen Pygg model without overwriting unrelated providers; clearly show whether the user must select it in OpenCode. |
| **Cursor / Cline / Continue** | One compatible-editors card with editor tabs. Show exact fields: OpenAI-compatible provider, `/v1` base URL, Pygg API key and model `auto`. Manual setup initially; avoid rewriting arbitrary editor configuration. |
| **SDK / curl** | Language/shell tabs with copyable examples using `/v1` for OpenAI-compatible calls and environment-variable token references. Include an Anthropic example using the root URL. |

Token handling:

- Generate one narrowly scoped token per configured tool; task permission stays off.
- Provider keys never enter tool configuration.
- For launchers, retain the client secret in Credential Manager and inject it only at launch.
- Existing Apply flows write client tokens into tool configuration: disclose that plainly. Manual setup reveals a newly created client token once for copying; afterward show only its fingerprint. Lost tokens require replacement.
- Remove disconnects the integration and revokes its owned token. Rollback must preserve later unrelated edits.

Confirmation states: **Not configured → Configured, awaiting request → Working**, or an actionable failure. Use the newest request for that specific token, showing requested alias, actual model, timestamp and result. A validated config file is not proof that a model request succeeded.

**4. Make savings visible**

Home example:

> **Today:** 84 requests · 51 free · 8 local · 5 subscription · 20 paid  
> **Known API cost:** $0.12 · 3 requests with unknown cost  
> **Estimated difference vs [selected strongest model]: $1.08 lower**  
> Based on 72 comparable requests. View calculation.

Let the user confirm the comparison model; strength cannot be inferred from price alone.

Usage adds:

- Free, local, subscription, paid and unknown-billing breakdowns.
- Actual versus estimated API cost.
- A comparison chart and per-request calculations.
- Routing reasons and quota fallback outcomes.

Calculate comparison cost using observed input/output tokens and the baseline model’s versioned prices. Include all billable attempts attributable to an auto request in its observed cost.

Exclude requests missing required usage/prices or incompatible with the baseline; show coverage. State cache assumptions because cache behavior may differ between models. Subscription fees and local operating costs remain separate.

Label this **“Estimated comparison using the same tokens”**: another model could produce different output lengths. Show negative differences honestly. Unknown cost is never zero, and incomplete totals must never produce a definitive savings claim.

**5. Move complexity out of the main path**

| Keep readily visible | Put behind Advanced or Settings |
|---|---|
| Provider status, billing category, model availability, reconnect | Endpoint override, API format/translation details, pools, independent-quota confirmation |
| Model names and pricing availability | Tiers, context/capability overrides, routing simulator |
| API key name, monthly limit, usage, revoke | Concurrency, retry behavior, minimum tier, connection restrictions, data-use rules |
| Relevant quota/cost warnings | Detailed health diagnostics and raw provider errors |
| Gateway running status | Port and startup configuration in Settings |
| Existing task attention indicator | Task policies, budgets, workspaces, MCP and agent runners under Smart Tasks |

Do not bury unknown-cost warnings or spending controls. Keep localhost binding enforced. Add no default telemetry, chat interface or session/cookie bridge.

**6. Small implementation steps**

Paths below are relative to `apps/desktop/` unless stated otherwise; new files are marked.

1. **Document the structure.** Update `docs/design/SCREENS.md` with navigation, setup states, terminology and cost-comparison rules.
2. **Simplify navigation.** Touch `src/main.tsx`, `src/components/CommandPalette.tsx`, `src/components/SettingsView.tsx`, `src/locales/vi/gateway.ts`. Rename Home/Providers; nest task destinations; remove task launch redirection.
3. **Extract Endpoint.** Add `src/components/gateway/Endpoint.tsx`; touch `Overview.tsx`, `Clients.tsx`, `main.tsx`. Move endpoint details and API Keys without changing token behavior.
4. **Extract CLI Tools.** Add `src/components/gateway/CliTools.tsx`; touch `Agents.tsx`, `Overview.tsx`, `main.tsx`. Move existing gateway integrations and manual snippets.
5. **Add token-specific verification.** Touch `src/lib/api.ts`, `src-tauri/src/gateway/store.rs`, `src-tauri/src/gateway/mod.rs`, `CliTools.tsx`. Expose newest request per client and distinguish configuration from successful use.
6. **Add secure Claude launcher.** Touch `src-tauri/src/integrations.rs`, `src-tauri/src/cli/setup.rs`, `src-tauri/src/cli/mod.rs`, `CliTools.tsx`. Add credential-backed launch, model selection and revocation; test secret redaction and process-environment isolation.
7. **Make first-run eligibility reliable.** Touch `src-tauri/src/gateway/auto.rs`, `catalog.rs`, `tiers.rs`, `src/lib/api.ts`, `CliTools.tsx`. Add tool-specific eligibility preflight; preserve routing policy.
8. **Reduce provider detail density.** Touch `Connections.tsx`, `AutoRouting.tsx`, `src/styles.css`. Introduce Advanced disclosures while retaining actionable health and cost warnings.
9. **Replace Home’s setup wall.** Touch `Overview.tsx`, `UsageAlerts.tsx`, `src/styles.css`. Add resumable setup and the compact working state.
10. **Build comparison accounting.** Touch `src-tauri/src/gateway/store.rs`, `pricing.rs`, `src/lib/api.ts`; add `src-tauri/src/gateway/savings.rs`. Test missing costs, failed attempts, cache assumptions and negative differences.
11. **Expose savings.** Touch `Overview.tsx`, `Usage.tsx`, `CostOverTime.tsx`, `src/lib/money.ts`, `src/locales/vi/gateway.ts`. Show provenance, coverage and baseline selection; verify first-run, keyboard navigation and existing integration rollback.
