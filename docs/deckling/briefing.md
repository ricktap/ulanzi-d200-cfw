# Briefing for the AI: rebuilding Deckling

You are working on the repo https://github.com/ricktap/ulanzi-d200-cfw (Deckling, custom firmware and desktop app for the Ulanzi D200). This briefing gives you the task, the rules and the design of the UI.

## 1. What you read first

| File | Why |
| --- | --- |
| `architecture.md` | The target model. Normative. In case of conflict this file wins |
| `developer-guide.html` | Four running examples (Hue, clock, AMS, music). The code in the lower part of the file is a complete, small reference for the resolver and the package formats |
| `click-dummy/logic.js` | Behaviour of the configurator as readable code |
| `click-dummy/Main.dc.html` | Layout and look of the configurator |
| `icon-workbench.html` | Working tool for icon developers. Its check report is the template for the checks the core runs when it loads an icon set |
| `diagrams/*.svg` | Two diagrams: packages and core, path from signal to pixel |

In the repo itself: `CLAUDE.md`, `docs/architecture.md`, `docs/config.md`, `docs/app.md`, `docs/design/`.

## 2. The state of the repo

- `device/`: `d200fw`, Rust without dependencies, draws what it gets and reports keys. Stays unchanged.
- `app/src-tauri/src/`: engine (`engine.rs`), renderer (`render.rs`, tiny-skia, resvg), built-in widgets (`apps.rs`), cards (`cards.rs`), icons (`icons.rs`, compiled in by `build.rs`), integrations (`teams.rs`, `mqtt.rs`, `media.rs`, `ide.rs`), config (`config.rs`).
- `app/ui/`: plain HTML and JS without a bundler. `main.js` has about 1350 lines and re-renders everything. Actions and apps are fixed constants `ACTIONS` and `APPS` there.
- `app/ui/icons/night-signal/`: 218 SVG icons, eight of them with states through `data-part` (`STATEFUL.md`).
- Tests: `cargo test --lib` and `tests/e2e/run.sh` against a simulated device.

## 3. The target model in thirteen rules

1. Three package kinds: plugin (with code), icon set (data), theme (data). Plugins may embed an icon set.
2. Namespaces in reverse domain notation, `package:name`. Names without a prefix belong to the core.
3. Plugins define signals, actions, interfaces, apps and icons only in their own namespace.
4. Themes define variables in all namespaces. Icons read interfaces, signals and variables from all namespaces.
5. Roles are interfaces. Signals are signals. The app translates signals and settings into properties of an interface.
6. An icon reads the app only through `prop`. It never names a signal of the app.
7. Every binding with `prop`, `bind` or a variable outside the core variables has a `fallback`.
8. Icons are selected, not resolved. No stacking, no active icon set.
9. Exactly one theme of type `full`, plus any number of type `partial` in an order. No `extends`.
10. Plugins do not set colours and do not listen to keys. The core draws and detects key presses.
11. Signals and properties of interfaces are typed: `flag`, `percent`, `number`, `color`, `text`.
12. A published interface does not change. Breaking changes are a new version in the name (`@2`). Apps may fulfil several versions at the same time.
13. Settings are on the plugin and on the app, each as a schema in `config`. The settings of the plugin `core` are the global ones. There is no Integrations view and no options menu.

## 4. What you should not do

- The archive of superseded example pages was not imported.
- Do not introduce calls like `set_face`, `set_mood`, `set_effect` or `push_colors`. They come from a rejected draft.
- Do not build a resolution chain for icons and no semantic icon roles like `role:mic.on`.
- Do not extend the binding language with conditions or formulas. Computing happens in the plugin.
- No custom HTML from plugins in the configurator. Fields are built from the schema.
- Do not change `d200fw` and the protocol unless the user asks for it.
- Do not introduce a bundler or TypeScript in `app/ui/` without asking. ES modules without a build are fine.

## 5. Work packages

Each package should work on its own and pass the existing tests.

1. **Measurement.** Throwaway test: recolour all 14 keys in a loop and read off the achievable frame rate over USB. Report the result to the user before animated themes are promised.
2. **Foundation.** `version` in `deck.json`, migration from version 1, keep unknown fields on save. Split `main.js` into ES modules, central store with undo.
3. **Signal bus and resolver.** Signal store in the core. Resolver for `bind`, `prop`, `tone`, `config`, `steps`, `map`, `prefix`, `suffix`, `fallback` in Rust, with unit tests. `resolve` in `developer-guide.html` serves as the template. Switch the eight stateful icons to binding files.
4. **Registry and core plugin.** Load interfaces, apps and actions as data. `apps.rs` publishes signals instead of drawing. Night Signal as the core's embedded icon set, loaded at runtime instead of compiled in. New Tauri command `catalog`.
5. **Switch the integrations.** `teams.rs`, `ide.rs`, `mqtt.rs` behind the same plugin interface, still in the same process.
6. **Configurator.** According to section 6 of this briefing.
7. **Packages from disk.** Icon sets and themes from a folder in the config directory. Themes of type `partial` with an order. When an icon set loads, the checks from `icon-workbench.html` run, including the comparison of the included interface copy with the installed interface.
8. **Third-party plugins.** Own process, JSON RPC over stdio. Only after asking.

Open points are in `architecture.md`, section 13. Do not decide them alone there. Ask the user.

## 6. Design of the UI

The click dummy under `click-dummy/` shows the behaviour. `Main.dc.html` is a page in the Claude Design Canvas format and runs only there. Read it as a template for layout, spacing and colours. `logic.js` is normal JavaScript and describes the behaviour completely.

### 6.1 Look

Night Signal: very dark surfaces, ivory lines, few accents.

| Purpose | Colour |
| --- | --- |
| App background | `#0B0D10` |
| Surfaces | `#12151A`, `#0F1216` |
| Borders | `#262B33`, emphasised `#3A4350` |
| Text | `#F4F1E8`, quieter `#C9CED6`, hints `#9AA3B0` |
| Selection and target | `#B79AF7` on `#1A1626` |
| On, connected | `#69E0C1` |
| Warning, missing | `#FFBF69` |
| Delete, muted | `#FF8278` |

Font Inter, weights 400 to 700. Controls at least 44 pixels high. Corners 10 to 14 pixels. No gradients, no shadows.

### 6.2 Layout

Header: name, device status, undo.

Below it four areas side by side. In a narrow window they stack.

1. **Sidebar**: Deck, Packages, Signals.
2. **Library** with three tabs.
   - Apps: grouped by namespace. Each row shows default icon, name and fulfilled roles. Tapping or dragging puts the app on the selected key.
   - Actions: grouped by namespace. Tapping appends the action to the section marked as target in the inspector.
   - Icons: only icons that share a role with the app of the selected key. Sorted by the richest shared role. Each preview runs with the real values of this app and the active theme. Below, two buttons: apply a set once, restore the plugins' default icons.
3. **Device**: at the top the profiles as a select, next to it "+ Profile" and "Edit profile". The profile editor folds open below: name, bound programs as tags with remove, programs to add, list of pages with rename, "Set as start page" and delete, "+ Page" and "Delete profile". The first profile is the default and cannot be deleted. A program belongs to at most one profile. The dummy also has a row "Program in foreground", which only serves the simulation. In the middle the deck with 13 square keys in a grid of five columns and the wide key at the bottom right across two columns. Below it the pages as tabs, a hint line and the Theme block.
   - Theme block: a separate select box for the theme of type `full`. It shows a colour swatch (generated from the core variables), name and ID and folds out into a list. Below it the themes of type `partial` as checkboxes with name and the line saying which variable they set. Hint: if two set the same variable, the lower one wins. In the real tool the order should be changeable by dragging.
4. **Inspector** for the selected key, from top to bottom:
   - App: name, ID, line "Fulfils: ..." with the roles.
   - Settings of the app: one select field per field from `config`. Appears only if the app has settings.
   - Icon: preview, ID, line "Reads: ..." with the richest shared role, button "Change icon" (opens the Icons tab), Label field.
   - On press, Long press, Extended (bottom key): one list of actions each, with remove. Long press and Extended are folded until they are used. One section is marked as target.
   - Test press, Clear key.

### 6.3 Behaviour

1. Putting an app on a key sets the default icon, preset actions, label and default values of the settings.
2. Keys can be swapped by dragging.
3. Every change is saved at once. Undo takes back the last step.
4. If a key's plugin is missing, it shows "Plugin missing" with a dashed border and keeps its assignment. The inspector explains this.
5. If the icon set of the selected icon is missing, the app's default icon applies.
6. Test press really runs the actions under On press.
7. Packages view: list of all packages with kind (plugin, icon set, theme, theme of type partial), ID, description and switch. A plugin also shows its state, a guided setup in steps and its settings. The fields are built from the plugin's `config` schema (text, switch, select). The plugin `core` shows the global settings.
8. Below the packages is an Application card with the little that belongs to no plugin: autostart, export, import.
9. Signals view: list of all signals with current value, for colours with a colour swatch. Updates continuously.

### 6.4 Not in the dummy

Cards on the wide key, several states per key, the JetBrains plugin, changing the order of the overlays, editing the parameters of an action and picking a program from the list of running programs. There is no agreed design for these points yet. Ask before you build them.

## 7. Dictionary old to new

| In the repo today | In the target model |
| --- | --- |
| `APPS` in `main.js`, widgets in `apps.rs` | Apps of the plugin `core` that publish signals and fulfil interfaces |
| `ACTIONS` in `main.js` | Actions, registered by plugins |
| `"app": {"type": "sysmon"}` on a key | `"app": "core:sysmon"` plus `"config"` |
| `"icon": "volume-on"` | `"icon": "core:volume-on"` |
| `icon_state` and `setIconState` | Binding file of the icon with `prop` |
| `settings.theme.palette` and `widgets` | Theme variables. The per-widget styles go away |
| `settings.mqtt`, `settings.teams` | Settings of the respective plugin |
| Settings dialog with seven tabs | Settings on the respective plugin in the Packages view |
| Profile select, pages menu, app picker dialog | Profile editor above the device |
| Status key and cards | open, see `architecture.md` section 13 |

## 8. How to check your work

- `cargo test --lib` and `tests/e2e/run.sh` pass.
- The four examples from `developer-guide.html` can be rebuilt with your package formats and behave the same.
- An icon from a foreign namespace works with an app without either of them knowing about the other.
- A theme of type `partial` that binds `accent` to a signal recolours all keys without any plugin setting a colour.
- A `deck.json` in version 1 is migrated, and a key with an unknown app stays unchanged after loading and saving.
