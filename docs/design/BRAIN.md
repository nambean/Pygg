# Pygg — The brain, the facts feed and the board

> **Status:** Current · the model facts feed (ADR-040), the role aliases, `pygg/brain` (ADR-041), the board and the Brain Storming screen (ADR-042), as designed and built · reviewed 2026-11-20 · how each part was verified: [Implementation Status](../IMPLEMENTATION-STATUS.md)

How a requirement typed into the user's own coding agent becomes a board of cards, each on a model picked by rule from measured facts, run one at a time under the user's approval.

The three parts, in the order they were built:

1. **The model facts feed** (ADR-040): per-role scores with their sources, published by the project, fetched by the app.
2. **`pygg/brain`** (ADR-041): a virtual model of the gateway that turns a requirement into a proposal. With it, the role aliases `auto/plan`, `auto/code`, `auto/test`, `auto/review`.
3. **The board** (ADR-042): the task engine's states as Kanban columns with WIP limits.

Each part is useful on its own: the feed ranks the role aliases, and the role aliases can be benchmarked with the existing harness.

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

Read on 2026-11-19: where each source keeps its data, under what terms, and whether the job may read it. Which sources the job reads is the owner's decision, not the job's.

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

1. **The user's own evidence**, from the ledger: `checks_passed` and `result_kept` for that model on cards of that role (ADR-029), once ten attempts exist. This layer wins: the route for a role alias and the brain's picks rank such a model first, and the reason names the runs. The role is on the task (`tasks.card_role`, set when the board makes a task from a card). The evidence is every finished attempt of such tasks whose `model_ref` (`<agent> · <model>`, the model `pygg/<connection/model>` for an agent pointed at the gateway) names the model, and a kept result counts once per task. A model with fewer than ten attempts shows its numbers but ranks by the next layer. The order within this layer is below.
2. **The feed**, the best `rank` among the role's sources that are not older than twelve months, with its `derived` label. Where a source has several rows for one model (agents, efforts), the row whose agent Pygg would run and whose effort the connection sends comes first, else the best row, and the line says which row it is.
3. **The tier** (`curated:v2` or `price-band:v1`, ADR-006, ADR-031), when nothing else is known.

The interface says which layer a line comes from. A `derived` fact reads "ranked by reasoning (LMArena, 2026-11-12)", never "best for planning". A choice the user saved is kept when the feed changes.

**Order within a list** (the role sheet, the planner pick, a role's row, and the route of a role alias, so the brain's picks too; `facts::order_for_role`): first the models with the user's own evidence, by kept rate in **bands of 0.1** (ten attempts do not tell 9 of 10 from 10), and within a band by known money per kept result (a model with unpriced requests after those fully priced), then checks passed. Then the models with a fact or a tier on one scale: a fact's `rank`, or for a model the feed does not name its tier's standing (T4 0.9, T3 0.7, T2 0.5, T1 0.3, T0 0.1), in **bands of 0.15** from each band's best, and within a band the cheaper first: billing (local, free, subscription, paid), then the list price of a typical agent turn (8,000 tokens in, four in five read from the cache where cache reads are priced, 1,000 out). Then the rest, cheapest first. A route's reason says when a model ranks ahead of its band's best (`cheaper than <model> in the same band for code`).

Why bands, and the run that showed the need: ADR-041 in [DECISIONS](../DECISIONS.md#adr-041--pyggbrain-the-router-is-the-brain).

**The store.** The feed adds no migration: it is a file next to the store, its settings two keys. Saved picks and workspace overrides (the Brain Storming screen) are `brain_picks` (migration 0042: scope `global` or a workspace id, role, `connection/model`, saved at). The role is on the task (`tasks.card_role`, nullable, never inferred for older tasks), and the board fills it, so the own-evidence layer fills from the board's tasks.

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

1. **Pick the planner.** The connected models ranked for `plan` (section 1), filtered as `auto` filters (connections, format, cooldowns, context, a statable cost, data-use terms). A pick the user saved wins; a saved pick that is no longer eligible gives way to the rule's pick, which waits if it is paid. A paid planner on its first use: the gateway answers at once with the brainstorm id and `waiting for your pick`, the app shows the ranked list, the user picks, and the client's next call (or `wait`) continues. A free or local planner runs at once.
2. **Call the planner** with the brain prompt and the requirement. The brain prompt is a prompt in the Skills & Prompts library (ADR-039), versioned, readable by the user; the version is written on the brainstorm.
3. **Parse** the reply against the board schema below. A card that does not fit is dropped and counted. `brain_schema_ok` is written as a metric for the planner model (true when every card fit).
4. **Pick a model per card**: the pick saved for the card's role on Brain Storming (the workspace's own, else the default) when it is eligible, otherwise by rule: role, the role's ranking, `auto`'s filters, the client's policy. Never the planner's suggestion, which the prompt does not ask for.
5. **Store the brainstorm** and answer the client. The app shows it under Tasks.

Every call the brain makes is a ledger entry under the client's request id and session with `route_kind` `brain`, so session and day limits hold (ADR-025). The planner's reply is stored on the brainstorm, shown to the user, and never sent with any other request (ADR-030).

**As built** (`gateway/brain.rs`, migration 0042: `brainstorms`, `brain_picks`, and `brainstorm_id`, `card_id`, `card_role` on `tasks`):

- The planner call asks for up to 16,000 output tokens, or the model's own maximum when lower: a reasoning planner thinks before it writes the board, and at the earlier 6,000 DeepSeek V4 Pro spent them all thinking. A reply cut at the limit with no text says so.
- The planner gets Pygg's prompt and the requirement alone: no list of the user's connections or models, since the prompt forbids naming a model and a planner has no use for the list.
- The brainstorm's session is the client's `x-pygg-session`; a call without one is a brainstorm of its own. A call in a session whose brainstorm is proposed, waiting, thinking or failed refines it; one whose brainstorm was approved or declined starts a new one.
- **A card has a model and fallbacks.** A pick saved for the role on Brain Storming is the card's model when it is eligible under the client's policy, with the reason `your pick for <role> on Brain Storming`. Otherwise the rule takes the role's route as `auto/<role>` would try it (at most two models of one connection while others are eligible): the first is the card's model, the next two its suggested fallbacks, each with the reason the rule gave (the fact or the tier). The user removes or adds fallbacks from the role's ranking (every line with its source and date), at most five, so the card's task stays within the engine's six steps. When the board pulls the card, every model is a step of its task, the card's model first: an attempt that fails (a connection error, a spent quota, a failed check) moves the task to the next model, as the engine already does.
- A card's runner and class, its own fallbacks (`your pick`), and whether it was dropped, survive a refinement by the card's id; the model and the suggested fallbacks are picked again by rule.
- Questions are carried by their text across a refinement: one the planner asks again keeps its id, one it stopped asking is marked answered, and a new question the planner gave an old id gets a fresh id, with the cards that named it following.
- `brain_schema_ok` is a `feedback` row with target kind `brainstorm` and source `brain` (migration 0043 widened the table's checks to allow it).
- Approval (the app) writes one board card per kept card in wave order (section 3), to be run by the agent the user chose. When the board pulls a card it makes the card's task, which keeps the brainstorm's and the card's ids and the role. Only OpenCode through its Pygg provider runs on the picked models (`pygg/<connection/model>`, one step per model of the ladder); Claude Code, Codex and Gemini CLI get one step, run with their own sign-in and are told the picks in the prompt. A paid step still waits for approval (ADR-010). Dependencies and the WIP limits order the pulls.
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

Approving a brainstorm needs a workspace (a Git repository, ADR-008). Each kept card becomes a card on the workspace's board; when the board pulls it, it becomes a task with its ladder as steps, judged by the workspace's approved checks (ADR-007). The cards build on the brainstorm's branch (`str/brain-<requirement>-<id>`, made at approval from the workspace's HEAD; `brainstorms.branch`, migration 0045): a card's task starts from the branch's tip, and Keep adds the card's commit to it, picked onto the tip when an earlier card was kept meanwhile. A result that conflicts, or a branch that is checked out, is kept as a branch of its own (ADR-042 as amended).

### Role aliases

`auto/plan`, `auto/code`, `auto/test`, `auto/review` (built; `auto.rs`, `Strategy::Role`) are `auto` with one change: after the hard filters, the ranking is the role's facts instead of the lowest eligible tier.

- **Filters:** `auto`'s. The client's minimum tier applies to a model whose tier is set; a model with no tier may still rank by a fact; a model with neither is out with "no fact for code and no tier set".
- **Order:** section 1's order for the role (`facts::order_for_role`): the user's own evidence, then facts and tiers on one scale in bands of 0.15, the cheaper first within a band, then the rest. The first five form the route as for `auto`.
- **The reason** (in `x-pygg-auto` and the ledger's route reason) names the fact: `Terminal-Bench 4.x #3 of 71 (64.85% +/-3.1) dated 2026-09-22, run with claude-code at max for code; paid; about $0.0052 at the provider's list price`, or `your runs 10/10 kept, 9/10 checks passed for code`, or `no fact for code; tier T3 from Pygg's table; …`, with `cheaper than <model> in the same band for code` when a model ranks ahead of its band's best. The ledger's route kind is the alias (`auto/code`).
- **The feed** is the one kept beside the store, read once per file version (`facts::cached`); facts off, or none kept, means every model ranks by its tier and the reason says `no fact`.
- **Measured once.** The overnight benchmark of 2026-11-20 ran `auto/code` beside `auto`, `auto/cheap` and named models, one attempt per case, before the bands ([report](../../benchmark/reports/2026-11-20-all-models.md)). The banded order and the other three aliases are not measured, so nothing is claimed about them (ADR-013).

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

From the board's own rows (`board_cards`), `tasks`, `attempts` and the ledger:

| Measure | From |
| --- | --- |
| Cycle time | Ready to Done, per card |
| Lead time | Created to Done |
| Throughput | Cards Done per day |
| Time blocked | Time in Blocked, per card and per reason |
| Known money per kept result | `tasks::quality` (ADR-029), per role model |

The first layer of section 1's ranking, the user's own evidence, reads the same kept results.

### MCP tools

For the IDE agent (`<app> mcp`), under the client's **May start tasks** permission:

- `pygg_brainstorm`: read a brainstorm by id, or by this session.
- `pygg_board`: read a workspace's board.
- `pygg_board_move`: move a card between Discovery and Ready.
- `pygg_model_facts`: the connected models with their role ranks and sources.

None approves a paid step or a brainstorm (ADR-010).

### As built (`board.rs`, migration 0044)

- **Cards are rows** (`board_cards`), made on approval from the brainstorm's kept cards in wave order, each with its lane, the ladder (the card's model, then its fallbacks), the runner, the class, the dependencies, the prompt and the times. The board is those rows joined to their tasks; the measures come from the rows' times and the tasks, nothing else is stored.
- **A tick every three seconds** (the app, while the engine runs) settles every lane from the task's state (`running`, `checking` once a check runs, `blocked` with the reason a paid step waits, over the cap or budget, interrupted, or a failed ladder; `review` on checks passed, needs review or accepted; `done` on kept; `discarded` on discarded), moves a Discovery card to Ready once every question and card it depends on is answered or done, puts a card whose dependency was dropped before approval in Blocked naming it, and holds an IDE-run card in Blocked as **Waiting for your agent** until **Hand to the engine**. Then it pulls Ready cards, expedite first, then by position, while Running has room under its limit, making one task per card (`tasks::create_task`) with one step per model of the ladder, run by the agent chosen at approval (with OpenCode; another agent gets one step and is told the picks). A ladder model the store has turned off since approval (a catalog rule, a connection removed) is left out, since the gateway would refuse it and the agent would fail with nothing to say; a ladder with none left blocks the card as **No enabled model: …**. No approval is given by the tick: a paid step waits as it always did (ADR-010). While the gateway is stopped the tick settles lanes and pulls nothing, since a card's steps run through the gateway (`pygg/<model>`) and would fail at once. Only the app's tick loop pulls: a command (`board_get`, `board_act`, approval) settles the lanes before it answers and leaves the pull to the loop, which runs inside the runtime a task's start needs. The limit gates the pull, not approvals: steps waiting for approval sit in Blocked and do not count, and approving several at once starts them all, so Running can read over its limit until they finish.
- **Checking's limit** is held by the runner: when an agent's run ends and the workspace already has that many tasks in their checks, the run waits with the phase `held` (the task list says *Waiting for Checking*; the board keeps the card in Running) and asks every two seconds; held runs take the room expedite first, then in board order. A task that is no card's is never held. Stop still works while a run waits.
- **Limits** are the Brain Storming defaults (`brain.wip_running`, `brain.wip_checking`, 1 each) unless a workspace's `board_settings` row differs; **Use the default limits** deletes the row.
- **Measures** from five done cards on: cycle time (Ready to Done) and lead time (created to Done) medians, cards done today, time blocked today. Known money per kept result, by role: the done cards' tasks' priced requests through the gateway (actual or estimated) over the kept results, with the count of requests that had no price. The cycle time says how many done cards it covers when some never passed through Ready with a time.
- **Moves:** `expedite` (one at a time) and `standard`, `hold` (Ready to Discovery, held until released), `release`, `drop` (confirmed; Discarded, never run), `engine` (an IDE card handed to the engine). A card with a task is the engine's; the app's task detail does the rest (approve, retry, keep, discard).
- **Surfaces:** the app's commands, the task API (`GET /pygg/v1/board?workspace=W`, `POST /pygg/v1/board/{card}/move` with `to: discovery|ready`) and the MCP tools above.

## 4. The interface

The screens are described as built in [SCREENS.md](SCREENS.md). This section keeps the rules they follow, the journey from the IDE to the board, what the IDE agent reads, and what is designed but not built.

**Mockups** (static HTML on the app's own stylesheet, screenshotted in headless Edge at 1440 px, dark; not app code): [Tasks list](../images/brain/tasks-list.png), [the planner pick](../images/brain/brainstorm-pick.png), [a proposal](../images/brain/brainstorm-proposed.png), [the role sheet](../images/brain/role-sheet.png), [the board](../images/brain/board.png) and [its list layout at 1024 px](../images/brain/board-narrow.png), [facts on Providers](../images/brain/providers-facts.png), [the Brain Storming screen](../images/brain/brain-storming.png). **Screenshots of the dev build:** [the role sheet](../images/brain/role-sheet-live.png), [the board with a card in Review](../images/brain/board-live.png), [a run held in Running while another is in Checking](../images/brain/board-held.png), [the measures at five done cards](../images/brain/board-measures.png), [a model ranked first by the user's own runs](../images/brain/brain-own-evidence.png).

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

As built: [SCREENS §5.1](SCREENS.md#51-tasks), with the brainstorm rows in [§5.2.2](SCREENS.md#522-brainstorm).

### 4.4 Brainstorm detail

As built: [SCREENS §5.2.2](SCREENS.md#522-brainstorm).

### 4.5 The role sheet

As built: [SCREENS §6, Models for a role](SCREENS.md#models-for-a-role).

### 4.6 The board

As built: [SCREENS §5.1.1](SCREENS.md#511-board); what moves a card is section 3 above.

### 4.7b Brain Storming

As built: [SCREENS §2.2, Brain Storming](SCREENS.md#brain-storming), and a project's own picks and limits in [SCREENS §6](SCREENS.md#a-projects-overrides).

### 4.7c How Tasks, Workspaces, Agents and Brain Storming link

The screens of the Smart Tasks group share one set of rows; nothing is stored twice. A brainstorm is a row under Tasks. Approval writes its cards to the workspace's board (`board_cards`), and the tasks the board makes carry `tasks.brainstorm_id` and `tasks.card_role`. A workspace's overrides are `brain_picks` rows scoped to it and its `board_settings` row. The workspace's approved checks are every card's definition of Done (ADR-007), and its paid steps per day and privacy apply to every card. The agent that runs the cards is chosen at approval from the agents installed. Which level a value comes from is in [SCREENS §2.2](SCREENS.md#brain-storming).

### 4.8 What the IDE agent reads

The reply from `pygg/brain` is plain text in a fixed shape, read by a model and a person: the brainstorm id and the app's link; the planner; the open questions, numbered, with "answer any in your next message"; the cards as `[role] title — auto/<role> → connection/model`; the known estimate and the unpriced count; one closing line: "Approve in Pygg; this client cannot approve." The response carries `x-pygg-brainstorm: <id>`.

### 4.10 Designed, not built

- **Brainstorm detail:** an edit made against an older revision refused with "updated from your answers; look again".
- **The role sheet:** a cost range from the last ten runs (the sheet shows the route's estimate), and opening it from a model on Providers.
- **Facts on Providers:** in a model's detail, one line per role with rank, source, date and the derived label, and **Why this model?** opening the role sheet; a saved pick's row saying when a newer fact exists.
- **The board:** a removed workspace's board kept read-only with the reason, and `+42 −18 · 6 files` on Review and Done cards (the task's diff is in its detail).
- **Brain Storming:** a known estimate above which approval asks again (approval already confirms every time), and an agent per role (the agent is chosen at approval).
- **Workspaces:** a row saying whether the workspace uses the brain defaults or which it overrides, with **Edit…**.
- **Notifications and palette:** a tray notification, "A brainstorm needs you", for a brainstorm that waits for a pick, is proposed or failed (SCREENS §2.5); palette commands Open Board, Brainstorms needing attention and Refresh model facts.
- **Later:** line comments on a result's diff sent back to the agent as one batch, stored on the card and delivered through the MCP task API (set aside by the owner on 2026-11-12); drag and drop on the board; a price-versus-rank view of the same facts; a no-gate fast path for a trivial plan.

## Out of scope, said plainly

- **Images.** The gateway has no images path, so `image` facts are stored and shown, not routed. Routing needs an images translation layer, decided separately.
- **Memory between brainstorms.** None (ADR-028, ADR-030).
- **The planner choosing the model.** Never; the prompt does not ask and the parser would not take it.
- **An adaptive ranking.** The user's own evidence only reorders above a minimum sample; nothing learns from conversation content (ADR-025).
