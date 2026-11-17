#!/usr/bin/env bash
# Runs cargo inside the pinned Ubuntu 24.04 GTK container, with the host Rust
# toolchain and cargo cache mounted in. Usage:
#   scripts/gtk-cargo.sh build --features gui
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

exec docker run --rm \
    -u "$(id -u):$(id -g)" \
    -v "$ROOT":/app \
    -v "$HOME/.cargo":/home/ubuntu/.cargo \
    -v "$HOME/.rustup":/home/ubuntu/.rustup \
    -v "$ROOT/.cargo-home":/cargo-home \
    -e CARGO_HOME=/cargo-home \
    -e RUSTUP_HOME=/home/ubuntu/.rustup \
    -e HOME=/tmp \
    -e PATH=/home/ubuntu/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
    -w /app \
    unidb-gtk-base \
    cargo "$@"
