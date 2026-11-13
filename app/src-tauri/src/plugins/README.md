# Built-in plugins

The plugins that ship with D200 Deck. Each folder holds one plugin:

| Folder | Package id | What it does |
|---|---|---|
| [core/](core/) | `com.deckling.core` | system monitor, mute and volume, weather, Pomodoro, media, commands, open, shortcuts; Night Signal icons |
| [teams/](teams/) | `com.deckling.teams` | Microsoft Teams meeting controls, presence, mail and calendar |
| [jetbrains/](jetbrains/) | `com.deckling.jetbrains` | whether a JetBrains IDE is debugging |
| [ha/](ha/) | `com.deckling.ha` | Home Assistant over MQTT: entities on keys, the remote page, the printer |
| [bambu/](bambu/) | `com.deckling.bambu` | Bambu Lab printers from Home Assistant on keys |
| [web/](web/) | `com.deckling.web` | an HTTP API with a token for scripts and other tools |

`core` without a prefix is not a plugin: it is what is fixed in the app (keys, pages,
profiles, the clock signals, MQTT). It lives in `app/src-tauri/core/`.

## Layout of a plugin

```
<name>/
  mod.rs        the code: a type that implements `Plugin`
  *.rs          helpers only this plugin uses (protocols, process scans)
  package/      the package data, embedded by build.rs as plugins/<package id>/
    deckling.json   manifest: id, name, description, settings schema (`config`)
    signals.json    the signals it publishes, with their types
    actions.json    the actions it offers, with their parameter schema
    apps/*.json     the apps users put on keys (roles, default icon, bindings, presets)
    iconset/        an embedded icon set (optional)
  README.md
```

## The interface

[mod.rs](mod.rs) defines `Plugin` and `Host`. A plugin runs in the engine's thread:

- `configure` at start and after every config change, `tick` once a second;
- `mqtt_topics` / `on_mqtt` for MQTT through the app's connection;
- `on_action` for its actions, `options` for `select` fields with a `source`;
- `on_message` for results of its own background threads (`Host::sender`);
- `stop` when it is switched off: close connections, stop servers and threads;
- `status` for the Packages view.

Through `Host` it publishes signals in its own namespace, publishes MQTT, reports errors and
asks the core to run an action, show a notification or press a key (`Host::request`).

A plugin never draws, never sets a colour on a key and never listens to keys
(docs/deckling/architecture.md). Values go out as signals; apps, icons and themes read them
through bindings.

## Adding one

1. Make `<name>/mod.rs` with a type that implements `Plugin`, and `<name>/package/` with at
   least `deckling.json`.
2. Add `pub mod <name>;` to [mod.rs](mod.rs) and the plugin to the list in `Engine::spawn`
   (`engine.rs`).
3. Add a README here and a row to the table above.

`registry.rs` tests that every built-in package loads without warnings; `icons.rs` tests that
every icon draws.

More: [docs/packages.md](../../../../docs/packages.md), [docs/config.md](../../../../docs/config.md).
