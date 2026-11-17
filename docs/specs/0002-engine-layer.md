# 0002. Engine layer and SQL family drivers

**Date**: 2026-11-17
**Status**: In Progress

## Summary

The engine layer turns a saved connection profile into a live session that returns the shared value model. This spec adds PostgreSQL, MySQL, MariaDB, and CockroachDB as the SQL family, then Redis, MongoDB, Cassandra, and MSSQL on the same contract, completing all nine engines. It also fixes the capability flags, TLS mapping, read only enforcement, cancellation, and error mapping for every engine.

## Context

Spec 0001 fixed the driver contract and built SQLite as the proof. The product promise is nine engines behind one interface, and the app services, the grid, and the exporters all expect the `Value` model. Now the contract must survive real servers: two wire protocols, three SQL dialects, TLS, pooled connections, cancellation, and the difference between a query and a statement. If each driver is written ad hoc, the grid will start seeing driver specific types and the error taxonomy will collapse into driver strings.

## Requirements

**User stories**
- As a user, I save a PostgreSQL, MySQL, MariaDB, or CockroachDB profile and run SQL against it.
- As a user, I see query results with the same column and value types as SQLite.
- As a security conscious user, read only connections refuse writes before they reach the server.
- As a user, I can cancel a running query on engines that support cancellation.

**Acceptance criteria**
- **AC-1**: a PostgreSQL profile connects, runs a select, and returns typed columns and rows in the shared value model.
- **AC-2**: a MySQL profile does the same against a real MySQL server.
- **AC-3**: a MariaDB profile does the same against a real MariaDB server.
- **AC-4**: a CockroachDB profile connects through the PostgreSQL wire protocol without a second driver implementation.
- **AC-5**: read only sessions reject write statements with the `read_only_blocked` error before the server sees them, and the session is also configured read only at the server.
- **AC-6**: PostgreSQL and MySQL sessions cancel a running statement through their engine specific path; engines that cannot cancel return `cancel_unsupported`.
- **AC-7**: every failure maps to `UniDbError` with the error taxonomy, a safe user message, and no SQL text in `Display`.
- **AC-8**: integration tests start real PostgreSQL, MySQL, and MariaDB servers with testcontainers and pass in CI.
- **AC-9**: the registry advertises only engines built into the binary, and a missing engine returns `engine_not_installed`.
- **AC-10**: a Redis profile runs a command line and maps scalars, arrays, and maps into the shared value model, with a read command allowlist for read only sessions.
- **AC-11**: a MongoDB profile runs a JSON command document and returns the reply as structured JSON, with a read command allowlist for read only sessions.
- **AC-12**: a Cassandra profile connects with optional plain text auth and runs CQL, mapping CQL values into the shared value model.
- **AC-13**: an MSSQL profile connects over TDS, runs a statement, maps column data into the shared value model, and uses SQL transactions.

## Decision

Add two driver modules over sqlx 0.9:

- `unidb-engine/src/postgres.rs` serves `Engine::PostgreSQL` and `Engine::CockroachDB`.
- `unidb-engine/src/mysql.rs` serves `Engine::MySQL` and `Engine::MariaDB`.

sqlx provides one pool and row model for both wire protocols, including TLS through rustls, which keeps the value mapping and error mapping close to the SQLite module. Each session takes one pooled connection so transactions apply to later statements, and the pool is retained for the cancellation path.

**TLS mapping**: `disable` to `Disabled`, `prefer` to `Prefer`, `require` to `Require`, `verify_ca` to `VerifyCa`, `verify_full` to `VerifyFull`. The `core` profile validation still rejects a non local host with TLS disabled.

**Read only**: the session layer rejects non query statements with `read_only_blocked`, and the driver also sets a read only session at the server (`SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY` for PostgreSQL and CockroachDB, `SET SESSION TRANSACTION READ ONLY` for MySQL and MariaDB).

**Cancellation**: PostgreSQL records `pg_backend_pid()` and cancels with `SELECT pg_cancel_backend($1)` over the pool. MySQL records `CONNECTION_ID()` and cancels with `KILL QUERY <id>`, formatting the integer id only. Both are documented as engine specific; SQLite keeps returning `cancel_unsupported`.

**Pooling**: five connections, ten second acquire timeout, no statement timeout by default so a long query stays under user control (spec 0001). Idle connections are recycled after five minutes.

**Value conversion**: shared enum, driver specific match on the type name with a safe fallback chain. Scalars cover booleans, integers, floats, decimals, text, bytes, JSON, timestamps, dates, and UUIDs. Arrays and engine specific types fall back to a readable text marker so the grid never crashes on an unfamiliar type; richer array support is a follow up once the grid exists.

**Errors**: one mapping per driver to the taxonomy. The SQL text is never attached to the error, matching spec 0001.

### Non SQL engines

- **Redis 1.8**: one multiplexed connection manager per profile. A statement is a command line with quoting supported. Read only sessions allow a fixed read command allowlist. `MULTI`, `EXEC`, and `DISCARD` back the transaction methods.
- **MongoDB 3.10**: a statement is a JSON command document, which is the honest v1 shape because MongoDB has no SQL dialect. Read only sessions allow a fixed read command allowlist. Transactions and cancellation are reported unsupported, and the capability flags reflect that.
- **Cassandra through scylla 1.10**: plain text authentication when a username and password are present. CQL statements return rows and values. CQL has no SQL style transactions or external cancellation, and the capability flags reflect that.
- **MSSQL through tiberius 0.13**: TDS over TCP with the encryption level mapped from the TLS mode. Certificate chain verification is limited to `trust_cert` in this build and is a follow up. Transactions use `BEGIN`, `COMMIT`, and `ROLLBACK` statements. Cancellation is reported unsupported, and the capability flags reflect that.

## Proposed stack additions

| Layer | Choice | Reason |
|---|---|---|
| PostgreSQL and CockroachDB | sqlx 0.9 `postgres` feature | One pool and row model, TLS through rustls, battle tested. |
| MySQL and MariaDB | sqlx 0.9 `mysql` feature | Same API, both protocols. |
| TLS | sqlx `tls-rustls-ring-native-roots` | System certificate store, pure Rust, matches spec 0001. |
| Container tests | testcontainers 0.28 | Real engine in tests, works on the GitHub runner Docker. |
| Redis | redis 1.8 with `tokio-comp` and `connection-manager` | Async multiplexed client, command reply mapping. |
| MongoDB | mongodb 3.10 | Official driver, JSON command execution. |
| Cassandra | scylla 1.10 | Pure Rust CQL driver with pooling. |
| MSSQL | tiberius 0.13 | Pure Rust TDS client, no FreeTDS. |

## Build plan

1. Add the sqlx postgres and mysql features and the testcontainers dev dependency, satisfies **AC-1**, **AC-8**
2. Extend the registry with shared registration so CockroachDB and MariaDB reuse the PostgreSQL and MySQL connectors, satisfies **AC-4**, **AC-9**
3. Implement the PostgreSQL session: connect, TLS mapping, read only session, execute, typed rows, transactions, cancel, satisfies **AC-1**, **AC-5**, **AC-6**, **AC-7**
4. Implement the MySQL session with the same surface using `KILL QUERY`, satisfies **AC-2**, **AC-5**, **AC-6**, **AC-7**
5. Register the new connectors in the app service bootstrap, satisfies **AC-1**, **AC-2**, **AC-9**
6. Integration tests with testcontainers for PostgreSQL, MySQL, and MariaDB; CockroachDB through the PostgreSQL path once its container image is verified, satisfies **AC-3**, **AC-4**, **AC-8**
7. Add the engine tests to CI and keep the SQLite suite unchanged, satisfies **AC-8**
8. Implement the Redis session: command line parsing, reply mapping, read command allowlist, transactions, satisfies **AC-10**
9. Implement the MongoDB session: JSON command parsing, reply mapping, read command allowlist, satisfies **AC-11**
10. Implement the Cassandra session: connection with optional plain text auth, CQL execution, value mapping, satisfies **AC-12**
11. Implement the MSSQL session: TDS connection with encryption mapping, statement execution, column data mapping, transactions, satisfies **AC-13**
12. Register all nine engines in the app service bootstrap, satisfies **AC-9**

## Consequences

**Positive**
- Four engines work through one contract without one-off UI code.
- Real container tests catch protocol and type mapping mistakes that mocks would miss.
- Read only is enforced twice, at the session layer and at the server.

**Negative / tradeoffs**
- sqlx postgres and mysql features grow the compile time and binary.
- Container tests need Docker on the runner and add a few minutes to CI.
- Array and engine specific types are lossy until a later grid focused spec.
- Cancellation depends on engine specific statements and can fail on restricted permissions.

**Neutral**
- CockroachDB and MariaDB share a connector, so an engine specific quirk has to be handled inside the shared module.
- The value fallback marker is visible to users for exotic types until arrays are supported.
- MongoDB takes a JSON command rather than SQL, so the editor needs an engine aware language mode later.
- MSSQL trusts the server certificate, so a hostile network can intercept a connection whose TLS mode asked for verification until full chain verification lands.
- Cassandra and MSSQL cannot cancel from this build, so the interface must disable the cancel control based on capability flags.
- Redis transactions run on a multiplexed connection, so unrelated commands from the same session can join the transaction; the session must not be shared.

## Follow-up

- [ ] Redis, MongoDB, Cassandra, and MSSQL connectors in a later engine spec.
- [ ] Array types and engine specific rich types once the result grid exists.
- [ ] Statement timeouts once user settings exist.

## References

**Project sources**
- Spec `0001`, the driver contract, value model, error taxonomy, and TLS rules.
- `.agents/skills/gtk4-libadwaita-internals` is not relevant here; the installed Rust skills do not cover sqlx.

**Practices & standards**
- One pool per connection profile, one connection per transaction scope.
- Parameterized statements for values, direct SQL only for user typed statements.
- TLS verification on by default for remote hosts.
- Integration tests against real services, not mocks, for protocol code.
