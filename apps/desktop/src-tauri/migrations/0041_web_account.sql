-- The account a web session's page says is signed in (an email or a name), read at each sign-in check where the
-- site's page carries it without a click (ADR-035). NULL where the site does not show it, or once signed out.
ALTER TABLE providers ADD COLUMN web_account TEXT;
