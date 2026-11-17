#!/usr/bin/env bash
# Runs inside the GTK container under xvfb-run: launches the app, captures the
# connections list, saves a SQLite connection through the form, and captures the
# security page.
set -euo pipefail

openbox >/dev/null 2>&1 &
sleep 1
./target/debug/unidb &
APP_PID=$!
trap 'kill "$APP_PID" 2>/dev/null || true' EXIT

for _ in $(seq 1 40); do
    WINDOW=$(xdotool search --onlyvisible --name UniDB 2>/dev/null | head -1 || true)
    [ -n "$WINDOW" ] && break
    sleep 0.3
done
if [ -z "${WINDOW:-}" ]; then
    echo "FAIL: UniDB window never appeared"
    exit 1
fi
eval "$(xdotool getwindowgeometry --shell "$WINDOW")"
echo "window: $WINDOW at $X,$Y size ${WIDTH}x${HEIGHT}"
sleep 2
import -window root /app/target/connections-page.png

xdotool mousemove $((X + 360)) $((Y + 80))
sleep 0.3
xdotool click 1
sleep 1
import -window root /app/target/connection-form.png

xdotool type --delay 30 "Demo SQLite"
sleep 0.3
xdotool key Tab
sleep 0.3
xdotool key space
sleep 0.6
xdotool key End
sleep 0.2
xdotool key Return
sleep 0.4
xdotool mousemove $((X + 580)) $((Y + 361))
sleep 0.2
xdotool click 1
sleep 0.4
xdotool type --delay 30 "/tmp/unidb-demo.db"
sleep 0.3
xdotool mousemove $((X + 1063)) $((Y + 728))
sleep 0.2
xdotool click 1
sleep 2
import -window root /app/target/connections-saved.png

xdotool mousemove $((X + 100)) $((Y + 154))
sleep 0.3
xdotool click 1
sleep 1
import -window root /app/target/security-page.png
echo "screenshots written"
