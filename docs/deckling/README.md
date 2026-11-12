# Deckling: complete package

As of 10 November 2026. Repo: https://github.com/ricktap/ulanzi-d200-cfw

| File | Content |
| --- | --- |
| `briefing.md` | Task, rules, work packages and UI design for an AI. Start here |
| `architecture.md` | Target model, normative. In case of conflict this file wins |
| `developer-guide.html` | Developer guide with four running examples. Open in a browser |
| `icon-workbench.html` | Working tool: load an icon and check it against an interface |
| `diagrams/packages-and-core.svg` | Who talks to whom |
| `diagrams/signal-to-pixel.svg` | Path of a value from the signal into the icon |
| `click-dummy/Main.dc.html` | Configurator as click dummy (Claude Design Canvas format, runs only there) |
| `click-dummy/logic.js` | Behaviour of the click dummy as readable JavaScript |
| `click-dummy/canvas.json` | Index of the canvas |

The HTML files, the click dummy and the diagrams are in German.

## For the AI

Give the AI the whole folder and start with: "Read briefing.md and follow it."

## Not decided yet

See `architecture.md`, section 13. The main points: measure the frame rate over USB, smooth colour transitions, cards on the wide key, several states per key, licence.
