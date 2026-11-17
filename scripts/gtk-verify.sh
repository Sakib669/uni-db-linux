#!/usr/bin/env bash
# Verifies the native UI headlessly inside the pinned GTK container. The flow
# script starts its own Xvfb and Openbox.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

docker run --rm \
    -u "$(id -u):$(id -g)" \
    -v "$ROOT":/app \
    -e HOME=/tmp \
    -w /app \
    unidb-gtk-base \
    /app/scripts/gtk-flow.sh
