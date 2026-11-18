# UniDB implementation status

Updated 2026-11-18 after completing the Platform features and verification pass.

## Built and committed

- Workspace, CI, pinned toolchain, container GUI gates, Docker verification (`unsafe_code = "forbid"`).
- Nine engines behind one session contract, live container tests for SQL family, Redis, MongoDB, Cassandra, MSSQL.
- App store with migrations, keyring master key, AES-256-GCM secrets, audit, backups, unclean shutdown marker, Security page.
- Libadwaita shell with sidebar sections, toasts, status pages, light and dark color schemes, persisted settings.
- Connection manager with URL parsing, encrypted passwords, test, connect, disconnect, edit, delete, audit.
- Navigator with database and table lists and lazy loading.
- SQL editor with GtkSourceView, result grid with null styling and 500 row cap, cancel, history and audit per run.
- Explain plan pane, bar charts, data compare with keyed row diff, and the ERD schema page with foreign key arrows.
- Table designer with dialect DDL, validation, preview, transactional apply, column introspection.
- Import and export: CSV and JSON import with coercion and reports, CSV and JSON export and masked export, audit.
- History with FTS5 search, saved queries, settings.
- Packaging: verified `.deb`, desktop entry, icon, AppStream, CycloneDX SBOM (502 components), release workflow, AppImage and Flatpak scripts, update notice with tests.
- Visual query builder with dialect SQL generation, tests, and a verified Builder page.
- Workflow: deterministic fake data, masking, migration runner with ledger, splitter, git status surface, tests, and full Migrations page UI with container screenshots.
- Time travel and archive: point in time table snapshots in store, keyed diff comparison, transactional restore, audit logging, and History page UI with container screenshots.
- Multi database query and team bundles: concurrent cross connection query runner with merged `_connection` result grid, signed JSON bundles with detached `.sig` verification dialog, audit logging, and container screenshots.
- Platform features: AI Schema Doctor (`unidb-core::doctor`), Scheduled Jobs (`0004_scheduled_jobs.sql`), Pure Rust Sandboxed WASM Plugin Runtime (`unidb-core::plugin`, `0006_wasm_plugins.sql`), Offline First Sync (`0005_sync_queue.sql`), Platform UI tabs, and verified container screenshots in `docs/evidence/`.

## Current gates

- `cargo test --workspace --no-default-features`: 136 tests passing (0 failed; 3 large image tests ignored by default).
- `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.
- GUI build and container screenshots current across all surfaces including Platform (`platform-doctor.png`, `platform-jobs.png`, `platform-plugins.png`, `platform-sync.png`).
- CycloneDX SBOM verified (`target/sbom.cdx.json`, 502 components).

## Deferred post v0.1.0

- arm64 package matrix, Flathub submission, full i18n translation catalogs, interactive drag/zoom ERD canvas, saved chart dashboard widgets.

