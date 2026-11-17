# 0001. Adopt a native GTK4 desktop architecture for UniDB

**Date**: 2026-11-17
**Status**: Accepted

## Summary

UniDB becomes one native Linux desktop program written in Rust. GTK4 and libadwaita, the GNOME toolkit, draw every window and widget with no browser engine inside. Database work runs on tokio worker threads, the app's own records live in one SQLite file, the operating system keyring holds one master key that encrypts every saved secret, and nine database engines sit behind one driver interface. This gives the project the speed and low memory it advertises, and it gives every later feature a fixed place to live.

## Decision

**Chosen option**: Option 2, native GTK4 with libadwaita and relm4.

UniDB is a single process Rust application. GTK4 and libadwaita own the interface on the main thread, a tokio runtime performs all database and network work on worker threads, and the two sides exchange typed messages over channels. The code lives in a Cargo workspace with separate crates for domain types, the app store, the engine layer, export, AI, sharing, services, and the GTK interface, and one thin binary ties them together. There is no HTTP surface, no local port, and no embedded browser.

**Implementation skills**: `z-rust-gtk4` (`victorzhuk/skills`, `.agents/skills/z-rust-gtk4/`) · `relm4-app` (`anson2251/relm4-app-skill`, `.agents/skills/relm4-app/`) · `gtk4-libadwaita-internals` (`cominotti/lushtext`, `.agents/skills/gtk4-libadwaita-internals/`) · `developing-gtk-apps` (`paresh-maheshwari/gnome-skills`, `.agents/skills/developing-gtk-apps/`) · `designing-gnome-ui` (`paresh-maheshwari/gnome-skills`, `.agents/skills/designing-gnome-ui/`)

## Proposed stack

| Layer | Choice | Reason |
|---|---|---|
| Language | Rust, edition 2024, MSRV 1.94 pinned in `rust-toolchain.toml` | Memory safety without a garbage collector, matches the speed and memory claims, and the repository is already Rust. The minimum version comes from sqlx 0.9. |
| Interface toolkit | GTK4 4.14 or newer with libadwaita 1.5 or newer, component model relm4 | Native widgets, native Wayland and X11, native accessibility, `GtkSourceView` for SQL editing, `GtkColumnView` for large tables, no web engine. The floor is the newest line shipped by Ubuntu 24.04 and Fedora 40. |
| TLS | rustls with `rustls-native-certs` for system trust and `rustls-pemfile` for custom CAs | Pure Rust TLS, system certificate store by default, and an explicit custom CA path for enterprise databases. |
| Application pattern | One process, layered modules, Cargo workspace of crates | One binary to ship and debug, with clear layer boundaries and no network boundaries between them. |
| Async runtime | tokio multi thread runtime, one bridge adapter to the glib main loop | Every chosen driver crate is tokio native, and the bridge keeps the interface thread free. |
| App records store | SQLite through sqlx with the FTS5 feature | One file for connections, profiles, history, saved queries, settings, and the audit log. Same stack already needed for the SQLite engine, and one file makes backup easy. |
| SQL engines | sqlx 0.9 for PostgreSQL, MySQL, MariaDB, CockroachDB, and SQLite | One query API and one pool model across the SQL engines. CockroachDB speaks the PostgreSQL wire protocol. |
| Key value engine | redis crate, current stable release | De facto Rust client; the scout saw a 2.0 release candidate, so the stable line is settled before the engine spec. |
| Document engine | mongodb 3.x, the official driver | Official support, BSON types, aggregation pipeline support. |
| Wide column engine | scylla 1.x | Pure Rust Cassandra CQL driver with a connection pool; verified current at 1.10.0 in the landscape scan. |
| Microsoft engine | tiberius 0.13 | The only pure Rust TDS client, so no FreeTDS C library is needed. |
| SSH tunnels | russh 0.62.x in process | Pure Rust tunnel with key and agent support, no dependency on a system ssh install, and testable. |
| Secrets | Linux Secret Service through keyring 3.x for one master key, AES-256-GCM for values in SQLite | Saved passwords never sit in plaintext, and the user never types a master passphrase on launch. |
| SQL editor | GtkSourceView 5 | Native syntax highlighting, undo, and search, with no embedded browser. |
| Tables | GtkColumnView over a custom GListModel | Widget level virtualization built into GTK, needed for large result sets. |
| Charts and diagrams | plotters rendering to Cairo for charts, custom Cairo drawing for the ERD | Native rendering inside GTK drawing areas, no JavaScript charting. |
| AI providers | Provider trait with adapters for Ollama, OpenAI, Anthropic, Gemini, Groq, and any OpenAI compatible base URL | Local default that works offline, plus every major remote provider, keys in the keyring, schema only payloads. |
| Search | SQLite FTS5 | Search over history and saved queries with no extra service. |
| Logging and health | tracing with tracing-appender local rotating logs, in app Health screen, opt in crash export | No telemetry. Support data stays on the machine unless the user exports it. |
| Packaging | `.deb` through cargo-deb and AppImage through appimagetool (glibc 2.39 baseline), Flatpak later | Matches the chosen release channels and the GTK floor. |
| Updates | Signed GitHub releases plus an in app notice | Nothing replaces the binary silently. |
| Tests | Unit tests, testcontainers integration against real engines, GTK component tests | Proves the engines actually work instead of only proving the mocks work. |
| Supply chain | GitHub Actions, cargo deny, cargo audit, committed `Cargo.lock`, GPG signed artifacts | Standard enterprise hygiene at low ongoing cost. |

### Pinned versions

Exact versions are fixed in the scaffold spec and committed in `Cargo.lock`. The landscape scan on 2026-11-17 verified these lines, and any patch version is re checked at scaffold time:

- gtk4, gdk4, libadwaita, gtk4-sourceview, and relm4: the newest release line that compiles against GTK 4.14 and libadwaita 1.5.
- sqlx 0.9.0 with the `sqlite` feature and FTS5 enabled.
- mongodb 3.10, scylla 1.10, tiberius 0.13.
- redis: settle the stable 1.x line versus the 2.0 release candidate before the engine spec; the API differs.
- russh 0.62, keyring 3.6, rustls with native certificates, plotters, tokio 1.x.
- `rust_decimal`, `chrono`, `uuid`, and `serde_json` for the value model.

## Architecture

### Process and threading model

There is one process and one binary. The GTK main thread owns every widget and runs the glib main loop. A tokio multi thread runtime, sized at the smaller of four workers and the CPU count, is created at startup and its handle is held by the application service layer. All crossing goes through one bridge adapter with three rules: one shot results travel on a relm4 `Sender`, which is safe to call from any thread; streams travel on a bounded `async_channel` consumed by `glib::spawn_future_local` on the main thread; and no GTK callback ever calls `block_on`. The adapter has its own tests because every later feature depends on it.

Each open connection is supervised by a connection actor task. The app service holds the actors in a `JoinSet`, restarts an actor once on panic with backoff, and owns a cancellation token tree (app, connection, query). On shutdown it cancels the tree, waits for the actors, and closes the pools before the runtime drops. A panicking GTK callback is caught by a panic hook that logs the crash, shows an error dialog, and preserves open editor buffers through the session restore file.

The actor owns the pool, serializes commands for that connection, and holds a `CancellationToken` per query so the Cancel button can stop either the wait or, where the engine allows, the server side work. Query results arrive in batches sized around 500 rows. The table view keeps at most 10,000 rows in memory; beyond that, rows spill to a temporary SQLite file and the grid shows an asynchronously prefetched windowed view, so the 60 frames per second budget holds. Cancelling a query drops the batch channel and signals the engine specific cancel path (PostgreSQL cancel token, MySQL needs a second connection and `KILL QUERY`, best effort elsewhere); the interface says which engines support a true server side cancel.

### Crate layout

```
crates/
  unidb-core      domain types, errors, config, value model, capability flags
  unidb-store     SQLite app store, migrations, keyring and AES-256-GCM secret service, audit log
  unidb-engine    driver traits plus one module per engine
  unidb-export    CSV, JSON, SQL, and clipboard serializers, import parsers
  unidb-ai        provider trait, adapters, prompt building, schema only redaction
  unidb-share     signed team bundle format, import and export
  unidb-app       services and orchestration: connection manager, query service, history, settings,
                  session restore, actor supervision, runtime handle, bridge adapter
  unidb-ui        relm4 components, windows, models, styling
src/main.rs       thin binary: parse the two flags, start logging, build the runtime, run the app
```

Dependencies point one way: `unidb-ui` depends on `unidb-app`, which depends on `unidb-core`, `unidb-engine`, `unidb-store`, `unidb-ai`, and `unidb-share`; nothing depends on `unidb-ui`. This keeps engine and store code testable without a display and keeps orchestration out of `main.rs`.

### Driver contract

One small set of traits, with engine modules implementing only what they support and declaring capabilities:

- `Engine`: static metadata (display name, default port, URL scheme, color, capability flags).
- `Connector`: build a pool from a connection profile, test a profile without saving it, and report a typed failure.
- `Session`: execute one statement with optional parameters, stream rows, cancel, and begin, commit, or roll back a transaction.
- `SchemaSource`: list databases, schemas, tables, columns, and indexes.
- `AdminOps`: engine specific extras such as explain plans, table DDL preview, or Redis command metadata.

The traits are object safe by using `#[async_trait]`, and the engine registry holds `Arc<dyn Connector>` and `Arc<dyn Session>`. Enum dispatch is rejected because it would force every engine type into one module and defeat separate compilation.

Every value crosses the boundary as one `Value` enum (null, bool, int, float, decimal, text, bytes, json, timestamp, uuid, array) so the grid and exporters never deal with driver types. Failures use one error taxonomy: auth, network, TLS, timeout, syntax, permission, unsupported, connection lost, cancelled, and internal. Read only mode is enforced in the session layer per engine (PostgreSQL read only transaction, MySQL `START TRANSACTION READ ONLY`, SQLite `PRAGMA query_only`, best effort elsewhere) and the interface blocks known write statements, while the database user's grants remain the real boundary.

All SQL is sent with parameters through the driver's prepared statement API. The current identity function in `main.rs` is deleted; there is no string interpolation path in the engine layer.

### Value and error model

`Value` maps to concrete Rust types: `rust_decimal` for decimal, `chrono` for timestamps (always UTC in the domain, with the display offset carried separately and never guessed), `uuid` for UUID, `serde_json` for JSON, and byte arrays for binary. Each engine module documents its conversion rules, including the lossy cases (for example MSSQL `datetimeoffset` and Cassandra collection types), and those rules are covered by engine tests.

One `UniDbError` type is shared by the engine, store, and AI layers: a stable code, a category from the taxonomy above, a retryable flag, a user facing message, a redacted detail string, and a source chain. SQL text and secrets are banned from `Display` and from every log field. The tracing layer enforces redaction before writing: SQL text is dropped by default (an explicit debug setting can include it in local logs only), secrets never pass through, and connection ids are hashed. Crash export uses an allowlist and never includes the database file or the app store.

### App data model

SQLite, migrated by `sqlx::migrate!` at startup. Secrets are stored as ciphertext plus nonce and nothing else. The bundled SQLite build enables FTS5, and a store test asserts that `query_history` search works so a build without the feature fails early.

| Table | Purpose and key columns |
|---|---|
| `connections` | id, name (unique), engine, host, port, database name, username, password ciphertext, password nonce, TLS mode, ssh profile id, read only flag, color, group, created and updated and last used timestamps |
| `ssh_profiles` | id, name, host, port, username, auth method (agent, key, password), key path, passphrase ciphertext and nonce, host key policy |
| `query_history` | id, connection id, sql text, started at, duration, row count, status, error code and message, cancelled flag; indexed on connection and time; mirrored into an FTS5 table |
| `saved_queries` | id, name, sql, optional connection id, folder, tags, timestamps |
| `settings` | key and JSON value (window state, retention, update check, AI defaults, timeouts) |
| `audit_log` | id, timestamp, action, connection id, outcome, detail JSON (connection opened, query run, secret decrypted, share imported, settings changed) |
| `app_meta` | key and value, including the master key version, the key check, and the unclean shutdown marker |

History is pruned at startup to 90 days by default and the audit log to 365 days; both are configurable in settings. Queries run in WAL mode with a `busy_timeout`, and the app is a single instance (`GtkApplication` by app id), so a second launch focuses the existing window instead of opening a second database handle.

### Security architecture

On first run the app generates a 32 byte random master key and stores it in the Secret Service under the fixed attributes `application=io.github.sakib669.UniDB` and `purpose=master-key`, then stores the key version and an HMAC of the key in `app_meta`. Every saved secret is stored in a versioned envelope: one version byte, a random 96 bit nonce, and the AES-256-GCM ciphertext, with associated data of `record type | record id | field` (for example `connection | <uuid> | password`). Decryption fails closed on an unknown version or a mismatched associated data.

Key rotation writes a new version and re encrypts every secret in one transaction, keeping the old version until the rewrite commits. If the keyring is locked, reports as missing, or holds a different key than the HMAC check, the app distinguishes all three cases, never falls back to plaintext, and marks the affected secrets as unusable so the user can re enter them. Plaintext lives in zeroizing buffers and is dropped as soon as the driver has it; this reduces exposure but cannot erase copies made inside driver, TLS, and allocator buffers, and the spec does not claim otherwise.

Every outbound path is explicit: the user's own database connections, SSH tunnel hosts, AI provider endpoints when the user invokes AI, and one GitHub update check per day that can be switched off. Nothing else leaves the machine, and there is no telemetry. The update check verifies a detached signature from a dedicated ed25519 key, whose public key ships in the binary, before it shows any notice; the release manifest URL is pinned.

TLS defaults to `verify-full` for any host that is not localhost. System trust comes from rustls-native-certs, custom CA files are loaded with rustls-pemfile, and each connection's TLS mode maps to one explicit verifier configuration. SSH host keys are checked against `known_hosts` with strict mode by default; an unknown host prompts once and the decision is remembered. Team bundles are plain JSON that never contains passwords, SSH paths, or AI keys, and carry a detached signature that the import screen shows before anything is accepted.

### Failure and recovery

Each connection has a state machine of disconnected, connecting, ready, degraded, lost, and reconnecting. A dropped connection shows a banner and reconnects with exponential backoff from one to thirty seconds with jitter. In flight user statements fail with a typed error and are never retried automatically, because retrying a write duplicates it; metadata reads may retry once.

Timeouts live in one config module and are exposed in settings: connect 10 seconds; statement execution has no silent timeout and is stopped only by the user or a lost connection; AI provider calls 60 seconds; metadata reads retry once after a network error.

If the keyring is locked or unavailable, the app starts in a restricted mode where saved secrets cannot be decrypted, explains why, and offers to unlock or re enter secrets for the session. If the app database fails an integrity check at startup, the newest good file is restored from five rolling backups and the user is told what happened. An unclean shutdown marker in `settings` lets the next launch offer recovery, including the editor buffers saved by the session restore file. Disk full during spill, backup, or export is detected, the operation stops with a clear error, temporary files are removed, and the app keeps running with existing tabs. AI provider calls have a timeout, a cancel control, and a clear retry, with Ollama failures suggesting that the local service is not running. Import conflicts are previewed and resolved per item.

### Configuration and file locations

| What | Where |
|---|---|
| App database and backups | `$XDG_DATA_HOME/unidb/` (`~/.local/share/unidb/`) |
| Logs | `$XDG_STATE_HOME/unidb/logs/`, daily rotation, seven day retention |
| Cache and temporary result spill | `$XDG_CACHE_HOME/unidb/`, spill under `spill/` and cleaned on exit and startup |
| Bootstrap config (file locations, log filter) | `$XDG_CONFIG_HOME/unidb/config.toml` |
| Everything else | the `settings` table |

`config.toml` is bootstrap only. After startup the `settings` table is authoritative for every preference, including update check, history retention, and timeouts; the file is read once and never merged at runtime. Environment overrides exist only for tests and packaging: `UNIDB_DATA_DIR`, `UNIDB_LOG`, `UNIDB_NO_UPDATE_CHECK`, and `OLLAMA_HOST`. No secrets are read from environment variables.

### Performance budgets, enforced in CI

- Cold start, process start to first painted window, at most 1.5 seconds at the 95th percentile on the reference machine (4 cores, 8 GB, SSD), including any schema migration. The scaffold benchmark measures the migration path so a slow migration shows up before release.
- Idle memory at most 150 MB with no connections open, and at most 300 MB with three open.
- App layer query overhead at most 5 milliseconds at the 95th percentile, network time excluded.
- A 1,000,000 row result streams to the grid without the process growing past 200 MB above idle, because the view is virtualized and rows spill to disk.
- Table scrolling holds 60 frames per second at 100,000 rows.
- CI runs startup and hot path benchmarks and fails on a regression above 20 percent against the stored baseline.

### Testing and delivery

Unit tests cover domain logic, crypto, value conversion, and import parsing. Integration tests use testcontainers. Every pull request runs SQLite, PostgreSQL, and MySQL containers; the remaining six engines (MariaDB, CockroachDB, Redis, MongoDB, Cassandra, SQL Server) run in a nightly job. GTK components get tests under Xvfb with `GDK_BACKEND=x11`, the harness is documented so contributors can run it locally, and the release checklist covers Wayland and X11, HiDPI, locked and unlocked keyring, and a tunnel to a real host.

The pipeline runs formatting, clippy with warnings denied, cargo deny, cargo audit, tests, benchmarks, and the package builds. Release artifacts are the `.deb`, the AppImage, a `SHA256SUMS` file, and detached GPG signatures, all attached to a tagged GitHub release.

### Packaging identity

Package name and binary are `unidb`. The desktop entry is `io.github.sakib669.UniDB.desktop`, icons install under `hicolor/scalable/apps/io.github.sakib669.UniDB.svg`, and the Flatpak id is `io.github.sakib669.UniDB`. The `.deb` declares GTK 4.14 and libadwaita 1.5 as dependencies. The AppImage bundles GTK, libadwaita, GtkSourceView, and the Adwaita icon and theme data, targets glibc 2.39 and newer (Ubuntu 24.04 baseline), and is tested on Ubuntu 24.04, Fedora 40, and Debian 13.

### Explicitly out of scope for v1

Team accounts and any sync server, the plugin runtime, offline first sync, Windows and macOS, localization beyond English, embedded AI inference, and multiple main windows. These have roadmap entries below and get their own specs.

## Consequences

**Positive**:
- Native widgets and a tokio core make the startup and memory claims achievable rather than aspirational.
- No network surface and no webview remove a whole class of local attack and configuration problems.
- One driver contract keeps nine engines testable and replaceable.
- The keyring plus AES-256-GCM design means backups and shared bundles are safe by construction.
- Packaging to `.deb` and AppImage follows directly from a plain native binary.

**Negative / tradeoffs**:
- GTK4 makes a highly custom visual identity harder than CSS would; the premium look depends on disciplined libadwaita styling.
- AppImage must bundle GTK, libadwaita, and GtkSourceView, which makes it large (roughly 80 to 150 MB) and adds a compatibility test burden across distributions.
- Charts, the ERD, and every custom data view are hand drawn, which is real work the web path would have bought from a library.
- The GTK 4.14 and libadwaita 1.5 floor means Ubuntu 24.04, Fedora 40, and Debian 13 or newer for the `.deb`; older distributions need the bundled AppImage.
- Nine engine modules are a long term maintenance surface proportional to the count.
- The team must know Rust and GTK; hiring is narrower than for a web UI.
- WASM plugins later means designing a host API and sandbox, which is not free.

**Neutral**:
- The project moves from one crate to a workspace, which adds build structure but improves compile times and test isolation.
- Settings live in SQLite rather than GSettings, so there is one data store to back up.
- The browser preview documented in the README disappears and the README must be rewritten.
- A running Secret Service is required; headless environments are not supported.
- The app id `io.github.sakib669.UniDB` is now fixed by packaging, so changing it later is a migration.
- The engine registry uses dynamic trait objects, which costs one virtual dispatch per call and keeps engine crates independent.

## Delivery roadmap (next specs, in order)

This spec settles the foundation only. The remaining decisions are sequenced so each one has a spec before build starts.

1. **Workspace and CI scaffold**: move to the crate layout, pin the toolchain, add the pipeline, delete the fake audit and the stub drivers. Enables every later spec.
2. **Engine layer contract**: the traits above, the value and error model, capability flags, and the first three engines (SQLite, PostgreSQL, MySQL and MariaDB). Then CockroachDB, Redis, MongoDB, Cassandra, and MSSQL.
3. **App store and credential security**: SQLite migrations, keyring master key, AES-256-GCM envelope, read only enforcement, audit log, backup and recovery, session restore, the real Security screen replacing the hardcoded one.
4. **Design system and app shell**: libadwaita shell, navigation, command palette, dark and light themes, per engine colors, accessibility, empty and error states.
5. **Connection manager and database navigator**: profiles, URL parsing, TLS, SSH tunnels, test and connect, tree browsing with lazy loading.
6. **SQL editor and result grid**: GtkSourceView editor, multi tab, streaming results, virtualization, explain.
7. **Table browsing and designer**: browse data, edit rows, create and alter tables with DDL preview.
8. **Import and export**: CSV, JSON, SQL, clipboard, preview and conflict handling.
9. **History, saved queries, and settings**: persistence, FTS5 search, folders and tags.
10. **Packaging, signing, updates, and release automation**: `.deb`, AppImage, signed releases, update notice, SBOM.
11. **Visual features**: ERD, charts and dashboard, visual query builder, explain visual, data compare.
12. **Workflow features**: migrations and git, time travel and archive, fake data and masking, multi database query, team bundles.
13. **Platform features**: AI schema doctor, scheduled jobs, WASM plugin runtime, offline first sync.

## Follow-up

- [ ] Create `AGENTS.md` (or run the audit skill) so the stack and conventions above are recorded for later build steps; there is no `AGENTS.md` today.
- [ ] Delete the hardcoded `SecurityAudit` in `src/main.rs` in the scaffold spec; the real screen comes from the app store and credential security spec.
- [ ] Commit `Cargo.lock`, pin the toolchain, and extend `.gitignore` for local artifacts.
- [ ] Define the reference machine and store the first benchmark baselines, including the migration path.
- [ ] Confirm the app id and reserve the Flathub name; register the GPG release key and the update manifest ed25519 key.
- [ ] Decide the core license (README says MIT) and the plugin license before the plugin spec.
- [ ] Rewrite the README claims to match measured results after the first release.
- [ ] Define the crash report format and the update check privacy note.
- [ ] Record the installed skills and the declined and deferred ones in the `## Agent skills` section of `AGENTS.md` once it exists (root level: the GTK, relm4, and libadwaita skills apply to the whole project).
- [ ] Connect the experimental GTK MCP server (`alex-oleshkevich/gtk-mcp`) in the local MCP client configuration; the agent cannot connect it. Treat it as experimental, its adoption is unverified.
- [ ] Available but not installed, revisit when the matching engine work starts: `melonask/sqlx-skills` `sqlx`, official `redis/agent-skills`, official `mongodb/agent-skills`, official `scylladb/agent-skills`, `balloob/llm-skills` `ollama`, `razzeee/skills` `flatpak-flathub`, `github/awesome-copilot` `github-actions-hardening`, plus the official Redis, MongoDB, GitHub, and DbHub MCP servers.
- [ ] Decide whether `.agents/`, `.claude/`, and `skills-lock.json` are committed; if yes, add them to the repository in the scaffold spec, along with a short note on how to update skills.
- [ ] Evaluate the cross check simplification: ship SQLite, PostgreSQL, and MySQL first, defer the other six engines behind the same trait, and decide in the engine spec.
- [ ] Evaluate shipping `.deb` only for v1 and deferring AppImage until the GTK bundling and glibc baseline cost is measured; decide in the packaging spec.

## Rationale

Reasoning and options: see [rationale.md](rationale.md).
