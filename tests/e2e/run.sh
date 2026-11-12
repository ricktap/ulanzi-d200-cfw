#!/bin/sh
# End-to-end tests: the real engine (headless) against a simulated D200
# (d200fw built for this computer, drawing into a file, keys from a FIFO),
# a minimal MQTT broker, a fake Teams API, a fake player and a fake IDE.
# Needs: Rust, a C compiler (cc), Python 3 with numpy and Pillow.
set -e
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
WORK=$(mktemp -d "${TMPDIR:-/tmp}/d200-e2e.XXXXXX")
HOST=$(rustc -vV | sed -n 's/^host: //p')
echo "building host d200fw ($HOST) and the headless engine..."
(cd "$ROOT/device" && cargo build --release --target "$HOST" -q)
(cd "$ROOT/app/src-tauri" && cargo build --example headless -q)
mkdir -p "$WORK/fakeide"
cc -o "$WORK/fakeide/webstorm" "$ROOT/tests/e2e/fakeide/webstorm.c"
export D200FW_HOST_BIN="$ROOT/device/target/$HOST/release/d200fw"
fail=0
for t in ${*:-test_core test_integrations test_screensaver test_wide_display test_packages test_web test_double}; do
    echo "== $t"
    python3 -I "$ROOT/tests/e2e/$t.py" "$ROOT" "$WORK" || fail=1
    pkill -f 'inspect=127.0.0.1:0' 2>/dev/null || true
    pkill -f 'Binaries/NMS' 2>/dev/null || true
done
echo "screenshots and logs: $WORK"
exit $fail
