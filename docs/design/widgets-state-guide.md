# Night Signal · widget handoff

This is a design prototype with simulated values and events; there are no live integrations. Open preview.html in a browser. The wide button has a 2.3:1 aspect ratio and square widgets are 1:1. Preview UI controls sit outside the device artwork. Graphs are illustrative rather than recorded measurements. Pixel-size check uses editable illustrative defaults of 128px square and 320px wide; these are NOT verified hardware dimensions or a physical-size calibration. Enter actual render widths. Sample covers are original abstract placeholders; real album/game images must use object-fit:cover in the same edge-to-edge slot. Sample clock intentionally begins at 10:42 on 10 November.

## Visual and motion rules
Near-black #11171D surface, ivory #F4F1E8 primary type, #9BA5B2 secondary type. Mint #69E0C1 active, lavender #B79AF7 informational, amber #FFBF69 attention, coral #FF8278 errors. State names and symbols supplement color. Hardware-first layout: square padding 5% of width, wide covers fill the left 42% edge-to-edge, text uses a 3%-width inset, and non-cover app symbols occupy 27%. All artwork typography scales with the tile width using container units. Two-line titles and one secondary line replace tiny footer metadata. Two shipped layout variants: Compact uses edge-to-edge 42%-width covers and minimal padding; Airy uses 24%-width square covers, 4%-width outer padding plus spacing, and restores short metadata. Both share the same provider data, automatic states, directional lighting and combined network tile. Choose per widget rather than forcing one style everywhere. Both support the pixel-size check. Thin progress tracks, one dominant value/title, brief secondary text. Detailed metadata remains in the editor/expanded view, not on the key. Truncate long titles to two lines. Do not scroll by default; optionally reveal overflow on press. Keep the large button's profile context available in the editor even while an app is displaying.

Wide-card state emphasis uses a soft directional light spilling from the upper-left, tinted to the current accent; there is no side stripe. Fade the light gently once on arrival; preserve dark contrast behind text.

Arrival: one 250–350 ms slide/fade with a 5px travel, then hold still. Never flash repeatedly. Crossfade state changes in 180 ms in production. Progress updates interpolate for up to 1 second. Equalizer only animates during confirmed playback. Pause it on pause, stale data, or loss of connection. An equalizer is an activity indicator, not measured audio levels. Indeterminate tracks only for actual unmeasured launch/heating; never show a made-up percentage. Respect reduced motion; retain text and progress without decorative motion. The prototype starts paused for reduced-motion users.

## Square screens
- CPU: percentage primary; optional temperature only if supplied by the host. Recent utilization history, 1-second sample target.
- RAM: used percentage; used / total memory in consistent units, 1–2-second sample target.
- Disk: used percentage; used / total for a named selected volume. Poll about every 30 seconds; no busy chart.
- Network: upload and download share ONE square, with upward mint and downward lavender arrows, separately labelled rates, units, and two thin sparklines. Rates come from byte-counter deltas and elapsed monotonic time. Display MB/s consistently; support KB/s and GB/s when needed. Recent 30-second history, 1-second sample target.
- Clock: HH:mm, local date; optional seconds track. Use configured timezone and 12/24-hour setting. Update minute or second track from actual time.

Poll targets are design defaults, not integration promises. When data is stale, freeze its last value, replace the activity indicator with “Updated 2m ago”, and render a muted state; when no valid value exists, show an em dash and “Unavailable”. Zero is a legitimate measurement and must not be used to represent failure.

## Wide apps
| App | Automatic states | Trigger/source | Interaction proposal |
| --- | --- | --- | --- |
| Email | Arriving → unread → cleared; disconnected | Provider new-message/unread events, only clear on confirmed read/archive | Short press opens inbox/message; long press dismisses this banner only |
| Spotify | Playing ↔ paused → idle; disconnected | Track/playback/position events | Short press play/pause; long press opens Spotify |
| Printer | Heating → printing ↔ paused → complete; error/offline | Printer telemetry; real job percentage and estimated remaining time | Short press details; long press notification dismissal only |
| Next meeting | Upcoming → soon (5m) → live → ended/next; canceled | Calendar times plus changes/cancellations, not evidence of call participation | Short press opens/join URL; long press dismisses reminder |
| Steam game | Launching → playing → exited → idle; offline | Steam or local process session events | Short press game/session; long press returns to profile |
| Push | Arriving → visible → dismissed/expired | Home Assistant or app notification payload | Short press details; long press dismisses |

Keep app availability and provider permissions explicit in implementation; these illustrations do not assert API support. Never infer “meeting joined” solely from its scheduled time. Never infer print completion from estimated remaining time. A missed meeting can be labeled “Started” until its scheduled end. Canceling print jobs is deliberately not a default long-press action.

## Wide-display ownership
The production wide button displays ONE app or notification at a time. The six-card preview shows all designs for review. Default is the configured app (for example Spotify) or profile. Keep its state running in the background during a notification.

Proposed notification priority: explicit critical alert > meeting starting > printer complete/error > push > email. Do not treat every push as critical; priority is explicit in payload. Queue lower-priority arrivals without interrupting a higher-priority card. Coalesce email into unread count and printer notifications by job; expire stale meeting reminders. Show ordinary notifications for 8 seconds, then restore the current state of the underlying app. Critical alerts remain until acknowledged with a visible dismiss affordance. Short press and long press act on the visible card. Avoid accidental destructive actions when a notification arrives during a hold: capture the visible card at press start.

Provider-driven state changes are immediate; the prototype's 6-second cycling is purely a demo. Profile changes should not replay previously acknowledged notifications. Paused playback has a frozen real position. Steam playtime is session duration, not a completion percentage. For privacy, provide notification content hiding (e.g. “New email” instead of sender/subject).


## Theme integration
All widget accents and background lighting are CSS-controlled. See THEMING.md and shared theme.css. State rendering sets data attributes only; it does not inject inline accent colors. Outline icons are inline SVG so they inherit the theme.
