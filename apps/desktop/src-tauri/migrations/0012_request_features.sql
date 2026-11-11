-- Schema v12: request features as auto-routing filters (ROADMAP B1). Which thinking types (adaptive, enabled) and effort
-- levels a model accepts, as JSON arrays, from the provider's model list (Anthropic's capabilities) or the user. NULL is
-- unknown, and unknown counts as missing: a request that asks for the feature is not sent there.
ALTER TABLE models ADD COLUMN thinking_types TEXT;
ALTER TABLE models ADD COLUMN effort_levels TEXT;
