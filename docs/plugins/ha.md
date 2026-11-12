# Home Assistant (`com.deckling.ha`)

Home Assistant entities on any key, over the app's MQTT connection (Packages, Deck, MQTT).

## App

**Home Assistant**: put it on a key and pick an id. Home Assistant decides what the key shows:
label, icon (by kind, from the plugin's own icon set: light, switch, sensor, climate, lock,
cover, scene, vacuum, automation, presence, energy), value, on/off, colour, progress and
background. Choose another icon on the key to override it.

## Settings

**Own MQTT broker** (off by default): host, port, user, password, base topic, TLS. Off, the
plugin uses the deck's shared connection (Packages, Deck, MQTT).

## Actions

`press` (sends `press` or `long_press` back to Home Assistant), `mqtt` (publish any topic),
`remote` (show the remote page: 13 keys Home Assistant sets directly).

## The Home Assistant side

Install the integration **D200 Deck** in Home Assistant
([custom_components/d200_deck](../../custom_components/d200_deck/), also through HACS). It
connects over MQTT, the [Web API](web.md), or both, sends the entities you pick and runs their
default action on a press. With the Web API, use the app **Web key** on the deck instead.

Everything about it, the MQTT topics and examples: [home-assistant.md](../home-assistant.md).

## Signals

`<id>.label`, `.value`, `.on`, `.color`, `.background`, `.progress`, `.kind`, `.icon`,
`.present` for every id; `printer.*` for the printer card.

Code: [app/src-tauri/src/plugins/ha](../../app/src-tauri/src/plugins/ha/).
