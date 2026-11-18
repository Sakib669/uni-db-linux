# UniDB scope

UniDB is a native Linux desktop database studio. This file tracks what is being built and in what order. Features are coarse. The build detail lives in the specs under `docs/specs/`.

## At a glance

| # | Feature | Status | Spec |
|---|---|---|---|
| 1 | Stack and architecture | in-progress | [0001](../specs/0001-native-gtk4-desktop-architecture/index.md) |
| 2 | Engine layer | in-progress | [0002](../specs/0002-engine-layer.md) |
| 3 | App store and security screens | in-progress | [0003](../specs/0003-app-store-security.md) |
| 4 | Design system and app shell | in-progress | [0004](../specs/0004-design-system-shell.md) |
| 5 | Connection manager and navigator | in-progress | [0005](../specs/0005-connection-manager.md) |
| 6 | SQL editor and result grid | in-progress | [0006](../specs/0006-sql-editor.md) |
| 7 | Table designer | in-progress | [0007](../specs/0007-table-designer.md) |
| 8 | Import and export | in-progress | [0008](../specs/0008-import-export.md) |
| 9 | History, saved queries, and settings | in-progress | [0009](../specs/0009-history-saved-queries-settings.md) |
| 10 | Packaging, signing, updates, and release automation | in-progress | [0010](../specs/0010-packaging-updates.md) |
| 11 | Visual features | in-progress | [0011](../specs/0011-visual-features.md) |
| 12 | Workflow features | in-progress | [0015](../specs/0015-fake-data.md) |

## Features

### 1. Stack and architecture

**Intent**: Replace the skeleton with the real foundation of the product. One native Rust process, GTK4 and libadwaita interface, tokio workers, one SQLite app store, keyring backed secrets, and nine engines behind one driver contract. This feature exists so every later feature has a fixed place to live.

**Approach**: Tracer Bullet. Stand up a thin end to end thread first (window, store, one engine, tests, packaging), then thicken it. No project default is recorded yet, so this is the default for production work.

**Done when**: the workspace builds and runs a native window on Linux; the app store migrates and survives a restart; one engine connects and runs a query; the pipe from the GTK interface to a worker and back is tested; CI runs the gates from the spec; the fake security audit and the stub drivers are gone.

- [x] Decide the stack (spec): [0001](../specs/0001-native-gtk4-desktop-architecture/index.md)
- [x] Scaffold from the decision: /develop Stack and architecture
  - [x] Workspace, core, store, engine, app service, bridge, and CI (verified: 36 tests, clippy and fmt clean, headless binary runs)
  - [x] Native window rendered and the probe button reaches the worker bridge (verified in the Ubuntu 24.04 container under Xvfb, screenshot at `docs/evidence/scaffold-probe.png`)
  - Code in `crates/` and `src/main.rs`
- [ ] Verify it: /check verify Stack and architecture
- [ ] Test it: /test Stack and architecture

### 2. Engine layer

**Intent**: Nine engines behind one driver contract, with the shared value model and error taxonomy. The SQL family comes first because it proves the contract against two wire protocols.

**Done when**: PostgreSQL, MySQL, MariaDB, CockroachDB, Redis, MongoDB, Cassandra, and MSSQL connect and run real requests, each covered by tests against a real server where practical, and read only and cancel behavior match the engine capabilities.

- [x] Decide the engine layer (spec): [0002](../specs/0002-engine-layer.md)
- [ ] Build it: /develop Engine layer
  - [x] SQL family: PostgreSQL, MySQL, MariaDB, and CockroachDB with live container tests (5 tests plus CockroachDB, all passing)
  - [x] Redis and MongoDB connectors with live container tests (4 tests passing)
  - [x] Cassandra and MSSQL connectors with live container tests (both passing; images are large so the tests are ignored by default)
  - All nine engines registered in the app service bootstrap
- [ ] Verify it: /check verify Engine layer
- [ ] Test it: /test Engine layer

### 3. App store and security screens

**Intent**: Make the app's own data real. Open the SQLite store at startup with the keyring master key, keep rolling backups, detect corruption and unclean shutdowns, persist connections with encrypted passwords, and show a Security page computed from live state instead of constants.

**Done when**: the app survives a restart with the same saved connections and secrets, the Security page shows the real checks and recent audit entries, a corrupt store or a missing key produces a diagnostic page with a retry, and the headless tests cover the store and service paths.

- [x] Decide the app store and security screens (spec): [0003](../specs/0003-app-store-security.md)
- [ ] Build it: /develop App store and security screens
  - [x] Store: integrity check, five rolling backups, shutdown marker, live counts (tests pass)
  - [x] App service: keyring bootstrap with injected provider, connection CRUD, security report, clean shutdown (5 headless tests pass)
  - [x] Interface: Studio, Security, and startup diagnostic pages verified with container screenshots in `docs/evidence/`
- [ ] Verify it: /check verify App store and security screens
- [ ] Test it: /test App store and security screens

### 4. Design system and app shell

**Intent**: Give the app one real shell and one design language. A libadwaita navigation split view with a sidebar, a header bar, grouped preference lists, empty states, toast feedback, and an accent mapped onto libadwaita named colors so every later screen plugs into the same structure.

**Done when**: the app window is a libadwaita shell with working sidebar navigation, the Security page reads as grouped rows, the Studio page shows a proper empty state, the startup failure page uses a status page, actions report through toasts, and the three states are captured as screenshots.

- [x] Decide the design system and app shell (spec): [0004](../specs/0004-design-system-shell.md)
- [ ] Build it: /develop Design system and app shell
  - [x] Embedded stylesheet with teal accent and utility classes (light and dark safe)
  - [x] Libadwaita shell: header bar, menu, refresh and backup actions, sidebar navigation split view
  - [x] Studio empty state, Security preference groups with status rows and audit rows, startup status page
  - [x] Screenshots in `docs/evidence/` for studio, security, and startup failure
- [ ] Verify it: /check verify Design system and app shell
- [ ] Test it: /test Design system and app shell

### 5. Connection manager and navigator

**Intent**: Turn saved profiles into the app's entry point. Add, test, save, edit, delete, connect, and disconnect a database connection from the sidebar, in details or URL mode, with encrypted passwords and audit entries. The database navigator loads databases and tables lazily after connect.

**Done when**: a user can create a connection from the form or a URL, test it, save it, see it in the list, connect and disconnect, edit and delete it, all persisted across restarts and audited. After connect, the navigator lists databases and lazily lists tables.

- [x] Decide the connection manager (spec): [0005](../specs/0005-connection-manager.md)
- [x] Build it: /develop Connection manager and navigator
  - [x] URL parsing in core with per scheme tests (22 core unit tests pass)
  - [x] Connections section with saved list, empty state, live badge, toast feedback
  - [x] New and edit form in details or URL mode with test, save, cancel, validation toasts
  - [x] Connect, disconnect, delete with audit entries; verified through the UI save flow screenshot
  - [x] Navigator: schema contract on all nine engines, live SQL family tests, and the connection, database, and table browser verified with a seeded table screenshot
- [ ] Verify it: /check verify Connection manager and navigator
- [ ] Test it: /test Connection manager and navigator

### 6. SQL editor and result grid

**Intent**: The working surface. A GtkSourceView SQL editor over live connections, Run and Cancel through the connection actor, a result grid over the shared value model with null styling and truncation notes, a status line with rows and timing, error feedback, and history plus audit entries for every run.

**Done when**: a user picks a live connection, writes SQL with highlighting, runs it with the button or Ctrl+Enter, sees typed results in the grid with timings, can clear results, sees errors without losing SQL, and every run lands in history and audit.

- [x] Decide the editor and result grid (spec): [0006](../specs/0006-sql-editor.md)
- [ ] Build it: /develop SQL editor and result grid
  - [x] GtkSourceView SQL editor with syntax highlighting, monospace, line numbers, and Ctrl+Enter
  - [x] Live connection selector, Run, Cancel, and Clear results with responsive busy state
  - [x] Result grid over the shared value model with headers, types, null styling, 500 row cap, truncation notice, and row and column timing status
  - [x] Error state that keeps the statement and shows the safe engine message
  - [x] Query cancel through the actor, unsupported engines reported honestly, headless tests for cancel and audit
  - [x] Query audit entries with outcome, elapsed time, and rows
  - [x] Verified with a seeded SQLite query in the container screenshot
- [ ] Follow up: virtualized grid, multi tab editor, cell copy, EXPLAIN
- [ ] Verify it: /check verify SQL editor and result grid
- [ ] Test it: /test SQL editor and result grid

### 7. Table designer

**Intent**: A guided way to create and change tables. Column editor with type, nullability, default, primary key, uniqueness, and auto increment; exact DDL preview; transactional apply; dialect aware statements for SQLite, PostgreSQL, CockroachDB, MySQL, and MariaDB; edit existing tables with add, rename, and drop columns; honest unsupported state for the other engines.

**Done when**: a user can create a table through the designer and see it appear in the navigator, load an existing table and add or rename or drop a column with previewed DDL, and every apply is all or nothing with clear errors.

- [x] Decide the table designer (spec): [0007](../specs/0007-table-designer.md)
- [x] Build it: /develop Table designer
  - [x] DDL domain module with SQLite, PostgreSQL, and MySQL dialects, validation, and unit tests
  - [x] Column introspection for the SQL engines through one session contract
  - [x] Transactional `ApplyStatements` actor command so partial DDL cannot land
  - [x] Designer page with connection and table selection, new table mode, column editor, SQL preview, apply, statuses, and toasts
  - [x] Edit existing table with add, rename, and drop column generation, verified by the SQLite service test
  - [x] Unsupported engine state and honest errors
  - [x] Verified create and apply through the container screenshots
- [ ] Follow up: type changes, index and foreign key editing, table rebuilds, schema diff
- [ ] Verify it: /check verify Table designer
- [ ] Test it: /test Table designer

### 8. Import and export

**Intent**: Move data in and out. Import CSV and JSON files into a live table with type coercion, header mapping, batched transactional inserts, and an honest report. Export the current query result as CSV or JSON through a save dialog. Domain parsing and formatting are pure and tested.

**Done when**: a user imports a CSV or JSON file into a table and sees rows read, inserted, and rejected with reasons; exports a query result to CSV or JSON; and every transfer is audited without recording row contents.

- [x] Decide the transfer model (spec): [0008](../specs/0008-import-export.md)
- [x] Build it: /develop Import and export
  - [x] Domain module: CSV and JSON parsing, type coercion, insert generation with quoting, CSV and JSON formatting, unit tests
  - [x] Service import with header mapping, missing column rules, rejection warnings, 200 row transactional batches, and audit entries
  - [x] Service export through the editor query path with file writing and audit entries
  - [x] Transfer page with connection and table selection, CSV or JSON format, header switch, path entry, Browse, Import, and report status
  - [x] Editor Export CSV and Export JSON buttons with a save dialog
  - [x] Verified import through the container screenshot with a seeded CSV
- [ ] Follow up: streaming imports with progress and cancel, interactive column mapping, update and upsert, Excel and Parquet
- [ ] Verify it: /check verify Import and export
- [ ] Test it: /test Import and export

### 9. History, saved queries, and settings

**Intent**: Make the records the app already keeps usable. History with FTS5 search, loading a statement back into the editor, and clearing with confirmation. Named saved queries with load and delete. A settings surface with a persisted color scheme applied immediately through the libadwaita style manager.

**Done when**: a user finds a past statement, loads it into the editor, saves a statement under a name and loads it later, clears history after confirmation, and chooses a color scheme that survives a restart.

- [x] Decide history, saved queries, and settings (spec): [0009](../specs/0009-history-saved-queries-settings.md)
- [x] Build it: /develop History, saved queries, and settings
  - [x] Saved query store model, save, list, delete, and history clearing
  - [x] Service wrappers for history search, saved queries, clearing, and settings
  - [x] History page with search, newest first rows, load into editor, and confirmed clear
  - [x] Saved queries group with name, save current SQL, activation to load, and delete
  - [x] Settings page with System, Light, and Dark color schemes, startup restore, and immediate apply
  - [x] Studio placeholder and probe surface removed from the shell
  - [x] Verified history, saved query, and dark scheme through container screenshots
- [ ] Follow up: history retention setting, folders and tags, snippets, editor preferences
- [ ] Verify it: /check verify History, saved queries, and settings
- [ ] Test it: /test History, saved queries, and settings

### 10. Packaging, signing, updates, and release automation

**Intent**: Make UniDB installable and releasable. A verified `.deb` built from the pinned container with a desktop entry, icon, AppStream metadata, and explicit dependencies. An AppImage script and Flatpak skeleton for other distributions. A release workflow on tags with gates, SBOM, checksums, optional signing, and a GitHub release. An update notice in Settings fed by a pinned release feed.

**Done when**: one command produces a verified `.deb`, a `v*` tag produces a signed-when-configured GitHub release with SBOM and checksums, and Settings reports whether a newer version exists without downloading anything.

- [x] Decide packaging, signing, updates, and release automation (spec): [0010](../specs/0010-packaging-updates.md)
- [x] Build it: /develop Packaging, signing, updates, and release automation
  - [x] Icon, desktop entry, AppStream metadata, and staged `.deb` build script
  - [x] Package verification: metadata, contents, extraction, and `unidb --version` inside the container
  - [x] CycloneDX SBOM generator, 502 components from cargo metadata
  - [x] AppImage build script and Flatpak manifest skeleton
  - [x] Release GitHub Actions workflow with gates, SBOM, checksums, optional GPG signing, and release publishing
  - [x] Update feed parsing, version comparison, HTTP fetch with `ureq`, service check, and headless tests
  - [x] Settings updates group with the check action, verified against a local feed
  - [x] Packaging, signing, update, and rollback documentation
- [ ] Follow up: arm64 matrix, signed apt repository, in app download and verification, Flathub submission
- [ ] Verify it: /check verify Packaging, signing, updates, and release automation
- [ ] Test it: /test Packaging, signing, updates, and release automation

### 11. Visual features

**Intent**: The first visual slice: an explain plan pane that runs the engine's explain statement and renders the plan beside the result, and a bar chart view of the current result with table and chart switching. Both use pure domain helpers over the shared value model. ERD, dashboard, visual query builder, and data compare follow with their own decisions.

**Done when**: a user explains a statement and reads the plan without losing the editor, charts a result and switches back to the table, and both work for every SQL engine supported by the explain contract.

- [x] Decide the visual slice (spec): [0011](../specs/0011-visual-features.md)
- [x] Build it: /develop Visual features
  - [x] Core visual module: explain statement selection per engine and chart series extraction, with unit tests
  - [x] Explain action through the normal execution path, JSON pretty printing, and the collapsible plan pane
  - [x] Results stack with the table, a cairo bar chart, labels and values, null skipping, and a 50 bar cap
  - [x] Table and chart switching plus a clear no numeric column message
  - [x] ERD: foreign key introspection for the SQL engines, pure layout function, and a Schema page with cards and relationship arrows
  - [x] Data compare: baseline capture, keyed row diff with counts and details in the plan pane, and an Export menu to free toolbar space
  - [x] Visual query builder: connection, table, column checkboxes, filters, order, limit, SQL preview, and open in editor, verified with a container screenshot
  - [x] Verified chart, explain plan, ERD, and compare through container screenshots
- [ ] Follow up: interactive ERD (drag, zoom, click to open), dashboard with saved charts, builder joins and aggregates, cross connection data compare, chart export
- [ ] Verify it: /check verify Visual features
- [ ] Test it: /test Visual features

### 12. Workflow features

**Intent**: Work in progress. The first slice is deterministic fake data generation from the Transfer page, inserted through the transactional path with audit. Migrations and git, time travel and archive, masking, multi database query, and team bundles remain with their decisions recorded as follow ups.

**Done when**: the workflow set is complete. For the current slice, a user can fill a table with reproducible generated rows and see the report.

- [ ] Decide all workflow features. First slice: [0015](../specs/0015-fake-data.md)
- [ ] Build it: /develop Workflow features
  - [x] Fake data: type aware deterministic generator, skipped integer keys, transactional insert, audit, Transfer controls, and container screenshot
  - [x] Migrations and git: numbered SQL files, safe statement splitter, per connection ledger with checksums, transactional runner with audit and SQLite tests; Migrations page and git status are follow ups
  - [ ] Time travel and archive
  - [x] Masking of exports: text and email masking, masked CSV and JSON entries in the editor Export menu, service path with audit, and headless tests
  - [ ] Multi database query and team bundles
- [ ] Verify it: /check verify Workflow features
- [ ] Test it: /test Workflow features

## Deferred

Features from the spec roadmap are enrolled here as their own scope features when their spec is written, in roadmap order: engine layer, app store and credential security, design system and shell, connection manager, SQL editor and grid, table designer, import and export, history and saved queries, packaging and updates, visual features, workflow features, platform features.
