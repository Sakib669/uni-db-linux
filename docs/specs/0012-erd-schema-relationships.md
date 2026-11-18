# 0012. ERD and schema relationships

**Date**: 2026-11-17
**Status**: In Progress

## Summary

This feature adds an entity relationship diagram to UniDB. A new Schema section loads every table in the connected database together with its columns and foreign keys, then draws table cards and relationship arrows on a scrollable canvas. Foreign key introspection joins the session contract for SQLite, PostgreSQL, CockroachDB, MySQL, and MariaDB, and the layout is a pure function with tests so card placement and edge resolution do not depend on GTK. It is the second slice of the visual features after explain and charts.

## Context

The navigator lists tables one by one, so users cannot see how tables relate. An ERD is the standard answer and the data is already close: table listing and column description exist from the designer, and only foreign key metadata is missing. Engines that are not relational have no ERD to draw and should say so honestly. The diagram needs to stay useful with dozens of tables, so it scrolls and caps the tables it loads rather than trying to render an unbounded schema.

## Requirements

**User stories**
- As a user, I open Schema and see my tables as cards with their columns.
- As a user, I see arrows between tables where foreign keys exist.
- As a user, I understand when the engine has no relational schema to draw.

**Acceptance criteria**
- **AC-1**: a Schema section lists live connections and loads the tables of the selected database automatically.
- **AC-2**: every table card shows the table name and its columns, with foreign key columns marked.
- **AC-3**: foreign key relations draw as arrows between the referencing and referenced cards, labelled with the referencing column.
- **AC-4**: foreign key introspection works for SQLite, PostgreSQL, CockroachDB, MySQL, and MariaDB; other engines show an explanatory empty state.
- **AC-5**: the canvas scrolls, caps loading at 24 tables, and reports how many tables are shown.
- **AC-6**: card placement and edge resolution are a pure function in `unidb-core::visual` with tests for grid placement, missing target tables, and self references.
- **AC-7**: failures while loading a table are reported per table in a status line without crashing the page, and a connection without tables shows an empty state.
- **AC-8**: headless tests cover SQLite foreign key introspection and the service method.
- **AC-9**: the diagram is verified with a container screenshot of a seeded schema with one relationship.

## Decision

- **Session contract**: a new `list_foreign_keys(database, table)` default method returns unsupported. SQLite uses `PRAGMA foreign_key_list`, PostgreSQL reads `information_schema` constraints with `constraint_column_usage`, MySQL reads `information_schema.key_column_usage` where `referenced_table_name` is set. MSSQL and the non relational engines keep the default and the interface explains why.
- **Model**: `ForeignKeyInfo` lives in `unidb-core::schema`; `ErdTable`, `ErdBox`, `ErdEdge`, `ErdLayout`, and `erd_layout(tables, width)` live in `unidb-core::visual`. Layout uses a fixed card width, a responsive column count from the canvas width, and uniform row heights based on the longest visible column list.
- **Interface**: a `GtkDrawingArea` inside a scrolled window renders the layout with cairo: rounded cards, a title, columns, `FK` markers, teal arrows with arrowheads, and labels. The page shows the connection selector, a status line, and the canvas. Table loading happens in one worker task that calls describe and foreign keys per table and reports a combined result.
- **Limits**: no drag and drop, no zoom, no panning beyond scroll, no layout optimisation for very large schemas. The cap of 24 tables is deliberate and reported. Clicking a card to open a table is a follow up.
- **Tests**: pure layout tests in core, SQLite introspection test in the engine, and a service test that creates a parent and child table and asserts the relationship.

## Build plan

1. Add `ForeignKeyInfo` and the ERD layout function with tests, satisfies **AC-2**, **AC-3**, **AC-6**
2. Add `list_foreign_keys` to SQLite, PostgreSQL, and MySQL and cover SQLite, satisfies **AC-4**, **AC-8**
3. Add the service method and its SQLite test, satisfies **AC-8**
4. Build the Schema page, connection selection, table loading, and status, satisfies **AC-1**, **AC-5**, **AC-7**
5. Render cards, columns, FK markers, arrows, and labels with cairo, satisfies **AC-2**, **AC-3**
6. Verify the ERD with a seeded relationship screenshot, satisfies **AC-9**

## Consequences

**Positive**
- Relationships become visible, which makes joins and surprises easier to spot.
- Foreign key metadata is now available for future features such as join suggestions and migrations.
- The layout function is reusable for the dashboard.

**Negative / tradeoffs**
- The diagram is static; dragging, zooming, and clicking wait for follow ups.
- Only 24 tables show at once.
- Foreign key introspection adds one query per table, so large schemas load slowly; a batching query is a future optimisation.

**Neutral**
- The Schema section uses the same connection selection pattern as Designer and Transfer.
- Arrows are drawn between cards without routing around each other, which is acceptable for a first version.

## Follow-up

- [ ] Click a card to open the table in the editor and a details panel.
- [ ] Drag cards, persist positions, zoom and fit to view.
- [ ] Batch foreign key queries and lazy loading for large schemas.
- [ ] Data compare built on the same schema model.

## References

**Project sources**
- Spec `0001`, visual features scope.
- Spec `0005`, table listing and connection selection.
- Spec `0007`, column introspection and the SQL dialect rules.
- Spec `0011`, visual helper module this feature extends.
- Installed skill `.agents/skills/designing-gnome-ui/`, canvas and empty state patterns.

**Practices & standards**
- Relationship model and layout in the domain layer with tests.
- One worker task per schema load, interface never blocks.
- Honest empty states for non relational engines.
