-- Schema v10: approvals as ADR-010 describes them. An agent reports its cost only after a run (its own list-price
-- estimate), so each attempt keeps that estimate, and an approval's estimate is the range over the agent's recent
-- runs of the same model. approvals.policy_version records what the estimate was based on; when that changes before
-- the user decides, the approval is invalidated and asked again.
ALTER TABLE attempts ADD COLUMN agent_estimate_micro_usd INTEGER;
