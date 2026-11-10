-- Schema v3: cost-safe gateway parity (ROADMAP P0-1..P0-4, ADR-025).

-- P0-1: the wire format a connection speaks. Requests in the same format are passed through unchanged;
-- other pairs are translated only where a contract test exists.
ALTER TABLE providers ADD COLUMN wire TEXT NOT NULL DEFAULT 'openai' CHECK (wire IN ('openai', 'anthropic'));

-- P0-2: connections that share a provider's rate limits and terms. NULL means the provider kind, so two keys
-- of the same provider are assumed to share limits until the user marks one as a separate organization.
ALTER TABLE providers ADD COLUMN quota_group TEXT;

CREATE TABLE connection_health (
  provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  -- '' for the whole connection.
  model_id    TEXT NOT NULL DEFAULT '',
  state       TEXT NOT NULL CHECK (state IN ('invalid_credentials', 'forbidden', 'payment_required', 'rate_limited',
                                             'model_unavailable', 'server_error', 'unreachable')),
  reason      TEXT NOT NULL,
  http_status INTEGER,
  -- Unix ms when the connection may be tried again; NULL means until the user fixes it.
  until_ms    INTEGER,
  updated_at  INTEGER NOT NULL,
  PRIMARY KEY (provider_id, model_id)
);

-- P0-3: per-client concurrency and what to do with a target whose price is unknown.
ALTER TABLE gateway_clients ADD COLUMN max_concurrency INTEGER NOT NULL DEFAULT 8 CHECK (max_concurrency BETWEEN 1 AND 256);
ALTER TABLE gateway_clients ADD COLUMN unknown_cost_policy TEXT NOT NULL DEFAULT 'block'
  CHECK (unknown_cost_policy IN ('block', 'allow'));

-- Spend held for requests in flight, so concurrent requests cannot pass a limit together.
CREATE TABLE budget_reservations (
  id             TEXT PRIMARY KEY,
  client_id      TEXT NOT NULL REFERENCES gateway_clients(id) ON DELETE CASCADE,
  reserved_micro INTEGER NOT NULL CHECK (reserved_micro >= 0),
  created_at     INTEGER NOT NULL
);
CREATE INDEX reservations_by_client ON budget_reservations (client_id);

-- P0-4: the model a client's explicit session is pinned to.
CREATE TABLE session_affinity (
  client_id       TEXT NOT NULL REFERENCES gateway_clients(id) ON DELETE CASCADE,
  session_key     TEXT NOT NULL,
  requested_model TEXT NOT NULL,
  provider_id     TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  model_id        TEXT NOT NULL,
  created_at      INTEGER NOT NULL,
  last_used_at    INTEGER NOT NULL,
  PRIMARY KEY (client_id, session_key, requested_model)
);

ALTER TABLE usage_events ADD COLUMN cache_write_tokens INTEGER;
ALTER TABLE usage_events ADD COLUMN session_key TEXT;
-- 'native' (same format passed through) or 'translated:<from>-><to>'.
ALTER TABLE usage_events ADD COLUMN route_mode TEXT;
