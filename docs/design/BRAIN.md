# Pygg — The brain, the facts feed and the board

> **Status:** Design · decided (ADR-040, ADR-041, ADR-042), not built · reviewed 2026-11-19

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
| `match` | A model line, as `gateway/tiers.rs` matches it (every part appears in the id), or `{exact: "<provider id>"}` |
| `role` | `plan`, `code`, `test`, `review`, `image` |
| `score` | The number the source published, unchanged, with its unit |
| `rank` | The score's place within that source, 0 to 1, 1 the best. Comparable across sources; `score` is not |
| `derived` | `true` when the role is not what the source measured (every `plan` and `review` fact; `test` until a source measures it) |
| `source` | The source's short name, from the feed's source list |
| `url` | Where the number was read |
| `observed` | The date the job read it |

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

- `names` maps each source's model name to a `match` or an exact provider id. It is kept by hand in the repository and reviewed in a pull request, because leaderboard names (`claude-sonnet-4.5`, `Claude Sonnet 4.5 (thinking)`) are not provider ids.
- Built by a scheduled workflow in this repository, weekly, from the sources' published data; a build whose sources are unreachable keeps the last facts and marks them.
- Signed with a key held in the workflow's secrets; the public key is compiled into the app. A feed that does not verify is refused and the last good one kept.
- Published as a release asset of this repository and fetched over HTTPS. The app checks daily; the user can refresh and can turn fetching off, in which case the tier table alone is used.

### Ranking in the app

For a role, each connected model's line is built from three layers, each shown with its source:

1. **The user's own evidence**, from the ledger: `checks_passed` and `result_kept` for that model on cards of that role (ADR-029), once ten attempts exist. This layer wins.
2. **The feed**, the best `rank` among the role's sources, with its `derived` label.
3. **The tier** (`curated:v1` or `price-band:v1`, ADR-006, ADR-031), when nothing else is known.

The interface says which layer a line comes from. A `derived` fact reads "ranked by reasoning (LMArena, 2026-11-12)", never "best for planning". A choice the user saved is kept when the feed changes; the screen shows that a newer fact exists.

## 2. `pygg/brain`

### What the client sends and gets

`pygg/brain` is a model name on `/v1/chat/completions`, `/v1/messages` and `/v1/responses`, so Claude Code, Codex, OpenCode, Cursor or `pygg ask` can call it. The client sends its conversation as it would to any model; the last user message is the requirement, and the client's `x-pygg-session` names the brainstorm.

The reply is one assistant message holding:

- the brainstorm's id and the app's link to it;
- the clarifying questions, numbered;
- the proposed cards, each with its role, its done criterion and the model picked;
- what the proposal would cost at most, from the role models' prices and the last ten runs; Unknown where a price is unknown.

A second call in the same session, with answers in the conversation, refines that brainstorm: the questions it answers close, cards change. It never starts another brainstorm; a new session does.

### Steps inside the gateway

1. **Pick the planner.** The connected models ranked for `plan` (section 1), filtered as `auto` filters (connections, format, cooldowns, context, a statable cost, data-use terms). A pick the user saved wins. A paid planner on its first use: the gateway answers at once with the brainstorm id and `waiting for your pick`, the app shows the ranked list, the user picks, and the client's next call (or `wait`) continues. A free or local planner runs at once.
2. **Call the planner** with the brain prompt, the requirement and the connected models with their role ranks. The brain prompt is a prompt in the Skills & Prompts library (ADR-039), versioned, readable by the user; the version is written on the brainstorm.
3. **Parse** the reply against the board schema below. A card that does not fit is dropped and counted. `brain_schema_ok` is written as a metric for the planner model (true when every card fit).
4. **Pick a model per card** by rule: role, the role's ranking, `auto`'s filters, the client's policy. Never the planner's suggestion, which the prompt does not ask for.
5. **Store the brainstorm** and answer the client. The app shows it under Tasks.

Every call the brain makes is a ledger entry under the client's request id and session with `route_kind` `brain`, so session and day limits hold (ADR-025). The planner's reply is stored on the brainstorm, shown to the user, and never sent with any other request (ADR-030).

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

`auto/plan`, `auto/code`, `auto/test`, `auto/review` are `auto` with one change: after the hard filters, rank by the role's facts instead of the lowest eligible tier. `x-pygg-auto` and the ledger carry the fact's source, rank and date. A model with no fact for the role keeps its tier rank. The aliases are measured with the benchmark harness on the C3 cases before any claim is made about them (ADR-013).

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
- A free or local candidate at the top skips this state.

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
- Sorted by the winning layer, then cost. Uncertainty is shown as the source publishes it; the sheet asserts no tie and changes no rule.
- Below a rule, **Not eligible** with each model's reason, as `auto` gives them.
- States: no feed yet or fetching off ("ranked by tier; facts off" with **Turn on**); a stale cached feed (its date, and "could not refresh" when a refresh failed); no match for the search; no eligible model; a saved pick whose model is no longer eligible (kept, marked, a new pick needed).
- Stress: 40 rows with 90-character ids and long localised labels; ids wrap, actions stack under the row at 683 px.

### 4.6 The board

One board per workspace, a workspace switcher when there are several. Reached from the Tasks list or a brainstorm's **Open Board**.

- **Header:** workspace; `Running 1/1 · Blocked 2 · Ready 6 · Done 3`; **Measures** as a disclosure (cycle time median, cards done today, time blocked today, known money per kept result, each with its source), shown only once five cards are done; the engine's state and **Start** beside the controls when it is stopped, in place, not a banner (SCREENS §1).
- **Columns:** the seven of ADR-042, each header `name · count/limit` with **Limit…**; defaults Running 1, Checking 1, the others none. A limit can be raised or set, never removed from Running.
- **Card:** title, role chip, `connection/model` (wraps; the full id and everything else in the card's detail), the attempt sequence chips, elapsed time or cost so far, ▲ for expedite; in Review and Done, `+42 −18 · 6 files`. Blocked cards carry the reason word (Waiting for approval, Over cap, Over budget, Interrupted, Needs an answer, Waiting for your agent) and `blocked 2 h 10 min`. One button per card, from its state: **Review** in Blocked and Review, **Open** elsewhere.
- **Moves by button, not drag:** **Expedite**, **Send back to Discovery**, **Drop** (confirmed). Running, Checking, Review and Done are the engine's. Buttons name exactly what is allowed; a drag target the engine refuses would teach the wrong model.
- **Opening a card** opens task detail (SCREENS §5.2) with `‹ Board`.
- **Dependencies:** a card whose prerequisite was dropped shows **Needs an answer** in Blocked with the missing card named; a cycle the planner produced is refused at parse time, so the board never holds one.
- **Width:** when seven readable columns do not fit the content area (the sidebar takes 224 px; at a 1024 px window that leaves about 114 px per column), the board becomes one list grouped by column, Blocked first, with every action a button. The switch is by available width, not window width, so it holds with the sidebar open and at Windows text scaling (819 and 683 px).
- **States:** no board yet ("Brainstorms start from your coding agent: ask it to use pygg/brain"); load error with **Try Again**; the engine stopped; a workspace removed (the board is kept read-only, with the reason).

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
