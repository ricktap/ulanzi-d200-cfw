# com.deckling.core

The apps and actions that ship with D200 Deck but are not fixed in it. Night Signal, the
built-in icon set, is its embedded icon set (in `app/ui/icons/night-signal/`).

| File | What |
|---|---|
| [mod.rs](mod.rs) | the plugin: system stats, mute, weather, Pomodoro, media polling; its actions; readers the built-in widgets use |
| [media.rs](media.rs) | Spotify and Music through AppleScript (macOS), or a test file (`D200_MEDIA_FILE`) |
| [package/](package/) | manifest with settings (weather place, Pomodoro times), signals, actions, apps |

**Apps:** Clock, Media, Mute, Pomodoro, System monitor, Weather. Their default faces are the
built-in widgets (`apps.rs`); any other icon the user picks is fed by the signals.

**Actions:** `command` (run a shell command), `open` (a file, folder, app or URL), `keys` (a
shortcut), `media`, `volume`, `mute`, `weather_refresh`, `pomodoro`.

**Signals:** `system.*` (cpu, ram, disk, network, temperature, histories), `audio.muted`,
`weather.*`, `pomodoro.*`, `media.*` (title, artist, album, progress, position, player).

The weather comes from Open-Meteo; only the coordinates of the chosen place are sent.
