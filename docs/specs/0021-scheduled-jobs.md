# 0021. Scheduled jobs

**Date**: 2026-11-18
**Status**: Complete

## Summary

Scheduled jobs allow users to register recurring SQL tasks per connection (such as periodic cleanup, rollup counts, or health checks) stored persistently in the SQLite app store. Each job tracks its cadence (`interval_seconds`), enabled state, last run timestamp, next scheduled run timestamp, last outcome, and row count, and can be executed on demand or via the service tick runner (`run_due_jobs`).

## Context

Database teams routinely need lightweight scheduled queries without configuring external cron daemons or database server extensions like `pg_cron`. Storing job schedules inside UniDB's encrypted store keeps credentials safe and works identically across all engines.

## Requirements

**Acceptance criteria**
* AC-1: SQLite migration `0004_scheduled_jobs.sql` adds `scheduled_jobs` with `id`, `name`, `connection_id`, `sql`, `interval_seconds`, `enabled`, `last_run_at`, `next_run_at`, `last_status`, `last_rows`, `created_at`.
* AC-2: `Store` supports `save_job`, `list_jobs`, `get_job`, `set_job_enabled`, `update_job_run`, and `delete_job`.
* AC-3: `AppService` provides `create_job`, `list_jobs`, `toggle_job`, `delete_job`, `run_job_now`, and `run_due_jobs`, executing statements on open connections, updating timestamps and outcomes, and auditing `job_executed` without logging SQL payload contents.
* AC-4: Headless store and service tests verify creation, enabling/disabling, manual execution, due job sweeping, failure tracking, and deletion.
* AC-5: The Platform page includes a Scheduled Jobs view to create, trigger, toggle, and delete jobs.

## Decision

* **Store migration**: `0004_scheduled_jobs.sql`.
* **Service logic**: `run_job_now` executes the job's SQL on `connection_id`, updates `last_run_at = now`, `next_run_at = now + interval_seconds`, `last_status = "success"` or error code, and `last_rows`. `run_due_jobs` scans enabled jobs whose `next_run_at <= now` and runs them if their connection is open.

## Build plan

1. Add `0004_scheduled_jobs.sql` and `ScheduledJob` CRUD methods to `unidb-store` with tests.
2. Add job management and execution methods to `AppService` in `unidb-app` with integration tests.
3. Wire Scheduled Jobs panel into the Platform section in `unidb-ui`.
4. Verify with workspace tests, clippy, fmt, and container GUI screenshots.
