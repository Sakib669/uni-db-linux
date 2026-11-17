# Rationale for 0001. Native GTK4 desktop architecture

## Context

> ⚠️ Premise note: two concerns are worth stating before the decision. First, "enterprise" usually implies deployment management, central policy, support terms, and audit export, while this product is a single user local desktop app with optional team file sharing; this plan reads enterprise as security, stability, and auditability, not as a server product, and the engineer should say so if a server or SSO is actually required. Second, the README describes roughly two years of product work for a team; one decision at a time is the only honest way to get there, so this spec settles the foundation and the delivery roadmap below sequences the rest.

UniDB is a Linux desktop database studio, not a web service. Its README promises Rust level speed (0.8 second startup, about 68 MB of memory), nine engines, offline use, no telemetry, and a free product with no paywall. Those promises are the hard forces on the stack: a garbage collected runtime or an embedded browser would put both performance numbers out of reach.

What exists today is a skeleton: one Rust binary of 86 lines, nine two line driver stubs that are not even compiled into the binary, an empty `frontend/index.html`, a security audit that hardcodes every field to true, and one commit. So this is effectively a greenfield build that inherits a Rust starting point and a set of product promises.

The engineer's answers shape the rest. The interface must be a dedicated native desktop app with no web based UI. The first release is the core studio plus security, with later phases after it. Usage is single user locally, with optional team sharing through files later. The team is small, the target is Linux only, and the distribution channels are `.deb`, then AppImage, then Flatpak. There is no backend and there should not be one.

Without this decision, every later feature invents its own threading model, its own secret handling, and its own idea of what a driver is. The skeleton stays and the product becomes a pile of inconsistent modules that cannot be tested or shipped. The cost of not deciding is paid again in every spec after this one.

## Options considered

### Option 1: Tauri 2 with a web interface

A Rust core hosts a webview and exposes typed commands; the interface is React or Svelte rendered as HTML and CSS. Verified current versions during the scan: Tauri 2.12.5, React 19.3.1, Vite 8.3.8.

**Pros**:
- The fastest path to a premium visual interface, with mature libraries for code editing and large tables.
- Small binary and memory use compared with Electron, and mature Linux packaging.
- Typed commands keep the surface between Rust and the interface explicit.

**Cons**:
- The interface is still HTML and CSS inside the system webview, which contradicts the engineer's explicit requirement for no web based UI.
- WebKitGTK versions differ across distributions, so rendering and feature support drift per user.
- Adds a second language and a webview attack surface on top of the local database surface.
- Memory use sits above a native toolkit for the same screen.

### Option 2: Native GTK4 with libadwaita and relm4

The interface is built from real GTK4 widgets in Rust, styled with libadwaita, organized with the relm4 component model. No browser engine runs at any point.

**Pros**:
- Genuinely native: real widgets, Wayland and X11, accessibility, and the GTK4 widget set already includes the pieces this app needs (`GtkSourceView` for SQL, `GtkColumnView` for big tables).
- Lowest memory and fastest startup of the realistic options, which is what the product advertises.
- No webview and no local port, so the process has no network listener at all.
- One language across the whole codebase, which matters for a small team.

**Cons**:
- A custom premium look is harder than with CSS, and libadwaita conventions must be followed or the app looks off.
- GTK4 minimum versions vary by distribution, which forces a version floor or library bundling in the AppImage.
- Charts, the ERD, and rich data views must be hand drawn rather than taken from a mature web library.
- Fewer Rust developers know GTK than know React.

### Option 3: Slint 1.19

A Rust native toolkit with its own declarative markup language, rendering without a webview.

**Pros**:
- Small and fast, with a design language closer to modern UI work than raw GTK.
- No webview and a single Rust codebase.
- Verified current release line at 1.19.

**Cons**:
- The widget ecosystem is much younger, and there is no equivalent of a mature source editor or a virtualized table widget, so two of the most important screens would be built from primitives.
- License terms need a careful review before shipping a free distributed app.
- A smaller community means fewer answers when something breaks.

### Option 4: Qt 6 through cxx-qt

Native Qt widgets driven from Rust through the cxx-qt bridge.

**Pros**:
- Very mature widgets, including strong table and text editing components, with long term support.
- Excellent Linux packaging and a long track record in desktop data tools.

**Cons**:
- Rust to C++ interop is an extra layer with its own build and debugging cost.
- LGPL and commercial terms complicate distribution for a free app.
- Larger runtime and dependency footprint than GTK4, and more moving parts than a small team should own.

## Rationale

The engineer explicitly rejected a web based interface, and that requirement is decisive. Tauri would have been the recommendation without it: it has the mature ecosystem and the fastest route to the premium look the README describes. With the requirement in place, choosing Tauri would mean shipping an interface the engineer does not want and paying for a webview on top of the native dependency anyway. That tradeoff is recorded here so it is not silently reversed later.

Among the native toolkits, GTK4 plus libadwaita wins on the forces that matter: native Linux integration, the two widgets this product depends on most, the lowest memory use, and the largest Rust community among the native options. relm4 adds a message driven component model that keeps a large interface maintainable, and its `Sender` works from tokio worker threads, which is exactly the bridge this architecture needs. Slint is promising but its missing table and editor widgets would be the entire cost of the app. Qt is mature but the C++ bridge and the license review are recurring costs a small team should avoid while the product is unproven.

The internal architecture follows from the same forces. One process and no IPC keeps the security boundary simple and removes a local attack surface. A tokio runtime is mandatory because every chosen driver is tokio native, and the glib bridge is the standard way to keep GTK single threaded and responsive. SQLite through sqlx gives the app one backup friendly store and reuses the exact stack already needed for the SQLite engine. The keyring master key with AES-256-GCM means no passphrase prompt at launch while still keeping plaintext out of backups and shared bundles. The driver contract is the only place engines are allowed to differ, so nine engines stay testable and replaceable.

The rest is deliberate boring technology: GTK4, sqlx, tokio, SQLite, rustls, and plotters are all proven and widely documented. The one newer piece, a sandboxed WebAssembly plugin runtime, is pushed to a later phase precisely because it is not settled technology for this use.

An independent cross check on 2026-11-17 confirmed the architecture and found twenty three missing contracts, mostly values a later build would have had to invent: pinned versions, the GTK floor, the async bridge rules, dynamic dispatch, actor supervision, the key lifecycle, the ciphertext envelope, the error shape, timeouts, logging redaction, test tiering, packaging identity, and the update trust anchor. Those resolutions are folded into `index.md` rather than left to the build. The cross check also proposed two simplifications: ship three engines first and defer six, and prove the `.deb` before paying for AppImage. Both are recorded as follow up items instead of being silently adopted, because the engineer chose the full nine engine scope and the AppImage channel for the first release, and each belongs in its own later spec.

## Supporting evidence: landscape scan, 2026-11-17

A read only scout checked current versions before the stack questions. Verified during the conversation:

- Tauri 2 stable; CLI tag 2.12.5; bundles `.deb` and AppImage; Flatpak is community packaging rather than a native target; security model is per window capabilities.
- React 19.3.1, Svelte 5.57.3, Vite 8.3.8 (npm registry at scan time).
- sqlx 0.9.0, pure Rust, PostgreSQL, MySQL, and SQLite, MSRV 1.94.
- tokio-postgres 0.7.18, mysql_async 0.37.1, redis crate with a 2.0 release candidate seen (stable line to confirm at build time), mongodb 3.10.1, scylla 1.10.0, tiberius 0.13.0, rusqlite 0.43.0, libsql 0.10.0.
- russh 0.62.7, pure Rust.
- keyring 3.6.3 seen in a dependency bump; registry version to confirm at build time.
- CodeMirror 6.0.2 and TanStack Table v9 (considered for the rejected web option).
- Tauri bundler plus `tauri-plugin-updater` for packaging and updates.

Marked unverified by the scout and to confirm at build time: exact stable versions of egui, iced, Dioxus, gpui, Vue, Solid, ssh2, rustls, age, aes-gcm, candle, llama.cpp bindings, Monaco stable, the Redis stable line, and the Ollama API stability guarantee.

## References

**Project sources** (verifiable, in this repo):
- `README.md`, the product promises for speed, memory, nine engines, offline use, and no telemetry.
- `Cargo.toml` and `src/main.rs`, the existing Rust starting point and the fake security audit that this decision replaces.
- Installed project skills `.agents/skills/z-rust-gtk4/`, `.agents/skills/relm4-app/`, `.agents/skills/gtk4-libadwaita-internals/`, `.agents/skills/developing-gtk-apps/`, and `.agents/skills/designing-gnome-ui/`, which carry the GTK and relm4 conventions this build should follow.

**Practices & standards**:
- Native toolkit over embedded browser when deep OS integration and low memory are product requirements.
- One process layered monolith with a single binary; extract services only when a real boundary forces it.
- Secrets in the operating system keyring, never in application files; AES-256-GCM with unique nonces and associated data for values at rest.
- Bounded channels for backpressure when streaming large result sets into a virtualized view.
- Capability sandboxing for untrusted extensions (WebAssembly with a narrow host API).
- Measurable performance budgets enforced in continuous integration, and a supply chain policy with license and advisory checks.

**Links** (web verified during the conversation):
- sqlx crate metadata: https://crates.io/api/v1/crates/sqlx
- Rust database crate landscape: https://lib.rs/database
- Tauri CLI 2.12.5 release, for the rejected option: https://github.com/tauri-apps/tauri/releases/tag/tauri-cli-v2.12.5
- Tauri v2 capabilities, for the rejected option: https://v2.tauri.app/security/capabilities/
- React latest, for the rejected option: https://registry.npmjs.org/react/latest
- Vite latest, for the rejected option: https://registry.npmjs.org/vite/latest
- GTK MCP server, selected for the local MCP configuration: https://github.com/alex-oleshkevich/gtk-mcp
- Agent skill registry used for discovery: https://www.skills.sh/
