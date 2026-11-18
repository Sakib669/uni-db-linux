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
    kill "${FEED_PID:-}" 2>/dev/null || true
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
sqlite3 /tmp/unidb-demo.db "CREATE TABLE posts (id INTEGER PRIMARY KEY, user_id INTEGER REFERENCES users(id), title TEXT);"
printf 'name\nAlan Turing\nGrace Hopper\n' > /tmp/unidb-import.csv
mkdir -p /tmp/unidb-feed
printf '{"tag_name":"v0.2.0","html_url":"https://example.test/unidb-0.2.0","published_at":"2026-12-01T00:00:00Z"}' > /tmp/unidb-feed/latest.json
python3 -m http.server 8099 --directory /tmp/unidb-feed >/dev/null 2>&1 &
FEED_PID=$!
export UNIDB_UPDATE_FEED=http://127.0.0.1:8099/latest.json
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

# Back to the editor with the imported rows, then show the chart and the plan.
xdotool mousemove $((X + 100)) $((Y + 154))
sleep 0.3
xdotool click 1
sleep 1
xdotool mousemove $((X + 700)) $((Y + 240))
sleep 0.3
xdotool click 1
sleep 0.3
xdotool key ctrl+a
xdotool type --delay 20 "SELECT * FROM users;"
sleep 0.3
xdotool key ctrl+Return
sleep 2
xdotool mousemove $((X + 887)) $((Y + 78))
sleep 0.3
xdotool click 1
sleep 1
import -window root /app/target/editor-chart.png
xdotool mousemove $((X + 804)) $((Y + 78))
sleep 0.3
xdotool click 1
sleep 2
import -window root /app/target/editor-explain.png

# Capture a baseline, rerun the same query, and compare the two results.
xdotool mousemove $((X + 976)) $((Y + 78))
sleep 0.3
xdotool click 1
sleep 1
xdotool mousemove $((X + 500)) $((Y + 78))
sleep 0.3
xdotool click 1
sleep 2
xdotool mousemove $((X + 976)) $((Y + 78))
sleep 0.3
xdotool click 1
sleep 1
import -window root /app/target/editor-compare.png

# Schema page is the eighth sidebar row.
xdotool mousemove $((X + 100)) $((Y + 354))
sleep 0.3
xdotool click 1
sleep 3
import -window root /app/target/schema-erd.png

# Builder page is the ninth sidebar row.
xdotool mousemove $((X + 100)) $((Y + 394))
sleep 0.3
xdotool click 1
sleep 3
import -window root /app/target/builder-page.png

# Pick the users table and preview the generated SQL.
xdotool mousemove $((X + 1052)) $((Y + 78))
sleep 0.3
xdotool click 1
sleep 0.6
xdotool key End
sleep 0.2
xdotool key Return
sleep 1
xdotool mousemove $((X + 757)) $((Y + 612))
sleep 0.3
xdotool click 1
sleep 1
import -window root /app/target/builder-preview.png

# Transfer page fake data controls.
xdotool mousemove $((X + 100)) $((Y + 234))
sleep 0.3
xdotool click 1
sleep 2
import -window root /app/target/transfer-fake-page.png

# Choose the users table and generate fake rows.
xdotool mousemove $((X + 798)) $((Y + 78))
sleep 0.3
xdotool click 1
sleep 0.6
xdotool key End
sleep 0.2
xdotool key Return
sleep 1
xdotool mousemove $((X + 564)) $((Y + 212))
sleep 0.3
xdotool click 1
sleep 2
import -window root /app/target/transfer-fake-generated.png

# History page is the sixth sidebar row.
xdotool mousemove $((X + 100)) $((Y + 274))
sleep 0.3
xdotool click 1
sleep 2
import -window root /app/target/history-page.png

# Save the current editor statement as a named query.
xdotool mousemove $((X + 600)) $((Y + 321))
sleep 0.3
xdotool click 1
sleep 0.3
xdotool type --delay 25 "Recent users"
sleep 0.3
xdotool mousemove $((X + 895)) $((Y + 321))
sleep 0.2
xdotool click 1
sleep 2
import -window root /app/target/history-saved.png

# Settings page is the seventh sidebar row.
xdotool mousemove $((X + 100)) $((Y + 314))
sleep 0.3
xdotool click 1
sleep 2
import -window root /app/target/settings-page.png

# Check the release feed for a newer version.
xdotool mousemove $((X + 925)) $((Y + 260))
sleep 0.3
xdotool click 1
sleep 2
import -window root /app/target/settings-update.png

# Switch to the dark scheme, capture it, then go back to the system scheme.
xdotool mousemove $((X + 927)) $((Y + 142))
sleep 0.3
xdotool click 1
sleep 0.6
xdotool key End
sleep 0.2
xdotool key Return
sleep 1
import -window root /app/target/settings-dark.png
xdotool mousemove $((X + 927)) $((Y + 142))
sleep 0.3
xdotool click 1
sleep 0.6
xdotool key Home
sleep 0.2
xdotool key Return
sleep 1

# Security page is the tenth sidebar row.
xdotool mousemove $((X + 100)) $((Y + 434))
sleep 0.3
xdotool click 1
sleep 1
import -window root /app/target/security-page.png
echo "screenshots written"
