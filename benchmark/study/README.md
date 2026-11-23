# Paired study on users' own work

> **Status:** Plan · ROADMAP focus item 5 (and the acceptance half of item 3) · not run yet

**Question:** on real tasks in people's own repositories, does working through Pygg keep as many outcomes acceptable as their usual way, and what does it change in time, cost and defects?

**Decision rule:** at least 30 paired tasks from at least 10 people. Pygg passes if its share of accepted outcomes is no more than 5 points below the usual way, as a point estimate. The 95% interval is reported beside it, whatever it shows.

## Who takes part

| | |
| --- | --- |
| People | 10 or more developers who use a coding agent (Claude Code, Codex, OpenCode) on their own repositories |
| Machine | Windows with Pygg installed (desktop app or `@pygg/cli`); macOS or Linux with the CLI |
| Time | About 3 hours each: 6 tasks, a short review, sending the export |
| Compensation | None in money (Pygg is MIT and free). Each person who finishes is credited in the README's Sponsors section, by name and a link of their choice, if they want |

## Arms

| Arm | What the person does |
| --- | --- |
| **U**, usual | Their usual agent and plan, without Pygg |
| **P**, Pygg | The same agent through Pygg (`pygg run <tool>`, `auto` with a fallback set up), handing well-defined subtasks to Smart Tasks where they want to |

## Steps

1. **Pick tasks.** The person picks 6 real tasks from their own backlog, as 3 pairs of like size and kind: two bug fixes of one module, two small features. A pair is written down before either is started (`log-template.csv`, columns `pair` and `task`).
2. **Assign.** `python -I scripts/study-assign.py --person ID --pairs 3` says which task of each pair goes to U and which to P, from a seed the person cannot pick.
3. **Work.** Each task is worked to the point where the person would merge it or gives up. For U, they note the start, end, tool, plan or cost and outcome in the log. For P, Pygg records it, and the person notes the start, end and outcome.
4. **Export.** `python -I scripts/study-export.py <store> --since <date> --out pygg-export.json` writes Pygg's figures for the study window: costs, times, attempts, checks (suggested, approved, ran) and kept-unchecked results. It writes no prompt, title, path, branch, file name or code. The person reads the file before sending it.
5. **Blind review.** For each pair, the person saves both diffs (`git diff base...branch > pair1-U.diff`). `python -I scripts/study-blind.py <folder>` renames them X and Y at random and writes a review sheet; the key stays in a separate file. A second reviewer from the same team is better where code may be shared with them. The reviewer marks each diff: merge as is / merge after a small fix (≤ 10 lines) / reject, and the defects they find.
6. **A week later**, the person adds defects found since in either arm's merged code.
7. **Send** the log, the review sheet, the key and the export. Diffs and code stay with the person.

## What is measured

| Measure | From |
| --- | --- |
| Accepted outcome (merge as is, or after a small fix) | Review sheet |
| Defects found in review, and in the week after | Review sheet, log |
| Elapsed time, start to the decision | Log |
| Money: known, estimated, unknown; plan use counted apart | Export (P), log (U) |
| Fallbacks during the session, and whether the session went on | Export (P) |
| Checks: suggested, approved, ran; results kept unchecked | Export (P); item 3's acceptance |

## Analysis

`python -I scripts/study-analyze.py <folder of returns>` gives:

- the accepted share in each arm, and the paired difference P − U with a 95% bootstrap interval resampled by person;
- the median time and cost per arm, defects per task, and checks accepted against suggested.

Every task counts, given up ones included (not accepted). Nothing is dropped after the fact. A task that went wrong for a reason outside both arms, such as an outage, is reported, not removed.

## Privacy

Code, prompts and diffs never leave the person's machine unless they share them with their own second reviewer. The study keeps only the log, the review sheet, the key and the export. Each person gets an ID (P01…). Results are published as counts and ranges, with no repository named.

## Files

| File | Use |
| --- | --- |
| [`log-template.csv`](log-template.csv) | The person's log, one row per task |
| [`recruit.md`](recruit.md) | The invitation |
| `scripts/study-assign.py`, `study-export.py`, `study-blind.py`, `study-analyze.py` | The steps above |
