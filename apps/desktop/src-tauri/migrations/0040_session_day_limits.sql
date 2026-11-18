-- Cost plan A3: money limits on an API key per routing session (the `x-pygg-session` a request names) and per UTC day,
-- next to the monthly one. NULL is off. They use the same holds as the monthly limit (`budget_reservations`), so a
-- request is refused before it is sent and one already running is never cut.
ALTER TABLE gateway_clients ADD COLUMN session_budget_micro_usd INTEGER;
ALTER TABLE gateway_clients ADD COLUMN day_budget_micro_usd INTEGER;
