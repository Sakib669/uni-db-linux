#!/usr/bin/env bash
# Verifies the native window headlessly: starts Xvfb, launches UniDB, finds the
# window, activates it, presses Tab to focus the probe button, presses Return,
# waits for the worker result, and saves a screenshot.
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
        xvfb-run -a -s "-screen 0 1200x800x24" bash -c "
            ./target/debug/unidb &
            APP_PID=\$!
            for i in \$(seq 1 30); do
                WINDOW=\$(xdotool search --name UniDB 2>/dev/null | head -1 || true)
                [ -n \"\$WINDOW\" ] && break
                sleep 0.3
            done
            if [ -z \"\${WINDOW:-}\" ]; then
                echo \"FAIL: UniDB window never appeared\"
                kill \$APP_PID 2>/dev/null || true
                exit 1
            fi
            echo \"window: \$WINDOW\"
            xdotool windowactivate --sync \$WINDOW
            xdotool key --window \$WINDOW Tab
            sleep 0.5
            xdotool key --window \$WINDOW Return
            sleep 2
            import -window root /app/target/unidb-probe.png
            kill \$APP_PID 2>/dev/null || true
            echo \"screenshot: target/unidb-probe.png\"
        "
    '
