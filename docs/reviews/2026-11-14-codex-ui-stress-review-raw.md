The original bug was a predictable consequence of the layout rules, and the earlier review did not exercise the state that exposed it. The new Models design improves containment, but **I would not consider the 1024 px layout finished**.

I reviewed the supplied screenshots and current source, including the requested sheets. Findings below distinguish visible defects from code paths that need browser reproduction. No files were edited; I did not run the browser check.

**1. Why this shipped**

Several rules combine to let content determine the width of the interface:

- **Automatic table layout is the default.** `.data-table` has `width: 100%`, but that does not prevent its intrinsic minimum width from exceeding its container. `.models-table` explicitly retains `table-layout: auto`. See [styles.css:473](../../apps/desktop/src/styles.css#L473) and [styles.css:487](../../apps/desktop/src/styles.css#L487).
- **`.narrow` means “reserve enough width for everything on one line.”** `width: 1%; white-space: nowrap` makes billing, price and actions compete for guaranteed space. It does not impose a maximum width. See [styles.css:480](../../apps/desktop/src/styles.css#L480).
- **`td.row-buttons` overrides the flex layout.** The general class declares flex, but the table-cell override restores `display: table-cell`; there is no wrapping flex container around the buttons. Combined with `.narrow`, the entire action strip becomes one indivisible width. See [styles.css:493](../../apps/desktop/src/styles.css#L493).
- **Unbroken identifiers remain intrinsic-width hazards.** Some places deliberately ellipsize them; others render raw IDs, paths, provider errors or user names without a wrapping contract. Examples include combo chips, expanded request details and connection metadata.
- **`min-width: 0` is applied selectively.** It exists on many useful wrappers, but is missing from several nested flex/grid children. A shrinkable outer container does not automatically make every descendant shrinkable.
- **Scroll wrappers contain symptoms, not readability failures.** `.table-box { overflow-x: auto }` can prevent page-wide overflow while still leaving actions offscreen and text columns unusably narrow. See [styles.css:472](../../apps/desktop/src/styles.css#L472).

The process gap is equally specific: the previous review checked screens, but not a **screen × data × interaction × size** matrix. Short names and untouched rows do not exercise long test responses. The review document records browser checks of links, filters and wording, but no repeatable geometry assertion covering the original reproduction. See [previous review](2026-11-14-codex-ui-review-all-screens.md#L1).

The current screenshots still miss that reproduction: **Providers shows Gemini selected, not the 40-model OpenRouter connection, and no model test result is visible.** The stress fixture adds long OpenRouter models, but the selected connection defaults to the first visible connection. See [Connections.tsx:64](../../apps/desktop/src/components/gateway/Connections.tsx#L64) and [demo.ts:333](../../apps/desktop/src/lib/demo.ts#L333).

That is why an obvious installed-app defect can survive several screenshot reviews: the relevant rendered state was never actually inspected.

**2. Ranked findings**

“Visible” means supported by the supplied screenshots. “Code risk” means the markup permits the defect, but its exact extent needs rendering.

| Severity / evidence | Screen and size | Defect and source | Concrete fix |
|---|---|---|---|
| **High — visible** | Providers, 1024 | The model column is the leftover column. The rate-limited model becomes `gemini-3…` while billing, price and four buttons remain fully visible. Routing explanations wrap into a tall, narrow paragraph. [ModelRow:623](../../apps/desktop/src/components/gateway/Connections.tsx#L623), [models-table:489](../../apps/desktop/src/styles.css#L489). | Put billing beneath the name; shorten price text; retain Test and a More menu. Allocate the name column explicitly rather than assigning it whatever remains. |
| **High — code risk** | Combos, both widths, after Test | `ComboTestButton` inserts its response directly into the already crowded, nonwrapping `.combo-head`. This repeats the original mistake of placing variable-length results beside actions. `max-width: 360px` does not provide wrapping for an unbroken identifier. [Combos.tsx:58](../../apps/desktop/src/components/gateway/Combos.tsx#L58), [Combos.tsx:114](../../apps/desktop/src/components/gateway/Combos.tsx#L114), [styles.css:893](../../apps/desktop/src/styles.css#L893). | Render the result in a separate full-width block below the header, with `min-width: 0; overflow-wrap: anywhere`. |
| **High — code risk** | Combos, especially 1024 | A chain wraps **between chips**, but a single chip has fixed height and an unbounded raw model ID. One sufficiently long model can overflow the whole card. The current fixture fits narrowly; that is not a constraint. [Combos.tsx:81](../../apps/desktop/src/components/gateway/Combos.tsx#L81), [styles.css:552](../../apps/desktop/src/styles.css#L552). | Use an ordered list with a constrained name track. Alternatively give chips `max-inline-size: 100%`, a separate shrinkable name span and automatic height. Provide full-name expansion/copy. |
| **High — code risk** | Usage → Requests, both widths, expanded failure | The detail grid uses `max-content 1fr`; its values include raw requested models, resolved models and errors. A long token can set the `1fr` track’s minimum width and widen the table. [styles.css:517](../../apps/desktop/src/styles.css#L517), [Usage.tsx:437](../../apps/desktop/src/components/gateway/Usage.tsx#L437). | Change to `max-content minmax(0, 1fr)` and give `dd` `min-width: 0; overflow-wrap: anywhere`. Exercise expanded failures in the release check. |
| **High — code risk** | Price, Auto, Limits, combo editor sheets; both window widths | Sheet titles ellipsize the very identifier needed to confirm which model/key/combo is being edited. Similar prefixes make this dangerous. No full-title alternative is rendered by `Sheet`. [styles.css:575](../../apps/desktop/src/styles.css#L575), [ui.tsx:35](../../apps/desktop/src/components/ui.tsx#L35). | Use a short operation title, then a separate selectable, wrapping full identifier. Do not rely solely on a mouse tooltip. |
| **Medium — visible** | Combos, 1024 and 1990 | The long combo name and strategy compete with four actions. At 1024 the strategy breaks into a three-line fragment; even at 1990 the name wraps despite substantial unused window space. [Combos.tsx:58](../../apps/desktop/src/components/gateway/Combos.tsx#L58), [styles.css:548](../../apps/desktop/src/styles.css#L548). | Make the header two tracks: `minmax(0,1fr) auto`; place strategy below the name. Move secondary actions into a menu. Stack the actions below at a compact container width. |
| **Medium — visible** | Guide, 1024 | Nine navigation items become narrow vertical stacks: “auto, combos and model names” occupies several lines, and Quick start becomes an oversized tile. [styles.css:856](../../apps/desktop/src/styles.css#L856), [styles.css:870](../../apps/desktop/src/styles.css#L870). | For the horizontal strip, use `grid-auto-columns: max-content` and single-line labels, with deliberate horizontal scrolling; or use a topic selector at this width. |
| **Medium — visible** | Home, 1024 | Five summary cards become four cards plus an isolated Failed card. This makes the last metric look detached and consumes another row. [styles.css:411](../../apps/desktop/src/styles.css#L411). | Choose a deliberate compact arrangement: three plus two, or a two-column metric list with a full-width final item. Avoid an accidental 4+1 layout. |
| **Medium — visible** | Providers, 1024 | The same rate-limit explanation appears in the connection health panel, the auto-skipped reason and the orange model health line. Repetition makes the model row much taller without adding a new decision. [ModelRow:629](../../apps/desktop/src/components/gateway/Connections.tsx#L629). | Show one concise status/reason in the row; put the raw provider response in expandable details. Keep connection-wide and model-specific status distinct. |
| **Medium — visible** | Endpoint, 1024 | The long key name creates a tall client cell, Last used breaks across lines, and spend details compete with actions. [Clients.tsx:40](../../apps/desktop/src/components/gateway/Clients.tsx#L40), [styles.css:536](../../apps/desktop/src/styles.css#L536). | Use a compact key-row/card layout at this width: name and policy above token, usage and actions. Keep timestamps together. Collapse detailed spend accounting behind a disclosure. |
| **Medium — code risk** | Endpoint, key revealed | Token content is nested inside a `td.mono` with generic nowrap, clipping and a 320 px maximum. `TokenCell` has its own reveal/wrapping behavior, but the ancestor still applies the clipping policy. [Clients.tsx:49](../../apps/desktop/src/components/gateway/Clients.tsx#L49), [styles.css:482](../../apps/desktop/src/styles.css#L482). | Remove generic identifier clipping from the token cell. Reveal in a dedicated wrapping block with Copy; verify the complete token remains reachable. |
| **Medium — visible** | Policies, 1024 | The long API-key name dominates one row; “assumed shared” wraps in every connection row. Useful values are being squeezed by prose and the unconstrained name column. [Policies.tsx:33](../../apps/desktop/src/components/gateway/Policies.tsx#L33), [Policies.tsx:63](../../apps/desktop/src/components/gateway/Policies.tsx#L63). | Give the name a defined flexible track, keep compact values together, and show “Shared” with “Assumed” as secondary text or explanatory help. Use a compact policy summary at the minimum width. |
| **Medium — visible / design judgment** | Providers, Usage, Endpoint, 1990 | The shared 1040 px page cap leaves large gutters while data-heavy content remains compressed or stacked. Comparison tables never sit side by side: two 520 px tracks plus their gap cannot fit inside the capped page’s content area. [styles.css:312](../../apps/desktop/src/styles.css#L312), [styles.css:529](../../apps/desktop/src/styles.css#L529). | Add a data-page width variant, approximately 1280–1440 px maximum. Keep Guide prose and Settings narrower. Choose a two-column threshold that can actually be reached. |
| **Medium — code risk** | Usage comparison tables | `.two-columns` forcibly ellipsizes the first cell for **all** tables, including the table whose first cell explains the routing category. Long client names and meaningful route descriptions can silently disappear. [Savings.tsx:154](../../apps/desktop/src/components/gateway/Savings.tsx#L154), [styles.css:533](../../apps/desktop/src/styles.css#L533). | Scope truncation to identifiers. Let category labels wrap; give truncated client identities an accessible full-name view. |
| **Medium — visible / code risk** | Agents, CLI Tools; both widths | Setup rows expose long backup/config paths inline. In the screenshot the backup basename is separated awkwardly; longer unbroken paths have no general wrapping rule on `.agent-text small`. [Agents.tsx:233](../../apps/desktop/src/components/gateway/Agents.tsx#L233), [styles.css:612](../../apps/desktop/src/styles.css#L612). | Keep the default row to tool, state and next action. Put backup path, scope and connection status in Details, with selectable wrapping paths and Copy. |
| **Medium — visible** | Agents, all widths | Repository is a short input occupying a small fraction of the available form width; real Windows paths are difficult to inspect. The folder icon is decorative, so it does not help select a repository. [Agents.tsx:84](../../apps/desktop/src/components/gateway/Agents.tsx#L84). | Make the wrapper full width, the input `flex: 1; min-width: 0`, and add a real Browse action. |
| **Medium — code risk** | Limits sheet, long connection labels | Allowed connections are a single flex row containing checkbox, label and trailing ID. The label has no explicit shrink/wrap contract; the ID adds another intrinsic-width claim. [Clients.tsx:324](../../apps/desktop/src/components/gateway/Clients.tsx#L324), [styles.css:360](../../apps/desktop/src/styles.css#L360). | Use checkbox + `minmax(0,1fr)` content; stack the ID beneath the label and wrap both. Keep checkboxes from shrinking. |
| **Medium — code risk** | Providers → Advanced, both widths | Endpoint and connection namespace values are flex content capped at 60%, but long URLs/IDs lack wrapping or a dedicated scroll/copy treatment. [Connections.tsx:416](../../apps/desktop/src/components/gateway/Connections.tsx#L416), [styles.css:295](../../apps/desktop/src/styles.css#L295). | Render these as full-width value rows beneath their labels, with `overflow-wrap: anywhere` and Copy. |
| **Medium — code risk** | Combo editor, both widths | Members ellipsize IDs without a full-name title or expansion. The native Add model select is also a poor way to distinguish 40 long, similar names. Its empty option says “Connect a provider first” whenever no options remain, including when everything has already been added. [Combos.tsx:234](../../apps/desktop/src/components/gateway/Combos.tsx#L234), [Combos.tsx:256](../../apps/desktop/src/components/gateway/Combos.tsx#L256), [styles.css:565](../../apps/desktop/src/styles.css#L565). | Use a searchable model picker with wrapping names and metadata. Provide full member identity on expansion. Distinguish no providers, no models and all models already added. |
| **Low — visible / code** | CLI Tools and Create API Key flow | “Set Up” and “Set Up…” differ between adjacent rows. The Create API Key sheet still submits with “Create Token” and then titles its result “Token for…”, despite the earlier terminology cleanup. [Clients.tsx:174](../../apps/desktop/src/components/gateway/Clients.tsx#L174), [Clients.tsx:198](../../apps/desktop/src/components/gateway/Clients.tsx#L198). | Use “Set Up…” consistently when opening a setup sheet, and “Create API Key” / “API key for…” throughout the creation flow. |

Two qualifications matter:

- The large blank area **below** short pages is expected in 2200 px-tall captures. It is not itself a layout bug. The wasted **horizontal** space on data-heavy pages is actionable.
- Settings’ narrower column is reasonable for a form. Connect’s input containment, Price’s short numeric fields, and the sheet body’s vertical scrolling are sensible foundations. The unscreenshoted sheets still need a real **640 px-height** pass; their code alone cannot establish that every error, footer and focus target stays usable.

**3. Verdict on the new Models table**

**Keep the idea; change the width allocation.**

Moving test results out of the action cell is correct. `.model-line { overflow-wrap: anywhere }` is also the right containment rule for errors and metadata. At 1990, the supplied Gemini table is usable.

At 1024, the name/routing column is visibly too small. The current solution protects secondary columns at the expense of the information the user is selecting.

I recommend:

- Put **billing under the model name**, using “Unknown” rather than “Unknown billing”.
- Show price as **“Not set”**, **“$0”**, or a compact input/output pair under a clearly labeled `/1M tokens` header. Preserve that unset means unknown cost.
- Keep **Test** visible; move **Edit price, Auto routing and Remove** into a **More** menu. Keep the enabled switch separate.
- Allow the model name to wrap to two lines, with an accessible full-name/copy view. Put the status badge on its own line when necessary.
- Show a concise auto verdict and the latest result. Expand raw errors and detailed reliability information on demand.
- Use an explicit column contract, for example a fixed-layout table with a `colgroup`, or a row grid with `minmax(0,1fr)` for the model. Switch to stacked rows below a container breakpoint.

Do not describe the current first column as “wide”: its `width: 100%; max-width: 0` makes it the **remaining-width column**. The screenshot demonstrates the difference.

**4. Release guard rails**

There is already a new, untracked [ui-overflow-check.mjs](../../scripts/ui-overflow-check.mjs#L1). Keep it, but do not treat its current success as release proof.

Its present gaps are concrete:

- It visits default screens without selecting OpenRouter, pressing Test, opening sheets or expanding failures. See [script:83](../../scripts/ui-overflow-check.mjs#L83).
- It scans only `.content *`; sheets rendered outside that subtree are excluded. See [script:49](../../scripts/ui-overflow-check.mjs#L49).
- It exempts hidden overflow and ellipsis, so it cannot detect important information being silently clipped.
- Its squeezed-cell heuristic only catches cells narrower than 48 px. The current model column can be unreadably narrow while remaining much wider than that.
- Its stated Node 18 requirement is insufficient for the unflagged global `WebSocket` it uses; pin a supported runtime or use the project’s browser-test tooling.

Adopt these shared CSS rules:

```css
/* Shrinkable content tracks */
.content-track {
  min-inline-size: 0;
}

.identifier,
.provider-error,
.path-value {
  min-inline-size: 0;
  overflow-wrap: anywhere;
}

/* Put this inside a td; keep the td a table cell. */
.row-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  justify-content: flex-end;
}
```

Also establish explicit policies: nowrap only for bounded atomic values; ellipsis only with a full-value recovery path; fixed-layout tables only with defined columns; intentional horizontal scrolling only for tables designed to require it. Never fix page overflow with blanket `overflow-x: hidden`.

Before every release, run a state-driven browser check at **1024×640, 1440×900 and 1990×1030** that:

1. Selects the stress OpenRouter connection and produces successful and failing model tests.
2. Tests the long combo, reveals a key, expands request errors and opens every requested sheet.
3. Checks page and component bounds, unexpected horizontal scrolling, clipped controls and minimum readable identity-column widths.
4. Saves screenshots for each state and fails on missing readiness markers or navigation errors.
5. Includes empty/loading/error states, a long unbroken identifier, and enlarged text.

Finally, replay the original failure once in the packaged Windows app. Browser geometry checks are valuable, but the release target is the installed WebView2 application.

The owner’s concern is justified: **screen coverage was mistaken for state coverage**. The stress data and new check are useful progress; the release gate must now reproduce the interaction that broke the installed app.