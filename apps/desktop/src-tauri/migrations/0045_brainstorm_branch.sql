-- A brainstorm's working branch (ADR-042, amended 2026-11-21): made at the workspace's HEAD when the brainstorm is
-- approved onto a board. Each card's task starts from its tip, and keeping a card's result adds its commit to it, so
-- a later card sees what the earlier ones built. NULL for brainstorms approved before this column existed.
ALTER TABLE brainstorms ADD COLUMN branch TEXT;
