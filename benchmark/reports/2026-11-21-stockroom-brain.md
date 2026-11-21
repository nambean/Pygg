# A real website, three ways: a strong model, a cheap model, and the cheap model through Brain Storming, 2026-11-21

> **Status:** Record · sponsored: the traffic was paid with API credit from [MixAICloud](https://us.mixaicloud.com), the project's first sponsor · run 2026-11-20 23:52 to 2026-11-21 12:35 (UTC+7) by the maintainer, at the owner's request: "can cheap models with Pygg's Brain Storming do as well as an expensive one, on a real project?" · reviewed blind by Claude Opus 5.5 and Codex.

**In one line:** building the same inventory and order website from a one-page specification, **Qwen 3.6 Flash alone never produced a server that starts (0 of 37 hidden API checks, three runs); through Brain Storming, with DeepSeek V4 Pro planning and taking over the cards Qwen failed, it built one that passes 37 of 37 and 20 of 21 hidden UI checks, as good as DeepSeek V4 Pro alone on the tests (36–37 and 17–20)**; of two blind reviewers, Codex scored it level with the strong model (7 and 7) and Opus below it (5 against 8), for broken editing and search in its pages. It cost **$5.78 and about two hours against $0.20–0.41 and 8–13 minutes** for DeepSeek V4 Pro alone: through MixAICloud the cheap model reads nothing from the prompt cache, and half its cards needed the strong model anyway.

## What was built

`Stockroom` ([SPEC.md](../results/2026-11-21-stockroom/SPEC.md)): sign-in, products with search, pages of 20 and CSV export, customers, orders whose creation takes stock and whose cancelling puts it back, allowed status changes only, refused deletes, a dashboard, phone layout, labelled fields, a JSON API with fixed status codes, and its own tests. Node.js with `node:sqlite`, from an empty repository.

| Arm | How | Runs |
| --- | --- | --- |
| **Strong** | DeepSeek V4 Pro alone, one OpenCode session with the whole specification | 3 |
| **Cheap** | Qwen 3.6 Flash alone, the same way | 3 |
| **Cheap + Brain** | the specification sent to `pygg/brain`: DeepSeek V4 Pro plans the board (19 cards) and does the one plan card; every code, test and review card runs on Qwen 3.6 Flash first and moves to DeepSeek V4 Pro only when the card's check fails (Pygg's default ladder); the board runs the cards one after another on one branch | 1 |

All through one Pygg gateway (the desktop app in development mode on a copy of the sponsor store), OpenCode 1.18.4, list prices from MixAICloud (DeepSeek V4 Pro $1.32 / $3.96 per million tokens, cached input $0.044; Qwen 3.6 Flash $0.20 / $1.20, cached $0.02).

## Results

**Hidden acceptance tests** ([acceptance/](../results/2026-11-21-stockroom/acceptance)): 37 API checks and 21 browser checks in headless Edge, written from the specification, checked against a reference implementation (37/37 and 21/21), and kept out of every agent's reach until the runs ended.

| Run | API checks | UI checks | Money | Time |
| --- | --- | --- | --- | --- |
| Strong, run 1 | 36/37 | 20/21 | $0.20 | 8 min |
| Strong, run 2 | **37/37** | 19/21 | $0.41 | 13 min |
| Strong, run 3 | **37/37** | 17/21 | $0.20 | 9 min |
| Cheap, run 1 | 0/37 | 2/21 * | $0.00 † | 1 min |
| Cheap, run 2 | 0/37 | 2/21 * | $0.19 | 11 min |
| Cheap, run 3 | 0/37 | 2/21 * | $0.06 | 5 min |
| **Cheap + Brain** | **37/37** | **20/21** | **$5.78** | about 2 h of card time |

\* Two UI checks pass on a page that never loads (no unlabelled field, nothing to scroll sideways). † Qwen ended its turn after reading the directory, before its first priced request finished.

How the cheap model failed alone: run 1 stopped after "Let me check what Node version is available" and wrote only `package.json`; run 2 wrote its Windows paths unescaped, so its files went into folders literally named `C:Users…` and the server's entry file was never found; run 3 wrote only `package.json` again.

**Blind review.** Each reviewer got the specification, the first run of each arm as handed in (no Git history, no `node_modules`; first runs were chosen before any Brain result existed) and its screenshots, as Project X, Y and Z, and scored eight criteria from 1 to 10 with evidence ([prompt](../results/2026-11-21-stockroom/reviews/REVIEW-PROMPT.md)). X was the strong model, Y the cheap one, Z the cheap one through Brain.

| Criterion | Codex: strong | Codex: cheap | Codex: Brain | Opus: strong | Opus: cheap | Opus: Brain |
| --- | --- | --- | --- | --- | --- | --- |
| Spec coverage | 9 | 1 | 7 | 9 | 1 | 6 |
| Correctness | 7 | 1 | 7 | 8 | 1 | 7 |
| Code quality | 6 | 1 | 7 | 6 | 1 | 6 |
| Security | 4 | 1 | 7 | 6 | 1 | 8 |
| Tests | 7 | 1 | 8 | 7 | 1 | 7 |
| UI and UX | 7 | 1 | 5 | 7 | 1 | 4 |
| Accessibility | 6 | 1 | 6 | 6 | 1 | 6 |
| Documentation | 8 | 1 | 8 | 8 | 1 | 5 |
| **Overall** | **7** | **1** | **7** | **8** | **1** | **5** |

Both reviewers ranked the strong model first and the cheap model last. They disagree on the Brain project: Codex scored it level with the strong model (7 and 7), Opus three points below (5 against 8). Both found its strengths in the code (separated modules, money in integer cents, transactional cancelling, server-side sessions that sign-out ends, about 100 API tests) and the same defects in the pages: the Edit buttons for products and customers do nothing, and, by Opus and the hidden UI test, the page search has no effect; Opus also found most of its table styles dead (selectors that never match), which is why its tables look bare. Both found the strong model's sessions weaker: a hard-coded fallback secret, and sign-out that cannot end a copied cookie (the hidden API test caught the latter on run 1). Full reviews: [Codex](../results/2026-11-21-stockroom/reviews/codex.md), [Opus](../results/2026-11-21-stockroom/reviews/opus.md).

## The Brain run, card by card

DeepSeek V4 Pro planned 19 cards in 16 waves, code first with each card's tests beside it ([proposal](../results/2026-11-21-stockroom/brain/proposal.md)). The board ran them on one branch ([log](../results/2026-11-21-stockroom/brain/board-B3.log), [after the pause](../results/2026-11-21-stockroom/brain/board-B3b.log)).

Of the 18 build cards (all but the plan card), **9 passed on Qwen 3.6 Flash alone and 9 needed DeepSeek V4 Pro** after Qwen failed the card's check.

| Model | Requests | Prompt tokens | Read from cache | Money |
| --- | --- | --- | --- | --- |
| Qwen 3.6 Flash, every attempt | 426 | 18.7 M | 0% | $4.19 |
| DeepSeek V4 Pro, the plan card and the takeovers | 156 | 6.9 M | 93% | $1.40 |
| DeepSeek V4 Pro as planner, three calls (twice its 16,000-token limit spent thinking, then the board) | 3 | — | — | $0.19 |

- **Brain made the cheap model useful.** Small cards with a check after each kept Qwen from the failures it made alone: half the build cards passed on it, and the check stopped every broken card before the next one built on it.
- **It did not make it cheap.** Qwen's prompts were never read from cache through MixAICloud, so every turn paid the full input price; three of its attempts ran to the 15-minute limit. A card Qwen failed cost more than DeepSeek doing it (card 8: $0.53 failed, then $0.12 passed). DeepSeek V4 Pro alone, reading 98–99% of its prompts from cache, built the whole site for less than one of Qwen's long cards.
- **Two cards' work was stranded.** The board's limit of one running card does not count cards waiting for a paid step's approval, so four cards were pulled and, once approved, ran at once from the same tip. Cards 13 (edit and delete pages) and 15 (orders page) then conflicted with the branch and were kept on branches of their own; later cards covered the orders page, but product and customer editing stayed unwired. That is the Brain project's main defect in both reviews.
- **The spending limit paused it.** At card 12 the run reached the test's $5 strict limit; it went on after the owner approved more, through the new Run again action.

## What this says about Pygg

- **On quality, mostly:** with Brain Storming, a model that could not build this site alone built one that passes every hidden API check, with better-structured and better-tested code than the strong model's by both reviewers; its pages were weaker (editing and search unwired), so one reviewer rated it level with the strong model and the other three points below.
- **On cost, not here:** a strong model with a warm prompt cache is cheaper per finished task than a cheap one without it, and a one-page website fits in one session of a strong model. Pygg should pick models by measured cost per kept result, not price per token; `auto/cheap` does since 2026-11-21 (expected cost from each model's own output and cache share), and the role order's own-evidence layer does once a model has ten runs of a role.
- **Where Brain should pay off:** projects larger than one session, cheap or local models that cache or cost nothing, and teams who want a check after every step. None of those was measured here.

## What the run changed in Pygg

Each found by this run, fixed with tests, and pushed before the run it affected:

| Commit | Change |
| --- | --- |
| f432017 | A brainstorm's cards build on one branch: each card starts from its tip, Keep adds the card to it (cherry-picked if the branch moved). Before, card 2 never saw card 1's code. |
| fe97c74 | The planner may think (16,000 tokens, then 32,000 in b8f1a8d); cards take the model saved per role on Brain Storming. |
| ae3842c | `brain-v2`: the planner is told every card must leave the checks passing. A test-first plan (eleven test cards before any code) would have jammed the board at the first red test. |
| c117b5e | Run again on a blocked card whose task failed; before, a failed card was a dead end. |

Still open: the board's limit should count cards waiting for approval, and a step refused by the spending limit should read "over the limit", not "every step failed".

## What went wrong on the way

- **Two earlier Brain runs are not counted.** In the first, fallbacks were removed to test the cheap model alone: Qwen failed card 2 (a test script pointing at a folder it never made) and, with no way to run it again, the board stopped at card 2 of 19. The second failed on my setup: the check `npm test --if-present` fails without a `package.json`, and the OpenCode configuration did not list the fallback models. The counted run used the check `if exist package.json (npm test --if-present) else (exit /b 0)` and every model.
- **The hidden tests were reachable at first.** They sat in a parent folder of the workspaces. A first strong run that wrongly worked in that parent folder listed it and was discarded; the counted strong and cheap runs show no reads of it in their logs; card 2 of the first, uncounted Brain run cannot be cleared (only its log's tail is kept). The tests were moved out of reach before the counted Brain run.
- **The user's part was played by a script:** it approved each paid step with a $1 cap and kept each result that reached Review, through the app's own commands, as the buttons do.

## What this does not show

- **One project, one Brain run.** The strong and cheap arms ran three times each; the Brain arm once, because of the cost. A different plan could do better or worse.
- **One provider.** Through another provider Qwen may cache, and the cost comparison would change.
- **Card time is not a race.** The Brain run's two hours include waiting for approvals and checks; a user approving by hand would add more.
- **Costs are list prices from Pygg's ledger**, not MixAICloud's bill.

## Reproducing

The specification, the hidden suites, the reference implementation, every project as handed in, the scores, the screenshots of the reviewed runs, the plan, the board logs and both reviews are in [`benchmark/results/2026-11-21-stockroom/`](../results/2026-11-21-stockroom), with a `SHA256SUMS` file. Run a suite with `node acceptance/api.mjs <project> <out.json>` and `node acceptance/ui.mjs <project> <out dir>` (Microsoft Edge needed for the second). Local paths in the results are masked.
