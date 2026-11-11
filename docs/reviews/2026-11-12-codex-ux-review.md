# UI/UX review by Codex, and what was done, 2026-11-12

> **Status:** Record · a review of the desktop interface and the answer to it · order: [ROADMAP](../ROADMAP.md) · built: [Implementation Status](../IMPLEMENTATION-STATUS.md)

**How it was run.** Codex CLI 0.163.0 (ChatGPT plan) in a read-only sandbox, `codex exec --sandbox read-only --ephemeral`, given 12 screenshots of the browser preview (Vietnamese, dark theme, demo data) and the source in `apps/desktop/src`. It was asked for at most 15 findings about what is wrong for the user, each with evidence and a fix small enough for an hour. It changed no file. Every finding was checked against the code before it was acted on.

| # | Finding (Codex) | Answer |
| --- | --- | --- |
| 1 | The approval button says "Approve up to $X" for agents that cannot stop at a cap | **Done.** Agents that cannot stop at the cap now get "Approve with a $X cap"; the hint above it already says what happens if they go over |
| 2 | Waiting decisions are behind the gateway's setup on start | **Done.** When the app opens with a paid step waiting, it opens Tasks on Needs Approval. The sidebar's Tasks badge now counts the tasks that need you, not every task |
| 3 | First run does not lead to the IDE workflow | **Done.** Get Started says that handing tasks over from the IDE needs no provider, with a link to Agents → From Your IDE |
| 4 | Discard removes the result with no confirmation | **Done.** Discard… asks first, with Cancel focused, and suggests Export Patch to keep a copy |
| 5 | Usage says providers never bill rejected requests | **Done.** It now says a request refused before answering is usually not billed, and one cut off after it started may have been, so its cost shows as unknown or estimated |
| 6 | A green "Ready" before any check overstates | **Done.** Green "Checked" only after a check answered in this session; otherwise a neutral "No problem seen". The "needs attention" filter is unchanged |
| 7 | New Task shows a spinner or the add-workspace form when a load failed | **Done.** A failed load says so, with Try Again |
| 8 | Placeholder text in dark fields is 3.4:1 | **Done.** A `--placeholder` token, 5.2:1 on dark fields |
| 9 | The Usage model table scrolls sideways | **Done** (found here first, before the review). In half-width tables the name column takes what is left and is cut with an ellipsis; the full name is in its tooltip |
| 10 | Usage tables drop rows after eight without saying so | **Done.** "Show all {n}" under a table that has more |
| 11 | English text and mixed terms in the Vietnamese interface | **In part.** Agents' sign-in descriptions are translated; four strings that two locale files translated differently now have one wording ("Tác vụ" in the sidebar, as in "Tác vụ mới"). Reasons written by the task engine (`tasks.rs`), such as "Step 2 is marked paid…", are still English: they are built with values in Rust, and translating them needs message keys, not more strings |
| 12 | Segmented controls have radio roles without arrow keys | **Done.** One Tab stop; arrow keys, Home and End move the choice |
| 13 | Spinners are silent to screen readers; hints are not tied to their fields | **Done.** Spinners are a status that says "Loading…" when unlabeled; a form hint is the field's description |
| 14 | What Keep as Branch does is said only afterwards | **Done.** One sentence under the buttons; the disclosure is "Review Changes" |
| 15 | Policies is a placeholder with a documentation path | **Done.** It points to where those controls are now: Workspaces, the daily budget in Settings, steps in New Task |

## Second round

Codex was then given the commit with these fixes and 7 new screenshots, and asked whether each fix solved its finding, what the fixes made worse, and for at most 5 new findings. It judged 11 solved and 4 partial (#1, #9, #11 as documented, #13 for the reported cases), found two regressions and three new problems:

| Finding (Codex, round 2) | Answer |
| --- | --- |
| #1: "Approve with a cap" still implies the agent stops there; the field is still called a spending cap | **Done.** For an agent that cannot stop at it, the button is "Approve Step" and the field "Stop further steps above" |
| #9, worse: model names cut to prefixes you cannot tell apart | **Done.** The two tables sit side by side only when each has 520 px; otherwise one under the other, with whole names |
| #7, worse: Try Again shows no progress | **Done.** It says "Trying again…" and is disabled while loading |
| New: Tasks can show "Ready for your first task" while loading or after a failed load | **Done.** A spinner until the list loads, or the error with Try Again |
| New: a search can leave a connection that the list no longer shows open below it | **Done.** Only a listed connection can be open; with no match, none is |
| New: a replaced key keeps the old key's check result | **Done.** Saving a key forgets the last check |

**Left for later:** task-engine reasons in Vietnamese (#11), and the native window itself, which this review saw only through the browser preview.
