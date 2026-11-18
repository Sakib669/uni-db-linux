# 0020. AI schema doctor

**Date**: 2026-11-18
**Status**: Complete

## Summary

The Schema Doctor analyzes database schemas across live connections and produces actionable diagnostics and dialect aware SQL fixes. It runs deterministically offline over table column definitions and foreign keys, and optionally enriches explanations via a configurable AI endpoint (redacting row data and secrets so only structural metadata ever leaves the machine).

## Context

Developers frequently ship tables without primary keys, forget indexes on foreign key columns, or leave nullable columns in hot paths. Reviewing every table manually across multiple databases is slow. An automated schema doctor that produces concrete severity ranked findings and ready to run SQL remediation scripts solves this inside the studio.

## Requirements

**Acceptance criteria**
* AC-1: `diagnose_schema` in `unidb-core` inspects tables, columns, and foreign keys for a given `SqlDialect` and flags: missing primary keys, unindexed foreign keys, nullable primary or `id` columns, unconstrained `id` types, and empty tables with zero columns.
* AC-2: Each finding carries severity (`Error`, `Warning`, `Info`), table name, column name (optional), explanation, and an optional dialect specific remediation SQL statement (`CREATE INDEX`, `ALTER TABLE`, etc.).
* AC-3: `AppService::diagnose_connection_schema(&self, connection_id, database)` inspects all tables and foreign keys on an open connection, computes a `SchemaHealthReport` (overall score 0..100, counts by severity, findings), and records an audit event `schema_doctor_run`.
* AC-4: If an optional `UNIDB_AI_ENDPOINT` / setting is configured, `redact_schema_summary` ensures zero row payloads, passwords, or hostnames are included in any prompt payload.
* AC-5: The Platform page includes a Schema Doctor view with connection selection, "Run diagnosis", health score banner, grouped findings list with severity badges, and "Open fix in Editor".
* AC-6: Unit tests in `unidb-core`, integration tests in `unidb-app`, and container GUI screenshots verify the workflow.

## Decision

* **Domain module**: `crates/unidb-core/src/doctor.rs` with `FindingSeverity`, `SchemaFinding`, `TableSchemaInput`, `SchemaHealthReport`, `diagnose_schema(dialect, tables)`, and `redact_schema_prompt(report)`.
* **Service layer**: `AppService::diagnose_connection_schema(connection_id, database)` gathers `list_tables`, `describe_table`, and `foreign_keys` for SQL connections (up to 50 tables), calls `diagnose_schema`, and audits the run.
* **UI**: Platform sidebar row (index 11) with sub-tab switcher (`Schema Doctor`, `Jobs`, `Plugins`, `Offline Sync`). Schema Doctor shows connection selector, Run Diagnosis button, score status label, and findings list with "Open SQL" buttons that load the fix into the Editor.

## Build plan

1. Add `crates/unidb-core/src/doctor.rs` with unit tests.
2. Add `diagnose_connection_schema` to `AppService` in `crates/unidb-app` with integration tests.
3. Add Platform sidebar section and Schema Doctor view to `crates/unidb-ui`.
4. Verify with workspace tests, clippy, fmt, and container GUI screenshot.

## Consequences

**Positive**
* Immediate, zero cost offline schema audits with copyable/runnable DDL fixes.
* Strict privacy guarantee: no row values or connection secrets ever enter schema summaries.

**Negative / tradeoffs**
* Index introspection across dialects is approximated by primary key and unique flags exposed by `TableColumnInfo`; multi column composite indexes are not introspected in v1.
