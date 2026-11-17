# 0011. Visual features

**Date**: 2026-11-17
**Status**: In Progress

## Summary

This feature begins the visual layer of UniDB. The first slice delivers two things users feel immediately: an explain plan pane that runs the engine's explain statement for the current SQL and shows the plan beside the result, and a chart view that turns the current result set into a bar chart without leaving the editor. A pure domain module decides the explain statement per engine and extracts chart series from the shared value model, so both are testable without GTK. The remaining visual features named in spec 0001, ERD, dashboard, visual query builder, and data compare, are sequenced as follow up milestones with recorded decisions.

## Context

The editor shows rows but not why a query is slow, and it shows numbers but not their shape. Those are the two highest value visual additions for a database studio and both are small enough to land honestly on top of the existing result pipeline. The engine sessions already execute arbitrary statements, and the result model already carries typed values, so explain and charts need no new driver work. ERD needs foreign key introspection that does not exist yet, the query builder needs a different interaction model, and data compare needs two result sets, so those are separate builds rather than a single round.

## Requirements

**User stories**
- As a user, I press Explain and see the engine's plan for my statement without losing the editor content.
- As a user, I press Chart and see my result as a bar chart, then switch back to the table.
- As a user, I understand which column became the bars and which column became the labels.

**Acceptance criteria**
- **AC-1**: the editor runs the correct explain statement per engine: `EXPLAIN QUERY PLAN` for SQLite, `EXPLAIN (FORMAT JSON)` for PostgreSQL and CockroachDB, `EXPLAIN FORMAT=JSON` for MySQL and MariaDB; engines without explain return a clear unsupported message.
- **AC-2**: a plan pane appears with the plan output, pretty printed when the engine returns JSON, and stays visible until dismissed or until the next explain.
- **AC-3**: the plan pane does not replace the result grid, and a failed explain shows the safe error without clearing results.
- **AC-4**: the Chart action renders a bar chart from the current result: the first numeric column supplies values, the first non numeric column supplies labels, nulls and non numeric values are skipped, and at most 50 bars are drawn.
- **AC-5**: the user can switch between the table and the chart, and a result without a numeric column explains why there is no chart.
- **AC-6**: explain statement selection and chart series extraction are pure functions with unit tests for every engine, JSON and text plans, numeric detection, null skipping, label fallback, and the 50 bar cap.
- **AC-7**: the chart and the explain pane are verified with container screenshots.
- **AC-8**: ERD, dashboard, visual query builder, and data compare are recorded as follow ups with the decisions they need, not silently dropped.

## Decision

- **Explain statement selection**: a pure function `explain_statement(engine, sql)` in `unidb-core::visual` trims a trailing semicolon and wraps the statement per engine. MSSQL, Redis, MongoDB, and Cassandra return the existing unsupported error because their explain output is not a SQL plan in this build.
- **Execution**: the editor calls the same `execute` path as Run, so history and audit treat explain runs like any other statement. The plan is rendered from the returned rows: one row per plan line, values joined, and a single JSON value pretty printed through serde_json.
- **Plan pane**: a `GtkRevealer` under the result area containing a read only GtkSourceView, so the plan is selectable and copyable. It starts hidden and is populated by the Explain action.
- **Charts**: the result area becomes a small `GtkStack` with the table and a `GtkDrawingArea`. A renderer draws horizontal bars with cairo, labels truncated to fit, values printed after each bar, and the teal accent color from the theme. The data comes from `chart_series(columns, rows, 50)` in the core module, so the rules are covered by unit tests. Charts are a view of the last result, not a second query.
- **Charts are deliberately simple**: no axes library, no chart types beyond bars, no export in this slice. A chart export and more types belong to the dashboard milestone.
- **Follow ups**:
  - ERD: needs `list_foreign_keys(database, table)` on the session contract and a canvas renderer.
  - Dashboard: needs persistent chart definitions in the store and a multi chart page.
  - Visual query builder: needs a select builder model that emits SQL and maps back from the editor text.
  - Data compare: needs two result snapshots and a row diff on a chosen key.
- **Verification**: the UI flow runs a query, opens the chart, runs explain on the same statement, and captures both panes.

## Build plan

1. Add `unidb-core::visual` with explain statement selection and chart series extraction plus unit tests, satisfies **AC-1**, **AC-6**
2. Add the Explain action, execution, JSON pretty printing, and the plan pane, satisfies **AC-2**, **AC-3**
3. Add the results stack, chart drawing area, bar renderer, and table and chart switching, satisfies **AC-4**, **AC-5**
4. Wire the editor toolbar actions and empty explanations, satisfies **AC-3**, **AC-5**
5. Verify the chart and explain pane with container screenshots, satisfies **AC-7**
6. Record the ERD, dashboard, query builder, and compare follow ups in the scope, satisfies **AC-8**

## Consequences

**Positive**
- Users get plan and shape insight without leaving the editor.
- Both features reuse the existing execution and result model, so no driver work and no second query paths.
- The pure visual module is a foundation for the dashboard and data compare later.

**Negative / tradeoffs**
- The plan pane shows engine output, not an interactive tree; reading stays close to raw SQL plans.
- Bar charts only, capped at 50 points, with simple cairo text.
- ERD and query builder, the most requested visual features, wait for their own specs.

**Neutral**
- Explain runs are recorded in history like normal statements.
- Charts render the last result, so clearing results also clears the chart.

## Follow-up

- [ ] ERD with foreign key introspection and layout.
- [ ] Dashboard with saved chart definitions.
- [ ] Visual query builder.
- [ ] Data compare and result diff.
- [ ] Chart export and additional chart types.

## References

**Project sources**
- Spec `0001`, visual features scope and explain requirement in the editor.
- Spec `0002`, session contract and engine capabilities.
- Spec `0006`, editor, result model, and execution path.
- Spec `0009`, history recording reused by explain runs.
- Installed skill `.agents/skills/designing-gnome-ui/`, chart and feedback patterns.

**Practices & standards**
- Pure extraction and selection logic in the domain layer with unit tests.
- Reuse one execution path, never add a parallel query route.
- Render from the shared value model, no driver types in the interface.
