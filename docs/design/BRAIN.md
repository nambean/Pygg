# Pygg — The brain, the facts feed and the board

> **Status:** Design, built · the model facts feed (ADR-040), the role aliases, `pygg/brain` (ADR-041), the board and the Brain Storming screen (ADR-042) are built; what each part has and has not been seen doing is in [Implementation Status](../IMPLEMENTATION-STATUS.md) · reviewed 2026-11-19

How a requirement typed into the user's own coding agent becomes a board of cards, each on a model picked by rule from measured facts, run one at a time under the user's approval. Nothing in this document claims that code exists; [Implementation Status](../IMPLEMENTATION-STATUS.md) says what does.

The three parts, in the order they are built:

1. **The model facts feed** (ADR-040): per-role scores with their sources, published by the project, fetched by the app.
2. **`pygg/brain`** (ADR-041): a virtual model of the gateway that turns a requirement into a proposal. With it, the role aliases `auto/plan`, `auto/code`, `auto/test`, `auto/review`.
3. **The board** (ADR-042): the task engine's states as Kanban columns with WIP limits.

Each part is useful on its own: the feed improves `auto` and the Providers screen; the role aliases can be benchmarked with the existing harness; proposals can run as ordinary tasks before the board exists.

## 1. The model facts feed

### What a fact is

| Field | Meaning |
| --- | --- |
| `ids` | The canonical ids the row is about: the tail of a provider id after a router's vendor prefix, lower-cased, `.`, `_` and spaces as `-`, a `:free`-style variant, `-latest` and a trailing date dropped (`openrouter/anthropic/claude-opus-4.5` and `claude-opus-4-5-20251101` both give `claude-opus-4-5`; `canonical` in `scripts/model-facts.py` and `gateway/facts.rs`, the same rule in both) |
| `name` | The name as the source writes it |
| `role` | `plan`, `code`, `test`, `review`, `image` |
| `score` | The number the source published, to two decimals, with its `unit` |
| `place`, `of` | The row's place in its source (1 is best; rows with the same score share a place: `place` is one more than the number of rows with a strictly better score) and the source's row count |
| `rank` | `(of − place) / (of − 1)`, so 1 is the best and 0 the worst within that source (1 when `of` is 1). Comparable across sources; `score` is not |
| `agent` | For Terminal-Bench and SWE-bench, the agent the row was run with (`claude-code`, `codex`, `mini-swe-agent`, …); absent elsewhere |
| `effort` | The reasoning effort the row was run with, when the source states it (`high`, `max`, …) |
| `licence` | The source's licence for this row (`Apache-2.0`, `CC-BY-4.0`, `CC-BY-NC-4.0`) |
| `derived` | `true` when the role is not what the source measured (every `plan` and `review` fact; `test` until a source measures it) |
| `source` | The source's id, from the feed's source list, which holds its name, URL, licence, attribution, what it measures and does not, its status (`ok`, or `stale` when carried forward), the date it was read and its row count |
| `url` | Where the row was read, when the source gives one per row |
| `date` | The row's own date in the source (`YYYY-MM-DD`): what the twelve-month rule reads |

A fact with no source has no row. A model the feed does not name has no fact and keeps its tier (ADR-006).

### Sources, per role

Read on 2026-11-19: where each source keeps its data, under what terms, and whether the job may read it. The owner confirms each row before the job's first run; the job does not decide.

| Role | Source | Data | Terms | Model names | In the feed |
| --- | --- | --- | --- | --- | --- |
| `code` | Terminal-Bench 4.x | `harbor-framework/terminal-bench`, `leaderboard/submissions/*.json` (73 rows): agent, model, reasoning effort, date, accuracy with a 95% CI half-width, trials, tokens, cost in USD | Repository Apache-2.0; no separate terms for the rows. Community submissions are closed; the maintainers run them | `source_filter.model_name` is a provider id (`anthropic/claude-sonnet-5`) | **Yes.** The best source: a CI, cost per run, provider ids. The number is for an agent and a model together; prefer the row whose agent Pygg runs (Claude Code, Codex) |
| `code` | SWE-bench Verified | `SWE-bench/swe-bench.github.io`, `data/leaderboards.json` (4 MB; Verified 180 rows): name, agent, model display and organisation, model release date, reasoning effort, resolved %, date, cost | **CC BY-NC 4.0** on that repository. Since 2025-11-18 only academic submissions | Display names (`Claude 4.5 Opus (high)`), mapped by hand | **Yes** (owner, 2026-11-19: Pygg is always open source and takes no money). Each row from this source carries `licence: CC-BY-NC-4.0` in the feed, the feed's source list says so, and THIRD_PARTY_NOTICES names it, so anyone redistributing Pygg or the feed commercially knows to drop these rows |
| `code` | Aider polyglot | `Aider-AI/aider`, `aider/website/_data/polyglot_leaderboard.yml` (69 rows): model, pass rate, edit format, date, cost, command | Apache-2.0 (the repository's licence) | The `command` field carries `provider/model`, so no mapping | **Yes, shown only.** The newest row is 2025-10-03, older than a year, so it does not rank (rule below) until the leaderboard moves |
| `plan`, `review` (derived) | LMArena, text with style control | Hugging Face `lmarena-ai/leaderboard-dataset`, Parquet, `latest` split (2026-11-18): rating with bounds, votes, rank, per category (`hard_prompts`, `coding`, `instruction_following`, `expert`, …) | Dataset **CC BY 4.0**. The arena.ai site terms (2026-02-23) forbid scraping and automated access to the site, so the job reads the dataset and never the site | Display names (`Claude Opus 5.5 (High)`), mapped by hand | **Yes.** `hard_prompts` ranks `plan` and `review`, both derived. The `coding` category is preference, not tests, and is not used for `code` |
| `plan`, `review` (derived) | LiveBench | `LiveBench/livebench.github.io`, `public/table_<date>.csv` (28 models on 2026-06-25), per task; categories need `categories_<date>.json` | The site repository states no licence; the benchmark repository's Apache-2.0 and MIT cover code | Config names (`claude-opus-4-6-thinking-auto-high-effort`) | **No** (owner, 2026-11-19). The data has no licence, so it cannot be copied into a feed; LMArena covers the roles |
| — | Artificial Analysis | Free API, `GET /api/v2/language/models` | Data Platform Terms v1.1 (2026-08-19) govern the free tier: no embedding raw data in a customer-facing product, no combining it with other sources into a product, and "model/provider selection guidance" is named a competitive product | — | **No.** Pygg is what the terms exclude |
| — | OpenRouter rankings | Datasets API, `programming` category, weekly token totals; needs an OpenRouter key | CC BY 4.0 with the citation "Source: OpenRouter (openrouter.ai/rankings), as of <date>" | Permaslugs | **Not in v1.** Usage is adoption, not quality; OpenRouter says so itself. A later "most used" line, never a rank |
| `image` | LMArena text-to-image | The same dataset, `text_to_image` subset | As above | As above | Stored, not routable |

Rules the job and the app follow for every source:

- **A fact older than twelve months is shown, never ranked.** Its line stays in the sheet with its date; the rule skips it.
- **Rows are agent and model together** on Terminal-Bench and SWE-bench. The feed keeps the agent; the app prefers the row whose agent Pygg would run, else the best row, and says which.
- **Nothing is averaged across sources.** Each source is its own line; the rank the rule uses is the best rank among the role's sources, and the line it came from is marked.
- **Effort is part of the identity.** `(high)`, `(max)` and `-thinking` variants are separate rows in every source; the feed keeps them apart and maps each to the model id plus the effort the gateway would send.
- Each source's terms are recorded in the feed's `sources` list with the date they were read, and **each fact carries its source's licence**, so a feed mixes CC BY, CC BY-NC and Apache rows without hiding which is which.

### The feed file

```text
model-facts/
  v1/
    facts.json        the feed: {version, built, sources: [...], names: {...}, facts: [...]}
    facts.json.sig    its signature (ed25519)
```

- **The names map**, `model-facts/names.json`, is kept by hand and reviewed in a pull request, because leaderboard names (`Claude 4.5 Opus`, `Claude Opus 5.5 (High)`) are not provider ids. One entry per source and name: `{"<source>": {"<name as the source writes it>": {"ids": ["claude-opus-4-5"], "effort": "high"}}}`. Terminal-Bench and Aider rows carry a `provider/model` id, which the builder makes canonical itself; SWE-bench and LMArena names go through the map, and a name it does not know is written to the build log and left out, never guessed (`python scripts/model-facts.py names` lists them with a proposed id to review). The first map was proposed from the real names on 2026-11-19 and reviewed by hand; system names (a fine-tune, a sampling scheme, an agent) were left out.
- **The app matches** a fact to a connected model when `canonical(model_id)` is one of the fact's `ids`; a connection's prefix plays no part, so one fact serves the same model on every connection.
- **Built by `scripts/model-facts.py`** in a scheduled workflow, weekly (and on demand), from the sources' published data. A source that is unreachable or no longer parses keeps its facts from the last published feed, with `observed` unchanged and the source marked `stale` in the source list; the build fails only when every source fails. The 4 MB SWE-bench file is read by the job, never by the app.
- **Signed** with an ed25519 key: the 32-byte seed, base64, is the repository secret `MODEL_FACTS_SIGNING_KEY`, held only in GitHub Actions; the public key is a constant in the app (`gateway/facts.rs`, `KEYS`, key id `mf-2026-1`). The signature is over the exact bytes of `facts.json` and is published beside it as `facts.json.sig`, one line `<key id>:<base64 signature>`. A feed that does not verify, or whose `built` is not later than the one already kept, is refused and the kept one stays. Without the secret the workflow builds and keeps the feed as a run artifact and publishes nothing.
- **Published** as the assets of one fixed pre-release tag of this repository, `model-facts-v1`, replaced on every build; the app fetches `https://github.com/nambean/Pygg/releases/download/model-facts-v1/facts.json` and its `.sig`. A `v2` of the format gets a new tag and a new app constant; `v1` stays for older apps.
- **Kept next to the store**, like the models.dev catalog (`reference/model-facts.json` and its `.sig`, ADR-031's pattern in `gateway/reference.rs`), verified again on every read, with the fetch time in `settings` (`model_facts.fetched_at`), the switch in `settings` (`model_facts.enabled`, on by default), the last fetch error in `model_facts.last_error`, and the public GET carrying nothing about the user. The app checks once a day when it starts, and so does `pygg gateway serve`; **Refresh** (and `pygg gateway facts --refresh yes`) fetches now; `pygg models facts` reads what is kept. **Off** removes the feed layer only: the user's own evidence and the tier still rank, and the kept file stays for when it is turned on again. Offline, the kept feed is used and its `built` date shown; the screens tell offline ("could not refresh"), stale (a `built` older than the weekly cadence) and refused (signature) apart.
- **Keys.** The `.sig` file names its key id; the app holds the public keys it trusts by id. Rotation: a release adds the new public key, then the secret changes; the old key is dropped in a later release. A compromised key is dropped in a release and every feed signed by it is refused from then on; a feed never grants trust to a key.
- **Attribution.** The source list carries each source's licence and the attribution line its licence asks for; the app shows the source's name and link wherever a fact is shown (SCREENS §3.2), and THIRD_PARTY_NOTICES.md lists the sources with their licences.

### Ranking in the app

For a role, each connected model's line is built from three layers, each shown with its source:

1. **The user's own evidence**, from the ledger: `checks_passed` and `result_kept` for that model on cards of that role (ADR-029), once ten attempts exist; the line is ordered by kept results over attempts, then checks passed over attempts. This layer wins. As built: the role is on the task (`tasks.card_role`, set when the board makes a task from a card), the evidence is every finished attempt of such tasks whose `model_ref` (`<agent> · <model>`, the model `pygg/<connection/model>` for an agent pointed at the gateway) names the model, a kept result counts once per task, and a model with fewer than ten attempts shows its numbers but ranks by the next layer. Seen on 2026-11-19 after ten kept flash runs on code cards: [brain-own-evidence.png](../images/brain/brain-own-evidence.png).
2. **The feed**, the best `rank` among the role's sources that are not older than twelve months, with its `derived` label. Where a source has several rows for one model (agents, efforts), the row whose agent Pygg would run and whose effort the connection sends comes first, else the best row, and the line says which row it is.
3. **The tier** (`curated:v1` or `price-band:v1`, ADR-006, ADR-031), when nothing else is known.

The interface says which layer a line comes from. A `derived` fact reads "ranked by reasoning (LMArena, 2026-11-12)", never "best for planning". A choice the user saved is kept when the feed changes; the screen shows that a newer fact exists.

**Order within a list** (the role sheet, the planner pick, a role's row): first by the layer that won (own evidence, then feed, then tier); within own evidence by kept rate, then checks-passed rate; within the feed by `rank`; within the tier by tier; then by estimated cost, then by `connection/model`. No tie is asserted; equal numbers stay in that order and both are shown.

**Phases and the store.** Phase 1 (the feed) adds no migration: the feed is a file next to the store, its settings two keys. Saved picks and workspace overrides (the Brain Storming screen) come with `pygg/brain` as one migration (`brain_picks`: scope `global` or a workspace id, role, `connection/model`, saved at). The role comes with `pygg/brain` on the task (`tasks.card_role`, nullable, never inferred for older tasks) and the board fills it, so the own-evidence layer fills from the board's first tasks on.

## 2. `pygg/brain`

### What the client sends and gets

`pygg/brain` is a model name on `/v1/chat/completions`, `/v1/messages` and `/v1/responses`, so Claude Code, Codex, OpenCode, Cursor or `pygg ask` can call it. The client sends its conversation as it would to any model; the last user message is the requirement, and the client's `x-pygg-session` names the brainstorm.

The reply is one assistant message holding:

- the brainstorm's id and the app's link to it;
- the clarifying questions, numbered;
- the proposed cards, each with its role, its done criterion and the model picked;
- the known estimate: the sum of each card's cost range from the last ten runs of its agent and model, with the count of cards that have no price (section 4.1: never called a ceiling).

A second call in the same session, with answers in the conversation, refines that brainstorm: the questions it answers close, cards change. It never starts another brainstorm; a new session does.

### Steps inside the gateway

1. **Pick the planner.** The connected models ranked for `plan` (section 1), filtered as `auto` filters (connections, format, cooldowns, context, a statable cost, data-use terms). A pick the user saved wins. A paid planner on its first use: the gateway answers at once with the brainstorm id and `waiting for your pick`, the app shows the ranked list, the user picks, and the client's next call (or `wait`) continues. A free or local planner runs at once.
2. **Call the planner** with the brain prompt, the requirement and the connected models with their role ranks. The brain prompt is a prompt in the Skills & Prompts library (ADR-039), versioned, readable by the user; the version is written on the brainstorm.
3. **Parse** the reply against the board schema below. A card that does not fit is dropped and counted. `brain_schema_ok` is written as a metric for the planner model (true when every card fit).
4. **Pick a model per card** by rule: role, the role's ranking, `auto`'s filters, the client's policy. Never the planner's suggestion, which the prompt does not ask for.
5. **Store the brainstorm** and answer the client. The app shows it under Tasks.

Every call the brain makes is a ledger entry under the client's request id and session with `route_kind` `brain`, so session and day limits hold (ADR-025). The planner's reply is stored on the brainstorm, shown to the user, and never sent with any other request (ADR-030).

**As built** (`gateway/brain.rs`, migration 0042: `brainstorms`, `brain_picks`, and `brainstorm_id`, `card_id`, `card_role` on `tasks`):

- The planner gets Pygg's prompt and the requirement alone: no list of the user's connections or models, since the prompt forbids naming a model and a planner has no use for the list.
- The brainstorm's session is the client's `x-pygg-session`; a call without one is a brainstorm of its own. A call in a session whose brainstorm is proposed, waiting, thinking or failed refines it; one whose brainstorm was approved or declined starts a new one.
- **A card has a model and fallbacks** (owner, 2026-11-19). The rule takes the role's route as `auto/<role>` would try it (at most two models of one connection while others are eligible): the first is the card's model, the next two its suggested fallbacks, each with the reason the rule gave (the fact or the tier). The user removes or adds fallbacks from the role's ranking (every line with its source and date), at most five, so the card's task stays within the engine's six steps. On approval every model is a step of the card's task, the card's model first: an attempt that fails (a connection error, a spent quota, a failed check) moves the task to the next model, as the engine already does.
- A card's runner and class, its own fallbacks (`your pick`), and whether it was dropped, survive a refinement by the card's id; the model and the suggested fallbacks are picked again by rule.
- Questions are carried by their text across a refinement: one the planner asks again keeps its id, one it stopped asking is marked answered, and a new question the planner gave an old id (Gemini numbered its new question `q1` again on the first live run) gets a fresh id, with the cards that named it following.
- `brain_schema_ok` is a `feedback` row with target kind `brainstorm` and source `brain` (migration 0043 widened the table's checks; the first live run wrote nothing until then).
- Approval (the app) makes one task per kept card in wave order, with one step on the card's model, run by the agent the user chose; the task keeps the brainstorm's and the card's ids and the role. Only OpenCode through its Pygg provider runs on the picked model (`pygg/<connection/model>`); Claude Code, Codex and Gemini CLI run with their own sign-in and are told the pick in the prompt. A paid step still waits for approval (ADR-010). Dependencies order the tasks; the engine runs them as it runs tasks today, and WIP limits come with the board.
- The candidates and the per-role rankings come from `auto::plan_with` with the role strategy, so the planner pick and every card follow exactly the filters a request to `auto/<role>` would.

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

Approving a brainstorm needs a workspace (a Git repository, ADR-008). Each card becomes a task with one step on its model; its approved checks are the workspace's (ADR-007).

### Role aliases

`auto/plan`, `auto/code`, `auto/test`, `auto/review` (built; `auto.rs`, `Strategy::Role`) are `auto` with one change: after the hard filters, the ranking is the role's facts instead of the lowest eligible tier.

- **Filters:** `auto`'s. The client's minimum tier applies to a model whose tier is set; a model with no tier may still rank by a fact; a model with neither is out with "no fact for code and no tier set".
- **Order:** the models with a fact that ranks (not older than twelve months), best `rank` first; then the models with no fact by tier, the higher first; within each, local, free, subscription, paid, then the estimated cost, then the name. The first five form the route as for `auto`.
- **The reason** (in `x-pygg-auto` and the ledger's route reason) names the fact: `Terminal-Bench 4.x #3 of 71 (64.85% +/-3.1) dated 2026-09-22, run with claude-code at max for code; paid; about $0.0052 at the provider's list price`, or `no fact for code; tier T3 from Pygg's table; …`. The ledger's route kind is the alias (`auto/code`).
- **The feed** is the one kept beside the store, read once per file version (`facts::cached`); facts off, or none kept, means every model ranks by its tier and the reason says `no fact`.
- **Not measured.** No benchmark run has compared a role alias with `auto` or a named model on the C3 cases, so nothing is claimed about them (ADR-013); the run is the owner's call, since it spends from the benchmark budget.

### The brain prompt

Kept as `brain-v1` in the starter skills, a prompt (`disable-model-invocation: true`). What it asks for:

- questions whose answer would change the cards, at most seven;
- cards small enough for one attempt and one set of checks, each with one role and one done criterion a check can test;
- the order and dependencies between cards;
- the reply in the schema above, and nothing else.

What it does not ask for: a model name, an estimate in time or points, or any content from the user's repository.

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

- Running's WIP limit is 1 by default. The user may raise it, per workspace; it cannot be removed. A limit of 1 means one paid step in flight.
- Daily budgets per workspace and for all tasks stay as they are (AGENTS.md). A card that would exceed one goes to Blocked before it starts.
- Expedite: one card at a time; it goes to the front of Ready and still waits for room.

### What the board measures

From `tasks`, `attempts` and the ledger, nothing new is stored:

| Measure | From |
| --- | --- |
| Cycle time | Ready to Done, per card |
| Lead time | Created to Done |
| Throughput | Cards Done per day |
| Time blocked | Time in Blocked, per card and per reason |
| Known money per kept result | `tasks::quality` (ADR-029), per role model |

The second layer of section 1's ranking is fed from these.

### MCP tools

For the IDE agent (`<app> mcp`), under the client's **May start tasks** permission:

- `pygg_brainstorm`: read a brainstorm by id, or by this session.
- `pygg_board`: read a workspace's board.
- `pygg_board_move`: move a card between Discovery and Ready.
- `pygg_model_facts`: the connected models with their role ranks and sources.

None approves a paid step or a brainstorm (ADR-010).

### As built (phase 4, `board.rs`, migration 0044)

- **Cards are rows** (`board_cards`), made on approval from the brainstorm's kept cards in wave order, each with its lane, the ladder (the card's model, then its fallbacks), the runner, the class, the dependencies, the prompt and the times. The board is those rows joined to their tasks; the measures come from the rows' times and the tasks, nothing else is stored.
- **A tick every three seconds** (the app, while the engine runs) settles every lane from the task's state (`running`, `checking` once a check runs, `blocked` with the reason a paid step waits, over the cap or budget, interrupted, or a failed ladder; `review` on checks passed, needs review or accepted; `done` on kept; `discarded` on discarded), moves a Discovery card to Ready once every question and card it depends on is answered or done, puts a card whose dependency was dropped before approval in Blocked naming it, and holds an IDE-run card in Blocked as **Waiting for your agent** until **Hand to the engine**. Then it pulls Ready cards, expedite first, then by position, while Running has room under its limit, making one task per card (`tasks::create_task`) with one step per model of the ladder, run by the agent chosen at approval. A ladder model the store has turned off since approval (a catalog rule, a connection removed) is left out, since the gateway would refuse it and the agent would fail with nothing to say; a ladder with none left blocks the card as **No enabled model: …**. No approval is given by the tick: a paid step waits as it always did (ADR-010). The limit gates the pull, not approvals: steps waiting for approval sit in Blocked and do not count, and approving several at once starts them all, so Running can read over its limit until they finish.
- **Checking's limit** is held by the runner: when an agent's run ends and the workspace already has that many tasks in their checks, the run waits with the phase `held` (the task list says *Waiting for Checking*; the board keeps the card in Running) and asks every two seconds; held runs take the room expedite first, then in board order. A task that is no card's is never held. Stop still works while a run waits. Seen on 2026-11-19 in the dev build: [board-held.png](../images/brain/board-held.png), the second card waiting in Running while the first is in Checking.
- **Limits** are the Brain Storming defaults (`brain.wip_running`, `brain.wip_checking`, 1 each) unless a workspace's `board_settings` row differs; **Use the default limits** deletes the row.
- **Measures** from five done cards on: cycle time (Ready to Done) and lead time (created to Done) medians, cards done today, time blocked today. Known money per kept result, by role: the done cards' tasks' priced requests through the gateway (actual or estimated) over the kept results, with the count of requests that had no price. The cycle time says how many done cards it covers when some never passed through Ready with a time. Seen on 2026-11-19 at five done cards: [board-measures.png](../images/brain/board-measures.png).
- **Moves:** `expedite` (one at a time) and `standard`, `hold` (Ready to Discovery, held until released), `release`, `drop` (confirmed; Discarded, never run), `engine` (an IDE card handed to the engine). A card with a task is the engine's; the app's task detail does the rest (approve, retry, keep, discard).
- **Surfaces:** the app's commands, the task API (`GET /pygg/v1/board?workspace=W`, `POST /pygg/v1/board/{card}/move` with `to: discovery|ready`) and the MCP tools above.

## 4. The interface

Reviewed against [SCREENS.md](SCREENS.md) on 2026-11-19 (a read-only Codex review of the draft, and a survey of comparable products: OpenRouter, Cursor, Cline, Kilo Code, LMArena, Vibe Kanban, Conductor, Codex cloud, Claude Code web, Jules, Devin, Linear agents, Kiro). What was taken from each is in the table at the end of this section.

**Mockups** (static HTML on the app's own stylesheet, screenshotted in headless Edge at 1440 px, dark; not app code; approved by the owner on 2026-11-19): [Tasks list](../images/brain/tasks-list.png), [the planner pick](../images/brain/brainstorm-pick.png), [a proposal](../images/brain/brainstorm-proposed.png), [the role sheet](../images/brain/role-sheet.png), [the board](../images/brain/board.png) and [its list layout at 1024 px](../images/brain/board-narrow.png), [facts on Providers](../images/brain/providers-facts.png), [the Brain Storming screen](../images/brain/brain-storming.png).

### 4.1 Rules the screens keep

- **No chat.** The requirement is typed in the coding agent; the brain's questions are answered there. No text box in the app reaches a model (ADR-028).
- **Approvals inline** in the app, never in a notification or an API (ADR-010, SCREENS §1).
- **Honest values** (SCREENS §3): Unknown is a word; estimates read `≈$…`; the sum of known estimates is called **Known estimate**, never a ceiling, and when any card is unpriced the line reads `Known estimate ≈$1.20 · total Unknown · 2 cards unpriced`.
- **Every rank shows its source and date where it is shown**, and a derived rank says **derived**. A rank never hides in a tooltip.
- **The resolved model is written out** as `connection/model` wherever a card is shown, selectable, the prefix never truncated (SCREENS §3.1). A long id wraps. After approval a card's model is frozen; a change is a new attempt the user starts from the card, never a silent switch.
- **The LLM classifies, rules pick, the user approves.** No screen calls a model "the AI's choice".
- **Nothing auto-approves on a timer or a confidence score.** A blocked card waits, and shows how long it has waited.
- **Keyboard and screen readers:** every control is named, the sheet traps and returns focus (SCREENS §6), polling never moves focus or drops an unsaved edit, and nothing is reachable only by hover.

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

- A brainstorm is a row with a lightbulb icon and a state word: **Waits for your pick**, **Thinking** (the planner runs), **Proposed**, **Approved · board**, **Declined**, **Planner failed**.
- **Needs Attention** pins **Waits for your pick**, **Proposed** and **Planner failed**, button **Review**.
- The row: the requirement's first line, the client and session that asked (so two IDEs at once are two rows), the planner or "no planner yet", questions open/answered, cards, known estimate.
- A **Board** entry in the segmented control once a workspace has a board.

### 4.4 Brainstorm detail

`‹ Tasks` goes back. Reloads every two seconds while **Waits for your pick** or **Thinking**; a reload never moves focus or discards an edit in progress, and an edit made against an older revision is refused with "updated from your answers; look again".

**Header:** the requirement; the state, client and time; the planner as `connection/model · tier · billing · brain-v1`; cost so far (the planner calls, from the ledger); the known estimate.

**Waits for your pick** (inline, like a paid step's approval):

- The text says this is the first paid request Pygg would start on its own, and shows the planner's estimated cost and the limit it counts against (session or day).
- The candidates are the `plan` ranking: each as `connection/model · tier · billing · ≈$ per brainstorm`, with its winning evidence line (`your runs 6/8 kept`, or `LMArena hard prompts #5 · 2026-11-18 · derived`, or `tier only`). "Why this model?" opens the layer chain and the other sources.
- **Remember this planner** (on by default; saves it as the plan role on the Brain Storming screen, 4.7b). Buttons **Decline** and **Use <connection/model>**.
- After the pick the planner call starts; the proposal reaches the client on its next call or `wait`, and the app's row turns **Thinking**.
- With no eligible candidate: the reasons grouped by connection, as `auto`'s 422 gives them, and nothing to click but **Decline**.
- A free or local candidate at the top skips this state. So does a **saved planner**, paid or not: the pick is asked only when nothing is saved and the rule's pick is paid. A saved planner that is no longer eligible brings the state back, with the reason.

**Questions:** a read-only list, each **open** or **answered** with the answer, who answered (the client) and when. The caption says answers come from the coding agent. A card that depends on an open question shows a lock.

**Cards:** grouped by wave (cards that depend on nothing, then cards that depend on those), a role chip on each. A row is compact: title, role chip, `connection/model` (selectable, wraps), billing and cost range (`≈$0.05–0.30 · last 10 runs`, or Unknown), the winning evidence line, **Change…**, **Drop**. A disclosure holds the full done criterion, dependencies, the runner, the class, and the other evidence lines.

- **Change…** opens the role sheet (4.5). Changing one card asks whether to apply to every card of that role.
- **Runner** defaults to the engine for every role. **My IDE agent** is offered per card and puts the card into an explicit **Waiting for your agent** state on the board, with how long it has waited and **Hand to the engine** as the way out; nothing times out.
- A dropped card is struck through with **Restore** until approval. **Drop** is not confirmed; approval shows the count.
- No reordering and no text editing in the app; refinement comes from the IDE.

**Approve:** `Known estimate … · today's task budget left $…`, the WIP limit for Running, then **Decline brainstorm** and **Approve <n> cards…**. The button is disabled until the proposal has loaded and is at its current revision. Approval validates first: the workspace exists and is a Git repository (else a workspace picker appears, like New Task's, and the proposal is kept); every model is still eligible (an ineligible one shows its reason and needs a new pick); costs are recomputed. `ConfirmSheet` then names the workspace, the count, that paid cards still wait for approval one by one, the known estimate and the unpriced count.

**States:** loading; a load error with **Try Again** (never an empty list); **Thinking**; **Planner failed**, with the raw reply behind a disclosure, the dropped-card count when the schema partly failed ("7 cards kept, 2 dropped"), the planner's schema record ("failed 1 of 3 times"), **Pick another planner** and **Retry**; zero valid cards (the same screen, nothing to approve); **Declined**; the IDE that never came back (the brainstorm stays **Proposed**; nothing paid starts without the user's approval here); stress: 40 cards, 12 questions, 240-character criteria, 90-character ids, at 1024, 1440, 1990, 819 and 683 px.

### 4.5 The role sheet

Used by the planner pick, a card's **Change…**, and a model's facts on Providers.

- Header: `Models for code · facts as of 2026-11-18 · Refresh`, search, and **Eligible under my policy** / **All connections**.
- One row per model: `connection/model · tier · billing · ≈$ range`, then **one evidence line**, the layer that won: `your runs 8/10 kept · 9/10 checks passed`, or `Terminal-Bench 4.0 #3 of 73 · 61.2% ±3.1 · 2026-06-30`, or `LMArena hard prompts #5 · derived · 2026-11-18`, or `tier only`. A source's name links to its URL.
- **Why this model?** on a row opens a disclosure: the chain `your saved pick › your runs (10+ attempts) › facts feed › tier`, with the layer that won marked; every other source line with its date, CI or sample, and a one-line statement of what it measures and does not ("Terminal-Bench: tasks solved in a terminal with an agent, not your repository"); a fact older than twelve months, shown and marked as not ranking.
- Sorted as section 1 says: the winning layer, then that layer's measure, then cost, then the id. Uncertainty is shown as the source publishes it; the sheet asserts no tie and changes no rule.
- Below a rule, **Not eligible** with each model's reason, as `auto` gives them.
- States: no feed yet or fetching off ("ranked by tier; facts off" with **Turn on**); a stale cached feed (its date, and "could not refresh" when a refresh failed); no match for the search; no eligible model; a saved pick whose model is no longer eligible (kept, marked, a new pick needed).
- Stress: 40 rows with 90-character ids and long localised labels; ids wrap, actions stack under the row at 683 px.
- **As built** (`RoleSheet.tsx`): a sheet opened by a card's **Change…** and **Add a fallback…**, by the planner pick's "Why these models?" and by **Change…** on Brain Storming. Header `Models for code · facts as of 2026-11-18` (or *ranked by tier; facts off* with **Turn facts on**, *no facts yet; ranked by tier*, *could not refresh* when stale), a search field, and with a brainstorm the toggle **Eligible under my policy** / **All connections** (`brainstorm_candidates` with the role: the client's route for that role, as `pick_models` ranks it). One row per model: `connection/model · tier · billing · ≈$ per run` (the route's estimate; without a client, the output price per Mtok or *price unknown*), the evidence line, *current* and *your saved pick* badges, **Not eligible: <reason>** under **All connections**, **Why this model?** with the chain (saved pick › your runs › feed › tier, the winner marked) and every source line with its agent, effort, votes, licence and link, a fact older than a year marked as not ranking, and **Use**. **Refresh facts** in the footer. The cost range from the last ten runs and the Providers entry point are not built: the estimate is the route's, and Providers keeps its own facts line.

### 4.6 The board

One board per workspace, a workspace switcher when there are several. Reached from the Tasks list or a brainstorm's **Open Board**.

- **Header:** workspace; `Running 1/1 · Blocked 2 · Ready 6 · Done 3`; **Measures** as a disclosure (cycle time median, cards done today, time blocked today, known money per kept result, each with its source), shown only once five cards are done; the engine's state and **Start** beside the controls when it is stopped, in place, not a banner (SCREENS §1).
- **Columns:** the seven of ADR-042. Running and Checking show `count/limit` with **Limit…**, default 1, raised but never removed; the queue columns show their count. A card that depends on an unanswered question sits in **Discovery** (ADR-042), never in Ready.
- **Card:** title, role chip, `connection/model` (wraps; the full id and everything else in the card's detail), the attempt sequence chips, elapsed time or cost so far, ▲ for expedite; in Review and Done, `+42 −18 · 6 files`. Blocked cards carry the reason word (Waiting for approval, Over cap, Over budget, Interrupted, Needs an answer, Waiting for your agent) and `blocked 2 h 10 min`. One button per card, from its state: **Review** in Blocked and Review, **Open** elsewhere.
- **Moves by button, not drag:** **Expedite**, **Send back to Discovery**, **Drop** (confirmed). Running, Checking, Review and Done are the engine's. Buttons name exactly what is allowed; a drag target the engine refuses would teach the wrong model.
- **Opening a card** opens task detail (SCREENS §5.2) with `‹ Board`; a cost on a card is the task's cost line, and its source (ledger, the agent's estimate, unknown) is in that detail, as today.
- **Dependencies:** a card whose prerequisite was dropped shows **Needs an answer** in Blocked with the missing card named; a cycle the planner produced is refused at parse time, so the board never holds one.
- **Width:** when seven readable columns do not fit the content area (the sidebar takes 224 px; at a 1024 px window that leaves about 114 px per column), the board becomes one list grouped by column, Blocked first, with every action a button. The switch is by available width, not window width, so it holds with the sidebar open and at Windows text scaling (819 and 683 px).
- **States:** no board yet ("Brainstorms start from your coding agent: ask it to use pygg/brain"); load error with **Try Again**; the engine stopped; a workspace removed (the board is kept read-only, with the reason).
- **As built** (`Board.tsx`, `board.css`): reached as the **Board** entry after the Tasks filters (with the count of cards in Blocked and Review); the header carries the workspace or its switcher, the counts, **Measures** as a disclosure and **Use the default limits** when the workspace differs; **Limit…** on Running and Checking edits in place. A card shows its title, the role chip, the attempt chips of its task, the ladder's first model and how many follow, the Blocked reason and time, the phase and elapsed time while it runs, and its buttons. The list layout switches at a board width of 980 px by container query, Blocked first, then Review, Running, Checking, Ready, Discovery, Done. A card's task opens with **‹ Board** and returns to the board. As it ran on 2026-11-19, the first card in Review: [board-live.png](../images/brain/board-live.png) (the dev build on a store copy, not a mockup). Not built yet: the engine's state in the header, a removed workspace's read-only board, `+42 −18 · 6 files` on Review and Done (the task's diff is in its detail).

### 4.7 Facts on Providers

- No new column. A model's detail (the row's disclosure) gains **Facts**: one line per role with rank, source, date and the derived label, and **Why this model?** opening the role sheet (4.5).
- The feed's switch, date and **Refresh** live on the Brain Storming screen (4.7b). Off means the tier table alone. A failed refresh keeps the old date and says so; a feed that fails its signature check shows "Facts refused (signature)" and keeps the last good feed. With no feed at all: "ranked by tier; no facts yet".
- Saved picks survive a feed change; the row says when a newer fact exists.

### 4.7b Brain Storming: set up once, applied to every project

A screen of its own in the Smart Tasks group (owner, 2026-11-19), so the picks and defaults live in one place instead of a checkbox in a brainstorm and a question on a card. It is settings: nothing on it sends anything to a model.

- **Models by role:** one row per role (plan, code, test, review) with the saved pick as `connection/model`, its billing and cost range, its evidence line, **Change…** (the role sheet, 4.5) and **Clear**. A cleared role reads "By the rules (no saved pick)" and shows what the rules would pick today. The plan row is also the planner. The chain stays: a workspace's override › this saved pick › your runs › feed › tier.
- **Defaults for a new brainstorm:** runner (engine or IDE agent), WIP limits for Running and Checking (Running never below 1), the known estimate above which approval asks again, whether a planner picked inside a brainstorm is saved here, and what **Change…** on a card offers first (ask, this card, all of its role).
- **Facts and prompt:** the feed switch, its date and **Refresh**, the source list with each source's licence, row count and date, and the brain prompt's version with **Open** into Skills & Prompts. The Providers header no longer carries the feed switch; a model's facts stay in its detail (4.7).
- **Projects that differ:** the workspaces with an override (which roles and limits), **Edit…** and **Use defaults**; other workspaces offer **Override…**. An override holds only what differs.
- **How it starts:** the reminder that a brainstorm starts from the coding agent with the model `pygg/brain`, with the one-line command per agent. No text box.
- **States:** no connected model for a role ("nothing eligible for code: connect a provider or loosen the policy"); a saved model no longer eligible (kept, marked, with the reason); facts off (every role row reads "by tier"); stress at 683 px with 90-character ids wrapping under the role chip.
- **As built** (`BrainStorming.tsx`, settings keys `brain.*`, `brain_picks` scope `global` or a workspace id): the five sections above. **Models by role** shows the saved pick with its evidence line (your runs, the fact, or the tier), a cleared role with what the rules would pick today, a saved model no longer eligible marked as such, **Change…** the role sheet (4.5), **Clear**. **Defaults** hold the runner, both WIP limits, **Remember a planner picked in a brainstorm** (the pick's checkbox starts from it), and what **Change…** on a card offers first (the card's Change… follows it: ask with This card / All cards of the role / Cancel, or one of the two without asking); the approval-asks-again threshold is not built (approval already confirms every time). **Projects that differ** lists every workspace with its own picks (`role → model`) and its own WIP limits, and **Use defaults**; a workspace's own picks are written from a brainstorm's **Change…** and its limits from its board, so there is no **Override…** sheet here. **How it starts** shows the three commands.

### 4.7c How Tasks, Workspaces, Agents and Brain Storming link

The four screens of the Smart Tasks group share one set of rows; nothing is stored twice.

| From | To | The link |
| --- | --- | --- |
| Brain Storming → Tasks | A brainstorm is a row under Tasks (4.3); approving it turns each card into a task row (`tasks.brainstorm_id`, `tasks.card_role`) and the workspace's board (4.6) is those tasks grouped by state | The screen never lists brainstorms itself; **How it starts** points at Tasks |
| Brain Storming → Workspaces | **Projects that differ** lists workspaces by their `workspaces` row; **Override…** writes `brain_picks` rows scoped to that workspace and nothing else; **Use defaults** deletes them | A workspace row on Workspaces shows "Brain: uses defaults" or "Brain: overrides code, Running 2" with **Edit…** opening the same sheet |
| Workspaces → the board | A board exists per workspace that has at least one card; the workspace's **approved checks** are every card's definition of Done (ADR-007), its **paid steps per day** and **privacy** apply to every card, and removing the workspace freezes its board read-only | The Workspaces badge counts, as today, workspaces whose checks wait for review; a board with no approved checks sends every card to Review unverified, and the workspace row says so |
| Tasks → Workspaces | A task row names its workspace; a brainstorm from an IDE carries the repository, which Workspaces adds by itself (SCREENS §5.3) | The **Board** entry in the Tasks filter opens the board of the task list's workspace, or a switcher when there are several |
| Agents → a card's runner | A card run by the engine runs one of the installed agent CLIs (ADR-026) with the card's model when that agent is pointed at the gateway; the agent for a role is the Brain Storming default (one per role, from the agents Agents lists as installed) | A role whose agent is not installed shows "no agent for code: install one on Agents" on the Brain Storming row, and approval refuses such cards |
| Settings → budgets | The daily task budget and per-workspace budget stay where they are (Settings, Workspaces); the Brain Storming screen only reads them in the approval line | — |

Precedence for anything that exists at two levels (picks, WIP limits, runner): the workspace's override, then the Brain Storming default, then the rules. A screen always says which level a value came from.

### 4.8 What the IDE agent reads

The reply from `pygg/brain` is plain text in a fixed shape, read by a model and a person: the brainstorm id and the app's link; the planner; the open questions, numbered, with "answer any in your next message"; the cards as `[role] title — auto/<role> → connection/model`; the known estimate and the unpriced count; one closing line: "Approve in Pygg; this client cannot approve." The response carries `x-pygg-brainstorm: <id>`.

### 4.9 Notifications, palette, checks

- A tray notification for **Waits for your pick**, **Proposed** and **Planner failed** says "A brainstorm needs you" and nothing more (SCREENS §2.5).
- Palette: Open Board, Brainstorms needing attention, Refresh model facts.
- `pnpm ui-check` gains these screens and states: brainstorm detail with 40 cards and 12 questions, the pick state, planner failed, the role sheet with 40 rows and a disclosure open, the board in both layouts with Blocked cards, at all five sizes. Keyboard and screen-reader flows are checked by hand, since the script does not establish them.

### 4.10 Later, not in v1

- Line comments on a result's diff sent back to the agent as one batch, stored on the card and delivered through the MCP task API (set aside by the owner on 2026-11-12).
- Drag and drop on the board.
- A price-versus-rank view of the same facts.
- A no-gate fast path for a trivial plan.

### 4.11 What was taken from comparable products

| Product | Taken | Avoided |
| --- | --- | --- |
| OpenRouter rankings | The date beside every number; a one-line "what this measures and does not" | Usage share as quality; OpenRouter itself says it is adoption |
| Cursor Auto | — | An auto-pick that hides or later changes the model; users report plan burn and lost trust |
| Cline | One remembered model per role | A recommendation with no visible reason |
| Kilo Code Auto | A visible chain for where a pick came from; a role default with a stated fallback | Mappings in prose that drift from the interface |
| Artificial Analysis | A versioned, dated evidence source; effort as part of the identity | A composite index that does not name its evals (and its terms exclude Pygg) |
| LMArena | Published uncertainty and vote counts next to a rank | A sorted list that reads a one-point gap as a verdict |
| Vibe Kanban | Column moves driven by the agent's state; attempts as children of a card | No WIP limit, no cost, no Blocked column; approval per tool call |
| Conductor | One computed next action per card; checks as the definition of Done | — |
| Codex cloud, Claude Code web | `+N −M` on the card; a named "waiting on you" state | A blocked card that expires silently |
| Jules, Devin | A named awaiting-approval state; the end-of-task summary | Timer or confidence-gated auto-approval |
| Linear agents | The session state machine (`awaitingInput` is Blocked) and the question record on the card | — |
| Claude Code plan mode | Structured questions apart from one approval gate; never an approve button before the content | Auto-approval under any mode; Enter as the trigger |
| Kiro specs | Waves of independent cards | — |

## Out of scope, said plainly

- **Images.** The gateway has no images path, so `image` facts are stored and shown, not routed. Routing needs an images translation layer, decided separately.
- **Memory between brainstorms.** None (ADR-028, ADR-030).
- **The planner choosing the model.** Never; the prompt does not ask and the parser would not take it.
- **An adaptive ranking.** The user's own evidence only reorders above a minimum sample; nothing learns from conversation content (ADR-025).
