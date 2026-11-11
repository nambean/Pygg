-- H1 (ADR-030): what a task's next step is given when an earlier attempt failed. No new tables: the attempt rows
-- already hold the checks, the agent's final message and why it failed. Two things were missing:
--   handoff_sent        the text actually added to this attempt's prompt, so the user (and pygg_task_status) can
--                       see what the agent received; NULL when it was given the task alone.
--   changed_paths_json  the paths a discarded attempt changed (git diff --name-only before its worktree goes), so a
--                       later step can be told what was tried and that none of it is in its copy.
ALTER TABLE attempts ADD COLUMN handoff_sent TEXT;
ALTER TABLE attempts ADD COLUMN changed_paths_json TEXT;
