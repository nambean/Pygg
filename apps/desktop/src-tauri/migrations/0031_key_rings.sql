-- Several API keys on one connection, each from a separate account with its own quota, used in turn (ADR-038). A
-- connection's keys are its key ring: the key it was made with is the first, and each one added later was confirmed by
-- the user as another account's (`separate_account_at`), as a pool member is (ADR-034). Secrets stay in the system's
-- credential store; a row holds only the reference. Web sessions and sign-ins never have keys here (ADR-026, ADR-035).
CREATE TABLE provider_keys (
  id                   TEXT PRIMARY KEY,
  provider_id          TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  label                TEXT NOT NULL,
  credential_ref       TEXT NOT NULL,
  rank                 INTEGER NOT NULL,
  enabled              INTEGER NOT NULL DEFAULT 1,
  separate_account_at  INTEGER,
  created_at           INTEGER NOT NULL
);
CREATE INDEX provider_keys_by_provider ON provider_keys(provider_id, rank);

-- A key's cooldowns and blocks, as `connection_health` is a connection's: a 401 blocks the key until the user fixes it,
-- a 402 or a 429 cools it (one model, or the whole key); the connection's other keys go on.
CREATE TABLE key_health (
  key_id       TEXT NOT NULL REFERENCES provider_keys(id) ON DELETE CASCADE,
  model_id     TEXT NOT NULL,
  state        TEXT NOT NULL,
  reason       TEXT NOT NULL,
  http_status  INTEGER,
  until_ms     INTEGER,
  updated_at   INTEGER NOT NULL,
  PRIMARY KEY (key_id, model_id)
);

-- The rate-limit headers each key's account last sent, per model, for the quota-aware order (quota.rs reads them).
CREATE TABLE key_limits (
  key_id        TEXT NOT NULL REFERENCES provider_keys(id) ON DELETE CASCADE,
  model_id      TEXT NOT NULL,
  headers_json  TEXT NOT NULL,
  http_status   INTEGER,
  observed_at   INTEGER NOT NULL,
  PRIMARY KEY (key_id, model_id)
);

-- How a connection orders its keys for a new request: in_order, round_robin, least_used or quota_aware; and where
-- round_robin starts next.
ALTER TABLE providers ADD COLUMN key_strategy TEXT NOT NULL DEFAULT 'in_order';
ALTER TABLE providers ADD COLUMN key_cursor INTEGER NOT NULL DEFAULT 0;

-- Which key sent each attempt (NULL: the connection's only key, or before this migration).
ALTER TABLE usage_events ADD COLUMN key_id TEXT;
CREATE INDEX usage_events_by_key ON usage_events(key_id, created_at) WHERE key_id IS NOT NULL;
