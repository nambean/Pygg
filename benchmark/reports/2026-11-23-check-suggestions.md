# Checks for thin-test repositories, 2026-11-23

> **Status:** Record · run 2026-11-23 (UTC+7) by the maintainer · ROADMAP focus item 3 · $0 · 20 public GitHub repositories, read only · raw results: [`results/2026-11-23-check-suggestions`](../results/2026-11-23-check-suggestions/) (commits in `commits.txt`)

**In one line:** Pygg now suggests a check for all 20 repositories, up from 10. All 11 that had no test check before get one, and 8 of those get a test-running one. Every suggestion says what it covers. Whether people accept the suggestions was not measured: that needs users (focus item 5).

## What changed

- **Detection beyond package.json scripts and Cargo:**
  - TypeScript's `tsc --noEmit` when no typecheck script exists.
  - `node --test` for test files no script runs.
  - Python `compileall` (parse only) and `pytest` or `unittest`.
  - `go vet` and `go test`.
  - `make test`.
  - `dotnet build` and `dotnet test`.
  - Maven and Gradle `test`.
- **What each check covers:** each check, suggested or approved, shows one line on what passing it means and what it leaves unchecked (`checks::covers`). A parse check is called a smoke check, so a pass never reads as "correct".
- **Check This Result:** for a result no check ran on, the app lists the workspace's checks with that line. The user approves one, and **Check This Result** runs the approved checks on the result as it is, without running the agent again. The same works from the task API (`POST /pygg/v1/tasks/{id}/check`) and MCP (`pygg_check_result`). Agents cannot add a check, only run the approved ones. If every check passes, the result is ready; if one fails, the result stays not ready and shows the failure.

## Method

1. Shallow-clone 20 small public repositories in five languages, chosen from `gh search repos` (size 50–800 kB, 30–400 stars, pushed in 2026).
2. Run `detect_proof` (checks.rs), which reads files only.
3. Run [`scripts/proof-check-suggestions.py`](../../scripts/proof-check-suggestions.py) on the result. It compares the suggestions with the rules before item 3. Where `python -m compileall` was suggested, it runs that check on a copy twice: as cloned, and with a syntax error added to one file. Nothing else ran: no install, test or build, so no repository code executed.

## Results

| Measure | Value |
| --- | --- |
| Repositories with a suggested check | 10 → **20** of 20 |
| No test check before item 3 ("thin") | 11 |
| Thin ones with a check now | **11 / 11** |
| Thin ones with a test-running check now | 8 / 11 (the other 3 get only a parse or build check) |
| Suggestions that name what they cover | 20 / 20 repositories |
| `compileall` caught the seeded syntax error | 7 / 7 |
| `compileall` passed on the code as cloned | 6 / 7 (`genomics_general` still has Python 2 files) |

| Repository | Before | After |
| --- | --- | --- |
| Clever/microplane | — | go vet, go test, make test |
| Kentzo/git-archive-all | — | compileall, pytest, make test |
| apenwarr/git-subtrac | — | go vet, go test |
| benc-uk/workflow-dispatch | install, lint, build | + tsc --noEmit |
| fdemmer/django-weasyprint | — | compileall, pytest |
| jongracecox/anybadge | — | compileall, pytest |
| mdlayher/vsock | — | go vet, go test |
| openstack/bashate | — | compileall, unittest |
| simonhmartin/genomics_general | — | compileall |
| slashmili/jdatetime | — | compileall, pytest |
| witt3rd/oh-my-hermes | — | compileall |
| 9 others | package.json scripts or Cargo | 4 gain `tsc --noEmit`; the rest unchanged |

- **A check can fail on untouched code.** `compileall` fails on `genomics_general` as cloned, so every result there would fail it. Running a suggested check on the base before approving it is the next step.
- **Not measured:** whether people accept a suggestion (ROADMAP target 12 of 20), or say what it covers in their own words (18 of 20). Both need the paired evaluation with users (focus item 5).
