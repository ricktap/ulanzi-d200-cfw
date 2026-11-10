#!/bin/sh
# Run d200fw on the D200 until the next reboot. Nothing on the device is changed
# permanently: the binary lives in /tmp and a reboot brings back the stock UI.
set -e
. "$(dirname "$0")/common.sh"
need_device
need_bin
adb push "$BIN" /tmp/d200fw >/dev/null
sh_dev "chmod 755 /tmp/d200fw; setsid /tmp/d200fw </dev/null >/tmp/d200fw.log 2>&1 & sleep 2"
adb forward tcp:$PORT tcp:$PORT >/dev/null
if sh_dev "netstat -tln" | grep -q ":$PORT "; then
    echo "d200fw is running (until reboot). Host port $PORT is forwarded."
    echo "Next: python3 host/deck.py host/deck.example.json"
else
    echo "d200fw did not start. Device log:" >&2
    sh_dev "cat /tmp/d200fw.log" >&2
    exit 1
fi
