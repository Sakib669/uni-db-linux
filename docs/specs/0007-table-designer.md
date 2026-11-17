# 0007. Table designer

**Date**: 2026-11-17
**Status**: Proposed

## Summary

This feature adds a visual table designer: create a table by naming columns and setting types, nullability, defaults, primary keys, uniqueness, and auto increment, then preview the exact DDL before applying it. It also loads an existing table from the navigator, lets the user add, rename, or drop columns, and applies the changes as dialect specific `ALTER TABLE` statements inside a transaction. The designer is limited to the SQL engines in this build, and the other engines explain why they are not supported.

## Context

The editor makes the user write every statement by hand. A database studio also needs the guided path: define a table through controls, see the SQL that will run, and apply it safely. The engine sessions already list databases and tables for the navigator, and the connection actors already execute statements, so the designer can reuse both. The missing pieces are a dialect aware DDL model in the domain layer, column introspection for existing tables, and a transaction command so a multi statement apply is all or nothing.

## Requirements

**User stories**
- As a user, I create a table by filling in a name and a list of columns.
- As a user, I set a column's type, nullability, default, primary key, uniqueness, and auto increment.
- As a user, I preview the DDL before running it.
- As a user, I open an existing table, add or rename or drop a column, and apply the change.

**Acceptance criteria**
- **AC-1**: the Designer section lists live connections and their tables, and supports a new table mode and editing an existing table.
- **AC-2**: the column editor captures name, type, nullable, default, primary key, unique, and auto increment, with add and remove column actions and a minimum of one column.
- **AC-3**: the DDL preview shows the exact statements that Apply will run, generated for the target engine dialect.
- **AC-4**: validation rejects empty or invalid identifiers, duplicate column names, a table without columns, auto increment on a non integer type, and auto increment on more than one column, with a clear message and no execution.
- **AC-5**: Apply runs the statements inside a transaction through the connection actor: on any error the transaction rolls back and the error is reported safely; on success the table list refreshes and a toast confirms.
- **AC-6**: editing an existing SQL table loads its columns from the engine and generates `ALTER TABLE` statements for added, renamed, and dropped columns, with unchanged columns omitted.
- **AC-7**: engines without designer support show an explanatory empty state instead of broken controls.
- **AC-8**: headless tests cover DDL generation for SQLite, PostgreSQL, and MySQL, validation failures, and a real SQLite create and alter cycle through the service.
- **AC-9**: the designer flow is verified with a container screenshot showing the preview and the applied result.

## Decision

- **Domain layer**: a new `unidb-core::schema` module owns `TableDesign`, `ColumnDesign`, and `SqlDialect` (SQLite, PostgreSQL, MySQL) with `create_table_sql`, `alter_table_sql`, and `validate`. The interface never concatenates SQL itself; it asks this module for statements, so the preview and the executed statements are the same text by construction.
- **Dialects**: three families cover the supported engines: SQLite (also used for `AUTOINCREMENT` and `PRAGMA` style introspection), PostgreSQL (also CockroachDB), and MySQL (also MariaDB). Quoting uses double quotes for SQLite and PostgreSQL, backticks for MySQL. Identifiers are validated as `[A-Za-z_][A-Za-z0-9_]*` before quoting, so quoting cannot be exploited.
- **Introspection**: a new `describe_table(database, table)` session method returns ordered `TableColumnInfo` rows. SQLite uses `PRAGMA table_info`, PostgreSQL uses `information_schema.columns` with the primary key join, MySQL uses `information_schema.columns` with `COLUMN_KEY`. Other engines return the existing unsupported error.
- **Transaction apply**: a new `ApplyStatements` actor command opens a transaction, executes each statement, rolls back on the first failure, and commits only on success. The designer's apply uses it, so partial DDL cannot land.
- **Rename tracking**: the designer keeps the original name for loaded columns. A changed name becomes a rename statement (SQLite uses `ALTER TABLE ... RENAME COLUMN`, PostgreSQL and MySQL use the same modern syntax), a missing original column becomes a drop, and a new column becomes an add.
- **Limits**: type changes, index editing, foreign keys, and generated columns are follow-ups. The UI makes the generated SQL visible, so a user can always switch to the editor for anything the designer does not cover.

## Build plan

1. Add the DDL module with validation and per dialect generation plus unit tests, satisfies **AC-3**, **AC-4**, **AC-8**
2. Add `describe_table` to the SQL sessions and cover SQLite in tests, satisfies **AC-6**
3. Add the transaction apply command and service methods with SQLite create and alter tests, satisfies **AC-5**, **AC-8**
4. Build the Designer page with connection and table selection, new table mode, and the column editor, satisfies **AC-1**, **AC-2**
5. Add the preview, apply flow, status and toasts, and unsupported engine empty state, satisfies **AC-3**, **AC-5**, **AC-7**
6. Add edit existing table with add, rename, and drop generation, satisfies **AC-6**
7. Verify the designer flow with a container screenshot, satisfies **AC-9**

## Consequences

**Positive**
- Guided table creation with a preview makes the app usable without memorizing DDL.
- Dialect differences live in one tested module instead of scattered strings.
- Transactional apply prevents half created tables.

**Negative / tradeoffs**
- Only SQL engines are covered; document and key value engines remain query only.
- Type changes, indexes, and foreign keys wait for follow-ups.
- Introspection is a new per engine code path that needs live container coverage over time.

**Neutral**
- The designer talks to the same actors as the editor, so busy states are shared per connection.
- SQLite `ALTER TABLE` support depends on the bundled SQLite version, which is modern in this build.

## Follow-up

- [ ] Change column type and reorder columns through table rebuilds where required.
- [ ] Index and foreign key editing with a relationship view.
- [ ] Schema diff against a target database for migrations.
- [ ] Save designs as reusable templates.

## References

**Project sources**
- Spec `0001`, table designer place in the app and transaction rules.
- Spec `0002`, session contract and engine capabilities.
- Spec `0005`, table listing and the navigator this feature extends.
- Spec `0006`, execution path and error surfaces reused by Apply.
- Installed skill `.agents/skills/designing-gnome-ui/`, forms and feedback patterns.

**Practices & standards**
- One SQL generation module per dialect with identifier validation before quoting.
- Preview equals execution: the same generated statements feed the preview and Apply.
- Multi statement changes run in a transaction with rollback on failure.
