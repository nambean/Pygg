-- PV-06, the slice that costs nothing (ADR-030): the rate-limit headers a provider last sent for a connection and
-- model (anthropic-ratelimit-*, x-ratelimit-*, retry-after), kept as sent. Shown as "as the provider last said";
-- nothing reserves or routes on them, and a provider that sends none has no row (unknown stays unknown).
CREATE TABLE rate_limits (
  provider_id  TEXT NOT NULL,
  model_id     TEXT NOT NULL,
  headers_json TEXT NOT NULL,
  http_status  INTEGER,
  observed_at  INTEGER NOT NULL,
  PRIMARY KEY (provider_id, model_id)
);
