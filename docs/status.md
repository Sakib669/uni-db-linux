# UniDB implementation status

Updated 2026-11-18 after round 22 of the autonomous build.

## Built and committed

- Workspace, CI, pinned toolchain, container GUI gates, Docker verification.
- Nine engines behind one session contract, live container tests for SQL family, Redis, MongoDB, Cassandra, MSSQL.
- App store with migrations, keyring master key, AES-256-GCM secrets, audit, backups, unclean shutdown marker, Security page.
- Libadwaita shell with sidebar sections, toasts, status pages, light and dark color schemes, persisted settings.
- Connection manager with URL parsing, encrypted passwords, test, connect, disconnect, edit, delete, audit.
- Navigator with database and table lists and lazy loading.
- SQL editor with GtkSourceView, result grid with null styling and row cap, cancel, history and audit per run.
- Explain plan pane, bar charts, data compare with keyed row diff, and the ERD schema page with foreign key arrows.
- Table designer with dialect DDL, validation, preview, transactional apply, column introspection.
- Import and export: CSV and JSON import with coercion and reports, CSV and JSON export and masked export, audit.
- History with FTS5 search, saved queries, settings.
- Packaging: verified `.deb`, desktop entry, icon, AppStream, SBOM, release workflow, AppImage and Flatpak scripts, update notice with tests.
- Visual query builder with dialect SQL generation, tests, and a verified Builder page.
- Workflow: deterministic fake data, masking, migration runner with ledger, splitter, git status surface, tests, and full Migrations page UI with container screenshots.
- Time travel and archive: point in time table snapshots in store, keyed diff comparison, transactional restore, audit logging, and History page UI with container screenshots.
- Multi database query and team bundles: concurrent cross connection query runner with merged `_connection` result grid, signed JSON bundles with detached `.sig` verification dialog, audit logging, and container screenshots.

## Current gates

- `cargo test --workspace --no-default-features`: 126 tests passing.
- `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`: clean.
- `cargo fmt --all`: clean.
- GUI build and container screenshots current for all workflow surfaces (including Team bundles, Multi DB query, Snapshots, and Migrations git status).

## Remaining, in roadmap order

- Platform: AI schema doctor, scheduled jobs, WASM plugin runtime, offline first sync.
- Verification: `/check verify`, `/check review`, `/test` per feature.
- Hardening: arm64 packages, accessibility pass, performance on large results, i18n, final security review, dashboard with saved charts.

## Next steps

1. Platform feature decisions and slices (AI schema doctor, scheduled jobs, WASM plugin runtime, offline first sync).
2. Verification and hardening pass across the built areas.
