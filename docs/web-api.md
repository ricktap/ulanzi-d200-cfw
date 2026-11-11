# Web API

The built-in plugin **Web API** (`com.deckling.web`) runs an HTTP server in D200 Deck, so
scripts, CI jobs and other tools on your network can drive the deck without MQTT.

## Switch it on

Packages → Web API → Settings:
1. **Generate** a token. Every request needs it.
2. Switch on **Run the web server** (port 8765 unless you change it).

The plugin's card shows the addresses it listens on. It listens on every network interface; the
first time, macOS may ask whether D200 Deck may accept incoming connections.

Keep the token secret. Generating a new one locks out every script that uses the old one.

## Requests

Send the token as `Authorization: Bearer <token>` (or `?token=<token>`). Bodies are JSON (up to
64 KB). Answers are JSON; errors have `{"error": "..."}` and a status code (401 for a missing or
wrong token).

| Method and path | Does |
|---|---|
| `GET /api` | name, version and the endpoints |
| `GET /api/signals` | every signal with its value |
| `GET /api/signals/<name>` | one signal, e.g. `com.deckling.core:system.cpu` |
| `PUT /api/signals/<name>` | sets the signal `com.deckling.web:<name>`; body: any JSON value, or `{"value": ...}` |
| `GET /api/keys` | the key ids set over HTTP, with their labels |
| `PUT /api/keys/<id>` | describes a key (same fields as Home Assistant's `ha/<id>/set`: `label`, `kind`, `icon`, `value`, `on`, `color`, `progress`, `background`) |
| `DELETE /api/keys/<id>` | removes it |
| `POST /api/notify` | a notification on the wide key: text, or JSON like MQTT `notify` (`{title, text, icon, color, duration, priority}` or a full card) |
| `POST /api/actions/<action>` | runs an action with `{"params": {...}}`, e.g. `core:page`, `core:profile`, `com.deckling.core:media` |
| `POST /api/keys/<0-13>/press` | presses a key of the page on the device; `{"event": "long"}` for a long press (also `down`, `up`) |

Ids and signal names are letters, digits, `_`, `-` and `.`.

**Programs:** the actions `com.deckling.core:command`, `com.deckling.core:open` and
`com.deckling.core:keys` start programs or type on the computer. They are refused (403) unless
you switch on **Allow actions that start programs or type**.

## Keys from a script

1. Describe the key:

   ```sh
   curl -X PUT -H "Authorization: Bearer $TOKEN" \
     -d '{"label": "Build", "kind": "sensor", "value": "green", "on": true}' \
     http://studio.local:8765/api/keys/build
   ```

2. In the configurator, put the app **Web key** on a key and pick the id `build`.

The key updates whenever the script sends a new description.

A press on that key sets the signal `com.deckling.web:<id>.pressed` (`press` or `long_press`). If
the plugin has a **Webhook** URL, the press is also posted there as
`{"id": "build", "event": "press"}`.

## Signals for themes and icons

`PUT /api/signals/ci.status` with `"failed"` sets `com.deckling.web:ci.status`. A theme
variable or an icon binding can read it, for example an accent that turns coral when the
build fails:

```json
"accent": { "bind": "com.deckling.web:ci.status", "map": { "failed": "#FF8278", "passed": "#69E0C1" }, "fallback": "#69E0C1" }
```

## Tests

`tests/e2e/test_web.py` runs the server in the real engine against a simulated D200: token,
keys drawn on the device, signals, presses, actions and notifications.
