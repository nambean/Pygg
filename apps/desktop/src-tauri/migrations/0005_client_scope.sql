-- Schema v5: answers to the follow-up review (docs/reviews/2026-11-11-remaining-gateway-risks-response.md).

-- FR-01: the connections a client token may send requests to. NULL means every connection; otherwise a JSON
-- array of connection ids, and every other connection is skipped for this client, whatever the combo says.
ALTER TABLE gateway_clients ADD COLUMN allowed_connections TEXT;

-- FR-02: how many further targets may be tried after a failure the provider may have billed (an HTTP 5xx).
-- 0, the default, stops there: a possible double charge needs an explicit opt-in.
ALTER TABLE gateway_clients ADD COLUMN paid_retries INTEGER NOT NULL DEFAULT 0 CHECK (paid_retries BETWEEN 0 AND 5);
