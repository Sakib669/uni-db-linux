# 0014. Visual query builder

**Date**: 2026-11-18
**Status**: In Progress

## Summary

This slice adds a visual query builder. A user picks a live connection and a table, checks the columns to select, adds filters with an operator and a value, chooses an order column and direction, sets a limit, previews the generated SQL, and opens it in the editor or runs it. The SQL generation is a pure, dialect aware function in `unidb-core::builder` with tests, so the preview and the editor text come from the same rule set.

## Context

The editor requires SQL knowledge, which is exactly what a visual builder avoids for simple reads. The pieces already exist: live connections, table listing, column description, dialect quoting in the transfer module, and the editor that can receive generated text. A builder closes the gap for users who want to explore data without writing statements and is the last large visual feature after explain, charts, ERD, and compare.

## Requirements

**Acceptance criteria**
- **AC-1**: a Builder section lists live connections and their tables and describes the selected table's columns.
- **AC-2**: columns are checkable, at least one is required, and the selection order follows the table column order.
- **AC-3**: filters support equals, not equals, greater than, greater than or equal, less than, less than or equal, like, is null, and is not null, with quoted string values, and can be added and removed.
- **AC-4**: an order column with an ascending or descending direction and an optional row limit are applied.
- **AC-5**: the generated SQL is dialect aware for SQLite, PostgreSQL, CockroachDB, MySQL, and MariaDB, with validated identifiers and escaped values.
- **AC-6**: Preview renders the SQL and Open in editor loads it into the editor and switches sections.
- **AC-7**: invalid designs, such as no table, no columns, or invalid identifiers, produce clear errors instead of SQL.
- **AC-8**: generation is unit tested for every operator, escaping, order and limit, dialect quoting, and validation failures.
- **AC-9**: the builder is verified with a container screenshot of a generated query.

## Decision

- **Domain**: `unidb-core::builder` holds `QueryBuilder`, `QueryFilter`, `FilterOp`, and `build_query(builder, dialect)`. Identifiers are validated with the same rule as the table designer and quoted through the shared dialect helper. Values are always string literals with quote escaping; `IS NULL` and `IS NOT NULL` ignore the value; `LIKE` wraps the value in percent signs.
- **Validation**: a table is required, at least one column is required, every identifier must be valid, and a limit of zero is rejected. Filters reference columns of the chosen table only in the interface; the domain checks identifier validity and does not introspect.
- **Interface**: a Builder section with a connection selector, table selector, checkable column list, filter row with column, operator, value, and Add, a filter list with remove, order column selector with direction, limit spin, Preview, and Open in editor. The preview uses the same read only GtkSourceView as the designer and explain plan.
- **Execution**: Open in editor is the run path in this slice, so the builder never executes by itself and history/audit stay on the editor's single path.
- **Limits**: single table only, no joins, no aliases, no aggregates, no grouping, and string literal values only. Joins and aggregates are follow ups.
- **Testing**: pure unit tests in core and a container screenshot of the generated query.

## Build plan

1. Add the builder model, operator set, dialect aware generation, and tests, satisfies **AC-2**, **AC-3**, **AC-4**, **AC-5**, **AC-8**
2. Build the Builder page with connection, table, columns, filters, order, and limit, satisfies **AC-1**, **AC-2**, **AC-3**, **AC-4**
3. Add preview, open in editor, and error states, satisfies **AC-6**, **AC-7**
4. Verify the builder with a container screenshot, satisfies **AC-9**

## Consequences

**Positive**
- Simple reads become possible without SQL knowledge.
- The builder model is reusable for joins, aggregates, and the dashboard later.

**Negative / tradeoffs**
- String literal comparisons can behave unexpectedly against numeric columns depending on the engine.
- One table, no joins.
- The builder does not execute directly, so results still come from the editor.

**Neutral**
- Generated SQL is plain text the user can edit, which keeps the editor the source of truth.

## Follow-up

- [ ] Joins with relationship discovery from the ERD model.
- [ ] Aggregates, grouping, and having.
- [ ] Typed values and parameter preview.
- [ ] Run directly in the builder with a result grid.

## References

- Spec `0001`, visual features scope.
- Spec `0006`, editor target for generated SQL.
- Spec `0007`, identifier validation and dialect rules.
- Spec `0008`, identifier quoting helper.
- Spec `0012`, relationship model usable for joins later.
