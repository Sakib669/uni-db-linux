#!/usr/bin/env bash
# Verifies the native UI headlessly inside the pinned GTK container.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

docker run --rm \
    -u "$(id -u):$(id -g)" \
    -v "$ROOT":/app \
    -e HOME=/tmp \
    -w /app \
    unidb-gtk-base \
    bash -lc 'xvfb-run -a -s "-screen 0 1200x800x24" /app/scripts/gtk-flow.sh'
