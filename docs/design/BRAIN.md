# Pygg — The brain, the facts feed and the board

> **Status:** Current · built, on hold (owner review) · verification per part: [Implementation Status](../IMPLEMENTATION-STATUS.md)

A requirement typed into the user's own coding agent becomes a board of cards, each on a model picked by rule from measured facts, run one at a time under the user's approval.

| Part | ADR | What | Status |
| --- | --- | --- | --- |
| 1. The model facts feed | ADR-040 | Signed per-role scores with sources, published weekly, fetched daily by the app | Built; live on a store copy |
| 2. `pygg/brain` and the role aliases | ADR-041 | `auto/plan`, `auto/code`, `auto/test`, `auto/review`; a virtual model that turns a requirement into proposed cards | Built; live on a store copy |
| 3. The board | ADR-042 | Kanban over the task engine with WIP limits | Built; ran live once in the dev build |

Each part works alone: the feed ranks the role aliases, and the aliases can be benchmarked with the existing harness.

## 1. The model facts feed

**What:** one fact per model, role and source row: canonical model ids, role (`plan`, `code`, `test`, `review`, `image`), score, place, `rank` (1 best, 0 worst within the source), agent and effort when stated, licence, `derived` (role not what the source measured), source and date.

**Sources** (terms read 2026-11-19; the owner decides which are read):

| Source | Licence |
| --- | --- |
| Terminal-Bench 4.x (`code`) | Apache-2.0 |
| SWE-bench Verified (`code`) | CC BY-NC 4.0 (rows marked so a commercial redistributor can drop them) |
| Aider polyglot (`code`) | Apache-2.0 |
| LMArena text, `hard_prompts` (`plan`, `review`, derived) | CC BY 4.0 (dataset; the site is never scraped) |
| LMArena text-to-image (`image`) | CC BY 4.0 |
| LiveBench | No licence on the data |
| Artificial Analysis | Terms exclude products like Pygg |
| OpenRouter rankings | CC BY 4.0 (usage, not quality) |

- **In the feed:** Terminal-Bench, SWE-bench Verified, LMArena. **Shown only:** Aider (newest row over a year old), LMArena images (no images path). **Not used:** LiveBench, Artificial Analysis, OpenRouter.

**Key rules:**

- No source, no row; a model the feed does not name keeps its tier (ADR-006).
- Older than twelve months: shown, never ranked.
- Nothing averaged across sources; each source is its own line.
- Effort is identity: `(high)`, `(max)`, `-thinking` rows stay apart.
- Agent and model together (Terminal-Bench, SWE-bench): prefer the row with Pygg's agent, and say which.
- Unknown source names are logged and left out, never guessed (`model-facts/names.json`, reviewed by hand).
- **Signed** ed25519 (key id `mf-2026-1`); a feed that does not verify, or is not newer, is refused; a feed never grants trust to a key.
- **Published** at the release tag `model-facts-v1` (`https://github.com/nambean/Pygg/releases/download/model-facts-v1/facts.json` and `.sig`); the GET carries nothing about the user.
- **Refresh:** once a day at start; **Refresh** or `pygg gateway facts --refresh yes` now; `pygg models facts` reads the kept feed; **Off** removes the feed layer only.
- **Attribution:** every shown fact names its source and link; sources listed in THIRD_PARTY_NOTICES.md.
- Where: `scripts/model-facts.py` (weekly workflow), `gateway/facts.rs`.

**Ranking in the app** — each model's line for a role comes from the first layer that applies, and the screens name it:

| Layer | Ranks when |
| --- | --- |
| 1. The user's own evidence (kept results and checks on cards of that role, ADR-029) | From ten attempts |
| 2. The feed (best `rank` under twelve months old, with its `derived` label) | The feed names the model |
| 3. The tier (`curated:v2`, `price-band:v1`) | Nothing else is known |

- Order (`facts::order_for_role`): own evidence first in bands of 0.1; then facts and tiers on one scale in bands of 0.15, cheaper first within a band; then the rest, cheapest first. Why bands: [ADR-041](../DECISIONS.md#adr-041--pyggbrain-the-router-is-the-brain).
- A derived fact reads "ranked by reasoning (LMArena, date)", never "best for planning".
- A choice the user saved is kept when the feed changes.

## 2. `pygg/brain`

**Role aliases** `auto/plan`, `auto/code`, `auto/test`, `auto/review`: `auto`'s filters, then §1's order; the first five form the route.

- The reason (`x-pygg-auto`, ledger) names the fact, own runs or tier that won.
- A model with neither fact nor tier is out; facts off ranks every model by tier.
- Status: `auto/code` measured once in the [2026-11-20 benchmark](../../benchmark/reports/2026-11-20-all-models.md), before the bands; the banded order and the other three aliases are unmeasured (ADR-013).

**`pygg/brain`** is a model name on `/v1/chat/completions`, `/v1/messages` and `/v1/responses`; the last user message is the requirement, `x-pygg-session` names the brainstorm.

- **Reply:** brainstorm id and app link, numbered questions (at most 7), proposed cards (at most 40) with role, done criterion and model, and the known estimate.
- **Refinement:** another call in the same session with answers refines the brainstorm; a new session starts another.
- **Planner:** picked by rule from the `plan` ranking; a saved pick wins; a paid planner waits for the user's pick in the app; a free or local one runs at once.
- **Prompt:** `brain-v2` in Skills & Prompts (ADR-039); it never asks for a model name, a time estimate or repository content. Since v2 it tells the planner that each card is judged by the checks right after it, so every card must leave the tests passing, with a test in the same card as its code or a later one: a planner that went test-first (eleven test cards before any code) jammed the board at the first red test (2026-11-21).
- **Models per card:** the role's saved pick on Brain Storming, else the rule's pick plus two suggested fallbacks (the user may set up to five); never the planner's suggestion.
- **Approval** (app only, needs a Git workspace, ADR-008) writes one board card per kept card on one branch per brainstorm.
- Only OpenCode through its Pygg provider runs the picked models; Claude Code, Codex and Gemini CLI run one step on their own sign-in and are told the picks.
- Every brain call is ledgered (`route_kind` `brain`), so session and day limits hold (ADR-025); the planner's reply is never sent with another request (ADR-030).
- Where: `gateway/brain.rs`, migrations 0042–0045.

## 3. The board

**Columns:** Discovery → Ready → Running → Checking → Review → Done, plus Blocked and Discarded.

| From | To | On |
| --- | --- | --- |
| Discovery | Ready | Every question and card it depends on is answered or done |
| Ready | Running | Running has room; expedite first, then oldest |
| Running | Checking | The agent ends |
| Checking | Review | Approved checks pass (ADR-007) |
| Running or Checking | Blocked | Paid step waits (ADR-010), ladder fails, over budget, interrupted |
| Blocked | Ready or Running | The user approves, retries or answers |
| Review | Done or Discarded | Keep as Branch (ADR-009), Export Patch, or Discard |

**Key rules:**

- Running WIP limit 1 by default (one paid step in flight); raised per workspace, never removed; Checking has its own limit.
- Expedite: one card at a time, front of Ready.
- Daily budgets are unchanged ([AGENTS](../AGENTS.md)); a card that would exceed one blocks before it starts.
- The board approves nothing; steps waiting for approval do not count against the limit.
- An IDE-run card waits as **Waiting for your agent** until **Hand to the engine**.
- Measures (from five done cards): cycle time, lead time, cards done today, time blocked, known money per kept result by role.
- MCP tools `pygg_brainstorm`, `pygg_board`, `pygg_board_move` (Discovery ↔ Ready only), `pygg_model_facts`; none approves anything.
- Where: `board.rs` (a 3 s tick), migration 0044.

## 4. The interface

Screens as built: [SCREENS.md](SCREENS.md).

### 4.1 Rules the screens keep

- **No chat:** requirements and answers are typed in the coding agent (ADR-028).
- **Approvals inline** in the app, never by notification or API (ADR-010).
- **Honest values:** Unknown is a word; the sum is **Known estimate**, never a ceiling.
- **Every rank shows its source and date**; a derived rank says **derived**; never only in a tooltip.
- **The resolved model is written out** as `connection/model`; after approval a card's model is frozen.
- **The LLM classifies, rules pick, the user approves**; nothing auto-approves.

### 4.3 Tasks list

[SCREENS §5.1](SCREENS.md#51-tasks).

### 4.4 Brainstorm detail

[SCREENS §5.2.2](SCREENS.md#522-brainstorm).

### 4.5 The role sheet

[SCREENS §6, Models for a role](SCREENS.md#models-for-a-role).

### 4.6 The board

[SCREENS §5.1.1](SCREENS.md#511-board); moves: §3.

### 4.7b Brain Storming

[SCREENS §2.2](SCREENS.md#brain-storming); a project's overrides in [SCREENS §6](SCREENS.md#a-projects-overrides).

### 4.7c How Tasks, Workspaces, Agents and Brain Storming link

One set of rows: brainstorms, `board_cards`, `tasks.brainstorm_id`/`card_role`, and a workspace's `brain_picks` and `board_settings` overrides; the workspace's checks, budget and privacy apply to every card.

### 4.8 What the IDE agent reads

Plain text: brainstorm id and link, planner, numbered questions, cards as `[role] title — auto/<role> → connection/model`, known estimate, and "Approve in Pygg; this client cannot approve." Header `x-pygg-brainstorm`.

### 4.10 Designed, not built

- Refusing an edit made against an older brainstorm revision.
- Role sheet: cost range from the last ten runs; opening it from Providers.
- Facts per role in a model's detail on Providers.
- Board: a removed workspace's board kept read-only; diff stats on cards.
- A known-estimate threshold that asks again; an agent per role.
- Workspaces row showing brain overrides.
- Tray notification and palette commands for brainstorms.
- Later: diff line comments back to the agent, drag and drop, a price-versus-rank view.
- Out of scope: routing `image`, memory between brainstorms (ADR-028, ADR-030), the planner choosing models.
