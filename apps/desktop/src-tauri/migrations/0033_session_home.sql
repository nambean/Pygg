-- The way home (ROADMAP item 4): a pinned session remembers the target it was meant for, its home, and when the home
-- last answered it. After a fallback the session goes back home once the home's prompt cache is still warm or the
-- fallback's has gone cold too, so one passing refusal no longer keeps it away for the pin's two hours. Rows from
-- before this migration have no home; the next request of the session gives them one.
ALTER TABLE session_affinity ADD COLUMN home_provider_id TEXT;
ALTER TABLE session_affinity ADD COLUMN home_model_id TEXT;
ALTER TABLE session_affinity ADD COLUMN home_used_at INTEGER;
