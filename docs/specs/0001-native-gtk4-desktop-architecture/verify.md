# Verify: Stack and architecture · spec 0001 · updated 2026-11-17

_Steps derived from the scope feature's Done when line. Spec 0001 is a decision spec with no acceptance criteria, so these map to the scaffold outcomes instead. `/check verify` runs these; `/test` locks the durable ones._

## Commands

- [ ] `cargo test --workspace --no-default-features` → 36 tests pass (core 12, store 13, engine 6, app 5, no failures) → store migrations, secret envelope, engine query, bridge, and service
- [ ] `cargo clippy --workspace --all-targets --no-default-features -- -D warnings` → no warnings → gate for merged code
- [ ] `cargo fmt --all -- --check` → no diff → formatting gate
- [ ] `cargo run --no-default-features` → prints the headless build message and exits 0 → thin binary works without GTK
- [ ] `docker build -t unidb-gtk-base -f docker/gtk-build.Dockerfile docker` then the build command in `docs/development.md` → the GTK app compiles in the Ubuntu 24.04 container → GTK floor and native build
- [ ] `cargo run --no-default-features -- --version` → prints `unidb 0.1.0` → binary identity
- [ ] Create a store at a temporary path and reopen it → connections, settings, audit entries, and history survive; the same keyring provider decrypts the saved password → app store persistence
- [ ] Open a store with a different keyring key → fails with `master_key_mismatch`, not silent data loss → key lifecycle
- [ ] Open a store with the master key missing → fails with `master_key_missing` → no plaintext fallback

## UI / manual

- [ ] Install `build-essential pkg-config libgtk-4-dev libadwaita-1-dev libgtksourceview-5-dev`, then `cargo run --features gui` → a window titled UniDB opens on X11 or Wayland → native shell (verified in the Ubuntu 24.04 container under Xvfb; the host still lacks the dev packages)
- [ ] With the window open, confirm the app id is `io.github.sakib669.UniDB` and the window closes cleanly → packaging identity
- [ ] Confirm `pkg-config --modversion gtk4` reports 4.14 or newer and `pkg-config --modversion libadwaita-1` reports 1.5 or newer → GTK floor

## Remaining work before this feature can close

- [ ] Render the native window and wire the window's actions to `unidb_app::AppService` through the bridge (the bridge and service are tested headless; the GTK side is not wired yet)
- [ ] Run the GTK CI job on GitHub, which installs the packages and builds `--features gui`

## Coverage

Decision spec 0001 has no acceptance criteria. Coverage maps to the scope Done when: workspace builds (commands), app store migrates and survives restart (commands), one engine connects and runs a query (commands), the interface to worker pipe is tested (commands, headless), CI gates exist (workflow file), fake audit and stub drivers are gone (committed removal).
