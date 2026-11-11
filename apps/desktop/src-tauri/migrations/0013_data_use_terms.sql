-- Schema v13: data-use terms as a filter (ROADMAP B2, review GW-10). providers.data_use_terms, terms_source and
-- terms_checked_at exist since v1; the user records them per connection. A client may then be limited to connections
-- whose terms say the provider does not train on what is sent. Unknown counts as may-train.
ALTER TABLE gateway_clients ADD COLUMN require_no_training INTEGER NOT NULL DEFAULT 0 CHECK (require_no_training IN (0, 1));
