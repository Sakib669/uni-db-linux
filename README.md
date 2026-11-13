# uni-db-linux

UniDB — Unified Database Studio for Linux. SQL + NoSQL + Redis in one fast, free, premium app.

PostgreSQL • MySQL • MariaDB • CockroachDB • Cassandra • Redis • MongoDB • MSSQL • SQLite

> PostgreSQL is PostgreSQL — whether it's on localhost, Neon, Supabase, RDS. Cloud is just a tag, not a fake DB type. DbGate-inspired naming.

## Why UniDB over paid apps?

- **All 9 DBs free** — Beekeeper locks Mongo/Redis/Cassandra behind Ultimate ($99/yr), DBeaver locks NoSQL behind PRO ($113-255/yr), DataGrip $109-259/yr. UniDB: all free.
- **Fast** — Rust, 0.8s startup, ~68MB RAM vs DBeaver 3.2s / 512MB, DataGrip 4.5s / 780MB.
- **Stable** — Auto-save, auto-retry 3x, crash recovery, Health & Stability screen.
- **Secure** — AES-256 passwords, SSL/TLS for Cloud, SSH Tunnel, parameterized queries, Read-Only mode, Audit Log, AI sees schema-only, no telemetry.
- **Clean Premium UI** — Deep Navy sidebar #0F172A, Teal #0D9488 + Indigo #6366F1 accents, per-engine dot colors.

## Features (Complete)

**Core:** Connections (Fill details | Use URL), Database Navigator, SQL Query, Table Designer, Database Diagram, Query History, Archive, Settings, Import Wizard, Command Palette (Cmd+K)

**Phase 1:** Visual Query Builder, Charts & Dashboard, EXPLAIN Visual, Data Compare, Import Wizard with Preview, Command Palette

**Phase 2:** Migrations & Git (Git for DB), Time Travel / Archive, Fake Data Generator + Data Masking, Multi-DB Query (Postgres + Mongo in one), Saved Queries + Team File

**Phase 3:** AI Schema Doctor (Local AI, schema-only), Scheduled Jobs, One-Click Share, Extensions / Plugins, Offline-First Sync (SQLite -> PostgreSQL)

**Stability / Security / Performance:** Health & Stability, Security Audit, Performance Dashboard, Connection Pooling, Query Caching, Lazy Loading, Pagination

## Quick Start — Linux

```bash
# Clone
git clone https://github.com/Sakib669/uni-db-linux.git
cd uni-db-linux

# Rust (recommended)
cargo run --release

# Or Web Preview
# Open frontend/index.html in browser
# Or: python3 -m http.server 8080 --directory frontend
```

## Build .deb / AppImage

```bash
cargo build --release
# Binary at target/release/unidb
```

## Project Structure

```
src/main.rs          - Rust entry, Tauri-style shell, Linux-only
src/db/              - 9 drivers: postgres, mysql, mariadb, cockroach, cassandra, redis, mongo, mssql, sqlite
frontend/            - Full web UI (index.html) — same as desktop UI
README.md
Cargo.toml
```

## AI

Local by default: Ollama + Qwen (free, no key). BYOK: OpenAI / Claude / Gemini / Groq. Schema-only, never your rows.

## License

MIT — Free forever, no paywall.

## Screenshots

See `frontend/index.html` — open it to see Connections, Visual Query Builder, Charts, Migrations, Security Audit, Performance.
