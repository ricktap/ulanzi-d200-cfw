# Running d200fw on the device

`d200fw` replaces only the stock **app**. The kernel, bootloader and system partitions stay
untouched, and adb stays available.

## Try it (until reboot)

```sh
scripts/test-keys.sh   # key grid; prints each press. Do this first on a new unit
scripts/try.sh         # copies the binary to /tmp and starts it
adb reboot             # back to stock
```

The D200 Deck app does the same thing on its own when it connects, so you don't need any of
this just to use the app.

When the app finds an older `d200fw` on the device, it replaces it with the one it embeds and
restarts it (once per app start). A boot install in `/userdata/d200fw/` is updated in place and
its wrapper restarts it; otherwise the copy in `/tmp` is replaced. The files are swapped with a
rename, so the running binary is never overwritten.

## Install for boot

```sh
scripts/install.sh
adb reboot
```

What `install.sh` changes:

1. It moves `/userdata/UlanziDeckKey` to `/userdata/UlanziDeckKey.stock`.
2. It puts `scripts/wrapper.sh` in its place.
3. It copies the binary to `/userdata/d200fw/`.

At boot the wrapper starts `d200fw --boot`, which shows **PRESS A KEY: STOCK UI** for 3 seconds.

## Ways back to stock

- Press any key during the boot prompt. The stock UI runs for that boot.
- `adb shell touch /userdata/d200fw/disable` gives the stock UI on every boot until you remove
  the file.
- If `d200fw` crashes 5 times in a row, the wrapper falls back to stock by itself.
- `scripts/uninstall.sh` and then `adb reboot` puts the original file back.
- If the device hangs at "Starting up", hold the four corner keys while powering on (see
  [hardware.md](hardware.md#recovery)).

## Notes

- While `d200fw` runs, Ulanzi Studio cannot control the device, because its USB HID link
  belongs to the stock app.
- A Ulanzi firmware update may overwrite the wrapper. Run `install.sh` again afterwards.
- The scripts use the local build in `device/target/armv7-unknown-linux-musleabihf/release/` if
  there is one, and `bin/d200fw` otherwise (`scripts/common.sh`).
