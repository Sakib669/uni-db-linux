-- Offline first sync queue for drafting operations while disconnected.
CREATE TABLE IF NOT EXISTS sync_queue (
    id TEXT PRIMARY KEY,
    connection_id TEXT NOT NULL,
    label TEXT NOT NULL,
    sql_text TEXT NOT NULL,
    guard_query TEXT,
    expected_guard_value TEXT,
    status TEXT NOT NULL,
    error_message TEXT,
    created_at TEXT NOT NULL,
    synced_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_sync_queue_conn ON sync_queue (connection_id, created_at ASC);
