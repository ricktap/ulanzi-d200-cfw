# Web API (`com.deckling.web`)

The built-in plugin **Web API** (`com.deckling.web`) runs an HTTP server in D200 Deck, so
scripts, CI jobs and other tools on your network can drive the deck without MQTT.

## Switch it on

Packages → Web API → Settings:
1. **Generate** a token. Every request needs it.
2. Switch on **Run the web server** (port 8765 unless you change it).

The plugin's card shows the addresses it listens on. It listens on every network interface; the
first time, macOS may ask whether D200 Deck may accept incoming connections.

While the server runs, the deck announces itself on the network (mDNS, `_d200deck._tcp`), so
Home Assistant finds it by itself.

Keep the token secret. Generating a new one locks out every script that uses the old one.

## Requests

The full description is an OpenAPI 3.1 document at **`GET /api/openapi.json`**. It needs no
token (it holds no secrets), so tools such as Swagger UI, Postman or code generators can read
it directly, e.g. `http://studio.local:8765/api/openapi.json`.

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
| `POST /api/keys/<0-13>/press` | presses a key of the page on the device; `{"event": "long"}` for a long press (also `double`, `down`, `up`) |
| `GET /api/subscriptions` | webhooks registered over the API, with the seconds they have left |
| `POST /api/subscriptions` | registers or renews a webhook: `{"url": ..., "ttl": 900, "events": ["presses", "keys"]}` (10 to 3600 s, at most 16). `presses`: presses on Web keys (the default); `keys`: every key event on any key, posted as `{"type": "key", "key", "event", "profile", "page", "n"}` |
| `DELETE /api/subscriptions` | removes one: `{"url": ...}` |
| `GET /api/mqtt` | the deck's shared MQTT settings (the password only as `has_password`) |
| `PUT /api/mqtt` | sets them and connects: `{"host", "port", "username", "password", "tls", "base_topic"}`; saved to deck.json |
| `GET /api/openapi.json` | the OpenAPI description, **without a token** |

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

A press on that key sets the signal `com.deckling.web:<id>.pressed` (`press` or `long_press`) and
is posted as `{"id": "build", "event": "press"}` to the **Webhook** URL from the plugin's
settings and to every live subscription. A tool that wants presses registers its own URL with
`POST /api/subscriptions` and renews it before the `ttl` runs out; the Home Assistant
integration does this by itself.

## Signals for themes and icons

`PUT /api/signals/ci.status` with `"failed"` sets `com.deckling.web:ci.status`. A theme
variable or an icon binding can read it, for example an accent that turns coral when the
build fails:

```json
"accent": { "bind": "com.deckling.web:ci.status", "map": { "failed": "#FF8278", "passed": "#69E0C1" }, "fallback": "#69E0C1" }
```

## Tests

`tests/e2e/test_web.py` runs the server in the real engine against a simulated D200: token,
keys drawn on the device, signals, presses, actions, notifications, the OpenAPI document and a
subscription that receives a press. Code: [app/src-tauri/src/plugins/web](../../app/src-tauri/src/plugins/web/).
