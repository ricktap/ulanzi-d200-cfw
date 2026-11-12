"""Web API only: brightness."""

from __future__ import annotations

from homeassistant.components.number import NumberEntity, NumberMode

from .entity import StateEntity, web_only


async def async_setup_entry(hass, entry, async_add_entities) -> None:
    if web_only(entry):  # over MQTT the deck's own device has it
        async_add_entities([Brightness(entry.runtime_data)])


class Brightness(StateEntity, NumberEntity):
    _attr_native_min_value = 0
    _attr_native_max_value = 255
    _attr_native_step = 1
    _attr_mode = NumberMode.SLIDER

    def __init__(self, bridge) -> None:
        super().__init__(bridge, "brightness", icon="mdi:brightness-6")

    @property
    def native_value(self) -> float | None:
        return self.get()

    async def async_set_native_value(self, value: float) -> None:
        await self.bridge.run("core:brightness", {"value": str(int(value))})
