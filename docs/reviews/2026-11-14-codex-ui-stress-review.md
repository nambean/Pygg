# UI review with real-shaped data, 2026-11-14 (afternoon)

**Trigger.** The owner's screenshot of the installed app: Providers → OpenRouter with many `:free` models, after
pressing Test on a few. The Models table had grown wider than the page. Price and Auto routing were one word per line,
and the row buttons were cut off at the right edge. The owner asked why such obvious UI bugs still ship, and asked for
every screen to be reviewed again with Codex.

Codex's full answer: [2026-11-14-codex-ui-stress-review-raw.md](2026-11-14-codex-ui-stress-review-raw.md). It worked from
the source and 30 screenshots: every screen at 1024 and 1990 px with stress data, and at 1440 px with demo data.

## Why it shipped

- **The CSS let text set the table's width.** Tables used automatic layout. `.narrow` cells were
  `width: 1%; white-space: nowrap`, a promise to keep their content on one line however long. The test result was a
  `<small>` inside the nowrap actions cell (`ModelTestButton`). The provider's error message became one unbreakable
  line, and the browser took the width from the Price and Auto routing columns.
- **Reviews looked at screens, not states.** The earlier review (morning, the same day) used the demo data: short model ids, a
  handful of models, nothing tested. Nobody chose the 40-model connection or pressed Test, so the state that broke was
  never on screen. In Codex's words, screen coverage was mistaken for state coverage.

## What changed

| Area | Before | Now |
| --- | --- | --- |
| Preview data | Short ids, few models | `?stress`: 36 more OpenRouter `:free` models with long ids, a long connection label, key name and combo name, provider errors of 150 characters, every other test failing (`lib/demo.ts`) |
| Models table | Auto layout, nowrap columns, the test result in the actions cell | Fixed columns (`colgroup`): Model takes the width left; price 116 px, switch 74 px, actions 128 px. Billing, routing facts, the auto verdict, reliability, health and the last test go on lines under the model id, which wraps. **Test** stays a button; Edit Price, Auto Routing Facts and Remove are in a **More** menu (`MenuButton` in `ui.tsx`: fixed position, Escape and arrow keys, focus returns). The model's health reason is said once. |
| Combos | Name, summary, four buttons and the test result in one flex row; chips of fixed height | Name and summary in a shrinking column, buttons beside or under it (container query). The test result is on its own line, and chips wrap a long model id |
| Usage → request details | `max-content 1fr` grid | `max-content minmax(0, 1fr)`, values wrap |
| Sheets | Title cut with an ellipsis | The title wraps whole and can be selected (it names the model, key or combo) |
| Home | Five tiles as 4 + 1 at 1024 px | Five across, or 3 + 2 |
| Guide at 1024 px | Topics as stacked three-line buttons | One row of single-line tabs that scrolls sideways |
| Data pages | 1040 px wide at any window | Providers, Usage and Endpoint up to 1440 px, so the side-by-side tables appear on wide windows |
| Two-table rows | Every first cell ellipsized | Only identifiers are cut; labels wrap |
| Endpoint | Token cell clipped as an id; "Last used" broke | A revealed key wraps; "Last used" stays on one line; long key names wrap |
| Limits sheet | Label and id in one row | Label above, id below, both wrap; checkboxes don't shrink |
| Agents, Advanced | Long paths and URLs | Wrap |
| Combo editor | Members cut, "Connect a provider first" with connections present | Members wrap; says when no model is turned on |
| Policies | "assumed shared" wrapped in every row | **Shared** with *assumed* as a note |
| Wording | Create Token, Token for… | Create API Key, API Key for…, Copy API Key |
| Usage token mix | Long model ids kept on one line | They wrap in their column, up to 300 px wide |

Not changed, on purpose: **Set Up** (acts at once) and **Set Up…** (opens a sheet) follow the convention that an
ellipsis means more input comes next.

## The guard rail: `scripts/ui-overflow-check.mjs` (`pnpm ui-check`, and in CI)

Headless Edge or Chrome opens the preview with `?stress` at **1024 × 640, 1440 × 900 and 1990 × 1030**. It visits every
screen as it opens, plus ten states that broke or could break:

- OpenRouter with eight models tested;
- a row's menu;
- the Edit Price, Connect, Create API Key, Limits and combo editor sheets;
- combos tested;
- keys shown;
- a failed request expanded.

Each state must actually be reached (a selector it expects), or the check fails. It reports:

- anything past the window's edge (outside a box that scrolls sideways on purpose);
- anything cut off by a box that clips;
- content wider than its box;
- cells squeezed to a word per line;
- a model column under 260 px.

Before anything else it plants the original bug, the old test result in the Models table, and stops with exit code 2
if that bug is not found.

On its first run it found a real overflow that the screenshots had missed: the Usage token-mix table, 14 px past the
window at 1024 px. With the changes above, all 60 runs pass (20 screens and states at each of 3 sizes).

## Still to do

- Replay the original failure once in the installed app (WebView2), not only in the browser preview.
- ~~Enlarged text~~ (done the same evening: Windows' Text size zooms web content, so the 1024 px window lays out like
  819 and 683 px at 125 and 150 %; the check runs those two sizes too. Below 900 px the sidebar becomes a rail of icons,
  and a provider row cuts its label, never its routing prefix.)
- The check does not cover empty and error states in every sheet, or the
  Tasks screens (on hold).
- A searchable model picker in the combo editor (a native select of 40 similar ids is hard to use).
- A Narrator pass (from the morning's list).
