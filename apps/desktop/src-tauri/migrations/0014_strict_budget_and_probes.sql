-- Schema v14 (ROADMAP B4). A strict client limit holds each request at the most it could cost (its own output limit,
-- or the model's maximum output) and never uses a model whose price is unknown. Connection health counts repeated
-- failures of one kind, so a connection that keeps failing waits longer each time (a circuit breaker), and a probe
-- of the provider's free model list can lift an outage or a rejected key early.
ALTER TABLE gateway_clients ADD COLUMN strict_budget INTEGER NOT NULL DEFAULT 0 CHECK (strict_budget IN (0, 1));
ALTER TABLE connection_health ADD COLUMN strikes INTEGER NOT NULL DEFAULT 1;
