# Second interface review by Codex, against 9router's and OmniRoute's dashboards, 2026-11-12

> **Status:** Record · Codex's review as it wrote it (read-only, no files changed), and what was done · order: [ROADMAP](../ROADMAP.md) · built: [Implementation Status](../IMPLEMENTATION-STATUS.md)

Codex compared each screen with 9router's and OmniRoute's dashboard source and ranked the remaining problems. **What was built from it** (52a4e94):
- Home milestones from evidence;
- Review unpriced models;
- a next step after connecting;
- no global working line in CLI Tools;
- a truthful copy for new keys;
- strict limit defaults;
- the Cheapest first display;
- demo fixtures;
- shared spacing and sizes.

Built later the next day (2026-11-13), on the owner's decision:
- request pagination beyond 200 (Show More, 200 at a time);
- a chart metric switch (cost, requests, tokens);
- a check that `auto` has an eligible model before a tool is set up (a note under each tool on CLI Tools, per API surface).

## The review, as written

Pygg now has the right structure for a background gateway. Keep it. The remaining work is chiefly **making readiness, routing eligibility, and cost protection easy to verify**. Several features that appear missing in the screenshots already exist farther down the page or behind disclosures.

No files changed. This is a screenshot and source review, not an interactive accessibility test. **r12 duplicates r11’s Connect Provider sheet**, so template visuals remain unverified.

Local references below are relative to `apps/desktop/src/`, unless otherwise stated.

**1. Direct comparison with 9router and OmniRoute**

I read both READMEs, repository documentation, and dashboard source. OmniRoute’s supplied URL redirects to `cheaper-inference/OmniRoute`. These are comparisons with the source available during this review, rather than claims about every deployed version.

| Area | Reference dashboards | Pygg’s remaining gap / advantage |
|---|---|---|
| Provider capacity | 9router displays quota bars and reset countdowns. OmniRoute puts quota windows beneath individual connections. See [9router ProviderLimitCard.js:149](https://github.com/decolua/9router/blob/master/src/app/%28dashboard%29/dashboard/usage/components/ProviderLimits/ProviderLimitCard.js#L149), [QuotaProgressBar.js:111](https://github.com/decolua/9router/blob/master/src/app/%28dashboard%29/dashboard/usage/components/ProviderLimits/QuotaProgressBar.js#L111), and [OmniRoute ConnectionQuotaPanel.tsx:110](https://github.com/cheaper-inference/OmniRoute/blob/main/src/app/%28dashboard%29/dashboard/providers/%5Bid%5D/components/ConnectionQuotaPanel.tsx#L110). | Pygg has provider-stated limits and account balances, but hides limits under Advanced. Surface a compact capacity summary beside connection health. `components/gateway/Connections.tsx:237`, `:250`, `:389`. |
| Usage chart | 9router lets users switch between requests, tokens, and cost. [UsageChart.js:25](https://github.com/decolua/9router/blob/master/src/app/%28dashboard%29/dashboard/usage/components/UsageChart.js#L25). | Pygg already has a useful cost chart with unknown-cost markers. Add Requests and Tokens using existing buckets; zero known spend currently reveals little about traffic. `components/gateway/CostOverTime.tsx:31`. |
| Request investigation | 9router has a paginated details view with provider/date filters and latency. OmniRoute gives request logs their own page and accepts request/correlation links. [RequestDetailsTab.js:99](https://github.com/decolua/9router/blob/master/src/app/%28dashboard%29/dashboard/usage/components/RequestDetailsTab.js#L99), [OmniRoute logs/page.tsx:46](https://github.com/cheaper-inference/OmniRoute/blob/main/src/app/%28dashboard%29/dashboard/logs/page.tsx#L46). | Pygg already has client/model/result filters, latency, and expandable details. Its weakness is discovery and the 200-entry ceiling—not absence of a log. `components/gateway/Usage.tsx:117`, `:151`, `:310`. |
| Endpoint | 9router auto-provisions a default key. OmniRoute supports key search and status/type filters. [EndpointPageClient.js:251](https://github.com/decolua/9router/blob/master/src/app/%28dashboard%29/dashboard/endpoint/EndpointPageClient.js#L251), [ApiManagerPageClient.tsx:129](https://github.com/cheaper-inference/OmniRoute/blob/main/src/app/%28dashboard%29/dashboard/api-manager/ApiManagerPageClient.tsx#L129). | Pygg’s separate OpenAI and Anthropic URLs are immediately understandable. Tool-owned keys also remove much of the need for a default key. Keep this; add key filtering only when the list grows. `components/gateway/Endpoint.tsx:23`, `components/gateway/Clients.tsx:39`. |
| CLI tools | 9router uses recognizable tool summary cards with installation/configuration states. OmniRoute shares a status badge that can show the configuration timestamp. [ToolSummaryCard.js:7](https://github.com/decolua/9router/blob/master/src/app/%28dashboard%29/dashboard/cli-tools/components/ToolSummaryCard.js#L7), [CliStatusBadge.tsx:41](https://github.com/cheaper-inference/OmniRoute/blob/main/src/app/%28dashboard%29/dashboard/cli-code/components/CliStatusBadge.tsx#L41). | Pygg offers fewer distinct editor entry points and uses generic icons, but **actual requests from each tool’s key are stronger evidence than configuration alone**. Preserve that distinction. `components/gateway/CliTools.tsx:30`, `:47`. |
| Onboarding | 9router’s README describes connect-provider → configure-tool. OmniRoute has a dismissible readiness card linked to onboarding. [9router Quick Start](https://github.com/decolua/9router#-quick-start), [FirstRunReadinessCard.tsx:42](https://github.com/cheaper-inference/OmniRoute/blob/main/src/app/%28dashboard%29/dashboard/FirstRunReadinessCard.tsx#L42). | Pygg now matches the conceptual path, but its checklist completion condition is weaker than its tool-status implementation. `components/gateway/Overview.tsx:35`. |
| Density and scope | OmniRoute’s dashboard includes numerous analytical sections and routing tools; its analytics page alone has several tabs. [analytics/page.tsx:25](https://github.com/cheaper-inference/OmniRoute/blob/main/src/app/%28dashboard%29/dashboard/analytics/page.tsx#L25). | Pygg’s restrained navigation is better suited to “turn on, return to IDE.” Borrow capacity visibility and investigation affordances without copying the breadth. |

**What Pygg does better and should keep**

- **Honest savings:** user-selected baseline, compared-request coverage, exclusions, and the same-token caveat. This is a more defensible product promise than the broad savings claims in the reference [READMEs](https://github.com/decolua/9router#readme). `components/gateway/Savings.tsx:33`, `:53`, `:67`.
- **Cost provenance:** actual, estimated, unknown, and potentially charged attempts remain distinguishable. `components/gateway/Usage.tsx:93`, `components/gateway/Clients.tsx:96`.
- **Reversible setup and credential storage:** the launcher avoids permanently replacing Claude Code settings; integrations explain backup/restoration. `components/gateway/CliTools.tsx:36`, `:78`.
- **Templates fail honestly:** missing capability facts exclude models; the UI explains unmet requirements rather than silently weakening them. `components/gateway/ComboTemplates.tsx:70`, `:82`, `:111`.
- **Local-only endpoint clarity.** Keep the compact URL presentation and explicit reachability explanation. `components/gateway/Endpoint.tsx:32`.

**2. Top 10 remaining usability problems, ranked**

| Rank | Screen / evidence | Concrete fix |
|---|---|---|
| **1 — High** | **Home readiness can be false.** Any active key completes “Set up your coding tool”; provider readiness only requires enabled/key/models, not an eligible coding route. `components/gateway/Overview.tsx:34`, `:41`. | Derive tool completion from launcher/integration state. Keep a third, passive “First request received” milestone. Show whether the chosen tool’s policy has an eligible model. |
| **2 — High** | **Unknown pricing has no direct recovery path.** r02 lists unknown billing, missing prices, and unset routing facts together. Alerts send users to Usage rather than the offending configuration. `components/gateway/UsageAlerts.tsx:22`, `components/gateway/Connections.tsx:437`. | Add “Review unpriced models” linking to a filtered Providers view. Explain that selecting a billing category does not establish model eligibility or price. |
| **3 — High** | **CLI Tools still gives conflicting success signals.** r04 says Scripts is working while all three tools are unconfigured. Tool-specific status exists, but a global latest-request message remains below setup. `components/gateway/CliTools.tsx:38`, `:42`. | Remove the global success message from this screen. Place each tool’s waiting/working/failed state inside its own row, with a filtered Usage link. |
| **4 — High** | **Key-creation instructions contradict Show.** Creation says the key is not stored and cannot be shown again; Endpoint supports retrieval. `components/gateway/Clients.tsx:149`, `:154`, `:89`. | Return credential-storage success with creation. Show either “Saved in Windows Credential Manager” or the accurate one-time-copy fallback. Also replace the stale “Overview” destination with CLI Tools. |
| **5 — Medium** | **Capacity is difficult to scan.** r02 shows “1 model cooling down,” while quota observations sit inside Advanced. `components/gateway/Connections.tsx:103`, `:250`. | Show remaining capacity, reset countdown, observation age, and source in a compact strip. Use “Unknown” when unstated; keep cooldown separate from quota exhaustion. |
| **6 — Medium** | **Usage puts investigation below too much content.** In r06, even Task Results precedes the request ledger. `components/gateway/Usage.tsx:108`, `:109`, `:117`. | Add Summary / Requests tabs within Usage. Open Requests directly from failures and tool status, preserving client/model filters. Keep Smart Tasks results outside the primary gateway summary while that work is on hold. |
| **7 — Medium** | **Provider selection requires too much reading.** r11 fills almost the entire height; local options are below the fold. Subtitles are only 10.5px. `styles.css:640`. | Put “Use a local runtime” and a few clearly explained starting options first, followed by the full searchable catalog. Separate free tiers from trial credit visually. Keep all providers discoverable. |
| **8 — Medium** | **Cost protection is split across defaults and key editing.** Policies offers a monthly limit, but strict reservation is only in the key sheet. `components/gateway/Policies.tsx:135`, `components/gateway/Clients.tsx:227`. | Show “Estimated limit” or “Strict reservation” alongside every key’s limit. Explain the distinction where the limit is entered. Consider adding strict mode to new-key defaults. |
| **9 — Medium** | **Combo presentation implies a fixed execution order even for Cheapest first.** Both strategies render numbered arrows. r05 also shows wrapped arrows stranded at line starts. `components/gateway/Combos.tsx:55`, `:69`, `styles.css:527`. | Use an ordered chain only for Ordered fallback. For Cheapest first, show an eligible member pool and explain request-dependent ordering. Put test results below the header, not among its action buttons. |
| **10 — Medium** | **The review fixtures contradict themselves.** r00 has no requests but spending alerts; r01 says $8.40/$10 while Endpoint shows a $20 limit and different spend. The alert fixture is hardcoded. `lib/demo.ts:202`, `:754`. | Derive demo alerts, savings, and totals from the same scenario. First run should have no historical warnings. Recapture r12 with the actual template sheet. |

**3. Visual polish**

These are design judgments for a Windows desktop app. I used the skill’s hierarchy and progressive-disclosure principles, rather than treating macOS styling as a Windows requirement.

| Component / CSS | Specific adjustment |
|---|---|
| **Section rhythm:** `.group`, `.group-heading`, `.savings-hero` — `styles.css:279`, `:309`, `:804` | r01/r06 have warning and savings cards touching. Use a consistent 16px gap between independent blocks and 24px between major sections. Reserve separators for rows inside a block. |
| **Heading hierarchy:** `.group h2`, `.toolbar-title h1` — `styles.css:281`, `:150` | A 13px section heading barely separates itself from row labels. Use 15px section headings, 13px row titles, and 12px supporting text. Keep the toolbar title quiet. |
| **Readable density:** `.preset-text small`, `.chain-meta`, `.form-hint` — `styles.css:640`, `:846`, `:563` | Provider subtitles at 10.5px and combo facts at 10px feel unfinished at this window size. Raise essential explanatory text to 12px; disclose secondary facts instead of shrinking them. |
| **Buttons:** `.button.small`, `.segmented button` — `styles.css:319`, `:450` | The 22px controls feel cramped beside 44px rows. Use 28px compact actions and roughly 30–32px principal actions. Standardize “Set Up…” when it opens a sheet and “Set Up” when it acts immediately. |
| **CLI rows:** `.agent-row`, `.agent-title`, `.provider-logo` — `styles.css:585`, `:588`, `:641` | Claude is a separate card while Codex/OpenCode share one. Put all three in one consistently divided list. Use recognizable tool artwork where available; reserve generic plugs for generic connections. |
| **Badges:** `.badge` and tone variants — `styles.css:328` | The shape is consistent. The vocabulary needs distinct dimensions: installed, configured, request succeeded, and cooling down. Green “Set up” should not be mistaken for demonstrated routing success. |
| **Savings:** `.savings-hero-body`, `.savings-line` — `styles.css:808`, `:815` | Keep the large figure. Shorten the comparison sentence and move methodology into disclosure. Keep coverage and unknown-cost exclusions visible beside the claim. |
| **Forms:** `.form-label`, `.form-hint` — `styles.css:560`, `:563` | r07’s “auto from tier” and long wrapping labels are hard to parse. Prefer “Minimum model tier,” explanatory option names, and labels that fit one line at normal width. |
| **Settings width:** `.settings` versus `.page` — `styles.css:278`, `:304` | The narrower Settings form is reasonable. Align its section title treatment and vertical rhythm with other screens; a narrow form does not need a separate visual language. |

The stylesheet already provides semantic colors, a visible focus ring, and increased-contrast adaptations (`styles.css:32`, `:59`, `:79`). Preserve them. Screenshot appearance alone does not establish contrast or keyboard accessibility failures.

**4. First-run path**

**It is now as short conceptually as 9router’s documented path, but equivalent completion effort is not yet demonstrated.** 9router describes connect-provider → configure-tool; Pygg has those same two stages and can create the tool key during setup. [9router Quick Start](https://github.com/decolua/9router#-quick-start).

What is still missing:

1. **One obvious next action.** In r00, connecting a provider should be primary; tool setup should become primary once a usable route exists.
2. **A continuation after connection:** “Provider connected → Set up your tool,” retaining onboarding context.
3. **Eligibility confirmation:** model discovery alone does not prove that `auto` can satisfy tools, tier, context, and pricing policy.
4. **Tool selection before manual instructions.** The large Claude snippet should not dominate first run for a Codex user.
5. **A truthful finish:** configured → waiting for that tool → first successful request, with an optional explicit test that explains potential cost.
6. **Clean fixtures and current wording.** Home still describes Claude setup as a snippet rather than the launcher (`components/gateway/Overview.tsx:42`).

The target should remain: **Connect provider → Configure chosen tool → Resume work**. Verification can happen passively.

**5. Ordered implementation steps — at most 12**

Paths below use the same local prefix; Rust paths are repository-relative.

1. **Repair demo scenarios and recapture screenshots.**  
   `lib/demo.ts`. **Backend: no.**

2. **Correct key-storage and destination copy; expose storage outcome.**  
   `components/gateway/Clients.tsx`, `lib/api.ts`, `apps/desktop/src-tauri/src/commands.rs:635`. **Backend: yes**, to distinguish successful vault storage from fallback.

3. **Remove global CLI success; add per-tool Usage links.**  
   `components/gateway/CliTools.tsx`, `components/gateway/Agents.tsx`. **Backend: no.**

4. **Make Home completion reflect tool configuration and eligibility.**  
   `components/gateway/Overview.tsx`, `components/gateway/AutoRouting.tsx`, `lib/api.ts` if needed. **Backend: no expected**, reusing existing integration and route-plan APIs.

5. **Carry onboarding forward after provider connection; collapse manual setup initially.**  
   `components/gateway/Connections.tsx`, `components/gateway/CliTools.tsx`, `main.tsx`. **Backend: no.**

6. **Add filtered recovery for missing prices and routing facts.**  
   `components/gateway/UsageAlerts.tsx`, `components/gateway/Connections.tsx`, `main.tsx`. **Backend: no** for navigation/filtering.

7. **Surface existing quota/account observations with countdown and freshness.**  
   `components/gateway/Connections.tsx`, `lib/format.ts`, `styles.css`. **Backend: no** for currently available data.

8. **Show limit mode clearly; optionally support strict defaults.**  
   `components/gateway/Clients.tsx`, `components/gateway/Policies.tsx`, `lib/api.ts`; Rust default/store handling. **Backend: yes** for strict defaults; labeling alone is frontend.

9. **Add Summary / Requests navigation and filtered entry points.**  
   `components/gateway/Usage.tsx`, `components/gateway/Overview.tsx`, `main.tsx`. **Backend: no.**

10. **Add request-history pagination beyond 200 entries.**  
    `components/gateway/Usage.tsx`, `lib/api.ts`, `apps/desktop/src-tauri/src/commands.rs`, `apps/desktop/src-tauri/src/gateway/store.rs`. **Backend: yes.**

11. **Differentiate combo strategies and improve test-result placement.**  
    `components/gateway/Combos.tsx`, `styles.css`. **Backend: no.**

12. **Apply shared polish and chart metric switching.**  
    `styles.css`, `components/gateway/Connections.tsx`, `components/gateway/CliTools.tsx`, `components/gateway/Savings.tsx`, `components/gateway/CostOverTime.tsx`. **Backend: no** for Requests/Cost using existing buckets.

All steps preserve localhost operation, Credential Manager storage, honest unknown costs, and the existing account-risk boundary. Smart Tasks requires no new implementation work.
