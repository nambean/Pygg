-- ROADMAP C4 (ADR-029): quality signals from what happens to a result, in TensorZero's metric and feedback shape
-- (TensorZero itself is archived; only its data model is borrowed). A signal that is not known has no row: it is
-- never recorded as 0 or false.

CREATE TABLE metrics (
  name        TEXT PRIMARY KEY,
  type        TEXT NOT NULL CHECK (type IN ('boolean', 'float')),
  level       TEXT NOT NULL CHECK (level IN ('request', 'attempt', 'task')),
  optimize    TEXT NOT NULL CHECK (optimize IN ('max', 'min')),
  description TEXT NOT NULL
);

INSERT INTO metrics (name, type, level, optimize, description) VALUES
  ('checks_passed', 'boolean', 'attempt', 'max', 'The approved checks passed on this attempt'),
  ('result_kept', 'boolean', 'task', 'max', 'The user kept the result as a branch or a patch (false: discarded it)'),
  ('retried', 'boolean', 'task', 'min', 'The user ran an interrupted task again'),
  ('cancelled', 'boolean', 'task', 'min', 'The user stopped the task'),
  ('declined', 'boolean', 'attempt', 'min', 'The user declined a step that waited for approval'),
  ('rework_minutes', 'float', 'task', 'min', 'Minutes the user says they spent fixing the result');

CREATE TABLE feedback (
  id          TEXT PRIMARY KEY,
  metric_name TEXT NOT NULL REFERENCES metrics (name),
  target_kind TEXT NOT NULL CHECK (target_kind IN ('request', 'attempt', 'task')),
  -- A usage_events, attempts or tasks id.
  target_id   TEXT NOT NULL,
  value_bool  INTEGER CHECK (value_bool IS NULL OR value_bool IN (0, 1)),
  value_float REAL,
  source      TEXT NOT NULL CHECK (source IN ('check', 'user', 'engine', 'client')),
  tags_json   TEXT,
  created_at  INTEGER NOT NULL,
  CHECK (value_bool IS NOT NULL OR value_float IS NOT NULL)
);

CREATE INDEX feedback_by_target ON feedback (target_kind, target_id);
CREATE INDEX feedback_by_metric ON feedback (metric_name, created_at);
