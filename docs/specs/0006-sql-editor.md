# 0006. SQL editor and result grid

**Date**: 2026-11-17
**Status**: In Progress

## Summary

This feature gives UniDB its core working surface: a SQL editor with syntax highlighting, a Run action on a live connection, a cancel action where the engine supports it, a result grid that renders the shared value model, a status line with rows and timing, and clear error states. It turns the navigator from a window into a workflow, because now the user can run queries against the database they connected to.

## Context

The engine layer, the store, the connection manager, and the navigator all exist, but the only way to run a query is the scaffold probe. Every later feature (the table designer, import and export, charts, and AI help) assumes the user can write and run SQL. Spec 0001 chose GtkSourceView for the editor and a virtualized GtkColumnView for large results; this feature lands the first honest version of both: a real source view and a result table with a bounded in memory cap and a clear truncation notice. Virtualization and tabs are follow up milestones in the same feature.

## Requirements

**User stories**
- As a user, I pick a live connection, write SQL, and press Run.
- As a user, I see the columns and rows of the result, with nulls distinguished and timings shown.
- As a user, I cancel a long query on engines that support it.
- As a user, a failed query shows a clear error without losing my SQL.

**Acceptance criteria**
- **AC-1**: the Editor section lists live connections in a selector and runs against the selected one.
- **AC-2**: the editor is a GtkSourceView with SQL syntax highlighting, monospace text, and Ctrl+Enter runs the current statement.
- **AC-3**: Run executes through the connection actor on the worker runtime, and the interface stays responsive: Run disables and Cancel enables while a query runs.
- **AC-4**: the result grid renders the shared value model with one column per result column, nulls styled distinctly, up to 500 rows, and a truncation notice when more rows exist; the status line shows row count, affected count when applicable, and elapsed milliseconds.
- **AC-5**: a failed query shows a persistent error message with the safe error text and keeps the editor content.
- **AC-6**: Cancel calls the engine cancel path; engines without cancellation return a clear unsupported message instead of hanging the interface.
- **AC-7**: every executed query is recorded in history and in the audit log with its outcome and timing.
- **AC-8**: clearing results works and the editor stays ready for the next statement.
- **AC-9**: headless tests cover the cancel unsupported path and the query audit entry.
- **AC-10**: the editor flow is verified with a container screenshot that shows a real query result against a seeded SQLite file.

## Decision

- **Editor widget**: `sourceview5` 0.11 (the gtk-rs release matched to GTK4 0.11), with the SQL language from `LanguageManager` and the Adwaita style scheme. The view sits in a scrolled window and uses the monospace font from the theme.
- **Shortcut**: a key controller on the editor runs the statement on Ctrl+Enter and does nothing otherwise, so typing is never intercepted.
- **Execution**: one statement per Run, sent through `AppService::execute`, which already owns the actor, the history, and the timeouts policy. The interface receives a typed result or a typed error; it never calls a driver.
- **Cancellation**: a new actor command calls `Session::cancel`, which the engines implement or reject as unsupported. The UI surfaces the engine limitation honestly.
- **Result grid**: a `gtk::Grid` inside a scrolled window for v1, capped at 500 rows, with a muted NULL style and a header style. A virtualized `GtkColumnView` model replaces it in the follow up milestone when the grid needs millions of rows.
- **Audit**: the service writes a `query_run` audit entry for each execution with outcome, elapsed time, and row count, in addition to the history row it already writes. SQL text stays out of the audit detail.
- **Layout**: a vertical paned view, editor on top and result area below, with a status line and error label above the grid.

## Build plan

1. Add `sourceview5` and the editor page structure with the live connection selector, satisfies **AC-1**, **AC-2**
2. Add Run and Cancel to the app service and the actor, including the cancel unsupported path, satisfies **AC-3**, **AC-6**
3. Build the SQL source view, keyboard shortcut, and editor actions, satisfies **AC-2**, **AC-8**
4. Build the result grid, NULL styling, truncation notice, and status line, satisfies **AC-4**
5. Add the error state and clear action, satisfies **AC-5**, **AC-8**
6. Write the query audit entry and headless tests for cancel and audit, satisfies **AC-7**, **AC-9**
7. Verify the editor and result grid with a container screenshot and seeded data, satisfies **AC-10**

## Consequences

**Positive**
- The product finally does its core job: run SQL and show data.
- All execution stays behind the actor contract, so later multipane or tabs work reuses it.
- History and audit become meaningful because they record real queries.

**Negative / tradeoffs**
- The first grid is capped and not virtualized, so huge results are visibly truncated rather than fully browsable.
- Single editor tab only, so a user working across two statements switches by clearing.
- SQL errors are strings from the engine mapped to safe messages, so some dialect detail is hidden until the error taxonomy is richer.

**Neutral**
- The Studio probe page remains as a diagnostic example until the editor fully replaces it.
- The editor state lives in the page model, not the store; drafts are not persisted yet.

## Follow-up

- [ ] Virtualized grid model and streaming batches for large result sets.
- [ ] Multi tab editor with per tab connection and draft persistence.
- [ ] Cell selection, copy as CSV, and row details.
- [ ] EXPLAIN visualization.

## References

**Project sources**
- Spec `0001`, GtkSourceView and virtualized grid choices, execution and cancellation policy.
- Spec `0002`, session contract and engine capabilities.
- Spec `0003`, history and audit tables.
- Spec `0005`, live connections and the navigator.
- Installed skill `.agents/skills/designing-gnome-ui/`, feedback and control patterns.

**Practices & standards**
- One statement per run with a typed result, no interface level driver access.
- Bounded in memory display with an explicit truncation note rather than silent truncation.
- Keyboard first execution, no interception of normal typing.
