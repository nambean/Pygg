-- API keys (client tokens), 2026-11-12, owner: a key must be viewable again, and a revoked key deletable.
--
-- token_ref: the OS credential-store entry (vault.rs, `client-…`) holding a copy of the token, so Endpoint → API Keys
-- can show it again on request. The store itself still keeps only the token's hash; a key made before this version
-- has no copy and cannot be shown. Revoking a key deletes its copy.
ALTER TABLE gateway_clients ADD COLUMN token_ref TEXT;

-- deleted_at: a revoked key the user deleted. It is no longer listed, and authentication already refuses it; the row
-- stays so the ledger's past requests keep the key's name.
ALTER TABLE gateway_clients ADD COLUMN deleted_at INTEGER;
