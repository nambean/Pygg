-- Schema v6: gateway lifecycle (LC-01).
-- The ownership generation that journaled an attempt. A gateway that takes the store starts a new generation;
-- every journal entry of an earlier generation is then known to be finished (its owner released the store only
-- after all its work was settled) or orphaned (its owner died), and becomes an interrupted ledger entry.
-- NULL for entries journaled before this column existed: treated as an earlier generation.
ALTER TABLE budget_reservations ADD COLUMN owner_generation TEXT;
