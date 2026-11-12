# d200fw protocol

TCP on `127.0.0.1:7300` on the device. Reach it with `adb forward tcp:7300 tcp:7300`.

- Commands are text lines ending in `\n`.
- Image commands are followed by raw RGB888 bytes, row-major, in landscape orientation. The
  device rotates them.
- Each command gets one reply line, `OK` or `ERR ...` unless the table says otherwise.
- Only one host is served. When a new host connects, the old connection is dropped.

| Command | Reply / effect |
|---|---|
| `PING` | `PONG d200fw <version>` |
| `INFO` | `INFO version=.. keys=14 screen=1280x720 key=196x196 brightness=N` |
| `LAYOUT` | one `RECT <key> <x> <y> <w> <h>` per key, then `OK` |
| `IMG <key> <len>` + bytes | image for one key. `len` must be `w*h*3` for that key |
| `BLIT <x> <y> <w> <h>` + bytes | image anywhere on the 1280x720 screen |
| `IMGR <key> <len>` + bytes | like `IMG`, run-length coded (see below). Since 0.3.0 |
| `BLITR <x> <y> <w> <h> <len>` + bytes | like `BLIT`, run-length coded. Since 0.3.0 |
| `FILL <key> <rrggbb>` | fills a key with one colour |
| `CLEAR [rrggbb]` | fills the whole screen |
| `BRIGHT <0-255>` | sets the backlight |
| `SAVER <scene> <fps> <motion 0/1>` | the device draws the screensaver itself (`snow`, `pink`, `beach`, `leaves`, `rain`), fading in over what the keys show. Since 0.2.0 |
| `SAVER DOT <0/1>` | shows or hides the mint "notification waiting" dot on the wide key |
| `SAVER OFF` | stops the screensaver. So do any key press, `IMG`, `BLIT`, `FILL`, `CLEAR`, and the host disconnecting |
| `BYE` | closes the connection |
| *(device to host)* `KEY <key> <1/0>` | key press / release, sent at any time |

**Run-length coding** (`IMGR`, `BLITR`): RGB888 pixels as runs. A control byte `c`: `0..=127`
is followed by `c + 1` pixels that are copied; `128..=255` by one pixel that is repeated
`c - 126` times (2..129). The runs add up to exactly `w * h` pixels. Lossless; Night Signal keys
shrink to about 8 % (flat backgrounds, text, line icons). The app sends only the rectangle of a
key that changed (`BLITR`), and the whole key (`IMGR`) when that is smaller or nothing is known.

The screensaver scenes are the app's (`scene/`, shared code); the host sends only these
commands, no frames. A host that gets `ERR` for `SAVER` (older d200fw) draws the frames itself.

The device only reports raw down and up. Press, long press, states and pages all live in the
host app.

With no host connected, the device shows the key numbers and highlights presses.

## Command line

```
d200fw [options] [--stock <path> [-- <stock args>...]]
  --port <n>        TCP port on 127.0.0.1 (default 7300)
  --fb <dev>        framebuffer (default /dev/fb0)
  --input <dev>     keypad event device (default /dev/input/event0)
  --boot            boot mode: offer the stock UI first (needs --stock)
  --boot-wait <s>   seconds to wait for a key in boot mode (default 3)
  --stock <path>    stock app to exec when a key is held at boot
  --no-kill         do not stop the vendor app/watcher
  --test            show the key layout, print key presses, no server
```

`--fb` can be a plain file and `--input` a FIFO. With those, a build for the computer itself
works as a simulated device (see [testing.md](testing.md)).
