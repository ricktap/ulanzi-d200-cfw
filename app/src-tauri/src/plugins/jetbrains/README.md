# com.deckling.jetbrains

Whether a JetBrains IDE is debugging.

| File | What |
|---|---|
| [mod.rs](mod.rs) | the plugin: scans while a JetBrains IDE is in front (or every few seconds where the front app is unknown) |
| [ide.rs](ide.rs) | the process scans: a debugger attached to a JetBrains IDE, and the Steam game that is running (used by the core's Steam card) |
| [package/](package/) | manifest, signals, the Debugger app |

**App:** Debugger. Its face shows the IDE that is debugging; a press sends the IDE's resume or
debug shortcut (set on the key).

**Signals:** `debugger.active`, `debugger.name`.
