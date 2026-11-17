# 0004. Design system and application shell

**Date**: 2026-11-17
**Status**: In Progress

## Summary

This feature replaces the functional two page layout with the real application shell: a libadwaita sidebar navigation window, a shared visual language, standard preference style lists, empty states, and toast feedback. The goal is that every later screen, the connection manager, the editor, the grid, and the settings, plugs into one shell and one set of components instead of inventing its own layout.

## Context

Spec 0003 proved the data flow but kept the interface deliberately plain: buttons in a row, a multiline label for the security report, and a custom error box. That is not an application. Spec 0001 promised a clean, premium native interface, and the GNOME design conventions require standard containers, a clear navigation structure, symbolic icons, live feedback, and accessible controls. Building the connection manager on top of the current layout would multiply one off screens. The shell and the design language must come first so every later feature inherits them.

## Requirements

**User stories**
- As a user, I see a real application window with a sidebar and a header bar, not a row of buttons.
- As a user, I can move between sections with the keyboard and the pointer, and the window title follows the section.
- As a user, the Security page reads like a settings page with grouped rows and clear status, not a wall of text.
- As a user, actions such as a backup report back with a short toast.
- As a user, the interface respects my light or dark system theme and my accent settings.

**Acceptance criteria**
- **AC-1**: the window uses `AdwApplicationWindow`, `AdwToolbarView`, `AdwHeaderBar`, and a sidebar navigation pattern from libadwaita, with a menu button for About and Quit.
- **AC-2**: the sidebar switches between Studio and Security, and the header title and content page title follow the section; pointer and keyboard selection both work.
- **AC-3**: the Security page uses `AdwPreferencesPage` with an `AdwPreferencesGroup` of checks and a group of recent audit rows; each check has a symbolic status icon, a title, and a detail.
- **AC-4**: the Studio page shows a libadwaita empty state with an icon, a title, a description, and the probe action until the connection manager exists.
- **AC-5**: startup failure shows a status page with a warning icon, the safe error, and a retry action instead of the custom error box.
- **AC-6**: a backup action reports success or failure with a toast, and the button shows a busy state while it runs.
- **AC-7**: the brand accent is applied through libadwaita named colors so light, dark, and high contrast themes keep working; no hardcoded window or sidebar surface colors.
- **AC-8**: all interface states are verified with container screenshots: studio, security, and startup failure.
- **AC-9**: the stylesheet is embedded in the binary; the app reads no external asset at runtime.
- **AC-10**: controls are reachable by keyboard, icon only controls have tooltips, and every label and row has a readable accessible name.

## Decision

- **Shell**: `AdwApplicationWindow` containing `AdwToolbarView` with an `AdwHeaderBar`, and an `AdwNavigationSplitView` with a sidebar page and a content page. The sidebar is a list of sections; the content page holds a stack with the section pages.
- **Navigation**: sections are Studio and Security now. Later feature specs add Connections, Queries, and Settings rows to the same sidebar instead of a new window.
- **Design tokens**: teal `#0D9488` is the accent, mapped onto `accent_bg_color`, `accent_color`, and `accent_fg_color` so libadwaita themes it correctly. Indigo `#6366F1` is the secondary accent for selections that are not primary actions. Navy `#0F172A` is reserved for engine identity dots and the brand wordmark, never for window or sidebar surfaces. Spacing and typography use libadwaita defaults and style classes.
- **Components**: status pill as a label with a status class, engine badge as a symbolic circle plus name using the engine color from `unidb-core`, empty state through `AdwStatusPage`, feedback through `AdwToastOverlay`, persistent degraded state through `AdwBanner` when a later feature needs it.
- **Stylesheet**: one embedded `resources/style.css` that defines only the accent named colors, status classes, monospace class, and brand wordmark. No layout styles that libadwaita provides.
- **Verification**: screenshots from the GTK container for the studio empty state, the security page, and the diagnostic page, run through the existing scripts.

## Build plan

1. Add the embedded stylesheet with accent named colors, status classes, monospace, and brand wordmark, satisfies **AC-7**, **AC-9**
2. Rebuild the shell with `AdwApplicationWindow`, `AdwToolbarView`, `AdwHeaderBar`, and `AdwNavigationSplitView`, satisfies **AC-1**, **AC-2**
3. Move the Studio content into an `AdwStatusPage` empty state with the probe action, satisfies **AC-4**, **AC-10**
4. Rebuild the Security page with `AdwPreferencesPage` groups and dynamic rows for checks and audit entries, satisfies **AC-3**, **AC-10**
5. Add the startup failure status page with a retry action, satisfies **AC-5**
6. Add the toast overlay and report backup results as toasts with a busy button state, satisfies **AC-6**
7. Update the verification scripts and capture the three screenshots, satisfies **AC-8**
8. Run the gates and record the screenshots in evidence, satisfies **AC-8**

## Consequences

**Positive**
- Every later screen has one shell, one navigation model, and one set of components.
- The accent comes from libadwaita named colors, so dark and high contrast themes keep working.
- The Security page becomes readable and reviewable instead of a text dump.

**Negative / tradeoffs**
- The split view adds a sidebar that still has only two sections, so it looks sparse until the connection manager lands.
- Dynamic rows are built in Rust rather than declared, which is more code than the text labels they replace.
- The custom brand colors are deliberately limited to the accent, so the app looks closer to GNOME than to the old marketing page.

**Neutral**
- Studio content is a placeholder until the connection manager feature, so the empty state is the honest surface.
- The toast overlay is the standard feedback path from here on.

## Follow-up

- [ ] Add engine badges and connection rows once the connection manager exists.
- [ ] Add a settings section and a keyboard shortcuts dialog in a later feature.
- [ ] Review high contrast and 200 percent text scaling with the accessibility tools available.

## References

**Project sources**
- Spec `0001`, interface toolkit, native Linux, accessibility, and the brand colors.
- Spec `0003`, the pages and the security report that this shell now presents.
- Installed skill `.agents/skills/designing-gnome-ui/`, GNOME HIG container, navigation, control, and feedback patterns.

**Practices & standards**
- Standard libadwaita containers before custom widgets.
- Symbolic icons with tooltips on icon only controls.
- Named color accents instead of hardcoded surfaces, so system themes keep working.
- Empty, loading, and error states are part of the design, not an afterthought.
