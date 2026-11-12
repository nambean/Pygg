-- N6 (live paid-step approval, 2026-11-12): a paid step whose agent sends its requests through Pygg's own gateway is
-- priced by Pygg's ledger, not by the agent. OpenCode, pointed at the gateway as a custom provider, estimated such a
-- run at $0.0000, and the cap check took that as the cost. attempts.agent_estimate_micro_usd keeps its name and now
-- holds the step's cost as Pygg knows it; this column says where it came from:
--   cost_source  'pygg'   the sum of the gateway's own entries tagged with this attempt (its session header), all
--                         of them priced; NULL cost when any of them had no price
--                'agent'  the agent's own list-price estimate, for requests that did not go through Pygg
--                NULL     runs recorded before v19 (the agent's estimate)
ALTER TABLE attempts ADD COLUMN cost_source TEXT;
