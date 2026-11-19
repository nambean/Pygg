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

| Role | Source | What it measures | Terms to check before the job reads it |
| --- | --- | --- | --- |
| `code` | SWE-bench Verified | Resolved real GitHub issues, with tests | The results repository's licence |
| `code` | Aider polyglot | Edits across languages, with tests | The leaderboard YAML's licence in the Aider repository |
| `code` | Terminal-Bench | Tasks done in a terminal | The results' licence |
| `plan`, `review` (derived) | LMArena text | Human preference, blind pairs | The dataset's licence on Hugging Face and the site's terms |
| `plan`, `review` (derived) | LiveBench | Reasoning, maths, data, with fresh questions | The repository's licence |
| `plan`, `review` (derived) | Artificial Analysis | Its intelligence index | Its API terms; left out if they do not allow this use |
| `image` | LMArena text-to-image | Human preference on images | As LMArena text |

Each source's terms are recorded in the feed's `sources` list with the date they were read. A source whose terms do not allow the use is left out and said to be. The owner reads the terms; the job does not decide.

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

### Screens

Under Smart Tasks, replacing the Tasks list as the main view once built:

- **Brainstorms.** Each proposal: its requirement, planner and prompt version, the questions with their state, the cards with role, model, runner and class, the cost ceiling. Change a model (from the role's ranked list, each line with its source), drop a card, approve. No text box for answers: they come from the client.
- **Board.** The columns, each with its WIP limit and count; cards with their attempt sequence chip; Blocked pinned. Raise a limit, expedite a card, approve a paid step.
- **Facts.** On Providers, each model shows its role ranks with their sources and dates, and the feed's date; refresh and turn fetching off.

### MCP tools

For the IDE agent (`<app> mcp`), under the client's **May start tasks** permission:

- `pygg_brainstorm`: read a brainstorm by id, or by this session.
- `pygg_board`: read a workspace's board.
- `pygg_board_move`: move a card between Discovery and Ready.
- `pygg_model_facts`: the connected models with their role ranks and sources.

None approves a paid step or a brainstorm (ADR-010).

## Out of scope, said plainly

- **Images.** The gateway has no images path, so `image` facts are stored and shown, not routed. Routing needs an images translation layer, decided separately.
- **Memory between brainstorms.** None (ADR-028, ADR-030).
- **The planner choosing the model.** Never; the prompt does not ask and the parser would not take it.
- **An adaptive ranking.** The user's own evidence only reorders above a minimum sample; nothing learns from conversation content (ADR-025).
