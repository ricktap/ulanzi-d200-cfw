# D200 hardware

Most of this was found by BadUStudio and D200-Unchained, then checked against a D200 while this
project was built.

## System

- SoC: Rockchip RK3308 (D200H: board V2.0, RK3308H3), 32-bit ARMv7, Buildroot Linux.
- USB gives an **adb root shell**. It needs no unlocking and runs as its own service, so it stays
  up whatever the deck app does.
- No network on the device. Everything goes over USB.
- Writable storage: `/userdata` (it persists) and `/tmp` (tmpfs, cleared at boot).

## Screen

- `/dev/fb0`: 720x1280, XRGB8888, stride 2880 bytes.
- The panel is mounted rotated. In landscape coordinates (1280x720), a pixel (x, y) is at
  framebuffer position `fb_x = y`, `fb_y = 1279 - x`. `d200fw` does the rotation; hosts always
  send landscape images.
- Backlight: `/sys/class/backlight/backlight/brightness` (0-255).

## Keys

The keys are a GPIO matrix keypad on `/dev/input/event0`, sending 16-byte `input_event`s
(32-bit ARM).

Keys are numbered in reading order. Each one shows the screen area under it:

| Key | Row | Window (landscape x, y, w, h) |
|---|---|---|
| 0-4 | top | x = 11 + 260·i, y = 3, 196x196 |
| 5-9 | middle | x = 11 + 260·(i-5), y = 262, 196x196 |
| 10-12 | bottom | x = 11 + 260·(i-10), y = 521, 196x196 |
| 13 | bottom right, wide | x = 791, y = 521, 456x196 |

Linux keycodes in key order: `29, 15, 14, 13, 12, 11, 10, 9, 8, 7, 34, 33, 31, 30`.

Run `scripts/test-keys.sh` on a new unit. Every key should light the window above it.

## Vendor processes

- `/userdata/UlanziDeckKey` is the stock app.
- `WatcherProcess` restarts it, and the two restart each other. The watcher also checks
  `/tmp/app.pid`.
- To take over the screen, stop both processes, then kill them (`SIGSTOP`, then `SIGKILL`).
  `d200fw` does this unless it runs with `--no-kill`.

## adb quirks

- `adb shell` does not return the command's exit code. Scripts echo a marker and check it.
- Background jobs started from `adb shell` die when the shell ends. Detach them with
  `setsid ... &`.
- `adb forward tcp:7300 tcp:7300` makes the device's 127.0.0.1:7300 reachable from the computer.

## Recovery

- **Stuck on "Ulanzi – Starting up":** hold the **four corner keys** while powering on. This
  fixed a boot hang during development.
- The Rockchip maskrom mode (`rkdeveloptool`) is the last resort if the system itself is
  damaged. `d200fw` never writes to system partitions.
