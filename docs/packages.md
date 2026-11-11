# Packages, signals and bindings

How the Deckling model (`docs/deckling/architecture.md`) is built so far. Work packages 1 to 7
of `docs/deckling/briefing.md` are in place; 8 (third-party plugins) is not.

## Pieces

| Module | Role |
|---|---|
| `bind.rs` | `Signals` (latest value per signal, with a change set) and the resolver for `bind`, `prop`, `tone`, `config`, `steps`, `map`, `prefix`, `suffix`, `fallback`. Records what a resolution read |
| `registry.rs` | Loads packages: plugins (interfaces, signals, actions, apps, embedded icon set) and icon sets. Runs the load-time checks. `catalog()` for the UI |
| `icons.rs` | `apply_parts` sets resolved bindings into an SVG (`progress`, `visible`, `text`, any attribute); `render_bound` rasterises with resvg |
| `migrate.rs` | deck.json version 1 to 2, and `legacy_props` for the old `icon_state` objects |
| `app/src-tauri/core/` | `core`, what is fixed in the app: primitive interfaces, `time.*` signals, navigation actions, the plain button |
| `app/src-tauri/plugins/<id>/` | The built-in plugins as data: `com.deckling.core`, `com.deckling.teams`, `com.deckling.jetbrains`, `com.deckling.ha` |
| `app/ui/icons/night-signal/` | Night Signal, the icon set embedded in `com.deckling.core`: `deckling.json`, `svg/`, binding files in `icons/`, interface copies in `interfaces/` |

## Built-in plugins

`plugins.rs` is the interface for plugins that run in the engine's thread. A plugin publishes
signals in its own namespace (the host prefixes the id), takes its actions, fills `select`
fields from a `source`, and reads and writes MQTT through the app's connection: it names topics
relative to the base topic and publishes through the host. It never draws, sets no colour on a
key and sees no key presses.

| Plugin | Behind the plugin interface | Still in the engine |
|---|---|---|
| `com.deckling.ha` (`ha.rs`) | entities by id, the remote page keys, printer signals, `press` | `mqtt` and `remote` actions |
| `com.deckling.teams` (`teams_plugin.rs`, protocol in `teams.rs`) | meeting link, pairing, Microsoft 365 sign-in, presence, mail, calendar, `meeting` and `presence` actions | |
| `com.deckling.jetbrains` (`jetbrains.rs`) | debugger detection | |
| `com.deckling.core` (`core_plugin.rs`) | system monitor, mute and volume, weather, Pomodoro, media, commands, open, shortcuts | |

A plugin's calls: `mqtt_topics`/`on_mqtt`, `on_action`, `options`, `configure` (at start and
after every config change), `tick` (once a second), `on_message` (results of its own background
threads, sent with `Host::sender`) and `status` (merged into the UI's `integrations` event).

The cards on the wide key and the built-in Teams and debugger widgets are part of the core.
They read the plugins' signals back (`teams_plugin::mail`, `events`, `meeting`, `presence`,
`jetbrains::active`): a new mail, a meeting starting soon and pairing with Teams are seen as
signal changes.

MQTT itself is part of `core`: the connection, key events, profile, brightness, page,
availability, discovery and notifications.

Action parameters may name a key setting: `{"id": "{id}"}` is filled from the key's `config`
when the action runs. Placeholders that name no setting stay as they are.

## Packages folder

`<config dir>/packages/<id>/` holds icon sets and themes; each folder has a `deckling.json`.
`build.rs` embeds the built-in packages (`core/`, `plugins/`, `themes/`, Night Signal);
`registry::builtin()` parses them, `registry::load_disk` adds the folder's packages and makes the
result current (`registry::current()`). The engine loads the folder at start; the Packages view
can reload it, add a dropped folder (`install_package`, copied into the folder) and remove a
package from disk. Every icon set from disk goes through the load-time checks below, including
the comparison of its interface copies with the installed interfaces.

```json
{
  "id": "com.example.ocean",
  "type": "theme",
  "name": "Ocean",
  "theme": { "type": "full", "variables": { "background": "#0A1A2A", "base": "#E0F0FF", "muted": "#335577", "accent": "#33CCFF" } }
}
```

Built-in themes: `com.deckling.amber` and `com.deckling.glacier` (full), `com.deckling.rainbow`
(the accent cycles every second) and `com.deckling.cpu-heat` (the accent follows the CPU load).
deck.json keeps the choice in `theme` and the overlays in order in `overlays` (version 5).

## Icon sets

An icon set folder has a `deckling.json` (`"type": "iconset"`), SVGs in `svg/`, binding files in
`icons/` and copies of foreign interfaces it was drawn against in `interfaces/`. Every SVG is
an icon. Without a binding file it fulfils only `button` and has no bound parts.

A binding file:

```json
{
  "id": "com.deckling.core:light",
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

## Signals

`core` signals (the clock) are published by the engine every second (`Engine::publish_core`);
every plugin publishes its own. Types are in each package's
`signals.json`.

| Package | Signals |
|---|---|
| `core` | `time.clock`, `time.second`, `time.tick`, `time.hour_angle`, `time.minute_angle`, `time.second_angle`, `time.second_progress` |
| `com.deckling.core` | `system.cpu`, `system.ram`, `system.disk`, `system.net_down`, `system.net_up`, `audio.muted`, `weather.temperature`, `weather.summary`, `pomodoro.progress`, `pomodoro.running`, `pomodoro.phase`, `pomodoro.remaining`, `media.playing`, `media.title`, `media.artist`, `media.progress` |
| `com.deckling.teams` | `connected`, `paired`, `error`, `in_meeting`, `mic_on`, `video`, `meeting.muted`, `meeting.hand_raised`, `meeting.blurred`, `meeting.recording`, `meeting.sharing`, `meeting.can_toggle_mute`, `meeting.can_toggle_video`, `signed_in`, `account`, `presence`, `presence.activity`, `presence.error`, `mail.unread`, `mail.latest.id`, `mail.latest.from`, `mail.latest.subject`, `mail.latest.link`, `mail.error`, `calendar.count`, `calendar.<n>.id`, `.subject`, `.start`, `.end`, `.join`, `.teams` (next 10 events) |
| `com.deckling.jetbrains` | `debugger.active`, `debugger.name` |
| `com.deckling.ha` | `<id>.label`, `.value`, `.on`, `.color`, `.background`, `.progress`, `.kind`, `.icon`, `.present`; `printer.*` |

Plugin signals carry their namespace: `com.deckling.core:system.cpu`, `com.deckling.teams:mic_on`.

## Drawing a key

| Key | Drawn as |
|---|---|
| Core app, default icon (or none chosen) | the built-in widget (`apps.rs`), as before |
| App with another chosen icon, or a plugin app | that icon with its bindings, fed by the app's `implements` and the key's `config` |
| App from a missing plugin | dashed "Plugin missing" placeholder |
| Plain button | icon and label; bound icons read the state's `props` and theme variables |

The engine remembers what each key's icon read and redraws a plain button when one of those
signals changes. App keys redraw every second as before.

Theme variables come from `plugins.core.config.theme`: the palette gives `background`, `accent`, `info`,
`warning`, `danger`, plus fixed `base` and `muted`; `plugins.core.config.theme.variables` adds or overrides
any variable, also with bindings. Built-in widgets use the resolved palette colours.

## Tauri commands

| Command | Returns |
|---|---|
| `catalog` | plugins, interfaces, signals (with types), actions (with parameter schema), apps (with roles), icons (id, roles), warnings |
| `save_config(config)` | the deck; settings are read from `plugins.<id>.config` (`Config::settings` is the engine's typed view of them, written back by `Config::sync`) |
| `options(source, profile)` | values for a `select` with `{"source": ...}`: `core:pages`, `core:profiles` |
| `signals` | every signal with its current value and type |
| `preview_icon(icon, profile, page, key)` | the icon drawn with the live values of the key's app, as a data URL |

## Where this differs from the target model (for now)

- **Packages from disk are icon sets and themes.** Plugins in the packages folder are refused
  with a warning until third-party plugins run in their own process (work package 8).
- **Signals of type `series`** (`system.history.*`): the last values as a list, for the system
  monitor's graphs. The target model lists `flag`, `percent`, `number`, `color`, `text`.
- **Plugins run in the engine's thread.** `runtime.kind` `process` (own process, JSON RPC) is
  work package 8.
- **Default icons come from `com.deckling.core`.** The Teams and JetBrains apps use Night Signal
  icons instead of an icon set of their own.
- **Built-in widgets stay.** Core apps keep their drawn widgets as the face for the default
  icon. Apps publish signals for every other icon.
- **App behaviour on press is built in.** Core apps keep their press handling (cycling views,
  play/pause) in the engine, so their `on` presets are empty.
- **Apps may bind `icon`, `label`, `value` and `background`.** A plugin that decides per key
  what to show (Home Assistant) binds the app's default icon to a signal holding an icon id, and
  text, value and background likewise. An icon, label or colour chosen on the key wins.
- **The remote page** is the Home Assistant entities `remote-0` to `remote-12`, drawn with
  `com.deckling.ha:entity`.
- **Extra core interface `label { text }`** for icons that show a short text (thermostat).
  `clock` has `second_angle` in addition.
- **`props` on a state** give icons static values on keys without an app, on remote keys and on
  cards. The target model has no such field; it replaces `icon_state`.
- **Night Signal is the deck's own palette**, not a theme package: with no `theme` set, the
  palette in `plugins.core.config.theme` applies and the Theme block edits it.
  `plugins.core.config.theme.variables` comes after the overlays, as the deck's own overlay.
- **`range` in bindings:** `{"bind": ..., "range": [0, 100], "steps": [...]}` picks a step from
  low to high (clamped). Without `range`, steps cycle by the whole number (`time.tick`).
- **Settings schema extras:** a `config` field may have `group` (a heading in the Packages
  view), `min`/`max` (numbers), `placeholder`, `secret` (a password field) and the type `place`
  (a place search for the weather).
- **Configurator additions over the click dummy** (briefing 6.4):
  - Buttons can have several states ("+ State"): each has its own icon, label, colours and
    actions; new states get "Change state" (next) on press.
  - A bound icon on a button (no app feeds it) gets fixed values under "Icon values",
    stored as the state's `props`.
  - Buttons and app keys can have their own background colour (buttons also a text colour);
    without one the key follows the theme.
  - The Theme block edits the palette and `theme.variables`: a variable is a colour or bound
    to a signal, with a fallback and optional steps.
  - Not built: changing the order of overlays (there are no theme packages yet).
