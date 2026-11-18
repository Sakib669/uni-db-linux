# 0017. Migrations and git integration

**Date**: 2026-11-18
**Status**: In Progress

## Summary

This slice adds a migration runner. Numbered SQL files in a `migrations/` directory (for example `0001_create_users.sql`) are discovered, split into statements, applied in order inside a transaction per file, and recorded in the app store with a checksum so each migration runs once per connection. The directory is plain files, so it works with git like any other source. The runner is a service path with tests; the operator surface is a follow up.

## Context

The table designer covers one table at a time and the editor covers ad hoc SQL, but teams need repeatable schema changes tracked in version control. The store already records app data and the connection actors apply transactionally, so a migration ledger plus a safe SQL splitter is the missing piece. Keeping the files plain and numbered means git diff and review work without a bespoke format.

## Requirements

**Acceptance criteria**
- **AC-1**: a migrations directory is read in filename order; files must match `<number>_<name>.sql`.
- **AC-2**: SQL files are split into statements on semicolons outside quotes and comments, with tests for strings, comments, and trailing whitespace.
- **AC-3**: applied migrations are recorded per connection in the app store with name, checksum, and timestamp; re-running the same directory applies nothing.
- **AC-4**: each migration file is applied inside one transaction; a failure rolls back that file and stops the run, reporting the file name and safe error.
- **AC-5**: the report lists applied files and skipped files, and an audit entry records the migration names.
- **AC-6**: checksum mismatches for already applied files are reported so edited history is visible.
- **AC-7**: headless tests cover the splitter, the runner on SQLite, idempotent re-runs, and a failing migration rollback.

## Decision

- **Files**: `migrations/0001_create_users.sql`, ordered by numeric prefix, plain SQL, one or more statements separated by semicolons. Loading rejects names that do not match the pattern.
- **Splitter**: `unidb-core::sql_split::split_statements` walks the text, ignoring semicolons inside single quoted strings, double quoted identifiers, line comments, and block comments, and trims unknown trailing fragments.
- **Ledger**: new app store table `applied_migrations(connection_id, name, checksum, applied_at)` with a unique pair. Store methods list and record; checksums are FNV-1a hex over file contents.
- **Runner**: `AppService::apply_migrations(connection_id, dir)` loads files, reads applied names, reports checksum mismatches as warnings, and for each pending file splits and applies the statements through `apply_statements` (one transaction), then records the ledger row and audits. A failure returns the file name and error and rolls back that file.
- **Operator surface**: this slice exposes the service and report; a Migrations page and git status integration are follow ups. The files live in the user's project directory, so committing them to git is the integration in this version.

## Build plan

1. Add the SQL splitter with tests, satisfies **AC-2**
2. Add the migration ledger table and store methods with tests, satisfies **AC-3**
3. Add migration loading, planning, and the apply runner plus SQLite tests, satisfies **AC-1**, **AC-4**, **AC-5**, **AC-6**, **AC-7**
4. Record the operator surface and git status as follow ups.

## Consequences

**Positive**
- Schema changes become repeatable and reviewable in git.
- The ledger prevents double application.
- The splitter is reusable for script execution elsewhere.

**Negative / tradeoffs**
- No rollback files; a failed file rolls back itself only.
- No Migrations page yet.
- Checksums warn about edited history but do not repair it.

## Follow-up

- [x] Migrations page with plan, apply, and file preview.
- [ ] Git status and ordering checks in the UI.
- [ ] Down migrations and repair mode.
- [ ] Locking so two app instances cannot apply at once.

## References

- Spec `0001`, workflow features scope.
- Spec `0003`, app store and audit.
- Spec `0007`, transactional apply reused here.
