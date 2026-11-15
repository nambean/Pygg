-- Skills & Prompts (ADR-039): the user's own skills, kept in the store so a backup holds them. Pygg's starter skills
-- live in the code (skills.rs) and are never rows here. A skill is installed by writing its SKILL.md into an agent's
-- own skills folder; what is installed where is read from those folders, never recorded here.
CREATE TABLE skills (
  id           TEXT PRIMARY KEY,
  description  TEXT NOT NULL,
  body         TEXT NOT NULL,
  manual       INTEGER NOT NULL DEFAULT 1,
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL
);
