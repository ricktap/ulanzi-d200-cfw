# D200 Deck (desktop app)

Tauri app (Rust backend, plain HTML/JS UI) that drives an Ulanzi D200 running d200fw:
it draws every key, runs your actions, and edits the config.

## Design

Keys follow the Night Signal style:
- near-black faces with ivory line icons
- uppercase, letter-spaced labels and big bold figures
- status pills: mint means on or live, coral means muted or off, amber means away
- palette accents: a lavender graph, an amber pomodoro ring

The status key shows the profile (AUTO plus the app in front), whether Home Assistant is
connected, and the song or the clock.

## Configurator

The app's window follows `docs/deckling/briefing.md`, section 6. A sidebar switches between
three views.

- **Deck**: the library on the left (Apps, Actions, Icons), the device in the middle and the
  inspector on the right.
  - Tap an app to put it on the selected key, or drag it onto a key. Drag a key onto another to
    swap them. Drop an image file on a key to make it a button with that image.
  - Actions from the library go to the section marked as target in the inspector: On press,
    Long press, or Extended (key down, key up).
  - Icons shows only icons that share a role with the key's app, drawn with the app's live
    values. "Apply set" and "Restore default icons" work on the whole page.
  - Above the device: the profile, "+ Profile" and "Edit profile". The editor sets the name,
    the programs that switch to the profile, and the pages.
  - Below it: the pages, a line that says what just happened, brightness and the Theme block
    (palette colours and theme variables, also bound to a signal).
  - A button can have several states, each with its own icon, label, colours and actions; the
    action Change state switches between them. Emoji work as icons.
- **Packages**: every plugin with its switch, its state and its settings (from the plugin's
  `config` schema). `core` holds the deck's global settings, including MQTT. Teams has a guided
  setup. The Application card has autostart, export and import, and updates.
- **Signals**: every signal and its current value, updated once a second.

Every change is saved at once; Undo (or Cmd/Ctrl+Z) takes it back.

## Stateful icons

Eight icons take a state, from Home Assistant (`icon_state` on a remote key, translated to
props) or from the apps themselves (binding files, see [packages.md](packages.md)):
- `light` (on, color)
- `thermostat` (temperature, level, color)
- `pomodoro` (ring %, color)
- `analog-clock` (hands, ring, color; the clock widget's header shows the live time)
- `bambu-ams` and `bambu-ams-lite` (four spool colors and remaining %, also on the printer card)
- `downloads` (arrow color)
- `volume-muted` (mark color)

The state object matches `setIconState` from the icon package, e.g.
`{"on": true, "color": "#B79AF7"}` or `{"temperature": 21.5, "progress": 55}`.

## Theme

The Theme block under the device sets the palette colours (accent, info, warning, danger,
background). The per-widget overrides below are still read from deck.json
(`plugins.core.config.theme.widgets`), but the configurator no longer edits them: in the
target model they become theme variables. The widget package's THEMING.md:
- a semantic **palette** (active, info, attention, error, surface)
- per-widget overrides for `cpu`, `ram`, `disk`, `network`, `clock`, `mail`, `music`,
  `printer`, `meeting`, `game` and `push`:
  - accent and background light (color, intensity, idle factor)
  - surface, border, text and secondary text
  - icon strokes and icon accent
  - chart, progress and track
  - upload and download colors (network), cover tint (music, game)
  - layout for wide cards: **Compact** (42% edge-to-edge cover) or **Airy** (24% square cover,
    more padding, a short metadata line)

Error states keep their warning color. Icon accents follow the widget accent unless set.

## Wide display

The wide key shows one thing at a time. That's its **default content**: the profile, Spotify /
Music, the next meeting, email, the Steam game, or the printer. Pick it by clicking the key in
the grid. Otherwise it shows a **card** that takes over:

| Card | Source | Press | Hold |
|---|---|---|---|
| Meeting (5 min before, at start) | Microsoft 365 calendar (Teams sign-in) | open the join link | dismiss |
| Printer complete / error | `d200/printer` on MQTT | – | dismiss |
| Push | `d200/notify`, actions, pomodoro | open its `url` | dismiss |
| Email (new unread) | Microsoft 365 mail (sender and subject only) | open the message | dismiss |
| Steam (launched / exited) | the game's process under `steamapps/common`, cover from Steam | – | dismiss |

- **Priority:** critical › meeting › printer › push › email. A higher-priority card interrupts,
  and a lower one waits.
- **Duration:** ordinary cards stay 8 s; critical cards stay until you press the key.
- **Grouping:** cards of the same group replace each other, e.g. emails, or one print job.
- **Presses** act on the card that was visible when the press started.
- **Privacy:** Packages → Deck → Cards on the wide key can hide mail content.

Mail and calendar need the Teams sign-in to include `Mail.ReadBasic` and `Calendars.Read`. Sign
in again after updating.

## Screensaver

After a few idle minutes (no key presses), all 14 keys fade into one shared seasonal scene:
- winter: snow and a snowman
- spring: pink petals and a butterfly
- summer: a beach tide and a crab
- autumn: leaves and an acorn
- or rain, with a duck under an umbrella

"Automatic" follows the season, with an option for the southern hemisphere.

- **Waking:** a key press wakes the deck *without* running that key's action.
- **Alerts:** critical cards wake it; other cards wait behind a small mint dot.
- **Performance:** frames are sent over USB at the FPS you set (default 8). Turning motion off
  gives a still scene.

## Concepts

**Profiles** hold pages of keys. A profile can be bound to apps (bundle id such as
`com.apple.Safari`, or an app name). With *Switch with the active app* on, the deck
switches to the matching profile when that app comes to the front. If nothing matches, it
uses the default profile (the first one). Switching by hand (a profile action, HA, or the
sidebar) holds until the active app changes again.

**Keys 0–12** are buttons or built-in apps. **Key 13** (the wide one) is the *status key*: it
shows the current profile and page, and plays notifications.

**Four events per key**, all usable together:

| Event | When |
|---|---|
| Press | released before the long-press time (default 500 ms) |
| Long press | fires while still held, once the long-press time has passed |
| Key down | the moment the key goes down |
| Key up | every release |

For each press, either *Press* or *Long press* fires, never both. Every event takes a list of
actions:
- run a command, open an app/file/URL, or send a keyboard shortcut
- go to a page, the previous page or the home page
- switch profile (by name, `next`, or `auto`)
- change this key's state
- publish to MQTT
- set the brightness, or the volume (step up/down, set, mute)
- show a notification
- show the Home Assistant page
- media: play/pause, next, previous (Spotify / Music)
- Teams meeting: mute, camera, hand, blur, chat, leave
- Teams presence: toggle, back to automatic, or a specific status

**States:** a button can have several states, each with its own look and its own actions.
The usual toggle gives each state a "Change this key's state → Next" action. The current
state is remembered across restarts (in `state.json`).

**Built-in apps:**

| App | Press | Long press |
|---|---|---|
| System monitor | next view: CPU (+ temperature where the host reports it), RAM, disk (10-segment bar), network ↑ mint / ↓ lavender | – |
| Mute | toggle system mute (the icon follows the real state) | – |
| Weather | refresh (Open-Meteo; place set in Settings) | – |
| Time / date | toggle time and date | – |
| Pomodoro | start / pause / resume | reset |
| Media play/pause | play / pause (Spotify or Apple Music), shows state, song and artwork | next track |
| Teams mute | mute / unmute, live mic state | – |
| Teams camera | camera on / off, live state | – |
| Teams presence | available ↔ unavailable, live status | back to automatic |
| JetBrains debugger | start debugging, or resume while debugging; lights up while a debug session runs | stop |

App keys can have extra actions on top of their own behaviour.

**Status key content:** "auto" shows the song while Spotify or Music plays, and the profile
otherwise. A profile switch shows the profile for a moment. You can also choose "always the
profile" or "the song whenever one is loaded". Set it by clicking the status key in the grid.

**Example profiles:** Browser, Media (bound to Spotify and Music), Teams (new Teams) and
JetBrains (WebStorm, IntelliJ, PyCharm, …) are in the built-in example config.

**Icons:** the bundled **Night Signal** set (215 line icons for dark keys, in
`ui/icons/night-signal/`), picked by name (`"icon": "volume-on"`) from a searchable grid in the
editor. The built-in apps use it too. A key can also use an emoji, or an image file: drop one on
a key. Set icons keep their palette (ivory, mint, coral, amber, lavender). A custom text colour
recolours their ivory strokes.

## Home Assistant (MQTT)

The D200 has no network of its own, so the computer it's plugged into is the bridge. Run
D200 Deck with the same MQTT settings on every computer the KVM switches to. Remote-page
keys are retained on the broker, so the computer that has the D200 picks them up. See
[../docs/home-assistant.md](home-assistant.md) for topics and example automations.

## Integrations

**Media.** Spotify and Apple Music are controlled through AppleScript. macOS asks once to allow
D200 Deck to control each player. Browsers (YouTube etc.) can't be read: since macOS 15.4,
Apple only lets its own apps read the system "Now Playing" info. The player is never launched by
polling, but play/pause with no player open starts Music, like the keyboard's play key.

**Teams meetings.** New Teams has a local third-party API. Turn it on in Teams → Settings →
Privacy → *Manage API*. The first time you press a Teams key during a meeting, Teams asks to
allow D200 Deck. The pairing token is saved in `teams.json`. State is only reported during a
meeting.

**Teams presence.** This isn't in the local API. It uses Microsoft Graph
(`/me/presence`, `setUserPreferredPresence`) with a device-code sign-in under Packages → Microsoft Teams.
It needs an Entra app registration:
1. Create it: App registrations → New, single tenant.
2. Under Authentication, turn on *Allow public client flows*.
3. Under API permissions, add the delegated Microsoft Graph permission *Presence.ReadWrite*.
   Your admin may have to approve it.

Enter its client ID and your tenant (company domain or tenant ID). Toggling sets your chosen
"unavailable" status (default: Do not disturb) for 8 hours. Pressing again, or a long press,
goes back to automatic. Presence refreshes every 30 s. Tokens are kept in `teams-auth.json`
(owner-only).

**JetBrains debugger.** JetBrains IDEs have no API for this. The key watches for debuggee
processes started by an IDE: `node --inspect`, JDWP, debugpy/pydevd, dlv. The actions are the
IDE's macOS-keymap shortcuts (Debug ⌃D, Resume ⌥⌘R, Stop ⌘F2) and can be changed per key.
They go to the app in front, and need the Accessibility permission like all shortcuts.

## Platform notes

| | macOS | Windows | Linux |
|---|---|---|---|
| Deck, actions, MQTT, weather, clock, pomodoro, system monitor | yes | yes (untested) | yes (untested) |
| Active-app profiles | yes | yes (untested) | no |
| Mute app | yes | no | yes (`pactl`) |
| Keyboard shortcuts, JetBrains actions | yes (needs Accessibility permission) | no | no |
| Media (Spotify / Music) | yes | no | no |
| Teams meeting controls | yes (untested against real Teams) | should work (same API) | – |
| Teams presence | yes (untested against a real tenant) | yes | yes |
| Debugger detection | yes | yes (untested) | yes (untested) |

On macOS, shortcuts are sent through System Events. The first time, allow **D200 Deck** in
System Settings → Privacy & Security → Accessibility (and Automation if asked).

## Files

`~/Library/Application Support/dev.d200fw.deck/` on macOS:
- `deck.json`: the config. It's readable only by you, because it can hold the MQTT password.
- `state.json`: key states and app views.
- `teams.json`, `teams-auth.json`: Teams pairing token and Microsoft sign-in, only if you use them.

The format can still change between versions. A `deck.json` that no longer loads is moved
to `deck.json.old`, and the app starts from the built-in example.

## Build

```sh
cd app
npm install
npx tauri build      # -> src-tauri/target/release/bundle/{macos,dmg}
npx tauri dev        # run with live reload of the UI
cd src-tauri && cargo test --lib
```

The device firmware is embedded from `../bin/d200fw`. After changing `device/`, rebuild
it and copy it to `bin/`. Builds only target the OS you build on.

## Testing without a device

`D200_DIRECT=1 D200_PORT=<port>` skips adb and connects straight to that port, for example
a Mac build of d200fw run with `--no-kill --fb <file> --input <fifo>`.
`cargo run --example headless -- <deck.json>` runs the engine without the UI.
