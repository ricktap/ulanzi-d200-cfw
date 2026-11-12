"""Web API only: clear notification, Pomodoro start or pause, Pomodoro reset."""

from __future__ import annotations

from homeassistant.components.button import ButtonEntity

from .entity import StateEntity, web_only


async def async_setup_entry(hass, entry, async_add_entities) -> None:
    if not web_only(entry):
        return  # over MQTT the deck's own device has these
    b = entry.runtime_data
    async_add_entities([
        Button(b, "notify_clear", "core:notify_clear", {}, icon="mdi:bell-off"),
        Button(b, "pomodoro_toggle", "com.deckling.core:pomodoro", {"do": "press"}, icon="mdi:timer-play"),
        Button(b, "pomodoro_reset", "com.deckling.core:pomodoro", {"do": "reset"}, icon="mdi:timer-refresh"),
    ])


class Button(StateEntity, ButtonEntity):
    def __init__(self, bridge, key, action, params, **kw) -> None:
        super().__init__(bridge, key, **kw)
        self.action, self.params = action, params

    async def async_press(self) -> None:
        await self.bridge.run(self.action, self.params)
