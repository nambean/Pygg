# Pygg — The brain, the facts feed and the board

> **Status:** Current · the model facts feed (ADR-040), the role aliases, `pygg/brain` (ADR-041), the board and the Brain Storming screen (ADR-042), as designed and built · reviewed 2026-11-20 · how each part was verified: [Implementation Status](../IMPLEMENTATION-STATUS.md)

A requirement typed into the user's own coding agent becomes a board of cards, each on a model picked by rule from measured facts, run one at a time under the user's approval.

| Part | ADR | What |
| --- | --- | --- |
| 1. The model facts feed | ADR-040 | Per-role scores with their sources, published by the project, fetched by the app |
| 2. `pygg/brain` and the role aliases | ADR-041 | A virtual model that turns a requirement into a proposal; `auto/plan`, `auto/code`, `auto/test`, `auto/review` |
| 3. The board | ADR-042 | The task engine's states as Kanban columns with WIP limits |

Each part works alone: the feed ranks the role aliases, and the role aliases can be benchmarked with the existing harness.

## 1. The model facts feed

### What a fact is

| Field | Meaning |
| --- | --- |
| `ids` | Canonical ids: the tail of a provider id after a router's vendor prefix, lower-cased, `.`, `_` and spaces as `-`, a `:free`-style variant, `-latest` and a trailing date dropped (`openrouter/anthropic/claude-opus-4.5` and `claude-opus-4-5-20251101` both give `claude-opus-4-5`). One rule, `canonical`, in `scripts/model-facts.py` and `gateway/facts.rs` |
| `name` | The name as the source writes it |
| `role` | `plan`, `code`, `test`, `review`, `image` |
| `score`, `unit` | The source's number, two decimals |
| `place`, `of` | Place in the source (1 is best; ties share a place: one more than the rows with a strictly better score); the source's row count |
| `rank` | `(of − place) / (of − 1)`: 1 best, 0 worst within the source (1 when `of` is 1). Comparable across sources; `score` is not |
| `agent` | Terminal-Bench and SWE-bench only: the agent the row ran with (`claude-code`, `codex`, `mini-swe-agent`, …) |
| `effort` | The reasoning effort, when the source states it (`high`, `max`, …) |
| `licence` | The row's licence (`Apache-2.0`, `CC-BY-4.0`, `CC-BY-NC-4.0`) |
| `derived` | `true` when the role is not what the source measured (every `plan` and `review` fact; `test` until a source measures it) |
| `source` | Id in the feed's source list: name, URL, licence, attribution, what it measures and does not, status (`ok`, or `stale` when carried forward), date read, row count |
| `url` | Where the row was read, when the source gives one per row |
| `date` | The row's date (`YYYY-MM-DD`), read by the twelve-month rule |

No source, no row. A model the feed does not name keeps its tier (ADR-006).

### Sources, per role

Terms read on 2026-11-19. Which sources the job reads is the owner's decision.

| Role | Source | Data | Terms | Model names | In the feed |
| --- | --- | --- | --- | --- | --- |
| `code` | Terminal-Bench 4.x | `harbor-framework/terminal-bench`, `leaderboard/submissions/*.json` (73 rows): agent, model, effort, date, accuracy with 95% CI half-width, trials, tokens, USD cost | Repository Apache-2.0; no separate row terms. Maintainers run all submissions | Provider id in `source_filter.model_name` (`anthropic/claude-sonnet-5`) | **Yes**, the best source (CI, cost, provider ids). Scores an agent and model together; prefer rows with Pygg's agents (Claude Code, Codex) |
| `code` | SWE-bench Verified | `SWE-bench/swe-bench.github.io`, `data/leaderboards.json` (4 MB; Verified 180 rows): name, agent, model and organisation, release date, effort, resolved %, date, cost | **CC BY-NC 4.0**. Academic submissions only since 2025-11-18 | Display names (`Claude 4.5 Opus (high)`), mapped by hand | **Yes** (owner: Pygg is always open source, takes no money). Rows carry `licence: CC-BY-NC-4.0`, named in the source list and THIRD_PARTY_NOTICES, so a commercial redistributor knows to drop them |
| `code` | Aider polyglot | `Aider-AI/aider`, `aider/website/_data/polyglot_leaderboard.yml` (69 rows): model, pass rate, edit format, date, cost, command | Apache-2.0 | `command` carries `provider/model`; no mapping | **Shown only**: newest row 2025-10-03, over a year old, so it does not rank until the leaderboard moves |
| `plan`, `review` (derived) | LMArena, text with style control | Hugging Face `lmarena-ai/leaderboard-dataset`, Parquet, `latest` split (2026-11-18): rating with bounds, votes, rank per category (`hard_prompts`, `coding`, `instruction_following`, `expert`, …) | Dataset **CC BY 4.0**. arena.ai site terms (2026-02-23) forbid scraping, so the job reads the dataset, never the site | Display names (`Claude Opus 5.5 (High)`), mapped by hand | **Yes**: `hard_prompts` ranks `plan` and `review`. `coding` is preference, not tests: not used for `code` |
| `plan`, `review` (derived) | LiveBench | `LiveBench/livebench.github.io`, `public/table_<date>.csv` (28 models on 2026-06-25); categories in `categories_<date>.json` | Site repository has no licence (Apache-2.0 and MIT cover the benchmark's code) | Config names (`claude-opus-4-6-thinking-auto-high-effort`) | **No** (owner): unlicensed data; LMArena covers the roles |
| — | Artificial Analysis | Free API, `GET /api/v2/language/models` | Data Platform Terms v1.1 (2026-08-19): no raw data in a customer-facing product, no combining with other sources into a product; "model/provider selection guidance" is named a competitive product | — | **No**: Pygg is what the terms exclude |
| — | OpenRouter rankings | Datasets API, `programming`, weekly token totals; needs an OpenRouter key | CC BY 4.0, citation "Source: OpenRouter (openrouter.ai/rankings), as of <date>" | Permaslugs | **Not in v1**: usage is adoption, not quality. Later perhaps a "most used" line, never a rank |
| `image` | LMArena text-to-image | Same dataset, `text_to_image` subset | As above | As above | Stored, not routable |

Rules for every source:

- **Older than twelve months: shown, never ranked** (366 days, `facts.rs`). The line stays with its date.
- **Agent and model together** (Terminal-Bench, SWE-bench): the feed keeps the agent; the app prefers the row whose agent Pygg would run, else the best row, and says which.
- **Nothing averaged across sources.** Each source is its own line; the rule uses the role's best rank and marks its line.
- **Effort is identity.** `(high)`, `(max)` and `-thinking` rows stay apart, each mapped to the model id plus the effort the gateway would send.
- **Each fact carries its source's licence**; the `sources` list records each source's terms and the date read.

### The feed file

```text
model-facts/
  v1/
    facts.json        the feed: {version, built, sources: [...], names: {...}, facts: [...]}
    facts.json.sig    its signature (ed25519)
```

- **Names map** `model-facts/names.json`: kept by hand, reviewed in a pull request. `{"<source>": {"<name as the source writes it>": {"ids": ["claude-opus-4-5"], "effort": "high"}}}`. Terminal-Bench and Aider ids are made canonical by the builder; SWE-bench and LMArena names go through the map. An unknown name is logged and left out, never guessed (`python scripts/model-facts.py names` lists them with a proposed id). System names (fine-tunes, sampling schemes, agents) are left out.
- **Matching:** `canonical(model_id)` is one of the fact's `ids`. The connection prefix plays no part: one fact serves a model on every connection.
- **Build:** `scripts/model-facts.py` in a scheduled workflow, weekly and on demand. A source that fails keeps its facts from the last published feed (`observed` unchanged, marked `stale`); the build fails only when every source fails. The app never reads the 4 MB SWE-bench file.
- **Signing:** ed25519. The 32-byte seed, base64, is the repository secret `MODEL_FACTS_SIGNING_KEY` (GitHub Actions only); the public key is `KEYS` in `gateway/facts.rs`, key id `mf-2026-1`. `facts.json.sig` is one line, `<key id>:<base64 signature>`, over the exact bytes. Refused: a feed that does not verify, or whose `built` is not later than the kept one. Without the secret the workflow keeps the feed as a run artifact and publishes nothing.
- **Published** as the assets of the fixed pre-release tag `model-facts-v1`, replaced every build. The app fetches `https://github.com/nambean/Pygg/releases/download/model-facts-v1/facts.json` and its `.sig`. A `v2` format gets a new tag and app constant; `v1` stays for older apps.
- **Kept** next to the store as `reference/model-facts.json` and `.sig` (ADR-031's pattern, `gateway/reference.rs`), verified on every read. Settings: `model_facts.fetched_at`, `model_facts.enabled` (on by default), `model_facts.last_error`. The GET carries nothing about the user.
- **Refresh:** the app and `pygg gateway serve` check once a day at start; **Refresh** or `pygg gateway facts --refresh yes` fetches now; `pygg models facts` reads the kept feed. **Off** removes the feed layer only (own evidence and tier still rank; the file stays). Offline, the kept feed is used with its `built` date. The screens tell offline ("could not refresh"), stale (`built` older than weekly) and refused (signature) apart.
- **Keys:** the `.sig` names its key id; the app trusts public keys by id. Rotation: a release adds the new key, then the secret changes, a later release drops the old key. A compromised key is dropped in a release and its feeds refused. A feed never grants trust to a key.
- **Attribution:** the source list carries each licence and its attribution line; the app shows the source's name and link wherever a fact is shown (SCREENS §3.2); THIRD_PARTY_NOTICES.md lists the sources and licences.

### Ranking in the app

Each connected model's line for a role comes from three layers, each shown with its source:

| Layer | What | Ranks when |
| --- | --- | --- |
| 1. The user's own evidence | Ledger `checks_passed` and `result_kept` on cards of that role (ADR-029). The role is `tasks.card_role`, set when the board makes a task. Counts every finished attempt whose `model_ref` (`<agent> · <model>`; `pygg/<connection/model>` for an agent on the gateway) names the model; a kept result counts once per task | From ten attempts, and then it wins (route, brain picks; the reason names the runs). Fewer: numbers shown, ranks by layer 2 |
| 2. The feed | Best `rank` among the role's sources under twelve months old, with its `derived` label. Several rows for one model (agents, efforts): the one with Pygg's agent and the connection's effort first, else the best; the line says which | When the feed names the model |
| 3. The tier | `curated:v2` or `price-band:v1` (ADR-006, ADR-031) | When nothing else is known |

- The screens name the layer of every line. A `derived` fact reads "ranked by reasoning (LMArena, 2026-11-12)", never "best for planning".
- A choice the user saved is kept when the feed changes.

**Order within a list** (`facts::order_for_role`; the role sheet, the planner pick, a role's row, and a role alias's route, so the brain's picks too):

1. Models with own evidence: kept rate in **bands of 0.1** (ten attempts do not tell 9 of 10 from 10); within a band, known money per kept result (unpriced requests after fully priced), then checks passed.
2. Models with a fact or a tier, on one scale: the fact's `rank`, else the tier's standing (T4 0.9, T3 0.7, T2 0.5, T1 0.3, T0 0.1), in **bands of 0.15** from each band's best; within a band the cheaper first: billing (local, free, subscription, paid), then the list price of a typical agent turn (8,000 tokens in, four in five read from cache where cache reads are priced, 1,000 out).
3. The rest, cheapest first.

A route's reason says when a model ranks ahead of its band's best (`cheaper than <model> in the same band for code`). Why bands: ADR-041 in [DECISIONS](../DECISIONS.md#adr-041--pyggbrain-the-router-is-the-brain).

**The store.** The feed adds no migration (a file and settings keys). Saved picks and workspace overrides are `brain_picks` (migration 0042: scope `global` or a workspace id, role, `connection/model`, saved at). `tasks.card_role` is nullable, never inferred for older tasks; the board fills it.

## 2. `pygg/brain`

### What the client sends and gets

- **Request:** `pygg/brain` is a model name on `/v1/chat/completions`, `/v1/messages` and `/v1/responses` (Claude Code, Codex, OpenCode, Cursor, `pygg ask`). The last user message is the requirement; `x-pygg-session` names the brainstorm.
- **Reply:** one assistant message: the brainstorm id and app link; numbered questions; proposed cards with role, done criterion and model; the known estimate (each card's cost range from the last ten runs of its agent and model, summed, with the count of unpriced cards; never a ceiling, §4.1).
- **Refinement:** another call in the same session, with answers, refines that brainstorm (answered questions close, cards change). Only a new session starts another.

### Steps inside the gateway

1. **Pick the planner** from the models ranked for `plan` (§1), filtered as `auto` filters (connections, format, cooldowns, context, a statable cost, data-use terms). A saved pick wins; one no longer eligible gives way to the rule's pick, which waits if paid. A paid planner on first use: the gateway answers at once with the brainstorm id and `waiting for your pick`; the user picks in the app; the client's next call (or `wait`) continues. A free or local planner runs at once.
2. **Call the planner** with the brain prompt (in the Skills & Prompts library, ADR-039; versioned, readable; the version is written on the brainstorm) and the requirement.
3. **Parse** against the board schema. A card that does not fit is dropped and counted; `brain_schema_ok` is written for the planner model (true when every card fit).
4. **Pick a model per card:** the role's saved pick on Brain Storming (the workspace's, else the default) when eligible, else by rule (below). Never the planner's suggestion.
5. **Store the brainstorm** and answer. The app shows it under Tasks.

Every brain call is a ledger entry under the client's request id and session, `route_kind` `brain`, so session and day limits hold (ADR-025). The planner's reply is stored and shown, never sent with another request (ADR-030).

**As built** (`gateway/brain.rs`; migration 0042: `brainstorms`, `brain_picks`, and `brainstorm_id`, `card_id`, `card_role` on `tasks`):

- **Planner output:** up to 16,000 tokens, or the model's maximum when lower (at 6,000, DeepSeek V4 Pro spent them all thinking). A reply cut at the limit with no text says so.
- **Planner input:** Pygg's prompt and the requirement only; no list of connections or models.
- **Parser limits:** at most 7 questions and 40 cards (`MAX_QUESTIONS`, `MAX_CARDS`; extra cards count as dropped). A cycle between cards, or no card at all, refuses the whole reply.
- **Session:** the client's `x-pygg-session`; a call without one is its own brainstorm. A session whose brainstorm is proposed, waiting, thinking or failed refines it; approved or declined starts a new one.
- **A card has a model and fallbacks.** An eligible saved pick is the model, reason `your pick for <role> on Brain Storming`. Otherwise the role's route as `auto/<role>` would try it (at most two models of one connection while others are eligible): the first is the model, the next two the suggested fallbacks, each with the rule's reason (fact or tier). The user adds or removes fallbacks from the role's ranking, at most five, keeping the task within the engine's six steps. On pull every model is a step, the card's model first; a failed attempt (connection error, spent quota, failed check) moves to the next.
- **Refinement keeps**, by card id: runner, class, the user's own fallbacks (`your pick`), dropped. The model and suggested fallbacks are picked again by rule.
- **Questions** are carried by text: one asked again keeps its id; one no longer asked is marked answered; a new question given an old id gets a fresh id, and the cards that named it follow.
- **`brain_schema_ok`** is a `feedback` row, target kind `brainstorm`, source `brain` (migration 0043 widened the checks).
- **Approval** (app only) writes one board card per kept card in wave order (§3), run by the agent the user chose. On pull the card's task keeps the brainstorm id, card id and role. Only OpenCode through its Pygg provider runs the picked models (`pygg/<connection/model>`, one step per ladder model); Claude Code, Codex and Gemini CLI get one step on their own sign-in and are told the picks in the prompt. A paid step waits for approval (ADR-010). Dependencies and WIP limits order the pulls.
- **Candidates and rankings** come from `auto::plan_with` with the role strategy: the same filters as a request to `auto/<role>`.

### The board schema

What the planner must return, and what is stored:

```text
brainstorm
  id, session, client, workspace?, planner_model, prompt_version, requirement
  state: waiting_pick | proposed | approved | declined
  questions: [{id, text, answered_by?}]
  cards: [{id, title, role, done_criterion, depends_on: [question or card ids], model, runner, class}]
    role:    plan | code | test | review      (image once the gateway has an images path)
    runner:  engine | ide                      (ADR-041: Pygg's task engine, or the user's agent)
    class:   standard | expedite               (ADR-042)
```

- **Approval needs a workspace** (a Git repository, ADR-008). A kept card becomes a card on its board; when pulled, a task with its ladder as steps, judged by the workspace's approved checks (ADR-007).
- **One branch per brainstorm:** `str/brain-<requirement>-<id>`, made at approval from HEAD (`brainstorms.branch`, migration 0045). A card's task starts from the tip; Keep adds the card's commit, picked onto the tip when an earlier card was kept meanwhile. A conflict, or a checked-out branch, keeps the result as a branch of its own (ADR-042 as amended).

### Role aliases

`auto/plan`, `auto/code`, `auto/test`, `auto/review` (built; `auto.rs`, `Strategy::Role`): `auto`, but after the hard filters the ranking is the role's facts instead of the lowest eligible tier.

- **Filters:** `auto`'s. The client's minimum tier applies to a model with a tier; one with no tier may rank by a fact; one with neither is out ("no fact for code and no tier set").
- **Order:** §1's (`facts::order_for_role`); the first five form the route.
- **Reason** (`x-pygg-auto`, ledger route reason) names the fact: `Terminal-Bench 4.x #3 of 71 (64.85% +/-3.1) dated 2026-09-22, run with claude-code at max for code; paid; about $0.0052 at the provider's list price`, or `your runs 10/10 kept, 9/10 checks passed for code`, or `no fact for code; tier T3 from Pygg's table; …`, plus `cheaper than <model> in the same band for code` when it applies. The ledger's route kind is the alias (`auto/code`).
- **Feed:** the kept one, read once per file version (`facts::cached`). Facts off or none kept: every model ranks by tier, reason `no fact`.
- **Measured once:** `auto/code` ran beside `auto`, `auto/cheap` and named models in the 2026-11-20 benchmark, one attempt per case, before the bands ([report](../../benchmark/reports/2026-11-20-all-models.md)). The banded order and the other three aliases are unmeasured; nothing is claimed (ADR-013).

### The brain prompt

`brain-v1` in the starter skills, a prompt (`disable-model-invocation: true`).

- **Asks for:** questions whose answer would change the cards (at most seven); cards small enough for one attempt and one set of checks, each with one role and one testable done criterion; order and dependencies; the reply in the schema above and nothing else.
- **Never asks for:** a model name, an estimate in time or points, any content from the user's repository.

## 3. The board

### Columns and what moves a card

| From | To | On |
| --- | --- | --- |
| Discovery | Ready | Every question the card depends on is answered |
| Ready | Running | Running has room under its WIP limit; expedite first, then oldest |
| Running | Checking | The agent ends |
| Checking | Review | The approved checks pass (ADR-007); unverified when there are none |
| Checking | Blocked | The ladder's last step fails; over the cap or budget; interrupted |
| Running | Blocked | A paid step waits for approval (ADR-010); a sensitive workspace asks |
| Blocked | Ready or Running | The user approves, retries or answers |
| Review | Done | Keep as Branch (ADR-009) or Export Patch |
| Review | Discarded | Discard |

### Limits

- **Running:** WIP limit 1 by default; the user may raise it per workspace, never remove it. 1 means one paid step in flight.
- **Daily budgets** per workspace and for all tasks are unchanged ([AGENTS](../AGENTS.md)); a card that would exceed one goes to Blocked before it starts.
- **Expedite:** one card at a time; front of Ready, still waits for room.

### What the board measures

From `board_cards`, `tasks`, `attempts` and the ledger:

| Measure | From |
| --- | --- |
| Cycle time | Ready to Done, per card |
| Lead time | Created to Done |
| Throughput | Cards Done per day |
| Time blocked | Time in Blocked, per card and per reason |
| Known money per kept result | `tasks::quality` (ADR-029), per role model |

§1's first layer, the user's own evidence, reads the same kept results.

### MCP tools

For the IDE agent (`<app> mcp`), under the client's **May start tasks** permission. None approves a paid step or a brainstorm (ADR-010).

| Tool | Does |
| --- | --- |
| `pygg_brainstorm` | Read a brainstorm by id, or by this session |
| `pygg_board` | Read a workspace's board |
| `pygg_board_move` | Move a card between Discovery and Ready |
| `pygg_model_facts` | The connected models with their role ranks and sources |

### As built (`board.rs`, migration 0044)

- **Cards are rows** (`board_cards`), made at approval from the kept cards in wave order: lane, ladder (the card's model, then its fallbacks), runner, class, dependencies, prompt, times. The board is these rows joined to their tasks; measures come from the rows' times and the tasks, nothing else is stored.
- **A tick every 3 s** (the app, while the engine runs):
  - Settles each lane from the task's state (the table above); `review` covers checks passed, needs review and accepted.
  - Moves a Discovery card to Ready once every question and card it depends on is answered or done; a card whose dependency was dropped before approval goes to Blocked, naming it.
  - Holds an IDE-run card in Blocked as **Waiting for your agent** until **Hand to the engine**.
  - Pulls Ready cards (expedite first, then by position) while Running has room, one task per card (`tasks::create_task`), one step per ladder model, run by the agent chosen at approval (OpenCode; another agent gets one step and is told the picks).
  - Leaves out a ladder model the store has turned off since approval (a catalog rule, a connection removed); with none left the card blocks as **No enabled model: …**.
  - Approves nothing: a paid step waits as always (ADR-010).
  - While the gateway is stopped: settles lanes, pulls nothing (steps run through `pygg/<model>`).
- **Only the tick loop pulls.** Commands (`board_get`, `board_act`, approval) settle lanes before answering and leave the pull to the loop, which runs inside the runtime a task's start needs.
- **The limit gates the pull, not approvals:** steps waiting for approval sit in Blocked and do not count; approving several starts them all, so Running can read over its limit until they finish.
- **Checking's limit** is held by the runner: when a run ends and the workspace already has that many tasks in checks, the run waits in phase `held` (task list: *Waiting for Checking*; board: still Running) and asks every 2 s; held runs take room expedite first, then board order. A task that is no card's is never held. Stop still works while a run waits.
- **Limits:** the Brain Storming defaults (`brain.wip_running`, `brain.wip_checking`, 1 each) unless the workspace's `board_settings` row differs; **Use the default limits** deletes the row.
- **Measures** from five done cards: median cycle time (Ready to Done) and lead time (created to Done), cards done today, time blocked today. Known money per kept result by role: the done cards' tasks' priced gateway requests (actual or estimated) over kept results, with the count of unpriced requests. The cycle time says how many done cards it covers when some never passed Ready with a time.
- **Moves:** `expedite` (one at a time), `standard`, `hold` (Ready to Discovery until released), `release`, `drop` (confirmed; Discarded, never run), `engine` (an IDE card handed to the engine). A card with a task is the engine's; the task detail does the rest (approve, retry, keep, discard).
- **Surfaces:** the app's commands, the task API (`GET /pygg/v1/board?workspace=W`, `POST /pygg/v1/board/{card}/move` with `to: discovery|ready`), the MCP tools above.

## 4. The interface

Screens as built: [SCREENS.md](SCREENS.md). Here: their rules, the journey, the IDE reply, what is not built.

- **Mockups** (static HTML on the app's stylesheet, headless Edge at 1440 px, dark; not app code): [Tasks list](../images/brain/tasks-list.png), [planner pick](../images/brain/brainstorm-pick.png), [a proposal](../images/brain/brainstorm-proposed.png), [role sheet](../images/brain/role-sheet.png), [board](../images/brain/board.png) and [its list layout at 1024 px](../images/brain/board-narrow.png), [facts on Providers](../images/brain/providers-facts.png), [Brain Storming](../images/brain/brain-storming.png).
- **Dev build screenshots:** [role sheet](../images/brain/role-sheet-live.png), [a card in Review](../images/brain/board-live.png), [a run held in Running while another is in Checking](../images/brain/board-held.png), [measures at five done cards](../images/brain/board-measures.png), [a model ranked first by the user's own runs](../images/brain/brain-own-evidence.png).

### 4.1 Rules the screens keep

- **No chat.** The requirement and the answers are typed in the coding agent; no text box in the app reaches a model (ADR-028).
- **Approvals inline** in the app, never in a notification or an API (ADR-010, SCREENS §1).
- **Honest values** (SCREENS §3): Unknown is a word; estimates read `≈$…`; the sum is **Known estimate**, never a ceiling: `Known estimate ≈$1.20 · total Unknown · 2 cards unpriced`.
- **Every rank shows its source and date** where shown, and a derived rank says **derived**. Never only in a tooltip.
- **The resolved model is written out** as `connection/model`, selectable, prefix never truncated (SCREENS §3.1); a long id wraps. After approval a card's model is frozen; a change is a new attempt started from the card.
- **The LLM classifies, rules pick, the user approves.** No screen says "the AI's choice".
- **Nothing auto-approves** on a timer or a confidence score. A blocked card waits and shows how long.
- **Keyboard and screen readers:** every control named, the sheet traps and returns focus (SCREENS §6), polling never moves focus or drops an unsaved edit, nothing hover-only.

### 4.2 The journey

```
IDE agent                        Gateway                             App
"Use pygg/brain: build a      →  no saved planner; the eligible
 shop website"                   candidates are paid
                              ←  "Brainstorm b_7f3 waits for your     Tasks: pinned "Waits for your pick"
                                  pick in Pygg"                        → Pick Planner (inline) → user picks
"continue b_7f3" or wait      →  planner runs, ledgered
                              ←  id, questions, cards, known estimate  Brainstorm b_7f3: Proposed
user answers in the IDE       →  refinement of b_7f3
                              ←  questions closed, cards changed       Proposed (revision 2)
                                                                       edit picks, drop cards, Approve… → Board
```

The app is read-mostly: pick, edit, approve, raise a limit, expedite, approve a paid step, review a result.

### 4.3 Tasks list

As built: [SCREENS §5.1](SCREENS.md#51-tasks), brainstorm rows in [§5.2.2](SCREENS.md#522-brainstorm).

### 4.4 Brainstorm detail

As built: [SCREENS §5.2.2](SCREENS.md#522-brainstorm).

### 4.5 The role sheet

As built: [SCREENS §6, Models for a role](SCREENS.md#models-for-a-role).

### 4.6 The board

As built: [SCREENS §5.1.1](SCREENS.md#511-board); what moves a card: §3.

### 4.7b Brain Storming

As built: [SCREENS §2.2, Brain Storming](SCREENS.md#brain-storming); a project's own picks and limits in [SCREENS §6](SCREENS.md#a-projects-overrides).

### 4.7c How Tasks, Workspaces, Agents and Brain Storming link

One set of rows, nothing stored twice:

| Row | Holds |
| --- | --- |
| Brainstorm | A row under Tasks |
| `board_cards` | Its cards, written at approval to the workspace's board |
| `tasks.brainstorm_id`, `tasks.card_role` | On the tasks the board makes |
| `brain_picks` (scoped to a workspace), `board_settings` | A workspace's overrides |

The workspace's approved checks are every card's Done (ADR-007); its paid steps per day and privacy apply to every card. The agent is chosen at approval from those installed. Which level a value comes from: [SCREENS §2.2](SCREENS.md#brain-storming).

### 4.8 What the IDE agent reads

Plain text in a fixed shape, for a model and a person: the brainstorm id and app link; the planner; open questions, numbered, with "answer any in your next message"; cards as `[role] title — auto/<role> → connection/model`; the known estimate and unpriced count; the closing line "Approve in Pygg; this client cannot approve." Header: `x-pygg-brainstorm: <id>`.

### 4.10 Designed, not built

| Screen | Not built |
| --- | --- |
| Brainstorm detail | Refusing an edit made against an older revision ("updated from your answers; look again") |
| Role sheet | A cost range from the last ten runs (it shows the route's estimate); opening it from a model on Providers |
| Facts on Providers | Per role in a model's detail: rank, source, date, derived label, and **Why this model?** opening the role sheet; a saved pick's row saying a newer fact exists |
| Board | A removed workspace's board kept read-only with the reason; `+42 −18 · 6 files` on Review and Done cards (the diff is in the task detail) |
| Brain Storming | A known estimate above which approval asks again (approval already always confirms); an agent per role (chosen at approval) |
| Workspaces | A row saying whether the workspace uses the brain defaults or which it overrides, with **Edit…** |
| Notifications, palette | Tray notification "A brainstorm needs you" for one waiting for a pick, proposed or failed (SCREENS §2.5); palette commands Open Board, Brainstorms needing attention, Refresh model facts |
| Later | Line comments on a result's diff sent back to the agent as one batch, stored on the card, delivered through the MCP task API (set aside by the owner); board drag and drop; a price-versus-rank view; a no-gate fast path for a trivial plan |

## Out of scope

- **Images:** no images path in the gateway, so `image` facts are stored and shown, not routed. Routing needs an images translation layer, decided separately.
- **Memory between brainstorms:** none (ADR-028, ADR-030).
- **The planner choosing the model:** never; the prompt does not ask and the parser would not take it.
- **Adaptive ranking:** own evidence only reorders above a minimum sample; nothing learns from conversation content (ADR-025).
