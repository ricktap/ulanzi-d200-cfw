# CSS theme API · Compact + Airy

`theme.css` is loaded AFTER the layout styles in preview.html, preview-compact.html and preview-airy.html. Edit this one file to theme both layouts. No build step or JavaScript changes required. Color values accept normal CSS colors: hex, rgb(), hsl(), oklch(), transparent, etc. The preview uses modern browser CSS (container units and color-mix).

## Global semantic palette
```css
:root {
  --accent-active: #74b9ff;
  --accent-info: #c7a8ff;
  --accent-attention: #ffd166;
  --accent-error: #ff6b81;
  --widget-background: #101820;
}
```
States select a semantic tone with data-tone="mint|lavender|amber|coral". These names are semantic aliases, not enforced colors. Setting --accent-active changes active text, icon accents, graphs, progress bars, equalizers AND the background light. Automatic state changes never overwrite CSS colors.

## Per-widget variables
Set these on a widget, an ancestor, or :root:

| Variable | Controls |
| --- | --- |
| --widget-accent | Override that widget's accent across every state |
| --widget-light | Independent background light color; defaults to effective widget accent |
| --widget-light-opacity | Light intensity, 0–1; default .9; 0 disables it |
| --widget-idle-light-factor | Idle light multiplier, default .28 |
| --widget-background | Solid surface below the light |
| --widget-border | Widget outline |
| --widget-text | Primary text |
| --widget-secondary | Description, metadata and sublabels |
| --widget-icon-color | Neutral outline icon strokes |
| --widget-icon-accent | Colored outline icon strokes/fills; defaults to widget accent |
| --widget-chart-color | Optional override for chart lines |
| --widget-progress-color | Progress tracks' active section and equalizer |
| --widget-track | Inactive progress track |
| --widget-equalizer-background | Background behind equalizer bars |
| --network-up-color | Upload arrow, graph and upload icon accent |
| --network-down-color | Download arrow and graph; independent of upload |
| --cover-background | Backdrop behind album/game artwork |
| --cover-tint | Optional color blend over artwork; transparent by default |

Stable data-widget values: cpu, ram, disk, network, clock, mail, music, printer, meeting, game, push.

```css
[data-widget="music"] {
  --widget-accent: #c7a8ff;
  --widget-light: #8970dd;
  --widget-light-opacity: .65;
}
[data-widget="network"] {
  --network-up-color: #69e0c1;
  --network-down-color: #74b9ff;
}
[data-widget="printer"][data-state="error"] {
  --widget-accent: #ff5577;
  --widget-light: #ff3355;
}
```
Override priority: per-widget public variable > inherited public variable > semantic state palette. Background light defaults to the effective accent unless --widget-light overrides it. No !important required. Use state-specific overrides when you want to preserve warning/error distinctions.

Line icons are inline SVG; their colors inherit these CSS variables. Cover artwork keeps its own image colors; use --cover-tint when a tint is desired. The interactive color pickers set root variables as a convenience and are not required by the theme. Reset theme removes those inline root overrides, restoring your theme.css values. Reloading also clears picker overrides. State changes and switching Compact/Airy do not reset colors.
