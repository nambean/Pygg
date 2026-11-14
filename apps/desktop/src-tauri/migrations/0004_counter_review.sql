-- Schema v4: answers to the architect counter-review (2026-11-11).

-- CR-02: when the user confirmed that a connection has its own provider quota. NULL with a quota group set
-- means a confirmation from before this column existed; it is backfilled with the connection's creation time.
ALTER TABLE providers ADD COLUMN quota_confirmed_at INTEGER;
UPDATE providers SET quota_confirmed_at = created_at WHERE quota_group IS NOT NULL;

-- CR-06: every attempt is journaled before it is sent, not only those that hold spend, so an attempt the
-- gateway never settled (a crash, a killed process) still becomes a ledger entry.
ALTER TABLE budget_reservations ADD COLUMN provider_id TEXT;
ALTER TABLE budget_reservations ADD COLUMN model_id TEXT;
ALTER TABLE budget_reservations ADD COLUMN requested_model TEXT;
ALTER TABLE budget_reservations ADD COLUMN session_key TEXT;
ALTER TABLE budget_reservations ADD COLUMN route_mode TEXT;

-- CR-03, CR-06: spend that may have been charged but cannot be stated (a server error, a dropped stream, an
-- interrupted attempt). Counted against a client's limit at the amount that was held for it; the cost itself
-- stays unknown (cost_micro_usd NULL).
ALTER TABLE usage_events ADD COLUMN at_risk_micro_usd INTEGER CHECK (at_risk_micro_usd IS NULL OR at_risk_micro_usd >= 0);

-- CR-07: expired session pins are deleted; this index keeps that cheap.
CREATE INDEX sessions_by_last_use ON session_affinity (last_used_at);
