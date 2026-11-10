#!/bin/sh
# Install d200fw so it starts at boot in place of the stock app.
# The stock app is kept as /userdata/UlanziDeckKey.stock; uninstall.sh restores it.
set -e
. "$(dirname "$0")/common.sh"
need_device
need_bin

state=$(sh_dev 'if head -n 3 /userdata/UlanziDeckKey 2>/dev/null | grep -q d200fw-wrapper; then echo wrapped; elif [ -f /userdata/UlanziDeckKey ]; then echo stock; else echo missing; fi')
case "$state" in
    stock|wrapped) ;;
    *) echo "/userdata/UlanziDeckKey not found: this firmware is laid out differently, not installing." >&2; exit 1 ;;
esac

echo "How the stock app is started on this unit:"
sh_dev "ps | grep -E 'UlanziDeckKey|WatcherProcess' | grep -v grep" | sed 's/^/  /'
launchers=$(sh_dev "grep -rl UlanziDeckKey /etc 2>/dev/null")
echo "  referenced in: ${launchers:-nothing under /etc}"

sh_dev "mkdir -p /userdata/d200fw"
adb push "$BIN" /userdata/d200fw/d200fw.new >/dev/null
adb push "$ROOT/scripts/wrapper.sh" /userdata/d200fw/wrapper.sh >/dev/null
sh_dev "chmod 755 /userdata/d200fw/d200fw.new /userdata/d200fw/wrapper.sh && mv /userdata/d200fw/d200fw.new /userdata/d200fw/d200fw"

if [ "$state" = stock ]; then
    sh_dev "mv /userdata/UlanziDeckKey /userdata/UlanziDeckKey.stock && cp /userdata/d200fw/wrapper.sh /userdata/UlanziDeckKey.tmp && chmod 755 /userdata/UlanziDeckKey.tmp && mv /userdata/UlanziDeckKey.tmp /userdata/UlanziDeckKey && sync"
    echo "stock app moved to /userdata/UlanziDeckKey.stock"
else
    sh_dev "cp /userdata/d200fw/wrapper.sh /userdata/UlanziDeckKey.tmp && chmod 755 /userdata/UlanziDeckKey.tmp && mv /userdata/UlanziDeckKey.tmp /userdata/UlanziDeckKey && sync"
    echo "updated existing install"
fi

check=$(sh_dev 'head -n 3 /userdata/UlanziDeckKey | grep -c d200fw-wrapper; [ -x /userdata/UlanziDeckKey.stock ] && echo stock-ok')
case "$check" in *stock-ok*) ;; *) echo "install check failed: $check" >&2; exit 1 ;; esac

echo
echo "Installed. Reboot the D200 to start d200fw:  adb reboot"
echo "At boot it shows 'PRESS A KEY: STOCK UI' for 3 s; press any key to get the stock app once."
echo "Then: adb forward tcp:$PORT tcp:$PORT && python3 host/deck.py host/deck.example.json"
