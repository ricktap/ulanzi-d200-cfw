# Testing

## Unit tests

```sh
cd app/src-tauri && cargo test --lib
```

They cover:

- **config:** the example config and the UI's output parse; invalid keys are rejected; an
  unparsable file starts fresh
- **icons:** all 218 icons render; colour overrides; stateful icons
- **cards:** card priority
- **screensaver:** the scenes render and move
- **pomodoro:** the cycle
- **parsers:** the media player, Teams meeting updates, Graph times, Steam appmanifests
- **shortcuts:** shortcut combos and active-app matching

## End-to-end tests (no device needed)

```sh
tests/e2e/run.sh
```

The script:

1. builds `d200fw` for the computer (`--target` = host triple) and the `headless` example
2. compiles a fake JetBrains IDE (`tests/e2e/fakeide/webstorm.c`)
3. runs four suites against a **simulated device**: a plain file as framebuffer and a FIFO as
   keypad

| Suite | Covers |
|---|---|
| `test_core.py` | protocol, layout, rotation, key events, press/long/down/up, states, pages, profiles, reconnects |
| `test_integrations.py` | MQTT (`minibroker.py`), Home Assistant discovery, the remote page, Teams local API (`faketeams.py`), the debugger key, media through a fake player file |
| `test_screensaver.py` | idle timeout, fades, wake swallowing the press, critical cards waking it |
| `test_wide_display.py` | status-key cards: priority, coalescing, critical cards until dismissed, Compact/Airy layouts |

`render_themes.py` writes theme comparison sheets as images. It is a visual check, not a test.

The tests need Python 3 with no extra packages, and the Rust toolchain.

## Test hooks (environment variables)

| Variable | Effect |
|---|---|
| `D200_DIRECT=1` | skip adb and connect straight to `D200_PORT` |
| `D200_PORT=<n>` | protocol port (default 7300) |
| `D200_MEDIA_FILE=<path>` | read and write the player state from a file instead of AppleScript, so tests never touch your real Spotify |
| `D200FW_HOST_BIN=<path>` | the host build of `d200fw` used by the e2e tests |
| `D200_HEADLESS=<path>` | a prebuilt `headless` example binary (optional) |

## Headless engine

```sh
cd app/src-tauri && cargo run --example headless -- path/to/deck.json
```

It reads commands on stdin:

- `state`
- `view <profile> <page> [remote]`
- `preview <key>`
- `trigger <key> <event>`
- `saver on|off`
- `notify <text>`

The e2e suites use it.

## Manual check on the device

1. `scripts/test-keys.sh` checks keys and windows.
2. `npx tauri dev`, then go through each key of the default profile.
