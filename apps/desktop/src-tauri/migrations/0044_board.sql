-- The board (ADR-042, docs/design/BRAIN.md section 3): an approved brainstorm's cards, each a task once the board
-- pulls it into Running under the workspace's WIP limit. A card without a task is in Discovery (a question or a
-- card it depends on is not done, or the user held it) or Ready; with a task its lane follows the task's state.
CREATE TABLE board_cards (
  id               TEXT PRIMARY KEY,
  workspace_id     TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  brainstorm_id    TEXT REFERENCES brainstorms(id) ON DELETE SET NULL,
  -- The card's id inside its brainstorm (c1), which other cards' dependencies name.
  card_id          TEXT NOT NULL,
  title            TEXT NOT NULL,
  role             TEXT NOT NULL CHECK (role IN ('plan', 'code', 'test', 'review')),
  done             TEXT NOT NULL,
  prompt           TEXT NOT NULL,
  -- Card ids (c2) and question ids (q1) of the brainstorm this card waits for.
  depends_on_json  TEXT NOT NULL DEFAULT '[]',
  -- The models in order, connection/model: the card's model first, its fallbacks after; one step each.
  ladder_json      TEXT NOT NULL DEFAULT '[]',
  agent            TEXT NOT NULL,
  runner           TEXT NOT NULL DEFAULT 'engine' CHECK (runner IN ('engine', 'ide')),
  class            TEXT NOT NULL DEFAULT 'standard' CHECK (class IN ('standard', 'expedite')),
  lane             TEXT NOT NULL DEFAULT 'discovery'
                   CHECK (lane IN ('discovery', 'ready', 'running', 'checking', 'blocked', 'review', 'done', 'discarded')),
  -- The user sent it back to Discovery: it stays there until they move it to Ready.
  held             INTEGER NOT NULL DEFAULT 0 CHECK (held IN (0, 1)),
  task_id          TEXT REFERENCES tasks(id) ON DELETE SET NULL,
  blocked_reason   TEXT,
  blocked_since    INTEGER,
  -- Time spent in Blocked before the current stay, milliseconds.
  blocked_ms       INTEGER NOT NULL DEFAULT 0,
  ready_at         INTEGER,
  started_at       INTEGER,
  done_at          INTEGER,
  position         INTEGER NOT NULL DEFAULT 0,
  created_at       INTEGER NOT NULL,
  updated_at       INTEGER NOT NULL
);
CREATE INDEX board_cards_by_workspace ON board_cards (workspace_id, lane);
CREATE INDEX board_cards_by_brainstorm ON board_cards (brainstorm_id, card_id);

-- A workspace's WIP limits where they differ from the Brain Storming defaults (settings brain.wip_running and
-- brain.wip_checking). Running's limit is never below 1.
CREATE TABLE board_settings (
  workspace_id TEXT PRIMARY KEY REFERENCES workspaces(id) ON DELETE CASCADE,
  wip_running  INTEGER NOT NULL DEFAULT 1 CHECK (wip_running >= 1),
  wip_checking INTEGER NOT NULL DEFAULT 1 CHECK (wip_checking >= 1),
  updated_at   INTEGER NOT NULL
);
