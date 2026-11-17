#!/usr/bin/env bash
# Runs inside the GTK container: starts Xvfb directly (the xvfb-run wrapper has
# been unreliable in this image), launches the app, saves a SQLite connection
# through the form, connects, and captures the navigator and security pages.
set -euo pipefail

XAUTH=/tmp/unidb-xauth
export DISPLAY=:99
export XAUTHORITY="$XAUTH"
touch "$XAUTH"
xauth add :99 . "$(mcookie)" >/dev/null 2>&1 || true
Xvfb :99 -screen 0 1200x800x24 -nolisten tcp -auth "$XAUTH" >/tmp/xvfb.log 2>&1 &
XVFB_PID=$!
cleanup() {
    kill "$APP_PID" 2>/dev/null || true
    kill "$XVFB_PID" 2>/dev/null || true
}
trap cleanup EXIT

for _ in $(seq 1 50); do
    xdotool getdisplaygeometry >/dev/null 2>&1 && break
    sleep 0.2
done
if ! xdotool getdisplaygeometry >/dev/null 2>&1; then
    echo "FAIL: Xvfb never became ready"
    exit 1
fi

openbox >/dev/null 2>&1 &
sleep 1
# Seed the demo SQLite file so the navigator proves table listing.
rm -f /tmp/unidb-demo.db
sqlite3 /tmp/unidb-demo.db "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT); INSERT INTO users (name) VALUES ('Ada');"
printf 'name\nAlan Turing\nGrace Hopper\n' > /tmp/unidb-import.csv
./target/debug/unidb &
APP_PID=$!

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

# Connect, then open the navigator.
xdotool mousemove $((X + 872)) $((Y + 207))
sleep 0.3
xdotool click 1
sleep 2
xdotool mousemove $((X + 100)) $((Y + 114))
sleep 0.3
xdotool click 1
sleep 2
import -window root /app/target/navigator-page.png

# Editor page is the third sidebar row; run a query against the seeded table.
xdotool mousemove $((X + 100)) $((Y + 154))
sleep 0.3
xdotool click 1
sleep 1
xdotool mousemove $((X + 700)) $((Y + 240))
sleep 0.3
xdotool click 1
sleep 0.3
xdotool type --delay 20 "SELECT * FROM users;"
sleep 0.3
xdotool key ctrl+Return
sleep 2
import -window root /app/target/editor-page.png

# Designer page is the fourth sidebar row.
xdotool mousemove $((X + 100)) $((Y + 194))
sleep 0.3
xdotool click 1
sleep 2
import -window root /app/target/designer-page.png

# Type a table name, preview the DDL, and apply it.
xdotool mousemove $((X + 600)) $((Y + 124))
sleep 0.3
xdotool click 1
sleep 0.3
xdotool type --delay 25 "events"
sleep 0.3
xdotool mousemove $((X + 937)) $((Y + 80))
sleep 0.3
xdotool click 1
sleep 1
import -window root /app/target/designer-preview.png
xdotool mousemove $((X + 1064)) $((Y + 80))
sleep 0.3
xdotool click 1
sleep 2
import -window root /app/target/designer-applied.png

# Transfer page is the fifth sidebar row.
xdotool mousemove $((X + 100)) $((Y + 234))
sleep 0.3
xdotool click 1
sleep 2
import -window root /app/target/transfer-page.png

# Pick the users table, fill the seed CSV path, and import.
xdotool mousemove $((X + 798)) $((Y + 78))
sleep 0.4
xdotool click 1
sleep 0.8
xdotool key End
sleep 0.2
xdotool key Return
sleep 0.5
xdotool mousemove $((X + 593)) $((Y + 130))
sleep 0.3
xdotool click 1
sleep 0.3
xdotool type --delay 20 "/tmp/unidb-import.csv"
sleep 0.3
xdotool mousemove $((X + 1064)) $((Y + 130))
sleep 0.3
xdotool click 1
sleep 3
import -window root /app/target/transfer-imported.png

# Security page is the seventh sidebar row.
xdotool mousemove $((X + 100)) $((Y + 314))
sleep 0.3
xdotool click 1
sleep 1
import -window root /app/target/security-page.png
echo "screenshots written"
