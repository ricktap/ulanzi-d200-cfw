# Home Assistant

MQTT is part of the app: D200 Deck connects to your MQTT broker (Settings → Home Assistant) and
offers the deck itself there (key events, profile, brightness, notifications, discovery). The
plugin `com.deckling.ha` reads and writes Home Assistant's keys through that connection. With discovery on,
Home Assistant gets a **Ulanzi D200** device:

| Entity | What it does |
|---|---|
| Device triggers "button_1 … button_13, short/long press" | presses on the Remote page, usable in the automation editor |
| `notify.ulanzi_d200_notification` | `notify.send_message` shows the message on the status key |
| `select.ulanzi_d200_profile` | current profile; selecting one switches the deck |
| `number.ulanzi_d200_brightness` | backlight 0–255 |
| `button.ulanzi_d200_show_remote_page` | jumps to the Remote page |
| `binary_sensor.ulanzi_d200_connected`, `sensor.ulanzi_d200_computer` | whether a computer has the D200, and which one |

The entity ids above are what HA usually generates. Check yours in the device page.

## Entities on any key

Home Assistant describes a key by id, and you put the app **Home Assistant**
(`com.deckling.ha:entity`) on any key and pick the id. Home Assistant decides what the key shows;
the plugin turns it into signals and picks an icon. Choose your own icon in the configurator to
override it.

Publish retained to `d200/ha/<id>/set` (id: letters, digits, `_`, `-`, `.`):

```json
{"label": "Kitchen", "kind": "light", "on": true, "color": "#FFCC66", "value": "on",
 "background": "#1F2937", "progress": 40, "icon": "light"}
```

| Field | Meaning |
|---|---|
| `label` | text on the key |
| `kind` | picks an icon of the plugin's set: `light`, `switch`, `sensor`, `climate`, `lock`, `unlock`, `cover`, `scene`, `vacuum`, `automation`, `presence`, `energy` |
| `icon` | overrides `kind`: a name of the plugin's set, a Night Signal name, a full icon id, or an emoji |
| `on` | on/off state, for icons with the `toggle` role (a light's glass) |
| `color` | the entity's colour, for icons with the `rgb` role |
| `value` | shown as a number or a pill |
| `progress` | 0-100, for icons with the `progress` role |
| `background` | the key's background colour |

An empty payload removes the id. A press sends `press` to `d200/ha/<id>/action`, a long press
`long_press` (the app's preset actions; you can change them on the key).

Each field is also a signal, e.g. `com.deckling.ha:light.kitchen.on`, so themes and icons on
other keys can read it.

## Topics

`d200` is the default base topic.

| Topic | Direction | Payload |
|---|---|---|
| `d200/notify` | to deck | text; or `{title, text, icon, color, duration, priority}`; or a full card `{kind, source, title, detail, note, icon, color, progress, url, critical, group}` (`kind`: push, email, meeting, printer, steam, media) |
| `d200/printer` | to deck | `{state, percent, remaining_min, name, job, spools, ams}`; `state`: heating, printing, paused, complete, error, offline; `spools`: `[{"color": "#69E0C1", "progress": 80}, …]` shows the AMS (`ams`: `"ams"` or `"ams-lite"`) |
| `d200/ha/<id>/set` | to deck, **retained** | an entity, see above; empty payload removes it |
| `d200/ha/<id>/action` | from deck | `press`, `long_press` |
| `d200/remote/<0-12>/set` | to deck, **retained** | the Remote page: the same fields as `ha/<id>/set` for id `remote-<k>`. The older fields still work: `bg` is `background`, `icon_state` gives `on`, `color`, `progress`, and `temperature` as `value`. `fg` is ignored (the theme colours the text) |
| `d200/remote/<0-12>/action` | from deck | `press`, `long_press`, `down`, `up` |
| `d200/remote/show` | to deck | anything shows the Remote page, `off` leaves it |
| `d200/key` | from deck | `{profile, page, key, event}` for every key on every page |
| `d200/profile`, `d200/profile/set` | from / to deck | profile name (`auto` = follow the active app again) |
| `d200/brightness`, `d200/brightness/set` | from / to deck | 0–255 |
| `d200/page` | from deck, retained | `{profile, page, remote}` |
| `d200/availability` | from deck, retained | `online <computer>` / `offline` |

The Remote page is reached from a key with the "Show Home Assistant page" action, a long press
on the status key (the default), or `d200/remote/show`.

## Examples

Show a light on Remote key 2, and keep it updated:

```yaml
automation:
  - alias: "D200: kitchen light key"
    triggers:
      - trigger: state
        entity_id: light.kitchen
      - trigger: homeassistant
        event: start
    actions:
      - action: mqtt.publish
        data:
          topic: d200/remote/2/set
          retain: true
          payload: >
            {"label": "Kitchen", "icon": "light-on",
             "value": "{{ states('light.kitchen') }}",
             "bg": "{{ '#ca8a04' if is_state('light.kitchen', 'on') else '#1f2937' }}"}
```

Toggle it on press, dim it on long press:

```yaml
  - alias: "D200: kitchen key pressed"
    triggers:
      - trigger: mqtt
        topic: d200/remote/2/action
    actions:
      - choose:
          - conditions: "{{ trigger.payload == 'press' }}"
            sequence:
              - action: light.toggle
                target: { entity_id: light.kitchen }
          - conditions: "{{ trigger.payload == 'long_press' }}"
            sequence:
              - action: light.turn_on
                target: { entity_id: light.kitchen }
                data: { brightness_pct: 20 }
```

(Or pick the device trigger "Ulanzi D200 · button_3 short press" in the automation editor.)

Animated doorbell notification:

```yaml
  - alias: "D200: doorbell"
    triggers:
      - trigger: state
        entity_id: binary_sensor.doorbell
        to: "on"
    actions:
      - action: mqtt.publish
        data:
          topic: d200/notify
          payload: '{"title": "Doorbell", "text": "Someone is at the front door", "icon": "notification", "color": "#f59e0b", "style": "flash", "duration": 8}'
```

A plain-text notification also works: `action: notify.send_message`, `target: entity_id:
notify.ulanzi_d200_notification`, `data: message: "Washing machine done"`.

## When the computer changes

Every computer runs D200 Deck with the same broker settings. The one that has the D200
publishes `online <name>`. When the KVM switches, the old one only publishes `offline` if
nobody else has taken over, and the new one picks up the retained remote keys within a few
seconds. While no computer has the D200, the Remote page isn't shown anywhere.

Printer card from the Home Assistant Bambu Lab integration (adjust the entity ids):

```yaml
  - alias: "D200: printer"
    triggers:
      - trigger: state
        entity_id: [sensor.x1c_print_status, sensor.x1c_print_progress]
    actions:
      - action: mqtt.publish
        data:
          topic: d200/printer
          retain: true
          payload: >
            {"state": "{{ {'running':'printing','pause':'paused','finish':'complete','failed':'error','prepare':'heating'}.get(states('sensor.x1c_print_status'), states('sensor.x1c_print_status')) }}",
             "percent": {{ states('sensor.x1c_print_progress') | float(0) }},
             "remaining_min": {{ states('sensor.x1c_remaining_time') | float(0) }},
             "name": "Bambu X1C", "job": "{{ states('sensor.x1c_task_name') }}"}
```

A critical alert that stays on the wide key until it's pressed:

```yaml
      - action: mqtt.publish
        data:
          topic: d200/notify
          payload: '{"kind": "push", "source": "Smoke alarm", "title": "Kitchen smoke detected", "icon": "warning", "critical": true}'
```

A light key that shows the real light colour:

```yaml
  - alias: "D200: desk light key"
    triggers:
      - trigger: state
        entity_id: light.desk
    actions:
      - action: mqtt.publish
        data:
          topic: d200/remote/3/set
          retain: true
          payload: >
            {"label": "Desk", "icon": "light",
             "icon_state": {"on": {{ is_state('light.desk', 'on') | tojson }},
                            "color": "{{ '#%02x%02x%02x' | format(*state_attr('light.desk', 'rgb_color') or [105, 224, 193]) }}"}}
```
