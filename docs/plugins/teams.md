# Microsoft Teams (`com.deckling.teams`)

Meeting controls through Teams' local API, and presence, mail and calendar through Microsoft 365.

## Apps

| App | On the key | Press |
|---|---|---|
| Teams mic | muted or not, in a meeting | mute or unmute |
| Teams camera | camera on or off | camera on or off |
| Teams presence | your availability | switch between available and unavailable (long press: automatic) |

## Actions

`meeting` (mute, camera, raise hand, leave, ...) and `presence` (toggle, set, automatic).

## Cards on the wide key

A **new mail** (sender and subject; hidden under Packages, Deck, Cards if you like) and a
**meeting** starting soon, with a press to join. Both come from the signals below.

## Setup (Packages, Microsoft Teams, Settings)

1. **Meeting controls:** in Teams, Settings, Privacy, Manage API: switch on the API for
   third-party apps. The first time you press a Teams key in a meeting, Teams asks to allow
   D200 Deck.
2. **Presence, mail and calendar (optional):** register an app in Microsoft Entra (single
   tenant, allow public client flows) with the delegated Microsoft Graph permissions
   `Presence.ReadWrite`, `Mail.ReadBasic` and `Calendars.Read`. Enter its client id and your
   tenant, then sign in with the code shown. Tokens stay in `teams-auth.json` next to deck.json.

Settings: API port (8124), client id, tenant, and what "unavailable" means (do not disturb,
busy, away, be right back, appear offline).

## Signals

`connected`, `paired`, `in_meeting`, `mic_on`, `video`, `meeting.*`, `signed_in`, `account`,
`presence`, `mail.unread`, `mail.latest.*`, `calendar.<n>.*` (the next 10 events).

Code: [app/src-tauri/src/plugins/teams](../../app/src-tauri/src/plugins/teams/).
