"""Web API only: remote page, screensaver, display, follow the program in front."""

from __future__ import annotations

from homeassistant.components.switch import SwitchEntity

from .entity import StateEntity, web_only


async def async_setup_entry(hass, entry, async_add_entities) -> None:
    if not web_only(entry):
        return  # over MQTT the deck's own device has these
    b = entry.runtime_data
    async_add_entities([
        Switch(b, "remote_page", "remote", icon="mdi:remote", on=("com.deckling.ha:remote", {}), off=("core:back", {})),
        Switch(b, "screensaver", icon="mdi:weather-night", action="core:screensaver"),
        Switch(b, "display", icon="mdi:monitor", action="core:display"),
        Switch(b, "follow", icon="mdi:application-cog", action="core:follow", config=True),
    ])


class Switch(StateEntity, SwitchEntity):
    def __init__(self, bridge, key, field="", action: str = "", on=None, off=None, **kw) -> None:
        super().__init__(bridge, key, field, **kw)
        self.on_action = on or (action, {"on": "on"})
        self.off_action = off or (action, {"on": "off"})

    @property
    def is_on(self) -> bool | None:
        v = self.get()
        return None if v is None else bool(v)

    async def async_turn_on(self, **kwargs) -> None:
        await self.bridge.run(*self.on_action)

    async def async_turn_off(self, **kwargs) -> None:
        await self.bridge.run(*self.off_action)
