#!/usr/bin/env bash
# Verifies the startup diagnostic page: a corrupt app database must show the
# diagnostic page instead of crashing or falling back.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

docker run --rm \
    -u "$(id -u):$(id -g)" \
    -v "$ROOT":/app \
    -e HOME=/tmp \
    -w /app \
    unidb-gtk-base \
    bash -lc '
        set -e
        mkdir -p /app/target/badstore
        printf "not a database" > /app/target/badstore/unidb.db
        XAUTH=/tmp/unidb-xauth
        export DISPLAY=:99 XAUTHORITY="$XAUTH"
        touch "$XAUTH"
        xauth add :99 . "$(mcookie)" >/dev/null 2>&1 || true
        Xvfb :99 -screen 0 1200x800x24 -nolisten tcp -auth "$XAUTH" >/tmp/xvfb.log 2>&1 &
        XVFB_PID=$!
        for _ in $(seq 1 50); do xdotool getdisplaygeometry >/dev/null 2>&1 && break; sleep 0.2; done
        openbox >/dev/null 2>&1 &
        sleep 1
        UNIDB_DATA_DIR=/app/target/badstore ./target/debug/unidb &
        APP_PID=$!
        for _ in $(seq 1 40); do
            WINDOW=$(xdotool search --onlyvisible --name UniDB 2>/dev/null | head -1 || true)
            [ -n "$WINDOW" ] && break
            sleep 0.3
        done
        if [ -z "${WINDOW:-}" ]; then
            echo "FAIL: diagnostic window never appeared"
            kill "$APP_PID" "$XVFB_PID" 2>/dev/null || true
            exit 1
        fi
        sleep 2
        import -window root /app/target/diagnostic-page.png
        kill "$APP_PID" "$XVFB_PID" 2>/dev/null || true
        echo "screenshot: target/diagnostic-page.png"
    '
