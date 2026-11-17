#!/usr/bin/env bash
# Verifies the native window headlessly: starts Xvfb and Openbox, launches
# UniDB, clicks the Studio and Security buttons, and saves a screenshot of each.
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
                echo \"FAIL: UniDB window never appeared\"
                kill \$APP_PID 2>/dev/null || true
                exit 1
            fi
            eval \$(xdotool getwindowgeometry --shell \$WINDOW)
            echo \"window: \$WINDOW at \$X,\$Y size \${WIDTH}x\${HEIGHT}\"
            sleep 2
            xdotool mousemove \$((X + 52)) \$((Y + 41))
            sleep 0.3
            xdotool click 1
            sleep 1
            import -window root /app/target/studio-page.png
            xdotool mousemove \$((X + 167)) \$((Y + 41))
            sleep 0.3
            xdotool click 1
            sleep 1
            import -window root /app/target/security-page.png
            kill \$APP_PID 2>/dev/null || true
            echo \"screenshots: target/studio-page.png target/security-page.png\"
        "
    '
