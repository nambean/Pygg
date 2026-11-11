-- Schema v11: daily budgets for smart tasks (ROADMAP A3). Only paid steps count; the overall daily limit is the
-- setting 'tasks.daily_budget_micro_usd', and a workspace may have its own on top. Days are UTC.
ALTER TABLE workspaces ADD COLUMN daily_budget_micro_usd INTEGER;
