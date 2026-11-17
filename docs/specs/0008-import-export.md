# 0008. Import and export

**Date**: 2026-11-17
**Status**: In Progress

## Summary

This feature moves data in and out of UniDB. Import reads a CSV or JSON file, maps its fields to a live table's columns, coerces values to the column types, previews the parsed and rejected rows, and inserts in transactional batches with a report. Export runs the current SQL statement and writes the result as CSV or JSON through a save dialog. The core parsing, coercion, statement generation, and formatting live in a tested domain module, so the interface only collects options and shows reports.

## Context

The studio can create tables and run queries, but real work moves data between files and databases. Users arrive with a CSV export from another tool or a JSON dump and need it in a table without writing INSERT statements by hand. They also need to get results out for reports and sharing. The engine sessions already describe tables and apply statements transactionally, and the editor already owns query execution, so both paths can be composed from existing contracts.

## Requirements

**User stories**
- As a user, I choose a live table and a CSV or JSON file and import its rows.
- As a user, I see how many rows were read, inserted, and rejected, with reasons for the first rejections.
- As a user, I choose whether the first CSV row is a header and whether to map columns by name.
- As a user, I export the result of the current query as CSV or JSON to a file I choose.

**Acceptance criteria**
- **AC-1**: the Transfer section lists live connections and their tables, takes a file path with a Browse action, a format choice (CSV or JSON), a header switch, and an Import action.
- **AC-2**: CSV parsing follows quoting and escaped quotes and supports comma, semicolon, and tab delimiters; JSON accepts an array of objects and newline delimited objects.
- **AC-3**: import maps file fields to table columns case insensitively, rejects unknown fields with a warning, and requires a mapping for every column that is not nullable and has no default.
- **AC-4**: values are coerced by target type: integers, floats, booleans, and text; empty fields become NULL when a null marker is configured, and a row with a coercion failure is rejected with a warning instead of aborting the import.
- **AC-5**: rows insert in batches inside transactions; the report states rows read, rows inserted, rows rejected, batches, and the first few rejection reasons.
- **AC-6**: import never modifies the file and never runs user supplied SQL text from the file.
- **AC-7**: export writes CSV with a header row and JSON as an array of objects, with nulls represented honestly; the report states rows written and the path.
- **AC-8**: every import and export writes an audit entry with counts, outcome, and no row contents.
- **AC-9**: headless tests cover CSV and JSON parsing, coercion failures, insert generation and quoting, SQLite import and export round trips, and audit entries.
- **AC-10**: the import flow is verified with a container screenshot showing a completed import report.

## Decision

- **Domain module**: a new `unidb-core::transfer` module owns `TransferFormat` (CSV, JSON), `ImportData` (headers plus string rows), `parse_import`, `coerce_value`, `insert_statements`, `rows_to_csv`, and `rows_to_json`. The `csv` crate handles RFC 4180 parsing and writing; `serde_json` handles JSON. Parsing and generation are pure functions with unit tests.
- **Coercion**: target type names are matched by family: integer types parse as `i64`, floating types and decimals as `f64`, boolean and bit types accept true, false, 0, and 1, everything else stays text. Numbers must parse exactly; a failure rejects the row, it never silently becomes text in a typed column.
- **Mapping**: headers are matched to table columns case insensitively. Extra file fields are reported and ignored. Table columns without a file field are allowed only when they are nullable or have a default; otherwise the import stops before writing anything with a clear mapping error.
- **Insert generation**: the core builds multi row `INSERT INTO` statements with quoted identifiers and literal escaping, batched at 200 rows. The service applies each batch through the existing transactional `apply_statements` command, so a failed batch rolls back and the report says which batch failed.
- **Null handling**: the import options carry an optional null marker; when a CSV field equals it exactly, the value becomes NULL. JSON nulls become NULL directly.
- **Export**: the editor adds Export CSV and Export JSON actions. They run the current statement, then ask for a save path through `GtkFileDialog`, and the service writes the file through the core formatters. Nulls become empty CSV fields and JSON nulls.
- **Audit**: `import_finished` and `export_finished` entries record rows in and out, format, outcome, and error codes. File paths are recorded in the import audit for traceability; row contents never are.
- **Limits**: only INSERT imports, no update or upsert modes, no column remapping UI beyond header names, no streaming for files larger than memory in this version. Those become follow up milestones.

## Build plan

1. Add the transfer module with CSV and JSON parsing, coercion, insert generation, and export formatting plus unit tests, satisfies **AC-2**, **AC-4**, **AC-6**, **AC-9**
2. Add import and export service methods using describe, apply statements, and execute, with SQLite round trip tests and audit entries, satisfies **AC-3**, **AC-5**, **AC-7**, **AC-8**, **AC-9**
3. Build the Transfer page with connection and table selection, format and header options, path entry, and Browse, satisfies **AC-1**
4. Wire the import action, report status, and toasts, satisfies **AC-1**, **AC-5**
5. Add Export CSV and Export JSON actions to the editor with a save dialog and toasts, satisfies **AC-7**
6. Verify the import flow with a seeded CSV and a container screenshot, satisfies **AC-10**

## Consequences

**Positive**
- Files and databases connect in both directions without hand written SQL.
- Pure domain parsing and generation are fast to test and safe to reuse for the upcoming clipboard and chart features.
- Transactional batches keep failed imports from landing half done.

**Negative / tradeoffs**
- Large files load fully into memory in this version.
- Mapping is by header name only; there is no interactive column remap.
- Insert only; update and upsert wait for a follow up.

**Neutral**
- Import uses the same transactional apply path as the table designer, so busy and error handling stay consistent.
- Export reuses editor execution instead of a second query path.

## Follow-up

- [ ] Streaming import for large files with progress and cancel.
- [ ] Interactive column mapping, update and upsert modes, dry run preview.
- [ ] Excel and Parquet formats.
- [ ] Delimited export presets and clipboard export.

## References

**Project sources**
- Spec `0001`, import and export scope and audit rules.
- Spec `0005`, table listing used by the Transfer page.
- Spec `0006`, execution and export entry point.
- Spec `0007`, transactional apply and table description reused here.
- Installed skill `.agents/skills/designing-gnome-ui/`, file dialogs and feedback patterns.

**Practices & standards**
- RFC 4180 CSV handling through a maintained crate, not split on commas.
- Pure domain parsing and generation with unit tests, no interface level file logic.
- Batched transactional inserts with explicit rejection reports.
