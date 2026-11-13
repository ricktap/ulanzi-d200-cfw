# Bambu Lab (`com.deckling.bambu`)

Bambu Lab printers from Home Assistant on keys: the app **Bambu Lab printer** draws an A1, A1
mini or P1S with the print's progress, the temperatures and the filament.

| File | Holds |
|---|---|
| `mod.rs` | the plugin (messages on `bambu/<id>/state` become signals, the device list, the `light` action) and the key face: the Rust port of the Night Signal printer apps' render function |
| `machines/*.svgpart` | the three machines as SVG fragments; `machine_svg` resolves their classes to theme colours and moves gantry, bed and head |
| `package/` | manifest, the app, its signals and the action |

The Home Assistant side is `custom_components/d200_deck/bambu.py`. User documentation:
[docs/plugins/bambu.md](../../../../../docs/plugins/bambu.md).

`BAMBU_PNG_DIR=<dir> cargo test --lib bambu` writes the faces as PNGs.
