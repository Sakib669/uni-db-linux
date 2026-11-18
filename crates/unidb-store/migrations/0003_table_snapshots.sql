-- Point in time table snapshots for time travel and archiving.
CREATE TABLE IF NOT EXISTS table_snapshots (
    id TEXT PRIMARY KEY,
    connection_id TEXT NOT NULL,
    database_name TEXT NOT NULL,
    table_name TEXT NOT NULL,
    row_count INTEGER NOT NULL,
    note TEXT,
    data_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_snapshots_connection ON table_snapshots (connection_id, created_at DESC);
