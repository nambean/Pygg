-- Why an answer read nothing from the provider's prompt cache (ROADMAP item 4), worked out from the ledger when the
-- attempt is settled: not_reported, below_minimum, marker_dropped, first_request, model_switched, not_cached_here,
-- cache_expired, prefix_changed or no_session. Empty for a cache hit, a failed attempt, an answer without token counts
-- and rows from before this migration. A reason is the ledger's best reading, never the provider's word.
ALTER TABLE usage_events ADD COLUMN cache_miss TEXT;
CREATE INDEX usage_by_session ON usage_events (client_id, session_key, created_at) WHERE session_key IS NOT NULL;
