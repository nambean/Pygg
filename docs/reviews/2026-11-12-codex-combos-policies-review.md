# Combos, savings, Policies and a Guide: review by Codex, 2026-11-12

> **Status:** Record · Codex's review as it wrote it (read-only, no files changed), and what was done · order: [ROADMAP](../ROADMAP.md) · built: [Implementation Status](../IMPLEMENTATION-STATUS.md)

Codex was asked, with research on 9router, OmniRoute, LiteLLM, Portkey and OpenRouter, how to redesign Combos with templates, make savings prominent, fill Policies and add a Guide. **What was built from it:**
- 750fd9b: the savings hero, with a suggested baseline.
- 85ef08f: combo templates and tabs.
- a96ba1e: Policies, with defaults for new keys.
- d78ed0d and 3dd3c16: the Guide.
- 9dbe82d: Test combo, drag to reorder, and cheapest first.

Not built from it:
- request pagination beyond 200;
- round robin across quota groups;
- a latency strategy.

## The review, as written

Pygg needs clearer hierarchy more than more features: **Combos should build understandable fallback routes; Home should lead with estimated savings; Policies should expose the controls that already exist; Guide should make setup self-service.** No files were changed.

My assessment is **Needs work**: the screenshots show useful underlying functionality, but the main benefits are buried beneath explanations and secondary controls. These are design recommendations for a Windows utility; I have not measured contrast or tested keyboard interaction.

**A. Combos**

Borrow the ordered editor and client-name previews from [9router](https://github.com/decolua/9router/blob/master/gitbook/content/en/features/combos.md), and the guided setup and route inspection from [OmniRoute](https://github.com/cheaper-inference/OmniRoute/blob/release/v3.8.54/docs/routing/AUTO-COMBO.md). Keep Pygg’s stricter retry and spending semantics.

Do not copy “always include free at the end.” A free model can lack the required tools, context, or protocol support. Likewise, combining several models does not combine their context windows.

**Screen structure**

At 1440×900, use the available content width for three elements:

1. **Header:** “Combos” with `From template` and `New combo`.
2. **Compact introduction:** “One model name. A fallback order you control.”
3. **One-column list of route cards.**

Each card should show:

| Element | Recommended content |
|---|---|
| Heading | `coding` · Ordered fallback · 3 members |
| Route | Numbered members with connection labels and model IDs |
| Member summary | Billing, tier, tools/vision, context, measured health |
| Readiness | “3 available” or “1 disabled · 1 missing model” |
| Actions | Copy name, Edit, Test; Delete in an overflow menu |
| Secondary detail | Last test result and timestamp, expandable |

Show member health as, for example, “94% success · 47 attempts · mean latency 2.1s · last 24h.” Use the actual measured statistic, not an invented percentile. With insufficient data, say **“Not measured.”** The existing reliability aggregation starts at `apps/desktop/src-tauri/src/gateway/store.rs:1637`.

Avoid an aggregate “healthy” badge that conceals one working member and three unavailable backups.

**Editor**

The current name-plus-members model is appropriate for ordered fallback (`store.rs:1207`). Improve the editor rather than introducing a four-step wizard for every manual edit.

Use a wider dialog with:

- **Name:** validate while typing; show “Clients send this value as `model`.”
- **Requirements for this route:** tools, vision, context and protocol filters used to help selection. Clearly distinguish selection aids from runtime guarantees.
- **Searchable member picker:** group by connection; filter by billing, tier and capabilities.
- **Numbered rows:** model, connection, capabilities, health, Remove and Test.
- **Ordering:** drag handles plus the existing Move up/Move down buttons.
- **Review strip:** unavailable members, shared quota groups, protocol mismatches and unknown metadata.
- **Footer:** Cancel and Create combo/Save changes.

The current picker filters enabled models but does not filter disabled connections (`Combos.tsx:91`). Exclude disabled connections from normal selection, while preserving and explaining existing disabled members.

Keep the shared-quota warning already implemented at `Combos.tsx:72`.

Use a short visible fallback explanation:

> Tries members in order when a key is rejected, quota is exhausted, or a model is unavailable. Once an answer starts streaming, Pygg does not switch models.

Put timeout, server-error and paid-retry details under **“When fallback happens.”**

Keyboard reordering must remain available. This follows the skill’s `accessibility.md › Mobility` guidance: “Offer alternatives to gestures.”

**Testing**

Separate two actions:

- **Preview route:** no upstream request; show eligibility for a selected API key, protocol, tool/image requirements and token sizes.
- **Send test request:** explicitly sends a short request, records it in Usage, and may incur cost.

Per-member testing can reuse `api.testModel` (`apps/desktop/src/lib/api.ts:272`). A combo test needs a separate path through the real combo resolver. It should identify the answering member and any skipped or failed members. One successful request proves that path worked; it does not prove every backup works.

Do not test automatically when opening an editor or selecting a template.

**Move Automatic Routing out**

The large explainer and simulator currently enter the Combos page at `Combos.tsx:60`. Move them to **Gateway → Routing**, with tabs **Automatic** and **Combos**. Keep the default destination on Automatic because `auto` is Pygg’s main benefit.

Automatic should show:

- `auto`: lowest eligible tier at or above the key’s minimum; then local, free, subscription, paid; then estimated cost.
- `auto/cheap`: lowest estimated request cost among eligible models; no minimum tier.
- The existing simulator and exclusion reasons.

Those distinctions are implemented in `apps/desktop/src-tauri/src/gateway/auto.rs:9`. Do not label `auto` simply “cheapest.”

**Templates**

Templates should resolve rules into concrete members **at creation time**. Current storage cannot retain dynamic selection rules. Explain: “This creates a saved list. New models are not added automatically.”

For all coding templates, default to enabled connections/models with verified tool support, T2 or above, and the selected client protocol. Unknown capability metadata does not satisfy a requirement. Allow the user to change these filters before resolving.

For price ordering, show the reference workload used—for example, 2,000 input and 1,000 output tokens. Static ordering cannot promise cheapest routing for every request.

| Template | Purpose | Selection rules and order | Backend |
|---|---|---|---|
| **Free first** | Try qualifying free resources before paid backups. | Local → free → paid; within each group, higher tier first, then lower reference cost. Paid members require known prices. | Ordered fallback works now |
| **Subscription only** | Use already purchased plans without spilling into paid API usage. | Subscription billing only; higher tier first, then measured success, then stable ID. Exhaustion ends in an error. | Works now |
| **Use my plans first** | Prefer subscriptions, then free resources, then paid backups. | Subscription → local → free → paid; higher tier within the first three groups; paid ordered by reference cost. | Works now |
| **Strong coding, cheap backup** | Start strong and retain an affordable alternative. | First: highest-tier tools model with context ≥128k. Then distinct T2+ tools models with context ≥128k, ordered by reference cost. | Works now |
| **No metered API spend** | Exclude subscriptions and paid API models. | Free/local billing only; local before free, then higher tier. Explain local electricity/hardware costs are outside the ledger. | Works now |
| **Local only** | Keep model processing on this computer. | Local billing **and a verified loopback endpoint**; higher tier first, then measured latency. Unknown endpoint locality is excluded. | Works now; needs resolver locality checks |
| **Long-context coding** | Keep every backup suitable for large requests. | Tools, T2+, known context ≥200k; reference cost ascending, then higher tier. | Works now |
| **Measured fast** | Prefer models that recently answered faster. | Tools, T2+, sufficient recent measurements; observed latency ascending, success rate as tie-breaker. Unmeasured candidates shown separately. | Frozen order works now; adaptive ranking needs backend |
| **Spread free quotas** | Distribute requests across independent free capacity. | Qualifying free models/accounts from independently declared quota groups; rotate the eligible starting member. | New round-robin strategy |
| **Cheapest for this request** | Choose according to actual request size. | Known-price eligible members; rank by estimated request cost, then deterministic tie-breakers. | New per-combo cost strategy; recommend existing `auto/cheap` meanwhile |

“Works now” means the execution model already supports the resulting list; the template resolver and UI still need implementation.

A saved list is also **not a permanent rule-enforced boundary**. Billing or endpoint metadata may change later. If Pygg promises “never paid” or “local only” at runtime, persist that constraint and enforce it before every attempt; otherwise describe the template as a list selected from today’s metadata.

**Template flow**

`Pick template → Preview resolved members → Name → Create`

The preview should show:

- Matching members and their exact order.
- Why each was selected.
- Excluded candidates with reasons.
- Missing stages: “No subscription matched; free and paid stages remain.”
- The selected key’s access restrictions and retry policy.
- Whether the result is a static list or an adaptive strategy.

If nothing matches, disable Create and offer precise remedies: **Connect provider**, **Refresh model facts**, or **Change requirements**. Never silently add paid models, lower the tier, remove privacy requirements, or broaden locality.

A single match may be created, with “One member matched; this route has no backup.” Revalidate before saving if connections changed during the flow.

**Strategies worth adding next**

1. **Request-cost ordering within a combo.** Reuse the cost logic behind `auto/cheap`, restricted to explicitly selected members. This supports “these models only, cheapest eligible first.”
2. **Round robin across independent quota groups.** Useful for spreading free or subscription capacity. Avoid presenting accounts sharing a quota as independent capacity.
3. **Lowest observed latency, with a quality floor.** Require adequate samples, bounded measurement windows and stable tie-breaking. Keep existing session affinity and request compatibility checks.

All three must preserve budget enforcement, key restrictions, shared-quota handling and stream boundaries. Defer model-based routing, Fusion, nested combos and pipelines: they introduce additional calls, cost and debugging complexity.

**B. Savings on Home and Usage**

The calculation is already substantially right. The presentation is too quiet.

Home currently places Savings after the five activity counters (`apps/desktop/src/components/gateway/Overview.tsx:75`, `:88`). Usage places it after API Costs (`Usage.tsx:91`, `:107`).

**Home layout**

Use this order:

1. Compact gateway status.
2. Urgent operational/spending alerts, when present.
3. **Estimated savings hero.**
4. Activity counters and recent requests.

For the screenshot’s data:

> **≈$2.13**  
> Estimated savings · Last 24 hours  
> Compared with anthropic/claude-sonnet-4-5  
> Routed cost $0.10 · Same-token comparison $2.22  
> **45 of 65 answered requests compared**  
> 20 requests excluded for unknown cost · 3 failed attempts may have additional unknown charges  
> `Change comparison` · `View calculation`

Use unrounded values for the calculation; displayed rounded amounts may differ by a cent.

Make the amount approximately 32–36px with tabular numerals. Keep the baseline, period and coverage close enough to read together. Green may support a positive amount, but text must carry its meaning.

**Usage layout**

Keep the period selector at the top, then show:

- The same savings hero for the selected period.
- Known API cost, split into reported and estimated amounts.
- Unknown-cost request count.
- Cost per day and ledger.

Keep subscription fees and local runtime accounting separate. Do not add them to a token-price savings figure without a defensible allocation method.

An optional percentage is acceptable as a secondary figure:

`(baseline cost − routed cost) / baseline cost`

Only show it when the baseline denominator is positive. Never clamp a negative result: display **“≈$X more than the comparison model.”**

**Before a baseline is chosen**

Show:

> **Choose your comparison model**  
> Estimate what these requests would have cost using the model you would otherwise use.

Keep known API spend and free/local/subscription request counts visible. Do not show `$0 saved`.

Propose a baseline, but require acceptance:

> Suggested: [model], your most-used explicitly requested paid model with a known price in this period.

Use actual direct-request history, excluding combo/auto names. If that evidence is absent, use the picker without a suggestion. Never select the most expensive connected model to maximize the headline.

Persist the accepted baseline; do not change it when another model becomes more expensive. If its price disappears, show “Comparison unavailable—add a price or choose another model.”

**Honesty requirements**

Keep these visible or one click away:

- “Estimate using the same tokens and cache split; another model may produce different output.”
- Coverage by **request count**, not an implied share of total spend.
- Unknown costs excluded, never treated as zero.
- Subscription requests excluded from token-price comparison.
- Known failed-attempt costs included.
- Unknown failed-attempt charges explicitly counted.
- Local figures exclude electricity and hardware.

The backend already models these categories at `apps/desktop/src-tauri/src/gateway/savings.rs:35`. Its known failed costs are period-wide, including failures without a compared answer; describe that accurately rather than calling the routed total solely the cost of matched successful requests.

The compact Home variant currently hides subscription and methodology explanations (`Savings.tsx:60`). Retain a concise caveat on Home. Also expose exclusions independently of whether a savings figure exists; currently several are gated by `figure` at `Savings.tsx:55`.

**C. Policies**

Move **Policies into Gateway**. It controls gateway API keys, routing and spending; placing it under Smart Tasks misrepresents its scope. Remove Policies from the task-screen classification too (`apps/desktop/src/main.tsx:167`).

Replace the placeholder at `main.tsx:257` with these sections, in this order:

| Section | Content and editing |
|---|---|
| **Defaults for new API keys** | Editable creation defaults for tier, budget, strictness, concurrency, unknown-price policy, retries, privacy, connection scope and task permission |
| **API-key policies** | Searchable table: name, monthly limit, spent/held, concurrency, auto tier, unknown-price handling, retries, privacy and connection scope; Edit opens the shared Limits editor |
| **Connection policies** | Terms and their source, quota independence, pool membership; link directly to the relevant Providers editor |
| **Gateway guarantees** | Read-only: localhost only, credentials in Windows Credential Manager, unknown cost never `$0` |
| **Smart Tasks — on hold** | Collapsed summary linking to existing Settings and Workspaces controls; no new task workflow |

The existing Limits editor already covers most per-key controls (`apps/desktop/src/components/gateway/Clients.tsx:185`). Extract and reuse it so Endpoint and Policies cannot drift.

**Defaults versus overrides**

For the first release, use explicit **creation defaults**:

> Applied when a key is created. Existing keys keep their saved settings.

Show each key’s effective values and differences from today’s defaults. Do not call matching values “inherited”: the current schema stores concrete per-key values.

Provide **“Apply defaults to selected keys”** only with a preview of changed fields. Do not silently reset monthly budgets or expand allowed connections.

True live inheritance would need nullable overrides, defined precedence and migration semantics. It is unnecessary for the first useful Policies screen.

**New defaults**

Recommended starting defaults:

- `auto` minimum tier: **T2**.
- Paid retries after timeout/server error: **0**.
- Unknown prices: **Block**, with a clear explanation of when this affects eligibility and budget enforcement.
- May start tasks: **Off**.
- Monthly budget: explicitly **No limit**, or a user-entered amount; never imply a cap exists.
- Strict budget: retain/expose the existing behavior, explaining that it requires a monetary limit and sufficient price/output metadata.
- Concurrency: expose the existing default.
- No-training requirement: explicit choice based on declared provider terms; unknown terms do not count as compliant.
- Connection scope: explicit “All permitted connections” versus a selected set; explain whether future connections enter that scope.

The minimum tier applies to `auto`; `auto/cheap` ignores it, as the existing Limits hint states (`Clients.tsx:256`). Do not imply the tier setting governs every explicit model or combo.

Creation defaults must apply consistently to UI-created keys, CLI-created keys and setup-generated keys through the shared backend creation function (`store.rs:659`).

**D. Guide menu**

Add **Guide** under **App**, immediately above Settings, and add a command-palette entry. Use a searchable guide page with a short topic list and deep links from relevant screens.

Bundle the content with the app so it works offline and matches the installed version. Add Copy buttons to commands and links to longer online documentation. This follows `offering-help.md › Best practices`: “directly relate the help you provide to the precise action or task.”

Use PowerShell examples by default. Display `pygg` and `pygg-cli.exe` as alternative launchers for the same commands.

**1. Quick start**

Outline: connect provider → refresh model facts → create a limited gateway key → configure a tool → verify a request in Usage.

```powershell
pygg status
pygg doctor
pygg setup
```

Explain that provider credentials belong in Providers and remain in Windows Credential Manager. Tools use a **Pygg gateway token**, not a provider key.

**2. Install the CLI with npm**

```powershell
node --version
npm --version
npm install -g @pygg/cli@dev
pygg --version
pygg --help
```

Explain that `@dev` is the development channel. If Node/npm are absent, link to their installation instructions; the desktop installation also supplies the native CLI.

Source: `packages/npm/pygg/README.md:6`.

Native alternative:

```powershell
& "$env:LOCALAPPDATA\Pygg\pygg-cli.exe" doctor
```

**3. Claude Code**

```powershell
pygg setup claude-launcher --dry-run
pygg setup claude-launcher
pygg run --model auto claude
```

Undo:

```powershell
pygg setup claude-launcher --remove
```

Explain that this routes one launch through Pygg and requires the gateway running. Keep `claude-code` integration as a separately explained option rather than mixing its behavior with the launcher.

Sources: `apps/desktop/src-tauri/src/cli/setup.rs:18`, `cli/run.rs:35`.

**4. Codex**

```powershell
pygg setup codex-models --dry-run
pygg setup codex-models
codex --profile pygg
```

Undo:

```powershell
pygg setup codex-models --remove
```

These commands come directly from `cli/setup.rs:21`. Explain how to select `auto` from the configured models. This is gateway routing, distinct from MCP.

**5. OpenCode**

```powershell
pygg setup opencode-models --dry-run
pygg setup opencode-models
opencode
```

Select Pygg and `auto` in the tool. Undo:

```powershell
pygg setup opencode-models --remove
```

Source: `cli/setup.rs:22`.

**6. Editors and MCP**

For editors supporting an OpenAI-compatible custom provider, show copyable fields:

```text
Base URL: http://127.0.0.1:8741/v1
API key:  <Pygg gateway token>
Model:    auto
```

Describe compatibility as dependent on the editor’s provider support.

For MCP:

```text
Command: pygg
Arguments: mcp
```

Explain that the editor launches this process over stdio. Do not instruct users to configure port 8741 as an MCP endpoint.

Keep task-starting instructions out of the main setup path while tasks are on hold. Avoid a universal MCP JSON example: editor schemas differ, and a copied configuration should not persist provider secrets.

**7. SDKs and HTTP**

A PowerShell request avoids requiring an SDK installation:

```powershell
$headers = @{ Authorization = "Bearer $env:PYGG_TOKEN" }
$body = @{
    model = "auto"
    messages = @(
        @{ role = "user"; content = "Reply with OK." }
    )
    max_tokens = 16
} | ConvertTo-Json -Depth 5

Invoke-RestMethod `
    -Uri "http://127.0.0.1:8741/v1/chat/completions" `
    -Method Post `
    -Headers $headers `
    -ContentType "application/json" `
    -Body $body
```

Label `PYGG_TOKEN` as a gateway token supplied locally. The sample sends a real request.

Document the supported surfaces:

```text
OpenAI Chat Completions: /v1/chat/completions
OpenAI Responses:       /v1/responses
Anthropic Messages:     /v1/messages
```

These routes are registered at `apps/desktop/src-tauri/src/gateway/mod.rs:417`.

**8. Combos and auto**

Explain ordered fallback, streaming boundaries, static templates and the difference between `auto` and `auto/cheap`.

Copyable CLI examples, with `CONNECTION` and `MODEL` marked as placeholders:

```powershell
pygg gateway combo --name coding --member CONNECTION/MODEL --member BACKUP_CONNECTION/BACKUP_MODEL
pygg gateway auto --alias auto --client "Claude Code" --tools yes --prompt 2000 --max-tokens 1000
pygg gateway auto --alias auto/cheap --client "Claude Code" --tools yes --prompt 2000 --max-tokens 1000
pygg gateway test-model --model CONNECTION/MODEL
```

The `auto` command previews without sending; `test-model` sends a short request and may cost money (`cli/gateway.rs:272`, `:185`).

**9. Costs, limits and command reference**

Explain known versus estimated versus unknown cost, held spend, strict limits, subscription accounting and savings coverage.

```powershell
pygg gateway create-client --name "My tool" --monthly-usd 10
pygg gateway client --name "My tool" --auto-min-tier T2 --paid-retries 0 --tasks no
pygg gateway usage
pygg gateway --help
```

The command reference should include exact forms for advanced connection/model administration:

```powershell
pygg gateway add-connection --preset gemini --key-env GEMINI_API_KEY
pygg gateway refresh-models --id CONNECTION
pygg gateway lookup
pygg gateway model --model CONNECTION/MODEL --tier T2 --context 128000 --tools yes
pygg gateway terms --id CONNECTION --terms no_training --source "Provider terms URL"
pygg gateway list
```

Explain that `terms` records a declaration, not independent verification. Do not ask users to put literal provider keys into commands.

Include headless operation separately:

```powershell
pygg gateway serve
```

Explain store ownership and running one gateway for that store. Keep `agent detect | run | task | quality` in an advanced reference, with task commands marked on hold.

The authoritative command forms are at `cli/gateway.rs:47`.

**10. Troubleshooting and FAQ**

```powershell
pygg status
pygg doctor
pygg diagnostics --out "$env:USERPROFILE\Desktop\pygg-diagnostics.zip"
pygg agent detect
```

Troubleshooting should map symptoms to actions:

- Cannot connect → gateway status and configured port.
- No eligible model → simulator exclusion reasons.
- 429 → provider quota or key concurrency limit.
- Budget refusal → spent/held amounts and strict-limit requirements.
- Unknown cost → model pricing in Providers.
- Missing tool integration → setup status and dry-run.
- npm command missing → terminal restart/PATH and native CLI alternative.

FAQ should answer: “Does Pygg provide models?”, “Is free unlimited?”, “Why is cost unknown?”, “Why did it stop after a server error?”, “Does fallback retry poor answers?”, and “Can another computer reach it?” The last answer is **No: localhost only**.

**E. Ordered implementation list**

These are proposed changes, not edits made in this review.

| Order | Small deliverable | Files touched |
|---|---|---|
| 1 | Promote savings while preserving period, baseline and coverage; handle negative/unavailable states | `components/gateway/Savings.tsx`, `Overview.tsx`, `Usage.tsx`, `src/styles.css` |
| 2 | Add evidence-based baseline suggestions requiring acceptance; expose exclusions before baseline selection | `src-tauri/src/gateway/savings.rs`, `src-tauri/src/commands.rs`, `src/lib/api.ts`, `Savings.tsx` |
| 3 | Create Routing with Automatic/Combos tabs; relocate the existing simulator | `src/main.tsx`, new `components/gateway/Routing.tsx`, `AutoRouting.tsx`, `Combos.tsx` |
| 4 | Improve cards and editor: searchable picker, connection filtering, health/sample sizes, accessible ordering | `Combos.tsx`, `src/lib/api.ts`, `src/styles.css` |
| 5 | Implement deterministic template resolution and preview/name/create flow | New `src/lib/comboTemplates.ts`, new `components/gateway/ComboTemplates.tsx`, `Combos.tsx` |
| 6 | Add combo route preview and explicit live test, reusing real routing and ledger behavior | New `src-tauri/src/gateway/combo_preview.rs`, `gateway/mod.rs`, `commands.rs`, `src-tauri/src/lib.rs`, `src/lib/api.ts`, combo editor |
| 7 | Replace Policies placeholder; move navigation; reuse Limits editor | `src/main.tsx`, new `components/gateway/Policies.tsx`, `Clients.tsx`, extracted shared Limits component |
| 8 | Add creation defaults through the shared backend; preview application to selected existing keys | `gateway/store.rs`, `commands.rs`, `src/lib/api.ts`, `Policies.tsx`; migration and `db.rs` only if required |
| 9 | Ship bundled Guide with search, deep links and copyable PowerShell commands | New `components/Guide.tsx`, new `src/lib/guide.ts`, `src/main.tsx`, `src/styles.css`, relevant localization resources |
| 10 | Add request-cost ordering, then quota-aware round robin; evaluate adaptive latency afterward | `gateway/store.rs`, new combo strategy module, `gateway/mod.rs`, `gateway/anthropic.rs`, `gateway/responses.rs`, `src/lib/api.ts`, combo editor, migration/`db.rs` |

For validation, prioritize template exclusion/no-match cases, default propagation across every key-creation path, savings omissions and negative results, and strategy behavior under shared quotas, streaming and paid-retry restrictions. Manually check keyboard reordering, dialog focus, larger text and narrow windows.

Keep task development on hold throughout. None of this requires telemetry, a chat interface, remote access, or moving credentials out of Windows Credential Manager.
