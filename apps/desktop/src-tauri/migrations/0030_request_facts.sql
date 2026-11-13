-- What each attempt was, as it was when it was sent (Codex's fifth discussion, T2; fourth review, B2 and B3): the
-- request it belongs to and its place among that request's attempts, so fallbacks read as one request ("answered after
-- fallback"); the billing type it was sent under, so later changes to settings do not rewrite history; how the route was
-- chosen (auto, auto/cheap, a combo, a pool, a model by name, and whether a session pin chose it); and what happened to
-- the client's prompt-cache hints (kept, translated, dropped, none). Older rows keep these empty: "not recorded".
ALTER TABLE usage_events ADD COLUMN request_id TEXT;
ALTER TABLE usage_events ADD COLUMN attempt INTEGER;
ALTER TABLE usage_events ADD COLUMN billing TEXT;
ALTER TABLE usage_events ADD COLUMN route_kind TEXT;
ALTER TABLE usage_events ADD COLUMN cache_hint TEXT;
CREATE INDEX usage_events_by_request ON usage_events(request_id) WHERE request_id IS NOT NULL;
