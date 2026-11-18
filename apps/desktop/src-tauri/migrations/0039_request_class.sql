-- Claude Code's gateway hint headers (cost plan A2, gateway/hints.rs): what kind of request each was (`main`,
-- `subagent`, `workflow`, `compaction`, `auxiliary`, or `other` for a value outside that list), the subagent that sent
-- it (its type and the random id Claude Code gives each spawn) and the user prompt it served (a random id). Fixed
-- vocabularies and random ids only, never prompt text. Empty for requests without the headers and rows from before.
ALTER TABLE usage_events ADD COLUMN request_class TEXT;
ALTER TABLE usage_events ADD COLUMN agent_type TEXT;
ALTER TABLE usage_events ADD COLUMN agent_id TEXT;
ALTER TABLE usage_events ADD COLUMN prompt_id TEXT;
