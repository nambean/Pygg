-- Pools of API-key connections (ADR-034): connections of one provider, each with its own account and a quota the user
-- confirmed as separate, reached together as `<pool>/<model>` and tried in `pool_rank` order. NULL: in no pool.
ALTER TABLE providers ADD COLUMN pool TEXT;
ALTER TABLE providers ADD COLUMN pool_rank INTEGER;
