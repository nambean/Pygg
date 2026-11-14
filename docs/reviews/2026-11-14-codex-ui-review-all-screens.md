# Codex's review of every screen before the open-source release, 2026-11-14

> **Status:** Record · Codex's findings in short, and what was done · earlier rounds: [first](2026-11-12-codex-ux-review.md), [second](2026-11-12-codex-review-round2.md), [third](2026-11-13-codex-screens-savings-npm-review.md), [fourth](2026-11-14-codex-providers-web-guard-review.md), [fifth](2026-11-14-codex-token-optimization-discussion.md) · design: [SCREENS](../design/SCREENS.md)

**How it was run.** Codex CLI 0.163.0, `codex exec --sandbox read-only --ephemeral`, given 17 screenshots of the browser preview (every gateway screen, Guide, Settings and Agents; dark, light, first run, 1024 wide) and the source of every screen, sheet and state. Tasks and Workspaces (on hold) were out of scope. No request was sent and no money spent.

**Its verdict.** A clear direction and a usable foundation; one focused pass, not a redesign. The remaining problems are misleading states, links that do not open what they name, and inconsistent wording about cost protection.

## Done (its ten first changes, and the quick wording fixes)

| # | Finding | Done |
| --- | --- | --- |
| 1 | **Blocker:** testing a model without a price, or a combo, promised "a fraction of a cent" | Model: "may incur a small charge at its price", or, with no price, "This may incur a charge. Pygg cannot estimate it because this model has no recorded price." Combo: "A paid model may answer, and fallback may try more than one model." |
| 2 | Dark-mode destructive buttons: white on `#FF6961` is 2.8:1 | `--destructive-fill` (`#C2001B` light, `#B4232B` dark, about 6.4:1 and 6.5:1); the lighter red stays for text and icons |
| 3 | Policies showed the strict-limit control three times, and said the limit "is never passed" | One **Limit mode** row: "Strict reserves the most a request could cost before sending it… Provider price changes or unreported charges can make the bill differ either way"; the example in a disclosure |
| 4 | "How auto chooses" opened the Combos tab | Opens the simulator tab from Home and CLI Tools; the sidebar still opens Combos |
| 5 | A failed tool only mentioned Usage; "See it in Usage" opened the default view | **View Failed Requests** under a tool's failed status opens Requests filtered to that tool's failures; Home's link opens Requests filtered to the client; a drill-down clears unrelated filters |
| 6 | Show More could run twice, append into another period, list a row twice | One page at a time ("Loading older attempts…"), an answer for another period is dropped, rows are de-duplicated |
| 7 | "Nothing to show" when only the loaded attempts were searched | "No matches among the {n} attempts loaded. Show More loads older ones." |
| 8 | Billing, independent quota, enabling and removing a model failed silently | Each says "Couldn't save … / Not saved: …"; the control keeps the saved value |
| 9 | Policies: a failed load looked like "No connections yet"; a refresh overwrote edited defaults | Loading, error with Try Again, empty and populated states; the defaults draft is kept once edited |
| 10 | Usage Summary too long; "How Pygg Saved It" overstated cause | Repeated Tool Output is a closed disclosure with its share in the line; the section is **Comparison Breakdown** |
| — | Wording | "Create API Key" everywhere (no "client token"); "Clients" → Endpoint → API Keys; failed attempts say **Attempted model**; Agents opens with "Experimental agent execution. To route your coding tool's model requests through Pygg, use CLI Tools." and a Check Again on failure; API-key sign-ins "may incur provider charges"; the Guide says "Working" appears after an answer and how to fix "no eligible model"; the post-connect banner points to CLI Tools' check; the unknown-cost alert names subscriptions too; Routing says when it shows 50 of more changes |

Checked in the browser preview: the links, the filters they set, one Limit mode row, the destructive token in dark, the collapsed observation, no console errors.

## Later (in order)

1. ~~Readiness by tool and format~~ (done 2026-11-14: Home and CLI Tools check Claude Code with its Anthropic format and
   adaptive thinking, `CLAUDE_CODE_NEEDS`; Agents shows `pygg run codex` / `pygg run opencode` as copyable blocks).
2. ~~Keyboard and screen reader basics~~ (done 2026-11-14): the provider picker has one Tab stop, the arrow keys, Home and
   End move through every provider and Enter picks one (not a radio group: picking opens the provider's form, so moving
   must not pick); every hint is the description of its control (`FormRow` without a field id becomes a named group;
   settings rows through `useRowHints`); after page navigation focus goes to the page heading unless the page focused
   something itself; **Skip to content** is the first Tab stop; the usage chart is one Tab stop with arrow keys.
3. ~~Query-level filters for Requests~~ (done 2026-11-14: `store::usage_requests_matching` filters by client, model and
   problems or unknown cost over the whole period, with the match count; the client choices come from the period's totals
   and "No client" no longer means "every client"). Paging is still by offset, not a cursor.
4. ~~Provider rows at 1024 × 640~~ (done 2026-11-14: in a list narrower than 760 px the health dots and the status go to a
   second line, a container query on `.select-list`).
5. ~~Endpoint: the effective policy when creating a key; reveal with progress~~ (done 2026-11-14: Create API Key shows
   what the key gets from Policies → Defaults, and the limit field shows the default limit instead of "No limit"; Show
   says Reading… while the Credential Manager answers).
6. ~~Settings: About~~ (done 2026-11-14: version, licence, source, report an issue, third-party licences, where the log and backups are). Retention and diagnostics progress states: done 2026-11-14 (the period choice is held and says it is counting or
   deleting; Save Diagnostics says Saving…; the bundle list names the log tail).
7. A Narrator and native Windows pass before calling accessibility done.
