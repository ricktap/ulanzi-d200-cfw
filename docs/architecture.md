# Architecture

```
 computer                                               D200 (Linux, ARMv7)
┌──────────────────────────────────────────────┐  USB  ┌───────────────────────┐
│ D200 Deck (Tauri)                            │  adb  │ d200fw                │
│  ui/ (HTML/JS) ──invoke──▶ lib.rs commands   │ ────▶ │  TCP 127.0.0.1:7300   │
│                              │               │ fwd   │  /dev/fb0 (rotated)   │
│                              ▼               │       │  /dev/input/event0    │
│                    engine.rs (one thread)    │ ◀──── │  KEY k 1/0            │
│        ▲      ▲       ▲        ▲       ▲     │       └───────────────────────┘
│     device  mqtt   teams    media    ide     │
│     .rs     .rs    .rs      .rs      .rs     │──── MQTT ────▶ Home Assistant
└──────────────────────────────────────────────┘
```

## Device side (`device/`)

`d200fw` has no dependencies.

| File | Contents |
|---|---|
| `hw.rs` | framebuffer, rotation, backlight, keypad |
| `font.rs` | a tiny built-in font for the idle screen and boot prompt |
| `main.rs` | arguments, stopping the vendor processes, the TCP server, the boot prompt |

Its job is to draw what it is sent and report the keys, and to run the screensaver
(`saver.rs`) on the device, so no frames go over USB. The scenes are in `scene/`, a small
crate (tiny-skia) shared with the app.

## Desktop app (`app/src-tauri/src/`)

The **engine** (`engine.rs`) is a single thread with a `Msg` channel. Every input becomes a
message:

- key events from the device
- MQTT messages
- ticks
- UI commands
- updates from the pollers: Steam
- results from the built-in plugins' background work (`Msg::Plugin`: media, weather, Teams, Graph, ...)

The engine owns all state. It decides what each key shows and sends only the keys that changed.

| Module | Role |
|---|---|
| `lib.rs` | Tauri setup and the commands the UI calls (`get_config`, `save_config`, `icon_set`, `preview`, `trigger`, `teams_sign_in`, …) |
| `config.rs` | `deck.json` types (serde), loading and saving, defaults (`default-config.json`). Unknown fields are kept |
| `migrate.rs` | `deck.json` migrations between versions |
| `bind.rs` | signals and the binding resolver (see [packages.md](packages.md)) |
| `registry.rs` | packages: interfaces, signals, actions, apps, icons; load-time checks; the `catalog` |
| `engine.rs` | key handling: down, up, press and long press; states, pages, profiles, the status key, the card queue, the screensaver |
| `render.rs` | the drawing canvas: tiny-skia plus ab_glyph text (variable SF weights, tracking, emoji fallback). Also `Th`, the resolved theme |
| `apps.rs` | built-in widgets: system monitor, mute, weather, clock, pomodoro, media, Teams, debugger |
| `cards.rs` | wide-key notification cards (meeting, email, printer, push, critical) in Compact and Airy layouts |
| `screensaver.rs` | which season to show; the scenes themselves are in `scene/` (shared with d200fw, which draws them; the app draws them only for older firmware) |
| `icons.rs` | Night Signal SVGs compiled in (`build.rs` → `icons_gen.rs`), resvg rendering, `apply_parts` for bound icons |
| `device.rs` | adb detection, pushing the binary, port forwarding, the protocol client |
| `mqtt.rs` | rumqttc client, topics, Home Assistant discovery |
| `plugins/mod.rs` | the interface for built-in plugins (signals, actions, options, MQTT, ticks, background results, requests to the core) |
| `plugins/ha/` | plugin `com.deckling.ha`: Home Assistant entities, the remote page, printer |
| `plugins/teams/` | plugin `com.deckling.teams`: meeting link, presence, mail, calendar as signals |
| `plugins/jetbrains/` | plugin `com.deckling.jetbrains`: debugger state |
| `plugins/web/` | plugin `com.deckling.web`: the HTTP API (its own server thread, requests handled in the engine's thread) |
| `plugins/core/` | plugin `com.deckling.core`: system, mute and volume, weather, Pomodoro, media, commands |
| `plugins/teams/api.rs` | Teams local WebSocket API (meetings) and Microsoft Graph (presence, mail, calendar) |
| `plugins/core/media.rs` | Spotify / Music through AppleScript |
| `plugins/jetbrains/ide.rs` | debugger detection (process scan) and the Steam game currently running |
| `platform.rs` | front app, keyboard shortcuts, volume, open, per OS |

The UI (`app/ui/`) is plain HTML/CSS/JS with `withGlobalTauri`, so there is no build step. It
renders key previews by asking the engine (`preview`), so the editor and the device always show
the same pixels.

The UI code is ES modules in `app/ui/js/` (`main.js` boots it). All state is in `store.js`: every
change goes through `store.edit(label, fn)`, which keeps an undo step (Undo button, Cmd/Ctrl+Z)
and saves deck.json. Apps, actions and icon roles come from the `catalog` command, so the UI has
no list of its own.

| Module | Part of the window |
|---|---|
| `library.js` | the apps on the left |
| `picker.js` | the picker dialog for icons and actions |
| `deck.js` | the device: previews, selection, dragging, image drops, page tabs |
| `profiles.js` | the profile editor above the device |
| `theme.js` | the Theme block on the Appearance view |
| `inspector.js`, `actions.js` | the inspector: what the key shows (app, settings, icons, text, colours) and does (action sections) |
| `packages.js` | the Packages view and the Application card |
| `signals.js` | the Signals view |

`cargo run --example uibridge -- deck.json 8790` serves the UI to a normal browser against the
real engine (commands over HTTP), for working on the UI and for UI tests with Playwright.

## Rendering

- Keys are drawn at device resolution: 196x196, and 456x196 for the wide key.
- The canvas works in **Night Signal** tokens. Colours come from `Th::resolve(theme, widget, tone)`.
  Widgets never hard-code colours.
- SVG icons are rendered with resvg. Bound icons get their resolved values as string edits
  before rendering, because resvg ignores CSS `calc()`/`var()`. `icons.rs` removes those and sets
  the plain attributes on the `data-part` elements.
- The Deckling model (plugins, apps, interfaces, signals, bindings) is described in
  [packages.md](packages.md).

## Data on disk

On macOS: `~/Library/Application Support/dev.d200fw.deck/`

- `deck.json`: the config, owner-only because it can hold the MQTT password
- `state.json`: key states and views
- `teams.json`, `teams-auth.json`: Teams pairing token and Graph tokens, owner-only
