-- A web session's requests cost $0 by default (owner, 2026-11-13): the site is used on the user's own plan, paid by
-- the month or free, never per request, so the ledger records each request at $0 with the source "web session".
-- Subscription stays as a choice for a web session, counting its requests as quota (cost Unknown, as before).
-- Every web session so far was 'subscription', the only value allowed until now, so none of them was the user's choice.
UPDATE providers SET default_billing = 'free'
 WHERE default_billing = 'subscription' AND kind IN ('muse-web', 'chatgpt-web', 'perplexity-web', 'deepseek-web', 'grok-web');
