-- pygg/brain (ADR-041, docs/design/BRAIN.md): a brainstorm is a client's requirement, the planner that read it, and
-- the proposal (questions and cards with a model per card picked by rule) the user edits and approves in the app.
CREATE TABLE brainstorms (
  id TEXT PRIMARY KEY,
  client_id TEXT,
  session TEXT NOT NULL,
  workspace_id TEXT REFERENCES workspaces(id) ON DELETE SET NULL,
  state TEXT NOT NULL CHECK (state IN ('waiting_pick', 'thinking', 'proposed', 'approved', 'declined', 'failed')),
  requirement TEXT NOT NULL,
  planner TEXT,
  prompt_version TEXT NOT NULL DEFAULT 'brain-v1',
  revision INTEGER NOT NULL DEFAULT 0,
  proposal_json TEXT NOT NULL DEFAULT '{}',
  raw_reply TEXT,
  note TEXT,
  error TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX brainstorms_by_session ON brainstorms (client_id, session);

-- A pick the user saved for a role: `global`, or a workspace id for a project that differs (BRAIN.md 4.7b).
CREATE TABLE brain_picks (
  scope TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('plan', 'code', 'test', 'review')),
  provider_id TEXT NOT NULL,
  model_id TEXT NOT NULL,
  saved_at INTEGER NOT NULL,
  PRIMARY KEY (scope, role)
);

-- A task made from a card keeps which brainstorm and card it came from and the card's role (BRAIN.md 4.7c).
ALTER TABLE tasks ADD COLUMN brainstorm_id TEXT;
ALTER TABLE tasks ADD COLUMN card_id TEXT;
ALTER TABLE tasks ADD COLUMN card_role TEXT;

-- Whether a planner's reply fit the board schema (ADR-029): the own-evidence layer for the plan role. The level
-- column takes request, attempt or task (migration 0015); a brainstorm is recorded at the task level, its target
-- kind says `brainstorm`.
INSERT INTO metrics (name, type, level, optimize, description) VALUES
  ('brain_schema_ok', 'boolean', 'task', 'max', 'The planner answered pygg/brain in the board schema, every card kept (target: a brainstorm)');
