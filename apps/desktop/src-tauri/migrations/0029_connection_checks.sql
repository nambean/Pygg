-- Checks of a connection's state (owner, 2026-11-13: "we have no way to check the providers' state again"): reading an
-- API provider's model list (free, no prompt), or a web session's sign-in. Kept so the Providers screen shows the last
-- check after a restart, and so the health strip counts checks beside real requests. Rows older than seven days are
-- deleted as new ones come.
CREATE TABLE connection_checks (
  provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  at          INTEGER NOT NULL,
  ok          INTEGER NOT NULL CHECK (ok IN (0, 1)),
  kind        TEXT NOT NULL,
  detail      TEXT NOT NULL
);
CREATE INDEX connection_checks_by_provider ON connection_checks(provider_id, at);
