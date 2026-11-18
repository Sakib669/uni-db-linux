# 0019. Multi database query, team bundles, and git status

**Date**: 2026-11-18
**Status**: Complete

## Summary

This slice completes the workflow feature set with three connected capabilities:
1. **Multi database query**: Run a SQL statement across multiple open connections concurrently and view a merged result set prefixed with a `_connection` identifier column alongside per target timing and errors.
2. **Team bundles**: Export and import shareable plain JSON team bundles containing sanitized connection definitions and saved queries, paired with a SHA-256 signature verified and shown before import. Secrets, passwords, SSH paths, and AI keys are never included.
3. **Git status surface**: Inspect the git repository status (branch name and dirty file count) of the migrations directory during migration planning and display it directly on the Migrations page.

## Context

Engineering teams frequently run the same diagnostic or schema verification query across multiple databases (such as staging, read replicas, or tenant SQLite shards), share workspace connection definitions and curated SQL snippets without exposing passwords, and check whether migration SQL files in a workspace have uncommitted git changes before applying them.

## Requirements

**Acceptance criteria**
* AC-1: `merge_multi_results` in `unidb-core` merges `QueryResult` outputs from multiple named targets, prepending a `_connection` column and aligning heterogeneous column sets safely up to 500 rows.
* AC-2: `AppService::execute_multi` executes a statement across a slice of open connection IDs concurrently, recording an audit event without logging SQL payloads or secrets.
* AC-3: The Editor view provides a "Run on all" action in the results status bar that runs the editor SQL across all open live connections and displays merged results and per target timings.
* AC-4: `TeamBundle` in `unidb-core` serializes version, bundle name, author, timestamp, sanitized connections (no passwords, SSH paths, or AI keys), saved queries, and a deterministic SHA-256 payload signature.
* AC-5: `AppService::export_team_bundle`, `inspect_team_bundle`, and `import_team_bundle` write both `.json` and detached `.sig` files, verify signatures on inspect before import, prevent secret leakage, and audit bundle export and import.
* AC-6: The Connections page allows exporting a team bundle and importing a bundle with signature verification feedback shown before acceptance.
* AC-7: `git_status` in `unidb-app` detects git branch and uncommitted change counts for a migrations folder and surfaces it on the Migrations page status line.
* AC-8: Headless unit and integration tests cover multi database execution, bundle signing and tamper detection, and git status detection. Container GUI verification captures the updated Editor, Connections, and Migrations surfaces.

## Decision

* **Core models**:
  * Add `crates/unidb-core/src/bundle.rs` with `BundleConnection`, `BundleQuery`, `TeamBundle`, `BundleVerification`, canonical SHA-256 signing/verification, and strict validation ensuring no secret fields are serialized.
  * Add `MultiTargetResult` and `merge_multi_results` to `crates/unidb-core/src/visual.rs` (or a dedicated `multi.rs` module) to align columns across heterogeneous results and cap at 500 rows.
* **Service layer**:
  * `AppService::execute_multi(&self, connection_ids: &[Uuid], sql: &str)` runs statements across live connections and returns `(Vec<MultiTargetSummary>, QueryResult)`.
  * `AppService::export_team_bundle(&self, name: &str, author: &str, path: &Path)` builds, signs, and writes the bundle JSON plus detached `.sig` file.
  * `AppService::inspect_team_bundle(&self, path: &Path)` parses and validates the signature, returning `BundlePreview` without mutating the store.
  * `AppService::import_team_bundle(&self, path: &Path)` verifies and imports non duplicate connections (without passwords) and saved queries into the local `Store`.
  * `git_dir_status(dir: &Path) -> GitStatus` in `unidb-app/src/migrations.rs` runs `git -C` status checks safely and returns branch and dirty count.
* **Interface**:
  * Editor status bar gains a "Run on all" button on the right edge (leaving existing toolbar coordinates unchanged).
  * Connections toolbar gains "Export bundle" and "Import bundle" buttons on the right of "New connection", with a bundle preview/verification dialog.
  * Migrations status line appends git branch/cleanliness info returned by `plan_migrations_with_git`.

## Build plan

1. Add `bundle.rs` and `merge_multi_results` to `unidb-core` with comprehensive unit tests (AC-1, AC-4, AC-8).
2. Add `execute_multi`, team bundle service methods, and `git_dir_status` to `unidb-app` with integration tests (AC-2, AC-5, AC-7, AC-8).
3. Wire "Run on all", Team bundle export/import, and Migrations git status in `unidb-ui` (AC-3, AC-6, AC-7).
4. Verify with workspace tests, clippy, fmt, and container GUI verification screenshots (AC-8).

## Consequences

**Positive**
* Complete cross connection query federation without external coordinator services.
* Safe team sharing of connection profiles and SQL libraries with verifiable signatures and zero credential exposure.
* Immediate awareness of uncommitted git state in migration directories.

**Negative / tradeoffs**
* Cross connection merges align columns by name rather than executing distributed SQL joins.
* Team bundle signatures use SHA-256 content hashes and detached `.sig` integrity manifests rather than GPG web of trust keyservers.
