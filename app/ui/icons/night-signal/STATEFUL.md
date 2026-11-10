# Night Signal · eight stateful SVGs

Open preview.html to adjust real inline SVGs. Copy icon-state.js into your app and call setIconState(svgElement, type, state). No dependencies or network requests. All SVGs have a transparent 64 × 64 canvas and individually addressable parts. These files use generic component symbols; they do not control hardware by themselves.

## Controls
| File | CSS variables | Meaning |
| --- | --- | --- |
| volume-muted.svg | --mute-color | Volume-up geometry, plus rotated 45°, coral by default |
| light.svg | --light-fill | Any RGB/CSS color for glass; transparent for off; same outline in both states |
| thermostat.svg | --thermostat-color, --thermostat-progress, --thermostat-number-color | Left thermometer tube color and level 0–100; live temperature text on the right |
| pomodoro.svg | --pomodoro-color, --pomodoro-progress | Ring color and percentage, 0–100 |
| analog-clock.svg | --clock-color, --clock-progress, --clock-fill, --clock-hour-angle, --clock-minute-angle, --clock-second-angle | Ring/second-hand color, ring percentage, optional dial fill and three hand angles (include deg in CSS). Zero degrees points up; clockwise rotation. |
| bambu-ams.svg / bambu-ams-lite.svg | --spool-1-color through --spool-4-color; --spool-1-progress through --spool-4-progress | Four independent colors and remaining-filament percentages |
| downloads.svg | --download-color | Arrow color; tray remains ivory. Grey idle, mint active, or any custom color |

Thermostat uses the simple thermometer at left and a temperature number at right, with no frame, house, dial or progress ring. The percentage fills its tube upward; the bulb retains the chosen color. Set temperature through setIconState(..., {temperature:22.5}), or update [data-part="thermostat-temperature"]. It is SVG text using Inter/Arial/sans-serif for editable numbers; other icon geometry remains font-independent. The degree mark is vector geometry. Temperature text scales down for longer numbers.

Spool order: enclosed AMS left to right 1–4; AMS Lite top-left, top-right, bottom-left, bottom-right. Percentages mean arc fill, NOT liquid/area fill. Progress accepts 0–100 and maps to each ring’s actual circumference, preserving percentages in SVG renderers as well as browsers. Butt caps prevent a false colored dot at 0%. Temperature-to-percentage mapping, timer elapsed vs remaining, and filament telemetry are chosen by the host app. Clock progress is independent of time; bind it to seconds, a countdown, or leave static.

## JavaScript examples
```js
setIconState(lightSvg, 'light', {on:true, color:'#B79AF7'});
setIconState(lightSvg, 'light', {on:false});
setIconState(thermostatSvg, 'thermostat', {color:'#FF8278', progress:68, temperature:22});
setIconState(timerSvg, 'pomodoro', {color:'#FFBF69', progress:42});
setIconState(clockSvg, 'analog-clock', {
  color:'#69E0C1', progress:50,
  time:{hours:10, minutes:10, seconds:30}
});
setIconState(amsSvg, 'bambu-ams', {spools:[
  {color:'#69E0C1', progress:80}, {color:'#B79AF7', progress:65},
  {color:'#FFBF69', progress:40}, {color:'#FF8278', progress:95}
]});
setIconState(downloadSvg, 'downloads', {color:downloading?'#69E0C1':'#87909E'});
```

setIconState clamps percentages and updates both presentation attributes and inline styles, so serializing the SVG preserves the chosen state. CSS variables are useful for untouched inline SVGs; after invoking the helper, its direct style overrides take priority. Choose either CSS binding or the helper for each instance. CSS variables cannot cross an img boundary. Use inline SVG or serialize the modified SVG into an image URL for device rendering. Keep hand transforms intact in vector editors; no raster elements are required. Thermostat temperature uses system font fallback; bundle a font or outline the final text when exact cross-renderer typography is required. Base and track colors can be edited directly in source if needed.

## Complete collection
The updated full collection includes canonical light.svg, pomodoro.svg and analog-clock.svg as well as the existing light-on/light-off, pomodoro-focus and clock filenames for compatibility. Those older names share the new controllable geometry. network-download.svg also receives the independently colored arrow. All unchanged icons are preserved.

Original artwork may be used, edited, and redistributed for personal or commercial projects without attribution. The preview is a design/control demonstration and contains no live hardware integrations.
