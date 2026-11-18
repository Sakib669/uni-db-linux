#!/usr/bin/env bash
# Runs cargo inside the pinned Ubuntu 24.04 GTK container, with the host Rust
# toolchain and cargo cache mounted in. Usage:
#   scripts/gtk-cargo.sh build --features gui
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

CARGO_DIR="$HOME/.cargo"
if [ ! -f "$CARGO_DIR/bin/cargo" ] && [ -f "/home/sakib/.cargo/bin/cargo" ]; then
    CARGO_DIR="/home/sakib/.cargo"
fi
RUSTUP_DIR="$HOME/.rustup"
if [ ! -d "$RUSTUP_DIR/toolchains" ] && [ -d "/home/sakib/.rustup/toolchains" ]; then
    RUSTUP_DIR="/home/sakib/.rustup"
fi

exec docker run --rm \
    -u "$(id -u):$(id -g)" \
    -v "$ROOT":/app \
    -v "$CARGO_DIR":/home/ubuntu/.cargo \
    -v "$RUSTUP_DIR":/home/ubuntu/.rustup \
    -v "$ROOT/.cargo-home":/cargo-home \
    -e CARGO_HOME=/cargo-home \
    -e RUSTUP_HOME=/home/ubuntu/.rustup \
    -e HOME=/tmp \
    -e PATH=/home/ubuntu/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
    -w /app \
    unidb-gtk-base \
    /home/ubuntu/.cargo/bin/cargo "$@"
