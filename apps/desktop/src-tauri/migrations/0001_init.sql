-- Schema v1 (plan §5). Times are Unix milliseconds. Money is integer micro-USD (1 USD = 1 000 000), so the
-- ledger never accumulates floating-point error. Secrets never appear here: providers.credential_ref names
-- an OS credential store entry (src/vault.rs).

CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE workspaces (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  path          TEXT NOT NULL UNIQUE,
  privacy_class TEXT NOT NULL DEFAULT 'standard' CHECK (privacy_class IN ('standard', 'sensitive')),
  -- Git-only writes in MVP (ADR-008): anything that cannot be isolated is read-only.
  isolation     TEXT NOT NULL DEFAULT 'git_worktree' CHECK (isolation IN ('git_worktree', 'read_only')),
  created_at    INTEGER NOT NULL
);

CREATE TABLE workspace_checks (
  id           TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  command      TEXT NOT NULL,
  -- Where it was found, e.g. 'package.json' or 'user'.
  source       TEXT NOT NULL,
  -- Detected checks are suggestions until the user approves them.
  approved     INTEGER NOT NULL DEFAULT 0 CHECK (approved IN (0, 1)),
  UNIQUE (workspace_id, command)
);

CREATE TABLE providers (
  id               TEXT PRIMARY KEY,
  kind             TEXT NOT NULL,
  account_label    TEXT NOT NULL,
  endpoint         TEXT,
  enabled          INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  credential_ref   TEXT,
  data_use_terms   TEXT NOT NULL DEFAULT 'unknown' CHECK (data_use_terms IN ('no_training', 'may_train', 'unknown')),
  terms_source     TEXT,
  terms_checked_at INTEGER,
  created_at       INTEGER NOT NULL
);

CREATE TABLE models (
  id                TEXT PRIMARY KEY,
  provider_id       TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  model_id          TEXT NOT NULL,
  billing           TEXT NOT NULL DEFAULT 'unknown'
                    CHECK (billing IN ('free', 'local', 'subscription', 'paid', 'unknown')),
  tier              INTEGER CHECK (tier BETWEEN 0 AND 4),
  -- 'curated:v1', 'user' or NULL when unassigned.
  tier_source       TEXT,
  -- Each capability with its own provenance (ADR-011).
  capabilities_json TEXT NOT NULL DEFAULT '{}',
  price_json        TEXT,
  observed_at       INTEGER,
  enabled           INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  UNIQUE (provider_id, model_id)
);

CREATE TABLE tasks (
  id                   TEXT PRIMARY KEY,
  workspace_id         TEXT NOT NULL REFERENCES workspaces(id),
  title                TEXT NOT NULL,
  prompt               TEXT NOT NULL,
  strategy             TEXT NOT NULL,
  state                TEXT NOT NULL,
  base_revision        TEXT,
  -- Immutable snapshots keep history reproducible.
  policy_snapshot_json TEXT NOT NULL DEFAULT '{}',
  created_at           INTEGER NOT NULL,
  updated_at           INTEGER NOT NULL
);
CREATE INDEX tasks_by_workspace ON tasks (workspace_id, updated_at);

CREATE TABLE task_stages (
  id         TEXT PRIMARY KEY,
  task_id    TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  position   INTEGER NOT NULL,
  name       TEXT NOT NULL,
  check_spec TEXT,
  state      TEXT NOT NULL,
  UNIQUE (task_id, position)
);

CREATE TABLE attempts (
  id                 TEXT PRIMARY KEY,
  stage_id           TEXT NOT NULL REFERENCES task_stages(id) ON DELETE CASCADE,
  number             INTEGER NOT NULL,
  agent              TEXT NOT NULL,
  -- "Provider / Account · Model" identity, fixed once the attempt starts.
  model_ref          TEXT NOT NULL,
  tier               INTEGER NOT NULL CHECK (tier BETWEEN 0 AND 4),
  billing            TEXT NOT NULL,
  worktree_path      TEXT,
  state              TEXT NOT NULL,
  check_results_json TEXT,
  escalation_reason  TEXT,
  started_at         INTEGER,
  finished_at        INTEGER,
  UNIQUE (stage_id, number)
);

CREATE TABLE route_decisions (
  id                 TEXT PRIMARY KEY,
  stage_id           TEXT NOT NULL REFERENCES task_stages(id) ON DELETE CASCADE,
  eligible_json      TEXT NOT NULL,
  excluded_json      TEXT NOT NULL,
  deciding_rule      TEXT,
  chosen             TEXT,
  created_at         INTEGER NOT NULL
);

CREATE TABLE usage_events (
  id             TEXT PRIMARY KEY,
  -- NULL for gateway-only traffic, which has a client instead of a task.
  attempt_id     TEXT REFERENCES attempts(id),
  client_id      TEXT,
  provider       TEXT NOT NULL,
  model          TEXT NOT NULL,
  input_tokens   INTEGER,
  output_tokens  INTEGER,
  cache_tokens   INTEGER,
  cost_micro_usd INTEGER,
  -- Unknown cost is never zero (ADR-011): cost_micro_usd is NULL then.
  cost_status    TEXT NOT NULL CHECK (cost_status IN ('actual', 'estimated', 'unknown')),
  source         TEXT NOT NULL,
  created_at     INTEGER NOT NULL,
  CHECK (cost_status = 'unknown' OR cost_micro_usd IS NOT NULL)
);
CREATE INDEX usage_by_time ON usage_events (created_at);

CREATE TABLE approvals (
  id                  TEXT PRIMARY KEY,
  attempt_id          TEXT NOT NULL REFERENCES attempts(id) ON DELETE CASCADE,
  cap_micro_usd       INTEGER NOT NULL,
  estimate_low_micro  INTEGER,
  estimate_high_micro INTEGER,
  policy_version      TEXT NOT NULL,
  state               TEXT NOT NULL CHECK (state IN ('pending', 'approved', 'denied', 'invalidated')),
  created_at          INTEGER NOT NULL,
  decided_at          INTEGER
);

CREATE TABLE audit_events (
  id            TEXT PRIMARY KEY,
  actor         TEXT NOT NULL,
  action        TEXT NOT NULL,
  entity        TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at    INTEGER NOT NULL
);
