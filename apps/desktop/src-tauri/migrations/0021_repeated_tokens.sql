-- P1-4 in shadow: about how many tokens of tool output a request repeated from earlier in itself. Measured, never
-- removed: deduplicating would rewrite an agent's history and break the prompt cache (ROADMAP P1-4). NULL: the
-- request had no tool output, or was recorded before this column.
ALTER TABLE usage_events ADD COLUMN repeated_tokens INTEGER;
