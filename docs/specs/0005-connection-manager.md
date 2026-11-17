# 0005. Connection manager and database navigator

**Date**: 2026-11-17
**Status**: In Progress

## Summary

This feature turns saved connection profiles into a real workflow. Users add a connection by filling fields or pasting a URL, test it, save it with an encrypted password, connect and disconnect, and see their saved connections in the sidebar. The database navigator, which browses databases and tables with lazy loading, is the second half of the feature and follows immediately after the profile workflow.

## Context

The store and the engine layers both exist, but the interface has no way to create a connection, so the only query path is a hardcoded SQLite probe. Every later feature, the editor, the table designer, the import wizard, and the navigator, needs a live connection selected from a real list. Without this feature those screens cannot be built honestly. The product promise is also explicit about dual connection modes, fill details or use a URL, and about cloud databases being the same engine with a different host, so the profile model must capture that.

## Requirements

**User stories**
- As a user, I add a connection by filling in name, engine, host, port, database, user, password, TLS, and read only.
- As a user, I paste a connection URL and UniDB fills the profile.
- As a user, I test a connection before saving it.
- As a user, I connect, see that the connection is live, and disconnect.
- As a user, I edit or delete a saved connection, and the password stays encrypted.

**Acceptance criteria**
- **AC-1**: the sidebar has a Connections section listing saved connections with engine, name, host, database, and a local or cloud tag, ordered by name.
- **AC-2**: the new connection dialog captures name, engine, host, port, database, username, password, TLS mode, and read only, validates them with the core rules, and saves the profile.
- **AC-3**: a URL can be pasted instead of fields, and it fills engine, host, port, database, username, TLS mode, and read only where the scheme supports it.
- **AC-4**: a test action reports success or a safe failure without saving the profile.
- **AC-5**: connect starts the connection actor and marks the row live; disconnect stops it; the status survives page switches.
- **AC-6**: editing keeps the existing password when the password field is left empty; deleting removes the profile and writes an audit entry.
- **AC-7**: every network action runs on the worker runtime and the interface never blocks.
- **AC-8**: startup restores the saved list from the store and the list survives a restart.
- **AC-9**: headless tests cover URL parsing, profile validation, save and edit and delete, and test-connection error mapping.
- **AC-10**: the Connections page and the new connection dialog are verified with a container screenshot.

**Navigator phase** (build plan items 9 and 10):
- **AC-11**: after connect, the navigator shows the databases for the engine and lazily loads tables for the selected database.
- **AC-12**: navigator loads are engine specific through one schema contract and failures appear as rows with a safe message, not a crashed page.

## Decision

- **URL parsing** lives in `unidb-core` as `ConnectionProfile::from_url`, using the `url` crate. Schemes: `postgresql` and `postgres` for PostgreSQL, `cockroachdb` for CockroachDB, `mysql` for MySQL, `mariadb` for MariaDB, `redis`, `mongodb`, `mssql` and `sqlserver` for MSSQL, and `sqlite` for a file path. Query parameters `sslmode` map onto `SslMode`; a missing TLS mode defaults through the existing local or cloud rule. A URL without a username or database is valid.
- **Dialog**: `AdwDialog` with an `AdwPreferencesPage`, a mode switch between Details and URL, entry rows, engine and TLS combo rows, a read only switch, and Test, Cancel, Save actions. Save validates first and shows a toast on failure.
- **List**: `AdwPreferencesGroup` with `AdwActionRow` per connection, a symbolic server icon, the engine color as a small badge dot, and Connect or Disconnect, Edit, and Delete in a row menu.
- **State**: the list is loaded from the store on startup and after every mutation. Live connection state is a set of profile ids held by the page model, so it survives section switches while the service keeps the actors.
- **Passwords**: the dialog never reads an existing password. An empty password on edit keeps the stored one. The store encryption path is unchanged.
- **Navigator**: `SchemaSource` trait implementations per SQL engine (SQLite, PostgreSQL, MySQL, MariaDB, CockroachDB) plus MongoDB collections and Redis key scan. The UI uses a two level list: databases, then tables with lazy load on selection. This phase lands in the next build steps.

## Build plan

1. Add URL parsing to `unidb-core` with tests for every scheme and for TLS parameters, satisfies **AC-3**
2. Add `test_connection` to the app service over the engine registry, satisfies **AC-4**, **AC-9**
3. Build the Connections section and the saved list from the store, satisfies **AC-1**, **AC-8**
4. Build the connection dialog with details and URL modes, validation, Test, and Save, satisfies **AC-2**, **AC-3**, **AC-4**, **AC-7**
5. Add connect and disconnect actions with live row state and audit entries, satisfies **AC-5**, **AC-6**
6. Add edit and delete with password retention and safe failures, satisfies **AC-6**
7. Add headless tests for URL parsing and service connection actions, satisfies **AC-9**
8. Verify the page and dialog with container screenshots, satisfies **AC-10**
9. Add `SchemaSource` implementations for the SQL family and the key value and document engines, satisfies **AC-11**, **AC-12**
10. Add the lazy navigator tree and failure rows, satisfies **AC-11**, **AC-12**

## Consequences

**Positive**
- Saved connections become the entry point for every later feature.
- The URL parser keeps the documented dual connection mode honest.
- Live connection state is visible and auditable.

**Negative / tradeoffs**
- The dialog is the most complex piece of UI code so far and needs careful error handling.
- URL parsing cannot express every engine option, so unusual profiles fall back to details mode.
- The navigator phase grows the engine crates again with schema queries per engine.

**Neutral**
- The Studio probe remains until the query editor replaces it.
- Connection state lives in the interface model, not the store, because it is runtime only.

## Follow-up

- [ ] SSH tunnel profiles and key selection in the dialog.
- [ ] Cloud provider presets once the URL parser is proven.
- [ ] Password rotation and key rotation policy for saved secrets.

## References

**Project sources**
- Spec `0001`, connection profiles, TLS rules, read only enforcement, keyring secrets.
- Spec `0002`, the engine registry and connector contract.
- Spec `0003`, store persistence and audit entries.
- Spec `0004`, the shell, sidebar, and preference rows this feature plugs into.
- Installed skill `.agents/skills/designing-gnome-ui/`, dialog, control, and feedback patterns.

**Practices & standards**
- Standard `AdwDialog` and `AdwPreferencesGroup` instead of custom forms.
- URL parsing in the domain layer with a documented scheme table.
- No password read back into the interface; empty means unchanged.
