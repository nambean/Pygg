-- Combo strategies, 2026-11-12 (Combos review with Codex, after 9router and OmniRoute).
--
-- order:    the members in the order the user set (what every combo did before).
-- cheapest: for each request, the members by that request's estimated cost; free, local and plan members count as
--           $0, and a member whose price is unknown goes last. The fallback rules stay the same.
ALTER TABLE combos ADD COLUMN strategy TEXT NOT NULL DEFAULT 'order' CHECK (strategy IN ('order', 'cheapest'));
