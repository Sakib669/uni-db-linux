# 0016. Masking exported data

**Date**: 2026-11-18
**Status**: In Progress

## Summary

This slice adds masking to exports. Alongside the normal CSV and JSON export, the editor can export a masked copy where free text, emails, identifiers, and timestamps are replaced with safe constants while numbers and booleans stay intact. Masking is a pure function in `unidb-core::mask` with tests, and the service writes the masked file through the same query and formatting path with an audit entry.

## Context

Exports often leave the machine for support, analytics, or bug reports, and real names and emails should not travel with them. The export path already receives typed values, so masking is one transform between the query result and the formatter. The fake data slice covers generating new rows; masking covers existing data on the way out.

## Requirements

**Acceptance criteria**
- **AC-1**: the Export menu offers CSV and JSON plus masked CSV and masked JSON.
- **AC-2**: masking replaces text values with a fixed marker, emails with a safe placeholder, and uuid, json, binary, array, and timestamp values with safe markers, while nulls, integers, floats, decimals, and booleans pass through.
- **AC-3**: masked exports never contain original text values.
- **AC-4**: masking is a pure function with unit tests for every rule, and the service path is tested on SQLite.
- **AC-5**: the audit entry records the format, rows, and that masking was applied.

## Decision

- **Domain**: `unidb-core::mask` owns `mask_value(value)` and `mask_rows(rows)`. Free text becomes `[masked]`, email shaped text becomes `masked@example.test`, and every other non numeric, non boolean type becomes `[masked]` while preserving the column structure. Nulls stay null.
- **Service**: `export_query_masked` runs the same statement, clones and masks the rows, formats through the existing CSV and JSON writers, writes the file, and audits with a `masked` flag.
- **Interface**: the editor Export menu gains masked CSV and masked JSON entries; there is no separate masking page in this slice.
- **Limits**: no per column mask policies, no reversible tokens, no masking of query history. Those are follow ups.

## Build plan

1. Add the mask module and tests, satisfies **AC-2**, **AC-4**
2. Add the masked export service path with audit and a SQLite test, satisfies **AC-3**, **AC-4**, **AC-5**
3. Add the masked export menu entries, satisfies **AC-1**

## Consequences

**Positive**
- Safe exports for support and sharing.
- Masking rules are explicit and tested instead of ad hoc.

**Negative / tradeoffs**
- Masking is coarse; all text becomes the same marker.
- No per column policy yet.

## Follow-up

- [ ] Per column mask policies and partial masking.
- [ ] Reversible tokenization with a local key.
- [ ] Masking query history and clipboard copies.

## References

- Spec `0008`, export path and formatting.
- Spec `0015`, fake data sibling in the workflow area.
