-- UniDB app store schema, version 1. Spec 0001, app data model.
-- Secrets are ciphertext plus nonce. There is no plaintext secret column.

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS connections (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    engine TEXT NOT NULL,
    host TEXT NOT NULL DEFAULT '',
    port INTEGER NOT NULL DEFAULT 0,
    database TEXT NOT NULL DEFAULT '',
    username TEXT NOT NULL DEFAULT '',
    password_ciphertext BLOB,
    password_nonce BLOB,
    ssl_mode TEXT NOT NULL DEFAULT 'verify_full',
    ssh_profile_id TEXT,
    read_only INTEGER NOT NULL DEFAULT 0,
    color TEXT NOT NULL DEFAULT '',
    group_name TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_used_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_connections_engine ON connections (engine);

CREATE TABLE IF NOT EXISTS ssh_profiles (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    host TEXT NOT NULL,
    port INTEGER NOT NULL DEFAULT 22,
    username TEXT NOT NULL,
    auth_method TEXT NOT NULL,
    key_path TEXT,
    passphrase_ciphertext BLOB,
    passphrase_nonce BLOB,
    host_key_policy TEXT NOT NULL DEFAULT 'strict',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS query_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    connection_id TEXT,
    sql TEXT NOT NULL,
    started_at TEXT NOT NULL,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    row_count INTEGER,
    status TEXT NOT NULL,
    error_code TEXT,
    error_message TEXT,
    cancelled INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_history_connection_time
    ON query_history (connection_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_history_time ON query_history (started_at DESC);

CREATE VIRTUAL TABLE IF NOT EXISTS query_history_fts USING fts5(
    sql,
    content='query_history',
    content_rowid='id'
);

CREATE TRIGGER IF NOT EXISTS query_history_ai AFTER INSERT ON query_history BEGIN
    INSERT INTO query_history_fts (rowid, sql) VALUES (new.id, new.sql);
END;

CREATE TRIGGER IF NOT EXISTS query_history_ad AFTER DELETE ON query_history BEGIN
    INSERT INTO query_history_fts (query_history_fts, rowid, sql)
    VALUES ('delete', old.id, old.sql);
END;

CREATE TRIGGER IF NOT EXISTS query_history_au AFTER UPDATE ON query_history BEGIN
    INSERT INTO query_history_fts (query_history_fts, rowid, sql)
    VALUES ('delete', old.id, old.sql);
    INSERT INTO query_history_fts (rowid, sql) VALUES (new.id, new.sql);
END;

CREATE TABLE IF NOT EXISTS saved_queries (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    sql TEXT NOT NULL,
    connection_id TEXT,
    folder TEXT NOT NULL DEFAULT '',
    tags TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ts TEXT NOT NULL,
    action TEXT NOT NULL,
    connection_id TEXT,
    outcome TEXT NOT NULL,
    detail_json TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_audit_time ON audit_log (ts DESC);

CREATE TABLE IF NOT EXISTS app_meta (
    key TEXT PRIMARY KEY NOT NULL,
    value BLOB NOT NULL
);
