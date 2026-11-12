# com.deckling.ha

Home Assistant over the app's MQTT connection (the connection itself is part of the core:
Packages, Deck, MQTT).

| File | What |
|---|---|
| [mod.rs](mod.rs) | the plugin: keys described by id on `<base>/ha/<id>/set`, the remote page (`remote/<k>/set`), the printer (`printer`); `describe` turns a key description into signals (also used by the web plugin) |
| [package/](package/) | manifest, signals, actions, the Home Assistant app and its own icon set (13 icons: light, switch, sensor, climate, lock, ...) |

**App:** Home Assistant (`entity`): put it on any key and pick an id. Home Assistant decides
label, icon (by `kind` or `icon`), value, on/off, colour, progress and background.

**Actions:** `press` (sends `press` or `long_press` to `<base>/ha/<id>/action`), `mqtt`
(publish anything), `remote` (show the remote page).

**Signals:** `<id>.label`, `.value`, `.on`, `.color`, `.background`, `.progress`, `.kind`,
`.icon`, `.present`; `printer.*` for the printer card.

The Home Assistant side is the integration in
[custom_components/d200_deck](../../../../../custom_components/d200_deck): pick entities there
and they appear here as ids, kept up to date (over the Web API they use the app Web key
instead). Topics and examples:
[docs/home-assistant.md](../../../../../docs/home-assistant.md).
