# CLAUDE.md

Guide for working on this repository with Claude Code (or any agent).

## What this is

Custom software for the Ulanzi D200 stream controller:

- `device/`: `d200fw`, a Rust replacement for the stock app on the device (ARMv7 Linux). Its
  only dependency is `scene/`, the screensaver scenes (tiny-skia), shared with the app.
- `app/`: D200 Deck, a Tauri v2 desktop app. The Rust engine is in `app/src-tauri/src/`, the
  plain HTML/JS UI in `app/ui/`. It draws all keys and runs the actions.

Read `docs/architecture.md` first, then `docs/config.md` and `docs/protocol.md`.

The target model (Deckling: plugins, icon sets, themes, signals, bindings) is in
`docs/deckling/architecture.md` and is normative. The work plan is `docs/deckling/briefing.md`.

## Commands

```sh
# device binary (cross build, links with rust-lld; no cross gcc needed)
cd device && cargo build --release
cp device/target/armv7-unknown-linux-musleabihf/release/d200fw bin/d200fw   # ALWAYS after device changes

# app
cd app && npm install
npx tauri dev                    # run with UI reload
npx tauri build                  # bundle for this OS
cd app/src-tauri && cargo test --lib

# end-to-end (simulated device, fake broker/Teams/IDE; no hardware)
tests/e2e/run.sh

# the configurator in Chromium against the real engine (needs Node and Playwright)
tests/ui/run.sh
cd app/src-tauri && cargo run --example uibridge -- /tmp/deck.json 8790   # UI in a browser

# release: Actions → CI → Run workflow with release=1.2.3 (or push tag v1.2.3);
# the version comes from the tag. Every release is tagged and lists its changes.
scripts/set-version.sh 1.2.3     # sets Cargo.toml/lock, tauri.conf.json, package(-lock).json
```

## Conventions

- **`deck.json` has a version.** A change to its shape bumps `config::VERSION` and adds a step
  in `migrate.rs` with a test. Unknown fields must survive load and save (`extra` maps). Update
  `default-config.json`, `testdata/ui-config.json` and `docs/config.md` with it.
- **The engine is one thread.** All state lives in `engine.rs`, and everything else sends `Msg`s
  to it. Don't add shared mutable state.
- **Colours come from the theme.** Use `Th::resolve(theme, widget, tone)`. Never hard-code hex
  values in widgets or cards. Follow the Night Signal tokens in `docs/design/README.md`.
- **Keep the UI build-free.** Plain JS ES modules in `app/ui/js/`, `withGlobalTauri`, no bundler
  and no frameworks. Every config edit goes through `store.edit()` (undo and saving).
- **Icons:** the SVGs in `app/ui/icons/night-signal/svg/` are compiled in by `build.rs`. To add
  an icon, add the SVG and add it to `manifest.json`. `icons.rs` tests that every icon renders.
  An icon with changing parts gets a binding file in `icons/` (see `docs/packages.md`);
  `registry.rs` tests that the built-in packages load without warnings.
- **Plugins never set colours or listen to keys.** Values go out as signals; icons and themes
  read them through bindings. Don't add per-icon state code to `icons.rs`.
- The UI text is in English and short. Docs are in English.

## Gotchas

- **CSP:** `tauri.conf.json` has a strict CSP. `fetch()` of local files is blocked, so the UI
  gets the icon list from the Rust command `icon_set`. Add Rust commands for data; don't loosen
  the CSP.
- **`replaceChildren(x)` with `null`** inserts the text "null". Use the `put()` helper in
  `js/dom.js`.
- **Upper-case local names** in Rust `let` patterns are read as constants. Keep locals lower
  case.
- **resvg ignores CSS `calc()`/`var()` and `pathLength`.** Bound icons are changed as strings
  in `icons.rs` (`strip_css`, `apply_parts`). Edit the attributes, not the styles.
- **adb shell** gives no exit codes, and kills background jobs when it ends. Echo a marker and
  check it, and detach jobs with `setsid`. See `docs/hardware.md`.
- `d200fw` writes `/tmp/app.pid` only when it stops the vendor app (on the device). Tests run it
  with `--no-kill`.
- **Don't trigger macOS prompts in tests.** Use the test hooks:
  - `D200_DIRECT=1` with `D200_PORT` skips adb.
  - `D200_MEDIA_FILE` replaces AppleScript to Spotify/Music.
  - `D200FW_HOST_BIN` and `D200_HEADLESS` point the e2e tests at prebuilt binaries.
- The app embeds `bin/d200fw` with `include_bytes!`. If you change `device/` and don't copy the
  new binary to `bin/`, the app ships the old one.

## Hardware facts (short)

- Framebuffer: 720x1280 XRGB, rotated. Landscape (x, y) maps to fb (y, 1279 - x).
- Keys 0-12 are 196x196, key 13 is 456x196 at (791, 521).
- Keycodes in key order: `29 15 14 13 12 11 10 9 8 7 34 33 31 30`.
- Boot hang recovery: hold the four corner keys while powering on.

## Not tested yet

See `docs/status.md`: real Teams and Graph, Windows/Linux, the D200H, the bundled app after the
redesign.
