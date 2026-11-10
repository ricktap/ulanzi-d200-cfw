# Packages, signals and bindings

How the Deckling model (`docs/deckling/architecture.md`) is built so far. Work packages 1 to 4
of `docs/deckling/briefing.md` are in place; 5 to 8 are not.

## Pieces

| Module | Role |
|---|---|
| `bind.rs` | `Signals` (latest value per signal, with a change set) and the resolver for `bind`, `prop`, `tone`, `config`, `steps`, `map`, `prefix`, `suffix`, `fallback`. Records what a resolution read |
| `registry.rs` | Loads packages: plugins (interfaces, signals, actions, apps, embedded icon set) and icon sets. Runs the load-time checks. `catalog()` for the UI |
| `icons.rs` | `apply_parts` sets resolved bindings into an SVG (`progress`, `visible`, `text`, any attribute); `render_bound` rasterises with resvg |
| `migrate.rs` | deck.json version 1 to 2, and `legacy_props` for the old `icon_state` objects |
| `app/src-tauri/core/` | The core plugin as data: `deckling.json`, `interfaces.json`, `signals.json`, `actions.json`, `apps/*.json` |
| `app/ui/icons/night-signal/` | Night Signal, the core's embedded icon set: `deckling.json`, `svg/`, binding files in `icons/`, interface copies in `interfaces/` |

## Icon sets

An icon set folder has a `deckling.json` (`"type": "iconset"`), SVGs in `svg/`, binding files in
`icons/` and copies of foreign interfaces it was drawn against in `interfaces/`. Every SVG is
an icon. Without a binding file it fulfils only `button` and has no bound parts.

A binding file:

```json
{
  "id": "core:light",
  "roles": ["rgb", "toggle", "button"],
  "file": "light.svg",
  "parts": { "light-glass": { "fill": { "prop": "rgb.color", "fallback": { "tone": "accent" } } } }
}
```

`progress` sets `stroke-dasharray`. resvg ignores `pathLength`, so the length comes from the
element's `data-circumference` (100 when missing) and `pathLength` is removed while drawing.

The eight former stateful icons (`light`, `volume-muted`, `thermostat`, `pomodoro`,
`analog-clock`, `bambu-ams`, `bambu-ams-lite`, `downloads`) are binding files now. Their
fallbacks draw the old defaults.

## Checks at load time

As in the icon workbench report, collected in `Registry.warnings` and shown in the catalog:

- definitions outside the package's namespace
- bindings without `fallback` (`prop`, `bind`, and `tone` outside the core variables)
- parts named in a binding file but missing from the SVG
- properties an icon reads for a role it does not declare, or that the interface does not have
- an icon set's interface copy that differs from the installed interface
- two packages with the same id

## Core signals

Published by the engine every second (`Engine::publish_core`). Types are in
`app/src-tauri/core/signals.json`.

`time.clock`, `time.second`, `time.tick`, `time.hour_angle`, `time.minute_angle`,
`time.second_angle`, `time.second_progress`, `system.cpu`, `system.ram`, `system.disk`,
`system.net_down`, `system.net_up`, `audio.muted`, `weather.temperature`, `weather.summary`,
`pomodoro.progress`, `pomodoro.running`, `pomodoro.phase`, `pomodoro.remaining`,
`media.playing`, `media.title`, `media.artist`, `media.progress`, `teams.connected`,
`teams.in_meeting`, `teams.mic_on`, `teams.video`, `presence.availability`, `debugger.active`.

## Drawing a key

| Key | Drawn as |
|---|---|
| Core app, default icon (or none chosen) | the built-in widget (`apps.rs`), as before |
| App with another chosen icon | that icon with its bindings, fed by the app's `implements` and the key's `config` |
| App from a missing plugin | dashed "Plugin missing" placeholder |
| Plain button | icon and label; bound icons read the state's `props` and theme variables |

The engine remembers what each key's icon read and redraws a plain button when one of those
signals changes. App keys redraw every second as before.

Theme variables come from `settings.theme`: the palette gives `background`, `accent`, `info`,
`warning`, `danger`, plus fixed `base` and `muted`; `settings.theme.variables` adds or overrides
any variable, also with bindings. Built-in widgets use the resolved palette colours.

## Tauri commands

| Command | Returns |
|---|---|
| `catalog` | plugins, interfaces, signals (with types), actions (with parameter schema), apps (with roles), icons (id, roles), warnings |
| `options(source, profile)` | values for a `select` with `{"source": ...}`: `core:pages`, `core:profiles` |
| `signals` | every signal with its current value and type |
| `preview_icon(icon, profile, page, key)` | the icon drawn with the live values of the key's app, as a data URL |

## Where this differs from the target model (for now)

- **Packages are built in.** `build.rs` embeds the core plugin and Night Signal as package
  files; `registry.rs` parses them at runtime through the same loader that will read packages
  from disk (work package 7). The registry is built once (`registry::builtin()`).
- **Settings stay in `settings`.** The core plugin's `config` schema names keys there
  (`long_press_ms`, `screensaver.enabled`, ...). Moving them to `plugins.<id>.config` comes with
  the integrations (work package 5).
- **Teams, JetBrains and MQTT are still core.** Their apps and signals carry core names
  (`core:teams_mute`, `teams.mic_on`) until package ids are decided (architecture section 13.1).
- **Built-in widgets stay.** Core apps keep their drawn widgets as the face for the default
  icon. Apps publish signals for every other icon.
- **App behaviour on press is built in.** Core apps keep their press handling (cycling views,
  play/pause) in the engine, so their `on` presets are empty.
- **Extra core interface `label { text }`** for icons that show a short text (thermostat).
  `clock` has `second_angle` in addition.
- **`props` on a state** give icons static values on keys without an app, on remote keys and on
  cards. The target model has no such field; it replaces `icon_state`.
- **Themes as packages, overlays and their order** come with work package 7. Until then,
  `settings.theme.variables` plays the part of one overlay.
