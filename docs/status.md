# Status

Draft. The config format may still change, and there is no migration (see [config.md](config.md)).

## Works and is tested

- `d200fw` on a real D200: screen, rotation, keys, taking over from the vendor app, boot prompt,
  `install.sh` / `uninstall.sh`.
- Desktop app on macOS against a simulated device: 18 unit tests and 61 end-to-end checks
  (see [testing.md](testing.md)).
- MQTT and Home Assistant discovery against a minimal test broker.

## Built but not tested against the real thing

- Teams meeting controls against a real Teams meeting. They are tested against a fake of the
  local API.
- Teams presence, mail and calendar cards through Microsoft Graph against a real tenant.
- Windows and Linux builds. The code paths exist; see the platform table in [app.md](app.md).
- The bundled `.dmg` app, clicked through by hand after the latest UI changes.
- The D200H (RK3308H3). It is expected to match the D200. Run `scripts/test-keys.sh` to check.

## Known limits

- Browser media (YouTube etc.) can't be read: since macOS 15.4 only Apple's apps can read
  "Now Playing".
- JetBrains has no debugger API. The key detects debuggee processes and sends shortcuts.
- Teams presence needs an Entra app registration that may need admin consent.
- Keyboard shortcuts on macOS need the Accessibility permission.
- While `d200fw` runs, Ulanzi Studio can't talk to the device.

## Ideas / next steps

- Try the bundled app on a real device after the redesign.
- Check Teams and Graph against a real account.
- A `LICENSE` file, CI (GitHub Actions: `cargo test`, e2e on macOS), and release builds of the
  app.
