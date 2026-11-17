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

## Deferred

Features from the spec roadmap are enrolled here as their own scope features when their spec is written, in roadmap order: engine layer, app store and credential security, design system and shell, connection manager, SQL editor and grid, table designer, import and export, history and saved queries, packaging and updates, visual features, workflow features, platform features.
