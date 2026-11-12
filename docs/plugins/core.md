# Core apps (`com.deckling.core`)

The apps and actions that ship with D200 Deck. Its icon set is Night Signal.

## Apps

| App | On the key | Press | Long press |
|---|---|---|---|
| Clock | time and date (analog or digital) | | |
| System monitor | CPU, memory, disk or network with a live graph | next view | |
| Mute | whether the computer's sound is muted | mute or unmute | |
| Weather | temperature and conditions for a place | refresh | |
| Pomodoro | the timer ring and phase | start, pause, resume | reset |
| Media | what Spotify or Music plays, with the cover | play or pause | next track |

Each app also publishes its values as signals, so any other icon that fits (for example a ring
for the CPU) can show them.

## Actions

| Action | Does |
|---|---|
| `command` | runs a shell command |
| `open` | opens a file, folder, app or URL |
| `keys` | sends a keyboard shortcut |
| `media` | play, pause, next, previous |
| `volume` | louder, quieter, a level |
| `mute` | mute or unmute |
| `weather_refresh` | fetches the weather now |
| `pomodoro` | start or pause, reset |

## Settings (Packages, Core apps)

- **Weather:** the place (searched by name), Fahrenheit. The weather comes from Open-Meteo; only
  the place's coordinates are sent.
- **Pomodoro:** focus and break lengths, rounds, sound.

## Signals

`system.cpu`, `.ram`, `.disk`, `.net_down`, `.net_up`, `.temperature` and their histories,
`audio.muted`, `weather.*`, `pomodoro.*`, `media.*` (title, artist, album, progress, player).
The Signals view shows them live.

The deck itself (namespace `core`) publishes `key.event` on every key event:
`{key, event, profile, page, n}` with `event` one of `press`, `long_press`, `double_press`,
`down`, `up`. Plugins can follow it (`watches()`); the Web API forwards it to subscriptions with
`"events": ["keys"]`. `deck.state` is what Home Assistant shows of the deck (see
[Home Assistant](../home-assistant.md)); it changes when any part does.

Code: [app/src-tauri/src/plugins/core](../../app/src-tauri/src/plugins/core/).
