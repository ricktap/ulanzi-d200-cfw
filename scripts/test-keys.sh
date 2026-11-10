#!/bin/sh
# Diagnostic: show the key grid and print key presses (no host needed).
# Use it to check that the layout and key mapping match your D200/D200H.
# Ctrl+C to stop; reboot the device afterwards to get the stock UI back.
set -e
. "$(dirname "$0")/common.sh"
need_device
need_bin
adb push "$BIN" /tmp/d200fw >/dev/null
sh_dev "chmod 755 /tmp/d200fw"
adb shell /tmp/d200fw --test
