# UniDB scope

UniDB is a native Linux desktop database studio. This file tracks what is being built and in what order. Features are coarse. The build detail lives in the specs under `docs/specs/`.

## At a glance

| # | Feature | Status | Spec |
|---|---|---|---|
| 1 | Stack and architecture | in-progress | [0001](../specs/0001-native-gtk4-desktop-architecture/index.md) |
| 2 | Engine layer | in-progress | [0002](../specs/0002-engine-layer.md) |

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

## Deferred

Features from the spec roadmap are enrolled here as their own scope features when their spec is written, in roadmap order: engine layer, app store and credential security, design system and shell, connection manager, SQL editor and grid, table designer, import and export, history and saved queries, packaging and updates, visual features, workflow features, platform features.
