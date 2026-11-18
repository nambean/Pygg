-- `pygg run` launches and what they changed in Git (cost plan A1, launches.rs): one row per launch, written when the
-- tool starts and settled when it exits. The session key is the one the launch sends as `x-pygg-session`, so the
-- ledger's rows for the launch are `usage_events.session_key = launches.session_key`. Only the repository's folder,
-- branch and commit ids are kept, never file names or code. A launch outside a Git repository has no repository.
CREATE TABLE launches (
  session_key    TEXT PRIMARY KEY,
  tool           TEXT NOT NULL,
  model          TEXT,
  started_at     INTEGER NOT NULL,
  finished_at    INTEGER,
  exit_code      INTEGER,
  repo           TEXT,
  branch         TEXT,
  head_before    TEXT,
  head_after     TEXT,
  -- Tracked files with changes not committed when the tool started, and after it exited.
  dirty_before   INTEGER,
  dirty_after    INTEGER,
  -- Commits made from head_before to head_after (NULL when head_after does not descend from it).
  commits        INTEGER,
  -- head_before against the working tree after the tool exited: tracked files only, so what was already
  -- uncommitted at the start (dirty_before) is included.
  files_changed  INTEGER,
  insertions     INTEGER,
  deletions      INTEGER
);
CREATE INDEX launches_by_repo ON launches (repo, started_at);
-- A launch's receipt and the status line read one session's rows.
CREATE INDEX usage_by_session_key ON usage_events (session_key, created_at) WHERE session_key IS NOT NULL;
