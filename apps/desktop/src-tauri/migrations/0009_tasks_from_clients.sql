-- Schema v9: smart tasks started from where people already work (an IDE agent through the MCP server, a
-- script), through the gateway's task API. A client may start tasks only when the user allows it; tasks
-- remember the client that started them, and a client sees only its own.
ALTER TABLE gateway_clients ADD COLUMN may_run_tasks INTEGER NOT NULL DEFAULT 0 CHECK (may_run_tasks IN (0, 1));
ALTER TABLE tasks ADD COLUMN client_id TEXT;
CREATE INDEX tasks_by_client ON tasks (client_id, updated_at);
