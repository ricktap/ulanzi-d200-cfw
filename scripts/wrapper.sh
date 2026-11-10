#!/bin/sh
# d200fw-wrapper: installed as /userdata/UlanziDeckKey by d200fw's install.sh.
# Starts d200fw instead of the stock app. The stock app is kept as
# /userdata/UlanziDeckKey.stock and is used when:
#   - a key is pressed during the boot prompt
#   - /userdata/d200fw/disable exists
#   - d200fw crashes 5 times in a row
STOCK=/userdata/UlanziDeckKey.stock
FW=/userdata/d200fw/d200fw
LOG=/tmp/d200fw.log

if [ -e /userdata/d200fw/disable ] || [ ! -x "$FW" ]; then
    exec "$STOCK" "$@"
fi

BOOT=--boot
n=0
while [ $n -lt 5 ]; do
    "$FW" $BOOT --stock "$STOCK" -- "$@" >>"$LOG" 2>&1 && exit 0
    echo "d200fw exited with $?, restart $n" >>"$LOG"
    BOOT=
    n=$((n + 1))
    sleep 2
done
echo "d200fw keeps crashing, falling back to the stock UI" >>"$LOG"
exec "$STOCK" "$@"
