-- Installed sandboxed WebAssembly plugins.
CREATE TABLE IF NOT EXISTS wasm_plugins (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    exports_json TEXT NOT NULL,
    wasm_base64 TEXT NOT NULL,
    installed_at TEXT NOT NULL
);
