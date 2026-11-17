# 0009. History, saved queries, and settings

**Date**: 2026-11-17
**Status**: In Progress

## Summary

This feature turns the records UniDB already keeps into working surfaces. A History section lists executed statements newest first, searches them through the existing FTS5 index, loads a statement back into the editor, and clears the log. A Saved queries group names and stores useful statements, loads them into the editor, and deletes them. A Settings section persists the color scheme preference and applies it immediately through the libadwaita style manager. History, saved query CRUD, and settings all go through the app store, so they survive restarts.

## Context

The app store already has the `query_history` table with FTS5 and the `settings` key value table, and every executed statement already lands there. What is missing is the saved query table, the store and service methods, and the interface that makes the data useful. The editor currently forgets every statement once the user clears it, and there is no preference surface at all. Spec 0001 places these surfaces in the shell and requires that persistence and search stay in the store layer, not in widgets.

## Requirements

**User stories**
- As a user, I open History and see my recent statements with time, outcome, and row count.
- As a user, I search history by words and load a statement back into the editor.
- As a user, I save the current editor statement under a name and load it later.
- As a user, I clear the history and delete saved queries.
- As a user, I choose a light, dark, or system color scheme and it persists.

**Acceptance criteria**
- **AC-1**: a History section lists the newest statements with a one line preview, time, status, row count, and duration, and a search entry filters through FTS5.
- **AC-2**: activating a history row or a saved query loads its SQL into the editor and switches to the Editor section.
- **AC-3**: the user can save the current editor SQL under a name, list saved queries with their creation time, and delete one, all persisted in the store.
- **AC-4**: clearing history deletes all history rows after confirmation and refreshes the list.
- **AC-5**: saved queries and history survive an app restart because they live in the app store.
- **AC-6**: a Settings section offers System, Light, and Dark color schemes, applies the choice at once through the libadwaita style manager, and restores it at startup.
- **AC-7**: the settings value round trips through the store and is exposed through the service, not read from widgets.
- **AC-8**: headless tests cover saved query save, list, delete, history search and clear, and the settings round trip.
- **AC-9**: the History and Settings surfaces are verified with a container screenshot, including a loaded statement and the applied theme choice.

## Decision

- **Schema**: a migration `0002_saved_queries.sql` adds `saved_queries` with `id` (UUID text), `name`, `sql`, optional `connection_id`, optional `folder`, `tags` JSON text, and `created_at` and `updated_at` RFC 3339 timestamps, plus an index on `name`. New installs and existing installs both move forward through the migration runner the app already uses, and the pre-migration backup rule stays in force.
- **Store**: `SavedQuery` model with serde, `save_saved_query` (insert new UUID, return the row), `list_saved_queries` (newest first), `delete_saved_query`, and `clear_history`. History listing reuses `search_history`, so FTS5 matching is not duplicated.
- **Service**: thin wrappers `search_history`, `list_saved_queries`, `save_saved_query`, `delete_saved_query`, `clear_history`, `get_setting`, and `set_setting` so the interface never holds the store. Writes go through the store transaction rules like every other record.
- **Interface**: the sidebar keeps one row per surface: History and Settings join the existing sections, and the Studio placeholder is removed because the editor replaces it. History uses an `AdwPreferencesGroup` with a search entry and rows; activating a row loads the editor through the same method the editor page uses. Saved queries use a name entry plus a Save button and a list with load and delete actions.
- **Settings**: the only setting in this version is `color_scheme` with values `system`, `light`, and `dark`. It maps to `adw::ColorScheme` and `adw::StyleManager`, and is applied from a bootstrap message after the store opens, with a safe fallback to System. Additional settings, such as history retention and editor preferences, reuse the same table and service methods.
- **Destructive actions**: clearing history asks for confirmation through an `AdwAlertDialog` and is one store statement. Deleting a saved query is immediate with a toast, matching the connection delete rule.
- **Limits**: history retention pruning stays callable but is not exposed yet; folders and tags are stored but not shown. Those become follow ups.

## Build plan

1. Add the saved queries migration, model, and store methods, satisfies **AC-3**, **AC-5**, **AC-8**
2. Add history clear and service wrappers for history, saved queries, and settings, satisfies **AC-7**, **AC-8**
3. Build the History section with search, rows, load into editor, clear with confirmation, satisfies **AC-1**, **AC-2**, **AC-4**
4. Build the saved queries group with save, load, and delete, satisfies **AC-2**, **AC-3**
5. Build the Settings section with the color scheme control, startup restore, and immediate apply, satisfies **AC-6**, **AC-7**
6. Remove the Studio placeholder and keep the probe out of the interface, satisfies navigation coherence
7. Add headless store and service tests for saved queries, history, and settings, satisfies **AC-8**
8. Verify the History and Settings surfaces with a container screenshot, satisfies **AC-9**

## Consequences

**Positive**
- The history the app already records becomes usable, and repeat work gets faster.
- Saved queries give a place for reusable statements before migration and snippet features arrive.
- A real settings table exists for later preferences without another schema decision.

**Negative / tradeoffs**
- Saved queries only store name and SQL in this version; folders and tags are schema ready but invisible.
- Clearing history is destructive with only a confirmation dialog.
- The color scheme applies to the whole application, so a future per editor theme needs a richer settings model.

**Neutral**
- The Studio placeholder and probe leave the interface; the probe stays in the service for tests.
- History rows render a preview and load on activation instead of embedding a read only editor.

## Follow-up

- [ ] Folders and tags for saved queries with drag and drop.
- [ ] History retention setting and prune schedule.
- [ ] Snippet expansion and favorite queries in the command palette.
- [ ] Editor font size and tab width settings.

## References

**Project sources**
- Spec `0001`, shell surfaces, store persistence, and settings scope.
- Spec `0003`, store, migrations, audit, and credentials.
- Spec `0006`, execution path that writes history and the editor state this feature reloads.
- Spec `0008`, transfer action patterns reused for the report rows and destructive action confirmation.
- Installed skill `.agents/skills/designing-gnome-ui/`, preferences pages and dialogs.

**Practices & standards**
- Persistence and search in the store, never in widgets.
- Migration first, model and store methods next, interface last.
- Destructive actions confirmed or reversible through a toast.
