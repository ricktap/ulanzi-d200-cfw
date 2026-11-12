# Home Assistant

MQTT is part of the app: D200 Deck connects to your MQTT broker (Packages → Deck → MQTT) and
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

## The integration (recommended)

`custom_components/d200_deck` is the counterpart in Home Assistant. It reaches D200 Deck over
**MQTT** (the MQTT integration on the same broker as the deck), over the deck's **Web API**
([plugins/web.md](plugins/web.md): switch on the server and copy the token), or **both**.

**Install:**
- With HACS: [open this repository in HACS](https://my.home-assistant.io/redirect/hacs_repository/?owner=ricktap&repository=ulanzi-d200-cfw&category=integration) (My Home Assistant link; it adds the
  repository as a custom repository), install **D200 Deck**, restart Home Assistant.
- By hand: copy `custom_components/d200_deck` into the `custom_components` folder of your
  Home Assistant configuration, then restart.

**Found by itself:** while the deck's Web API runs (Packages, Web API, Run the web server), the
deck announces itself on the network. Home Assistant then shows **D200 Deck discovered** under
Settings, Devices & services. Click Configure and enter the token. If Home Assistant has the MQTT
integration, it also offers to **set up the deck's MQTT connection with Home Assistant's
broker**: it sends the broker's address and login to the deck over the Web API, the deck saves
them (Packages, Deck, MQTT) and connects, and the integration uses MQTT and the Web API. With the
Mosquitto add-on, the deck gets Home Assistant's own address instead of `core-mosquitto`.

**Set up by hand:** [add the integration](https://my.home-assistant.io/redirect/config_flow_start/?domain=d200_deck) (or Settings, Devices & services, Add integration,
**D200 Deck**):
1. Choose the connection: MQTT, Web API, or both.
2. MQTT: the base topic (`d200` unless you changed it in D200 Deck).
3. Web API: the computer running D200 Deck (name or address), the port (8765) and the token. The
   integration checks them right away and, as above, can hand Home Assistant's broker to the
   deck.
4. Pick the entities to put on the deck. Change the list later under the integration's
   Configure.

With the Web API, presses come back to a Home Assistant webhook. The integration registers it
with the deck (`POST /api/subscriptions`) and renews it every 5 minutes, sending every key
again at the same time, so a deck that restarted is complete within minutes. The deck must be
able to reach Home Assistant's internal URL (Settings, System, Network).

What it does:
- **Every chosen entity becomes a key id** (its entity id, e.g. `light.kitchen`). In D200 Deck,
  put the app **Home Assistant** (MQTT) or **Web key** (Web API) on a key and pick the id. The key follows the entity: label,
  icon by domain, on/off, a light's colour and brightness, a sensor's value, a cover's position,
  a thermostat's temperature. Taking an entity off the list removes it from the deck.
- **A press runs the entity's default action:** toggle (lights, switches, fans, covers, media
  players, automations, climate), turn on (scenes, scripts), press (buttons), lock or unlock,
  start or send home (vacuums). Switch this off under Configure to only use automations.
- **Every press fires the event `d200_deck_press`** with `id`, `entity_id`, `event`
  (`press` or `long_press`) and `base_topic`, for your own automations.
- **When a computer takes the D200** (`online` on `d200/availability`), every key is sent again.

**One device per deck, "Ulanzi D200"** (Settings, Devices). Over MQTT the deck announces it
itself (brightness, profile, notifications, connected, computer, remote page); the integration
joins it and adds:

| Entity | Shows |
|---|---|
| `event.ulanzi_d200_key_event` | every press, long press and double press on **any** key, with the attributes `key` (0 to 13), `profile` and `page` |
| `sensor.ulanzi_d200_page` | the page the D200 shows; `remote` on the remote page |

Over the Web API alone there is no MQTT device, so the integration makes "Ulanzi D200" itself,
with **Connected** (the deck answers, checked every minute), **Computer** and **Profile** as
well. (HACS lists one more device, "D200 Deck" from HACS: that is HACS's record of the download,
not a deck.)

The key events show up in the logbook and history, and the automation editor offers them as a
trigger (Entity, State or the device's triggers). Over MQTT they come from `d200/key`; over the
Web API only, the deck posts them to the webhook (`"events": ["presses", "keys"]`).

Any key, not only Home Assistant keys: key 4 on the page Media pressed twice:

```yaml
automation:
  - alias: "D200: key 4 double press"
    triggers:
      - trigger: state
        entity_id: event.ulanzi_d200_key_event
        not_to: unavailable
    conditions:
      - condition: template
        value_template: >
          {{ trigger.to_state.attributes.event_type == 'double_press'
             and trigger.to_state.attributes.key == 4
             and trigger.to_state.attributes.page == 'Media' }}
    actions:
      - action: scene.turn_on
        target: { entity_id: scene.movie }
```

Services:

| Service | Does |
|---|---|
| `d200_deck.notify` | a notification on the wide key: `message`, `title`, `icon`, `color`, `duration`, `critical` |
| `d200_deck.set_key` | a key by id without an entity: `key_id`, `label`, `kind`, `icon`, `value`, `on`, `color`, `progress`, `background` |
| `d200_deck.clear_key` | removes a key id |
| `d200_deck.show_remote_page` | shows (`show: true`) or leaves the remote page |

All take `base_topic` to address one deck when there are several: its MQTT base topic or its
Web API host.

A long press dims the kitchen light:

```yaml
automation:
  - alias: "D200: kitchen long press"
    triggers:
      - trigger: event
        event_type: d200_deck_press
        event_data: { id: light.kitchen, event: long_press }
    actions:
      - action: light.turn_on
        target: { entity_id: light.kitchen }
        data: { brightness_pct: 20 }
```

A doorbell key without an entity:

```yaml
      - action: d200_deck.set_key
        data: { key_id: doorbell, label: Door, kind: presence, value: "ringing", on: true }
```

Tests: `tests/ha/run.sh` (needs `pip install -r tests/ha/requirements.txt`) runs the integration
in Home Assistant's test harness.

The sections below describe the MQTT topics themselves, for setups without the integration.

## A broker of its own

By default every plugin uses the deck's one MQTT connection (Packages, Deck, MQTT). The Home
Assistant plugin can use a broker of its own instead: Packages, Home Assistant, Settings, **Own
MQTT broker**. Its keys, the remote page, the printer and its `mqtt` action then go to that
broker; key events, discovery and notifications stay on the deck's connection.

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
