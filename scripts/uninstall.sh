#!/bin/sh
# Restore the stock app and remove d200fw from the D200.
set -e
. "$(dirname "$0")/common.sh"
need_device
res=$(sh_dev 'if [ -x /userdata/UlanziDeckKey.stock ]; then mv /userdata/UlanziDeckKey.stock /userdata/UlanziDeckKey && rm -rf /userdata/d200fw && sync && echo restored; else echo no-backup; fi')
if [ "$res" = restored ]; then
    echo "Stock app restored. Reboot to start it: adb reboot"
else
    echo "No /userdata/UlanziDeckKey.stock found; nothing to restore." >&2
    exit 1
fi
