# com.deckling.teams

Microsoft Teams meetings and Microsoft 365.

| File | What |
|---|---|
| [mod.rs](mod.rs) | the plugin: the meeting link, sign-in, polling presence, mail and calendar; readers the cards and widgets use |
| [api.rs](api.rs) | the protocols: Teams' local third-party API (WebSocket) and Microsoft Graph with a device-code sign-in |
| [package/](package/) | manifest with settings (API port, Entra client id, tenant, what "unavailable" means), signals, actions, apps |

**Apps:** Teams mic, Teams camera, Teams presence.

**Actions:** `meeting` (mute, camera, hand, leave, ...), `presence` (toggle, set, auto).

**Signals:** `connected`, `paired`, `in_meeting`, `mic_on`, `video`, `meeting.*`, `signed_in`,
`account`, `presence`, `mail.*` (unread count, latest sender and subject), `calendar.<n>.*`
(the next 10 events).

**Setup** (also shown step by step under Packages):
1. In Teams: Settings, Privacy, Manage API: switch on the API for third-party apps. Teams asks
   to pair the first time a Teams key is pressed in a meeting.
2. For presence, mail and calendar: register an app in Microsoft Entra (single tenant, public
   client flows) with the delegated Graph permissions `Presence.ReadWrite`, `Mail.ReadBasic`
   and `Calendars.Read`, enter its client id and tenant, and sign in. Tokens are kept in
   `teams-auth.json` next to deck.json, readable only by you.

The new-mail and meeting cards on the wide key are drawn by the core from these signals.
