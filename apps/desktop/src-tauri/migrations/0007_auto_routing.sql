-- Schema v7: rule-based Auto routing (ROADMAP P0-5, ADR-006).

-- What auto routing needs to know about a model. NULL is unknown, and unknown counts as missing: a model is
-- only chosen for what it is known to support. The tier (T0 local … T4 frontier) uses the existing
-- `models.tier` (0–4) and `tier_source` columns from v1 and is always the user's; the other facts come from the
-- provider's model list where it reports them, or from the user.
ALTER TABLE models ADD COLUMN context_tokens INTEGER CHECK (context_tokens IS NULL OR context_tokens > 0);
ALTER TABLE models ADD COLUMN max_output_tokens INTEGER CHECK (max_output_tokens IS NULL OR max_output_tokens > 0);
ALTER TABLE models ADD COLUMN supports_tools INTEGER CHECK (supports_tools IS NULL OR supports_tools IN (0, 1));
ALTER TABLE models ADD COLUMN supports_vision INTEGER CHECK (supports_vision IS NULL OR supports_vision IN (0, 1));
-- 'user', or 'provider: <kind> /models' with the time it was read.
ALTER TABLE models ADD COLUMN caps_source TEXT;
ALTER TABLE models ADD COLUMN caps_observed_at INTEGER;

-- The lowest tier `auto` may choose for this client.
ALTER TABLE gateway_clients ADD COLUMN auto_min_tier TEXT NOT NULL DEFAULT 'T2'
  CHECK (auto_min_tier IN ('T0', 'T1', 'T2', 'T3', 'T4'));

-- Why the gateway chose this target, for auto routes.
ALTER TABLE usage_events ADD COLUMN route_reason TEXT;
