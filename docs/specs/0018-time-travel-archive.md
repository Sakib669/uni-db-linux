# 0018. Time travel and archive

**Date**: 2026-11-18
**Status**: Complete

## Summary

This slice adds time travel snapshots and archive management for database tables. Before running risky queries, migrations, or data imports, a user can capture a point in time snapshot of a table. Snapshots are stored in the local SQLite app store with metadata, timestamp, and serialized rows. Users can list snapshots, compare a snapshot against live table data using keyed row diffs, and restore snapshot rows transactionally back into the table.

## Context

Database studios frequently run destructive statements like updates, deletes, drops, and bulk imports. When an accidental update or corrupting import occurs, recovering the previous state requires point in time recovery from database backups or manual reconstruction. Storing lightweight table snapshots in the local application store provides an immediate safety net that developers can create and restore without external backup infrastructure.

## Requirements

**Acceptance criteria**
* AC-1: a user can create a point in time snapshot of a table with connection, database, table name, row count, and an optional note.
* AC-2: snapshots are persisted in the SQLite store table `table_snapshots` with unique id, timestamps, and serialized column and row data.
* AC-3: snapshots can be listed for a connection, ordered newest first, with row counts and creation times.
* AC-4: a snapshot can be compared with live table state, producing a keyed row diff that shows unchanged, added, removed, and modified rows.
* AC-5: a snapshot can be restored transactionally to the live table, applying inserts through the existing transactional batch executor.
* AC-6: snapshot capture and restoration are recorded in the security audit log with table names and counts without logging row payload contents.
* AC-7: headless tests cover capture, retrieval, listing, diff comparison, and restoration on SQLite.
* AC-8: container GUI verification captures snapshots in the interface with a screenshot.

## Decision

* **Store schema**: new table `table_snapshots(id TEXT PRIMARY KEY, connection_id TEXT NOT NULL, database_name TEXT NOT NULL, table_name TEXT NOT NULL, row_count INTEGER NOT NULL, note TEXT, data_json TEXT NOT NULL, created_at TEXT NOT NULL)` in `unidb-store`.
* **Model**: `TableSnapshot` in `unidb-store` with id, connection id, database, table, row count, note, created at, and helper methods to serialize and deserialize `QueryResult`.
* **Service**: `AppService::create_snapshot(connection_id, database, table, note)` queries the table rows (capped at 500 rows for safe storage), records the snapshot row, and audits `snapshot_created`.
* **Listing**: `AppService::list_snapshots(connection_id)` lists stored snapshots for the connection.
* **Diffing**: `AppService::diff_snapshot(connection_id, snapshot_id)` reads the snapshot and queries current table state, running `diff_results` to report row level discrepancies.
* **Restore**: `AppService::restore_snapshot(connection_id, snapshot_id)` extracts columns and rows from the snapshot, generates transactional dialect insert statements, executes them via `apply_statements`, and audits `snapshot_restored`.
* **Interface**: History page gains a Snapshots preference group displaying recent snapshots with table name, row count, timestamp, and actions to compare or restore.

## Build plan

1. Add migration `0003_table_snapshots.sql` and store methods in `unidb-store` with tests, satisfies AC-2 and AC-3.
2. Add snapshot capture, listing, diffing, and restore methods in `unidb-app` with tests, satisfies AC-1, AC-4, AC-5, AC-6, AC-7.
3. Wire snapshots into the UI History and Editor views in `unidb-ui`, satisfies AC-1, AC-3, AC-4.
4. Verify with GUI flow and container screenshot, satisfies AC-8.

## Consequences

**Positive**
* Immediate local safety net before running destructive queries.
* Reuses existing diff results and transactional insert infrastructure.
* Audited and secure with zero external dependencies.

**Negative / tradeoffs**
* Snapshots are capped at 500 rows to keep the local store lightweight.
* Not intended for multi gigabyte table archiving.

## References

* Spec `0001`, workflow features roadmap.
* Spec `0003`, SQLite app store and audit logging.
* Spec `0006`, result diffing and plan pane.
* Spec `0008`, transactional insert batching.
