# 0003. App store bootstrap and security screens

**Date**: 2026-11-17
**Status**: In Progress

## Summary

This feature makes the app's own data real. UniDB now opens the SQLite app store at startup with the keyring held master key, keeps five rolling backups, detects corruption and unclean shutdowns, saves connections with encrypted passwords, and writes an audit trail. It replaces the scaffold probe screen with two working pages: a studio page and a Security page that shows the real state of the store instead of a hardcoded all green list.

## Context

The scaffold has a tested store and a tested bridge, but the interface never uses the store, the Security screen does not exist, and startup always runs without persistence. Spec 0001 requires keyring backed secrets, an audit log, backup and recovery, and a no telemetry security posture. Those promises are only real when the app opens the store on launch, survives a restart with the same secrets, and shows the user the actual security state. This feature is the bridge between the engine layer and every later workflow, because the connection manager, the editor history, and the settings all read through the same store.

## Requirements

**User stories**
- As a user, my saved connections and passwords are still there after I restart the app.
- As a user, I can see why the app failed to start when the keyring is locked or the key is missing, instead of a blank window or plaintext fallback.
- As a user, I can see the real security checks and the recent audit entries on a Security page.
- As a user, the app keeps backups and tells me when the last shutdown was unclean.

**Acceptance criteria**
- **AC-1**: startup opens the store at the XDG data directory with the keyring master key; a missing key, a locked keyring, or a mismatched key yields a diagnostic page with the reason and a retry, and there is no plaintext fallback path.
- **AC-2**: connections persist across a restart with their passwords encrypted, deletion removes them, and save, delete, open, and close actions appear in the audit log.
- **AC-3**: the store runs `PRAGMA integrity_check` on open, keeps five rolling backups under `backups/`, and reports a safe error with restore guidance when the database is corrupt.
- **AC-4**: the app sets an unclean shutdown marker on startup and clears it on a clean exit; the next start reports when the previous shutdown was unclean.
- **AC-5**: the Security page lists real checks: keyring state, key check state, encrypted secret count, audit entry count, read only default, TLS rule for remote hosts, and no telemetry.
- **AC-6**: the Security page lists the ten most recent audit entries, newest first, with time, action, and outcome.
- **AC-7**: the app service bootstrap is testable headless through an injected data directory and key provider.
- **AC-8**: headless tests cover backup rotation, integrity reporting, the shutdown marker, connection CRUD with encrypted passwords, and the security report.
- **AC-9**: the GUI shows the studio page and the security page when the store opens, and a diagnostic page when it does not; the diagnostic page is verified with a container screenshot.

## Decision

- **Data directory**: `$XDG_DATA_HOME/unidb/` or `~/.local/share/unidb/`. The database is `unidb.db`, backups are `backups/unidb-YYYYMMDD-HHMMSS.db`, and migrations run after the integrity check and after the pre migration backup.
- **Startup**: `AppService::bootstrap` opens the store with `KeyringKeyProvider`, marks the run in progress, builds the engine registry with all nine engines, and returns the service. `bootstrap_in` takes the data directory and a key provider so tests never need the desktop keyring.
- **Failure path**: keyring locked, key missing, key mismatch, and corrupt store are distinct errors. The GUI renders a diagnostic page with the safe user message, the error code, and a retry button that re-runs bootstrap.
- **Security report**: computed from live store state, not constants. Each check carries `pass`, `warn`, or `fail`, a plain label, and a detail. The report includes counts and the previous unclean shutdown state.
- **Audit**: the existing `audit_log` table is written by the service for connection save, delete, open, and close. The Security page reads the newest entries.
- **Shutdown**: the interface returns, the service closes its actors, sends shutdown to the runtime, and marks a clean shutdown. A crash leaves the marker, and the next startup reports it.
- **UI shell**: this feature keeps a minimal two page shell, a studio page and a security page, with a stack switcher in the header. The design system feature next replaces the styling and layout, not the data flow.

## Build plan

1. Extend the store with the database path, backups directory, integrity check, pre migration backup, five backup rotation, counts, and the shutdown marker, satisfies **AC-3**, **AC-4**, **AC-5**
2. Add headless store tests for backups, integrity, marker, and counts, satisfies **AC-8**
3. Add the app paths module, the security report model, and `AppService` bootstrap with injected data directory and key provider, satisfies **AC-1**, **AC-5**, **AC-7**
4. Add service connection CRUD, audit writes, report, and clean shutdown, satisfies **AC-2**, **AC-5**, **AC-6**
5. Add headless service tests for bootstrap, persistence, unclean marker, and the report, satisfies **AC-2**, **AC-7**, **AC-8**
6. Rework the interface into a studio page, a security page, and a diagnostic page with retry, satisfies **AC-1**, **AC-5**, **AC-6**, **AC-9**
7. Update the main binary to bootstrap before the window and shut down cleanly, satisfies **AC-1**, **AC-4**
8. Verify the diagnostic page with the GTK container screenshot, satisfies **AC-9**

## Consequences

**Positive**
- The app's data survives restarts and the keyring rules are enforced in one place.
- Security claims are now visible and computed, which makes them reviewable.
- Backups give a recovery path before migrations touch data.

**Negative / tradeoffs**
- A desktop Secret Service becomes a startup requirement; headless and container runs show the diagnostic page until one exists.
- Backups copy the database bytes, so they grow with history until rotation prunes them.
- The two page shell is functional but plain until the design system feature lands.

**Neutral**
- The security report is read only, so it cannot repair anything; repair stays in the store and recovery paths.
- The audit log keeps growing until its retention setting is applied at startup.

## Follow-up

- [ ] Keyring unlock and retry flow inside the app, plus a test container with a session keyring for the success path screenshot.
- [ ] Apply the audit log retention setting at startup.
- [ ] Connection import and export once the connection manager exists.

## References

**Project sources**
- Spec `0001`, security architecture, backup and recovery, keyring and audit rules.
- Spec `0002`, the engine registry that the bootstrap now builds.

**Practices & standards**
- No plaintext secret fallback, ever.
- Integrity check before migration, backup before migration, restore the newest good copy.
- Security status computed from live state instead of hardcoded values.
