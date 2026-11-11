-- Schema v8: task engine v1 (ROADMAP P1-1). A task is a ladder of agent steps fixed when it is created
-- (tasks.policy_snapshot_json). Each attempt runs one step in its own worktree and is judged by the
-- workspace's approved checks; a failed attempt hands a bounded summary of why to the next step.

-- While running: preparing, generating or checking.
ALTER TABLE tasks ADD COLUMN phase TEXT;
-- Why the task is where it is: the last failure, a declined paid step, the app closing while it ran.
ALTER TABLE tasks ADD COLUMN reason TEXT;
-- The user's decision on a finished result: 'kept' (as result_branch) or 'discarded'.
ALTER TABLE tasks ADD COLUMN result TEXT CHECK (result IS NULL OR result IN ('kept', 'discarded'));
ALTER TABLE tasks ADD COLUMN result_branch TEXT;

-- The step of the ladder this attempt ran (0-based); attempts.number keeps counting across retries.
ALTER TABLE attempts ADD COLUMN rung INTEGER NOT NULL DEFAULT 0;
ALTER TABLE attempts ADD COLUMN base_revision TEXT;
-- The model the agent reported using, which may differ from the one asked for.
ALTER TABLE attempts ADD COLUMN agent_model TEXT;
-- How the agent run ended: 'exit 0', 'timed out', the agent's own error.
ALTER TABLE attempts ADD COLUMN detail TEXT;
ALTER TABLE attempts ADD COLUMN files_changed INTEGER;
ALTER TABLE attempts ADD COLUMN insertions INTEGER;
ALTER TABLE attempts ADD COLUMN deletions INTEGER;
-- The agent's final message, and the redacted end of its output.
ALTER TABLE attempts ADD COLUMN summary TEXT;
ALTER TABLE attempts ADD COLUMN log_tail TEXT;

CREATE INDEX usage_by_attempt ON usage_events (attempt_id);
