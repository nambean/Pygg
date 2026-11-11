-- Review GW-8 (ROADMAP B6): Gemini tool-call signatures kept across a restart. Gemini 3 refuses a request whose
-- history holds one of its tool calls without the signature it gave; the gateway remembered them in memory only,
-- so a conversation that went on after a restart got Google's placeholder instead. Kept 30 days.
CREATE TABLE tool_signatures (
  call_id    TEXT PRIMARY KEY,
  signature  TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE INDEX tool_signatures_by_age ON tool_signatures (created_at);
