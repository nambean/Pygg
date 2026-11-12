-- Web-session connections (ADR-035): an AI site with no API, used through a Pygg window the user signs in to.
-- web_profile: the name of the connection's own WebView2 profile folder under `websessions/`, random so a new
--   connection never reuses a removed one's folder. NULL for every other connection.
-- web_ack_at / web_ack_version: when the user acknowledged the account-risk warning, and which text of it.
-- web_state: `signed_in`, `signed_out`, `needs_you` (a check or challenge the user must deal with), or
--   `page_changed`; web_state_reason is Pygg's own wording, never text from the page.
ALTER TABLE providers ADD COLUMN web_profile TEXT;
ALTER TABLE providers ADD COLUMN web_ack_at INTEGER;
ALTER TABLE providers ADD COLUMN web_ack_version INTEGER;
ALTER TABLE providers ADD COLUMN web_state TEXT;
ALTER TABLE providers ADD COLUMN web_state_reason TEXT;
ALTER TABLE providers ADD COLUMN web_state_at INTEGER;
