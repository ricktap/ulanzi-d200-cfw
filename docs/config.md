# deck.json

The app edits this file for you. This page is for hand edits and scripting. The format can still
change while the project is a draft: a file that no longer loads is moved to `deck.json.old`,
and the app starts from `app/src-tauri/src/default-config.json`.

The source of truth is `app/src-tauri/src/config.rs`.

```jsonc
{
  "settings": { ... },
  "status_key": { "app": "profile", "show": "auto", "actions": { "press": [{"type": "home"}] } },
  "profiles": [
    {
      "name": "Default",            // the first profile is the fallback
      "apps": [],                   // bundle ids / app or exe names this profile follows
      "pages": {                    // ordered; the first page is home
        "main": {
          "0": { "states": [ { "look": {...}, "actions": {...} } ] },
          "1": { "app": { "type": "sysmon", "modes": ["cpu", "ram"] } }
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
| `app` | built-in widget, see below. Optional |
| `states` | list of states. State 0 is the first one. An `{"type":"state","to":"next"}` action cycles through them |

A state has a `look` and `actions`.

**`look`**

| Field | Meaning |
|---|---|
| `label` | text |
| `icon` | Night Signal name (`"volume-on"`), emoji, or image path |
| `image` | full-key image |
| `icon_state` | state for a stateful icon, for example `{"on": true}` or `{"temperature": 21.5, "progress": 60}` |
| `bg`, `fg` | colours, `#rrggbb` |

**`actions`**

`press`, `long`, `down` and `up` each hold a list of actions:

- `down` and `up` fire on every press.
- Exactly one of `press` and `long` fires, depending on `settings.long_press_ms`.

## Actions (`{"type": ..., ...}`)

| type | fields | effect |
|---|---|---|
| `command` | `command` | shell command |
| `open` | `target` | app, file, folder or URL |
| `keys` | `combo` | shortcut, for example `cmd+shift+4` |
| `page` / `back` / `home` | `page` | navigation |
| `profile` | `profile` | a name, `next` or `auto` |
| `state` | `to` | `next` or an index |
| `mqtt` | `topic`, `payload`, `retain` | publish |
| `brightness` | `value` | `128`, `+25`, `-25` |
| `notify` | `text`, `icon`, `color` | card on the status key |
| `remote` | | show the Home Assistant remote page |
| `media` | `command` | `playpause`, `next`, `previous`, `play`, `pause` |
| `teams` | `command` | `toggle-mute`, `toggle-video`, `toggle-hand`, `toggle-background-blur`, `leave-call`, `toggle-ui-chat` |
| `volume` | `value` | `+5`, `-5`, `40`, `mute`, `unmute`, `toggle-mute` |
| `presence` | `to` | `toggle`, `auto`, `Available`, `Busy`, `DoNotDisturb`, `BeRightBack`, `Away`, `Offline` |

## Built-in apps (`"app": {"type": ...}`)

| type | options |
|---|---|
| `sysmon` | `modes`: any of `cpu`, `ram`, `disk`, `net`. A press cycles through them |
| `mute` | |
| `weather` | uses `settings.weather` |
| `clock` | `h12` |
| `pomodoro` | uses `settings.pomodoro` |
| `media` | |
| `teams_mute`, `teams_camera`, `teams_presence` | |
| `debugger` | `idle_keys`, `active_keys`, `stop_keys` |

State 0 of an app key can still add colours and extra actions.

## status_key

| Field | Values |
|---|---|
| `app` | `profile`, `media`, `meeting`, `email`, `steam`, `printer` |
| `show` | `auto` (the song while a player plays, else the default), `profile`, `media` |
| `actions` | as for keys. The default is: press → home, long press → remote page |

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
| `theme` | `palette {active, info, attention, error, background}` and `widgets {<name>: {...}}` |

For the theme fields per widget, see [design/widgets-theming.md](design/widgets-theming.md).
They are: `accent`, `light`, `light_opacity`, `idle_light_factor`, `background`, `border`,
`text`, `secondary`, `icon_color`, `icon_accent`, `chart`, `progress`, `track`, `network_up`,
`network_down`, `cover_tint`, and `layout` (`compact` / `airy`).

Widget names: `cpu`, `ram`, `disk`, `network`, `clock`, `mail`, `music`, `printer`, `meeting`,
`game`, `push`.
