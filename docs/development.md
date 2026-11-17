# Development

UniDB is a Rust workspace. The core builds anywhere. The native interface needs GTK 4.14 and libadwaita 1.5 development packages.

## Host prerequisites

Ubuntu 24.04, Linux Mint 22, Fedora 40, Debian 13, or newer:

```bash
sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev libgtksourceview-5-dev
```

## Gates without GTK

These run on any machine and on CI:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo test --workspace --no-default-features
cargo run --no-default-features
```

## GTK build and verification container

When the host lacks GTK development packages, build and verify the native app in the pinned Ubuntu 24.04 image. The image carries the GTK stack; the host Rust toolchain and cargo cache are mounted in.

```bash
docker build -t unidb-gtk-base -f docker/gtk-build.Dockerfile docker
scripts/gtk-cargo.sh build --features gui
```

Run the native window under Xvfb (no host display or GTK packages needed):

```bash
scripts/gtk-verify.sh
```

That script starts Xvfb, launches UniDB, finds the window, clicks the SQLite probe button by keyboard focus, saves `target/unidb-probe.png`, and fails if the window never appeared.

## Repository layout

- `crates/unidb-core`: domain types, value model, error taxonomy, profiles.
- `crates/unidb-store`: SQLite app store, migrations, keyring, AES-256-GCM envelopes.
- `crates/unidb-engine`: driver traits and engine drivers.
- `crates/unidb-app`: UI bridge, connection actors, app services.
- `crates/unidb-ui`: GTK4 and libadwaita interface, built with the `gui` feature.
- `docs/specs`: architecture decisions.
- `docs/scope`: coarse feature scope.
