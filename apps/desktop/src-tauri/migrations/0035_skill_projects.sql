-- Skills & Prompts (ADR-039): project folders the user added for skills. Pygg installs into a project's own
-- .claude/skills or .agents/skills only when the folder is listed here, and reads the project's instruction files and
-- skill folders to show what each agent adds to every request there. What is installed is read from the folders, never
-- recorded here.
CREATE TABLE skill_projects (
    path TEXT PRIMARY KEY,
    added_at INTEGER NOT NULL
);
