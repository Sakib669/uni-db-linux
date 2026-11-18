# 0023. Offline first sync

**Date**: 2026-11-18
**Status**: Complete

## Summary

Offline first sync lets developers draft and queue SQL statements when a database connection is offline or unreachable. Queued operations are persisted safely in SQLite (`sync_queue`) with connection target, SQL text, optional baseline row count guard for optimistic conflict detection, status (`pending`, `synced`, `conflict`, `failed`), and timestamps. When the connection comes back online, the user can review and flush the queue transactionally.

## Context

Engineers working from trains, flights, or restricted VPN environments still compose schema updates and data fixes while disconnected. Queuing operations locally with explicit conflict checks prevents lost work and eliminates ad hoc scratch files.

## Requirements

**Acceptance criteria**
* AC-1: SQLite migration `0004_scheduled_jobs.sql` / `0005_sync_queue.sql` adds `sync_queue` storing `id`, `connection_id`, `label`, `sql`, `guard_query`, `expected_guard_value`, `status`, `error_message`, `created_at`, `synced_at`.
* AC-2: `Store` exposes `enqueue_sync_item`, `list_sync_items`, `update_sync_status`, and `delete_sync_item`.
* AC-3: `AppService` provides `queue_offline_operation`, `list_sync_queue`, `flush_sync_queue(connection_id)`, and `remove_sync_item`. When flushing, if an item specifies `guard_query` + `expected_guard_value`, `flush_sync_queue` checks the guard first and marks `conflict` instead of applying stale writes when the remote value diverged.
* AC-4: Successful sync runs apply SQL statements transactionally through `apply_statements` and record `offline_sync_flushed` in the security audit log without logging SQL text.
* AC-5: Headless tests verify offline queuing while disconnected, conflict detection when remote state changed, clean transactional sync on reconnect, and audit logging.
* AC-6: The Platform page includes an Offline Sync view showing pending/synced items with Queue, Sync Now, and Remove controls.

## Decision

* **Store migration**: `0005_sync_queue.sql` creates `sync_queue` and an index on `(connection_id, created_at ASC)`.
* **Conflict prevention**: Optional optimistic guard SQL checks live state before replaying queued writes inside a transaction.

## Build plan

1. Add `0005_sync_queue.sql` and `SyncQueueItem` store methods in `unidb-store` with tests.
2. Add `queue_offline_operation`, `list_sync_queue`, `flush_sync_queue`, and `remove_sync_item` to `AppService` in `unidb-app` with integration tests.
3. Wire Offline Sync panel into the Platform section in `unidb-ui`.
4. Verify with workspace tests, clippy, fmt, and container GUI screenshots.
