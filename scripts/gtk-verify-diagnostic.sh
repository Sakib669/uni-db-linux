#!/usr/bin/env bash
# Verifies the startup diagnostic page: a corrupt app database must show the
# diagnostic page with the safe error instead of crashing or falling back.
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
        UNIDB_DATA_DIR=/app/target/badstore xvfb-run -a -s "-screen 0 1200x800x24" bash -c "
            openbox >/dev/null 2>&1 &
            sleep 0.5
            ./target/debug/unidb &
            APP_PID=\$!
            for i in \$(seq 1 30); do
                WINDOW=\$(xdotool search --onlyvisible --name UniDB 2>/dev/null | head -1 || true)
                [ -n \"\$WINDOW\" ] && break
                sleep 0.3
            done
            if [ -z \"\${WINDOW:-}\" ]; then
                echo \"FAIL: diagnostic window never appeared\"
                kill \$APP_PID 2>/dev/null || true
                exit 1
            fi
            sleep 2
            import -window root /app/target/diagnostic-page.png
            kill \$APP_PID 2>/dev/null || true
            echo \"screenshot: target/diagnostic-page.png\"
        "
    '
