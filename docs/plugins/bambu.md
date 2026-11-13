# Bambu Lab (`com.deckling.bambu`)

Bambu Lab printers on keys. The data comes from Home Assistant: its
[Bambu Lab integration](https://github.com/greghesp/ha-bambulab) talks to the printers, and the
D200 Deck integration passes them on to the deck.

## Setting it up

1. In Home Assistant, add your printers with the Bambu Lab integration, and set up the D200 Deck
   integration ([home-assistant.md](../home-assistant.md)). Nothing more is needed there: it
   finds the printers by itself and sends each one to the deck.
2. In D200 Deck, put the app **Bambu Lab printer** on a key.
3. Under the key's settings pick the **Printer** (the list comes from Home Assistant). **Drawn
   as** is the machine on the key: automatic by the model Home Assistant reports, or A1, A1 mini
   or P1S by hand. Other enclosed printers (P1P, X1C) are drawn as the P1S.

## The key

| View | Shows |
|---|---|
| Progress | the machine with its print growing and the nozzle in the loaded filament's colour, the state, the percentage (or Warmup, Setup, Done, Check, Offline), the time left, a thin progress line. The chamber light is drawn on or off |
| Temperatures | nozzle and heated bed, each with its target |
| Filament | up to four spools of the AMS with material, colour and what is left; without an AMS the external spool |

A **press** goes to the next view. A **long press** asks Home Assistant to toggle the chamber
light. Nothing on the key pauses or stops a print. Colours follow the theme: printing and heating
in the warning colour, a finished print in the accent, an error in danger, paused in info.

The states: `idle`, `heating`, `calibrating`, `printing`, `paused`, `complete`, `error`,
`offline`. Home Assistant derives them from the printer's print status and current stage.

## Action

| Action | Does |
|---|---|
| `light` | asks Home Assistant to toggle the chamber light of `device` |

## Signals

Per printer (`<id>` is its Home Assistant device id): `<id>.name`, `<id>.status`,
`<id>.progress`, `<id>.light`, and `<id>.state` with everything the key draws.

## Without the integration

Any tool can describe a printer: a retained message on `<base>/bambu/<id>/state`, or
`PUT /api/topics/bambu/<id>/state` over the [Web API](web.md):

```json
{"name": "P1S", "model": "p1s", "status": "printing", "progress": 34, "remainingMinutes": 96,
 "nozzle": 245, "nozzleTarget": 245, "bed": 70, "bedTarget": 70, "lightOn": true,
 "filamentColor": "#74B9FF",
 "spools": [{"material": "PLA", "remaining": 80, "color": "#FFFFFF"}]}
```

An empty payload removes the printer. A long press publishes `light` on
`<base>/bambu/<id>/action` (MQTT only).

The drawings are the Night Signal printer apps; `mod.rs` is the Rust port of their render
function. Code: [app/src-tauri/src/plugins/bambu](../../app/src-tauri/src/plugins/bambu/).
