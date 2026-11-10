# Night Signal — U200 SVG icon collection

A collection of custom vector icons, designed for dark button displays. App icons are original line-style interpretations for recognition, not official brand assets. Product and company names belong to their owners. Bambu machine silhouettes are illustrative rather than technical drawings.

## Files
- `svg/`: Individually named standalone transparent SVG files, each 64 × 64 with a shared viewBox.
- `contact-sheet.png` / `contact-sheet.svg`: labelled overview.
- `gallery.html`: offline searchable gallery; open in a browser. Click an icon to download it.
- `sprite.svg`: all icons as named symbols.
- `manifest.json`: icon names, categories, palette, and geometry settings.

## Editing
Drag any standalone SVG into Figma, Illustrator, Inkscape, or another vector editor. Paths and shapes remain editable; no raster content or required fonts. Weather and door icons use named SVG masks to hide background strokes beneath foreground objects, preserving transparent backgrounds. Move each mask with its foreground artwork when rearranging those layers. Use unique mask IDs when inserting multiple copies inline. Ungroup to reposition parts. All icon backgrounds are transparent. Preview on a dark surface such as #101419.

Artwork uses a 2.4-unit rounded stroke on a 64-unit canvas. Scale uniformly. Icons are static and animation-ready; add animation in your app. Semantic color parts are marked with `data-color` attributes, inside `g#artwork`. Accent color is not the only state cue: on/off variants also change geometry.

Palette: ivory #F4F1E8, mint #69E0C1, coral #FF8278, amber #FFBF69, lavender #B79AF7. For color changes in a vector editor, select matching strokes/fills. In source, replace the hexadecimal fallback colors (also replace matching fill colors on solid dots).

When inlined in HTML, stroke colors can be overridden with --icon-base, --icon-mint, --icon-coral, --icon-amber, --icon-lavender. These variables do not cross an img boundary; inline the SVG or edit the file. Solid filled details retain their explicit fill colors; edit those separately. Scope each inline SVG's style rules to that SVG when combining independently themed instances. Set meaningful labels in your application; decorative icons should be hidden from assistive technology. Inline copies should use unique title IDs.

Sprite example: <svg viewBox="0 0 64 64"><use href="sprite.svg#volume-on"/></svg>. Serve over HTTP when using external sprites; local file browser restrictions vary. Standalone SVGs work independently.

Recommended button composition: place icon above a short state label; retain margins. Keep working state separate from pending/unavailable state in the application. This set contains artwork only and implements no integrations.

You may use, modify, and redistribute this artwork for personal or commercial projects. No attribution is required.


This package contains 215 icons. Badge and foreground masks preserve transparent backgrounds; move masks with their artwork. Give masks and title elements unique IDs when inlining repeated copies.


# Night Signal — three dynamic controls

Three transparent editable SVGs; the app chooses the state with four desk-light colors and two colors per display icon. No power symbols or state-specific SVGs.

| SVG | First color variable | Second color variable |
| --- | --- | --- |
| desk-lights.svg | --desk-light-upper-1 (2 bars), --desk-light-upper-2 (3 bars) | --desk-light-lower-1 (2 bars), --desk-light-lower-2 (3 bars) |
| ultrawides.svg | --screen-left | --screen-right |
| ultrawides-backlight.svg | --backlight-left | --backlight-right |

On: #69E0C1 (desk/screen), #B79AF7 (monitor backlighting). Off: #87909E. Any custom colors work. Monitor 1 is left, monitor 2 is right. Default state: upper desk bars on in mint, lower bars off in grey; left screen/backlight on, right off.

Inline SVG example:
```html
<svg style="--screen-left:#87909E;--screen-right:#69E0C1" ...>
  <!-- Paste ultrawides.svg contents here -->
</svg>
```

CSS variables do not cross an <img> boundary; inline the SVG, or edit the source before loading it. Each part is a named <g> with a `data-part` attribute and no fixed child stroke colors. Desk groups have IDs desk-light-upper-1, desk-light-upper-2, desk-light-lower-1, desk-light-lower-2; data-part values are upper-1, upper-2, lower-1, lower-2. Group 1 controls the second and fourth bars; group 2 controls the first, middle, and fifth bars, counting left to right. Set groups 1 and 2 to the same color for monochrome, or different colors for alternating multicolor. Each row has five bars, placed close to the desktop. The upper row arches upward and the lower row mirrors it downward: each middle bar is offset two units away from the desktop and its neighbors one unit, relative to the outer bars. Upper bars fan outward at -12, -6, 0, 6, 12 degrees from left to right; lower bars mirror these rotations to shine down and outward. The light bars use 1.6-unit strokes. Screen and backlight group IDs and data-part values match their CSS variable names without the leading --.

You can also directly change the group style.stroke in JavaScript, or replace BOTH its stroke attribute and style value in source/vector editors. Preserve unique IDs when embedding multiple copies in a page. Neutral monitor outlines and stands in the backlight icon stay ivory; only the rays are state-colored. In ultrawides.svg the complete left and right monitor silhouettes change color independently.

Canvas 64 × 64; 2.4-unit rounded outlines and 1.6-unit desk light bars. Original editable vector artwork. Use, modify, and redistribute for personal or commercial projects; no attribution required.
