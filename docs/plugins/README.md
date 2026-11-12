# Plugins

D200 Deck ships with these plugins. Each one is a package with its own namespace: it brings
apps for keys, actions, signals and settings. Switch them on and off, and change their settings,
under **Packages** in the app.

| Plugin | Package id | What it does |
|---|---|---|
| [Core apps](core.md) | `com.deckling.core` | clock, system monitor, mute and volume, weather, Pomodoro, media; commands, opening things, shortcuts; the Night Signal icons |
| [Microsoft Teams](teams.md) | `com.deckling.teams` | meeting controls (mute, camera, hand, leave), presence, new mail and meetings on the wide key |
| [JetBrains](jetbrains.md) | `com.deckling.jetbrains` | whether a JetBrains IDE is debugging; resume or start debugging from a key |
| [Home Assistant](ha.md) | `com.deckling.ha` | Home Assistant entities on keys over MQTT, the remote page, the printer card |
| [Web API](web.md) | `com.deckling.web` | an HTTP API with a token (and an OpenAPI description) for scripts and other tools |

`core` without a prefix is not a plugin: it is what is fixed in the app (keys, pages, profiles,
states, brightness, notifications, the clock signals and the MQTT connection). Its settings are
the deck's global settings (Packages, Deck).

Besides plugins, a package can be an **icon set** or a **theme**. The app ships the themes Amber,
Glacier (full), Rainbow and CPU heat (partial); more icon sets and themes go into the packages
folder ([packages.md](../packages.md)).

How plugins work inside the app, and how to write one: [packages.md](../packages.md) and the
[plugins source folder](../../app/src-tauri/src/plugins/).
