-- A daily request limit per web-session connection (owner, 2026-11-13, after reading why 9router users are banned:
-- bursts of automated requests trip the sites' abuse detection). NULL uses Pygg's default (websession::DEFAULT_DAILY_LIMIT).
-- Counted from the ledger: requests sent to the site since 00:00 UTC.
ALTER TABLE providers ADD COLUMN web_daily_limit INTEGER CHECK (web_daily_limit IS NULL OR web_daily_limit BETWEEN 1 AND 1000);
