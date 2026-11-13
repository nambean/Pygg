-- The web guard (ADR-035, owner 2026-11-13, after DeepSeek suspended the owner's account following a stream of long
-- prompts): each request a web session sends is kept with its weight (a unit, and one more per 2,000 characters of
-- prompt), so the guard can cap an account's hour and day by weight, and an account's rest and back-off survive a
-- restart of the app. Rows older than two days are deleted as new ones come.
CREATE TABLE web_pace (
  provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  at          INTEGER NOT NULL,
  weight      INTEGER NOT NULL CHECK (weight >= 1),
  outcome     TEXT NOT NULL
);
CREATE INDEX web_pace_by_provider ON web_pace(provider_id, at);
-- When the rest after the last reply ends; back-off after a sign of trouble (until when, why, how many in a row).
ALTER TABLE providers ADD COLUMN web_next_at INTEGER;
ALTER TABLE providers ADD COLUMN web_backoff_until INTEGER;
ALTER TABLE providers ADD COLUMN web_backoff_reason TEXT;
ALTER TABLE providers ADD COLUMN web_strikes INTEGER NOT NULL DEFAULT 0;
