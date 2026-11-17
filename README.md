# uni-db-linux

UniDB is a native Linux desktop database studio: one fast, free app for SQL, NoSQL, and Redis.

Engines: PostgreSQL, MySQL, MariaDB, CockroachDB, Cassandra, Redis, MongoDB, MSSQL, SQLite.

> Status: foundation scaffold. The workspace, app store, secret handling, connection service, bridge, and SQLite engine are built and tested. The feature list below is the product roadmap, not shipped code. The architecture decision is in `docs/specs/0001-native-gtk4-desktop-architecture/`.

## Why UniDB

- **All 9 engines free** with no paywall.
- **Fast by design** with Rust and native GTK4; startup and memory budgets are enforced in CI as the features land.
- **Secure by design**: a keyring master key with AES-256-GCM for saved secrets, TLS for cloud connections, SSH tunnels, read only mode, an audit log, schema only AI, and no telemetry.
- **Native Linux**: GTK4 and libadwaita on Wayland and X11, with no browser engine.

## What works today

- Cargo workspace with separate crates for core, store, engine, app services, and the interface.
- SQLite app store with migrations, keyring master key, AES-256-GCM envelopes, audit log, and FTS5 query history search.
- Driver traits plus a SQLite connector, tested against real SQLite.
- tokio to interface bridge, supervised connection actors, and app services, tested headless.
- CI gates: formatting, clippy, tests, cargo deny, cargo audit, plus a separate job that builds the GTK app.

## Build and run

Prerequisites: a Linux desktop distribution with GTK4 4.14 or newer (Ubuntu 24.04, Linux Mint 22, Fedora 40, Debian 13, or newer), rustup, and the GTK development packages:

```bash
sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev libgtksourceview-5-dev
```

Then:

```bash
# native app
cargo run --features gui

# headless check on a machine without GTK headers
cargo run --no-default-features

# tests and gates
cargo test --workspace --no-default-features
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo fmt --all -- --check
```

The `gui` feature is off by default so the workspace builds on machines that only need the core. The app needs a running Secret Service (GNOME Keyring or KWallet) to open saved passwords.

## Project structure

```
crates/unidb-core    domain types, value model, error taxonomy, connection profiles
crates/unidb-store   SQLite app store, migrations, keyring, AES-256-GCM envelopes, audit log
crates/unidb-engine  driver traits and the SQLite connector
crates/unidb-app     bridge adapter, connection actors, app service
crates/unidb-ui      GTK4 and libadwaita shell (gui feature)
src/main.rs          thin binary
docs/specs/          architecture decisions
docs/scope/          project scope
```

## Roadmap

Foundation first, then the engine layer, the app store and security screens, the design system and app shell, the connection manager, the SQL editor and result grid, the table designer, import and export, history and saved queries, packaging and updates, then the visual, workflow, and platform features. The full order is in spec 0001.

## AI

Local first with Ollama, plus remote providers through BYOK in a later phase. Schema only, never rows.

## License

MIT.
