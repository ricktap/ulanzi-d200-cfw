#!/bin/sh
# Drives the configurator in a browser against the real engine (no device needed):
# builds examples/uibridge, starts it on a fresh deck.json and runs configurator.test.js.
# Needs Node and Playwright with Chromium (require('playwright')).
set -e
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
WORK=$(mktemp -d "${TMPDIR:-/tmp}/d200-ui.XXXXXX")
(cd "$ROOT/app/src-tauri" && cargo build --example uibridge -q)
PORT=${PORT:-8791}
D200_DIRECT=1 D200_PORT=7397 D200_MEDIA_FILE="$WORK/media.json" \
  "$ROOT/app/src-tauri/target/debug/examples/uibridge" "$WORK/deck.json" "$PORT" > "$WORK/bridge.log" 2>&1 &
PID=$!
trap 'kill $PID 2>/dev/null' EXIT INT TERM
sleep 2
node "$ROOT/tests/ui/configurator.test.js" "$WORK/deck.json" "$WORK" "http://127.0.0.1:$PORT/"
echo "screenshots: $WORK"
