# com.deckling.web

An HTTP API on the local network, so scripts, CI jobs and other tools can drive the deck
without MQTT.

| File | What |
|---|---|
| [mod.rs](mod.rs) | the plugin and the server: a thread accepts connections, checks the token and hands each request to the plugin in the engine's thread, which answers it |
| [package/](package/) | manifest with settings (server switch, port, token, permission for program actions, webhook), signals, the press action, the Web key app |

While it runs, the deck announces itself over mDNS (`_d200deck._tcp`).

Off until **Run the web server** is switched on, and only with a token
(`Authorization: Bearer <token>`, compared in constant time). Actions that start programs or
type (`command`, `open`, `keys`) are refused unless allowed in the settings.

**Endpoints:** `GET /api`, `GET /api/signals[/<name>]`, `PUT /api/signals/<name>`,
`GET /api/keys`, `PUT|DELETE /api/keys/<id>`, `POST /api/notify`,
`POST /api/actions/<action>`, `POST /api/keys/<0-13>/press`, `GET|POST|DELETE /api/subscriptions`, `GET|PUT /api/mqtt`,
`GET /api/openapi.json` (no token).

**App:** Web key: shows a key a script describes with `PUT /api/keys/<id>`. A press sets
`<id>.pressed` and is posted to the webhook, if one is set.

**Signals:** `<id>.*` as for Home Assistant keys, `<id>.pressed`, and any `<name>` a script
sets.

Reference with examples: [docs/plugins/web.md](../../../../../docs/plugins/web.md). Tests: unit tests in
mod.rs, `tests/e2e/test_web.py` against the real engine.
