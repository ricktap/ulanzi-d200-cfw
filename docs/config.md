# deck.json

The app edits this file for you. This page is for hand edits and scripting. The source of truth
is `app/src-tauri/src/config.rs`; the target model is `docs/deckling/architecture.md`.

The file has a `version`. This app writes version 3. An older file is migrated when it is loaded
(`migrate.rs`); the original stays next to it as `deck.v<N>.json`. A newer version, or a file that
does not parse, is moved to `deck.json.old` and the app starts from `default-config.json`.
Fields, apps and actions this version does not know are kept when the file is saved.

```jsonc
{
  "version": 3,
  "settings": { ... },
  "status_key": { "app": "profile", "show": "auto", "on": { "press": [{"action": "core:home"}] } },
  "profiles": [
    {
      "name": "Default",            // the first profile is the fallback
      "apps": [],                   // bundle ids / app or exe names this profile follows
      "pages": {                    // ordered; the first page is home
        "main": {
          "0": { "states": [ { "label": "Vol +", "icon": "com.deckling.core:volume-up",
                               "on": { "press": [{"action": "com.deckling.core:volume", "params": {"value": "+5"}}] } } ] },
          "1": { "app": "com.deckling.core:sysmon", "config": { "modes": ["cpu", "ram"] } },
          "2": { "app": "com.deckling.core:clock", "states": [ { "icon": "com.deckling.core:pomodoro" } ] }
        }
      }
    }
  ]
}
```

Keys `0`-`12` are the square keys. The wide key (13) is the **status key** and is set up in
`status_key`.

## Key

| Field | Meaning |
|---|---|
| `app` | app id, see below. Missing: a plain button (`core:button`) |
| `config` | the app's settings for this key, by the schema in the app's `config` |
| `states` | list of states. State 0 is the first one. A `core:state` action cycles through them |

A key whose app comes from a plugin that is not installed shows "Plugin missing" and keeps all
its fields.

A state holds the look and the actions:

| Field | Meaning |
|---|---|
| `label` | text |
| `icon` | icon id (`"com.deckling.core:volume-on"`), emoji, or image path. On an app key: the chosen icon |
| `image` | full-key image |
| `props` | static interface properties for the icon, e.g. `{"toggle.on": true, "rgb.color": "#B79AF7"}`. An app supplies these itself |
| `bg`, `fg` | colours, `#rrggbb` |
| `on` | actions per event, see below |

On an app key, state 0 holds the chosen icon, label, colours and extra actions. While the chosen
icon is the app's default (or none is set), core apps draw their built-in widget. Any other icon
is drawn with its bindings, fed by the app's interfaces.

**`on`**

`press`, `long`, `down` and `up` each hold a list of actions:

- `down` and `up` fire on every press.
- Exactly one of `press` and `long` fires, depending on `settings.long_press_ms`.

## Namespaces

`core:` is only what is fixed in the app. Everything that ships as a plugin has its own id:

| Namespace | What |
|---|---|
| `core` | navigation, profiles, states, brightness, notifications, the plain button, MQTT |
| `com.deckling.core` | the built-in apps, commands, shortcuts, volume, media; the Night Signal icons |
| `com.deckling.teams` | Teams meeting controls and presence |
| `com.deckling.jetbrains` | the debugger app |
| `com.deckling.ha` | Home Assistant entities by id (app `com.deckling.ha:entity`), the remote page, MQTT publish |

## Actions (`{"action": ..., "params": {...}}`)

An action is the id of an action a plugin offers, with its parameters. Built-in actions:

| action | params | effect |
|---|---|---|
| `core:page` / `core:back` / `core:home` | `page` | navigation |
| `core:profile` | `profile` | a name, `next` or `auto` |
| `core:state` | `to` | `next` or an index |
| `core:brightness` | `value` | `128`, `+25`, `-25` |
| `core:notify` | `text`, `icon`, `color` | card on the status key |
| `com.deckling.core:command` | `command` | shell command |
| `com.deckling.core:open` | `target` | app, file, folder or URL |
| `com.deckling.core:keys` | `combo` | shortcut, for example `cmd+shift+4` |
| `com.deckling.core:volume` | `value` | `+5`, `-5`, `40`, `mute`, `unmute`, `toggle-mute` |
| `com.deckling.core:media` | `command` | `playpause`, `next`, `previous`, `play`, `pause` |
| `com.deckling.teams:meeting` | `command` | `toggle-mute`, `toggle-video`, `toggle-hand`, `toggle-background-blur`, `leave-call`, `toggle-ui-chat` |
| `com.deckling.teams:presence` | `to` | `toggle`, `auto`, `Available`, `Busy`, `DoNotDisturb`, `BeRightBack`, `Away`, `Offline` |
| `com.deckling.ha:mqtt` | `topic`, `payload`, `retain` | publish |
| `com.deckling.ha:remote` | | show the Home Assistant remote page |

The list, with a parameter schema per action, comes from the Tauri command `catalog`
(`actions.json` in `app/src-tauri/core/` and in each plugin under `app/src-tauri/plugins/`).

## Apps

| id | settings (`config`) |
|---|---|
| `com.deckling.core:sysmon` | `modes`: any of `cpu`, `ram`, `disk`, `net`. A press cycles through them |
| `com.deckling.core:mute` | |
| `com.deckling.core:weather` | uses `settings.weather` |
| `com.deckling.core:clock` | `h12` |
| `com.deckling.core:pomodoro` | uses `settings.pomodoro` |
| `com.deckling.core:media` | |
| `com.deckling.teams:mute`, `com.deckling.teams:camera`, `com.deckling.teams:presence` | |
| `com.deckling.jetbrains:debugger` | `idle_keys`, `active_keys`, `stop_keys` |

Each app is a file in its plugin's `apps/` folder: name, description, default icon, settings
schema and the interfaces it fulfils (`implements`). See [packages.md](packages.md).

## status_key

| Field | Values |
|---|---|
| `app` | `profile`, `media`, `meeting`, `email`, `steam`, `printer` |
| `show` | `auto` (the song while a player plays, else the default), `profile`, `media` |
| `on` | as for keys. The default is: press → home, long press → remote page |

## settings

| Section | Fields |
|---|---|
| top level | `brightness`, `long_press_ms`, `follow_active_app` |
| `weather` | `location {name, lat, lon}`, `fahrenheit` |
| `pomodoro` | `work`, `short_break`, `long_break` (minutes), `rounds`, `sound` |
| `mqtt` | `enabled`, `host`, `port`, `username`, `password`, `tls`, `base_topic`, `discovery` |
| `teams` | `port` (8124), `client_id`, `tenant`, `away` |
| `screensaver` | `enabled`, `idle_minutes`, `scene` (`auto`, `snow`, `pink`, `beach`, `leaves`, `rain`), `southern`, `motion`, `fps` |
| `cards` | `email`, `meetings`, `steam`, `printer`, `hide_content` |
| `theme` | `palette {active, info, attention, error, background}`, `widgets {<name>: {...}}` and `variables` |

`theme.variables` sets theme variables over the palette (`accent`, `background`, `base`,
`muted`, `info`, `warning`, `danger`, or any namespaced name). A value is a colour or a binding,
for example `"accent": {"bind": "time.second", "steps": ["#FF4D4D", "#3DDC84"]}`. Built-in
widgets follow the bound palette colours too.

For the theme fields per widget, see [design/widgets-theming.md](design/widgets-theming.md).
They are: `accent`, `light`, `light_opacity`, `idle_light_factor`, `background`, `border`,
`text`, `secondary`, `icon_color`, `icon_accent`, `chart`, `progress`, `track`, `network_up`,
`network_down`, `cover_tint`, and `layout` (`compact` / `airy`).

Widget names: `cpu`, `ram`, `disk`, `network`, `clock`, `mail`, `music`, `printer`, `meeting`,
`game`, `push`.
