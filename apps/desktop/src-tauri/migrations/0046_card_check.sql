-- A planned card's check (ROADMAP focus item 6, the narrow planner): the command, proposed by the planner and
-- seen and edited by the user before the plan is approved, that its task runs after the workspace's approved
-- checks. NULL: nothing tests the card, a person reviews it.
ALTER TABLE board_cards ADD COLUMN check_command TEXT;
