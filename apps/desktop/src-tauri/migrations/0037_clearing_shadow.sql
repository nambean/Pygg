-- Clearing old tool results, measured in shadow (ROADMAP item 4): for an OpenAI-format client's request, what Anthropic's
-- context editing would clear at its defaults (past 100,000 prompt tokens, all but the last three tool results). How many
-- results, their estimated tokens, and the estimated tokens from the newest cleared result to the end, which a new clearing
-- would make the provider write to its cache again. Nothing is cleared. Empty for requests without tool output, Anthropic-
-- format requests, and rows from before this migration.
ALTER TABLE usage_events ADD COLUMN clear_count INTEGER;
ALTER TABLE usage_events ADD COLUMN clear_tokens INTEGER;
ALTER TABLE usage_events ADD COLUMN clear_tail_tokens INTEGER;
