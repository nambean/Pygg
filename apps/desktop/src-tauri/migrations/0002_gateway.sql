-- Schema v2: the local gateway (ADR-016). Client tokens are stored only as SHA-256 hashes; the token itself
-- is shown once when it is created.

CREATE TABLE gateway_clients (
  id                 TEXT PRIMARY KEY,
  name               TEXT NOT NULL,
  token_hash         TEXT NOT NULL UNIQUE,
  -- The first characters, so the user can tell tokens apart without the secret.
  token_prefix       TEXT NOT NULL,
  -- Spending limit per calendar month (UTC) on known costs; NULL means no limit.
  monthly_budget_micro_usd INTEGER CHECK (monthly_budget_micro_usd IS NULL OR monthly_budget_micro_usd > 0),
  created_at         INTEGER NOT NULL,
  last_used_at       INTEGER,
  revoked_at         INTEGER
);

-- A named, ordered list of models. Requests for the combo's name try each member in order.
CREATE TABLE combos (
  id         TEXT PRIMARY KEY,
  name       TEXT NOT NULL UNIQUE,
  created_at INTEGER NOT NULL
);

CREATE TABLE combo_members (
  combo_id    TEXT NOT NULL REFERENCES combos(id) ON DELETE CASCADE,
  position    INTEGER NOT NULL,
  provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  model_id    TEXT NOT NULL,
  PRIMARY KEY (combo_id, position)
);

-- Billing type for models of this connection that have no row of their own in `models`.
ALTER TABLE providers ADD COLUMN default_billing TEXT NOT NULL DEFAULT 'unknown'
  CHECK (default_billing IN ('free', 'local', 'subscription', 'paid', 'unknown'));

-- What happened to each upstream request, so failed attempts and fallbacks are visible in Usage.
ALTER TABLE usage_events ADD COLUMN outcome TEXT NOT NULL DEFAULT 'ok';
ALTER TABLE usage_events ADD COLUMN http_status INTEGER;
ALTER TABLE usage_events ADD COLUMN latency_ms INTEGER;
ALTER TABLE usage_events ADD COLUMN requested_model TEXT;
ALTER TABLE usage_events ADD COLUMN error TEXT;
CREATE INDEX usage_by_client ON usage_events (client_id, created_at);
