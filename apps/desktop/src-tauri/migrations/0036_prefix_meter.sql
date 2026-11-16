-- The tool definitions and system prompt each request carried (ROADMAP item 4): their estimated size in tokens and a
-- 64-bit fingerprint of them, so a session's first request shows what it carries and a later change can be counted.
-- Numbers only: the text is never stored. Empty for a request that carried neither, and for rows from before this
-- migration.
ALTER TABLE usage_events ADD COLUMN tools_tokens INTEGER;
ALTER TABLE usage_events ADD COLUMN system_tokens INTEGER;
ALTER TABLE usage_events ADD COLUMN prefix_hash INTEGER;
