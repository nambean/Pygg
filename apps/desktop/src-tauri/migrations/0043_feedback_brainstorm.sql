-- The brain's quality signal (ADR-029, ADR-041) could not be written: feedback's target kind takes request, attempt or
-- task, and its source check, user, engine or client (migration 0015). Seen on the first live run of pygg/brain,
-- 2026-11-19. SQLite cannot widen a CHECK in place, so the table is rebuilt with `brainstorm` and `brain` allowed; the
-- rows and the indexes are kept.
CREATE TABLE feedback_new (
  id          TEXT PRIMARY KEY,
  metric_name TEXT NOT NULL REFERENCES metrics (name),
  target_kind TEXT NOT NULL CHECK (target_kind IN ('request', 'attempt', 'task', 'brainstorm')),
  -- A usage_events, attempts, tasks or brainstorms id.
  target_id   TEXT NOT NULL,
  value_bool  INTEGER CHECK (value_bool IS NULL OR value_bool IN (0, 1)),
  value_float REAL,
  source      TEXT NOT NULL CHECK (source IN ('check', 'user', 'engine', 'client', 'brain')),
  tags_json   TEXT,
  created_at  INTEGER NOT NULL,
  CHECK (value_bool IS NOT NULL OR value_float IS NOT NULL)
);
INSERT INTO feedback_new SELECT id, metric_name, target_kind, target_id, value_bool, value_float, source, tags_json, created_at FROM feedback;
DROP INDEX IF EXISTS feedback_by_target;
DROP INDEX IF EXISTS feedback_by_metric;
DROP TABLE feedback;
ALTER TABLE feedback_new RENAME TO feedback;
CREATE INDEX feedback_by_target ON feedback (target_kind, target_id);
CREATE INDEX feedback_by_metric ON feedback (metric_name, created_at);
