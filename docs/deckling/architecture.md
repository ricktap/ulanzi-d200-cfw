# Deckling: Architecture and design

As of: 10 November 2026. Repo: https://github.com/ricktap/ulanzi-d200-cfw

This document is the normative description of the target model. Running examples are in `developer-guide.html`, the operating concept in `briefing.md` and in the click dummy under `click-dummy/`.

## 1. Summary

Plugins produce values and offer actions. Apps translate these values into interfaces. Icons read only interfaces, signals and theme variables. Themes set variables and may bind them to signals. Per key, the user picks an app and an icon, and sets one full theme with any number of overlays.

The core draws every key itself. Plugins never deliver pixels and never set colours.

Diagrams: `diagrams/packages-and-core.svg` and `diagrams/signal-to-pixel.svg`.

## 2. Terms

| Term | Meaning | Belongs to |
| --- | --- | --- |
| Signal | Named value over time, for example `com.ricktap.hue:lamp.color` | Plugin or core |
| Action | Something a plugin can do, for example `com.ricktap.hue:lamp.toggle` | Plugin or core |
| Interface (role) | List of named, typed properties, without data | Core (primitives) or plugin |
| App | Fulfils interfaces by mapping signals and settings to their properties. Brings a default icon and preset actions | Plugin |
| Icon | SVG with named parts plus a binding file. Names the roles it is drawn for | Icon set |
| Theme variable | Named value for the look, fixed or bound to a signal | Theme |
| Setting | Parameter of an app per key | App, stored on the key |

The words role and interface mean the same thing. Roles are interfaces, signals are signals. A signal never carries the name of a role.

## 3. Packages

There are exactly three kinds. Each package is a folder with a `deckling.json`.

| Package | Provides | Code |
| --- | --- | --- |
| Plugin | Signals, actions, interfaces, apps, embedded icon set | yes |
| Icon set | Icons with bindings | no |
| Theme | Variables, type `full` or `partial` | no |

A plugin may embed an icon set (folder `iconset/`). It has exactly the same format as a standalone icon set and provides the default icon of each app. The core is itself a plugin with the ID `core`. Night Signal is its embedded icon set.

Themes and icon sets are independent of each other. A theme refers to no icon set and embeds none.

## 4. Namespaces and permissions

The package ID in reverse domain notation is the namespace. The separator between package and name is the colon, inside the name the dot: `com.ricktap.bambu:ams.casing`. Names without a prefix belong to the core (`accent`, `time.second`, `toggle`).

| Package | May define | May read |
| --- | --- | --- |
| Plugin | Signals, actions, interfaces, apps and icons, only in its own namespace | its own settings |
| Icon set | Icons in its own namespace | Interfaces, signals and theme variables from all namespaces |
| Theme | Variables in all namespaces | Signals from all namespaces |

Rules:

1. Plugins do not set colours, not even at runtime. There are no calls like `set_mood` or `push_colors`.
2. Plugins do not listen to keys. The core detects the key press and calls the actions entered on the key.
3. A theme variable in a foreign namespace needs no permission from the plugin. It is a key that a theme sets and an icon reads.
4. Two installed packages with the same ID are rejected. The notation prevents collisions but does not prove origin.

## 5. Plugins

### 5.1 Manifest

```json
{
  "id": "com.ricktap.hue",
  "type": "plugin",
  "name": "Philips Hue",
  "version": "1.0.0",
  "runtime": { "kind": "process", "main": "plugin.js" },
  "config": [
    { "key": "bridge", "label": "Bridge", "type": "select", "values": { "source": "com.ricktap.hue:bridges" } }
  ],
  "contributes": {
    "signals": "signals.json",
    "actions": "actions.json",
    "interfaces": "interfaces.json",
    "apps": "apps/",
    "iconset": "iconset/"
  }
}
```

`runtime.kind` is `builtin` (Rust module in the core, for Core, Teams, JetBrains, Home Assistant) or `process` (own process, JSON RPC over stdio, later).

### 5.2 Settings of a plugin

`config` in the manifest describes the settings of the plugin with the same schema as the settings of an app (section 5.6). The configurator builds the fields in the Packages view from it. Types: `text`, `toggle`, `select`, `number`. A field with `"secret": true` is stored in the keychain.

The settings of the plugin `core` are the global settings of the deck: threshold for a long press, whether profiles follow the program in the foreground, screensaver. So there is no separate options menu. Outside the packages only autostart, export and import remain.

A plugin with a connection also reports a state (connected, setup needed, error) and may offer a guided setup in steps. There is no separate view for integrations. Integration is just another word for a plugin with a connection.

### 5.3 Interface to the core

| Call | Purpose |
| --- | --- |
| `publish(name, value)` | publishes a signal in its own namespace |
| `onAction(name, fn)` | receives an action |
| `provideOptions(name, fn)` | provides values for a select field at runtime |

The core tells a plugin which of its signals are currently shown. A plugin may leave the rest idle.

### 5.4 Signals are typed

Every signal is registered with a type. Tools and the configurator then know how to show a value.

```json
[
  { "name": "com.ricktap.hue:lamp.power", "type": "flag" },
  { "name": "com.ricktap.hue:lamp.color", "type": "color" },
  { "name": "com.ricktap.hue:lamp.brightness", "type": "percent" }
]
```

Types: `flag`, `percent`, `number`, `color`, `text`. Later `image`. The same types apply to the properties of interfaces.

### 5.5 Interfaces

Primitives of the core. The list should stay small; it is the most public contract in the system.

```json
{
  "button": {},
  "toggle": { "on": "flag" },
  "progress": { "value": "percent" },
  "rgb": { "color": "color" },
  "clock": { "time": "text", "hour_angle": "number", "minute_angle": "number", "style": "text" }
}
```

A plugin defines its own interfaces in its own namespace, for example `com.ricktap.hue:rgb_light { on, color, brightness }` or `com.ricktap.bambu:ams { slot1_remaining, slot1_color, ... }`. A renamed field breaks foreign icons. So three rules apply:

1. A published interface does not change any more. The only thing allowed is adding properties. Icons must handle missing values anyway.
2. Renaming, removing or a different type is a new interface with a version in the name: `com.ricktap.hue:rgb_light@2`. Without a suffix, version 1 is meant.
3. An app may fulfil the old and the new version at the same time. That is just another block in `implements` and costs no code. Old icons keep working as long as the plugin keeps the old version.

An icon set includes a copy of every foreign interface it was drawn against (folder `interfaces/`). On load, the core compares the copy with the installed interface and reports in the configurator when an icon was built for a different version. If an icon reads a property that no longer exists, the `fallback` applies. The icon is then poorer, but not broken.

These rules are decided, but not yet tried on any example.

### 5.6 Apps

An app is the translation of signals and settings into properties.

```json
{
  "id": "com.ricktap.bambu:ams",
  "name": "AMS",
  "icon": "com.ricktap.bambu:rings",
  "config": [
    { "key": "unit", "label": "Device", "type": "select", "default": "ams1",
      "values": { "source": "com.ricktap.bambu:units" } }
  ],
  "implements": {
    "com.ricktap.bambu:ams": {
      "slot1_remaining": { "bind": "com.ricktap.bambu:{unit}.slot1.remaining", "fallback": 0 },
      "slot1_color": { "bind": "com.ricktap.bambu:{unit}.slot1.color", "fallback": "#87909E" }
    }
  },
  "on": { "press": [], "long": [] }
}
```

Rules:

1. `implements` names a source per interface and property: `bind` (signal) or `config` (setting), optionally with `steps`, `map`, `fallback`.
2. An app may fulfil several interfaces, the richest first. Every app fulfils `button` by itself.
3. `config` describes the settings. The configurator builds the fields from this schema. Type `select` has `values` as a fixed list `[{label, value}]` or as `{ "source": "<name>" }`; then the plugin provides the values through `provideOptions`.
4. Placeholders in `bind` such as `{unit}` are filled from the settings of the key. Several devices are the same app with a different setting.
5. `on` is a preset of the actions. On the key they can be changed freely afterwards.
6. `icon` names the default icon from the embedded icon set.

Example of a setting with fixed values (clock):

```json
"config": [
  { "key": "display", "label": "Display", "type": "select", "default": "analog",
    "values": [ { "label": "Analog", "value": "analog" }, { "label": "Digital", "value": "digital" } ] }
],
"implements": { "clock": { "style": { "config": "display" } } }
```

## 6. Binding language

Applies the same way in apps, icons and themes. It is small on purpose. Anyone who needs conditions or formulas computes in the plugin and publishes the result as a signal.

| Key | Effect | Allowed in |
| --- | --- | --- |
| `bind` | reads a signal, placeholders from the settings | App, icon, theme |
| `config` | reads a setting of the key | App |
| `prop` | reads a property of an interface, notation `<interface>.<property>` | Icon |
| `tone` | reads a theme variable | Icon |
| `steps` | picks from a list by a number, cyclically (value modulo length) | everywhere |
| `map` | picks from a table by a named value | everywhere |
| `prefix`, `suffix` | puts text before and after the value | everywhere |
| `fallback` | replacement when the value is missing | everywhere |

Resolution in this order: read the source. If the value is missing, resolve `fallback` and return it. Otherwise apply `steps`, then `map`, then `prefix` and `suffix`. Values in `steps`, `fallback` and theme variables may themselves be bindings.

`fallback` is required for `prop`, for `bind` and for `tone` on variables outside the core variables. The core checks this on load.

Signals of the core: `time.clock`, `time.second`, `time.tick`, `time.hour_angle`, `time.minute_angle`. Planned: `key.index`, `key.column`, `key.row` (different per key) and `key.press`.

## 7. Icons

An icon consists of an SVG and a binding file.

```json
{
  "id": "com.jemand.neon:bulb",
  "roles": ["rgb", "toggle", "button"],
  "file": "bulb.svg",
  "parts": {
    "outline": { "stroke": { "tone": "base" } },
    "glass": {
      "visible": { "prop": "toggle.on", "fallback": false },
      "fill": { "prop": "rgb.color", "fallback": "#FFD84D" }
    }
  }
}
```

Contract for the SVG:

1. Parts are named through `data-part`. An element may carry several names.
2. No fixed colours. Colours come from `tone`, `prop` or `bind`.
3. `progress` fills a line proportionally from 0 to 100 (`pathLength="100"`). `visible` shows or hides. `text` sets the content of a text element. Any other key is set as an SVG attribute.
4. An icon never names a signal of the app. The link to the app goes through `prop`.
5. PNG is allowed, but follows no theme and no signal.

Selection instead of resolution:

1. Icons are not stacked. There is no active icon set and no order.
2. The user picks one icon per key. Offered is every icon that shares at least one role with the app, sorted by the richest shared role in the app's order.
3. The default is the icon from the plugin's embedded set (`app.icon`).
4. Applying a whole set is a one-time action that writes the selection into the keys where an icon shares more than `button`.
5. If the selected icon is missing, the default icon applies.

### 7.1 Tool for icon developers

`icon-workbench.html` loads the binding file and SVG and checks the icon against an interface. The sliders are built from the types of the properties. Each value can be switched off to see the `fallback`. Plus a preview per role and a check report: missing parts in the SVG, read properties the interface does not know, missing `fallback`, fixed colours, `progress` without `pathLength`. A plugin's own interfaces are loaded as a file.

The core should run the same checks when it loads an icon set.

## 8. Themes

```json
{
  "id": "com.jemand.bernstein",
  "type": "theme",
  "name": "Bernstein",
  "theme": {
    "type": "full",
    "variables": {
      "background": "#211A14",
      "base": "#FFE6C2",
      "muted": "#5A4A3A",
      "accent": "#FF9F43",
      "com.ricktap.bambu:ams.led": { "bind": "time.second", "steps": ["#FF4D4D", "#3DDC84"], "fallback": "#FF4D4D" }
    }
  }
}
```

1. Core variables: `background`, `base`, `muted`, `accent`. A theme of type `full` sets all four. Exactly one is active.
2. A theme of type `partial` sets single variables. Any number are active as overlays. There is deliberately no `extends`, so that no theme requires another.
3. If two overlays set the same variable, the later one in the list wins. The user can change the order.
4. The value of a variable is fixed or a binding to a signal. This makes rainbow, Hue Sync and blinking indicators themes of type `partial`.
5. The icon of a theme in the configurator is generated from its core variables. A custom image is optional.

## 9. Core, rendering and performance

Building blocks: signal bus (last value per signal), registry (interfaces, apps, actions, icons, themes from all packages), engine (keys, profiles, pages, actions), renderer (resolves bindings, sets values into the SVG, renders with `resvg`), device connection.

1. The renderer redraws a key only when a value it reads has changed. For this it remembers, per key, the signals and variables read.
2. Every image goes to the device separately over USB. The screensaver runs at 8 frames per second today. Whether animated themes look smooth on all 14 keys is settled by a throwaway test before implementation.
3. Measures: prerender icons per tone as a mask and only recomposite on a colour change. Send a key only when its colour has visibly changed. Configurable upper limit for frames per second.
4. If that is not enough, `d200fw` colours on the device itself and only receives palettes. This changes nothing in the model.

## 10. deck.json

```json
{
  "version": 2,
  "theme": "com.jemand.bernstein",
  "overlays": ["com.jemand.ams-blau", "com.ricktap.hue-sync"],
  "plugins": {
    "core": { "config": { "hold_ms": 500, "follow_front_app": true } },
    "com.ricktap.hue": { "enabled": true, "config": { "bridge": "wohnzimmer" } }
  },
  "profiles": [
    {
      "name": "Default",
      "apps": [],
      "pages": {
        "main": {
          "6": {
            "app": "com.ricktap.bambu:ams",
            "icon": "com.jemand.neon:spools",
            "label": "AMS 2",
            "config": { "unit": "ams2" },
            "on": { "press": [], "long": [{ "action": "core:page", "params": { "page": "Druck" } }] }
          }
        }
      }
    }
  ]
}
```

Profiles: the first profile is the default. `apps` names the programs a profile is bound to. A program belongs to at most one profile. When it comes to the front and the core plugin is set up for it, the deck switches to this profile, otherwise to the default. `pages` is ordered; the first page is the start page.

1. Old files (version 1) are migrated on load; the original stays as `deck.v1.json`.
2. Unknown apps, icons and fields are kept on save. A key with a missing plugin is shown as a placeholder.
3. Secrets (MQTT password, tokens) belong in the system keychain.

## 11. Split of the existing code

| Package | Content | Modules today |
| --- | --- | --- |
| `core` | Hotkey, command, open, volume, brightness, pages, profiles, clock, system monitor, Pomodoro, weather, media. Night Signal as embedded icon set | `engine.rs`, `render.rs`, `apps.rs`, `platform.rs`, `media.rs`, `icons.rs` |
| Microsoft 365 | Meeting actions, microphone, camera, presence. Mail and calendar | `teams.rs` |
| JetBrains | Debugger | `ide.rs` |
| Home Assistant | MQTT send, remote keys, printer | `mqtt.rs` |

The eight existing stateful icons already work through `data-part` and states. They are the natural first test for the binding language.

## 12. Implementation order

1. Foundation: version field and migration in `config.rs`, pass unknown fields through. Split `main.js` into ES modules, central store with undo.
2. Signal bus and resolver for `bind`, `prop`, `tone`, `config`, `steps`, `map`, `fallback`. Switch the stateful icons to it.
3. Registry and core plugin: interfaces, apps and actions as data. `apps.rs` publishes signals instead of drawing.
4. Teams, JetBrains and Home Assistant on the same interface, still in the same process.
5. New configurator.
6. Load icon sets and themes from disk, overlays with order.
7. Third-party plugins as a separate process.

## 13. Open decisions

1. Name: the repo is called Deckling, the app is called D200 Deck. Decide the core namespace and own package IDs.
2. Smooth transitions: `steps` jumps. A flowing rainbow needs crossfading or a core signal that is already a colour.
3. Values per key: `key.index`, `key.column`, `key.row` are signals with context.
4. Inheritance of interfaces: would save duplicate mappings, costs a concept. Only when needed.
4a. Versions of interfaces: the `@2` notation and the copy included in the icon set are decided, but untried.
5. Several states per key: in the new model probably a core app that fulfils `toggle`.
6. Wide key and cards: the obvious choice is an app for the wide key with a signal as trigger.
7. Long texts and images: rules for truncation and for signals of type image.
8. Runtime for third-party plugins: process with stdio or WASM.
9. Licence: must be settled before opening up.

## 14. Rejected and why

| Idea | Why rejected |
| --- | --- |
| Plugin sends a key image as data (`set_face`) | Icons could then not come from third parties and would follow no theme |
| Plugin sets colours (`set_mood`, `set_effect`, `push_colors`) | violates the namespace permissions. Effects are themes with bindings |
| Semantic icon roles (`role:mic.on`) with a resolution chain | hidden logic. Roles are now interfaces, the user chooses |
| Stackable icon sets | the same hidden logic. Replaced by selection and one-time applying |
| Theme suggests an icon set | unnecessary dependency between packages |
| `extends` for themes | forces installing both themes. Overlays leave the choice to the user |
| Widget as its own return channel | The key press is the core's job. An app only presets actions |
| Separate Integrations view and an options menu | Integration is a plugin with a connection. Settings are on the plugin, global ones on the plugin `core` |
| Signals named after roles (`toggle.on` as a signal) | Roles are interfaces, signals are signals. The app translates |
