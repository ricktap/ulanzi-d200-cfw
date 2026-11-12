# JetBrains (`com.deckling.jetbrains`)

Whether a JetBrains IDE (IntelliJ IDEA, WebStorm, PyCharm, GoLand, PhpStorm, Rider, CLion,
RustRover, DataGrip) is debugging.

## App

**Debugger**: shows which IDE is debugging. A press sends the IDE's resume shortcut while it
debugs and its debug shortcut otherwise; both shortcuts are settings on the key.

It looks only while a JetBrains IDE is the program in front (on systems that cannot tell, every
few seconds), so it costs nothing otherwise.

## Signals

`debugger.active`, `debugger.name`.

Code: [app/src-tauri/src/plugins/jetbrains](../../app/src-tauri/src/plugins/jetbrains/) (the
process scan in `ide.rs` also finds the Steam game for the wide key's Steam card).
